//! Inline controls for the one supported rule. No separate model or task runner.
use super::*;
use crate::design_process::parametric::{self, Definition, Parameters};
pub(super) struct Draft {
    pub base: crate::application_host::CommandContext,
    initial: Option<Parameters>,
    pub id: String,
    pub task: Option<String>,
    pub question: String,
    pub definition: Definition,
    pub story: u32,
    pub layer: u32,
    pub discussion: Option<(
        String,
        crate::evidence::topics::Exchange,
        Option<crate::evidence::topics::Topic>,
    )>,
    pub input_review: Option<(parametric::Inputs, bool)>,
}
pub(super) struct CanvasMove {
    pub option: u32,
    pub comparison: String,
    pub initial: Parameters,
    pub start: Option<Point>,
    pub dragged: bool,
    pub fit_pending: bool,
}
impl AtelierApp {
    pub(super) fn pending_parametric_parameters(&self) -> bool {
        self.research_ui.canvas_move.is_some()
            || self.research_ui.parameters.as_ref().is_some_and(|d| {
                d.input_review.is_some()
                    || d.initial
                        .as_ref()
                        .is_none_or(|p| p != &d.definition.candidates[0])
            })
    }

    pub(super) fn begin_parametric_from_discussion(&mut self) {
        let result = (|| {
            let d = self
                .research_ui
                .draft
                .as_ref()
                .ok_or("Start with an object question")?;
            if d.base != self.host.context()
                || d.exchange.question.trim().is_empty()
                || self.process_ui.preview.is_some()
            {
                return Err("Discussion inputs changed or a preview is pending".to_owned());
            }
            let p = &self.host.document.editor.project;
            let element = d.exchange.objects.iter().find_map(|o| match o.link.target {
                crate::evidence::Target::Element { id } => p.elements.iter().find(|e| e.id == id),
                _ => None,
            });
            let origin = element
                .and_then(|e| match &e.shape {
                    Shape::ConceptMass { spec } => spec.outline.first().copied(),
                    Shape::Zone { spec } => spec.outline.first().copied(),
                    _ => None,
                })
                .unwrap_or(Point::new(0., 0.));
            let a = Parameters {
                origin_mm: [
                    (origin.x * 1000.).round() as i64,
                    (origin.y * 1000.).round() as i64,
                ],
                court_width_mm: 20000,
                court_depth_mm: 14000,
                wing_depth_mm: 6000,
                storeys: 2,
                floor_height_mm: 3300,
            };
            let mut b = a.clone();
            b.court_width_mm = 18000;
            let definition=Definition{rule_version:1,candidates:[a,b],site_sha256:crate::automation_journal::canonical_hash(&p.site)?,sources:d.exchange.objects.iter().map(|o|o.link.clone()).collect(),assumptions:"Explicit concept assumptions: dimensions, storeys and height are authored, origin starts at the referenced authored object; no measured scale, legal floor area, capacity or approval inferred.".into()};
            self.research_ui.parameters = Some(Draft {
                base: d.base,
                initial: None,
                id: storage::new_document_identity(),
                task: None,
                question: d.exchange.question.clone(),
                definition,
                story: element.map_or(0, |e| e.story),
                layer: 0,
                discussion: None,
                input_review: None,
            });
            Ok(())
        })();
        self.research_ui.error = result.err().unwrap_or_default();
    }
    pub(super) fn begin_parametric_adjustment(&mut self) {
        let result = (|| {
            if self.process_ui.preview.is_some()
                || self.workbench_pointer_active()
                || self.research_ui.parameters.is_some()
                || self.project_conditions_has_draft()
                || self.pending_other_design_parameters()
            {
                return Err(
                    "Finish or cancel the current model interaction before changing design tasks"
                        .into(),
                );
            }
            let p = &self.host.document.editor.project;
            let task = p.design_process.task().ok_or("No parametric task")?;
            let option = p
                .plan_options
                .active_design_option
                .ok_or("Activate a parametric candidate")?;
            let parameters = parametric::current_parameters(p, &task.id, option)?;
            let mut definition = task
                .intent
                .as_ref()
                .and_then(|v| v.parametric.clone())
                .ok_or("No parametric definition for this task")?;
            let inputs = parametric::current_inputs(p, &task.id, option)?;
            let input_review = if inputs.check_current(p).is_err() {
                Some((parametric::capture_inputs(p, &task.id, option)?, false))
            } else {
                None
            };
            definition.site_sha256 = inputs.site_sha256;
            definition.sources = inputs.sources;
            definition.candidates[0] = parameters.clone();
            self.research_ui.parameters = Some(Draft {
                base: self.host.context(),
                initial: Some(parameters),
                id: storage::new_document_identity(),
                task: Some(task.id.clone()),
                question: task.question.clone(),
                definition,
                story: task.story,
                layer: 0,
                discussion: None,
                input_review,
            });
            self.project_conditions_ui.open = false;
            self.program_ui.open = false;
            self.process_ui.error.clear();
            Ok::<_, String>(())
        })();
        self.research_ui.error = result.err().unwrap_or_default();
        // Explicit entry reveals the existing controls once; subsequent edits keep
        // the user's scroll position. Opening a draft never changes task or authority.
        self.show_right = true;
        self.tab = Tab::Assistant;
        self.research_ui.focus_question = true;
    }
    pub(super) fn bound_parametric_discussion(&self) -> bool {
        let Some(d) = &self.research_ui.draft else {
            return false;
        };
        let p = &self.host.document.editor.project;
        let Some(task) = p
            .design_process
            .tasks
            .iter()
            .find(|t| Some(&t.id) == d.exchange.scope.task.as_ref())
        else {
            return false;
        };
        d.exchange.scope.parameter_input_sha256.is_some()
            && task.intent.as_ref().is_some_and(|i| i.parametric.is_some())
            && task
                .candidates
                .iter()
                .any(|c| Some(c.option) == d.exchange.scope.option)
            && d.exchange.objects.iter().all(|o| match o.link.target {
                crate::evidence::Target::Element { id } => p
                    .elements
                    .iter()
                    .any(|e| e.id == id && e.design_option == d.exchange.scope.option),
                crate::evidence::Target::ProgramRow { .. } => true,
            })
    }
    pub(super) fn adjust_parametric_from_discussion(&mut self) {
        let result = (|| {
            if self.research_ui.parameters.is_some()
                || self.research_ui.canvas_move.is_some()
                || self.process_ui.preview.is_some()
            {
                return Err(
                    "Finish or cancel the current model interaction before changing design tasks"
                        .to_owned(),
                );
            }
            let d = self
                .research_ui
                .draft
                .as_ref()
                .ok_or("Start with an object question")?;
            let p = &self.host.document.editor.project;
            if !self.bound_parametric_discussion()
                || d.base != self.host.context()
                || d.exchange.scope != crate::evidence::topics::Scope::capture(p)
                || d.exchange
                    .objects
                    .iter()
                    .any(|o| o.link.status(p) != crate::evidence::LinkStatus::Unchanged)
                || d.exchange.question.trim().is_empty()
            {
                return Err(
                    "Discussion inputs changed; explicitly capture a new contribution".to_owned(),
                );
            }
            let snapshot = (d.id.clone(), d.exchange.clone(), d.previous.clone());
            self.begin_parametric_adjustment();
            let parameters = self
                .research_ui
                .parameters
                .as_mut()
                .ok_or_else(|| self.research_ui.error.clone())?;
            parameters.question = snapshot.1.question.clone();
            parameters.discussion = Some(snapshot);
            Ok(())
        })();
        self.research_ui.error = result.err().unwrap_or_default();
    }
    pub(super) fn preview_parametric(&mut self) {
        let result = (|| {
            let d = self
                .research_ui
                .parameters
                .as_ref()
                .ok_or("No parametric draft")?;
            if d.base != self.host.context() {
                return Err(
                    "Discussion inputs changed; explicitly capture a new contribution".into(),
                );
            }
            if let Some(task) = &d.task {
                let reviewed = match &d.input_review {
                    Some((_, false)) => {
                        return Err(
                            "Review changed design inputs before updating this candidate".into(),
                        );
                    }
                    Some((inputs, true)) => Some(inputs.clone()),
                    None => None,
                };
                let change = parametric::prepare_update_with_inputs(
                    &self.host.document.editor,
                    &d.id,
                    task,
                    d.definition.candidates[0].clone(),
                    d.question.clone(),
                    reviewed,
                    crate::design_process::Entry::ManualInterface,
                )?;
                if let Some((id, exchange, previous)) = &d.discussion {
                    let mut commands = vec![crate::command::ModelCommand::Evidence {
                        edit: crate::evidence::EvidenceEdit::RecordTopic {
                            id: id.clone(),
                            exchange: exchange.clone(),
                            expected_previous: previous.clone(),
                        },
                    }];
                    commands.extend_from_slice(change.commands());
                    self.host
                        .document
                        .editor
                        .prepare_change(d.base.revision, commands)
                        .map_err(|e| e.message)
                } else {
                    Ok(change)
                }
            } else {
                parametric::prepare(
                    &self.host.document.editor,
                    &d.id,
                    "Two-wing concept study".into(),
                    d.question.clone(),
                    d.definition.clone(),
                    d.story,
                    d.layer,
                )
            }
        })();
        let id = self
            .research_ui
            .parameters
            .as_ref()
            .map(|d| d.id.clone())
            .unwrap_or_else(storage::new_document_identity);
        self.process_set_preview(id, "Parametric two-wing proposal", result);
    }
    pub(super) fn parametric_contents(&mut self, ui: &mut Ui) {
        let l = self.language;
        let current_context = self.host.context();
        let has_rule = self
            .host
            .document
            .editor
            .project
            .design_process
            .task()
            .is_some_and(|t| t.intent.as_ref().is_some_and(|v| v.parametric.is_some()));
        if has_rule
            && self.research_ui.parameters.is_none()
            && ui.button(l.translate("Adjust generated wings")).clicked()
        {
            self.begin_parametric_adjustment();
        }
        if has_rule
            && self.research_ui.parameters.is_none()
            && ui
                .button(l.translate("Move generated wings on canvas"))
                .clicked()
        {
            self.begin_parametric_canvas_move();
        }
        if self.research_ui.canvas_move.is_some() {
            ui.label(
                l.translate("Drag the blue origin handle. Release to review; Escape cancels."),
            );
            if ui
                .button(l.translate("Cancel canvas parameter move"))
                .clicked()
            {
                self.cancel_parametric_canvas_move(false);
            }
            return;
        }
        let project = &self.host.document.editor.project;
        if let Some(d) = &mut self.research_ui.parameters {
            ui.group(|ui| {
                ui.strong(l.translate("L-shaped two-wing concept"));
                ui.label(l.translate(
                    "Editable concept assumptions; not survey, usable floor area or compliance",
                ));
                ui.collapsing(l.translate("Your design question"), |ui| {
                    ui.label(&d.question);
                    ui.label(&d.definition.assumptions);
                });
                if !self.process_ui.error.is_empty() {
                    ui.colored_label(Color32::LIGHT_RED, l.translate(&self.process_ui.error));
                }
                if let Some((inputs, reviewed)) = &mut d.input_review {
                    ui.label(l.translate(
                        "Design inputs changed; the original rule and other candidate are retained",
                    ));
                    changed_input_summary(ui, l, &d.definition.inputs(), inputs, project);
                    ui.add_enabled(
                        current_context == d.base,
                        egui::Checkbox::new(
                            reviewed,
                            l.translate(
                                "I reviewed these changed inputs; update only this candidate",
                            ),
                        ),
                    );
                }
                let count = if d.task.is_some() { 1 } else { 2 };
                for i in 0..count {
                    ui.push_id(i, |ui| {
                        ui.strong(if d.task.is_some() {
                            l.translate("Current candidate")
                        } else if i == 0 {
                            l.translate("Candidate A")
                        } else {
                            l.translate("Candidate B")
                        });
                        let p = &mut d.definition.candidates[i];
                        egui::Grid::new("two_wing_parameters")
                            .num_columns(2)
                            .show(ui, |ui| {
                                let [origin_x, origin_y] = &mut p.origin_mm;
                                for (label, value) in
                                    [("Origin X (mm)", origin_x), ("Origin Y (mm)", origin_y)]
                                {
                                    ui.label(l.translate(label));
                                    let response = ui.add(DragValue::new(value).speed(100));
                                    #[cfg(test)]
                                    ui.ctx().data_mut(|data| {
                                        data.insert_temp(
                                            Id::new(("parametric", i, label)),
                                            response.id,
                                        )
                                    });
                                    let _ = response;
                                    ui.end_row();
                                }
                                for (label, value) in [
                                    ("Court width (mm)", &mut p.court_width_mm),
                                    ("Court depth (mm)", &mut p.court_depth_mm),
                                    ("Wing depth (mm)", &mut p.wing_depth_mm),
                                    ("Concept storeys", &mut p.storeys),
                                    ("Floor height (mm)", &mut p.floor_height_mm),
                                ] {
                                    ui.label(l.translate(label));
                                    let response =
                                        ui.add(DragValue::new(value).speed(
                                            if label == "Concept storeys" { 1 } else { 100 },
                                        ));
                                    #[cfg(test)]
                                    ui.ctx().data_mut(|data| {
                                        data.insert_temp(
                                            Id::new(("parametric", i, label)),
                                            response.id,
                                        )
                                    });
                                    let _ = response;
                                    ui.end_row();
                                }
                            });
                        match p.geometry() {
                            Ok(g) => {
                                let area = g.iter().map(|s| s.area()).sum::<f64>();
                                let volume = g.iter().map(|s| s.volume()).sum::<f64>();
                                ui.label(format!(
                                    "{}: {area:.2} m² · {}: {volume:.2} m³",
                                    l.translate("Mass footprint sum"),
                                    l.translate("Concept envelope volume sum")
                                ));
                            }
                            Err(e) => {
                                ui.colored_label(Color32::LIGHT_RED, l.translate(&e));
                            }
                        }
                    });
                }
            });
        }
    }
    pub(super) fn parametric_draft_actions(&mut self, ui: &mut Ui) {
        let l = self.language;
        let (preview, cancel) = ui
            .horizontal(|ui| {
                let width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.;
                let preview = ui
                    .add_sized(
                        [width, 36.],
                        egui::Button::new(l.translate("Preview two-wing parameters")).wrap(),
                    )
                    .clicked();
                let cancel = ui
                    .add_sized(
                        [width, 36.],
                        egui::Button::new(l.translate("Cancel parameter draft")).wrap(),
                    )
                    .clicked();
                (preview, cancel)
            })
            .inner;
        if cancel {
            self.research_ui.parameters = None;
        }
        if preview {
            self.preview_parametric();
            if self.process_ui.preview.is_none() {
                self.research_ui.focus_question = true;
            }
        }
    }
}

impl AtelierApp {
    fn begin_parametric_canvas_move(&mut self) {
        if self.research_ui.parameters.is_some() {
            return;
        }
        if self.view != View::Plan
            || self.trace.pointer_mode_active()
            || self.trace.enabled
            || self
                .host
                .document
                .editor
                .project
                .plan_options
                .active_worksheet
                .is_some()
            || self.pending_design_parameters()
            || self.project_conditions_has_draft()
            || self.menu_gated()
        {
            self.research_ui.error =
                "Return to the ordinary plan and finish pending settings before moving parameters"
                    .into();
            return;
        }
        self.begin_parametric_adjustment();
        if self.research_ui.parameters.is_none() {
            return;
        }
        let result = (|| {
            let d = self
                .research_ui
                .parameters
                .as_ref()
                .ok_or("No parametric draft")?;
            let p = &self.host.document.editor.project;
            if d.input_review.is_some() {
                return Err(
                    "Review changed design inputs in the parameter controls before a canvas move"
                        .into(),
                );
            }
            let task = p.design_process.task().ok_or("No parametric task")?;
            let option = p
                .plan_options
                .active_design_option
                .ok_or("Activate a parametric candidate")?;
            let members = task
                .candidates
                .iter()
                .find(|c| c.option == option)
                .ok_or("Activate a parametric candidate")?;
            if members
                .initial_mass_ids
                .iter()
                .any(|id| self.host.document.editor.locked_element(*id))
            {
                return Err(
                    "Manual changes are protected; regeneration cannot replace these members"
                        .into(),
                );
            }
            let state = CanvasMove {
                option,
                comparison: task.compare(p)?.fingerprint,
                initial: d.definition.candidates[0].clone(),
                start: None,
                dragged: false,
                fit_pending: true,
            };
            self.host.document.editor.set_tool(Tool::Select);
            self.research_ui.canvas_move = Some(state);
            Ok::<_, String>(())
        })();
        if let Err(e) = result {
            self.research_ui.parameters = None;
            self.research_ui.error = e;
        }
    }
    pub(super) fn cancel_parametric_canvas_move(&mut self, conflict: bool) {
        self.research_ui.canvas_move = None;
        self.research_ui.parameters = None;
        if conflict {
            self.research_ui.error =
                "Discussion inputs changed; explicitly capture a new contribution".into();
        }
    }
    pub(super) fn parametric_canvas_context_current(&self) -> bool {
        let Some(d) = self.research_ui.parameters.as_ref() else {
            return false;
        };
        let Some(m) = self.research_ui.canvas_move.as_ref() else {
            return false;
        };
        let p = &self.host.document.editor.project;
        d.base == self.host.context()
            && d.task == p.design_process.current_task
            && p.plan_options.active_design_option == Some(m.option)
            && p.plan_options.active_worksheet.is_none()
    }
    pub(super) fn fit_parametric_canvas_if_pending(&mut self, r: Rect) {
        if !self
            .research_ui
            .canvas_move
            .as_ref()
            .is_some_and(|m| m.fit_pending)
        {
            return;
        }
        self.research_ui.canvas_move.as_mut().unwrap().fit_pending = false;
        let Some(d) = self.research_ui.parameters.as_ref() else {
            return;
        };
        let parameters = &d.definition.candidates[0];
        let Ok(shapes) = parameters.geometry() else {
            return;
        };
        let mut points = vec![self.screen(
            r,
            Point::new(
                parameters.origin_mm[0] as f64 / 1000.,
                parameters.origin_mm[1] as f64 / 1000.,
            ),
        )];
        points.extend(
            shapes
                .iter()
                .flat_map(|s| &s.outline)
                .map(|p| self.screen(r, *p)),
        );
        let bounds = Rect::from_points(&points);
        let viewport = r.shrink(30.);
        if viewport.width() > 1. && viewport.height() > 1. && !viewport.contains_rect(bounds) {
            let scale = (viewport.width() / bounds.width().max(1.))
                .min(viewport.height() / bounds.height().max(1.));
            self.plan_center = self.world(r, bounds.center());
            self.plan_scale =
                (self.plan_scale * f64::from(scale)).clamp(3., self.plan_view_scale_max(250.));
        }
    }
    pub(super) fn parametric_canvas_input(
        &mut self,
        ui: &mut Ui,
        r: Rect,
        response: &egui::Response,
        interactive: bool,
    ) -> bool {
        if self.research_ui.canvas_move.is_none() {
            return false;
        }
        if self.menu_gated() {
            self.cancel_parametric_canvas_move(false);
            return true;
        }
        if !self.parametric_canvas_context_current() {
            self.cancel_parametric_canvas_move(true);
            return true;
        }
        let pointer = ui.input(|i| i.pointer.latest_pos());
        let dragging = self
            .research_ui
            .canvas_move
            .as_ref()
            .unwrap()
            .start
            .is_some();
        if dragging
            && (pointer.is_none_or(|p| !r.contains(p)) || !interactive || !ui.input(|i| i.focused))
        {
            self.cancel_parametric_canvas_move(false);
            return true;
        }
        let initial = self
            .research_ui
            .canvas_move
            .as_ref()
            .unwrap()
            .initial
            .clone();
        let handle = self.screen(
            r,
            Point::new(
                initial.origin_mm[0] as f64 / 1000.,
                initial.origin_mm[1] as f64 / 1000.,
            ),
        );
        // press_origin is cleared on release. Retain the actual start even
        // when a complete native gesture arrives in one input batch.
        let (raw_press, ambiguous_press) = ui.input(|i| {
            let mut presses = i.raw.events.iter().filter_map(|event| match event {
                egui::Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    ..
                } => Some(*pos),
                _ => None,
            });
            (presses.next(), presses.next().is_some())
        });
        if ambiguous_press {
            self.cancel_parametric_canvas_move(false);
            return true;
        }
        let press_origin = ui.input(|i| i.pointer.press_origin()).or(raw_press);
        if interactive
            && response.hovered()
            && ui.input(|i| i.focused)
            && ui.input(|i| i.pointer.primary_pressed() && !i.key_down(Key::Space))
            && press_origin.is_some_and(|p| p.distance(handle) <= 10.)
        {
            self.research_ui.canvas_move.as_mut().unwrap().start =
                press_origin.map(|p| self.world(r, p));
        }
        let start = self.research_ui.canvas_move.as_ref().unwrap().start;
        if let Some(start) = start {
            // The widget may never acquire a dragged id in a coalesced batch.
            // Use egui's unchanged click threshold for this captured handle.
            let released_drag = ui.input(|i| {
                i.pointer.primary_released()
                    && (i.pointer.is_decidedly_dragging()
                        || (raw_press.is_some()
                            && !i.pointer.could_any_button_be_click()
                            && !i.pointer.any_click()))
            });
            if response.dragged_by(PointerButton::Primary)
                || response.drag_stopped()
                || released_drag
            {
                self.research_ui.canvas_move.as_mut().unwrap().dragged = true;
            }
            if self.research_ui.canvas_move.as_ref().unwrap().dragged
                && let Some(at) = pointer
            {
                let target = self.world(r, at);
                let mut p = initial.clone();
                p.origin_mm = [
                    initial.origin_mm[0] + ((target.x - start.x) * 1000.).round() as i64,
                    initial.origin_mm[1] + ((target.y - start.y) * 1000.).round() as i64,
                ];
                self.research_ui
                    .parameters
                    .as_mut()
                    .unwrap()
                    .definition
                    .candidates[0] = p;
            }
            if ui.input(|i| i.pointer.primary_released()) {
                let unchanged = !self.research_ui.canvas_move.as_ref().unwrap().dragged
                    || self
                        .research_ui
                        .parameters
                        .as_ref()
                        .unwrap()
                        .definition
                        .candidates[0]
                        == initial;
                let current = self
                    .host
                    .document
                    .editor
                    .project
                    .design_process
                    .task()
                    .and_then(|t| t.compare(&self.host.document.editor.project).ok())
                    .is_some_and(|c| {
                        c.fingerprint == self.research_ui.canvas_move.as_ref().unwrap().comparison
                    });
                if !current {
                    self.cancel_parametric_canvas_move(true);
                } else if unchanged {
                    self.cancel_parametric_canvas_move(false);
                } else {
                    self.research_ui.canvas_move = None;
                    self.preview_parametric();
                }
            }
        }
        true
    }
    pub(super) fn paint_parametric_canvas(&self, ui: &Ui, r: Rect) {
        let Some(d) = self.research_ui.parameters.as_ref() else {
            return;
        };
        let moving = self.research_ui.canvas_move.as_ref();
        if moving.is_none() {
            let project = &self.host.document.editor.project;
            // A numeric draft is a read-only projection of the existing rule.
            // Never present a stale draft as belonging to a newly selected task.
            if self.view != View::Plan
                || self.trace.enabled
                || project.plan_options.active_worksheet.is_some()
                || d.base != self.host.context()
                || d.task.as_ref() != project.design_process.current_task.as_ref()
                || d.initial
                    .as_ref()
                    .is_none_or(|initial| initial == &d.definition.candidates[0])
            {
                return;
            }
        }
        let parameters = &d.definition.candidates[0];
        let painter = ui.painter_at(r);
        let color = if parameters.geometry().is_ok() {
            ACCENT
        } else {
            Color32::LIGHT_RED
        };
        if let Ok(shapes) = parameters.geometry() {
            for shape in &shapes {
                painter.add(egui::Shape::closed_line(
                    shape.outline.iter().map(|p| self.screen(r, *p)).collect(),
                    Stroke::new(2., color),
                ));
            }
            if moving.is_none() {
                let l = self.language;
                painter.text(
                    r.left_bottom() + vec2(12., -28.),
                    Align2::LEFT_BOTTOM,
                    format!(
                        "{} · {}: {:.2} m\n{}: {:.2} m² · {}: {:.2} m³",
                        l.translate("Unconfirmed two-wing draft"),
                        l.translate("Concept envelope height"),
                        shapes[0].height,
                        l.translate("Mass footprint sum"),
                        shapes.iter().map(|s| s.area()).sum::<f64>(),
                        l.translate("Concept envelope volume sum"),
                        shapes.iter().map(|s| s.volume()).sum::<f64>(),
                    ),
                    FontId::proportional(12.),
                    color,
                );
            }
        }
        let Some(m) = moving else {
            // Numeric previews do not install a handle, consume pointer input or
            // set a cursor. Invalid parameters leave no previous ghost behind.
            return;
        };
        let at = self.screen(
            r,
            Point::new(
                parameters.origin_mm[0] as f64 / 1000.,
                parameters.origin_mm[1] as f64 / 1000.,
            ),
        );
        painter.circle_filled(at, 6., Color32::WHITE);
        painter.circle_stroke(at, 7., Stroke::new(2., color));
        painter.line_segment(
            [at - vec2(10., 0.), at + vec2(10., 0.)],
            Stroke::new(1.5, color),
        );
        painter.line_segment(
            [at - vec2(0., 10.), at + vec2(0., 10.)],
            Stroke::new(1.5, color),
        );
        painter.text(
            at + vec2(12., -12.),
            Align2::LEFT_BOTTOM,
            self.language.translate("Move both generated wings"),
            FontId::proportional(12.),
            color,
        );
        let hovered = ui.input(|i| {
            i.pointer
                .hover_pos()
                .is_some_and(|p| r.contains(p) && p.distance(at) <= 10.)
        });
        ui.ctx().set_cursor_icon(if m.start.is_some() {
            CursorIcon::Grabbing
        } else if hovered {
            CursorIcon::Move
        } else {
            CursorIcon::Default
        });
        #[cfg(test)]
        ui.ctx()
            .data_mut(|data| data.insert_temp(Id::new("parametric_canvas_handle"), at));
    }
}

fn parameter_rows(p: &Parameters) -> [(&'static str, String); 7] {
    [
        ("Origin X (mm)", p.origin_mm[0].to_string()),
        ("Origin Y (mm)", p.origin_mm[1].to_string()),
        ("Court width (mm)", p.court_width_mm.to_string()),
        ("Court depth (mm)", p.court_depth_mm.to_string()),
        ("Wing depth (mm)", p.wing_depth_mm.to_string()),
        ("Concept storeys", p.storeys.to_string()),
        ("Floor height (mm)", p.floor_height_mm.to_string()),
    ]
}
impl AtelierApp {
    pub(super) fn parametric_preview_contents(
        ui: &mut Ui,
        before: &Project,
        after: &Project,
        l: UiLanguage,
    ) {
        for record in &after.design_process.updates {
            if before
                .design_process
                .updates
                .iter()
                .any(|old| old.id == record.id)
            {
                continue;
            }
            let Some(parameters) = &record.parametric_parameters else {
                continue;
            };
            if let [
                crate::design_process::update::Source::ReviewedParametricDefinition {
                    inputs, ..
                },
            ] = record.sources.as_slice()
            {
                ui.strong(l.translate("Reviewed input version"));
                if let Ok(old) = parametric::current_inputs(before, &record.task_id, record.option)
                {
                    changed_input_summary(ui, l, &old, inputs, before);
                }
            }
            ui.strong(l.translate("Review parameter version"));
            if record.before_sha256 == record.after_sha256 {
                ui.label(
                    l.translate("Envelope unchanged; parameter and comparison versions change"),
                );
            }
            let old = parametric::current_parameters(before, &record.task_id, record.option).ok();
            for (i, (label, value)) in parameter_rows(parameters).into_iter().enumerate() {
                let previous = old.as_ref().map(|p| parameter_rows(p)[i].1.clone());
                if previous.as_ref() != Some(&value) {
                    ui.label(format!(
                        "{}: {} → {value}",
                        l.translate(label),
                        previous.as_deref().unwrap_or(l.translate("Unknown"))
                    ));
                }
            }
        }
    }
    pub(super) fn parameter_version_summary(
        ui: &mut Ui,
        l: UiLanguage,
        version: &crate::design_process::Version,
        is_parametric: bool,
    ) {
        if let Some(p) = &version.parametric_parameters {
            ui.label(l.translate(if version.parametric_inputs.is_some() {
                "Reviewed input version"
            } else {
                "Original input version"
            }));
            ui.label(format!(
                "{} · {}: {} · {}: {}",
                version.name,
                l.translate("Concept storeys"),
                p.storeys,
                l.translate("Floor height (mm)"),
                p.floor_height_mm
            ));
            ui.collapsing(l.translate("Saved rule parameters"), |ui| {
                for (label, value) in parameter_rows(p) {
                    ui.label(format!("{}: {value}", l.translate(label)));
                }
                if let Some(inputs) = &version.parametric_inputs {
                    ui.label(l.translate("Reviewed input version"));
                    ui.label(format!(
                        "{}: {}",
                        l.translate("Source version"),
                        crate::automation_journal::canonical_hash(inputs).unwrap_or_default()
                    ));
                }
            });
        } else if is_parametric {
            ui.label(
                l.translate("Parameter version unknown; geometry and history remain readable"),
            );
        }
    }
}

fn changed_input_summary(
    ui: &mut Ui,
    l: UiLanguage,
    old: &parametric::Inputs,
    new: &parametric::Inputs,
    project: &Project,
) {
    if new.check_current(project).is_err() {
        ui.label(l.translate("Review changed design inputs before updating this candidate"));
        return;
    }
    if old.site_sha256 != new.site_sha256 {
        ui.label(l.translate(
            "Declared site conditions changed; measurement and approval remain unknown",
        ));
    }
    for (old, new) in old.sources.iter().zip(&new.sources) {
        if old.baseline_sha256 == new.baseline_sha256 {
            continue;
        }
        match &new.target {
            crate::evidence::Target::Element { id } => {
                let name = project
                    .elements
                    .iter()
                    .find(|e| e.id == *id)
                    .map(|e| e.name.as_str())
                    .unwrap_or(l.translate("Unknown"));
                ui.label(format!(
                    "{} #{id} · {name}",
                    l.translate("Referenced object changed")
                ));
            }
            crate::evidence::Target::ProgramRow { sheet_id, key } => {
                ui.label(format!(
                    "{} · {key}",
                    l.translate("Referenced requirement changed")
                ));
                if let Some(row) = project
                    .program
                    .sheets
                    .iter()
                    .find(|s| s.id == *sheet_id)
                    .and_then(|s| s.rows.iter().find(|r| r.key == *key))
                {
                    ui.label(&row.values.label);
                    ui.label(&row.values.scope);
                    if !row.values.note.is_empty() {
                        ui.label(&row.values.note);
                    }
                    match &row.values.area {
                        crate::program::Area::Known { m2 } => {
                            ui.label(format!("{}: {m2:.2} m²", l.translate("Required area")));
                        }
                        crate::program::Area::Unknown { reason } => {
                            ui.label(format!("{}: {reason}", l.translate("Unknown")));
                        }
                    }
                }
            }
        }
    }
}
