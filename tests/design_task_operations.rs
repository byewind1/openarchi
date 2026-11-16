mod common;
use atelier_bim::{
    automation::ReadService,
    automation_change::{ChangeService, Commit, Operation},
    command::ModelCommand,
    design_process::{Entry, service},
    editor::Editor,
    model::{Point, Shape},
    storage,
};
use serde_json::{Value, json};

fn fixture() -> (std::path::PathBuf, Editor, String) {
    let root = std::env::temp_dir().join(format!("oa-task-{}", storage::new_document_identity()));
    std::fs::create_dir(&root).unwrap();
    let editor = Editor::new(common::synthetic_building_98());
    let task = storage::new_document_identity();
    (root, editor, task)
}
fn create(editor: &Editor, task: &str) -> atelier_bim::model_service::PreparedChange {
    service::prepare_create(
        editor,
        task,
        service::Draft {
            name: "Task checkpoint fixture".into(),
            question: "Keep both editable options across restart".into(),
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
    .unwrap()
}
fn admit(
    service: &mut ChangeService,
    editor: &Editor,
    task: &str,
    proposal: atelier_bim::model_service::PreparedChange,
) -> String {
    let target = ReadService::new("first".into()).target(editor);
    let value = service.prepare_task_proposal(
        editor,
        target,
        task,
        "Two editable space candidates",
        proposal,
    );
    value["result"]["preview_id"]
        .as_str()
        .unwrap_or_else(|| panic!("{value}"))
        .into()
}

#[test]
fn task_checkpoint_child_readback() {
    let Some(root) = std::env::var_os("OA_TASK_CHECKPOINT_CHILD") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    let editor = Editor::from_stored_document(&root.join("saved.json")).unwrap();
    // Reading after restart does not need write permission or grant one.
    let mut service =
        ChangeService::new("second".into(), false).with_journal_directory(root.join("journal"));
    let checkpoint = service.task_checkpoint(&editor);
    assert_eq!(checkpoint["total"], 1);
    assert_eq!(checkpoint["operations"][0]["phase"], "committed");
    assert_eq!(checkpoint["operations"][0]["current_effect"], "AfterImage");
    let old = &checkpoint["operations"][0];
    let operation = service.operation(
        &editor,
        Operation {
            target: ReadService::new("second".into()).target(&editor),
            original_target: Some(serde_json::from_value(old["original_target"].clone()).unwrap()),
            request_id: old["operation_identity"]["request_id"]
                .as_str()
                .unwrap()
                .into(),
        },
    );
    assert_eq!(operation["result"]["status"], "committed");
    assert_eq!(
        operation["result"]["operation_identity"],
        old["operation_identity"]
    );
    assert_eq!(
        service.write_authorization(&editor).unwrap_err()["error"]["code"],
        "WRITE_NOT_AUTHORIZED"
    );
    assert_eq!(editor.revision, 0);
    assert!(editor.undo.is_empty());
    println!(
        "CHILD_CHECKPOINT={}",
        json!({"checkpoint":checkpoint,"original_lookup":operation})
    );
}

#[test]
fn real_task_proposal_uses_original_journal_and_new_process_readback() {
    let (root, mut editor, task) = fixture();
    let before = editor.project.clone();
    let proposal = create(&editor, &task);
    let mut service =
        ChangeService::new("first".into(), true).with_journal_directory(root.join("journal"));
    let preview = admit(&mut service, &editor, &task, proposal);
    assert_eq!(editor.project, before);
    let target = ReadService::new("first".into()).target(&editor);
    let request = storage::new_document_identity();
    let commit = Commit {
        target: target.clone(),
        preview_id: preview.clone(),
        request_id: request.clone(),
    };
    let receipt = service.commit(&mut editor, commit);
    assert_eq!(receipt["result"]["status"], "committed", "{receipt}");
    assert_eq!(editor.project.elements.len(), before.elements.len() + 18);
    let after = editor.project.clone();
    let again = service.commit(
        &mut editor,
        Commit {
            target: target.clone(),
            preview_id: preview,
            request_id: request.clone(),
        },
    );
    assert_eq!(again["result"]["status"], "committed");
    assert_eq!(editor.project, after);
    assert_eq!(editor.undo.len(), 1);
    let checkpoint = service.task_checkpoint(&editor);
    assert_eq!(
        checkpoint["operations"][0]["operation_identity"],
        receipt["result"]["operation_identity"]
    );
    storage::save_document(
        &editor.project,
        &root.join("saved.json"),
        editor.project_identity(),
    )
    .unwrap();
    let journal: Value = serde_json::from_slice(
        &std::fs::read(
            root.join("journal")
                .join(format!("{}.json", editor.project_identity())),
        )
        .unwrap(),
    )
    .unwrap();
    let record = journal["records"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap();
    assert!(record["batch"].is_null());
    assert_eq!(record["intent_payload"]["task_id"], task);
    assert_eq!(
        record["intent_payload"]["before_hash"],
        record["before_hash"]
    );
    assert_eq!(record["intent_payload"]["after_hash"], record["after_hash"]);
    assert_eq!(
        record["payload_hash"],
        atelier_bim::automation_journal::canonical_hash(&record["intent_payload"]).unwrap()
    );
    drop(service);
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "task_checkpoint_child_readback", "--nocapture"])
        .env("OA_TASK_CHECKPOINT_CHILD", &root)
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
    assert!(String::from_utf8_lossy(&child.stdout).contains("CHILD_CHECKPOINT="));
    // A restarted caller may inspect the old identity but cannot resend commit.
    let mut restarted =
        ChangeService::new("first".into(), true).with_journal_directory(root.join("journal"));
    let denied = restarted.commit(
        &mut editor,
        Commit {
            target,
            preview_id: "lost-original-preview".into(),
            request_id: request,
        },
    );
    assert_eq!(denied["error"]["code"], "RECOVERY_REQUIRED");
    assert_eq!(editor.project, after);
    drop(restarted);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn task_readback_keeps_committed_outcome_but_reports_manual_conflict_without_regrant() {
    let (root, mut editor, task) = fixture();
    let proposal = create(&editor, &task);
    let mut service =
        ChangeService::new("first".into(), true).with_journal_directory(root.join("journal"));
    let preview = admit(&mut service, &editor, &task, proposal);
    let target = ReadService::new("first".into()).target(&editor);
    assert_eq!(
        service.commit(
            &mut editor,
            Commit {
                target: target.clone(),
                preview_id: preview,
                request_id: "owned".into()
            }
        )["result"]["status"],
        "committed"
    );
    let zone = editor.project.design_process.task().unwrap().candidates[0].initial_zone_ids[0];
    let pending = service::prepare_edit(&editor, zone, Point::new(0.1, 0.), None).unwrap();
    let pending = admit(&mut service, &editor, &task, pending);
    editor
        .apply(ModelCommand::MoveElements {
            ids: vec![zone],
            offset: Point::new(0., 0.2),
        })
        .unwrap();
    let manual = editor.project.clone();
    let history = format!("{:?}", editor.undo);
    for _ in 0..2 {
        let cp = service.task_checkpoint(&editor);
        assert_eq!(cp["total"], 1);
        assert_eq!(cp["operations"][0]["phase"], "committed");
        assert_eq!(cp["operations"][0]["current_effect"], "Conflict");
    }
    let stale = service.commit(
        &mut editor,
        Commit {
            target: target.clone(),
            preview_id: pending,
            request_id: "late".into(),
        },
    );
    assert_eq!(stale["error"]["code"], "REVISION_CONFLICT");
    let proposal = service::prepare_edit(&editor, zone, Point::new(0., -0.2), None).unwrap();
    let denied =
        service.prepare_task_proposal(&editor, target, &task, "Do not repair user edit", proposal);
    assert_eq!(denied["error"]["code"], "WRITE_AUTHORIZATION_SUSPENDED");
    assert_eq!(editor.project, manual);
    assert_eq!(format!("{:?}", editor.undo), history);
    drop(service);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn task_admission_rejects_missing_durability_and_mismatched_task_without_effect() {
    let (root, editor, task) = fixture();
    let before = editor.project.clone();
    let target = ReadService::new("first".into()).target(&editor);
    let mut service = ChangeService::new("first".into(), true);
    let missing = service.prepare_task_proposal(
        &editor,
        target.clone(),
        &task,
        "Need durable result",
        create(&editor, &task),
    );
    assert_eq!(missing["error"]["code"], "DURABILITY_NOT_CONFIGURED");
    let mismatch = service.prepare_task_proposal(
        &editor,
        target.clone(),
        &storage::new_document_identity(),
        "Wrong task",
        create(&editor, &task),
    );
    assert_eq!(mismatch["error"]["code"], "DESIGN_TASK_MISMATCH");
    std::fs::write(root.join("blocked"), b"not a directory").unwrap();
    let mut service =
        ChangeService::new("first".into(), true).with_journal_directory(root.join("blocked"));
    let unavailable = service.prepare_task_proposal(
        &editor,
        target,
        &task,
        "Must fail closed",
        create(&editor, &task),
    );
    assert_eq!(unavailable["error"]["code"], "JOURNAL_UNAVAILABLE");
    assert_eq!(editor.project, before);
    assert!(editor.undo.is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn task_creation_safe_undo_and_recovery_preserve_immutable_history() {
    let (root, mut editor, task) = fixture();
    let before = editor.project.clone();
    let proposal = create(&editor, &task);
    let mut changes =
        ChangeService::new("first".into(), true).with_journal_directory(root.join("journal"));
    let preview = admit(&mut changes, &editor, &task, proposal);
    let target = ReadService::new("first".into()).target(&editor);
    assert_eq!(
        changes.commit(
            &mut editor,
            Commit {
                target: target.clone(),
                preview_id: preview,
                request_id: "create-task".into()
            }
        )["result"]["status"],
        "committed"
    );
    let history = editor.project.design_process.tasks.clone();
    assert_eq!(
        changes.undo(
            &mut editor,
            Operation {
                target: target.clone(),
                request_id: "create-task".into(),
                original_target: None
            }
        )["result"]["status"],
        "undone"
    );
    assert_eq!(editor.project.elements, before.elements);
    assert_eq!(editor.project.design_process.tasks, history);
    assert!(editor.project.design_process.current_task.is_none());
    let saved = root.join("undone.json");
    storage::save_document(&editor.project, &saved, editor.project_identity()).unwrap();
    drop(changes);
    let mut reopened = Editor::from_stored_document(&saved).unwrap();
    let mut changes =
        ChangeService::new("second".into(), false).with_journal_directory(root.join("journal"));
    let current = ReadService::new("second".into()).target(&reopened);
    let lookup = changes.operation(
        &reopened,
        Operation {
            target: current.clone(),
            request_id: "create-task".into(),
            original_target: Some(target.clone()),
        },
    );
    assert_eq!(lookup["result"]["status"], "undone");
    assert_eq!(lookup["result"]["current_effect"], "BeforeImage");
    assert_eq!(lookup["result"]["recovery_required"], false);
    // Reconciliation remains read-only with permission disabled.
    assert!(
        changes.recover(
            &mut reopened,
            atelier_bim::automation_change::Recover {
                target: current,
                expected_revision: 0,
                original_target: target.clone(),
                request_id: "create-task".into()
            }
        )["error"]
            .is_object()
    );
    changes.authorize_current(&reopened);
    let current = ReadService::new("second".into()).target(&reopened);
    let recovered = changes.recover(
        &mut reopened,
        atelier_bim::automation_change::Recover {
            target: current,
            expected_revision: 0,
            original_target: target,
            request_id: "create-task".into(),
        },
    );
    assert_eq!(recovered["result"]["status"], "undone");
    assert!(recovered["result"]["restoration_receipt"].is_null());
    assert_eq!(reopened.project.design_process.tasks, history);
    assert!(reopened.undo.is_empty());
    drop(changes);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[cfg(debug_assertions)]
fn unknown_task_intent_requires_original_identity_instead_of_new_request() {
    let (root, mut editor, task) = fixture();
    let before = editor.project.clone();
    let proposal = create(&editor, &task);
    let mut changes =
        ChangeService::new("first".into(), true).with_journal_directory(root.join("journal"));
    let preview = admit(&mut changes, &editor, &task, proposal);
    let target = ReadService::new("first".into()).target(&editor);
    changes.set_durability_probe(|stage| {
        if stage == atelier_bim::automation_change::DurableStage::IntentStored {
            panic!("expected protected-copy fault after durable intent");
        }
    });
    let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        changes.commit(
            &mut editor,
            Commit {
                target,
                preview_id: preview,
                request_id: "original-unknown".into(),
            },
        )
    }));
    assert!(stopped.is_err());
    assert_eq!(editor.project, before);
    drop(changes);
    let mut changes =
        ChangeService::new("second".into(), true).with_journal_directory(root.join("journal"));
    let target = ReadService::new("second".into()).target(&editor);
    let replaced = changes.prepare_task_proposal(
        &editor,
        target,
        &task,
        "Replacement must be refused",
        create(&editor, &task),
    );
    assert_eq!(replaced["error"]["code"], "TASK_OPERATION_PENDING");
    assert_eq!(
        replaced["error"]["operation_identity"]["request_id"],
        "original-unknown"
    );
    assert_eq!(editor.project, before);
    assert!(editor.undo.is_empty());
    drop(changes);
    std::fs::remove_dir_all(root).unwrap();
}
