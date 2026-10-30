use super::*;
use crate::reshape::{self, ReshapeCommand};
pub(super) const COMMANDS: &[&str] = &[
    "Trim",
    "Magic Wand",
    "Adjust",
    "Intersect",
    "Fillet/Chamfer…",
    "Resize…",
    "Trim Elements to Roof/Shell",
];
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ReshapeSession {
    pub name: String,
    ids: Vec<u64>,
    revision: u64,
    anchor: Option<Point>,
    reference: Option<Point>,
    pub dialog: bool,
    graphical: bool,
    scale: f32,
    thickness: bool,
    radius: f32,
    chamfer: bool,
    error: String,
}
impl ReshapeSession {
    fn new(name: &str, ids: Vec<u64>, revision: u64) -> Self {
        Self {
            name: name.into(),
            ids,
            revision,
            anchor: None,
            reference: None,
            dialog: matches!(name, "Resize…" | "Fillet/Chamfer…"),
            graphical: true,
            scale: 1.,
            thickness: true,
            radius: 0.25,
            chamfer: false,
            error: String::new(),
        }
    }
}
impl AtelierApp {
    fn trim_command_down(ctx: &Context) -> bool {
        ctx.input(|i| {
            i.modifiers.mac_cmd && !i.modifiers.alt && !i.modifiers.shift && !i.modifiers.ctrl
        })
    }
    pub(super) fn trim_modifier_active(&self, ctx: &Context) -> bool {
        (Self::trim_command_down(ctx) || self.trim_shortcut_gesture)
            && self.reshape_session.is_none()
            && self.edit_dialog.is_none()
            && self.pointer_drag.is_none()
            && self.drag.is_none()
            && !self.trace.pointer_mode_active()
            && !self.orientation_pick_active
            && self.rotated_grid_tool.is_none()
            && !self.pick_armed
            && !self.inject_armed
    }
    pub(super) fn reshape_enabled(&self, name: &str) -> bool {
        if name == "Trim Elements to Roof/Shell" || self.view == View::Model {
            return false;
        }
        if self.reshape_session.is_some()
            || self.edit_dialog.is_some()
            || self.pointer_drag.is_some()
        {
            return false;
        }
        if name == "Trim" {
            return true;
        }
        if name == "Magic Wand" {
            return matches!(self.editor.tool, Tool::Wall | Tool::Slab | Tool::Polyline);
        }
        let ids = self.editor.selection_ids();
        if ids.is_empty() || ids.iter().any(|id| self.editor.locked_element(*id)) {
            return false;
        }
        if !self.editor.project.edit_state.suspend_groups
            && self
                .editor
                .project
                .edit_state
                .groups
                .iter()
                .any(|g| g.iter().any(|id| ids.contains(id)))
        {
            return false;
        }
        let es: Vec<_> = self
            .editor
            .project
            .elements
            .iter()
            .filter(|e| ids.contains(&e.id))
            .collect();
        let paths: Vec<_> = es
            .iter()
            .filter_map(|e| reshape::straight_path(e))
            .collect();
        let hosted =
            self.editor.project.elements.iter().any(
                |e| matches!(e.shape,Shape::Opening{host_wall,..} if ids.contains(&host_wall)),
            );
        match name {
            "Resize…" => es.iter().all(|e| !matches!(e.shape, Shape::Opening { .. })),
            "Adjust" => !hosted && paths.len() == ids.len(),
            "Intersect" => {
                !hosted && ids.len() == 2 && paths.len() == 2 && paths.iter().all(|p| p.len() == 2)
            }
            "Fillet/Chamfer…" => {
                !hosted
                    && paths.len() == ids.len()
                    && ((ids.len() == 1 && paths[0].len() > 2)
                        || (ids.len() == 2
                            && paths.iter().all(|p| p.len() == 2)
                            && es[0].kind() == es[1].kind()))
            }
            _ => false,
        }
    }
    pub(super) fn reshape_tip(&self, name: &str) -> String {
        let detail = match name {
            "Trim" => {
                "Hold Cmd and click a straight wall or polyline segment to trim; release Cmd to exit"
            }
            "Magic Wand" => {
                "Choose Wall, Slab or Polyline; trace straight edges or enclosed regions"
            }
            "Adjust" => "Select straight walls or polylines, then click an edge or draw a boundary",
            "Intersect" => {
                "Select two straight walls or two-node lines; grouped elements require Suspend Groups"
            }
            "Fillet/Chamfer…" => {
                "Two straight lines or polyline corners; walls support Chamfer only"
            }
            "Resize…" => "Set a ratio or define it graphically, then click a fixed point",
            _ => "Requires Roof/Shell model support, which is not available yet",
        };
        format!(
            "{}\n{}",
            self.language.translate(name),
            self.language.translate(detail)
        )
    }
    pub(super) fn reshape_toolbar(&mut self, ui: &mut Ui) {
        for &name in COMMANDS {
            let enabled = self.reshape_enabled(name);
            let selected = self
                .reshape_session
                .as_ref()
                .is_some_and(|s| s.name == name);
            let tip = self.reshape_tip(name);
            let response = ui
                .add_enabled_ui(enabled, |ui| {
                    let (r, response) = ui.allocate_exact_size(vec2(28., 28.), Sense::click());
                    #[cfg(test)]
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(Id::new(("reshape_button", name)), r));
                    response.widget_info(|| {
                        WidgetInfo::labeled(
                            WidgetType::Button,
                            enabled,
                            self.language.translate(name),
                        )
                    });
                    if response.hovered() || selected {
                        ui.painter().rect_filled(r, 3., PANEL_ALT);
                    }
                    Self::paint_tool_icon(
                        ui.painter(),
                        r.shrink(5.),
                        name,
                        if enabled { INK } else { MUTED },
                    );
                    response
                })
                .inner
                .on_hover_text(&tip)
                .on_disabled_hover_text(&tip);
            if response.clicked() {
                self.dispatch_edit(name);
            }
        }
    }
    pub(super) fn start_reshape(&mut self, name: &str) {
        self.wall_edit = None;

        self.disarm_transfer();
        self.editor.cancel_wall_draft();
        self.editor.cancel_polyline();
        self.editor.anchor = None;
        if name == "Intersect" {
            if let Err(e) = self.editor.reshape(ReshapeCommand::Intersect {
                ids: self.editor.selection_ids(),
            }) {
                self.editor.status = e;
            }
            return;
        }
        self.reshape_session = Some(ReshapeSession::new(
            name,
            self.editor.selection_ids(),
            self.editor.revision,
        ));
        self.editor.status = match name {
            "Trim" => "Click the segment to trim · Escape cancels",
            "Magic Wand" => "Hover an edge or enclosed region, then click · Escape cancels",
            "Adjust" => "Click a boundary edge or two points · Escape cancels",
            _ => "Set reshape parameters",
        }
        .into();
    }
    pub(super) fn reshape_dialog(&mut self, ctx: &Context) {
        let Some(mut s) = self.reshape_session.take() else {
            return;
        };
        if !s.dialog {
            self.reshape_session = Some(s);
            return;
        }
        let mut ok = false;
        let mut cancel = false;
        let lang = self.language;
        let wall = s.ids.iter().any(|id| {
            self.editor
                .project
                .elements
                .iter()
                .any(|e| e.id == *id && matches!(e.shape, Shape::Wall { .. }))
        });
        if wall && s.name == "Fillet/Chamfer…" {
            s.chamfer = true;
        }
        let modal = Modal::new(Id::new("reshape_parameters")).show(ctx, |ui| {
            ui.set_width(450.);
            ui.heading(lang.translate(&s.name));
            ui.separator();
            if s.name == "Resize…" {
                ui.checkbox(&mut s.graphical, lang.translate("Define graphically"));
                ui.add_enabled_ui(!s.graphical, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(lang.translate("Scale factor"));
                        ui.add(
                            DragValue::new(&mut s.scale)
                                .speed(0.01)
                                .range(0.001..=1000.),
                        );
                        ui.label(format!("{:.1}%", s.scale * 100.));
                    });
                });
                ui.checkbox(
                    &mut s.thickness,
                    lang.translate("Resize wall thickness and height"),
                );
                ui.label(lang.translate("After OK, click the fixed point in the plan"));
            } else {
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!wall, |ui| {
                        ui.radio_value(&mut s.chamfer, false, lang.translate("Fillet"));
                    });
                    ui.radio_value(&mut s.chamfer, true, lang.translate("Chamfer"));
                });
                ui.horizontal(|ui| {
                    ui.label(lang.translate("Radius"));
                    ui.add(DragValue::new(&mut s.radius).speed(0.01).suffix(" m"));
                });
                ui.label(lang.translate(
                    "Polyline: apply to all corners; radius is limited by adjacent edges",
                ));
                if wall {
                    ui.label(lang.translate("Curved walls are not supported; choose Chamfer"));
                }
            }
            if !s.error.is_empty() {
                ui.colored_label(Color32::LIGHT_RED, lang.translate(&s.error));
            }
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ok = ui.button(lang.translate("OK")).clicked();
                cancel = ui.button(lang.translate("Cancel")).clicked();
            });
        });
        if cancel || modal.should_close() {
            self.editor.status = "Reshape cancelled".into();
            return;
        }
        if ok {
            if s.name == "Trim" {
                // Each cut (including after undo) resolves against the current model.
                s.revision = self.editor.revision;
            }
            if s.revision != self.editor.revision {
                s.error = "Project changed; reopen this command".into();
            } else if s.name == "Resize…" {
                s.dialog = false;
                self.editor.status = "Click the fixed point · Escape cancels".into();
            } else {
                match self.editor.reshape(ReshapeCommand::Corner {
                    ids: s.ids.clone(),
                    radius: s.radius,
                    chamfer: s.chamfer,
                }) {
                    Ok(()) => return,
                    Err(e) => s.error = e,
                }
            }
        }
        self.reshape_session = Some(s);
    }
    pub(super) fn reshape_canvas(
        &mut self,
        ui: &mut Ui,
        r: Rect,
        response: &egui::Response,
    ) -> bool {
        let cmd_trim = self.trim_modifier_active(ui.ctx());
        let magic_transient = !cmd_trim
            && self.reshape_session.is_none()
            && self.edit_dialog.is_none()
            && self.pointer_drag.is_none()
            && self.editor.anchor.is_none()
            && self.editor.wall_points.is_empty()
            && self.editor.polyline_points.is_empty()
            && ui.input(|i| i.key_down(Key::Space))
            && matches!(self.editor.tool, Tool::Wall | Tool::Slab | Tool::Polyline);
        let transient = cmd_trim || magic_transient;
        if cmd_trim {
            if response.hovered() && ui.input(|i| i.pointer.primary_pressed()) {
                self.trim_shortcut_gesture = true;
            }
            if ui.input(|i| i.pointer.primary_released() || !i.focused) {
                self.trim_shortcut_gesture = false;
            }
            // Swallow a release belonging to Trim even if Cmd was released first.
            if !Self::trim_command_down(ui.ctx()) {
                return true;
            }
        }
        if magic_transient {
            self.start_reshape("Magic Wand");
        }
        let session = if cmd_trim {
            Some(ReshapeSession::new(
                "Trim",
                self.editor.selection_ids(),
                self.editor.revision,
            ))
        } else {
            self.reshape_session.take()
        };
        let Some(mut s) = session else {
            return false;
        };
        if s.dialog {
            self.reshape_session = Some(s);
            return true;
        }
        if s.name == "Trim" {
            // Each cut (including after undo) resolves against the current model.
            s.revision = self.editor.revision;
        }
        if s.revision != self.editor.revision {
            self.editor.status = "Project changed; reopen this command".into();
            return true;
        }
        let Some(pos) = response.hover_pos().filter(|p| r.contains(*p)) else {
            if !transient {
                self.reshape_session = Some(s);
            }
            return true;
        };
        let point = self.world(r, pos);
        let tolerance = 7. / self.plan_scale;
        if matches!(s.name.as_str(), "Trim" | "Magic Wand") {
            ui.ctx().set_cursor_icon(CursorIcon::None);
            Self::paint_tool_icon(
                ui.painter(),
                Rect::from_min_size(pos + vec2(3., 3.), vec2(22., 22.)),
                &s.name,
                INK,
            );
        } else {
            ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
        }
        let edge = reshape::edge_at(
            &self.editor.project,
            self.editor.story,
            point,
            tolerance,
            if s.name == "Adjust" { &s.ids } else { &[] },
        );
        let mut command = None;
        let mut preview = None;
        let mut error = None;
        match s.name.as_str() {
            "Trim" => {
                if let Some((id, _, _)) = edge {
                    command = Some(ReshapeCommand::Trim {
                        id,
                        point,
                        story: self.editor.story,
                    });
                }
            }
            "Adjust" => {
                let boundary = s
                    .anchor
                    .map(|a| (a, point))
                    .or_else(|| edge.map(|(_, a, b)| (a, b)));
                if let Some((a, b)) = boundary {
                    command = Some(ReshapeCommand::Adjust {
                        ids: s.ids.clone(),
                        a,
                        b,
                    });
                    ui.painter_at(r).line_segment(
                        [self.screen(r, a), self.screen(r, b)],
                        Stroke::new(2., ACCENT),
                    );
                }
            }
            "Resize…" => {
                if let Some(origin) = s.anchor {
                    let scale = if s.graphical {
                        s.reference
                            .map(|reference| origin.distance(point) / origin.distance(reference))
                            .unwrap_or(1.)
                    } else {
                        s.scale
                    };
                    if !s.graphical || s.reference.is_some() {
                        command = Some(ReshapeCommand::Resize {
                            ids: s.ids.clone(),
                            origin,
                            scale,
                            thickness: s.thickness,
                        });
                    }
                    ui.painter_at(r)
                        .line_segment([self.screen(r, origin), pos], Stroke::new(1.5, ACCENT));
                } else if !s.graphical {
                    command = Some(ReshapeCommand::Resize {
                        ids: s.ids.clone(),
                        origin: point,
                        scale: s.scale,
                        thickness: s.thickness,
                    });
                }
            }
            "Magic Wand" => {
                match reshape::trace(
                    &self.editor.project,
                    self.editor.story,
                    &s.ids,
                    point,
                    tolerance,
                ) {
                    Ok(path) => preview = Some(vec![path]),
                    Err(e) => error = Some(e),
                }
            }
            _ => {}
        }
        if let Some(c) = &command {
            match reshape::candidate(&self.editor.project, c) {
                Ok((p, ids)) => {
                    preview = Some(
                        p.elements
                            .iter()
                            .filter(|e| ids.contains(&e.id))
                            .map(|e| match &e.shape {
                                Shape::Polyline { points, arcs } => {
                                    geometry::polyline_path(points, arcs)
                                }
                                _ => {
                                    let mut ps = geometry::element_footprint(&p, e).to_vec();
                                    if !ps.is_empty() {
                                        ps.push(ps[0]);
                                    }
                                    ps
                                }
                            })
                            .collect(),
                    );
                }
                Err(e) => error = Some(e),
            }
        }
        if let Some(paths) = preview {
            for path in paths {
                for w in path.windows(2) {
                    ui.painter_at(r).line_segment(
                        [self.screen(r, w[0]), self.screen(r, w[1])],
                        Stroke::new(2.5, ACCENT),
                    );
                }
            }
        }
        if response.clicked_by(PointerButton::Primary) {
            if s.name == "Magic Wand" {
                match self.editor.magic_wand(point, tolerance) {
                    Ok(()) => return true,
                    Err(e) => self.editor.status = e,
                }
            } else if s.name == "Adjust" && s.anchor.is_none() && edge.is_none() {
                s.anchor = Some(point);
                self.editor.status = "Click the second boundary point · Escape cancels".into();
            } else if s.name == "Resize…" && s.graphical && s.anchor.is_none() {
                s.anchor = Some(point);
                self.editor.status = "Click the reference distance point · Escape cancels".into();
            } else if s.name == "Resize…" && s.graphical && s.reference.is_none() {
                if s.anchor.unwrap().distance(point) > 0.01 {
                    s.reference = Some(point);
                    self.editor.status = "Click the new distance point · Escape cancels".into();
                }
            } else if let Some(c) = command {
                match self.editor.reshape(c) {
                    Ok(()) => {
                        if s.name == "Trim" && !transient {
                            s.revision = self.editor.revision;
                            self.reshape_session = Some(s);
                        }
                        return true;
                    }
                    Err(e) => self.editor.status = e,
                }
            } else {
                self.editor.status = error.unwrap_or_else(|| "Click a supported edge".into());
            }
        }
        // Modifier modes exist only for this frame; explicit toolbar/menu modes persist.
        if !transient {
            self.reshape_session = Some(s);
        }
        true
    }
}
