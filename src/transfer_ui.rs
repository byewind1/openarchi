// AC29-style parameter transfer UI: pick up / inject commands, one-shot
// cursor modes and the Element Transfer Settings dialog. Model semantics live
// in `transfer.rs` and `Editor`; this layer only routes input and reports.
use crate::transfer::{TransferType, group_label_key, groups_for};
use egui::RichText;

impl AtelierApp {
    pub(super) fn shortcut_label(&self, macos: &str, other: &str) -> String {
        if cfg!(target_os = "macos") {
            macos.into()
        } else {
            other.into()
        }
    }

    fn transfer_status(&self, key: &str) -> String {
        self.language.translate(key).into()
    }

    fn element_name(&self, id: u64) -> String {
        self.editor
            .project
            .elements
            .iter()
            .find(|element| element.id == id)
            .map(|element| element.name.clone())
            .unwrap_or_else(|| "—".into())
    }

    /// Edit > Element Settings style commands. With a suitable selection the
    /// transfer happens immediately; without one the next plan/3D click
    /// performs it and the mode disarms.
    pub(super) fn pick_up_command(&mut self) {
        let ids = self.editor.selection_ids();
        if ids.len() == 1 {
            let id = ids[0];
            let set = self.editor.project.transfer_settings.clone();
            match self.editor.pick_up_from(id, &set) {
                Ok(transfer_type) => {
                    let name = self.element_name(id);
                    let type_label = self
                        .language
                        .translate(transfer_type.label_key())
                        .to_string();
                    let template = self
                        .language
                        .translate("Picked up {type} parameters from {name}");
                    self.editor.status = template
                        .replace("{type}", &type_label)
                        .replace("{name}", &name);
                }
                Err(error) => self.editor.status = self.transfer_status(&error),
            }
        } else {
            self.pick_armed = true;
            self.inject_armed = false;
            self.editor.status =
                self.transfer_status("Pick up parameters · click a source element");
        }
    }

    pub(super) fn inject_command(&mut self) {
        let ids = self.editor.selection_ids();
        if ids.is_empty() {
            if self.editor.pick_up_buffer.entries.is_empty() {
                self.editor.status = self.transfer_status("Pick up parameters before injecting");
                return;
            }
            self.inject_armed = true;
            self.pick_armed = false;
            self.editor.status = self.transfer_status("Inject parameters · click a target element");
        } else {
            let set = self.editor.project.transfer_settings.clone();
            match self.editor.inject_into(&ids, &set) {
                Ok(count) => {
                    let template = self
                        .language
                        .translate("Injected parameters into {count} element(s)");
                    self.editor.status = template.replace("{count}", &count.to_string());
                }
                Err(error) => self.editor.status = self.transfer_status(&error),
            }
        }
    }

    /// One click of a transfer gesture or one-shot mode. `inject` selects the
    /// syringe; a miss only reports and (for armed modes) has already
    /// consumed the mode.
    pub(super) fn execute_transfer(&mut self, inject: bool, hit: Option<u64>) {
        let Some(id) = hit else {
            self.editor.status = self.transfer_status(if inject {
                "No element to inject into"
            } else {
                "No element to pick up from"
            });
            return;
        };
        if inject {
            let set = self.editor.project.transfer_settings.clone();
            match self.editor.inject_into(&[id], &set) {
                Ok(count) => {
                    let template = self
                        .language
                        .translate("Injected parameters into {count} element(s)");
                    self.editor.status = template.replace("{count}", &count.to_string());
                }
                Err(error) => self.editor.status = self.transfer_status(&error),
            }
        } else {
            let set = self.editor.project.transfer_settings.clone();
            match self.editor.pick_up_from(id, &set) {
                Ok(transfer_type) => {
                    let name = self.element_name(id);
                    let type_label = self
                        .language
                        .translate(transfer_type.label_key())
                        .to_string();
                    let template = self
                        .language
                        .translate("Picked up {type} parameters from {name}");
                    self.editor.status = template
                        .replace("{type}", &type_label)
                        .replace("{name}", &name);
                }
                Err(error) => self.editor.status = self.transfer_status(&error),
            }
        }
    }

    pub(super) fn disarm_transfer(&mut self) {
        if self.pick_armed || self.inject_armed {
            self.pick_armed = false;
            self.inject_armed = false;
            self.editor.status = self.transfer_status("Parameter transfer cancelled");
        }
    }

    pub(super) fn open_transfer_settings(&mut self) {
        self.transfer_dialog = Some(self.editor.project.transfer_settings.clone());
    }

    pub(super) fn transfer_settings_dialog(&mut self, ctx: &Context) {
        if self.transfer_dialog.is_none() {
            return;
        }
        let title = self
            .language
            .translate("Element Transfer Settings")
            .to_string();
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        if let Some(working) = self.transfer_dialog.as_mut() {
            egui::Window::new(RichText::new(title).strong())
                .open(&mut open)
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.set_min_width(340.);
                    ui.label(RichText::new(self.language.translate("Transfer Sets")).strong());
                    let mut activate: Option<usize> = None;
                    for (index, set) in working.sets.iter().enumerate() {
                        if ui
                            .selectable_label(index == working.active, set.name.clone())
                            .clicked()
                        {
                            activate = Some(index);
                        }
                    }
                    if let Some(index) = activate {
                        working.active = index;
                    }
                    ui.horizontal(|ui| {
                        let active = working.active.min(working.sets.len() - 1);
                        if ui.button(self.language.translate("New")).clicked() {
                            let name = format!("Transfer Set {}", working.sets.len() + 1);
                            working.sets.push(crate::transfer::TransferSet {
                                name,
                                groups: Default::default(),
                            });
                            working.active = working.sets.len() - 1;
                        }
                        if ui.button(self.language.translate("Duplicate")).clicked() {
                            let mut set = working.sets[active].clone();
                            set.name = format!("{} Copy", set.name);
                            working.sets.push(set);
                            working.active = working.sets.len() - 1;
                        }
                        if ui
                            .add_enabled(
                                working.sets.len() > 1,
                                egui::Button::new(self.language.translate("Delete")),
                            )
                            .clicked()
                        {
                            working.sets.remove(active);
                            working.active =
                                working.active.saturating_sub(1).min(working.sets.len() - 1);
                        }
                    });
                    let active = working.active.min(working.sets.len() - 1);
                    ui.horizontal(|ui| {
                        ui.label(self.language.translate("Name"));
                        ui.text_edit_singleline(&mut working.sets[active].name);
                    });
                    ui.separator();
                    ui.label(
                        RichText::new(self.language.translate("Transferred parameters")).strong(),
                    );
                    for transfer_type in [
                        TransferType::Wall,
                        TransferType::Slab,
                        TransferType::Stair,
                        TransferType::Polyline,
                        TransferType::Door,
                        TransferType::Window,
                    ] {
                        ui.label(
                            RichText::new(self.language.translate(transfer_type.label_key()))
                                .weak()
                                .strong(),
                        );
                        for group in groups_for(transfer_type) {
                            let mut checked = working.sets[active].allows(group);
                            if ui
                                .checkbox(
                                    &mut checked,
                                    self.language.translate(group_label_key(group)),
                                )
                                .changed()
                            {
                                if checked {
                                    working.sets[active].groups.insert((*group).into());
                                } else {
                                    working.sets[active].groups.remove(*group);
                                }
                            }
                        }
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(self.language.translate("OK")).clicked() {
                            apply = true;
                        }
                        if ui.button(self.language.translate("Cancel")).clicked() {
                            cancel = true;
                        }
                    });
                });
        }
        if apply {
            if let Some(settings) = self.transfer_dialog.take() {
                self.editor.set_transfer_settings(settings);
            }
        } else if cancel || !open {
            self.transfer_dialog = None;
        }
    }

    /// Toolbar hint listing which transfer types currently hold picked-up
    /// parameters, AC29 style.
    pub(super) fn pick_up_tooltip(&self) -> String {
        let shortcut = self.shortcut_label("⌥C", "Alt+C");
        let label = self.language.translate("Pick Up Parameters");
        format!("{label} ({shortcut})")
    }

    pub(super) fn inject_tooltip(&self) -> String {
        let shortcut = self.shortcut_label("⌘⌥C", "Ctrl+Alt+C");
        let label = self.language.translate("Inject Parameters");
        format!("{label} ({shortcut})")
    }

    /// Shared hover resolution for the transfer cursor: records the hovered
    /// element for the highlight when a click would act on it, and reports
    /// whether the cursor should be drawn primed with liquid.
    pub(super) fn transfer_hover_state(
        &mut self,
        kind: crate::transfer_cursor::TransferCursor,
        hit: Option<u64>,
    ) -> bool {
        hit.is_some_and(|id| {
            let transferable = match kind {
                crate::transfer_cursor::TransferCursor::PickUp => true,
                crate::transfer_cursor::TransferCursor::Inject => self
                    .editor
                    .project
                    .elements
                    .iter()
                    .find(|element| element.id == id)
                    .and_then(crate::transfer::TransferType::of)
                    .is_some_and(|transfer_type| {
                        self.editor.pick_up_buffer.get(transfer_type).is_some()
                    }),
            };
            if transferable {
                self.transfer_hover = Some(id);
            }
            transferable
        })
    }

    /// AC29-style hover highlight for the element a transfer cursor is over:
    /// a light wash plus a bright outline on its plan footprint.
    pub(super) fn paint_transfer_hover(&self, p: &Painter, r: Rect, element: &Element) {
        const HOVER_OUTLINE: Color32 = Color32::from_rgb(120, 220, 255);
        let wash = Color32::from_rgba_unmultiplied(255, 255, 255, 30);
        let stroke = Stroke::new(2., HOVER_OUTLINE);
        if let Shape::Polyline { points, .. } = &element.shape {
            let path: Vec<Pos2> = points.iter().map(|point| self.screen(r, *point)).collect();
            if path.len() > 1 {
                p.add(egui::Shape::line(path, stroke));
            }
            return;
        }
        let outline = geometry::element_footprint(&self.editor.project, element);
        let path: Vec<Pos2> = outline.iter().map(|point| self.screen(r, *point)).collect();
        p.add(egui::Shape::convex_polygon(
            path.clone(),
            wash,
            Stroke::NONE,
        ));
        p.add(egui::Shape::closed_line(path, stroke));
    }
}
