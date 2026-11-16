use super::*;
use crate::workbench::{
    WorkbenchCommand,
    workspace::{self, PanelGroup, Placement},
};
enum LayoutAction {
    Place(String, Placement),
    Group(String, String),
    Ungroup(String),
    Save,
    Hide(String),
}
impl AtelierApp {
    fn native_panel_visible(&self, id: &str) -> bool {
        match id {
            workspace::TOOLBOX => self.show_left,
            workspace::NAVIGATOR => self.show_right,
            _ => true,
        }
    }
    fn workspace_group_contents(
        &mut self,
        ui: &mut Ui,
        group: &mut PanelGroup,
    ) -> Option<LayoutAction> {
        let language = self.language;
        let visible = group
            .panels
            .iter()
            .filter(|p| self.native_panel_visible(p))
            .cloned()
            .collect::<Vec<_>>();
        let active = visible
            .iter()
            .find(|p| **p == group.active && workspace::panel_label(p).is_some())
            .or_else(|| visible.iter().find(|p| workspace::panel_label(p).is_some()))
            .or(visible.first())
            .cloned();
        let mut action = None;
        let idle = !self.workbench_pointer_active() && !self.menu_gated();
        let controls_start = ui.cursor().min.y;
        ui.horizontal_wrapped(|ui| {
            for id in &visible {
                let label = workspace::panel_label(id)
                    .map(|s| language.translate(s))
                    .unwrap_or(id);
                let response = ui
                    .add_enabled(
                        idle && workspace::panel_label(id).is_some(),
                        Button::new(label).selected(active.as_ref() == Some(id)),
                    )
                    .on_hover_text(language.translate("Right-click panel title for layout"));
                if response.clicked() {
                    group.active = id.clone();
                }
                response.context_menu(|ui| {
                    if let Some(next) =
                        self.workspace_layout_menu(ui, group, active.as_deref(), idle)
                    {
                        action = Some(next);
                    }
                });
            }
        });
        ui.separator();
        let layout_header_height = ui.cursor().min.y - controls_start;
        if let Some(active) = active {
            ui.add_enabled_ui(!self.workbench.configuration_open, |ui| {
                match active.as_str() {
                    workspace::TOOLBOX => self.toolbox_contents(ui),
                    workspace::NAVIGATOR => self.inspector_contents(ui, layout_header_height),
                    _ => {
                        ui.label(
                            language
                                .translate("Panel provider is unavailable; saved slot retained"),
                        );
                        ui.label(active);
                    }
                }
            });
        }
        action
    }
    fn workspace_layout_menu(
        &self,
        ui: &mut Ui,
        group: &PanelGroup,
        active: Option<&str>,
        idle: bool,
    ) -> Option<LayoutAction> {
        let language = self.language;
        let mut action = None;
        for place in [Placement::Left, Placement::Right, Placement::Floating] {
            if ui
                .add_enabled(idle, Button::new(language.translate(place.label())))
                .clicked()
            {
                action = Some(LayoutAction::Place(group.id.clone(), place));
                ui.close();
            }
        }
        if let Some(active) = active {
            for (target, label) in [
                (workspace::TOOLBOX, "Group with Toolbox"),
                (workspace::NAVIGATOR, "Group with Navigator"),
            ] {
                if !group.panels.iter().any(|p| p == target)
                    && ui
                        .add_enabled(idle, Button::new(language.translate(label)))
                        .clicked()
                {
                    action = Some(LayoutAction::Group(active.to_owned(), target.into()));
                    ui.close();
                }
            }
            if group.panels.len() > 1
                && ui
                    .add_enabled(
                        idle,
                        Button::new(language.translate("Ungroup active panel")),
                    )
                    .clicked()
            {
                action = Some(LayoutAction::Ungroup(active.to_owned()));
                ui.close();
            }
            if workspace::panel_label(active).is_some()
                && ui
                    .add_enabled(idle, Button::new(language.translate("Hide active panel")))
                    .clicked()
            {
                action = Some(LayoutAction::Hide(active.to_owned()));
                ui.close();
            }
        }
        ui.separator();
        if ui
            .add_enabled(
                idle,
                Button::new(language.translate("Save workspace layout")),
            )
            .clicked()
        {
            action = Some(LayoutAction::Save);
            ui.close();
        }

        action
    }
    pub(super) fn native_workspace_ui(&mut self, ui: &mut Ui) {
        #[cfg(test)]
        self.workbench.panel_rects.clear();
        let ctx = ui.ctx().clone();
        let bounds = ui.available_rect_before_wrap();
        let viewport = [
            bounds.min.x.round() as i32,
            bounds.min.y.round() as i32,
            bounds.width().round() as i32,
            bounds.height().round() as i32,
        ];
        let mut workspace = std::mem::take(&mut self.workbench.workspace);
        if workspace.recover(viewport) {
            self.workbench.layout_generation = self.workbench.layout_generation.wrapping_add(1);
        }
        let generation = self.workbench.layout_generation;
        let idle = !self.workbench_pointer_active() && !self.menu_gated();
        let mut action = None;
        // Docks consume the parent Ui before the central canvas; floating windows never do.
        for group in &mut workspace.groups {
            if !group.panels.iter().any(|id| self.native_panel_visible(id)) {
                continue;
            }
            let id = Id::new(("workspace_group", generation, &group.id));
            match group.placement {
                Placement::Left | Placement::Right => {
                    let maximum = (bounds.width() * 0.45).clamp(80., 2048.);
                    let panel = if group.placement == Placement::Left {
                        egui::Panel::left(id)
                    } else {
                        egui::Panel::right(id)
                    };
                    let output = panel
                        .default_size(group.extent.min(maximum as u32) as f32)
                        .size_range(80.0..=maximum)
                        .resizable(idle)
                        .frame(
                            Frame::new()
                                .fill(PANEL)
                                .inner_margin(Margin::symmetric(8, 8)),
                        )
                        .show(ui, |ui| self.workspace_group_contents(ui, group));
                    #[cfg(test)]
                    self.workbench
                        .panel_rects
                        .insert(group.id.clone(), output.response.rect);
                    // Pinned egui0.36.2 Panel::resize_widget_id: record user resize only.
                    // Intrinsic/minimum widget width and viewport constraints are not personal edits.
                    if ctx
                        .read_response(id.with("__resize"))
                        .is_some_and(|r| r.dragged() || r.drag_stopped())
                    {
                        group.extent =
                            (output.response.rect.width().round() as u32).clamp(80, 2048);
                    }
                    if output.inner.is_some() {
                        action = output.inner;
                    }
                }
                Placement::Floating => {
                    let rect = group.floating;
                    let output =
                        egui::Window::new(self.language.translate("Workspace panel group"))
                            .id(id)
                            .default_pos(pos2(rect.x as f64, rect.y as f64))
                            .default_size(vec2(rect.width as f64, rect.height as f64))
                            .min_size(vec2(32., 32.))
                            .max_size(vec2(
                                bounds.width().max(32.) as f64,
                                bounds.height().max(32.) as f64,
                            ))
                            .movable(idle)
                            .resizable(idle)
                            .collapsible(false)
                            .constrain_to(bounds)
                            .show(&ctx, |ui| {
                                let result = ScrollArea::vertical()
                                    .id_salt("workspace_float_scroll")
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| self.workspace_group_contents(ui, group));
                                result.inner
                            });
                    if let Some(output) = output {
                        #[cfg(test)]
                        self.workbench
                            .panel_rects
                            .insert(group.id.clone(), output.response.rect);
                        group.floating.x = output.response.rect.min.x.round() as i32;
                        group.floating.y = output.response.rect.min.y.round() as i32;
                        group.floating.width =
                            (output.response.rect.width().round() as u32).clamp(32, 8192);
                        group.floating.height =
                            (output.response.rect.height().round() as u32).clamp(32, 8192);
                        if let Some(result) = output.inner {
                            if result.is_some() {
                                action = result;
                            }
                        }
                    }
                }
            }
        }
        if let Some(action) = action {
            let mut new_generation = false;
            let result = match action {
                LayoutAction::Place(id, p) => {
                    new_generation = true;
                    workspace.place(&id, p)
                }
                LayoutAction::Group(panel, target) => {
                    new_generation = true;
                    workspace.group_with(&panel, &target)
                }
                LayoutAction::Ungroup(panel) => {
                    new_generation = true;
                    workspace.ungroup(
                        &panel,
                        &format!("workspace.{}", crate::storage::new_document_identity()),
                    )
                }
                LayoutAction::Hide(panel) => {
                    match panel.as_str() {
                        workspace::TOOLBOX => self.show_left = false,
                        workspace::NAVIGATOR => self.show_right = false,
                        _ => {}
                    };
                    Ok(())
                }
                LayoutAction::Save => {
                    self.workbench.workspace = workspace;
                    self.dispatch_workbench(WorkbenchCommand::SaveWorkspace);
                    return;
                }
            };
            if let Err(e) = result {
                self.workbench.configuration_error = e;
            } else if new_generation {
                self.workbench.layout_generation = self.workbench.layout_generation.wrapping_add(1);
            }
        }
        self.workbench.workspace = workspace;
    }
}
