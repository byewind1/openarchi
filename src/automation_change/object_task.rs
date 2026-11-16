//! Finite object-task scope on the existing coordinator. Never another executor/model.
use super::*;
use crate::{
    command::ModelCommand,
    evidence::{
        EvidenceEdit,
        topic_tasks::{self, Event, Mode, Phase},
    },
};
#[derive(Clone)]
pub(super) struct ObjectTaskScope {
    pub request_id: String,
    pub project_id: String,
    topic_id: String,
    index: usize,
    mode: Mode,
    members: Vec<u64>,
    story: u32,
    pub suspended: bool,
}
impl ChangeService {
    pub fn has_object_task_scope(&self) -> bool {
        self.object_task_scope.is_some()
    }
    pub fn requires_object_task_control(&self, editor: &Editor) -> bool {
        self.object_task_scope
            .as_ref()
            .is_some_and(|g| g.project_id == editor.project_identity())
            || editor
                .project
                .evidence
                .topics
                .iter()
                .flat_map(|t| &t.exchanges)
                .filter_map(|e| e.assignment.as_ref())
                .any(|a| a.state().is_ok_and(|(s, _)| s.active()))
    }
    pub(super) fn object_scope_binding(&self, editor: &Editor) -> bool {
        let Some(g) = &self.object_task_scope else {
            return true;
        };
        g.project_id == editor.project_identity()
            && topic_tasks::find(&editor.project, &g.request_id).is_some_and(|(t, i, e, a)| {
                t.id == g.topic_id
                    && i == g.index
                    && a.mode == g.mode
                    && a.mode.matches_context(&editor.project, e)
                    && editor.story == g.story
            })
    }
    pub(super) fn object_scope_authorization(&self, editor: &Editor) -> Result<(), Value> {
        let Some(g) = &self.object_task_scope else {
            if editor
                .project
                .evidence
                .topics
                .iter()
                .flat_map(|t| &t.exchanges)
                .filter_map(|e| e.assignment.as_ref())
                .any(|a| a.state().is_ok_and(|(s, _)| s.active()))
            {
                return Err(error("OBJECT_TASK_NATIVE_AUTHORITY_REQUIRED"));
            }
            return Ok(());
        };
        if g.suspended || !self.object_scope_binding(editor) {
            return Err(error("OBJECT_TASK_AUTHORIZATION_SUSPENDED"));
        }
        let Some((_, _, _, a)) = topic_tasks::find(&editor.project, &g.request_id) else {
            return Err(error("OBJECT_TASK_MISSING"));
        };
        if !a.state().is_ok_and(|(s, _)| s.active()) {
            return Err(error("OBJECT_TASK_FINISHED"));
        }
        Ok(())
    }
    /// Only trusted native confirmation invokes this. Saved requests/read RPCs never grant.
    pub fn authorize_object_task(&mut self, editor: &Editor, request_id: &str) -> Value {
        if let Err(e) = self.ensure_journal(editor) {
            return e;
        }
        let Some((t, index, e, a)) = topic_tasks::find(&editor.project, request_id) else {
            return error("OBJECT_TASK_MISSING");
        };
        if !a.state().is_ok_and(|(s, _)| s.active()) {
            return error("OBJECT_TASK_FINISHED");
        }
        if !a.mode.matches_context(&editor.project, e) {
            return error("OBJECT_TASK_SCOPE_MISMATCH");
        }
        let members = match a.mode.check_current(&editor.project) {
            Ok(v) => v,
            Err(message) => {
                return json!({"error":{"code":"OBJECT_TASK_SCOPE_CHANGED","message":message}});
            }
        };
        if let Mode::CreateLinkedSpace { story, .. } = &a.mode {
            if editor.story != *story {
                return error("OBJECT_TASK_SCOPE_CHANGED");
            }
        }
        let records = match self.journal.as_ref().unwrap().records() {
            Ok(r) => r,
            Err(message) => {
                return json!({"error":{"code":"JOURNAL_UNAVAILABLE","message":message}});
            }
        };
        let original = records.iter().find(|r| {
            r.identity.request_id == request_id
                && r.native_confirmation
                && r.phase == crate::automation_journal::Phase::Committed
        });
        let Some(original) = original else {
            return error("OBJECT_TASK_NATIVE_AUTHORITY_UNAVAILABLE");
        };
        let Some((_, _, old, old_a)) = topic_tasks::find(&original.after_image, request_id) else {
            return error("OBJECT_TASK_ORIGINAL_MISMATCH");
        };
        let mut original_input = old.clone();
        original_input.reply = None;
        original_input.assignment = None;
        let mut current_input = e.clone();
        current_input.reply = None;
        current_input.assignment = None;
        if old_a.mode != a.mode || original_input != current_input {
            return error("OBJECT_TASK_ORIGINAL_MISMATCH");
        }
        if records.iter().any(|r| {
            r.intent_payload
                .as_ref()
                .is_some_and(|v| v.get("object_task").is_some())
                && matches!(
                    r.phase,
                    crate::automation_journal::Phase::Intent
                        | crate::automation_journal::Phase::UndoIntent
                        | crate::automation_journal::Phase::Unknown
                )
        }) {
            return error("OBJECT_TASK_OPERATION_UNKNOWN");
        }
        if let Mode::CreateTwoWing { task_id, story, .. } = &a.mode {
            if editor.story != *story
                || e.objects.iter().any(|o| {
                    o.link.status(&editor.project) != crate::evidence::LinkStatus::Unchanged
                })
                || e.scope.parameter_input_sha256.is_some()
                    && e.scope.input_status(&editor.project)
                        != Some("Discussion design inputs unchanged")
            {
                return error("OBJECT_TASK_SCOPE_CHANGED");
            }
            let result = editor
                .project
                .design_process
                .tasks
                .iter()
                .find(|t| t.id == *task_id);
            let owned = records.iter().any(|r| {
                r.identity.request_id == *task_id
                    && !r.native_confirmation
                    && r.phase == crate::automation_journal::Phase::Committed
                    && r.intent_payload
                        .as_ref()
                        .is_some_and(|v| v["object_task"]["request_id"] == request_id)
                    && r.after_image
                        .design_process
                        .tasks
                        .iter()
                        .find(|t| t.id == *task_id)
                        == result
            });
            if result.is_some() && !owned {
                return error("OBJECT_TASK_RESULT_OWNERSHIP_MISMATCH");
            }
            if result.is_none() && records.iter().any(|r| r.identity.request_id == *task_id) {
                return error("OBJECT_TASK_CREATION_ALREADY_RECORDED");
            }
        }
        if let Mode::TwoWing { option, .. } = &a.mode {
            let original_state = original
                .after_image
                .design_process
                .parametric_states
                .get(option);
            let current_state = editor.project.design_process.parametric_states.get(option);
            if original_state != current_state {
                let owned = match current_state {
                    Some(crate::design_process::parametric::State::Update { id }) => {
                        records.iter().any(|r| {
                            r.identity.request_id == *id
                                && !r.native_confirmation
                                && r.phase == crate::automation_journal::Phase::Committed
                                && r.intent_payload
                                    .as_ref()
                                    .is_some_and(|v| v["object_task"]["request_id"] == request_id)
                        })
                    }
                    _ => false,
                };
                if !owned {
                    return error("OBJECT_TASK_USER_PARAMETER_CHANGE");
                }
            }
        }
        if let Mode::CreateLinkedSpace {
            zone, operation_id, ..
        } = &a.mode
        {
            let result = editor.project.elements.iter().find(|e| e.id == *zone);
            let link = editor
                .project
                .design_process
                .mass_spaces
                .iter()
                .find(|l| l.zone == *zone);
            let owned = records.iter().any(|r| {
                r.identity.request_id == *operation_id
                    && !r.native_confirmation
                    && r.phase == crate::automation_journal::Phase::Committed
                    && r.intent_payload
                        .as_ref()
                        .is_some_and(|v| v["object_task"]["request_id"] == request_id)
                    && r.after_image.elements.iter().find(|e| e.id == *zone) == result
                    && r.after_image
                        .design_process
                        .mass_spaces
                        .iter()
                        .find(|l| l.zone == *zone)
                        == link
            });
            if result.is_some() && !owned {
                return error("OBJECT_TASK_RESULT_OWNERSHIP_MISMATCH");
            }
            if result.is_none()
                && records
                    .iter()
                    .any(|r| r.identity.request_id == *operation_id)
            {
                return error("OBJECT_TASK_CREATION_ALREADY_RECORDED");
            }
        }
        self.object_task_scope = Some(ObjectTaskScope {
            request_id: request_id.into(),
            project_id: editor.project_identity().into(),
            topic_id: t.id.clone(),
            index,
            mode: a.mode.clone(),
            members,
            story: editor.story,
            suspended: false,
        });
        // Explicit native task consent, narrowed below. It does not start a listener/model.
        self.authorize_current(editor);
        json!({"result":{"status":"authorized","request_id":request_id,"scope":"object_task","assignee_identity":"declared_not_authenticated"}})
    }
    pub(crate) fn release_finished_object_task(&mut self, editor: &Editor, request_id: &str) {
        if self
            .object_task_scope
            .as_ref()
            .is_some_and(|g| g.request_id == request_id)
            && topic_tasks::find(&editor.project, request_id)
                .is_some_and(|(_, _, _, a)| a.state().is_ok_and(|(s, _)| !s.active()))
        {
            self.object_task_scope = None;
            self.set_enabled(false);
        }
    }
    pub fn unrestricted_write_authorization(&self, editor: &Editor) -> Result<(), Value> {
        self.write_authorization(editor)?;
        if self.object_task_scope.is_some() {
            return Err(error("OBJECT_TASK_SCOPE_MISMATCH"));
        }
        Ok(())
    }
    pub fn object_task_authorization(&self, editor: &Editor) -> Value {
        match &self.object_task_scope {
            None => json!({"status":"not_granted","persistent_authority":false}),
            Some(g) if g.project_id != editor.project_identity() => {
                json!({"status":"not_granted","persistent_authority":false})
            }
            Some(g) => {
                json!({"request_id":g.request_id,"topic_id":g.topic_id,"exchange_index":g.index,"mode":g.mode,"members":g.members,"status":match self.write_authorization(editor){Ok(())=>"authorized",Err(e) if e["error"]["code"]=="USER_INTERACTION_ACTIVE"=>"interaction_blocked",Err(_)=>"paused_or_finished"},"persistent_authority":false,"identity":"local_endpoint; assignee label is not authentication"})
            }
        }
    }
    pub(super) fn tag_object_intent(&self, payload: &mut Value, candidate: &PreparedChange) {
        let binding = candidate
            .commands()
            .iter()
            .find_map(|c| match c {
                ModelCommand::Evidence {
                    edit:
                        EvidenceEdit::SubmitTopicTask { id, request_id, .. }
                        | EvidenceEdit::TopicTaskEvent { id, request_id, .. },
                } => Some(json!({"request_id":request_id,"topic_id":id})),
                _ => None,
            })
            .or_else(|| {
                self.object_task_scope
                    .as_ref()
                    .map(|g| json!({"request_id":g.request_id,"topic_id":g.topic_id}))
            });
        if let Some(b) = binding {
            payload["object_task"] = b;
        }
    }
    pub(super) fn check_object_candidate(
        &self,
        editor: &Editor,
        candidate: &PreparedChange,
    ) -> Result<(), Value> {
        let Some(g) = &self.object_task_scope else {
            return Ok(());
        };
        self.object_scope_authorization(editor)?;
        let (_, _, _, assignment) = topic_tasks::find(&editor.project, &g.request_id)
            .ok_or_else(|| error("OBJECT_TASK_MISSING"))?;
        let running = assignment.state().is_ok_and(|(s, _)| s == Phase::Running);
        let cmds = candidate.commands();
        if cmds.len() == 1 {
            if let ModelCommand::Evidence { edit } = &cmds[0] {
                let allowed = match edit {
                    EvidenceEdit::TopicTaskEvent {
                        id,
                        exchange_index,
                        request_id,
                        event,
                        ..
                    } if id == &g.topic_id
                        && *exchange_index == g.index
                        && request_id == &g.request_id =>
                    {
                        if matches!(event, Event::Resume) {
                            false
                        } else {
                            if let Event::Complete { operation_ids } = event {
                                if let Mode::CreateLinkedSpace {
                                    operation_id, zone, ..
                                } = &g.mode
                                {
                                    if !operation_ids.contains(operation_id)
                                        || !editor.project.elements.iter().any(|e| e.id == *zone)
                                    {
                                        return Err(error("OBJECT_TASK_RESULT_MISSING"));
                                    }
                                }
                                let records = self
                                    .journal
                                    .as_ref()
                                    .ok_or_else(|| error("JOURNAL_UNAVAILABLE"))?
                                    .records()
                                    .map_err(|_| error("JOURNAL_UNAVAILABLE"))?;
                                if !operation_ids.iter().all(|id| {
                                    records.iter().any(|r| {
                                        r.identity.request_id == *id
                                            && !r.native_confirmation
                                            && r.phase
                                                == crate::automation_journal::Phase::Committed
                                            && r.intent_payload.as_ref().is_some_and(|v| {
                                                v["object_task"]["request_id"] == g.request_id
                                            })
                                    })
                                }) {
                                    return Err(error("OBJECT_TASK_RESULT_OWNERSHIP_MISMATCH"));
                                }
                            }
                            true
                        }
                    }
                    EvidenceEdit::RecordTopicReply {
                        id, exchange_index, ..
                    } => running && id == &g.topic_id && *exchange_index == g.index,
                    _ => false,
                };
                return if allowed {
                    Ok(())
                } else {
                    Err(error("OBJECT_TASK_SCOPE_MISMATCH"))
                };
            }
        }
        if running {
            if let Mode::CreateLinkedSpace {
                mass,
                source_sha256,
                story,
                option,
                zone,
                ..
            } = &g.mode
            {
                if editor.project.elements.iter().any(|e| e.id == *zone) {
                    return Err(error("OBJECT_TASK_CREATION_ALREADY_RECORDED"));
                }
                let [
                    ModelCommand::MassSpaces {
                        edit:
                            crate::mass_spaces::Edit::Create {
                                mass: source,
                                expected_source_sha256,
                                ..
                            },
                    },
                ] = cmds
                else {
                    return Err(error("OBJECT_TASK_SCOPE_MISMATCH"));
                };
                if source != mass
                    || expected_source_sha256 != source_sha256
                    || editor.project.next_id != *zone
                    || candidate
                        .project()
                        .elements
                        .iter()
                        .find(|e| e.id == *zone)
                        .is_none_or(|e| {
                            e.story != *story
                                || e.design_option != Some(*option)
                                || !matches!(e.shape, crate::model::Shape::Zone { .. })
                        })
                {
                    return Err(error("OBJECT_TASK_SCOPE_MISMATCH"));
                }
                g.mode
                    .check_current(&editor.project)
                    .map_err(|_| error("OBJECT_TASK_SCOPE_CHANGED"))?;
                return Ok(());
            }
            if let Mode::CreateTwoWing {
                task_id,
                story,
                layer,
                site_sha256,
            } = &g.mode
            {
                let (_, _, exchange, _) = topic_tasks::find(&editor.project, &g.request_id)
                    .ok_or_else(|| error("OBJECT_TASK_MISSING"))?;
                if editor
                    .project
                    .design_process
                    .tasks
                    .iter()
                    .any(|t| t.id == *task_id)
                {
                    return Err(error("OBJECT_TASK_CREATION_ALREADY_RECORDED"));
                }
                let reject = || error("OBJECT_TASK_SCOPE_MISMATCH");
                // Admit only the existing engine's finite append, not an arbitrary batch.
                let [
                    ModelCommand::SetOptions { .. },
                    ModelCommand::AddElements {
                        elements,
                        group: false,
                    },
                    ModelCommand::DesignProcess {
                        edit: crate::design_process::Edit::CreateIntent { task },
                    },
                    ModelCommand::SetOptions { .. },
                ] = cmds
                else {
                    return Err(reject());
                };
                let rule = task
                    .intent
                    .as_ref()
                    .and_then(|i| i.parametric.as_ref())
                    .ok_or_else(reject)?;
                if task.id != *task_id
                    || task.story != *story
                    || task.entry != crate::design_process::Entry::AgentProposal
                    || task.question != exchange.question
                    || rule.site_sha256 != *site_sha256
                    || elements.len() != 6
                    || task.source_mass_ids.len() != 2
                    || exchange
                        .objects
                        .iter()
                        .any(|o| !rule.sources.contains(&o.link))
                    || elements
                        .iter()
                        .any(|e| e.story != *story || e.layer.0 != *layer)
                    || !crate::design_process::intent::is_exact_task_append(
                        &editor.project,
                        candidate.project(),
                    )
                    .unwrap_or(false)
                {
                    return Err(reject());
                }
                for (ids, parameters) in [
                    (&task.source_mass_ids, &rule.candidates[0]),
                    (&task.candidates[0].initial_mass_ids, &rule.candidates[0]),
                    (&task.candidates[1].initial_mass_ids, &rule.candidates[1]),
                ] {
                    if ids.len() != 2 {
                        return Err(reject());
                    }
                    for (id, spec) in ids.iter().zip(parameters.geometry().map_err(|_| reject())?) {
                        if elements.iter().find(|e| e.id == *id).is_none_or(|e| {
                            e.shape != crate::model::Shape::ConceptMass { spec: spec.clone() }
                        }) {
                            return Err(reject());
                        }
                    }
                }
                return Ok(());
            }
            if let Mode::TwoWing { task_id, option } = &g.mode {
                let parametric=cmds.iter().any(|c|matches!(c,ModelCommand::DesignProcess{edit:crate::design_process::Edit::RecordUpdate{record}} if record.task_id==*task_id&&record.option==*option&&record.parametric_parameters.is_some()&&record.affected_ids==g.members));
                if parametric
                    && cmds.iter().all(|c| match c {
                        ModelCommand::UpdateElement { element } => g.members.contains(&element.id),
                        ModelCommand::DesignProcess {
                            edit: crate::design_process::Edit::RecordUpdate { record },
                        } => {
                            record.task_id == *task_id
                                && record.option == *option
                                && record.parametric_parameters.is_some()
                                && record.affected_ids == g.members
                        }
                        _ => false,
                    })
                {
                    return Ok(());
                }
                if !cmds.is_empty()&&cmds.iter().all(|c|match c {
                ModelCommand::Constraint{edit:crate::constraints::Edit::Run{id,..}}=>editor.project.rules.checks.iter().any(|c| {
                    if c.definition.id != *id { return false; }
                    if matches!(c.definition.scope,crate::constraints::Scope::Candidate{option:o,story} if o==Some(*option)&&story==g.story) { return true; }
                    let crate::constraints::Scope::Objects { ids } = &c.definition.scope else { return false; };
                    let p = &editor.project;
                    let items: Vec<_> = ids.iter().filter_map(|id| p.elements.iter().find(|e| e.id == *id)).collect();
                    crate::constraints::common_mass_spacing(p, &c.definition, &items)
                        && items.iter().any(|e| e.design_option == Some(*option) && e.story == g.story && g.members.contains(&e.id))
                }),
                ModelCommand::Diagram{edit:crate::diagrams::Edit::Refresh{id,..}}=>editor.project.drawings.diagrams.iter().any(|f|f.id==*id&&f.definition.option==Some(*option)&&f.definition.story==g.story),
                _=>false,
            }){return Ok(());}
            }
        }
        Err(error("OBJECT_TASK_SCOPE_MISMATCH"))
    }
    pub(super) fn check_object_operation(&self, payload: &Value) -> Result<(), Value> {
        if let Some(g) = &self.object_task_scope {
            if payload["object_task"]["request_id"] != g.request_id {
                return Err(error("OBJECT_TASK_OPERATION_OWNERSHIP_MISMATCH"));
            }
        }
        Ok(())
    }
    pub(super) fn check_object_creation_request(
        &self,
        candidate: &PreparedChange,
        request_id: &str,
    ) -> Result<(), Value> {
        if let Some(ObjectTaskScope {
            mode: Mode::CreateLinkedSpace { operation_id, .. },
            ..
        }) = &self.object_task_scope
        {
            if candidate.commands().iter().any(|c| {
                matches!(
                    c,
                    ModelCommand::MassSpaces {
                        edit: crate::mass_spaces::Edit::Create { .. }
                    }
                )
            }) && operation_id != request_id
            {
                return Err(error("OBJECT_TASK_CREATION_REQUEST_ID_MISMATCH"));
            }
        }
        Ok(())
    }
    pub fn object_task_operations(&mut self, editor: &Editor, request_id: &str) -> Value {
        if !crate::storage::valid_document_identity(request_id) {
            return error("INVALID_OBJECT_TASK_ID");
        }
        if let Err(e) = self.ensure_journal(editor) {
            return e;
        }
        match self.journal.as_ref().unwrap().records() {
            Err(message) => json!({"error":{"code":"JOURNAL_UNAVAILABLE","message":message}}),
            Ok(records) => {
                let rows=records.into_iter().filter(|r|r.intent_payload.as_ref().is_some_and(|v|v["object_task"]["request_id"]==request_id)).map(|r|json!({"request_id":r.identity.request_id,"original_target":{"instance_id":r.identity.document_session.split(':').next().unwrap_or(""),"project_id":r.identity.project_id,"document_session_id":r.identity.document_session.split(':').nth(1).and_then(|v|v.parse::<u64>().ok())},"phase":r.phase,"native_confirmation":r.native_confirmation})).collect::<Vec<_>>();
                json!({"result":{"task_request_id":request_id,"operations":rows,"read_only":true,"available_in_this_profile":true,"recovery":"query original target/request; never replay an unknown write"}})
            }
        }
    }
}
