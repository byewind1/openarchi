//! Inline task consent/lifecycle in the existing object discussion and preview.
use super::*;
use crate::evidence::{
    EvidenceEdit,
    topic_tasks::{Event, Mode},
    topics::Topic,
};
impl AtelierApp {
    pub(super) fn object_task_mode_contents(ui: &mut Ui, l: UiLanguage, mode: &Mode, p: &Project) {
        match mode {
            Mode::Research => {
                ui.label(l.translate("Permission: research answer only, no geometry changes"));
            }
            Mode::TwoWing { task_id, option } => {
                ui.label(l.translate(
                    "Permission: mapped two-wing parameters and existing checks/figures only",
                ));
                if let Some(t) = p.design_process.tasks.iter().find(|t| t.id == *task_id) {
                    if let Some(c) = t.candidates.iter().find(|c| c.option == *option) {
                        ui.label(format!("{} · {:?}", c.name, c.initial_mass_ids));
                    }
                }
            }
            Mode::CreateTwoWing { story, .. } => {
                ui.label(
                    l.translate("Permission: one new two-wing study; no changes to existing work"),
                );
                ui.label(l.translate("After creation: readback and task answer only; further model edits need a separate explicit action"));
                if let Some(s) = p.stories.iter().find(|s| s.id == *story) {
                    ui.label(&s.name);
                }
            }
            Mode::CreateLinkedSpace {
                mass,
                zone,
                operation_id,
                ..
            } => {
                ui.label(
                    l.translate("Permission: one linked concept Zone; no changes to existing work"),
                );
                ui.label(format!("#{mass} → Zone #{zone}"));
                ui.label(l.translate("After creation: readback and task answer only; further model edits need a separate explicit action"));
                ui.label(l.translate(
                    "Use the original creation operation identity; never replay an unknown write",
                ));
                ui.monospace(operation_id);
            }
        }
    }
    pub(super) fn preview_submit_object_task(&mut self) {
        let request_id = storage::new_document_identity();
        let result = (|| {
            let d = self
                .research_ui
                .draft
                .as_ref()
                .ok_or("Start with an object question")?;
            if d.base != self.host.context() {
                return Err(
                    "Discussion inputs changed; explicitly capture a new contribution".into(),
                );
            }
            let p = &self.host.document.editor.project;
            let mode = if self.research_ui.allow_task_space {
                let mode = Mode::linked_space(p, &d.exchange)?;
                if let Mode::CreateLinkedSpace { story, .. } = &mode {
                    if *story != self.host.document.editor.story {
                        return Err("Activate the source story".into());
                    }
                }
                mode
            } else if self.research_ui.allow_task_creation {
                Mode::CreateTwoWing {
                    task_id: storage::new_document_identity(),
                    story: self.host.document.editor.story,
                    layer: 0,
                    site_sha256: crate::automation_journal::canonical_hash(&p.site)?,
                }
            } else if self.research_ui.allow_task_parameters {
                Mode::TwoWing {
                    task_id: p
                        .design_process
                        .current_task
                        .clone()
                        .ok_or("No parametric task")?,
                    option: p
                        .plan_options
                        .active_design_option
                        .ok_or("Activate a parametric candidate")?,
                }
            } else {
                Mode::Research
            };
            self.host
                .document
                .editor
                .prepare_change(
                    d.base.revision,
                    vec![crate::command::ModelCommand::Evidence {
                        edit: EvidenceEdit::SubmitTopicTask {
                            id: d.id.clone(),
                            exchange: d.exchange.clone(),
                            expected_previous: d.previous.clone(),
                            request_id: request_id.clone(),
                            mode,
                        },
                    }],
                )
                .map_err(|e| e.message)
        })();
        self.process_set_preview(request_id, "Submit object task", result);
    }
    pub(super) fn preview_object_task_event(&mut self, t: &Topic, index: usize, event: Event) {
        let result = (|| {
            let a = t
                .exchanges
                .get(index)
                .and_then(|e| e.assignment.as_ref())
                .ok_or("Object task is missing")?;
            self.host
                .document
                .editor
                .prepare_change(
                    self.host.document.editor.revision,
                    vec![crate::command::ModelCommand::Evidence {
                        edit: EvidenceEdit::TopicTaskEvent {
                            id: t.id.clone(),
                            exchange_index: index,
                            expected_topic_sha256: crate::automation_journal::canonical_hash(t)?,
                            request_id: a.id.clone(),
                            event,
                        },
                    }],
                )
                .map_err(|e| e.message)
        })();
        self.process_set_preview(
            storage::new_document_identity(),
            "Control original object task",
            result,
        );
    }
    pub(super) fn object_task_preview_contents(
        ui: &mut Ui,
        l: UiLanguage,
        before: &Project,
        change: &crate::model_service::PreparedChange,
    ) {
        for command in change.commands() {
            match command {
                crate::command::ModelCommand::Evidence {
                    edit: EvidenceEdit::SubmitTopicTask { mode, exchange, .. },
                } => {
                    ui.strong(l.translate("Your design question"));
                    ui.label(&exchange.question);
                    ui.strong(l.translate("Explicit task consent"));
                    Self::object_task_mode_contents(ui, l, mode, before);
                    ui.label(l.translate(
                        "Only this task is authorized; selection changes cannot retarget it",
                    ));
                    ui.label(l.translate("Explicit handoff only; waits for an external client and does not start a model"));
                }
                crate::command::ModelCommand::Evidence {
                    edit:
                        EvidenceEdit::TopicTaskEvent {
                            id,
                            exchange_index,
                            event,
                            ..
                        },
                } => {
                    if let Some(exchange) = before
                        .evidence
                        .topics
                        .iter()
                        .find(|topic| &topic.id == id)
                        .and_then(|topic| topic.exchanges.get(*exchange_index))
                    {
                        ui.strong(l.translate("Your design question"));
                        ui.label(&exchange.question);
                    }
                    ui.label(l.translate(match event{Event::Cancel=>"Stops future writes; keeps committed work and original unknown operations",_=>"Review the original scope and operation identities; resume never replays a write"}));
                }
                _ => {}
            }
        }
    }
    pub(super) fn finish_native_object_task_control(
        &mut self,
        change: &crate::model_service::PreparedChange,
    ) {
        for command in change.commands() {
            match command {
                crate::command::ModelCommand::Evidence {
                    edit:
                        EvidenceEdit::SubmitTopicTask { request_id, .. }
                        | EvidenceEdit::TopicTaskEvent {
                            request_id,
                            event: Event::Resume,
                            ..
                        },
                } => {
                    let result = self
                        .host
                        .authorize_object_task(self.host.context(), request_id);
                    if result.get("error").is_some() {
                        self.research_ui.error=match result["error"]["code"].as_str().unwrap_or("") {
                            "OBJECT_TASK_OPERATION_UNKNOWN"=>"An original task operation is unresolved; inspect it before resuming",
                            "OBJECT_TASK_NATIVE_AUTHORITY_UNAVAILABLE"=>"Original task authority is unavailable in this profile; original evidence is required",
                            "OBJECT_TASK_USER_PARAMETER_CHANGE"=>"User parameter changes are protected; start a new scoped task instead of resuming this one",
                            "OBJECT_TASK_ORIGINAL_MISMATCH"=>"Saved task differs from its original consent; it cannot be resumed",
                            "OBJECT_TASK_CREATION_ALREADY_RECORDED"=>"This study already has an original operation; inspect it instead of generating again",
                            "OBJECT_TASK_SCOPE_CHANGED"=>"Task inputs or protected members changed; preserve user edits and review the scope",
                            _=>"Object task permission was not granted; inspect the original task",
                        }.into();
                    } else {
                        self.note_object_task_consent(true);
                    }
                    self.research_ui.draft = None;
                }
                crate::command::ModelCommand::Evidence {
                    edit:
                        EvidenceEdit::TopicTaskEvent {
                            request_id,
                            event: Event::Cancel,
                            ..
                        },
                } => {
                    self.host
                        .operations
                        .borrow_mut()
                        .release_finished_object_task(&self.host.document.editor, request_id);
                    self.note_object_task_consent(false);
                }
                _ => {}
            }
        }
    }
}
