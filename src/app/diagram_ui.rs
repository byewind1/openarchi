use super::*;
use crate::diagrams::{self, Definition, Diagram, Domain, Edit, Presentation, Status};

#[derive(Clone, Copy)]
enum FigureKind {
    Quantities,
    Functions,
    Axon,
    Location,
    Setback,
    Constraints,
}
impl FigureKind {
    fn matches(self, p: &Presentation) -> bool {
        match self {
            Self::Quantities => p.is_default(),
            Self::Functions => p.is_functional(),
            Self::Axon => p.is_axon(),
            Self::Location => matches!(p, Presentation::SiteLocation),
            Self::Constraints => matches!(p, Presentation::ConstraintOverlay),
            Self::Setback => matches!(p, Presentation::SetbackStudy { .. }),
        }
    }
}
impl AtelierApp {
    pub(super) fn proposed_figure_id(&self, after: &Project) -> Option<u64> {
        after
            .drawings
            .diagrams
            .iter()
            .find(|f| {
                self.host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .iter()
                    .find(|o| o.id == f.id)
                    != Some(*f)
            })
            .map(|f| f.id)
    }
    fn figure_definition(&self) -> Result<Definition, String> {
        let p = &self.host.document.editor.project;
        let task = p
            .design_process
            .task()
            .ok_or("Choose a saved design task and candidate first")?;
        let option = p.plan_options.active_design_option;
        if option != Some(task.baseline_option)
            && !task.candidates.iter().any(|c| Some(c.option) == option)
        {
            return Err("Choose a saved design task and candidate first".into());
        }
        Ok(Definition {
            presentation: Default::default(),
            story: task.story,
            option,
            domain: if task.is_mass() {
                Domain::ConceptMasses
            } else {
                Domain::Zones
            },
        })
    }
    pub(super) fn preview_figure(&mut self, edit: Edit) {
        if self.menu_gated() || self.workbench_pointer_active() || self.pending_design_parameters()
        {
            self.process_ui.error =
                "Finish or cancel the current model interaction before applying settings".into();
            return;
        }
        let label = match edit {
            Edit::Report { .. } => "Edit report template",
            Edit::Configure { .. } => "Configure linked model figure",
            Edit::Create { .. } => "Create linked model figure",
            Edit::Refresh { .. } => "Update linked model figure",
            Edit::Rename { .. } => "Rename linked model figure",
            Edit::Delete { .. } => "Delete linked model figure",
        };
        let e = &self.host.document.editor;
        let result = e
            .prepare_change(
                e.revision,
                vec![crate::command::ModelCommand::Diagram { edit }],
            )
            .map_err(|e| e.message);
        if let Err(message) = &result {
            self.host.document.editor.status = message.clone();
        }
        self.process_set_preview(crate::storage::new_document_identity(), label, result);
    }
    pub(super) fn open_figure(&mut self, id: u64) {
        if self.menu_gated() || self.workbench_pointer_active() || self.pending_design_parameters()
        {
            self.process_ui.error =
                "Finish or cancel the current model interaction before applying settings".into();
            return;
        }
        if self
            .host
            .document
            .editor
            .project
            .drawings
            .diagrams
            .iter()
            .any(|f| f.id == id)
        {
            self.drawings_ui.report_navigation = None;
            self.drawings_ui.figure_unknown_draft = None;
            self.drawings_ui.figure_axon_draft = None;
            self.drawings_ui.figure_site_draft = None;
            self.drawings_ui.figure_draft_id = None;
            if self
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .iter()
                .any(|f| {
                    f.id == id
                        && matches!(f.definition.presentation, Presentation::ConstraintOverlay)
                })
            {
                self.checks_ui.open = false;
            }
            self.ensure_canvas_selection_mode();
            self.view = View::Figure(id);
            self.drawings_ui.manager = false;
            self.process_ui.open = false;
        }
    }
    fn create_current_figure(&mut self, kind: FigureKind) {
        match self.figure_definition_for(kind) {
            Ok(mut definition) => {
                definition.presentation = self.figure_presentation(&definition, kind);
                let p = &self.host.document.editor.project;
                let name = format!(
                    "{} · {}",
                    definition
                        .option
                        .and_then(|id| p.design_options.name(id))
                        .unwrap_or("Main model"),
                    self.language.translate(match kind {
                        FigureKind::Functions => "Functional combination diagram",
                        FigureKind::Axon => "Exploded axon diagram",
                        FigureKind::Quantities => "Model quantity figure",
                        FigureKind::Location => "Local site location",
                        FigureKind::Constraints => "Constraint result overlay",
                        FigureKind::Setback => "Uniform setback study",
                    })
                );
                self.preview_figure(Edit::Create { name, definition });
            }
            Err(e) => {
                self.process_ui.error = e;
                self.process_ui.open = true;
            }
        }
    }
    pub(super) fn figure_task_controls(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            self.figure_task_button(ui, FigureKind::Quantities);
            if self
                .figure_definition_for(FigureKind::Functions)
                .is_ok_and(|d| d.domain == Domain::Zones)
            {
                self.figure_task_button(ui, FigureKind::Functions);
            }
            self.figure_task_button(ui, FigureKind::Axon);
            self.figure_task_button(ui, FigureKind::Location);
            self.figure_task_button(ui, FigureKind::Setback);
        });
    }
    pub(super) fn site_figure_controls(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            self.figure_task_button(ui, FigureKind::Location);
            self.figure_task_button(ui, FigureKind::Setback);
        });
    }
    fn figure_definition_for(&self, kind: FigureKind) -> Result<Definition, String> {
        if matches!(
            kind,
            FigureKind::Location | FigureKind::Setback | FigureKind::Constraints
        ) {
            self.figure_definition()
                .or_else(|_| {
                    Ok(Definition {
                        story: self.host.document.editor.story,
                        option: self
                            .host
                            .document
                            .editor
                            .project
                            .plan_options
                            .active_design_option,
                        domain: Domain::Site,
                        presentation: Default::default(),
                    })
                })
                .map(|mut d| {
                    if matches!(kind, FigureKind::Constraints) {
                        d.domain = Domain::Site;
                    }
                    d
                })
        } else {
            let mut definition = self.figure_definition()?;
            // A supplementary Zone is editable within an existing mass task.
            // Its quantities and declared purposes need no second task or copied context.
            if matches!(kind, FigureKind::Quantities | FigureKind::Functions)
                && definition.domain == Domain::ConceptMasses
                && self.host.document.editor.selected.is_some_and(|id| {
                    self.host.document.editor.project.elements.iter().any(|e| {
                        e.id == id
                            && e.story == definition.story
                            && e.design_option == definition.option
                            && e.worksheet.is_none()
                            && matches!(e.shape, Shape::Zone { .. })
                    })
                })
            {
                definition.domain = Domain::Zones;
            }
            // A saved functional figure remains reachable after a parameter commit
            // clears transient selection. This opens the same story/option figure;
            // it does not authorize creating a new Zone task from a mass.
            if matches!(kind, FigureKind::Functions)
                && definition.domain == Domain::ConceptMasses
                && self
                    .host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .iter()
                    .any(|f| {
                        f.definition.story == definition.story
                            && f.definition.option == definition.option
                            && f.definition.domain == Domain::Zones
                            && f.definition.presentation.is_functional()
                    })
            {
                definition.domain = Domain::Zones;
            }
            Ok(definition)
        }
    }
    fn figure_presentation(&self, definition: &Definition, kind: FigureKind) -> Presentation {
        match kind {
            FigureKind::Quantities => Presentation::ModelQuantities,
            FigureKind::Functions => Presentation::ZoneFunctions {
                unknown_purposes: vec![],
            },
            FigureKind::Location => Presentation::SiteLocation,
            FigureKind::Constraints => Presentation::ConstraintOverlay,
            FigureKind::Setback => Presentation::SetbackStudy {
                distance_mm: 3000,
                assumption: self.language.translate(diagrams::site::ASSUMPTION).into(),
            },
            FigureKind::Axon => Presentation::ExplodedAxon {
                offsets_mm: self.vertical_axon_offsets(definition, 1000),
            },
        }
    }
    fn vertical_axon_offsets(
        &self,
        d: &Definition,
        step: i32,
    ) -> Vec<diagrams::axon::DisplayOffset> {
        let mut ids: Vec<_> = self
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .filter(|e| {
                e.worksheet.is_none()
                    && e.story == d.story
                    && e.design_option == d.option
                    && matches!(
                        (d.domain, &e.shape),
                        (Domain::Zones, Shape::Zone { .. })
                            | (Domain::ConceptMasses, Shape::ConceptMass { .. })
                    )
            })
            .map(|e| e.id)
            .collect();
        ids.sort_unstable();
        ids.into_iter()
            .enumerate()
            .map(|(i, id)| diagrams::axon::DisplayOffset {
                element_id: id,
                xyz: [0, 0, (i as i32).saturating_mul(step)],
            })
            .collect()
    }
    fn figure_task_button(&mut self, ui: &mut Ui, kind: FigureKind) {
        let l = self.language;
        let definition = self.figure_definition_for(kind).ok();
        let existing = definition.as_ref().and_then(|d| {
            self.host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .iter()
                .rev()
                .find(|f| {
                    f.definition.story == d.story
                        && f.definition.option == d.option
                        && f.definition.domain == d.domain
                        && kind.matches(&f.definition.presentation)
                })
                .map(|f| f.id)
        });
        let missing_boundary = matches!(kind, FigureKind::Setback)
            && self.host.document.editor.project.site.boundary.is_none();
        let enabled = definition.is_some()
            && (!matches!(kind, FigureKind::Constraints)
                || existing.is_some()
                || !self.host.document.editor.project.rules.checks.is_empty())
            && (!missing_boundary || existing.is_some())
            && !self.menu_gated()
            && !self.workbench_pointer_active()
            && !self.pending_design_parameters();
        let response = ui.add_enabled(
            enabled,
            Button::new(
                l.translate(
                    if matches!(kind, FigureKind::Quantities)
                        && definition
                            .as_ref()
                            .is_some_and(|d| d.domain == Domain::Zones)
                        && self
                            .host
                            .document
                            .editor
                            .project
                            .design_process
                            .task()
                            .is_some_and(|t| t.is_mass())
                    {
                        if existing.is_some() {
                            "Show space quantity figure"
                        } else {
                            "Preview space quantity figure"
                        }
                    } else {
                        match (kind, existing.is_some()) {
                            (FigureKind::Functions, true) => "Show function diagram",
                            (FigureKind::Functions, false) => "Preview function diagram",
                            (FigureKind::Axon, true) => "Show exploded axon",
                            (FigureKind::Axon, false) => "Preview exploded axon",
                            (FigureKind::Constraints, true) => "Show constraint figure",
                            (FigureKind::Constraints, false) => "Preview constraint figure",
                            (FigureKind::Location, true) => "Show location diagram",
                            (FigureKind::Location, false) => "Preview location diagram",
                            (FigureKind::Setback, true) => "Show setback study",
                            (FigureKind::Setback, false) => "Preview setback study",
                            (FigureKind::Quantities, true) => "Show linked figure",
                            (FigureKind::Quantities, false) => "Preview linked figure",
                        }
                    },
                ),
            ),
        );
        if missing_boundary {
            response
                .clone()
                .on_hover_text(l.translate("A source boundary is required for a setback study"));
        }
        #[cfg(test)]
        self.drawings_ui.rects.insert(
            match kind {
                FigureKind::Functions => "figure.function.task",
                FigureKind::Axon => "figure.axon.task",
                FigureKind::Quantities => "figure.task",
                FigureKind::Location => "figure.location.task",
                FigureKind::Constraints => "figure.constraint.task",
                FigureKind::Setback => "figure.setback.task",
            }
            .into(),
            response.rect,
        );
        if response.clicked() {
            if let Some(id) = existing {
                self.open_figure(id);
            } else {
                self.create_current_figure(kind);
            }
        }
    }
    pub(super) fn current_constraint_figure_id(&self) -> Option<u64> {
        let d = self.figure_definition_for(FigureKind::Constraints).ok()?;
        self.host
            .document
            .editor
            .project
            .drawings
            .diagrams
            .iter()
            .rev()
            .find(|f| {
                f.definition.story == d.story
                    && f.definition.option == d.option
                    && matches!(f.definition.presentation, Presentation::ConstraintOverlay)
            })
            .map(|f| f.id)
    }
    pub(super) fn show_constraint_figure(&mut self) {
        if let Some(id) = self.current_constraint_figure_id() {
            self.open_figure(id);
        } else {
            self.create_current_figure(FigureKind::Constraints);
        }
    }
    pub(super) fn figure_manager_controls(&mut self, ui: &mut Ui) {
        self.report_controls(ui);
        ui.strong(self.language.translate("Linked model figures"));
        self.figure_task_controls(ui);
        self.figure_task_button(ui, FigureKind::Constraints);
        if self.figure_definition().is_err() {
            ui.small(
                self.language
                    .translate("Choose a saved design task and candidate first"),
            );
        }
        let items: Vec<_> = self
            .host
            .document
            .editor
            .project
            .drawings
            .diagrams
            .iter()
            .map(|f| {
                (
                    f.id,
                    f.name.clone(),
                    f.status(&self.host.document.editor.project),
                )
            })
            .collect();
        ScrollArea::vertical()
            .id_salt("figures")
            .max_height(120.)
            .show(ui, |ui| {
                for (id, name, status) in items {
                    if ui
                        .button(format!(
                            "#{id} · {name} · {}",
                            self.language.translate(status.label())
                        ))
                        .clicked()
                    {
                        self.open_figure(id);
                    }
                }
            });
    }
    pub(super) fn figure_shortcuts(&mut self, ctx: &Context) {
        if ctx.egui_wants_keyboard_input()
            || self.file_action.is_some()
            || self.close_prompt
            || self.recovery_open
            || self.help
        {
            return;
        }
        if self.personal_shortcuts(ctx) {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::S)) {
            self.open_command_preview("Save As…".into());
            return;
        }
        if !self.shortcut_overridden("file.save")
            && ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::S))
        {
            self.request_project_save();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::O)) {
            self.file_action = Some(FileAction::Open);
        }
        if self.canvas_selection_box.is_some()
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            self.canvas_selection_box = None;
            return;
        }
        self.edit_shortcuts(ctx);
        if ctx.input_mut(|i| {
            (!self.shortcut_overridden("view.plan") && i.consume_key(Modifiers::NONE, Key::F2))
                || i.consume_key(Modifiers::NONE, Key::Escape)
        }) {
            self.dispatch_workbench(crate::workbench::WorkbenchCommand::Plan);
        }
        if !self.shortcut_overridden("view.model")
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F3))
        {
            self.dispatch_workbench(crate::workbench::WorkbenchCommand::Model);
        }
    }
    pub(super) fn figure_canvas(&mut self, ui: &mut Ui, id: u64) {
        self.report_navigation_controls(ui, id);
        let l = self.language;
        let Some(figure) = self
            .host
            .document
            .editor
            .project
            .drawings
            .diagrams
            .iter()
            .find(|f| f.id == id)
            .cloned()
        else {
            ui.label(l.translate("Figure is missing"));
            if ui.button(l.translate("Return to model")).clicked() {
                self.dispatch_workbench(crate::workbench::WorkbenchCommand::Plan);
            }
            return;
        };
        let status = figure.status(&self.host.document.editor.project);
        let key = (self.host.document.editor.document_session(), id);
        if self.drawings_ui.figure_draft_id != Some(key) {
            self.drawings_ui.figure_draft_id = Some(key);
            self.drawings_ui.figure_name = figure.name.clone();
            self.drawings_ui.figure_export = std::path::Path::new(&self.path)
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join(format!("figure-{id}-v{}.svg", figure.generation))
                .to_string_lossy()
                .into();
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button(l.translate("Report order…")).clicked() {
                self.drawings_ui.report_open = true;
                self.drawings_ui.manager = true;
            }
            ui.strong(l.translate(status.label()));
            if ui.button(l.translate("Edit source model")).clicked() {
                match self.activate_process_option(figure.definition.option) {
                    Ok(()) => {
                        if self.process_ui.preview.is_none() {
                            self.host.document.editor.story = figure.definition.story;
                            self.dispatch_workbench(crate::workbench::WorkbenchCommand::Plan);
                        }
                    }
                    Err(e) => self.process_ui.error = e,
                }
            }
            let update = ui.add_enabled(
                status != Status::Current,
                Button::new(l.translate("Preview figure update")),
            );
            #[cfg(test)]
            self.drawings_ui
                .rects
                .insert("figure.refresh".into(), update.rect);
            if update.clicked() {
                self.preview_figure(Edit::Refresh {
                    id,
                    expected_sha256: diagrams::expected(&figure),
                });
            }
            let name = ui.label(l.translate("Figure name"));
            ui.add(TextEdit::singleline(&mut self.drawings_ui.figure_name).desired_width(210.))
                .labelled_by(name.id);
            if ui
                .add_enabled(
                    self.drawings_ui.figure_name != figure.name,
                    Button::new(l.translate("Preview rename")),
                )
                .clicked()
            {
                self.preview_figure(Edit::Rename {
                    id,
                    expected_sha256: diagrams::expected(&figure),
                    name: self.drawings_ui.figure_name.clone(),
                });
            }
            if ui.button(l.translate("Preview delete figure")).clicked() {
                self.preview_figure(Edit::Delete {
                    id,
                    expected_sha256: diagrams::expected(&figure),
                });
            }
        });
        ui.horizontal_wrapped(|ui| {
            let label = ui.label(l.translate("New SVG path"));
            ui.add(TextEdit::singleline(&mut self.drawings_ui.figure_export).desired_width(360.))
                .labelled_by(label.id);
            if ui
                .add_enabled(
                    status == Status::Current,
                    Button::new(l.translate("Export current SVG")),
                )
                .clicked()
            {
                let result = figure
                    .svg(&self.host.document.editor.project)
                    .and_then(|s| {
                        export_new_svg(
                            std::path::Path::new(self.drawings_ui.figure_export.trim()),
                            s.as_bytes(),
                        )
                    });
                self.drawings_ui.error = match result {
                    Ok(()) => l.translate("SVG saved to new file").into(),
                    Err(e) => e,
                };
            }
        });
        if figure.definition.presentation.is_functional() {
            self.function_figure_settings(ui, &figure);
        }
        if figure.definition.presentation.is_axon() {
            self.axon_figure_settings(ui, &figure);
        }
        if matches!(
            figure.definition.presentation,
            Presentation::SetbackStudy { .. }
        ) {
            self.site_figure_settings(ui, &figure);
        }
        if !self.process_ui.error.is_empty() {
            ui.colored_label(
                Color32::from_rgb(224, 125, 105),
                l.translate(&self.process_ui.error),
            );
        }
        if !self.drawings_ui.error.is_empty() {
            ui.label(&self.drawings_ui.error);
        }
        if matches!(
            figure.definition.presentation,
            Presentation::ConstraintOverlay
        ) {
            self.constraint_figure_details(ui, &figure, status);
        }
        ui.small(l.translate("Read-only figure; edit the shared model. Authored quantities are not net floor area or compliance."));
        self.drawings_ui.figure_zoom = self.drawings_ui.figure_zoom.max(1.);
        ui.horizontal(|ui| {
            ui.label(l.translate("Figure zoom"));
            ui.add(Slider::new(&mut self.drawings_ui.figure_zoom, 1. ..=3.).step_by(0.25));
        });
        // Reserve the provenance disclosure and spacing before fitting the page;
        // otherwise the scroll clip hides the bottom source/north line at 1x.
        let page_height = (ui.available_height() - 36.).max(100.);
        let width = ui.available_width().min(page_height * 1.5) * self.drawings_ui.figure_zoom;
        ScrollArea::both()
            .id_salt(("figure_page", id))
            .auto_shrink([false, false])
            .max_height((ui.available_height() - 28.).max(100.))
            .show(ui, |ui| {
                let rect = Self::paint_figure(ui, &figure, l, width);
                self.figure_selection_input(ui, &figure, status, rect);
            });
        ui.collapsing(l.translate("Basis, declarations and unknowns"), |ui| {
            ui.monospace(&figure.source_sha256);
            ui.label(l.translate(
                "Source catalogue declarations do not prove original-byte reads or approvals.",
            ));
            ScrollArea::vertical().max_height(180.).show(ui, |ui| {
                ui.monospace(
                    serde_json::to_string_pretty(&figure.snapshot.context).unwrap_or_default(),
                );
                if figure.definition.presentation.is_site() {
                    ui.monospace(
                        serde_json::to_string_pretty(&diagrams::site::summary(&figure.snapshot))
                            .unwrap_or_default(),
                    );
                }
            });
        });
    }
    fn site_figure_settings(&mut self, ui: &mut Ui, figure: &Diagram) {
        let Presentation::SetbackStudy {
            distance_mm,
            assumption,
        } = &figure.definition.presentation
        else {
            return;
        };
        let expected = diagrams::expected(figure);
        let clean = self
            .drawings_ui
            .figure_site_draft
            .as_ref()
            .is_none_or(|d| d.2 == d.4 && d.3 == d.5);
        if self
            .drawings_ui
            .figure_site_draft
            .as_ref()
            .is_none_or(|d| d.0 != figure.id || (d.1 != expected && clean))
        {
            self.drawings_ui.figure_site_draft = Some((
                figure.id,
                expected.clone(),
                *distance_mm,
                assumption.clone(),
                *distance_mm,
                assumption.clone(),
            ));
        }
        let l = self.language;
        let mut preview = None;
        let mut reload = false;
        ui.collapsing(l.translate("Setback study settings"), |ui| {
            ui.label(
                l.translate("Design assumption only. No approved setback requirement is selected."),
            );
            let d = self.drawings_ui.figure_site_draft.as_mut().unwrap();
            let conflict = d.1 != expected;
            ui.add(Slider::new(&mut d.4, 0..=100000).text(l.translate("Study inset (mm)")));
            let label = ui.label(l.translate("Design assumption"));
            ui.add(TextEdit::singleline(&mut d.5).desired_width(360.))
                .labelled_by(label.id);
            if conflict {
                ui.label(
                    l.translate("Figure changed; reload study settings to discard this draft."),
                );
                if ui.button(l.translate("Reload study settings")).clicked() {
                    reload = true;
                }
            }
            if ui
                .add_enabled(
                    !conflict && (d.4 != *distance_mm || d.5 != *assumption),
                    Button::new(l.translate("Preview study settings")),
                )
                .clicked()
            {
                preview = Some(Edit::Configure {
                    id: figure.id,
                    expected_sha256: d.1.clone(),
                    presentation: Presentation::SetbackStudy {
                        distance_mm: d.4,
                        assumption: d.5.clone(),
                    },
                });
            }
        });
        if reload {
            self.drawings_ui.figure_site_draft = Some((
                figure.id,
                expected,
                *distance_mm,
                assumption.clone(),
                *distance_mm,
                assumption.clone(),
            ));
        } else if let Some(edit) = preview {
            self.preview_figure(edit);
        }
    }
    fn axon_figure_settings(&mut self, ui: &mut Ui, figure: &Diagram) {
        let Presentation::ExplodedAxon { offsets_mm } = &figure.definition.presentation else {
            return;
        };
        let expected = diagrams::expected(figure);
        let step = offsets_mm
            .iter()
            .find(|o| {
                figure
                    .snapshot
                    .elements
                    .get(1)
                    .is_some_and(|e| e.id == o.element_id)
            })
            .map(|v| v.xyz[2])
            .unwrap_or(1000)
            .clamp(0, 3000);
        if self
            .drawings_ui
            .figure_axon_draft
            .as_ref()
            .is_none_or(|d| d.0 != figure.id || (d.1 != expected && d.2 == d.3))
        {
            self.drawings_ui.figure_axon_draft = Some((figure.id, expected.clone(), step, step));
        }
        let l = self.language;
        let mut preview = None;
        let mut reload = false;
        ui.collapsing(l.translate("Axon display separation"), |ui| {
            ui.label(l.translate(
                "Offsets affect this figure only. Objects follow ID order, not storeys.",
            ));
            let draft = self.drawings_ui.figure_axon_draft.as_mut().unwrap();
            let conflict = draft.1 != expected;
            ui.add(
                Slider::new(&mut draft.3, 0..=3000).text(l.translate("Vertical display step (mm)")),
            );
            let offsets: Vec<_> = figure
                .snapshot
                .elements
                .iter()
                .enumerate()
                .map(|(i, e)| diagrams::axon::DisplayOffset {
                    element_id: e.id,
                    xyz: [0, 0, i as i32 * draft.3],
                })
                .collect();
            if conflict {
                ui.label(
                    l.translate("Figure changed; reload axon settings to discard this draft."),
                );
                if ui.button(l.translate("Reload axon settings")).clicked() {
                    reload = true;
                }
            }
            if ui
                .add_enabled(
                    !conflict && offsets != *offsets_mm,
                    Button::new(l.translate("Preview display separation")),
                )
                .clicked()
            {
                preview = Some(Edit::Configure {
                    id: figure.id,
                    expected_sha256: draft.1.clone(),
                    presentation: Presentation::ExplodedAxon {
                        offsets_mm: offsets,
                    },
                });
            }
            for e in &figure.snapshot.elements {
                let xyz = offsets_mm
                    .iter()
                    .find(|o| o.element_id == e.id)
                    .map(|o| o.xyz)
                    .unwrap_or([0; 3]);
                ui.small(format!(
                    "#{} · ΔXYZ ({}, {}, {}) mm",
                    e.id, xyz[0], xyz[1], xyz[2]
                ));
            }
        });
        if reload {
            self.drawings_ui.figure_axon_draft = Some((figure.id, expected, step, step));
        } else if let Some(edit) = preview {
            self.preview_figure(edit);
        }
    }
    fn function_figure_settings(&mut self, ui: &mut Ui, figure: &Diagram) {
        let Presentation::ZoneFunctions { unknown_purposes } = &figure.definition.presentation
        else {
            return;
        };
        let expected = diagrams::expected(figure);
        // A clean UI draft follows Undo/Redo. A dirty draft keeps its original record
        // precondition and requires an explicit reload; observing a new state is no write grant.
        if self
            .drawings_ui
            .figure_unknown_draft
            .as_ref()
            .is_none_or(|d| d.0 != figure.id || (d.1 != expected && d.2 == d.3))
        {
            self.drawings_ui.figure_unknown_draft = Some((
                figure.id,
                expected.clone(),
                unknown_purposes.clone(),
                unknown_purposes.clone(),
            ));
        }
        let language = self.language;
        let mut preview = None;
        let mut reload = false;
        ui.collapsing(language.translate("Declared purpose groups"),|ui|{
            ui.label(language.translate("Exact authored labels, not approved uses. Mark unknown without changing the source Zone."));
            let draft=self.drawings_ui.figure_unknown_draft.as_mut().unwrap();let conflict=draft.1!=expected;
            if conflict{ui.label(language.translate("Figure changed; reload purpose settings to discard this draft."));if ui.button(language.translate("Reload purpose settings")).clicked(){reload=true;}}
            for g in figure.function_groups(){ui.horizontal_wrapped(|ui|{
                ui.label(format!("{} · {:.2} m² · #{}",g.purpose,g.area_m2,g.element_ids.iter().map(u64::to_string).collect::<Vec<_>>().join(",#")));
                let mut unknown=draft.3.contains(&g.purpose);
                if ui.checkbox(&mut unknown,language.translate("Unknown purpose")).changed(){draft.3.retain(|p|p!=&g.purpose);if unknown{draft.3.push(g.purpose.clone());draft.3.sort();}}
            });}
            if ui.add_enabled(!conflict&&draft.3!=*unknown_purposes,Button::new(language.translate("Preview purpose annotations"))).clicked(){preview=Some(Edit::Configure{id:figure.id,expected_sha256:draft.1.clone(),presentation:Presentation::ZoneFunctions{unknown_purposes:draft.3.clone()}});}
        });
        if reload {
            self.drawings_ui.figure_unknown_draft = Some((
                figure.id,
                expected,
                unknown_purposes.clone(),
                unknown_purposes.clone(),
            ));
        } else if let Some(edit) = preview {
            self.preview_figure(edit);
        }
    }
    fn constraint_figure_details(&mut self, ui: &mut Ui, figure: &Diagram, status: Status) {
        let l = self.language;
        let Ok(source) = diagrams::checks::Source::read(&figure.snapshot) else {
            return;
        };
        let mut locate = None;
        let mut check = None;
        if status != Status::Current {
            ui.colored_label(
                Color32::DARK_RED,
                l.translate("Figure is a previous snapshot; update before using its results."),
            );
        }
        ui.collapsing(l.translate("Full check list"),|ui|{
            ui.label(l.translate("Cited originals remain unverified in this portable figure; read originals in checks."));
            ScrollArea::vertical().id_salt(("constraint_figure_list",figure.id)).max_height(220.).show(ui,|ui|{
                for (i,e) in source.entries.iter().enumerate() {ui.group(|ui|{
                    ui.strong(format!("C{} · {} · {}",i+1,e.check.definition.title,l.translate(if status==Status::Current{e.state.label()}else{"Previous value; current result unknown"})));
                    ui.label(format!("{} · {} {} {}",l.translate(e.check.definition.metric.label()),l.translate(e.check.definition.relation.label()),e.check.definition.limit.map(|v|v.to_string()).unwrap_or_else(||l.translate("Unknown").into()),e.check.definition.metric.unit()));
                    for line in crate::diagrams::checks::reference_edge_lines(figure,e.check.definition.metric,&|s|l.translate(s).into()){ui.label(line);}
                    if let Some(report)=&e.check.report {for row in &report.rows {ui.label(format!("v{} · {:?} · {} {}",report.generation,row.ids,row.value.map(|v|format!("{v:.6}")).unwrap_or_else(||l.translate("Unknown").into()),report.input_definition.metric.unit()));}}
                    if let crate::constraints::Basis::DesignAssumption{note}=&e.check.definition.basis{ui.label(l.translate("Design assumption; not a surveyed or statutory limit"));ui.label(note);}
                    for lineage in &source.lineage {if e.active_ids.contains(&lineage.link.zone){ui.label(format!("#{} · {}",lineage.link.zone,l.translate(&lineage.status)));}}
                    if let Some(citation)=&e.citation {ui.label(&citation.text);}
                    ui.horizontal_wrapped(|ui|{
                        for id in &e.active_ids {if ui.add_enabled(self.host.document.editor.project.elements.iter().any(|v|v.id==*id),Button::new(format!("#{id}"))).clicked(){locate=Some(*id);}}
                        if ui.add_enabled(self.host.document.editor.project.rules.checks.iter().any(|c|c.definition.id==e.check.definition.id),Button::new(l.translate("Open check"))).clicked(){check=Some(e.check.definition.id.clone());}
                    });
                });}
                ui.label(format!("{}: {:?}",l.translate("Other scope"),source.other_scope_ids));
                ui.label(format!("{}: {:?}",l.translate("Unlocated checks"),source.unlocated_ids));
            });
        });
        if let Some(id) = locate {
            self.focus_mass_space_element(id);
        }
        if let Some(id) = check {
            self.checks_ui.open = true;
            self.checks_ui.focus = Some(id);
        }
    }
    pub(super) fn paint_figure(
        ui: &mut Ui,
        figure: &Diagram,
        language: UiLanguage,
        width: f32,
    ) -> Rect {
        let (rect, _) = ui.allocate_exact_size(vec2(width, width * 2. / 3.), Sense::hover());
        let painter = ui.painter_at(rect);
        let scale = width / 1200.;
        let point = |q: [f64; 2]| rect.min + vec2(q[0] as f32, q[1] as f32) * scale;
        painter.rect_filled(rect, 0., Color32::WHITE);
        if figure.definition.presentation.is_site() {
            match diagrams::site::Page::new(figure) {
                Ok(page) => {
                    for p in &page.paths {
                        let ring = |r: &Vec<Point>| {
                            geo::LineString::from(r.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>())
                        };
                        let polygon = geo::Polygon::new(
                            ring(&p.rings[0]),
                            p.rings[1..].iter().map(ring).collect(),
                        );
                        let c = p.color();
                        // As in the shared plan painter, fill one triangle mesh
                        // and stroke only the perimeter. Feathering each narrow
                        // cap triangle creates huge miters on sampled buffer arcs.
                        let mut mesh = egui::Mesh::default();
                        for tri in crate::derived_geometry::triangulate_with_boundary(&polygon) {
                            let index = mesh.vertices.len() as u32;
                            for q in tri {
                                mesh.colored_vertex(
                                    point(page.layout.point(Point::new(q.x, q.y))),
                                    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 31),
                                );
                            }
                            mesh.add_triangle(index, index + 1, index + 2);
                        }
                        painter.add(egui::Shape::mesh(mesh));
                        for ring in &p.rings {
                            painter.add(egui::Shape::closed_line(
                                ring.iter().map(|p| point(page.layout.point(*p))).collect(),
                                Stroke::new(
                                    (2. * scale).max(0.6),
                                    Color32::from_rgb(c[0], c[1], c[2]),
                                ),
                            ));
                        }
                    }
                    for (id, p, centered) in page.labels() {
                        painter.text(
                            point(p),
                            if centered {
                                Align2::CENTER_CENTER
                            } else {
                                Align2::LEFT_BOTTOM
                            },
                            format!("#{id}"),
                            FontId::proportional(12. * scale),
                            Color32::BLACK,
                        );
                    }
                    for (i, (_, c, label)) in diagrams::site::legend(&page).iter().enumerate() {
                        painter.rect_filled(
                            Rect::from_min_size(
                                point([835., diagrams::site::legend_y(&page, i)]),
                                vec2(10., 10.) * scale,
                            ),
                            0.,
                            Color32::from_rgb(c[0], c[1], c[2]),
                        );
                        painter.text(
                            point([853., diagrams::site::legend_y(&page, i) + 10.]),
                            Align2::LEFT_BOTTOM,
                            language.translate(label),
                            FontId::proportional(11. * scale),
                            Color32::BLACK,
                        );
                    }
                    for (x, y, text, size) in
                        figure.localized_text_lines(|s| language.translate(s).into())
                    {
                        painter.text(
                            point([x, y]),
                            Align2::LEFT_BOTTOM,
                            text,
                            FontId::proportional(size * scale),
                            Color32::BLACK,
                        );
                    }
                    let len = page.layout.scale_length();
                    painter.line_segment(
                        [
                            point([50., 700.]),
                            point([50. + len * page.layout.pixels_per_m, 700.]),
                        ],
                        Stroke::new(1.5, Color32::BLACK),
                    );
                    painter.text(
                        point([50., 693.]),
                        Align2::LEFT_BOTTOM,
                        format!("{len} m (model)"),
                        FontId::proportional(13. * scale),
                        Color32::BLACK,
                    );
                    if let Some(n) = &figure.snapshot.true_north {
                        let a = n.clockwise_from_model_y_deg.to_radians();
                        painter.arrow(
                            point([770., 680.]),
                            vec2(a.sin() as f32, -a.cos() as f32) * 35. * scale,
                            Stroke::new(1., Color32::BLACK),
                        );
                        painter.text(
                            point([720., 715.]),
                            Align2::LEFT_BOTTOM,
                            language.translate("N (declared)"),
                            FontId::proportional(12. * scale),
                            Color32::BLACK,
                        );
                    }
                }
                Err(e) => {
                    painter.text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        language.translate(&e),
                        FontId::proportional(14.),
                        Color32::DARK_RED,
                    );
                }
            }
            return rect;
        }
        if figure.definition.presentation.is_axon() {
            match diagrams::axon::Page::new(figure) {
                Ok(page) => {
                    for f in &page.faces {
                        let c = f.color;
                        painter.add(egui::Shape::convex_polygon(
                            f.points.map(|q| point(page.point(q))).to_vec(),
                            Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 115),
                            Stroke::NONE,
                        ));
                    }
                    for e in &page.edges {
                        painter.line_segment(
                            e.points.map(|q| point(page.point(q))),
                            Stroke::new(
                                scale.max(0.6),
                                Color32::from_rgba_unmultiplied(48, 59, 70, 166),
                            ),
                        );
                    }
                    for label in &page.labels {
                        painter.text(
                            point(page.point(label.point)),
                            Align2::CENTER_CENTER,
                            format!("#{}", label.element_id),
                            FontId::proportional(12. * scale),
                            Color32::BLACK,
                        );
                    }
                    for (name, v) in diagrams::axon::Page::axes() {
                        painter.arrow(
                            point([780., 735.]),
                            vec2(v[0] as f32, v[1] as f32) * 35. * scale,
                            Stroke::new(1., Color32::DARK_GRAY),
                        );
                        painter.text(
                            point([780. + v[0] * 48., 735. + v[1] * 48.]),
                            Align2::CENTER_CENTER,
                            name,
                            FontId::proportional(11. * scale),
                            Color32::BLACK,
                        );
                    }
                    for (x, y, text, size) in
                        figure.localized_text_lines(|s| language.translate(s).into())
                    {
                        painter.text(
                            point([x, y]),
                            Align2::LEFT_BOTTOM,
                            text,
                            FontId::proportional(size * scale),
                            Color32::BLACK,
                        );
                    }
                }
                Err(e) => {
                    painter.text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        e,
                        FontId::proportional(14.),
                        Color32::DARK_RED,
                    );
                }
            }
            return rect;
        }
        let layout = diagrams::Layout::new(figure);
        for e in &figure.snapshot.elements {
            let (outer, holes) = e.authored_prism_rings().unwrap();
            let ring = |r: &Vec<Point>| {
                geo::LineString::from(r.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>())
            };
            let polygon = geo::Polygon::new(ring(&outer), holes.iter().map(ring).collect());
            let rgb = figure.element_color(e);
            let color = Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], 153);
            for triangle in crate::derived_geometry::triangulate_with_boundary(&polygon) {
                let points = triangle
                    .map(|c| point(layout.point(Point::new(c.x, c.y))))
                    .to_vec();
                painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
            }
            for ring in std::iter::once(&outer).chain(holes.iter()) {
                painter.add(egui::Shape::closed_line(
                    ring.iter().map(|p| point(layout.point(*p))).collect(),
                    Stroke::new(scale.max(0.6), Color32::DARK_GRAY),
                ));
            }
            if !figure.definition.presentation.is_default()
                && let Shape::Zone { spec } = &e.shape
            {
                let name: String = e.name.chars().take(12).collect();
                painter.text(
                    point(layout.point(spec.label_point())),
                    Align2::CENTER_CENTER,
                    format!("#{} {}", e.id, name),
                    FontId::proportional(12. * scale),
                    Color32::BLACK,
                );
            } else {
                painter.text(
                    point(layout.point(outer[0])) + vec2(4., -4.) * scale,
                    Align2::LEFT_BOTTOM,
                    format!("#{}", e.id),
                    FontId::proportional(12. * scale),
                    Color32::BLACK,
                );
            }
        }
        for (i, g) in figure.function_groups().iter().enumerate() {
            let c = g.color;
            painter.rect_filled(
                Rect::from_min_size(point([835., 130. + i as f64 * 17.]), vec2(10., 10.) * scale),
                0.,
                Color32::from_rgb(c[0], c[1], c[2]),
            );
        }
        for (x, y, text, size) in figure.localized_text_lines(|s| language.translate(s).to_owned())
        {
            painter.text(
                point([x, y]),
                Align2::LEFT_BOTTOM,
                text,
                FontId::proportional(size * scale),
                Color32::BLACK,
            );
        }
        let length = layout.scale_length();
        let a = point([50., 738.]);
        let b = point([50. + length * layout.pixels_per_m, 738.]);
        painter.line_segment([a, b], Stroke::new(1.5, Color32::BLACK));
        for q in [a, b] {
            painter.line_segment(
                [q, q - vec2(0., 8. * scale)],
                Stroke::new(1., Color32::BLACK),
            );
        }
        painter.text(
            a - vec2(0., 10. * scale),
            Align2::LEFT_BOTTOM,
            format!("{length} m"),
            FontId::proportional(14. * scale),
            Color32::BLACK,
        );
        if let Some(n) = &figure.snapshot.true_north {
            let a = n.clockwise_from_model_y_deg.to_radians();
            let start = point([770., 680.]);
            painter.arrow(
                start,
                vec2(a.sin() as f32, -a.cos() as f32) * 35. * scale,
                Stroke::new(1.5, Color32::BLACK),
            );
            painter.text(
                point([720., 738.]),
                Align2::LEFT_BOTTOM,
                language.translate("N (declared)"),
                FontId::proportional(13. * scale),
                Color32::BLACK,
            );
        }
        rect
    }
}
pub(super) fn export_new_svg(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    if path
        .extension()
        .is_none_or(|e| !e.eq_ignore_ascii_case("svg"))
    {
        return Err("Choose a new .svg file path".into());
    }
    let parent = path.parent().ok_or("Choose a new .svg file path")?;
    let temporary = parent.join(format!(
        ".openarchi-{}.svg.tmp",
        crate::storage::new_document_identity()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        std::fs::hard_link(&temporary, path).map_err(|e| e.to_string())
    })();
    drop(file);
    let _ = std::fs::remove_file(temporary);
    result
}
