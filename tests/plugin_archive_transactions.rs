//! Shared transaction/history boundaries; explicit library authorization is not a
//! claim about every GUI/HTTP permission mode. Normal product events live in app tests.
use atelier_bim::{
    application_host::ApplicationHost,
    automation::{ReadService, Target},
    automation_change::{ChangeService, Commit, Operation, Prepare, Recover},
    automation_journal::{Journal, OperationIdentity, project_hash},
    command::ModelCommand,
    concept_mass::ConceptMassSpec,
    editor::Editor,
    model::{Material, Point, Project, Shape},
    model_service::{BatchChange, ElementTemplate, NamedElement},
    plugins::{archive::Archive, jobs::Bundle},
    storage,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
fn fixture() -> (PathBuf, ApplicationHost) {
    let root = std::env::temp_dir().join(format!(
        "oa-history-txn-{}",
        storage::new_document_identity()
    ));
    std::fs::create_dir(&root).unwrap();
    let mut p = Project::default();
    p.add(
        0,
        Material::Timber,
        Shape::ConceptMass {
            spec: ConceptMassSpec {
                arcs: vec![],
                hole_arcs: vec![],
                outline: vec![
                    Point::new(0., 0.),
                    Point::new(4., 0.),
                    Point::new(4., 6.),
                    Point::new(0., 6.),
                ],
                holes: vec![],
                height: 3.,
                bottom_offset: 0.,
            },
        },
    );
    let mut h = ApplicationHost::new(p);
    h.configure_plugins(&root.join("profile")).unwrap();
    (root, h)
}
fn review(h: &mut ApplicationHost) -> Bundle {
    let id = storage::new_document_identity();
    h.start_plugin_job("org.openarchi.quantities.report", &id)
        .unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        h.jobs.as_mut().unwrap().poll();
        let b = h
            .jobs
            .as_ref()
            .unwrap()
            .query(h.document.editor.project_identity(), &id)
            .unwrap();
        if b.result.is_some() {
            return b;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn append_and_save(root: &std::path::Path, h: &mut ApplicationHost) -> PathBuf {
    review(h);
    let snapshot = h.project_snapshot_for_save().collect().unwrap();
    let p = root.join(format!("saved-{}.json", storage::new_document_identity()));
    storage::save_document(&snapshot, &p, h.document.editor.project_identity()).unwrap();
    h.document.editor.project.job_archive = snapshot.job_archive;
    h.document.editor.mark_saved();
    p
}
fn batch(h: &ApplicationHost) -> BatchChange {
    let mut element = h.document.editor.project.elements[0].clone();
    element.id = 0;
    element.name = "New committed mass".into();
    BatchChange {
        creates: vec![NamedElement {
            name: "additional".into(),
            template: ElementTemplate {
                element,
                host: None,
            },
        }],
        ..Default::default()
    }
}
fn committed(s: &mut ChangeService, h: &mut ApplicationHost, t: &Target) -> String {
    let p = s.prepare(
        &h.document.editor,
        Prepare {
            target: t.clone(),
            expected_revision: h.document.editor.revision,
            change: batch(h),
        },
    );
    let request = storage::new_document_identity();
    let result = s.commit(
        &mut h.document.editor,
        Commit {
            target: t.clone(),
            preview_id: p["result"]["preview_id"].as_str().unwrap().into(),
            request_id: request.clone(),
        },
    );
    assert_eq!(result["result"]["status"], "committed", "{result}");
    request
}
#[test]
fn prepared_model_commit_keeps_new_history_and_user_edits_still_conflict() {
    let (root, mut h) = fixture();
    let initial = h.document.editor.project.clone();
    let id = initial.elements[0].id;
    let prepared = h
        .document
        .editor
        .prepare_change(
            0,
            vec![ModelCommand::MoveElements {
                ids: vec![id],
                offset: Point::new(0.25, 0.),
            }],
        )
        .unwrap();
    append_and_save(&root, &mut h);
    let history = h.document.editor.project.job_archive.clone();
    assert!(!history.is_empty());
    h.document
        .editor
        .commit_change("prepared-before-review", &prepared)
        .unwrap();
    assert_eq!(h.document.editor.project.job_archive, history);
    h.document.editor.undo();
    assert_eq!(h.document.editor.project.elements, initial.elements);
    assert_eq!(h.document.editor.project.job_archive, history);
    let prepared = h
        .document
        .editor
        .prepare_change(
            h.document.editor.revision,
            vec![ModelCommand::MoveElements {
                ids: vec![id],
                offset: Point::new(0.1, 0.),
            }],
        )
        .unwrap();
    h.document
        .editor
        .apply(ModelCommand::MoveElements {
            ids: vec![id],
            offset: Point::new(0., 0.03),
        })
        .unwrap();
    let user = h.document.editor.project.clone();
    assert_eq!(
        h.document
            .editor
            .commit_change("not-reauthed", &prepared)
            .unwrap_err()
            .code,
        "REVISION_CONFLICT"
    );
    assert_eq!(h.document.editor.project, user);
}
#[test]
fn durable_original_effect_survives_save_reopen_and_actual_safe_undo_preserves_archive() {
    let (root, mut h) = fixture();
    let before = h.document.editor.project.clone();
    let target = ReadService::new("first".into()).target(&h.document.editor);
    let mut service =
        ChangeService::new("first".into(), true).with_journal_directory(root.join("journal"));
    let request = committed(&mut service, &mut h, &target);
    let saved = append_and_save(&root, &mut h);
    let lookup = service.operation(
        &h.document.editor,
        Operation {
            target: target.clone(),
            request_id: request.clone(),
            original_target: None,
        },
    );
    assert_eq!(lookup["result"]["current_effect"], "AfterImage");
    let archive = h.document.editor.project.job_archive.clone();
    let undo = service.undo(
        &mut h.document.editor,
        Operation {
            target: target.clone(),
            request_id: request.clone(),
            original_target: None,
        },
    );
    assert_eq!(undo["result"]["status"], "undone", "{undo}");
    assert_eq!(h.document.editor.project.elements, before.elements);
    assert_eq!(h.document.editor.project.job_archive, archive);
    drop(service);
    let e = Editor::from_stored_document(&saved).unwrap();
    let current = ReadService::new("second".into()).target(&e);
    let mut service =
        ChangeService::new("second".into(), false).with_journal_directory(root.join("journal"));
    let lookup = service.operation(
        &e,
        Operation {
            target: current.clone(),
            request_id: request.clone(),
            original_target: Some(target.clone()),
        },
    );
    assert_eq!(lookup["result"]["current_effect"], "AfterImage");
    let mut e = e;
    let denied = service.recover(
        &mut e,
        Recover {
            target: current,
            expected_revision: 0,
            original_target: target,
            request_id: request,
        },
    );
    assert_eq!(denied["error"]["code"], "WRITE_NOT_AUTHORIZED");
    assert_eq!(e.project.job_archive, archive);
}
#[test]
fn readonly_append_never_renews_grant_and_true_user_geometry_blocks_safe_undo() {
    let (root, mut h) = fixture();
    let target = ReadService::new("owner".into()).target(&h.document.editor);
    let mut s =
        ChangeService::new("owner".into(), true).with_journal_directory(root.join("journal"));
    let req = committed(&mut s, &mut h, &target);
    append_and_save(&root, &mut h);
    h.document
        .editor
        .apply(ModelCommand::MoveElements {
            ids: vec![1],
            offset: Point::new(0., 0.03),
        })
        .unwrap();
    let user = h.document.editor.project.clone();
    let _ = s.operation(
        &h.document.editor,
        Operation {
            target: target.clone(),
            request_id: req.clone(),
            original_target: None,
        },
    );
    let proposed = s.prepare(
        &h.document.editor,
        Prepare {
            target: target.clone(),
            expected_revision: h.document.editor.revision,
            change: batch(&h),
        },
    );
    assert_eq!(proposed["error"]["code"], "WRITE_AUTHORIZATION_SUSPENDED");
    let undo = s.undo(
        &mut h.document.editor,
        Operation {
            target,
            request_id: req,
            original_target: None,
        },
    );
    assert_eq!(undo["error"]["code"], "UNDO_CONFLICT");
    assert_eq!(h.document.editor.project, user);
}
#[test]
fn integrity_hash_still_covers_history_and_corrupt_log_is_never_rehashed() {
    let (root, mut h) = fixture();
    append_and_save(&root, &mut h);
    let before = h.document.editor.project.clone();
    let mut without = before.clone();
    without.job_archive = Archive::default();
    assert!(before.same_model_state(&without));
    assert_ne!(
        project_hash(&before).unwrap(),
        project_hash(&without).unwrap()
    );
    h.document
        .editor
        .apply(ModelCommand::MoveElements {
            ids: vec![1],
            offset: Point::new(0.25, 0.),
        })
        .unwrap();
    let path = root.join("integrity.json");
    let mut j = Journal::open(path.clone()).unwrap();
    j.begin(
        OperationIdentity {
            caller_id: "fixture".into(),
            project_id: h.document.editor.project_identity().into(),
            document_session: "fixture:1".into(),
            request_id: "immutable".into(),
        },
        "payload".into(),
        &before,
        &h.document.editor.project,
    )
    .unwrap();
    drop(j);
    let mut raw: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let records = raw["records"].as_object_mut().unwrap();
    let record = records.values_mut().next().unwrap();
    record["after_image"]["job_archive"]["records"][0]["bundle"]["cancel_requested"] = true.into();
    let changed = serde_json::to_vec(&raw).unwrap();
    std::fs::write(&path, &changed).unwrap();
    assert!(Journal::open(path.clone()).is_err());
    assert_eq!(std::fs::read(path).unwrap(), changed);
}

#[test]
fn reopened_original_query_and_recovery_do_not_replay_model_or_lose_new_history() {
    let (root, mut h) = fixture();
    let target = ReadService::new("first".into()).target(&h.document.editor);
    let mut service =
        ChangeService::new("first".into(), true).with_journal_directory(root.join("journal"));
    let request = committed(&mut service, &mut h, &target);
    let path = append_and_save(&root, &mut h);
    let saved_bytes = std::fs::read(&path).unwrap();
    let expected = h.document.editor.project.clone();
    drop(service);
    drop(h);
    let mut e = Editor::from_stored_document(&path).unwrap();
    let current = ReadService::new("second".into()).target(&e);
    let mut service =
        ChangeService::new("second".into(), true).with_journal_directory(root.join("journal"));
    let result = service.recover(
        &mut e,
        Recover {
            target: current.clone(),
            expected_revision: 0,
            original_target: target.clone(),
            request_id: request.clone(),
        },
    );
    assert_eq!(result["result"]["status"], "committed", "{result}");
    assert!(result["result"]["restoration_receipt"].is_null());
    assert_eq!(result["result"]["history_changed"], false);
    assert_eq!(e.project, expected);
    assert_eq!(e.revision, 0);
    assert!(e.undo.is_empty() && e.redo.is_empty() && !e.dirty());
    assert_eq!(std::fs::read(path).unwrap(), saved_bytes);
    let original = service.operation(
        &e,
        Operation {
            target: current,
            original_target: Some(target),
            request_id: request,
        },
    );
    assert_eq!(original["result"]["current_effect"], "AfterImage");
}

#[test]
fn recovery_document_refuses_unbound_document_with_readonly_history() {
    let (root, mut h) = fixture();
    let original = ReadService::new("original".into()).target(&h.document.editor);
    review(&mut h);
    h.document.editor.project.job_archive =
        h.project_snapshot_for_save().collect().unwrap().job_archive;
    // Isolate the archive guard from the independent dirty/file/history guards.
    h.document.editor.mark_saved();
    assert!(!h.document.editor.dirty());
    assert!(h.document.editor.file_generation().is_none());
    let before = h.document.editor.project.clone();
    let target = ReadService::new("next".into()).target(&h.document.editor);
    let mut service =
        ChangeService::new("next".into(), true).with_journal_directory(root.join("journal"));
    let result = service.recover_document(
        &mut h.document.editor,
        Recover {
            target,
            expected_revision: 0,
            original_target: original,
            request_id: "not-replayed".into(),
        },
    );
    assert_eq!(result["error"]["code"], "DOCUMENT_ACTIVE");
    assert_eq!(h.document.editor.project, before);
}

#[test]
fn archive_merge_preserves_union_and_refuses_two_terminal_results_for_original_request() {
    use atelier_bim::plugins::{archive::Record, jobs::Terminal};
    let (_root, mut h) = fixture();
    let one = review(&mut h);
    let two = review(&mut h);
    let first = Archive::new(vec![Record {
        bundle: one.clone(),
        process: None,
    }])
    .unwrap();
    let second = Archive::new(vec![Record {
        bundle: two.clone(),
        process: None,
    }])
    .unwrap();
    let merged = first.merge(&second).unwrap();
    assert_eq!(merged.records().len(), 2);
    assert_eq!(merged, second.merge(&first).unwrap());
    let mut conflicting = one.clone();
    conflicting.result.as_mut().unwrap().terminal = Terminal::Failed {
        message: "different terminal observation".into(),
    };
    let conflicting = Archive::new(vec![Record {
        bundle: conflicting,
        process: None,
    }])
    .unwrap();
    assert!(first.merge(&conflicting).is_err());
    assert_eq!(first.records()[0].bundle, one);
    assert_eq!(second.records()[0].bundle, two);
}
