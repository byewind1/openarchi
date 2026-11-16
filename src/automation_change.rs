//! Transport-owned immutable previews over the existing model service.
mod authorization;
mod object_task;
mod task_checkpoint;
use crate::{
    automation::{ReadService, Target},
    editor::Editor,
    model::Project,
    model_service::{BatchChange, PreparedChange},
};
use authorization::WriteGrant;
use object_task::ObjectTaskScope;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prepare {
    pub target: Target,
    pub expected_revision: u64,
    pub change: BatchChange,
}
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct PreviewResult {
    pub preview_id: String,
    pub payload_hash: String,
    pub base_revision: u64,
    pub context_revision: Option<String>,
    pub changed_ids: Vec<u64>,
    pub changes: Vec<PreviewElementChange>,
    pub story_changes: Vec<crate::model_service::StoryElevationChange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_3d_style_change: Option<Model3dStyleChange>,
    pub site_context_change: Option<SiteContextChange>,
    pub program_change: Option<ProgramChange>,
    pub evidence_change: Option<EvidenceChange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules_change: Option<RulesChange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_template_change: Option<ReportTemplateChange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mass_space_change: Option<MassSpaceChange>,
    pub dependencies: Vec<PreviewHostDependency>,
    pub resolved_references: std::collections::BTreeMap<String, u64>,
    pub expires_in_seconds: u64,
}
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct Model3dStyleChange {
    pub before: crate::view_settings::Model3dStyle,
    pub after: crate::view_settings::Model3dStyle,
}
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct MassSpaceChange {
    pub before: Vec<crate::mass_spaces::Link>,
    pub after: Vec<crate::mass_spaces::Link>,
}
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct ReportTemplateChange {
    pub before: Vec<crate::diagrams::report::Template>,
    pub after: Vec<crate::diagrams::report::Template>,
}
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct EvidenceChange {
    pub before: crate::evidence::EvidenceRegister,
    pub after: crate::evidence::EvidenceRegister,
}
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct RulesChange {
    pub before: crate::rules::RuleBook,
    pub after: crate::rules::RuleBook,
}
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct ProgramChange {
    pub before: crate::program::Program,
    pub after: crate::program::Program,
}
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct SiteContextChange {
    pub before: crate::site::SiteContext,
    pub after: crate::site::SiteContext,
}
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PreviewAction {
    Create,
    Update,
    Remove,
}
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct PreviewElementChange {
    pub id: u64,
    pub action: PreviewAction,
    pub native_type: String,
}
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct PreviewHostDependency {
    pub element_id: u64,
    pub host_wall: u64,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Commit {
    pub target: Target,
    pub preview_id: String,
    pub request_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub target: Target,
    pub request_id: String,
    #[serde(default)]
    pub original_target: Option<Target>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cancel {
    pub target: Target,
    pub preview_id: String,
}
#[derive(Clone)]
struct Preview {
    context_revision: Option<String>,
    candidate: std::rc::Rc<PreparedChange>,
    created: Instant,
    hash: String,
    batch: Option<BatchChange>,
    native_confirmation: bool,
    intent_payload: Value,
}
impl Preview {
    fn execution_hash(&self) -> String {
        self.batch
            .as_ref()
            .map(ChangeService::execution_hash)
            .unwrap_or_else(|| self.hash.clone())
    }
}
struct Applied {
    preview_id: String,
    after: Project,
    history_len: usize,
    delta: crate::command::ProjectDelta,
    undone: bool,
    receipt: crate::model_service::ChangeReceipt,
    identity: crate::automation_journal::OperationIdentity,
    payload_hash: String,
    execution_hash: String,
    native_confirmation: bool,
}
#[cfg(debug_assertions)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurableStage {
    IntentStored,
    ModelCommitted,
    CommitStored,
    UndoIntentStored,
    ModelUndone,
    UndoStored,
}
pub struct ChangeService {
    read: ReadService,
    enabled: bool,
    remote_interaction_blocked: bool,
    remote_paused: bool,
    write_grant: Option<WriteGrant>,
    object_task_scope: Option<ObjectTaskScope>,
    previews: HashMap<String, Preview>,
    applied: HashMap<String, Applied>,
    sequence: u64,
    journal_directory: Option<std::path::PathBuf>,
    journal: Option<crate::automation_journal::Journal>,
    journal_project: Option<String>,
    #[cfg(debug_assertions)]
    probe: Option<Box<dyn FnMut(DurableStage)>>,
}
#[derive(Clone)]
pub(crate) struct OperationRow {
    pub request_id: String,
    pub original_target: Target,
    pub phase: String,
    pub native_confirmation: bool,
    pub changed_ids: Vec<u64>,
}
pub(crate) struct OperationRows {
    pub operations: Vec<OperationRow>,
    pub previews: Vec<String>,
}
fn error(code: &str) -> Value {
    json!({"error":{"code":code}})
}
impl ChangeService {
    pub(crate) fn current_target(&self, editor: &Editor) -> Target {
        self.read.target(editor)
    }
    pub(crate) fn cancel_current(&mut self, editor: &Editor, preview_id: String) -> Value {
        self.cancel(
            editor,
            Cancel {
                target: self.current_target(editor),
                preview_id,
            },
        )
    }
    pub(crate) fn inspect_current(
        &mut self,
        editor: &Editor,
        request_id: String,
        original_target: Target,
    ) -> Value {
        self.operation(
            editor,
            Operation {
                target: self.current_target(editor),
                request_id,
                original_target: Some(original_target),
            },
        )
    }
    pub(crate) fn undo_current(
        &mut self,
        editor: &mut Editor,
        request_id: String,
        original_target: Target,
    ) -> Value {
        self.undo_admitted(
            editor,
            Operation {
                target: self.current_target(editor),
                request_id,
                original_target: Some(original_target),
            },
            true,
        )
    }
    pub(crate) fn reconcile_current(
        &mut self,
        editor: &mut Editor,
        request_id: String,
        original_target: Target,
    ) -> Value {
        self.recover_admitted(
            editor,
            Recover {
                target: self.current_target(editor),
                expected_revision: editor.revision,
                request_id,
                original_target,
            },
            true,
        )
    }
    pub(crate) fn operation_rows(&mut self, editor: &Editor) -> Result<OperationRows, Value> {
        self.ensure_journal(editor)?;
        self.previews
            .retain(|_, p| p.created.elapsed() < Duration::from_secs(600));
        let current = self.read.target(editor);
        let mut operations = Vec::new();
        if let Some(journal) = &self.journal {
            for record in journal.records().map_err(|e| json!({"error":{"code":e}}))? {
                if record.identity.project_id != editor.project_identity() {
                    continue;
                }
                let Some((instance, session)) = record.identity.document_session.rsplit_once(':')
                else {
                    continue;
                };
                let Ok(session) = session.parse() else {
                    continue;
                };
                operations.push(OperationRow {
                    request_id: record.identity.request_id.clone(),
                    original_target: Target {
                        instance_id: instance.into(),
                        project_id: current.project_id.clone(),
                        document_session_id: session,
                    },
                    phase: serde_json::to_value(record.phase)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .into(),
                    native_confirmation: record.native_confirmation,
                    changed_ids: record
                        .receipt
                        .as_ref()
                        .or(record.planned_receipt.as_ref())
                        .and_then(|v| v.get("changed_ids"))
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .unwrap_or_default(),
                });
            }
        }
        operations.sort_by(|a, b| {
            (
                &a.original_target.instance_id,
                a.original_target.document_session_id,
                &a.request_id,
            )
                .cmp(&(
                    &b.original_target.instance_id,
                    b.original_target.document_session_id,
                    &b.request_id,
                ))
        });
        let mut previews: Vec<_> = self
            .previews
            .iter()
            .filter(|(_, p)| !p.native_confirmation && p.candidate.belongs_to_session(editor))
            .filter(|(id, _)| !self.applied.values().any(|a| &a.preview_id == *id))
            .map(|(id, _)| id.clone())
            .collect();
        previews.sort();
        Ok(OperationRows {
            operations,
            previews,
        })
    }
    pub fn new(instance: String, enabled: bool) -> Self {
        Self {
            read: ReadService::new(instance),
            enabled,
            remote_interaction_blocked: false,
            remote_paused: false,
            write_grant: None,
            object_task_scope: None,
            previews: HashMap::new(),
            applied: HashMap::new(),
            sequence: 0,
            journal_directory: None,
            journal: None,
            journal_project: None,
            #[cfg(debug_assertions)]
            probe: None,
        }
    }
    #[cfg(debug_assertions)]
    pub fn set_durability_probe(&mut self, probe: impl FnMut(DurableStage) + 'static) {
        self.probe = Some(Box::new(probe));
    }
    fn check(&self, editor: &Editor, target: &Target) -> Result<(), Value> {
        if self.read.target(editor) != *target {
            return Err(error("TARGET_MISMATCH"));
        }
        if !self.enabled {
            return Err(error("WRITE_NOT_AUTHORIZED"));
        }
        Ok(())
    }
    pub fn prepare(&mut self, editor: &Editor, p: Prepare) -> Value {
        self.prepare_with_originals(editor, p, None)
    }
    pub(crate) fn prepare_with_originals(
        &mut self,
        editor: &Editor,
        p: Prepare,
        admission: Option<&crate::project_repository::constraints::ConstraintCommitGuard>,
    ) -> Value {
        if let Err(e) = self.check(editor, &p.target) {
            return e;
        }
        if p.expected_revision != editor.revision {
            return error("REVISION_CONFLICT");
        }
        if let Err(e) = self.check_write_grant(editor) {
            return e;
        }

        self.previews
            .retain(|_, p| p.created.elapsed() < Duration::from_secs(600));
        if self.previews.len() >= 64 {
            return error("PREVIEW_CAPACITY");
        }
        if p.change.creates.len() + p.change.edits.len() > 256 {
            return error("BATCH_LIMIT");
        }
        match editor.prepare_batch_change_admitted(p.expected_revision, &p.change, admission) {
            Err(e) => json!({"error":e}),
            Ok(candidate) => {
                if let Err(e) = self.check_object_candidate(editor, &candidate) {
                    return e;
                }
                self.sequence += 1;
                let id = format!(
                    "{}-{}-{}",
                    self.read.target(editor).instance_id,
                    editor.document_session(),
                    self.sequence
                );
                let mut intent_payload = json!(p.change);
                self.tag_object_intent(&mut intent_payload, &candidate);
                let hash = crate::automation_journal::canonical_hash(&intent_payload)
                    .expect("validated scoped intent");
                let delta = candidate.difference();
                let changed_ids = candidate.affected_ids();
                let story_changes = candidate.story_changes();
                let changes = delta
                    .elements
                    .iter()
                    .map(|edit| PreviewElementChange {
                        id: edit.id,
                        action: match (&edit.before, &edit.after) {
                            (None, Some(_)) => PreviewAction::Create,
                            (Some(_), None) => PreviewAction::Remove,
                            _ => PreviewAction::Update,
                        },
                        native_type: edit
                            .after
                            .as_ref()
                            .or(edit.before.as_ref())
                            .expect("nonempty delta")
                            .kind()
                            .into(),
                    })
                    .collect();
                let affected: std::collections::HashSet<_> = changed_ids.iter().copied().collect();
                let mut pairs = std::collections::BTreeSet::new();
                for element in editor
                    .project
                    .elements
                    .iter()
                    .chain(&candidate.project().elements)
                {
                    if let crate::model::Shape::Opening { host_wall, .. } = &element.shape {
                        if affected.contains(&element.id) || affected.contains(host_wall) {
                            pairs.insert((element.id, *host_wall));
                        }
                    }
                }
                let dependencies = pairs
                    .into_iter()
                    .map(|(element_id, host_wall)| PreviewHostDependency {
                        element_id,
                        host_wall,
                    })
                    .collect();
                let site_context_change =
                    (editor.project.site != candidate.project().site).then(|| SiteContextChange {
                        before: editor.project.site.clone(),
                        after: candidate.project().site.clone(),
                    });
                let program_change =
                    (editor.project.program != candidate.project().program).then(|| {
                        ProgramChange {
                            before: editor.project.program.clone(),
                            after: candidate.project().program.clone(),
                        }
                    });
                let evidence_change = (editor.project.evidence != candidate.project().evidence)
                    .then(|| EvidenceChange {
                        before: editor.project.evidence.clone(),
                        after: candidate.project().evidence.clone(),
                    });
                let rules_change =
                    (editor.project.rules != candidate.project().rules).then(|| RulesChange {
                        before: editor.project.rules.clone(),
                        after: candidate.project().rules.clone(),
                    });
                let report_template_change = (editor.project.drawings.reports
                    != candidate.project().drawings.reports)
                    .then(|| ReportTemplateChange {
                        before: editor.project.drawings.reports.clone(),
                        after: candidate.project().drawings.reports.clone(),
                    });
                let old_links = &editor.project.design_process.mass_spaces;
                let new_links = &candidate.project().design_process.mass_spaces;
                let mass_space_change = (old_links != new_links).then(|| MassSpaceChange {
                    before: old_links
                        .iter()
                        .filter(|l| new_links.iter().find(|n| n.zone == l.zone) != Some(*l))
                        .cloned()
                        .collect(),
                    after: new_links
                        .iter()
                        .filter(|l| old_links.iter().find(|n| n.zone == l.zone) != Some(*l))
                        .cloned()
                        .collect(),
                });
                let before_style = editor.project.plan_options.model_3d_style;
                let after_style = candidate.project().plan_options.model_3d_style;
                let model_3d_style_change =
                    (before_style != after_style).then_some(Model3dStyleChange {
                        before: before_style,
                        after: after_style,
                    });
                let result = json!({"result":PreviewResult {preview_id:id.clone(),payload_hash:hash.clone(),base_revision:candidate.base_revision(),context_revision:None,changed_ids,changes,story_changes,model_3d_style_change,site_context_change,program_change,evidence_change,rules_change,report_template_change,mass_space_change,dependencies,resolved_references:candidate.resolved_references().clone(),expires_in_seconds:600}});
                self.previews.insert(
                    id,
                    Preview {
                        context_revision: None,
                        candidate: std::rc::Rc::new(candidate),
                        native_confirmation: false,
                        hash,
                        created: Instant::now(),
                        intent_payload,
                        batch: Some(p.change),
                    },
                );
                result
            }
        }
    }
    fn operation_key(
        editor: &Editor,
        identity: &crate::automation_journal::OperationIdentity,
    ) -> String {
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(editor.document_session(), identity)).unwrap())
        )
    }
    fn execution_hash(batch: &BatchChange) -> String {
        crate::automation_journal::canonical_hash(batch).expect("validated batch")
    }
    pub(crate) fn configure_journal_directory(
        &mut self,
        directory: Option<std::path::PathBuf>,
    ) -> Result<(), String> {
        if directory.as_ref().is_some_and(|path| !path.is_absolute()) {
            return Err("Absolute automation journal directory required".into());
        }
        if self.journal_directory == directory {
            return Ok(());
        }
        if self.journal.is_some() || !self.previews.is_empty() || !self.applied.is_empty() {
            return Err("Active operation journal cannot be replaced".into());
        }
        self.journal_directory = directory;
        Ok(())
    }
    pub fn with_journal_directory(mut self, path: std::path::PathBuf) -> Self {
        self.journal_directory = Some(path);
        self
    }
    fn identity(
        &self,
        editor: &Editor,
        request: &str,
        original: Option<&Target>,
    ) -> Result<crate::automation_journal::OperationIdentity, Value> {
        let current = self.read.target(editor);
        let target = original.unwrap_or(&current);
        if target.project_id != current.project_id {
            return Err(error("PROJECT_MISMATCH"));
        }
        Ok(crate::automation_journal::OperationIdentity {
            caller_id: "local-user".into(),
            project_id: current.project_id.clone(),
            document_session: format!("{}:{}", target.instance_id, target.document_session_id),
            request_id: request.into(),
        })
    }
    fn ensure_journal(&mut self, editor: &Editor) -> Result<(), Value> {
        let Some(directory) = &self.journal_directory else {
            return Ok(());
        };
        if self.journal_project.as_deref() == Some(editor.project_identity())
            && self.journal.is_some()
        {
            return Ok(());
        }
        self.journal = None;
        self.journal_project = None;
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(directory)
            .map_err(|_| error("JOURNAL_UNAVAILABLE"))?;
        let journal = crate::automation_journal::Journal::open(
            directory.join(format!("{}.json", editor.project_identity())),
        )
        .map_err(|message| json!({"error":{"code":"JOURNAL_UNAVAILABLE","message":message}}))?;
        self.journal = Some(journal);
        self.journal_project = Some(editor.project_identity().into());
        Ok(())
    }
    pub(crate) fn cited_preview_commands(
        &self,
        preview_id: &str,
    ) -> Option<Vec<crate::command::ModelCommand>> {
        self.previews
            .get(preview_id)
            .filter(|p| p.candidate.needs_cited_recheck())
            .map(|p| p.candidate.commands().to_vec())
    }
    pub fn commit(&mut self, editor: &mut Editor, p: Commit) -> Value {
        self.commit_admitted(editor, p, false)
    }
    fn commit_admitted(
        &mut self,
        editor: &mut Editor,
        p: Commit,
        native_confirmation: bool,
    ) -> Value {
        self.commit_with_originals(editor, p, native_confirmation, None)
    }
    pub(crate) fn commit_with_originals(
        &mut self,
        editor: &mut Editor,
        p: Commit,
        native_confirmation: bool,
        admission: Option<&crate::project_repository::constraints::ConstraintCommitGuard>,
    ) -> Value {
        use crate::automation_journal::Phase;
        if self.read.target(editor) != p.target {
            return error("TARGET_MISMATCH");
        }
        if !native_confirmation {
            if let Err(e) = self.check(editor, &p.target) {
                return e;
            }
        }
        self.observe_document(editor);
        if p.request_id.is_empty() || p.request_id.len() > 256 {
            return error("INVALID_REQUEST_ID");
        }
        if let Err(e) = self.ensure_journal(editor) {
            return e;
        }
        let identity = match self.identity(editor, &p.request_id, None) {
            Ok(id) => id,
            Err(e) => return e,
        };
        if let Some(applied) = self.applied.get(&Self::operation_key(editor, &identity)) {
            if applied.native_confirmation && !native_confirmation {
                return error("NATIVE_CONFIRMATION_REQUIRED");
            }
            let same_input = self.previews.get(&p.preview_id).is_some_and(|preview| {
                preview.hash == applied.payload_hash
                    && preview.execution_hash() == applied.execution_hash
                    && preview.candidate.base_revision() == applied.receipt.revision_before
                    && preview.candidate.belongs_to_session(editor)
            });
            if applied.preview_id != p.preview_id && !same_input {
                return error("REQUEST_ID_CONFLICT");
            }
            return self.operation(
                editor,
                Operation {
                    target: p.target,
                    request_id: p.request_id,
                    original_target: None,
                },
            );
        }
        if let Some(journal) = &self.journal {
            match journal.get(&identity) {
                Err(message) => {
                    return json!({"error":{"code":"OPERATION_OUTCOME_UNKNOWN","message":message}});
                }
                Ok(Some(_)) => return error("RECOVERY_REQUIRED"),
                Ok(None) => {}
            }
        }
        if editor.change_receipt(&p.request_id).is_some() {
            return error("REQUEST_ID_CONFLICT");
        }
        let Some(preview) = self.previews.get(&p.preview_id) else {
            return error("PREVIEW_NOT_FOUND");
        };
        if preview
            .candidate
            .project()
            .design_process
            .updates
            .iter()
            .any(|record| {
                !editor
                    .project
                    .design_process
                    .updates
                    .iter()
                    .any(|old| old.id == record.id)
                    && record.id != p.request_id
            })
        {
            return error("TASK_UPDATE_REQUEST_ID_MISMATCH");
        }
        if preview.native_confirmation != native_confirmation {
            return error("NATIVE_CONFIRMATION_REQUIRED");
        }
        if preview.created.elapsed() >= Duration::from_secs(600) {
            return error("PREVIEW_EXPIRED");
        }
        if !preview.candidate.matches_baseline(editor) {
            return error("REVISION_CONFLICT");
        }
        if preview
            .context_revision
            .as_ref()
            .is_some_and(|token| *token != self.read.context_revision(editor))
        {
            return error("CONTEXT_CONFLICT");
        }
        if !native_confirmation {
            if let Err(e) = self.write_authorization(editor) {
                return e;
            }
            if let Err(e) = self.check_object_candidate(editor, &preview.candidate) {
                return e;
            }
            if let Err(e) = self.check_object_operation(&preview.intent_payload) {
                return e;
            }
            if let Err(e) = self.check_object_creation_request(&preview.candidate, &p.request_id) {
                return e;
            }
            if self.object_task_scope.is_some() && preview.candidate.commands().iter().any(|c|matches!(c,crate::command::ModelCommand::Evidence{edit:crate::evidence::EvidenceEdit::RecordTopicReply{reply,..}} if reply.id!=p.request_id)){return error("OBJECT_TASK_REPLY_REQUEST_ID_MISMATCH");}
        }
        let mut rechecked;
        let preview = if let Some(guard) = admission {
            let fresh = match preview.candidate.recheck_cited(editor, guard) {
                Ok(candidate) => candidate,
                Err(e) => {
                    return json!({"error":{"code":e.code,"message":e.message},"operation_submitted":false});
                }
            };
            rechecked = preview.clone();
            rechecked.candidate = std::rc::Rc::new(fresh);
            &rechecked
        } else {
            preview
        };
        if let Err(e) = preview.candidate.validate_cited_admission(editor) {
            return json!({"error":{"code":e.code,"message":e.message},"operation_submitted":false});
        }
        let Some(revision_after) = editor.revision.checked_add(1) else {
            return error("REVISION_EXHAUSTED");
        };
        let planned = crate::model_service::ChangeReceipt {
            design_process_changed: editor.project.design_process
                != preview.candidate.project().design_process,
            edit_state_changed: editor.project.edit_state != preview.candidate.project().edit_state,
            request_id: p.request_id.clone(),
            document_session: editor.document_session(),
            revision_before: editor.revision,
            revision_after,
            changed_ids: preview.candidate.affected_ids(),
            site_context_changed: editor.project.site != preview.candidate.project().site,
            program_changed: editor.project.program != preview.candidate.project().program,
            evidence_changed: editor.project.evidence != preview.candidate.project().evidence,
            rules_changed: editor.project.rules != preview.candidate.project().rules,
            drawings_changed: editor.project.drawings != preview.candidate.project().drawings,
            resolved_references: preview.candidate.resolved_references().clone(),
        };
        let mut after_with_history = preview.candidate.project().clone();
        after_with_history.job_archive = match editor
            .project
            .job_archive
            .merge(&after_with_history.job_archive)
        {
            Ok(history) => history,
            Err(message) => {
                return json!({"error":{"code":"JOB_HISTORY_CONFLICT","message":message}});
            }
        };
        if let Some(journal) = &mut self.journal {
            if let Err(message) = journal.begin_model(
                identity.clone(),
                preview.hash.clone(),
                &editor.project,
                &after_with_history,
                preview.batch.clone(),
                editor.file_generation().map(str::to_owned),
                Some(json!(planned)),
                Some(preview.intent_payload.clone()),
            ) {
                return json!({"error":{"code":"JOURNAL_WRITE_FAILED","message":message}});
            }
        }
        #[cfg(debug_assertions)]
        if let Some(probe) = &mut self.probe {
            probe(DurableStage::IntentStored);
        }
        match editor.commit_change(&p.request_id, &preview.candidate) {
            Err(e) => {
                if let Some(journal) = &mut self.journal {
                    if journal
                        .transition(&identity, Phase::Cancelled, None)
                        .is_err()
                    {
                        return error("OPERATION_OUTCOME_UNKNOWN");
                    }
                }
                json!({"error":e})
            }
            Ok(receipt) => {
                #[cfg(debug_assertions)]
                if let Some(probe) = &mut self.probe {
                    probe(DurableStage::ModelCommitted);
                }
                self.applied.insert(
                    Self::operation_key(editor, &identity),
                    Applied {
                        preview_id: p.preview_id.clone(),
                        after: editor.project.clone(),
                        history_len: editor.undo.len(),
                        delta: editor.undo.last().expect("commit history").delta.clone(),
                        undone: false,
                        receipt: receipt.clone(),
                        identity: identity.clone(),
                        payload_hash: preview.hash.clone(),
                        execution_hash: preview.execution_hash(),
                        native_confirmation,
                    },
                );
                let hash = preview.hash.clone();
                self.previews.remove(&p.preview_id);
                if let Some(journal) = &mut self.journal {
                    if journal
                        .transition(&identity, Phase::Committed, Some(json!(receipt)))
                        .is_err()
                    {
                        return error("OPERATION_OUTCOME_UNKNOWN");
                    }
                }
                #[cfg(debug_assertions)]
                if let Some(probe) = &mut self.probe {
                    probe(DurableStage::CommitStored);
                }
                if native_confirmation {
                    self.observe_document(editor);
                } else {
                    self.advance_owned_write(editor);
                }
                json!({"result":{"status":"committed","receipt":receipt,"payload_hash":hash,"operation_identity":identity}})
            }
        }
    }
    pub fn operation(&mut self, editor: &Editor, p: Operation) -> Value {
        use crate::automation_journal::Phase;
        if self.read.target(editor) != p.target {
            return error("TARGET_MISMATCH");
        }
        if let Err(e) = self.ensure_journal(editor) {
            return e;
        }
        let identity = match self.identity(editor, &p.request_id, p.original_target.as_ref()) {
            Ok(id) => id,
            Err(e) => return e,
        };
        if let Some(journal) = &self.journal {
            return match journal.get(&identity) {
                Err(message) => {
                    json!({"error":{"code":"OPERATION_OUTCOME_UNKNOWN","message":message}})
                }
                Ok(None) => error("OPERATION_NOT_FOUND"),
                Ok(Some(record)) => {
                    let effect = journal
                        .reconcile(&identity, editor.project_identity(), &editor.project)
                        .map(|m| format!("{m:?}"))
                        .unwrap_or_else(|_| "Unknown".into());
                    let status = match record.phase {
                        Phase::Intent | Phase::UndoIntent | Phase::Unknown => "unknown",
                        Phase::Committed => "committed",
                        Phase::Undone => "undone",
                        Phase::Cancelled => "cancelled",
                    };
                    json!({"result":{"status":status,"phase":record.phase,"receipt":record.receipt,"operation_identity":identity,"current_effect":effect,"recovery_required":matches!(record.phase,Phase::Intent|Phase::UndoIntent|Phase::Unknown)||(record.phase==Phase::Committed&&effect!="AfterImage")||(record.phase==Phase::Undone&&effect!="BeforeImage")}})
                }
            };
        }
        match self
            .applied
            .get(&Self::operation_key(editor, &identity))
            .filter(|a| a.identity == identity)
        {
            None => error("OPERATION_NOT_FOUND"),
            Some(applied) => {
                json!({"result":{"status":if applied.undone{"undone"}else{"committed"},"receipt":applied.receipt,"operation_identity":identity}})
            }
        }
    }
    pub fn undo(&mut self, editor: &mut Editor, p: Operation) -> Value {
        self.undo_admitted(editor, p, false)
    }
    fn undo_admitted(
        &mut self,
        editor: &mut Editor,
        p: Operation,
        local_confirmation: bool,
    ) -> Value {
        use crate::automation_journal::Phase;
        if self.read.target(editor) != p.target {
            return error("TARGET_MISMATCH");
        }
        if !local_confirmation {
            if let Err(e) = self.check(editor, &p.target) {
                return e;
            }
        }
        if let Err(e) = self.ensure_journal(editor) {
            return e;
        }
        let identity = match self.identity(editor, &p.request_id, p.original_target.as_ref()) {
            Ok(id) => id,
            Err(e) => return e,
        };
        let Some(applied) = self
            .applied
            .get(&Self::operation_key(editor, &identity))
            .filter(|a| a.identity == identity)
        else {
            return error("UNDO_HISTORY_UNAVAILABLE");
        };
        if applied.native_confirmation && !local_confirmation {
            return error("NATIVE_CONFIRMATION_REQUIRED");
        }
        if applied.undone {
            return json!({"result":{"status":"undone","revision":editor.revision}});
        }
        if editor.revision != applied.receipt.revision_after
            || !editor.project.same_model_state(&applied.after)
            || editor.undo.len() != applied.history_len
            || editor
                .undo
                .last()
                .is_none_or(|record| record.delta != applied.delta)
            || editor.revision == u64::MAX
        {
            return error("UNDO_CONFLICT");
        }
        if !local_confirmation {
            if let Err(e) = self.check_write_grant(editor) {
                return e;
            }
            if self.object_task_scope.is_some() {
                let original = match self.journal.as_ref().map(|j| j.get(&identity)) {
                    Some(Ok(Some(r))) => r,
                    _ => return error("OBJECT_TASK_OPERATION_OWNERSHIP_UNAVAILABLE"),
                };
                if let Err(e) = self.check_object_operation(
                    original.intent_payload.as_ref().unwrap_or(&Value::Null),
                ) {
                    return e;
                }
            }
        }
        if let Some(journal) = &mut self.journal {
            if let Err(message) =
                journal.begin_undo(&identity, editor.file_generation().map(str::to_owned))
            {
                return json!({"error":{"code":"JOURNAL_WRITE_FAILED","message":message}});
            }
        }
        #[cfg(debug_assertions)]
        if let Some(probe) = &mut self.probe {
            probe(DurableStage::UndoIntentStored);
        }
        editor.undo();
        #[cfg(debug_assertions)]
        if let Some(probe) = &mut self.probe {
            probe(DurableStage::ModelUndone);
        }
        self.applied
            .get_mut(&Self::operation_key(editor, &identity))
            .expect("checked owned operation")
            .undone = true;
        if let Some(journal) = &mut self.journal {
            let receipt = journal
                .get(&identity)
                .ok()
                .flatten()
                .and_then(|r| r.receipt.clone());
            if journal
                .transition(&identity, Phase::Undone, receipt)
                .is_err()
            {
                return error("OPERATION_OUTCOME_UNKNOWN");
            }
        }
        #[cfg(debug_assertions)]
        if let Some(probe) = &mut self.probe {
            probe(DurableStage::UndoStored);
        }
        if local_confirmation {
            self.observe_document(editor);
        } else {
            self.advance_owned_write(editor);
        }
        json!({"result":{"status":"undone","revision":editor.revision}})
    }
    pub fn cancel(&mut self, editor: &Editor, p: Cancel) -> Value {
        if self.read.target(editor) != p.target {
            return error("TARGET_MISMATCH");
        }
        if let Some(applied) = self.applied.values().find(|a| a.preview_id == p.preview_id) {
            if applied.receipt.document_session != editor.document_session()
                || applied.identity.project_id != editor.project_identity()
            {
                return error("TARGET_MISMATCH");
            }
            return json!({"result":{"status":"already_committed"}});
        }
        if self
            .previews
            .get(&p.preview_id)
            .is_some_and(|preview| preview.native_confirmation)
        {
            return error("NATIVE_CONFIRMATION_REQUIRED");
        }
        if self
            .previews
            .get(&p.preview_id)
            .is_some_and(|preview| !preview.candidate.belongs_to_session(editor))
        {
            return error("TARGET_MISMATCH");
        }
        if self.previews.remove(&p.preview_id).is_some() {
            json!({"result":{"status":"cancelled"}})
        } else {
            error("PREVIEW_NOT_FOUND")
        }
    }
}

impl ChangeService {
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.write_grant = None;
            if let Some(g) = &mut self.object_task_scope {
                g.suspended = true;
            }
        }
    }
    pub fn describe(&self, editor: &Editor, target: &Target) -> Value {
        if self.read.target(editor) != *target {
            return error("TARGET_MISMATCH");
        }
        let build = || -> Result<BatchChange, String> {
            use crate::model::{OpeningKind, Point, Shape, SlabContour};
            use crate::model_service::{ElementReference, ElementTemplate, NamedElement};
            let p = Point::new;
            let mut elements = editor.wall_creation_candidates(vec![
                (p(0., 0.), p(8., 0.)),
                (p(8., 0.), p(8., 6.)),
                (p(8., 6.), p(0., 6.)),
                (p(0., 6.), p(0., 0.)),
            ])?;
            let door = editor.opening_in_batch_candidate(
                &elements,
                OpeningKind::Door,
                elements[0].id,
                2.,
                OpeningKind::Door.defaults(),
            )?;
            elements.push(door);
            elements.push(editor.slab_creation_candidate(SlabContour {
                points: vec![p(0., 0.), p(8., 0.), p(8., 6.), p(0., 6.)],
                ..Default::default()
            })?);
            let names = ["south", "east", "north", "west", "door", "floor"];
            Ok(BatchChange {
                creates: elements
                    .into_iter()
                    .zip(names)
                    .map(|(mut e, name)| {
                        e.id = 0;
                        let host = if let Shape::Opening { host_wall, .. } = &mut e.shape {
                            *host_wall = 0;
                            Some(ElementReference::Temporary("south".into()))
                        } else {
                            None
                        };
                        NamedElement {
                            name: name.into(),
                            template: ElementTemplate { element: e, host },
                        }
                    })
                    .collect(),
                ..Default::default()
            })
        };
        match build() {
            Err(message) => {
                json!({"error":{"code":"DESCRIPTION_CONTEXT_INVALID","message":message}})
            }
            Ok(example) => {
                json!({"result":{"command":"batch_change","version":1,"length_unit":"m","coordinate_space":"project","reference":{"existing":{"kind":"existing","value":1},"temporary":{"kind":"temporary","value":"south"}},"semantics":"Creates resolve together, then edits in order. One atomic commit and Undo. Templates id=0; new opening host_wall=0 plus explicit host. Update replaces a complete element template preserving the referenced ID. Move offset is world XY metres. Query fields=shape for full parameters. Angles in existing native templates retain documented degrees.","example":example,"edits_examples":[{"operation":"move","targets":[{"kind":"existing","value":1}],"offset":{"x":1.0,"y":0.0}},{"operation":"update","target":{"kind":"existing","value":1},"template":example.creates[0].template}],"prerequisites":"Valid project target, expected revision and explicit WRITE authorization. Example uses current active story and creation defaults; inspect/change all relevant dimensions before prepare."}})
            }
        }
    }
}

#[derive(Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrepareNative {
    #[serde(default)]
    pub expected_context_revision: Option<String>,
    pub target: Target,
    pub expected_revision: u64,
    pub change: crate::automation_registry::NativeBatch,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrepareDiscovered {
    #[serde(default)]
    pub expected_context_revision: Option<String>,
    pub target: Target,
    pub expected_revision: u64,
    pub change: crate::automation_registry::DiscoveredBatch,
}
impl ChangeService {
    pub fn prepare_native(&mut self, editor: &Editor, p: PrepareNative) -> Value {
        self.prepare_native_with_originals(editor, p, None)
    }
    pub(crate) fn prepare_native_with_originals(
        &mut self,
        editor: &Editor,
        p: PrepareNative,
        admission: Option<&crate::project_repository::constraints::ConstraintCommitGuard>,
    ) -> Value {
        let intent_payload = json!(p.change);
        self.prepare_intent(
            editor,
            p.target,
            p.expected_revision,
            p.expected_context_revision,
            p.change,
            intent_payload,
            admission,
        )
    }
    pub fn prepare_discovered(&mut self, editor: &Editor, p: PrepareDiscovered) -> Value {
        self.prepare_discovered_with_originals(editor, p, None)
    }
    pub(crate) fn prepare_discovered_with_originals(
        &mut self,
        editor: &Editor,
        p: PrepareDiscovered,
        admission: Option<&crate::project_repository::constraints::ConstraintCommitGuard>,
    ) -> Value {
        if let Err(e) = self.check(editor, &p.target) {
            return e;
        }
        if p.expected_revision != editor.revision {
            return error("REVISION_CONFLICT");
        }
        let intent_payload = json!(p.change);
        match p.change.resolve() {
            Ok(change) => self.prepare_intent(
                editor,
                p.target,
                p.expected_revision,
                p.expected_context_revision,
                change,
                intent_payload,
                admission,
            ),
            Err(message) => json!({"error":{"code":"INVALID_NATIVE_COMMAND","message":message}}),
        }
    }
    fn prepare_intent(
        &mut self,
        editor: &Editor,
        target: Target,
        expected_revision: u64,
        expected_context_revision: Option<String>,
        native: crate::automation_registry::NativeBatch,
        mut intent_payload: Value,
        admission: Option<&crate::project_repository::constraints::ConstraintCommitGuard>,
    ) -> Value {
        if let Err(e) = self.check(editor, &target) {
            return e;
        }
        if expected_revision != editor.revision {
            return error("REVISION_CONFLICT");
        }
        if let Err(e) = self.check_write_grant(editor) {
            return e;
        }
        if crate::design_process::service::working_scope(&editor.project).is_some()
            && !native.creates.is_empty()
            && expected_context_revision.is_none()
        {
            return error("CONTEXT_REQUIRED");
        }
        if let Some(token) = &expected_context_revision {
            if *token != self.read.context_revision(editor) {
                return error("CONTEXT_CONFLICT");
            }
            intent_payload = json!({"change":intent_payload,"expected_context_revision":token});
        }
        match native.lower(editor) {
            Err(message) => json!({"error":{"code":"INVALID_NATIVE_COMMAND","message":message}}),
            Ok(change) => {
                let mut result = self.prepare_with_originals(
                    editor,
                    Prepare {
                        target,
                        expected_revision,
                        change,
                    },
                    admission,
                );
                if let Some(id) = result["result"]["preview_id"].as_str().map(str::to_owned) {
                    if let Some(preview) = self.previews.get_mut(&id) {
                        if let Some(binding) = preview.intent_payload.get("object_task").cloned() {
                            intent_payload["object_task"] = binding;
                        }
                        let scoped_hash =
                            crate::automation_journal::canonical_hash(&intent_payload)
                                .expect("scoped native intent");
                        preview.hash = scoped_hash.clone();
                        result["result"]["payload_hash"] = json!(scoped_hash);
                        preview.intent_payload = intent_payload;
                        preview.context_revision = expected_context_revision.clone();
                        result["result"]["context_revision"] = json!(expected_context_revision);
                    }
                    if let Some(preview) = self.previews.get(&id) {
                        result["result"]["payload_hash"] = json!(preview.hash);
                    }
                }
                result
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recover {
    pub target: Target,
    pub expected_revision: u64,
    pub original_target: Target,
    pub request_id: String,
}
impl ChangeService {
    pub fn recover(&mut self, editor: &mut Editor, p: Recover) -> Value {
        self.recover_admitted(editor, p, false)
    }
    fn recover_admitted(
        &mut self,
        editor: &mut Editor,
        p: Recover,
        local_confirmation: bool,
    ) -> Value {
        use crate::automation_journal::Phase;
        if self.read.target(editor) != p.target {
            return error("TARGET_MISMATCH");
        }
        if !local_confirmation {
            if let Err(e) = self.check(editor, &p.target) {
                return e;
            }
        }
        if p.expected_revision != editor.revision {
            return error("REVISION_CONFLICT");
        }
        if let Err(e) = self.ensure_journal(editor) {
            return e;
        }
        let identity = match self.identity(editor, &p.request_id, Some(&p.original_target)) {
            Ok(id) => id,
            Err(e) => return e,
        };
        let Some(journal) = &self.journal else {
            return error("DURABILITY_NOT_CONFIGURED");
        };
        let record = match journal.get(&identity) {
            Ok(Some(record)) => record.clone(),
            Ok(None) => return error("OPERATION_NOT_FOUND"),
            Err(_) => return error("OPERATION_OUTCOME_UNKNOWN"),
        };
        let undo = matches!(record.phase, Phase::UndoIntent | Phase::Undone);
        if record.phase == Phase::Cancelled {
            return json!({"result":{"status":"cancelled"}});
        }
        if record.phase == Phase::Unknown {
            return error("OPERATION_OUTCOME_UNKNOWN");
        }
        let mut receipt = record.receipt.clone().or(record.planned_receipt.clone());
        if record.phase == Phase::Intent && record.receipt.is_none() {
            if let Some(value) = &mut receipt {
                value["provenance"] = json!("reconciled_from_durable_intent");
            }
        }
        if !undo && receipt.is_none() {
            return error("RECOVERY_RECEIPT_UNAVAILABLE");
        }
        let desired = if undo {
            match record.undo_image() {
                Some(image) => image,
                None => return error("RECOVERY_IMAGE_UNAVAILABLE"),
            }
        } else {
            record.after_image.clone()
        };
        let merged_history = match editor.project.job_archive.merge(&desired.job_archive) {
            Ok(history) => history,
            Err(message) => {
                return json!({"error":{"code":"JOB_HISTORY_CONFLICT","message":message}});
            }
        };
        let history_changed = merged_history != editor.project.job_archive;
        let opposite = if undo {
            Some(&record.after_image)
        } else {
            record.before_image.as_ref()
        };
        let mut restoration = None;
        if !editor.project.same_model_state(&desired) {
            if record.native_confirmation && !local_confirmation {
                return error("NATIVE_CONFIRMATION_REQUIRED");
            }
            if matches!(record.phase, Phase::Committed | Phase::Undone) {
                return error("RECOVERY_DOCUMENT_REQUIRED");
            }
            let generation = if undo {
                record.undo_generation.as_deref()
            } else {
                record.origin_generation.as_deref()
            };
            if opposite.is_none_or(|image| !editor.project.same_model_state(image))
                || generation.is_none()
                || generation != editor.file_generation()
                || !editor.file_generation_matches_disk()
                || editor.dirty()
                || !editor.undo.is_empty()
                || !editor.redo.is_empty()
            {
                return error("RECOVERY_CONFLICT");
            }
            if !local_confirmation {
                if let Err(e) = self.check_write_grant(editor) {
                    return e;
                }
            }
            if !local_confirmation {
                if let Err(e) = self
                    .check_object_operation(record.intent_payload.as_ref().unwrap_or(&Value::Null))
                {
                    return e;
                }
            }
            let prepared = match editor.prepare_recovery_snapshot(p.expected_revision, desired) {
                Ok(prepared) => prepared,
                Err(e) => return json!({"error":e}),
            };
            let recovery_id = format!(
                "recovery-{:x}",
                Sha256::digest(serde_json::to_vec(&(&identity, undo)).unwrap())
            );
            let receipt = match editor.commit_change(&recovery_id, &prepared) {
                Ok(receipt) => receipt,
                Err(e) => return json!({"error":e}),
            };
            if !undo {
                self.applied.insert(
                    Self::operation_key(editor, &identity),
                    Applied {
                        preview_id: "recovered".into(),
                        after: editor.project.clone(),
                        history_len: editor.undo.len(),
                        delta: editor.undo.last().unwrap().delta.clone(),
                        undone: false,
                        receipt: receipt.clone(),
                        identity: identity.clone(),
                        payload_hash: record.payload_hash.clone(),
                        native_confirmation: record.native_confirmation,
                        execution_hash: record
                            .batch
                            .as_ref()
                            .map(Self::execution_hash)
                            .unwrap_or_else(|| record.payload_hash.clone()),
                    },
                );
            }
            restoration = Some(receipt);
        }
        let phase = if undo {
            Phase::Undone
        } else {
            Phase::Committed
        };
        if let Some(journal) = &mut self.journal {
            if journal
                .transition(&identity, phase, receipt.clone())
                .is_err()
            {
                return error("OPERATION_OUTCOME_UNKNOWN");
            }
        }
        editor.project.job_archive = merged_history;
        if restoration.is_some() {
            if local_confirmation {
                self.observe_document(editor);
            } else {
                self.advance_owned_write(editor);
            }
        }
        json!({"result":{"status":if undo{"undone"}else{"committed"},"receipt":receipt,"restoration_receipt":restoration,"current_revision":editor.revision,"operation_identity":identity,"history_changed":history_changed,"reconciled":true}})
    }
}

#[cfg(all(test, debug_assertions))]
#[path = "automation_change_tests.rs"]
mod durability_tests;

impl ChangeService {
    /// Explicit document lifecycle action. The application additionally rejects a confirmed file.
    pub fn recover_document(&mut self, editor: &mut Editor, p: Recover) -> Value {
        use crate::automation_journal::Phase;
        if let Err(e) = self.check(editor, &p.target) {
            return e;
        }
        if let Err(e) = self.check_write_grant(editor) {
            return e;
        }
        if p.expected_revision != editor.revision {
            return error("REVISION_CONFLICT");
        }
        if editor.file_generation().is_some()
            || !editor.project.job_archive.is_empty()
            || editor.dirty()
            || !editor.undo.is_empty()
            || !editor.redo.is_empty()
        {
            return error("DOCUMENT_ACTIVE");
        }
        if !crate::storage::valid_document_identity(&p.original_target.project_id) {
            return error("PROJECT_MISMATCH");
        }
        let mut candidate = Editor::new(Project::default());
        if candidate
            .adopt_document_identity(p.original_target.project_id.clone())
            .is_err()
        {
            return error("PROJECT_MISMATCH");
        }
        if let Err(e) = self.ensure_journal(&candidate) {
            return e;
        }
        let identity = match self.identity(&candidate, &p.request_id, Some(&p.original_target)) {
            Ok(id) => id,
            Err(e) => return e,
        };
        let Some(journal) = &self.journal else {
            return error("DURABILITY_NOT_CONFIGURED");
        };
        let record = match journal.get(&identity) {
            Ok(Some(record)) => record.clone(),
            Ok(None) => return error("OPERATION_NOT_FOUND"),
            Err(_) => return error("OPERATION_OUTCOME_UNKNOWN"),
        };
        if record.native_confirmation {
            return error("NATIVE_CONFIRMATION_REQUIRED");
        }
        if matches!(record.phase, Phase::Unknown | Phase::Cancelled) {
            return error("OPERATION_NOT_RECOVERABLE");
        }
        let Some(before) = record.before_image.clone() else {
            return error("RECOVERY_IMAGE_UNAVAILABLE");
        };
        let undo = matches!(record.phase, Phase::UndoIntent | Phase::Undone);
        let (base, desired) = if undo {
            (
                record.after_image.clone(),
                record.undo_image().expect("checked before image"),
            )
        } else {
            (before, record.after_image.clone())
        };
        candidate = Editor::new(base);
        if candidate
            .adopt_document_identity(identity.project_id.clone())
            .is_err()
        {
            return error("PROJECT_MISMATCH");
        }
        let prepared = match candidate.prepare_recovery_snapshot(0, desired) {
            Ok(prepared) => prepared,
            Err(e) => return json!({"error":e}),
        };
        let recovery_id = format!(
            "document-recovery-{:x}",
            Sha256::digest(serde_json::to_vec(&(&identity, undo)).unwrap())
        );
        let restoration = match candidate.commit_change(&recovery_id, &prepared) {
            Ok(receipt) => receipt,
            Err(e) => return json!({"error":e}),
        };
        candidate.recovered_unsaved = true;
        candidate.status = "Recovered unsaved project".into();
        let mut receipt = record.receipt.clone().or(record.planned_receipt.clone());
        if !undo && receipt.is_none() {
            return error("RECOVERY_RECEIPT_UNAVAILABLE");
        }
        if record.phase == Phase::Intent {
            if let Some(value) = &mut receipt {
                value["provenance"] = json!("reconciled_into_recovered_document");
            }
        }
        let phase = if undo {
            Phase::Undone
        } else {
            Phase::Committed
        };
        if let Some(journal) = &mut self.journal {
            if journal
                .transition(&identity, phase, receipt.clone())
                .is_err()
            {
                return error("OPERATION_OUTCOME_UNKNOWN");
            }
        }
        self.applied.insert(
            Self::operation_key(&candidate, &identity),
            Applied {
                preview_id: "recovered-document".into(),
                after: candidate.project.clone(),
                history_len: candidate.undo.len(),
                delta: candidate.undo.last().unwrap().delta.clone(),
                undone: undo,
                receipt: restoration.clone(),
                identity: identity.clone(),
                payload_hash: record.payload_hash.clone(),
                native_confirmation: record.native_confirmation,
                execution_hash: record
                    .batch
                    .as_ref()
                    .map(Self::execution_hash)
                    .unwrap_or_else(|| record.payload_hash.clone()),
            },
        );
        *editor = candidate;
        self.advance_owned_write(editor);
        json!({"result":{"status":if undo{"undone"}else{"committed"},"target":self.read.target(editor),"receipt":receipt,"restoration_receipt":restoration,"recovered_document":true,"requires_save_as":true}})
    }
}
