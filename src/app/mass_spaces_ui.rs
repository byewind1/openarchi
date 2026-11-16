use super::*;
use crate::mass_spaces::{self, Edit};

#[derive(Default)]
pub(super) struct MassSpaceUi {
    source: Option<(u64, u64, String)>,
    height: f64,
    name: String,
    purpose: String,
    pub(super) dirty: bool,
}

impl AtelierApp {
    pub(super) fn preview_mass_space(&mut self, edit: Edit) {
        if (self.mass_spaces_ui.dirty && !matches!(edit, Edit::Create { .. }))
            || self.menu_gated()
            || self.workbench_pointer_active()
            || self.pending_other_design_parameters()
            || self.drawings_ui.report_draft.is_some()
        {
            self.host.document.editor.status =
                "Finish or cancel the current model interaction before applying settings".into();
            return;
        }
        let e = &self.host.document.editor;
        let result = e
            .prepare_change(
                e.revision,
                vec![crate::command::ModelCommand::MassSpaces { edit }],
            )
            .map_err(|e| e.message);
        if let Err(error) = &result {
            self.host.document.editor.status = error.clone();
        }
        self.process_set_preview(
            storage::new_document_identity(),
            "Edit mass-space links",
            result,
        );
    }

    fn mass_space_pending_controls(&mut self, ui: &mut Ui) -> bool {
        if self.mass_spaces_ui.dirty
            && self
                .mass_spaces_ui
                .source
                .as_ref()
                .is_some_and(|(session, id, _)| {
                    *session != self.host.document.editor.document_session()
                        || self.host.document.editor.selected != Some(*id)
                })
        {
            ui.label(
                self.language
                    .translate("Unconfirmed concept space parameters"),
            );
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button(self.language.translate("Return to source mass"))
                    .clicked()
                {
                    if let Some((session, id, _)) = &self.mass_spaces_ui.source {
                        if *session == self.host.document.editor.document_session() {
                            self.host.document.editor.set_selection(vec![*id]);
                        }
                    }
                }
                if ui
                    .button(self.language.translate("Cancel space parameters"))
                    .clicked()
                {
                    self.mass_spaces_ui = Default::default();
                }
            });
            ui.separator();
            return true;
        }
        false
    }

    /// A current candidate context only; this never changes a task or authorization.
    pub(super) fn mass_space_canvas_selection(&self) -> Option<Element> {
        let e = &self.host.document.editor;
        if e.selection_ids().len() != 1 {
            return None;
        }
        let task = e.project.design_process.task()?;
        if !task.is_mass()
            || e.story != task.story
            || !task
                .candidates
                .iter()
                .any(|c| Some(c.option) == e.project.plan_options.active_design_option)
        {
            return None;
        }
        let selected = e
            .project
            .elements
            .iter()
            .find(|x| Some(x.id) == e.selected)?;
        if selected.story != task.story
            || selected.design_option != e.project.plan_options.active_design_option
            || selected.worksheet.is_some()
        {
            return None;
        }
        if matches!(selected.shape, Shape::ConceptMass { .. }) {
            return Some(selected.clone());
        }
        if matches!(selected.shape, Shape::Zone { .. })
            && e.project.design_process.mass_spaces.iter().any(|l| {
                l.zone == selected.id
                    && e.project.elements.iter().any(|m| {
                        m.id == l.mass
                            && m.story == selected.story
                            && m.design_option == selected.design_option
                            && matches!(m.shape, Shape::ConceptMass { .. })
                    })
            })
        {
            return Some(selected.clone());
        }
        None
    }

    fn mass_space_creation_on_canvas(&self, selected: &Element) -> bool {
        matches!(selected.shape, Shape::ConceptMass { .. })
            && self
                .mass_space_canvas_selection()
                .is_some_and(|e| e.id == selected.id)
            && !self
                .host
                .document
                .editor
                .project
                .design_process
                .mass_spaces
                .iter()
                .any(|l| l.mass == selected.id)
    }

    /// Both presentations use the same source-bound transient parameter draft.
    fn mass_space_create_controls(&mut self, ui: &mut Ui, selected: &Element) -> Option<Edit> {
        let e = &self.host.document.editor;
        let language = self.language;
        if !self.mass_spaces_ui.dirty {
            let source = mass_spaces::element_stamp(&e.project, selected)
                .ok()
                .map(|stamp| (e.document_session(), selected.id, stamp));
            if self.mass_spaces_ui.source != source {
                self.mass_spaces_ui.name =
                    format!("{} #{}", language.translate("Concept space"), selected.id);
                self.mass_spaces_ui.purpose = language.translate("Unknown concept use").into();
            }
            self.mass_spaces_ui.source = source;
            if self.mass_spaces_ui.height == 0. {
                self.mass_spaces_ui.height = 3.;
            }
        }
        egui::containers::menu::MenuButton::new(language.translate("Space name and use"))
            .config(
                egui::containers::menu::MenuConfig::new()
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
            )
            .ui(ui, |ui| {
                ui.label(
                    language.translate(
                        "Optional; blanks keep an automatic name and unknown concept use",
                    ),
                );
                for (key, label, value) in [
                    ("name", "Name", &mut self.mass_spaces_ui.name),
                    ("purpose", "Space purpose", &mut self.mass_spaces_ui.purpose),
                ] {
                    let label = ui.label(language.translate(label));
                    let response = ui.text_edit_singleline(value).labelled_by(label.id);
                    self.mass_spaces_ui.dirty |= response.changed();
                    #[cfg(test)]
                    ui.ctx().data_mut(|data| {
                        data.insert_temp(Id::new(("mass_space_identity", key)), response.id)
                    });
                    let _ = (key, response);
                }
                ui.label(language.translate("Concept space: not measured or approved use"));
            });
        let height_changed = ui
            .horizontal(|ui| {
                let label = ui.label(language.translate("Assumed clear height"));
                ui.add(
                    DragValue::new(&mut self.mass_spaces_ui.height)
                        .speed(0.05)
                        .fixed_decimals(2)
                        .suffix(" m"),
                )
                .labelled_by(label.id)
                .changed()
            })
            .inner;
        if height_changed {
            self.mass_spaces_ui.dirty = true;
        }
        let mut edit = None;
        if ui.button(language.translate("Create editable space")).on_hover_text(language.translate("Single concept level; assumed clear height; original footprint and holes; no wall deduction or approved use")).clicked() {
            if let Some((session,mass,stamp))=&self.mass_spaces_ui.source {
                if *session==self.host.document.editor.document_session() {
                    edit=Some(Edit::Create{mass:*mass,expected_source_sha256:stamp.clone(),name:if self.mass_spaces_ui.name.trim().is_empty(){format!("{} #{}",language.translate("Concept space"),mass)}else{self.mass_spaces_ui.name.clone()},purpose:if self.mass_spaces_ui.purpose.trim().is_empty(){language.translate("Unknown concept use").into()}else{self.mass_spaces_ui.purpose.clone()},height:self.mass_spaces_ui.height,assumption:language.translate("Single concept level; assumed clear height; original footprint and holes; no wall deduction or approved use").into()});
                }
            }
        }
        if self.mass_spaces_ui.dirty
            && ui
                .button(language.translate("Cancel space parameters"))
                .clicked()
        {
            self.mass_spaces_ui = Default::default();
        }
        edit
    }

    pub(super) fn focus_mass_space_element(&mut self, id: u64) {
        if self.menu_gated() || self.workbench_pointer_active() || self.pending_design_parameters()
        {
            self.host.document.editor.status =
                "Finish or cancel the current model interaction before applying settings".into();
            return;
        }
        self.dispatch_workbench(crate::workbench::WorkbenchCommand::Select);
        self.host.document.editor.set_selection(vec![id]);
    }

    pub(super) fn mass_space_canvas_controls(&mut self, ui: &mut Ui) {
        // Pending source changes remain reachable even with the property panel hidden.
        if self.mass_space_pending_controls(ui) {
            return;
        }
        let Some(selected) = self.mass_space_canvas_selection() else {
            let e = &self.host.document.editor;
            if e.tool == Tool::Zone
                && e.project.design_process.task().is_some_and(|t| {
                    t.is_mass()
                        && t.candidates
                            .iter()
                            .any(|c| Some(c.option) == e.project.plan_options.active_design_option)
                })
            {
                ui.label(
                    self.language
                        .translate("Select a source mass above to create a linked concept space."),
                );
            }
            return;
        };
        let links = self
            .host
            .document
            .editor
            .project
            .design_process
            .mass_spaces
            .iter()
            .filter(|l| l.mass == selected.id || l.zone == selected.id)
            .cloned()
            .collect::<Vec<_>>();
        let busy = self.menu_gated()
            || self.workbench_pointer_active()
            || self.pending_other_design_parameters();
        let mut command = None;
        let mut select = None;
        ui.push_id("canvas_mass_spaces", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong(self.language.translate("Mass and concept spaces"));
                if self.mass_space_creation_on_canvas(&selected) {
                    ui.label(
                        self.language
                            .translate("One concept level; height assumed; use unknown"),
                    );
                    ui.add_enabled_ui(!busy, |ui| {
                        command = self.mass_space_create_controls(ui, &selected);
                    });
                } else {
                    if links.len() > 1 && matches!(selected.shape, Shape::ConceptMass { .. }) {
                        ui.add_enabled_ui(!busy && !self.mass_spaces_ui.dirty, |ui| {
                            ui.menu_button(self.language.translate("Select linked space"), |ui| {
                                for link in &links {
                                    if let Some(zone) = self
                                        .host
                                        .document
                                        .editor
                                        .project
                                        .elements
                                        .iter()
                                        .find(|e| e.id == link.zone)
                                    {
                                        if ui
                                            .button(format!("#{} · {}", zone.id, zone.name))
                                            .clicked()
                                        {
                                            select = Some(zone.id);
                                            ui.close();
                                        }
                                    }
                                }
                            });
                        });
                    }
                    for link in links.iter().filter(|_| {
                        links.len() == 1 || matches!(selected.shape, Shape::Zone { .. })
                    }) {
                        let p = &self.host.document.editor.project;
                        ui.label(format!("#{} → Zone #{}", link.mass, link.zone))
                            .on_hover_text(&link.assumption);
                        ui.label(self.language.translate(mass_spaces::status(p, link)));
                        ui.add_enabled_ui(!busy && !self.mass_spaces_ui.dirty, |ui| {
                            let other = if selected.id == link.mass {
                                link.zone
                            } else {
                                link.mass
                            };
                            if ui
                                .add_enabled(
                                    p.elements.iter().any(|e| e.id == other),
                                    Button::new(self.language.translate(
                                        if selected.id == link.mass {
                                            "Select linked space"
                                        } else {
                                            "Return to source mass"
                                        },
                                    )),
                                )
                                .clicked()
                            {
                                select = Some(other);
                            }
                            if ui
                                .button(self.language.translate("Preview space update"))
                                .clicked()
                            {
                                command = Some(Edit::Refresh {
                                    zone: link.zone,
                                    expected_sha256: mass_spaces::expected(link),
                                    height: None,
                                });
                            }
                        });
                    }
                    if self.mass_spaces_ui.dirty
                        && ui
                            .button(self.language.translate("Cancel space parameters"))
                            .clicked()
                    {
                        self.mass_spaces_ui = Default::default();
                    }
                }
            });
        });
        if let Some(id) = select {
            self.focus_mass_space_element(id);
        }
        if let Some(edit) = command {
            self.preview_mass_space(edit);
        }
    }

    pub(super) fn mass_space_controls(&mut self, ui: &mut Ui) {
        let e = &self.host.document.editor;
        let selected = e
            .selected
            .and_then(|id| e.project.elements.iter().find(|e| e.id == id))
            .cloned();
        if selected
            .as_ref()
            .is_some_and(|e| self.mass_space_creation_on_canvas(e))
        {
            // The current candidate's primary create controls live above the canvas.
            return;
        }
        if self.mass_space_pending_controls(ui) {
            return;
        }
        let Some(selected) = selected else {
            return;
        };
        if !matches!(
            selected.shape,
            Shape::ConceptMass { .. } | Shape::Zone { .. }
        ) {
            return;
        }
        let language = self.language;
        ui.strong(language.translate("Mass and concept spaces"));
        let mut command = None;
        let mut select = None;
        if matches!(selected.shape, Shape::ConceptMass { .. }) {
            ui.label(language.translate("One concept level; height assumed; use unknown"));
            ui.horizontal_wrapped(|ui| {
                command = self.mass_space_create_controls(ui, &selected);
            });
        }
        let p = &self.host.document.editor.project;
        let links = p
            .design_process
            .mass_spaces
            .iter()
            .filter(|l| l.mass == selected.id || l.zone == selected.id)
            .cloned()
            .collect::<Vec<_>>();
        for link in &links {
            ui.label(format!("#{} → Zone #{}", link.mass, link.zone));
            ui.label(language.translate(mass_spaces::status(p, link)));
            ui.collapsing(language.translate("Design assumption"), |ui| {
                ui.label(&link.assumption);
            });
            ui.horizontal_wrapped(|ui| {
                let other = if selected.id == link.mass {
                    link.zone
                } else {
                    link.mass
                };
                if ui
                    .button(language.translate(if selected.id == link.mass {
                        "Select linked space"
                    } else {
                        "Return to source mass"
                    }))
                    .clicked()
                    && p.elements.iter().any(|e| e.id == other)
                {
                    select = Some(other);
                }
                if ui
                    .button(language.translate("Preview space update"))
                    .clicked()
                {
                    command = Some(Edit::Refresh {
                        zone: link.zone,
                        expected_sha256: mass_spaces::expected(link),
                        height: None,
                    });
                }
                if ui.button(language.translate("Detach space link")).clicked() {
                    command = Some(Edit::Detach {
                        zone: link.zone,
                        expected_sha256: mass_spaces::expected(link),
                    });
                }
            });
        }
        // Existing-space association is opt-in and never grants regeneration permission.
        if matches!(selected.shape, Shape::Zone { .. }) && links.is_empty() {
            let p = &self.host.document.editor.project;
            let sources = p
                .elements
                .iter()
                .filter(|m| {
                    m.story == selected.story
                        && m.design_option == selected.design_option
                        && m.worksheet.is_none()
                        && matches!(m.shape, Shape::ConceptMass { .. })
                })
                .cloned()
                .collect::<Vec<_>>();
            ui.collapsing(language.translate("Link existing space to mass"), |ui| {
                ui.label(language.translate("Binding preserves existing geometry; no regeneration permission"));
                ScrollArea::vertical().id_salt("mass_link_sources").max_height(120.).show(ui, |ui| {
                    for m in sources {
                        if ui.button(format!("#{} · {}", m.id, m.name)).clicked() {
                            if let (Ok(ms), Ok(zs)) = (mass_spaces::element_stamp(p,&m),mass_spaces::element_stamp(p,&selected)) {
                                command = Some(Edit::LinkExisting { mass:m.id, zone:selected.id,
                                    expected_source_sha256:ms, expected_zone_sha256:zs,
                                    assumption:language.translate("Existing authored space; association only; no regeneration permission").into() });
                            }
                        }
                    }
                });
            });
        }
        ui.separator();
        if let Some(id) = select {
            self.focus_mass_space_element(id);
        }
        if let Some(edit) = command {
            self.preview_mass_space(edit);
        }
    }
}
