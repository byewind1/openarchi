use super::*;
use crate::evidence::{
    EvidenceEdit, Target,
    topics::{self, Exchange, Topic},
};

pub(super) struct Draft {
    pub base: crate::application_host::CommandContext,
    pub id: String,
    pub exchange: Exchange,
    pub previous: Option<Topic>,
}
pub(super) struct Review {
    pub session: u64,
    pub targets: Vec<Target>,
    pub generation: u64,
}
#[derive(Default)]
pub(super) struct ObjectResearchUi {
    pub focus_question: bool,
    pub draft: Option<Draft>,
    pub error: String,
    pub parameters: Option<super::parametric_ui::Draft>,
    pub canvas_move: Option<super::parametric_ui::CanvasMove>,
    pub allow_task_parameters: bool,
    pub allow_task_creation: bool,
    pub allow_task_space: bool,
    pub task_operations: std::collections::BTreeMap<String, serde_json::Value>,
    pub review: Option<Review>,
    pub review_generation: u64,
}
fn scope_summary(ui: &mut Ui, language: UiLanguage, e: &Exchange, p: &Project) {
    ui.label(language.translate("Discussion record only; no model write authorization"));
    let task = e
        .scope
        .task
        .as_ref()
        .and_then(|id| p.design_process.tasks.iter().find(|t| &t.id == id))
        .map(|t| t.name.as_str())
        .unwrap_or(language.translate("Main model"));
    let option = e
        .scope
        .option
        .and_then(|id| p.design_options.name(id))
        .unwrap_or(language.translate("Main model"));
    ui.label(format!(
        "{}: {task} · {option}",
        language.translate("Frozen task and option")
    ));
    if let Some(status) = e.scope.input_status(p) {
        ui.label(language.translate(status));
    }
}
fn context(
    ui: &mut Ui,
    language: UiLanguage,
    e: &Exchange,
    p: &Project,
    compact: bool,
) -> Option<u64> {
    if !compact {
        scope_summary(ui, language, e, p);
    }
    if let Some(reply) = &e.reply {
        ui.separator();
        ui.strong(language.translate("Agent contribution (declared identity)"));
        ui.label(&reply.agent_label);
        ui.label(&reply.text);
        ui.label(language.translate(
            "Research suggestion; not a user decision, model permission or compliance verdict",
        ));
        ui.collapsing(
            language.translate("Observations, inferences and design completion"),
            |ui| {
                for claim in &reply.claims {
                    let label = match claim.kind {
                        crate::design_process::intent::ClaimKind::Observation => "Observation",
                        crate::design_process::intent::ClaimKind::Inference => "Inference",
                        crate::design_process::intent::ClaimKind::DesignCompletion => {
                            "Design completion"
                        }
                    };
                    ui.label(format!("{}: {}", language.translate(label), claim.text));
                }
            },
        );
    }
    if compact {
        let mut edit = None;
        ui.collapsing(
            language.translate("Bound objects and recorded sources"),
            |ui| {
                scope_summary(ui, language, e, p);
                edit = bound_context(ui, language, e, p);
            },
        );
        edit
    } else {
        bound_context(ui, language, e, p)
    }
}
fn bound_context(ui: &mut Ui, language: UiLanguage, e: &Exchange, p: &Project) -> Option<u64> {
    let mut edit = None;
    for o in &e.objects {
        ui.group(|ui| {
            if let Ok(s) = o.summary() {
                ui.strong(s["name"].as_str().unwrap_or(""));
                ui.label(language.translate(s["kind"].as_str().unwrap_or("")));
                match &o.link.target {
                    Target::Element { id } => {
                        ui.label(format!(
                            "#{id} · {} m² · {} m · {} m³",
                            s["footprint_m2"], s["height_m"], s["envelope_m3"]
                        ));
                        ui.label(
                            language.translate("Authored geometry; not statutory area or capacity"),
                        );
                        if ui
                            .button(language.translate("Edit bound element"))
                            .clicked()
                        {
                            edit = Some(*id);
                        }
                    }
                    Target::ProgramRow { sheet_id, key } => {
                        ui.label(format!("{sheet_id} / {key}"));
                        let v = &s["values"];
                        ui.label(v["note"].as_str().unwrap_or(""));
                        if v["area"]["status"] == "unknown" {
                            ui.label(language.translate("Requirement area unknown"));
                        } else {
                            ui.label(format!("{} m²", v["area"]["m2"]));
                        }
                        ui.label(language.translate("Declared requirement; approval unverified"));
                    }
                }
            }
            ui.label(language.translate(match o.link.status(p) {
                crate::evidence::LinkStatus::Unchanged => "Link unchanged",
                crate::evidence::LinkStatus::Changed => "Link changed",
                crate::evidence::LinkStatus::Missing => "Link missing",
            }));
        });
    }
    ui.collapsing(language.translate("Recorded source quotes"), |ui| {
        for q in &e.sources {
            ui.monospace(format!("{} / {}", q.source_sha256, q.locator));
            ui.label(&q.text);
        }
        ui.label(language.translate(
            "Recorded quotes; original bytes and authority not verified by this local read",
        ));
    });
    edit
}
impl AtelierApp {
    pub(super) fn object_review_active(&self) -> bool {
        self.research_ui
            .review
            .as_ref()
            .is_some_and(|r| r.session == self.host.document.editor.document_session())
    }
    pub(super) fn related_selection_count(&self) -> usize {
        let targets = self
            .host
            .document
            .editor
            .selection_ids()
            .into_iter()
            .map(|id| Target::Element { id })
            .collect::<Vec<_>>();
        if targets.is_empty() || targets.len() > 16 {
            return 0;
        }
        let p = &self.host.document.editor.project;
        p.evidence
            .topics
            .iter()
            .filter(|t| t.related_to(p, &targets))
            .count()
    }
    pub(super) fn can_review_object_discussions(&self) -> bool {
        self.research_ui.draft.is_none()
            && self.research_ui.parameters.is_none()
            && self.research_ui.canvas_move.is_none()
            && self.process_ui.preview.is_none()
            && !self.project_conditions_has_draft()
            && !self.workbench_pointer_active()
            && !self.pending_design_parameters()
    }
    pub(super) fn review_selected_discussions(&mut self) {
        if !self.can_review_object_discussions() {
            return;
        }
        let targets = self
            .host
            .document
            .editor
            .selection_ids()
            .into_iter()
            .map(|id| Target::Element { id })
            .collect::<Vec<_>>();
        if targets.is_empty() || targets.len() > 16 {
            return;
        }
        self.research_ui.review_generation = self.research_ui.review_generation.wrapping_add(1);
        self.research_ui.review = Some(Review {
            session: self.host.document.editor.document_session(),
            targets,
            generation: self.research_ui.review_generation,
        });
        self.research_ui.error.clear();
        self.research_ui.focus_question = true;
        self.show_right = true;
        self.tab = Tab::Assistant;
    }
    pub(super) fn begin_object_research(&mut self, targets: Vec<Target>, previous: Option<Topic>) {
        let result = (|| {
            if self.research_ui.draft.is_some() {
                return Err("Finish or cancel the current object discussion".to_string());
            }
            if self.project_conditions_has_draft()
                || self.workbench_pointer_active()
                || self.pending_design_parameters()
            {
                return Err(
                    "Finish the active interaction before starting object research".to_string(),
                );
            }
            let mut exchange = topics::capture(
                &self.host.document.editor.project,
                &targets,
                "Object discussion".into(),
            )?;
            exchange.question.clear();
            let id = previous
                .as_ref()
                .map_or_else(storage::new_document_identity, |t| t.id.clone());
            self.research_ui.allow_task_parameters = false;
            self.research_ui.allow_task_creation = false;
            self.research_ui.allow_task_space = false;
            self.research_ui.review = None;
            self.research_ui.focus_question = true;
            self.research_ui.draft = Some(Draft {
                base: self.host.context(),
                id,
                exchange,
                previous,
            });
            self.project_conditions_ui.open = false;
            self.program_ui.open = false;
            self.show_right = true;
            self.tab = Tab::Assistant;
            Ok(())
        })();
        self.research_ui.error = result.err().unwrap_or_default();
    }
    pub(super) fn discuss_selection(&mut self) {
        let targets = self
            .host
            .document
            .editor
            .selection_ids()
            .into_iter()
            .map(|id| Target::Element { id })
            .collect();
        self.begin_object_research(targets, None);
    }
    pub(super) fn preview_object_research(&mut self) {
        let result = (|| {
            let d = self
                .research_ui
                .draft
                .as_ref()
                .ok_or("No object discussion draft")?;
            if d.base != self.host.context() {
                return Err(
                    "Discussion inputs changed; explicitly capture a new contribution".into(),
                );
            }
            let command = crate::command::ModelCommand::Evidence {
                edit: EvidenceEdit::RecordTopic {
                    id: d.id.clone(),
                    exchange: d.exchange.clone(),
                    expected_previous: d.previous.clone(),
                },
            };
            self.host
                .document
                .editor
                .prepare_change(d.base.revision, vec![command])
                .map_err(|e| e.message)
        })();
        self.process_set_preview(
            storage::new_document_identity(),
            "Save object discussion",
            result,
        );
    }
    pub(super) fn object_research_contents(&mut self, ui: &mut Ui) {
        let l = self.language;
        let reviewing = self.object_review_active()
            && self.research_ui.draft.is_none()
            && self.research_ui.parameters.is_none()
            && self.research_ui.canvas_move.is_none();
        if !self.research_ui.error.is_empty() {
            ui.colored_label(Color32::LIGHT_RED, l.translate(&self.research_ui.error));
        }
        let writing_question = self.research_ui.draft.is_some()
            && self.research_ui.parameters.is_none()
            && self.research_ui.canvas_move.is_none();
        if reviewing {
            let r = self.research_ui.review.as_ref().unwrap();
            ui.horizontal_wrapped(|ui| {
                ui.heading(l.translate(if r.targets.is_empty() {
                    "All saved discussions"
                } else {
                    "Related discussions"
                }))
                .on_hover_text(
                    l.translate("Viewing saved discussions only; no task is started or resumed"),
                );
                for target in &r.targets {
                    match target {
                        Target::Element { id } => {
                            ui.label(format!("#{id}"));
                        }
                        Target::ProgramRow { sheet_id, key } => {
                            ui.label(format!("{sheet_id} / {key}"));
                        }
                    }
                }
                ui.small(l.translate("Read only"));
            });
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button(l.translate("All discussions"))
                    .on_hover_text(l.translate("Show all saved discussions"))
                    .clicked()
                {
                    self.research_ui.review.as_mut().unwrap().targets.clear();
                    self.research_ui.focus_question = true;
                }
                if ui
                    .button(l.translate("Other assistant tools"))
                    .on_hover_text(l.translate("Return to other assistant tools"))
                    .clicked()
                {
                    self.research_ui.review = None;
                }
            });
        } else if !writing_question {
            ui.heading(l.translate("Object research"));
            self.parametric_contents(ui);
            ui.label(l.translate(
                "Selecting objects sends nothing. Start explicitly; captured targets stay fixed.",
            ));
            if ui.button(l.translate("Discuss selected objects")).clicked() {
                self.discuss_selection();
            }
        }
        let mut generate = false;
        let mut adjust = false;
        let bound_parameters = self.bound_parametric_discussion();
        let mut preview = false;
        let mut cancel = false;
        let mut add = None;
        let mut edit = None;
        let mut resume = None;
        let mut submit_task = false;
        let mut task_event = None;
        let mut task_operations = None;
        let mut release_permission = None;
        if let Some(d) = &mut self.research_ui.draft {
            ui.separator();
            ui.label(l.translate("Your design question"));
            let response = ui.add(
                TextEdit::multiline(&mut d.exchange.question)
                    .desired_rows(3)
                    .id_salt("object_research_question"),
            );
            #[cfg(test)]
            ui.ctx().data_mut(|data| {
                data.insert_temp(Id::new("object_research_question"), response.id)
            });
            let _ = response;
            scope_summary(ui, l, &d.exchange, &self.host.document.editor.project);
            ui.horizontal_wrapped(|ui| {
                for object in &d.exchange.objects {
                    let name = object
                        .summary()
                        .ok()
                        .and_then(|v| v["name"].as_str().map(str::to_owned))
                        .unwrap_or_default();
                    match &object.link.target {
                        Target::Element { id } => {
                            ui.label(format!("#{id} · {name}"));
                        }
                        Target::ProgramRow { sheet_id, key } => {
                            ui.label(format!("{sheet_id}/{key} · {name}"));
                        }
                    }
                }
            });
            if bound_parameters
                && ui
                    .button(l.translate("Adjust bound wings from this question"))
                    .clicked()
            {
                adjust = true;
            }
            ui.collapsing(l.translate("Add an existing requirement row"), |ui| {
                for s in &self.host.document.editor.project.program.sheets {
                    for r in &s.rows {
                        let target = Target::ProgramRow {
                            sheet_id: s.id.clone(),
                            key: r.key.clone(),
                        };
                        if !d.exchange.objects.iter().any(|o| o.link.target == target)
                            && ui
                                .button(format!("{} · {}", s.name, r.values.label))
                                .clicked()
                        {
                            add = Some(target);
                        }
                    }
                }
            });
            if ui
                .button(l.translate("Preview discussion record"))
                .clicked()
            {
                preview = true;
            }
            if ui.button(l.translate("Cancel discussion draft")).clicked() {
                cancel = true;
            }
            ui.label(l.translate("Local object comparison; no AI response has been generated"));
            if self
                .host
                .document
                .editor
                .project
                .design_process
                .task()
                .is_some_and(|t| t.intent.as_ref().is_some_and(|i| i.parametric.is_some()))
            {
                if ui
                    .checkbox(
                        &mut self.research_ui.allow_task_parameters,
                        l.translate("Allow parameter updates to the current two wings"),
                    )
                    .changed()
                    && self.research_ui.allow_task_parameters
                {
                    self.research_ui.allow_task_creation = false;
                    self.research_ui.allow_task_space = false;
                }
                ui.label(l.translate("Scope: position, court, depth, concept storeys and height; other work is protected"));
            }
            if ui
                .checkbox(
                    &mut self.research_ui.allow_task_creation,
                    l.translate("Allow one new two-wing study"),
                )
                .changed()
                && self.research_ui.allow_task_creation
            {
                self.research_ui.allow_task_parameters = false;
                self.research_ui.allow_task_space = false;
            }
            if self.research_ui.allow_task_creation {
                ui.label(l.translate("Scope: create two editable candidates on the current story; preserve all existing work"));
            }
            let space_available = crate::evidence::topic_tasks::Mode::linked_space(
                &self.host.document.editor.project,
                &d.exchange,
            )
            .is_ok();
            if ui
                .add_enabled(
                    space_available,
                    egui::Checkbox::new(
                        &mut self.research_ui.allow_task_space,
                        l.translate("Allow one linked concept space"),
                    ),
                )
                .on_disabled_hover_text(
                    l.translate("Choose one unlinked source mass in the current candidate"),
                )
                .changed()
                && self.research_ui.allow_task_space
            {
                self.research_ui.allow_task_parameters = false;
                self.research_ui.allow_task_creation = false;
            }
            if self.research_ui.allow_task_space {
                ui.label(l.translate("Scope: one new concept Zone from this mass; preserve source and existing spaces"));
            }
            if ui
                .button(l.translate("Preview task for connected Agent"))
                .clicked()
            {
                submit_task = true;
            }
            ui.label(l.translate(
                "Explicit handoff only; waits for an external client and does not start a model",
            ));
            if ui
                .button(l.translate(if bound_parameters {
                    "Start a new two-wing exploration"
                } else {
                    "Explore two wings from this question"
                }))
                .clicked()
            {
                generate = true;
            }
            ui.collapsing(l.translate("Captured objects and source details"), |ui| {
                edit = context(
                    ui,
                    l,
                    &d.exchange,
                    &self.host.document.editor.project,
                    false,
                );
            });
        }
        let result_navigation_ready = self.research_ui.draft.is_none()
            && self.research_ui.parameters.is_none()
            && self.research_ui.canvas_move.is_none()
            && self.process_ui.preview.is_none();
        let mut rendered_topics = 0;
        for (topic_index, t) in self
            .host
            .document
            .editor
            .project
            .evidence
            .topics
            .iter()
            .rev()
            .filter(|t| {
                !reviewing
                    || self.research_ui.review.as_ref().is_none_or(|r| {
                        r.targets.is_empty()
                            || t.related_to(&self.host.document.editor.project, &r.targets)
                    })
            })
            .enumerate()
        {
            rendered_topics += 1;
            // CollapsingHeader uses Extend even in a vertical scroll area.
            // Keep the complete question and accessible title, but lay it out
            // within this inspector before the header measures its minimum width.
            let question = t
                .exchanges
                .last()
                .map(|e| e.question.as_str())
                .unwrap_or("");
            let title = egui::WidgetText::from(question).into_galley(
                ui,
                Some(egui::TextWrapMode::Wrap),
                (ui.available_width() - ui.spacing().indent - ui.spacing().button_padding.x)
                    .max(1.),
                egui::TextStyle::Button,
            );
            egui::CollapsingHeader::new(title)
            .id_salt(("object_topic", &t.id, self.research_ui.review.as_ref().filter(|_|reviewing).map(|r|r.generation)))
            .default_open(reviewing && topic_index==0)
            .show(ui, |ui| {
                if ui.button(l.translate("New contribution with these objects")).clicked() {
                    resume = Some(t.clone());
                }
                for (index,e) in t.exchanges.iter().enumerate() {
                    if t.exchanges.len() > 1 {
                        ui.strong(&e.question);
                    }
                    if let Some(crate::evidence::topic_tasks::Assignment { mode: crate::evidence::topic_tasks::Mode::CreateTwoWing { task_id, .. }, .. }) = &e.assignment {
                        if let Some(task) = self.host.document.editor.project.design_process.tasks.iter().find(|t| &t.id == task_id) {
                            ui.strong(l.translate("Generated candidates"));
                            for candidate in &task.candidates {
                                let member = candidate.initial_mass_ids.iter().find(|id| self.host.document.editor.project.elements.iter().any(|element| element.id == **id && element.design_option == Some(candidate.option)));
                                let response = ui.add_enabled(member.is_some() && result_navigation_ready, Button::new(format!("{} · {}", l.translate("Edit generated candidate"), candidate.name)));
                                if response.clicked() { edit = member.copied(); }
                            }
                        } else {
                            ui.label(l.translate("Generated study is not available; original discussion retained"));
                        }
                    }
                    if let Some(crate::evidence::topic_tasks::Assignment { mode: crate::evidence::topic_tasks::Mode::CreateLinkedSpace { zone, .. }, .. }) = &e.assignment {
                        if let Some(result) = self.host.document.editor.project.elements.iter().find(|e|e.id == *zone && matches!(e.shape, Shape::Zone { .. })) {
                            if ui.add_enabled(result_navigation_ready, Button::new(format!("{} · #{} · {}",l.translate("Edit generated concept space"),zone,result.name))).clicked() {
                                edit=Some(*zone);
                            }
                        }
                    }
                    if let Some(status)=e.result_status(&self.host.document.editor.project) { ui.label(l.translate(status)); }
                    let mut task_details=|ui:&mut Ui| {
                    if let Some(a)=&e.assignment {
                        let (phase,agent)=a.state().unwrap_or((crate::evidence::topic_tasks::Phase::Cancelled,None));
                        let label=match phase {crate::evidence::topic_tasks::Phase::Queued=>"Waiting for a connected Agent to claim this task",crate::evidence::topic_tasks::Phase::Running=>"Agent reports this task is running",crate::evidence::topic_tasks::Phase::Completed=>"Agent reported completion; verify the result",crate::evidence::topic_tasks::Phase::Cancelled=>"Task cancelled; committed work is retained"};
                        ui.strong(l.translate(label));ui.monospace(&a.id);
                        if let Some(agent)=agent {ui.label(agent);}
                        Self::object_task_mode_contents(ui,l,&a.mode,&self.host.document.editor.project);
                        if let Some(crate::evidence::topic_tasks::Event::Progress{message})=a.events.last(){ui.label(message);}
                        if phase.active(){
                            let authorization=self.host.operations.borrow().object_task_authorization(&self.host.document.editor);
                            if authorization["status"]=="interaction_blocked" {ui.label(l.translate("Task writes wait while you are interacting with the canvas or a preview"));} else if authorization["request_id"]!=a.id || authorization["status"]!="authorized" {ui.label(l.translate("Task permission is not active; fresh reads do not resume it"));}
                            if ui.button(l.translate("Review and resume original task")).clicked(){task_event=Some((t.clone(),index,crate::evidence::topic_tasks::Event::Resume));}
                            if ui.button(l.translate("Stop further task writes")).clicked(){task_event=Some((t.clone(),index,crate::evidence::topic_tasks::Event::Cancel));}
                        }
                        if !phase.active() && self.host.operations.borrow().object_task_authorization(&self.host.document.editor)["request_id"]==a.id {
                            if ui.button(l.translate("End task permission")).clicked(){release_permission=Some(a.id.clone());}
                        }
                        if ui.button(l.translate("Read original task operations")).clicked(){task_operations=Some(a.id.clone());}
                        if let Some(result)=self.research_ui.task_operations.get(&a.id){ui.label(l.translate("Recorded operation phases; inspect original identities for current effects"));ui.monospace(serde_json::to_string_pretty(result).unwrap_or_default());}
                    }
                    };
                    let fold_task=reviewing && e.assignment.as_ref().is_some_and(|a|a.state().is_ok_and(|(phase,_)|!phase.active()));
                    if fold_task { ui.collapsing(l.translate("Task details"),|ui|task_details(ui)); }
                    else { task_details(ui); }
                    edit = context(ui, l, e, &self.host.document.editor.project, reviewing).or(edit);
                }
            });
        }
        if reviewing && rendered_topics == 0 {
            ui.label(l.translate("No saved discussions for these objects"));
        }
        if let Some(target) = add {
            let d = self.research_ui.draft.as_mut().unwrap();
            if d.base == self.host.context() {
                let mut targets = d
                    .exchange
                    .objects
                    .iter()
                    .map(|o| o.link.target.clone())
                    .collect::<Vec<_>>();
                targets.push(target);
                match topics::capture(
                    &self.host.document.editor.project,
                    &targets,
                    if d.exchange.question.is_empty() {
                        "Object discussion".into()
                    } else {
                        d.exchange.question.clone()
                    },
                ) {
                    Ok(mut e) => {
                        e.question = d.exchange.question.clone();
                        d.exchange = e;
                    }
                    Err(e) => self.research_ui.error = e,
                }
            } else {
                self.research_ui.error =
                    "Discussion inputs changed; explicitly capture a new contribution".into();
            }
        }
        if cancel {
            self.research_ui.draft = None;
            self.research_ui.error.clear();
        }
        if submit_task {
            self.preview_submit_object_task();
        }
        if let Some((topic, index, event)) = task_event {
            self.preview_object_task_event(&topic, index, event);
        }
        if let Some(id) = release_permission {
            self.host
                .operations
                .borrow_mut()
                .release_finished_object_task(&self.host.document.editor, &id);
            self.note_object_task_consent(false);
        }
        if let Some(id) = task_operations {
            let result = self
                .host
                .operations
                .borrow_mut()
                .object_task_operations(&self.host.document.editor, &id);
            self.research_ui.task_operations.insert(id, result);
        }
        if generate {
            self.begin_parametric_from_discussion();
        }
        if adjust {
            self.adjust_parametric_from_discussion();
        }
        if preview {
            self.preview_object_research();
        }
        if let Some(t) = resume {
            let targets = t
                .exchanges
                .last()
                .unwrap()
                .objects
                .iter()
                .map(|o| o.link.target.clone())
                .collect();
            self.begin_object_research(targets, Some(t));
        }
        if let Some(id) = edit {
            if let Some(e) = self
                .host
                .document
                .editor
                .project
                .elements
                .iter()
                .find(|e| e.id == id)
                .cloned()
            {
                match self.activate_process_option(e.design_option) {
                    Ok(()) if self.process_ui.preview.is_none() => {
                        self.host.document.editor.story = e.story;
                        self.host.document.editor.set_selection(vec![id]);
                        self.sync_process_space();
                        self.tab = Tab::Properties;
                        self.view = View::Plan;
                        self.fit_selection();
                        self.view = View::Plan;
                    }
                    Ok(()) => {}
                    Err(e) => self.research_ui.error = e,
                }
            } else {
                self.research_ui.error = "Discussion object is missing".into();
            }
        }
    }
    pub(super) fn object_research_preview(&self, ui: &mut Ui, p: &Project) {
        for t in &p.evidence.topics {
            if !self
                .host
                .document
                .editor
                .project
                .evidence
                .topics
                .contains(t)
            {
                if let Some(e) = t.exchanges.last() {
                    ui.strong(&e.question);
                    context(
                        ui,
                        self.language,
                        e,
                        &self.host.document.editor.project,
                        false,
                    );
                }
            }
        }
    }
}
