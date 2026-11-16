//! Task association over the existing durable transaction coordinator.
//! These local library methods are not remote task authorization or RPC commands.
use super::*;

const INTENT_KIND: &str = "trusted_design_task_proposal_v1";
const FIGURE_INTENT_KIND: &str = "trusted_native_figure_proposal_v1";
// Linked figures and bound report metadata only. This must never admit arbitrary
// drawings, source assets, task state or model geometry through a native shortcut.
fn only_native_figure_change(before: &Project, after: &Project) -> bool {
    if before.design_process.task().is_some()
        || after.design_process.task().is_some()
        || (before.drawings.diagrams == after.drawings.diagrams
            && before.drawings.reports == after.drawings.reports)
    {
        return false;
    }
    let mut expected = before.clone();
    expected.drawings.diagrams = after.drawings.diagrams.clone();
    expected.drawings.reports = after.drawings.reports.clone();
    &expected == after
}

impl ChangeService {
    /// Admit an already validated opaque shared-host proposal. The caller must
    /// retain the returned preview and its original request identity. No replay
    /// commands are manufactured from snapshots or an empty geometry batch.
    pub fn prepare_task_proposal(
        &mut self,
        editor: &Editor,
        target: Target,
        task_id: &str,
        label: &str,
        candidate: PreparedChange,
    ) -> Value {
        self.prepare_task_proposal_admitted(
            editor,
            target,
            task_id,
            label,
            std::rc::Rc::new(candidate),
            false,
        )
    }
    pub(super) fn prepare_task_proposal_admitted(
        &mut self,
        editor: &Editor,
        target: Target,
        task_id: &str,
        label: &str,
        candidate: std::rc::Rc<PreparedChange>,
        native_confirmation: bool,
    ) -> Value {
        if self.read.target(editor) != target {
            return error("TARGET_MISMATCH");
        }
        if !native_confirmation {
            if let Err(e) = self.check(editor, &target) {
                return e;
            }
        }
        if !native_confirmation {
            if let Err(e) = self.check_object_candidate(editor, &candidate) {
                return e;
            }
        }
        if !candidate.matches_baseline(editor) {
            return error("REVISION_CONFLICT");
        }
        if !native_confirmation {
            if let Err(e) = self.check_write_grant(editor) {
                return e;
            }
        }
        if label.trim().is_empty() || label.len() > 256 || label.chars().any(char::is_control) {
            return error("INVALID_TASK_INTENT");
        }
        let before = editor.project.design_process.task();
        let after = candidate.project().design_process.task();
        let native_figure = native_confirmation
            && task_id.is_empty()
            && only_native_figure_change(&editor.project, candidate.project());
        // Native standalone creation is proved by one validated typed command,
        // never by accepting arbitrary element/header differences.
        let native_mass_space = native_confirmation
            && task_id.is_empty()
            && before.is_none()
            && after.is_none()
            && candidate.only_mass_space_command();
        let native_surfaces = native_confirmation
            && task_id.is_empty()
            && before.is_none()
            && after.is_none()
            && candidate.only_zone_surface_commands();
        let native_conditions =
            native_confirmation && task_id.is_empty() && candidate.verified_conditions_command();
        let native_checks =
            native_confirmation && task_id.is_empty() && candidate.only_constraint_commands();
        let native_discussion =
            native_confirmation && task_id.is_empty() && candidate.only_object_discussion_command();
        let native_standalone = native_figure
            || native_mass_space
            || native_conditions
            || native_checks
            || native_discussion
            || native_surfaces;
        let switching = before.is_some_and(|t| t.id != task_id);
        let changing_task = !native_standalone && before.is_none_or(|t| t.id != task_id);
        let safe_transition = switching
            && (crate::design_process::intent::is_exact_task_append(
                &editor.project,
                candidate.project(),
            )
            .unwrap_or(false)
                || crate::design_process::intent::is_exact_context_activation(
                    &editor.project,
                    candidate.project(),
                )
                .unwrap_or(false));
        if !native_standalone
            && (after.is_none_or(|t| t.id != task_id) || (switching && !safe_transition))
        {
            return error("DESIGN_TASK_MISMATCH");
        }
        // A task checkpoint never silently degrades to an in-memory receipt.
        if let Err(e) = self.ensure_journal(editor) {
            return e;
        }
        if self.journal.is_none() {
            return error("DURABILITY_NOT_CONFIGURED");
        }
        // An unresolved owned intent is not permission to replace its request
        // identity. Query/reconcile the original operation first.
        match self.journal.as_ref().expect("configured journal").records() {
            Err(_) => return error("OPERATION_OUTCOME_UNKNOWN"),
            Ok(records) => {
                if let Some(record) = records.into_iter().find(|record| {
                    matches!(
                        record.phase,
                        crate::automation_journal::Phase::Intent
                            | crate::automation_journal::Phase::UndoIntent
                            | crate::automation_journal::Phase::Unknown
                    ) && (changing_task
                        || native_standalone
                        || (native_confirmation && record.native_confirmation)
                        || record.intent_payload.as_ref().is_some_and(|intent| {
                            intent["kind"] == INTENT_KIND && intent["task_id"] == task_id
                        }))
                }) {
                    return json!({"error":{"code":"TASK_OPERATION_PENDING",
                        "operation_identity":record.identity,
                        "next_action":"inspect_original_operation; do not replay"}});
                }
            }
        }
        self.previews
            .retain(|_, p| p.created.elapsed() < Duration::from_secs(600));
        if self.previews.len() >= 64 {
            return error("PREVIEW_CAPACITY");
        }
        let (before_hash, after_hash) = match (
            crate::automation_journal::project_hash(&editor.project),
            crate::automation_journal::project_hash(candidate.project()),
        ) {
            (Ok(a), Ok(b)) => (a, b),
            _ => return error("INVALID_TASK_INTENT"),
        };
        let mut intent_payload = json!({
            "kind": if native_surfaces {"trusted_native_zone_surfaces_proposal_v1"} else if native_discussion {"trusted_native_object_discussion_proposal_v1"} else if native_conditions {"trusted_native_project_conditions_proposal_v1"} else if native_checks {"trusted_native_quantified_checks_proposal_v1"} else if native_mass_space {"trusted_native_mass_space_proposal_v1"} else if native_figure {FIGURE_INTENT_KIND} else {INTENT_KIND}, "task_id": (!native_standalone).then_some(task_id), "label": label,
            "origin": if native_confirmation { "native_confirmation" } else { "trusted_shared_host_library" },
            "native_confirmation": native_confirmation, "original_target": target,
            "expected_revision": candidate.base_revision(),
            "before_hash": before_hash, "after_hash": after_hash,
        });
        self.tag_object_intent(&mut intent_payload, &candidate);
        let hash = match crate::automation_journal::canonical_hash(&intent_payload) {
            Ok(hash) => hash,
            Err(_) => return error("INVALID_TASK_INTENT"),
        };
        self.sequence += 1;
        let id = format!(
            "{}-{}-{}",
            target.instance_id,
            editor.document_session(),
            self.sequence
        );
        let result = json!({"result": {
            "preview_id": id, "task_id": (!native_standalone).then_some(task_id), "payload_hash": hash,
            "base_revision": candidate.base_revision(), "changed_ids": candidate.affected_ids(),
            "expires_in_seconds": 600, "durability": "existing_operation_journal",
        }});
        self.previews.insert(
            id,
            Preview {
                candidate,
                created: Instant::now(),
                hash,
                intent_payload,
                context_revision: Some(self.read.context_revision(editor)),
                batch: None,
                native_confirmation,
            },
        );
        result
    }

    /// Only called after the native modal's explicit confirmation. This does
    /// not enable remote writes or advance their grant across a user edit.
    pub(crate) fn confirm_native_task(
        &mut self,
        editor: &mut Editor,
        request_id: &str,
        label: &str,
        candidate: std::rc::Rc<PreparedChange>,
    ) -> Value {
        if request_id.is_empty() || request_id.len() > 256 {
            return error("INVALID_REQUEST_ID");
        }
        let target = self.read.target(editor);
        if !candidate.matches_baseline(editor) {
            return error("REVISION_CONFLICT");
        }
        let task_id = match candidate.project().design_process.task() {
            Some(task) => task.id.clone(),
            None if candidate.only_zone_surface_commands()
                || candidate.only_object_discussion_command()
                || candidate.only_constraint_commands()
                || candidate.verified_conditions_command()
                || only_native_figure_change(&editor.project, candidate.project())
                || (editor.project.design_process.task().is_none()
                    && candidate.only_mass_space_command()) =>
            {
                String::new()
            }
            None => return error("DESIGN_TASK_MISMATCH"),
        };
        // Never re-admit a request with an existing durable outcome. Inspect
        // its original identity, even if the caller still holds the preview.
        if let Err(e) = self.ensure_journal(editor) {
            return e;
        }
        if let Some(journal) = &self.journal {
            match journal.records() {
                Ok(records) => {
                    if let Some(record) = records
                        .into_iter()
                        .find(|record| record.identity.request_id == request_id)
                    {
                        return json!({"error":{"code":"RECOVERY_REQUIRED","operation_identity":record.identity,"original_target":record.intent_payload.as_ref().map(|intent|&intent["original_target"])},"operation_submitted":false});
                    }
                }
                Err(_) => return error("OPERATION_OUTCOME_UNKNOWN"),
            }
        }
        if let Err(e) = candidate.validate_cited_admission(editor) {
            return json!({"error":{"code":e.code,"message":e.message},"operation_submitted":false});
        }
        let preview = self.prepare_task_proposal_admitted(
            editor,
            target.clone(),
            &task_id,
            label,
            candidate,
            true,
        );
        let Some(preview_id) = preview["result"]["preview_id"].as_str() else {
            return preview;
        };
        let mut result = self.commit_admitted(
            editor,
            Commit {
                target,
                preview_id: preview_id.into(),
                request_id: request_id.into(),
            },
            true,
        );
        result["operation_submitted"] = json!(true);
        result
    }

    pub(crate) fn cancel_native_intent(
        &mut self,
        editor: &Editor,
        original_target: Target,
        request_id: &str,
    ) -> Value {
        if let Err(e) = self.ensure_journal(editor) {
            return e;
        }
        let identity = match self.identity(editor, request_id, Some(&original_target)) {
            Ok(id) => id,
            Err(e) => return e,
        };
        let Some(journal) = &mut self.journal else {
            return error("DURABILITY_NOT_CONFIGURED");
        };
        let record = match journal.get(&identity) {
            Ok(Some(record)) => record,
            Ok(None) => return error("OPERATION_NOT_FOUND"),
            Err(_) => return error("OPERATION_OUTCOME_UNKNOWN"),
        };
        if !record.native_confirmation || record.phase != crate::automation_journal::Phase::Intent {
            return error("NATIVE_INTENT_NOT_CANCELLABLE");
        }
        let matches_before = if let Some(before) = &record.before_image {
            editor.project.same_model_state(before)
        } else {
            crate::automation_journal::project_hash(&editor.project)
                .ok()
                .as_deref()
                == Some(record.before_hash.as_str())
        };
        if !matches_before {
            return error("RECOVERY_CONFLICT");
        }
        let receipt = json!({"status":"cancelled","provenance":"explicit_native_before_image_cancel","historical_execution":"unknown","model_changed":false});
        match journal.transition(
            &identity,
            crate::automation_journal::Phase::Cancelled,
            Some(receipt),
        ) {
            Ok(_) => {
                json!({"result":{"status":"cancelled","operation_identity":identity,"historical_execution":"unknown","model_changed":false}})
            }
            Err(_) => error("OPERATION_OUTCOME_UNKNOWN"),
        }
    }

    /// Read-only resume hints: a recorded outcome and its present model effect
    /// are different facts. Reading this never adopts a task or renews a grant.
    pub fn task_checkpoint(&mut self, editor: &Editor) -> Value {
        let Some(task) = editor.project.design_process.task() else {
            return json!({"status":"no_active_task","operations":[]});
        };
        if self.journal_directory.is_none() {
            return json!({"status":"not_configured","task_id":task.id,"operations":[]});
        }
        if let Err(e) = self.ensure_journal(editor) {
            return json!({"status":"unknown","task_id":task.id,"error":e["error"]});
        }
        let journal = self.journal.as_ref().expect("configured journal");
        let records = match journal.records() {
            Ok(records) => records,
            Err(message) => return json!({"status":"unknown","task_id":task.id,"message":message}),
        };
        let mut operations = Vec::new();
        let mut total = 0;
        for record in records {
            let Some(intent) = &record.intent_payload else {
                continue;
            };
            if intent["kind"] != INTENT_KIND || intent["task_id"] != task.id {
                continue;
            }
            total += 1;
            if operations.len() == 64 {
                continue;
            }
            let effect = match journal.reconcile(
                &record.identity,
                editor.project_identity(),
                &editor.project,
            ) {
                Ok(value) => format!("{value:?}"),
                Err(message) => {
                    return json!({"status":"unknown","task_id":task.id,"message":message});
                }
            };
            operations.push(json!({
                "operation_identity": record.identity, "original_target": intent["original_target"],
                "label": intent["label"], "phase": record.phase,
                "payload_hash": record.payload_hash, "before_hash": record.before_hash,
                "after_hash": record.after_hash, "current_effect": effect,
                "receipt": record.receipt, "undo_preserves_design_history":record.undo_preserves_design_history, "native_confirmation":record.native_confirmation,
                "next_action": "inspect_original_operation; do not replay",
            }));
        }
        json!({"status":"available","task_id":task.id,"operations":operations,
            "total":total,"truncated":total>64,"limit":64,
            "authorization":"association only; reading does not grant task writes",
            "portability":"host journal directory; not embedded in the project",
            "ordering":"canonical operation identity; not chronological"})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        design_process::{Entry, service},
        model::Shape,
        storage,
    };
    use std::rc::Rc;
    fn proposal(editor: &Editor, task: &str) -> Rc<PreparedChange> {
        Rc::new(
            service::prepare_create(
                editor,
                task,
                service::Draft {
                    name: "Native authority fixture".into(),
                    question: "Retain original operation and manual authority".into(),
                    statements: vec![],
                    candidate_names: ["A".into(), "B".into()],
                    source_ids: editor
                        .project
                        .elements
                        .iter()
                        .filter(|e| e.story == 0 && matches!(e.shape, Shape::Zone { .. }))
                        .map(|e| e.id)
                        .collect(),
                    entry: Entry::ManualInterface,
                },
            )
            .unwrap(),
        )
    }
    fn root() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "oa-native-authority-{}",
            storage::new_document_identity()
        ));
        std::fs::create_dir(&root).unwrap();
        root
    }

    #[test]
    fn remote_grant_cannot_claim_native_preview_or_undo_and_local_confirm_does_not_regrant() {
        let root = root();
        let mut editor = Editor::new(storage::synthetic_building_fixture());
        let before = editor.project.clone();
        let mut changes = ChangeService::new("native-owner".into(), false)
            .with_journal_directory(root.join("journal"));
        let task = storage::new_document_identity();
        let target = changes.current_target(&editor);
        let preview = changes.prepare_task_proposal_admitted(
            &editor,
            target.clone(),
            &task,
            "Native confirmed proposal",
            proposal(&editor, &task),
            true,
        );
        let id = preview["result"]["preview_id"].as_str().unwrap().to_owned();
        // Even a separate explicit remote grant does not adopt a native preview.
        changes.authorize_current(&editor);
        let stolen = changes.commit(
            &mut editor,
            Commit {
                target: target.clone(),
                preview_id: id.clone(),
                request_id: "steal".into(),
            },
        );
        assert_eq!(stolen["error"]["code"], "NATIVE_CONFIRMATION_REQUIRED");
        assert_eq!(
            changes.cancel(
                &editor,
                Cancel {
                    target: target.clone(),
                    preview_id: id.clone()
                }
            )["error"]["code"],
            "NATIVE_CONFIRMATION_REQUIRED"
        );
        assert_eq!(editor.project, before);
        let committed = changes.commit_admitted(
            &mut editor,
            Commit {
                target: target.clone(),
                preview_id: id,
                request_id: "native-original".into(),
            },
            true,
        );
        assert_eq!(committed["result"]["status"], "committed");
        assert_eq!(changes.authorization_status(&editor)["status"], "suspended");
        changes.authorize_current(&editor);
        assert_eq!(
            changes.undo(
                &mut editor,
                Operation {
                    target: target.clone(),
                    original_target: None,
                    request_id: "native-original".into()
                }
            )["error"]["code"],
            "NATIVE_CONFIRMATION_REQUIRED"
        );
        changes.set_enabled(false);
        let undone = changes.undo_current(&mut editor, "native-original".into(), target);
        assert_eq!(undone["result"]["status"], "undone");
        assert_eq!(changes.authorization_status(&editor)["status"], "read_only");
        assert_eq!(editor.project.elements, before.elements);
        assert_eq!(editor.project.design_process.tasks.len(), 1);
        drop(changes);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_pending_intent_blocks_new_task_identity_and_cancellation_preserves_user_history() {
        let root = root();
        let mut editor = Editor::new(storage::synthetic_building_fixture());
        let before = editor.project.clone();
        let task = storage::new_document_identity();
        let prepared = proposal(&editor, &task);
        let mut changes = ChangeService::new("native-before-crash".into(), false)
            .with_journal_directory(root.join("journal"));
        let original = changes.current_target(&editor);
        changes.set_durability_probe(|stage| {
            if stage == DurableStage::IntentStored {
                panic!("expected protected-copy stop after native intent")
            }
        });
        let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            changes.confirm_native_task(
                &mut editor,
                "original-pending",
                "Native original",
                prepared,
            )
        }));
        assert!(stopped.is_err());
        assert_eq!(editor.project, before);
        drop(changes);
        let mut changes = ChangeService::new("native-after-crash".into(), false)
            .with_journal_directory(root.join("journal"));
        let new_task = storage::new_document_identity();
        let prepared = proposal(&editor, &new_task);
        let blocked = changes.confirm_native_task(
            &mut editor,
            "different-request",
            "Must not replace unknown task",
            prepared,
        );
        assert_eq!(blocked["error"]["code"], "TASK_OPERATION_PENDING");
        assert_eq!(
            blocked["error"]["operation_identity"]["request_id"],
            "original-pending"
        );
        let zone = editor
            .project
            .elements
            .iter()
            .find(|e| matches!(e.shape, Shape::Zone { .. }))
            .unwrap()
            .id;
        editor
            .apply(crate::command::ModelCommand::MoveElements {
                ids: vec![zone],
                offset: crate::model::Point::new(0.1, 0.),
            })
            .unwrap();
        let manual = editor.project.clone();
        assert_eq!(
            changes.cancel_native_intent(&editor, original.clone(), "original-pending")["error"]["code"],
            "RECOVERY_CONFLICT"
        );
        assert_eq!(editor.project, manual);
        editor.undo();
        let history = format!("{:?}{:?}", editor.undo, editor.redo);
        let cancelled = changes.cancel_native_intent(&editor, original.clone(), "original-pending");
        assert_eq!(cancelled["result"]["status"], "cancelled");
        assert_eq!(cancelled["result"]["historical_execution"], "unknown");
        assert_eq!(editor.project, before);
        assert_eq!(format!("{:?}{:?}", editor.undo, editor.redo), history);
        assert!(!changes.enabled());
        let lookup = changes.inspect_current(&editor, "original-pending".into(), original);
        assert_eq!(lookup["result"]["status"], "cancelled");
        drop(changes);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn remote_recovery_cannot_restore_a_native_record_after_restart() {
        let root = root();
        let before_file = root.join("before.json");
        let original = Editor::new(storage::synthetic_building_fixture());
        storage::save_document(&original.project, &before_file, original.project_identity())
            .unwrap();
        let mut editor = Editor::from_stored_document(&before_file).unwrap();
        let mut changes = ChangeService::new("native-first".into(), false)
            .with_journal_directory(root.join("journal"));
        let original_target = changes.current_target(&editor);
        let task = storage::new_document_identity();
        let prepared = proposal(&editor, &task);
        assert_eq!(
            changes.confirm_native_task(
                &mut editor,
                "native-recover",
                "Native confirmed",
                prepared
            )["result"]["status"],
            "committed"
        );
        drop(changes);
        let mut editor = Editor::from_stored_document(&before_file).unwrap();
        let before = editor.project.clone();
        let mut changes = ChangeService::new("remote-restart".into(), true)
            .with_journal_directory(root.join("journal"));
        let recovery = Recover {
            target: changes.current_target(&editor),
            expected_revision: 0,
            original_target: original_target.clone(),
            request_id: "native-recover".into(),
        };
        assert_eq!(
            changes.recover(&mut editor, recovery)["error"]["code"],
            "NATIVE_CONFIRMATION_REQUIRED"
        );
        assert_eq!(editor.project, before);
        assert!(editor.undo.is_empty());
        let mut empty = Editor::new(Project::default());
        let recovery = Recover {
            target: changes.current_target(&empty),
            expected_revision: 0,
            original_target,
            request_id: "native-recover".into(),
        };
        assert_eq!(
            changes.recover_document(&mut empty, recovery)["error"]["code"],
            "NATIVE_CONFIRMATION_REQUIRED"
        );
        assert!(empty.project.elements.is_empty());
        drop(changes);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(all(test, debug_assertions))]
mod native_figure_regression {
    use super::*;
    use crate::{
        command::ModelCommand,
        diagrams::{Definition, Domain, Edit, Presentation},
    };
    fn figure(editor: &Editor) -> std::rc::Rc<PreparedChange> {
        std::rc::Rc::new(
            editor
                .prepare_change(
                    editor.revision,
                    vec![ModelCommand::Diagram {
                        edit: Edit::Create {
                            name: "Local native figure".into(),
                            definition: Definition {
                                story: 0,
                                option: None,
                                domain: Domain::Site,
                                presentation: Presentation::SiteLocation,
                            },
                        },
                    }],
                )
                .unwrap(),
        )
    }
    #[test]
    fn no_task_native_figure_cannot_be_stolen_by_remote_commit_or_renew_grant() {
        let root = std::env::temp_dir().join(format!(
            "oa-native-figure-grant-{}",
            crate::storage::new_document_identity()
        ));
        std::fs::create_dir(&root).unwrap();
        let mut editor = Editor::new(crate::storage::synthetic_building_fixture());
        let mut changes = ChangeService::new("figure-native".into(), true)
            .with_journal_directory(root.join("journal"));
        changes.authorize_current(&editor);
        let target = changes.read.target(&editor);
        let before = editor.project.clone();
        let prepared = figure(&editor);
        let preview = changes.prepare_task_proposal_admitted(
            &editor,
            target.clone(),
            "",
            "Native figure",
            prepared,
            true,
        );
        let id = preview["result"]["preview_id"].as_str().unwrap().to_owned();
        let stolen = changes.commit(
            &mut editor,
            Commit {
                target: target.clone(),
                preview_id: id.clone(),
                request_id: "remote-steal".into(),
            },
        );
        assert_eq!(stolen["error"]["code"], "NATIVE_CONFIRMATION_REQUIRED");
        assert_eq!(editor.project, before);
        let result = changes.commit_admitted(
            &mut editor,
            Commit {
                target: target.clone(),
                preview_id: id,
                request_id: "native-original".into(),
            },
            true,
        );
        assert_eq!(result["result"]["status"], "committed");
        assert_eq!(changes.authorization_status(&editor)["status"], "suspended");
        let _fresh = changes.read.context(&editor, &target);
        assert_eq!(changes.authorization_status(&editor)["status"], "suspended");
        assert_eq!(editor.project.elements, before.elements);
        drop(changes);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn no_task_unknown_native_intent_blocks_replacement_and_user_edits_block_cancellation() {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        let root = std::env::temp_dir().join(format!(
            "oa-native-figure-unknown-{}",
            crate::storage::new_document_identity()
        ));
        std::fs::create_dir(&root).unwrap();
        let mut editor = Editor::new(crate::storage::synthetic_building_fixture());
        let mut changes = ChangeService::new("figure-native".into(), false)
            .with_journal_directory(root.join("journal"));
        let original = changes.read.target(&editor);
        let before = editor.project.clone();
        let prepared = figure(&editor);
        changes.set_durability_probe(|stage| {
            if stage == DurableStage::IntentStored {
                panic!("protected no-task figure failure injection");
            }
        });
        assert!(
            catch_unwind(AssertUnwindSafe(|| changes.confirm_native_task(
                &mut editor,
                "original-pending",
                "Native figure",
                prepared
            )))
            .is_err()
        );
        assert_eq!(editor.project, before);
        drop(changes);
        let mut changes = ChangeService::new("figure-restarted".into(), false)
            .with_journal_directory(root.join("journal"));
        let next = figure(&editor);
        let blocked = changes.confirm_native_task(
            &mut editor,
            "replacement-forbidden",
            "Native figure",
            next,
        );
        assert_eq!(blocked["error"]["code"], "TASK_OPERATION_PENDING");
        assert_eq!(
            blocked["error"]["operation_identity"]["request_id"],
            "original-pending"
        );
        let id = editor.project.elements[0].id;
        editor
            .apply(ModelCommand::MoveElements {
                ids: vec![id],
                offset: crate::model::Point::new(0.1, 0.),
            })
            .unwrap();
        let user = editor.project.clone();
        assert_eq!(
            changes.cancel_native_intent(&editor, original.clone(), "original-pending")["error"]["code"],
            "RECOVERY_CONFLICT"
        );
        assert_eq!(editor.project, user);
        editor.undo();
        let history = format!("{:?}{:?}", editor.undo, editor.redo);
        let cancelled = changes.cancel_native_intent(&editor, original.clone(), "original-pending");
        assert_eq!(cancelled["result"]["status"], "cancelled");
        assert_eq!(cancelled["result"]["historical_execution"], "unknown");
        assert_eq!(format!("{:?}{:?}", editor.undo, editor.redo), history);
        assert_eq!(editor.project, before);
        drop(changes);
        std::fs::remove_dir_all(root).unwrap();
    }
}
