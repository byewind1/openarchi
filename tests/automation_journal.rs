use atelier_bim::{
    automation_journal::{Journal, OperationIdentity, Phase, RecoveryMatch},
    command::ModelCommand,
    editor::Editor,
    model::{Point, Project},
};
use std::path::PathBuf;
fn path() -> PathBuf {
    // Wall-clock resolution is not a uniqueness guarantee for parallel tests.
    // Atomically claim a fresh directory; never reuse or delete another fixture.
    for _ in 0..16 {
        let p = std::env::temp_dir().join(format!(
            "oa-journal-{}-{}",
            std::process::id(),
            atelier_bim::storage::new_document_identity()
        ));
        match std::fs::create_dir(&p) {
            Ok(()) => return p.join("operations.json"),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("cannot create journal fixture: {e}"),
        }
    }
    panic!("cannot allocate a unique journal fixture directory")
}

fn identity() -> OperationIdentity {
    OperationIdentity {
        caller_id: "client".into(),
        project_id: "project-a".into(),
        document_session: "original-session".into(),
        request_id: "one".into(),
    }
}
fn models() -> (Project, Project) {
    let mut e = Editor::new(Project::default());
    let before = e.project.clone();
    let wall = e
        .wall_creation_candidates(vec![(Point::new(0., 0.), Point::new(6., 0.))])
        .unwrap()
        .remove(0);
    e.apply(ModelCommand::AddElement { element: wall }).unwrap();
    (before, e.project)
}
#[test]
fn legacy_hash_only_before_image_keeps_exact_reconciliation_without_inventing_restore_image() {
    let path = path();
    let (before, after) = models();
    let id = identity();
    let mut journal = Journal::open(path.clone()).unwrap();
    journal
        .begin(id.clone(), "legacy".into(), &before, &after)
        .unwrap();
    assert!(
        !serde_json::to_value(journal.records().unwrap())
            .unwrap()
            .to_string()
            .contains("mass_space_figure_sources")
    );
    drop(journal);
    let mut wire: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    wire["records"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("before_image");
    std::fs::write(&path, serde_json::to_vec(&wire).unwrap()).unwrap();
    let journal = Journal::open(path).unwrap();
    assert!(journal.get(&id).unwrap().unwrap().before_image.is_none());
    assert_eq!(
        journal.reconcile(&id, "project-a", &before).unwrap(),
        RecoveryMatch::BeforeImage
    );
    let mut changed = before;
    changed.name = "Later user decision".into();
    assert_eq!(
        journal.reconcile(&id, "project-a", &changed).unwrap(),
        RecoveryMatch::Conflict
    );
}
#[test]
fn durable_identity_conflicts_and_no_cross_project_recovery() {
    let path = path();
    let (before, after) = models();
    let id = identity();
    let mut j = Journal::open(path.clone()).unwrap();
    assert!(Journal::open(path.clone()).is_err());
    j.begin(id.clone(), "payload".into(), &before, &after)
        .unwrap();
    assert!(
        j.begin(id.clone(), "different".into(), &before, &after)
            .is_err()
    );
    assert_eq!(
        j.reconcile(&id, "project-a", &before).unwrap(),
        RecoveryMatch::BeforeImage
    );
    assert_eq!(
        j.reconcile(&id, "project-a", &after).unwrap(),
        RecoveryMatch::AfterImage
    );
    assert!(j.reconcile(&id, "project-b", &after).is_err());
    j.transition(
        &id,
        Phase::Committed,
        Some(serde_json::json!({"operation":"one"})),
    )
    .unwrap();
    drop(j);
    let mut j = Journal::open(path.clone()).unwrap();
    assert_eq!(j.get(&id).unwrap().unwrap().phase, Phase::Committed);
    assert!(j.transition(&id, Phase::Intent, None).is_err());
    j.transition(
        &id,
        Phase::Undone,
        Some(serde_json::json!({"operation":"one"})),
    )
    .unwrap();
    drop(j);
    assert_eq!(
        Journal::open(path.clone())
            .unwrap()
            .get(&id)
            .unwrap()
            .unwrap()
            .phase,
        Phase::Undone
    );
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
#[test]
fn persistence_failure_poison_prevents_stale_receipt_and_further_writes() {
    let path = path();
    let (before, after) = models();
    let id = identity();
    let mut j = Journal::open(path.clone()).unwrap();
    j.begin(id.clone(), "payload".into(), &before, &after)
        .unwrap();
    std::fs::rename(&path, path.with_extension("evidence")).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(
        j.transition(&id, Phase::Committed, Some(serde_json::json!({"ok":true})))
            .unwrap_err()
            .contains("JOURNAL_OUTCOME_UNKNOWN")
    );
    assert!(j.get(&id).is_err());
    assert!(j.begin(id, "payload".into(), &before, &after).is_err());
    drop(j);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
#[test]
#[ignore = "subprocess fixture, invoked by durable_crash_windows"]
fn journal_crash_child() {
    let path = PathBuf::from(std::env::var_os("OA_JOURNAL_TEST_PATH").unwrap());
    let stage = std::env::var("OA_JOURNAL_TEST_STAGE").unwrap();
    let (before, after) = models();
    let mut j = Journal::open(path).unwrap();
    let id = identity();
    j.begin(id.clone(), "payload".into(), &before, &after)
        .unwrap();
    if stage == "marker" {
        j.transition(
            &id,
            Phase::Committed,
            Some(serde_json::json!({"committed":true})),
        )
        .unwrap();
    }
    std::process::exit(17);
}
#[test]
fn durable_crash_windows_keep_intent_distinct_from_commit_marker() {
    for stage in ["intent", "marker"] {
        let path = path();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "journal_crash_child", "--ignored", "--nocapture"])
            .env("OA_JOURNAL_TEST_PATH", &path)
            .env("OA_JOURNAL_TEST_STAGE", stage)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(17));
        let j = Journal::open(path.clone()).unwrap();
        let r = j.get(&identity()).unwrap().unwrap();
        assert_eq!(
            r.phase,
            if stage == "marker" {
                Phase::Committed
            } else {
                Phase::Intent
            }
        );
        assert_eq!(r.receipt.is_some(), stage == "marker");
        drop(j);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}

#[test]
fn journal_rejects_identical_images_instead_of_claiming_after_image_evidence() {
    let path = path();
    let project = Project::default();
    let mut journal = Journal::open(path.clone()).unwrap();
    assert_eq!(
        journal
            .begin(identity(), "payload".into(), &project, &project)
            .unwrap_err(),
        "NO_CHANGE"
    );
    assert!(journal.get(&identity()).unwrap().is_none());
    assert!(!path.exists());
    drop(journal);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn canonical_intent_and_resolved_batch_fingerprints_detect_changed_ledger_payloads() {
    use atelier_bim::{automation_journal::canonical_hash, model_service::BatchChange};
    let path = path();
    let (before, after) = models();
    let intent = serde_json::json!({"units":"m","dimensions":{"height":3,"width":6}});
    let hash = canonical_hash(&intent).unwrap();
    assert_eq!(
        hash,
        canonical_hash(&serde_json::json!({"dimensions":{"width":6,"height":3},"units":"m"}))
            .unwrap()
    );
    let mut journal = Journal::open(path.clone()).unwrap();
    journal
        .begin_model(
            identity(),
            hash,
            &before,
            &after,
            Some(BatchChange::default()),
            None,
            None,
            Some(intent),
        )
        .unwrap();
    drop(journal);
    let original = std::fs::read(&path).unwrap();
    assert!(Journal::open(path.clone()).is_ok());
    for field in ["intent_payload", "batch"] {
        let mut value: serde_json::Value = serde_json::from_slice(&original).unwrap();
        let record = value["records"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap();
        if field == "intent_payload" {
            record[field]["dimensions"]["height"] = serde_json::json!(9);
        } else {
            record[field]["version"] = serde_json::json!(77);
        }
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(Journal::open(path.clone()),Err(e) if e == "JOURNAL_CORRUPT"));
    }
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
