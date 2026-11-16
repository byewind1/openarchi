use atelier_bim::{
    automation::{ReadService, Target},
    automation_change::{Cancel, ChangeService, Commit, Operation, Prepare},
    command::ModelCommand,
    editor::Editor,
    model::{Point, Project},
    model_service::{BatchChange, ElementTemplate, NamedElement},
};
fn batch_for_current(editor: &Editor) -> BatchChange {
    let mut wall = editor
        .wall_creation_candidates(vec![(Point::new(10., 0.), Point::new(16., 0.))])
        .unwrap()
        .remove(0);
    wall.id = 0;
    BatchChange {
        creates: vec![NamedElement {
            name: "later-wall".into(),
            template: ElementTemplate {
                element: wall,
                host: None,
            },
        }],
        ..Default::default()
    }
}
fn setup() -> (Editor, ChangeService, Target, BatchChange) {
    let editor = Editor::new(Project::default());
    let target = ReadService::new("a".into()).target(&editor);
    let mut wall = editor
        .wall_creation_candidates(vec![(Point::new(0., 0.), Point::new(6., 0.))])
        .unwrap()
        .remove(0);
    wall.id = 0;
    let batch = BatchChange {
        creates: vec![NamedElement {
            name: "wall".into(),
            template: ElementTemplate {
                element: wall,
                host: None,
            },
        }],
        ..Default::default()
    };
    (editor, ChangeService::new("a".into(), true), target, batch)
}
#[test]
fn preview_commit_lost_response_recovery_retry_and_safe_undo() {
    let (mut editor, mut service, target, batch) = setup();
    let before = editor.project.clone();
    let preview = service.prepare(
        &editor,
        Prepare {
            target: target.clone(),
            expected_revision: 0,
            change: batch,
        },
    );
    assert_eq!(editor.project, before);
    assert!(editor.undo.is_empty());
    let id = preview["result"]["preview_id"].as_str().unwrap().to_owned();
    let receipt = service.commit(
        &mut editor,
        Commit {
            target: target.clone(),
            preview_id: id.clone(),
            request_id: "one".into(),
        },
    );
    assert_eq!(receipt["result"]["status"], "committed");
    assert_eq!(editor.project.elements.len(), 1);
    assert_eq!(editor.undo.len(), 1);
    let recovered = service.operation(
        &editor,
        Operation {
            target: target.clone(),
            request_id: "one".into(),
            original_target: None,
        },
    );
    assert_eq!(receipt["result"]["receipt"], recovered["result"]["receipt"]);
    service.commit(
        &mut editor,
        Commit {
            target: target.clone(),
            preview_id: id.clone(),
            request_id: "one".into(),
        },
    );
    assert_eq!(editor.project.elements.len(), 1);
    let result = service.undo(
        &mut editor,
        Operation {
            target: target.clone(),
            request_id: "one".into(),
            original_target: None,
        },
    );
    assert_eq!(result["result"]["status"], "undone");
    assert_eq!(editor.project, before);
    service.commit(
        &mut editor,
        Commit {
            target,
            preview_id: id,
            request_id: "one".into(),
        },
    );
    assert_eq!(editor.project, before);
}
#[test]
fn rejects_stale_preview_cancel_and_undo_after_user_edit() {
    let (mut editor, mut service, target, batch) = setup();
    let preview = service.prepare(
        &editor,
        Prepare {
            target: target.clone(),
            expected_revision: 0,
            change: batch.clone(),
        },
    );
    let id = preview["result"]["preview_id"].as_str().unwrap().to_owned();
    service.commit(
        &mut editor,
        Commit {
            target: target.clone(),
            preview_id: id,
            request_id: "one".into(),
        },
    );
    editor
        .apply(ModelCommand::MoveElements {
            ids: vec![editor.project.elements[0].id],
            offset: Point::new(1., 0.),
        })
        .unwrap();
    let after = editor.project.clone();
    let result = service.undo(
        &mut editor,
        Operation {
            target: target.clone(),
            request_id: "one".into(),
            original_target: None,
        },
    );
    assert_eq!(result["error"]["code"], "UNDO_CONFLICT");
    assert_eq!(after, editor.project);
    let preview = service.prepare(
        &editor,
        Prepare {
            target: target.clone(),
            expected_revision: editor.revision,
            change: batch,
        },
    );
    assert_eq!(preview["error"]["code"], "WRITE_AUTHORIZATION_SUSPENDED");
    assert_eq!(editor.project, after);
    // A deliberate trusted/native grant, not a refreshed read, is needed for subsequent work.
    service.authorize_current(&editor);
    let preview = service.prepare(
        &editor,
        Prepare {
            target: target.clone(),
            expected_revision: editor.revision,
            change: batch_for_current(&editor),
        },
    );
    let id = preview["result"]["preview_id"].as_str().unwrap().to_owned();
    assert_eq!(
        service.cancel(
            &editor,
            Cancel {
                target: target.clone(),
                preview_id: id.clone()
            }
        )["result"]["status"],
        "cancelled"
    );
    assert_eq!(
        service.commit(
            &mut editor,
            Commit {
                target,
                preview_id: id,
                request_id: "two".into()
            }
        )["error"]["code"],
        "PREVIEW_NOT_FOUND"
    );
}
#[test]
fn readonly_and_foreign_targets_reject_writes_without_mutation() {
    let (editor, _, target, batch) = setup();
    let before = editor.project.clone();
    let mut service = ChangeService::new("a".into(), false);
    assert_eq!(
        service.prepare(
            &editor,
            Prepare {
                target: target.clone(),
                expected_revision: 0,
                change: batch.clone()
            }
        )["error"]["code"],
        "WRITE_NOT_AUTHORIZED"
    );
    let mut service = ChangeService::new("b".into(), true);
    assert_eq!(
        service.prepare(
            &editor,
            Prepare {
                target,
                expected_revision: 0,
                change: batch
            }
        )["error"]["code"],
        "TARGET_MISMATCH"
    );
    assert_eq!(editor.project, before);
}

#[test]
fn same_request_name_in_new_document_session_is_not_an_old_operation() {
    let (mut editor, mut service, target, batch) = setup();
    let preview = service.prepare(
        &editor,
        Prepare {
            target: target.clone(),
            expected_revision: 0,
            change: batch.clone(),
        },
    );
    service.commit(
        &mut editor,
        Commit {
            target,
            preview_id: preview["result"]["preview_id"].as_str().unwrap().into(),
            request_id: "reusable-name".into(),
        },
    );
    let mut other = Editor::new(Project::default());
    let target = ReadService::new("a".into()).target(&other);
    assert!(service.write_authorization(&other).is_err());
    service.authorize_current(&other);
    let preview = service.prepare(
        &other,
        Prepare {
            target: target.clone(),
            expected_revision: 0,
            change: batch,
        },
    );
    let receipt = service.commit(
        &mut other,
        Commit {
            target,
            preview_id: preview["result"]["preview_id"].as_str().unwrap().into(),
            request_id: "reusable-name".into(),
        },
    );
    assert_eq!(receipt["result"]["status"], "committed");
    assert_eq!(other.project.elements.len(), 1);
}

#[test]
fn equivalent_previews_share_one_request_identity_and_receipt() {
    let (mut editor, mut service, target, batch) = setup();
    let first = service.prepare(
        &editor,
        Prepare {
            target: target.clone(),
            expected_revision: 0,
            change: batch.clone(),
        },
    );
    let second = service.prepare(
        &editor,
        Prepare {
            target: target.clone(),
            expected_revision: 0,
            change: batch,
        },
    );
    let first = service.commit(
        &mut editor,
        Commit {
            target: target.clone(),
            preview_id: first["result"]["preview_id"].as_str().unwrap().into(),
            request_id: "one".into(),
        },
    );
    let second = service.commit(
        &mut editor,
        Commit {
            target,
            preview_id: second["result"]["preview_id"].as_str().unwrap().into(),
            request_id: "one".into(),
        },
    );
    assert_eq!(first["result"]["receipt"], second["result"]["receipt"]);
    assert_eq!(editor.project.elements.len(), 1);
    assert_eq!(editor.undo.len(), 1);
}

#[test]
fn preview_identity_cannot_alias_another_instance_even_after_retargeting() {
    let (mut editor, mut a, target_a, batch) = setup();
    let mut b = ChangeService::new("other-instance".into(), true);
    let target_b = ReadService::new("other-instance".into()).target(&editor);
    let pa = a.prepare(
        &editor,
        Prepare {
            target: target_a,
            expected_revision: 0,
            change: batch.clone(),
        },
    );
    let pb = b.prepare(
        &editor,
        Prepare {
            target: target_b.clone(),
            expected_revision: 0,
            change: batch,
        },
    );
    assert_ne!(pa["result"]["preview_id"], pb["result"]["preview_id"]);
    let before = editor.project.clone();
    let rejected = b.commit(
        &mut editor,
        Commit {
            target: target_b,
            preview_id: pa["result"]["preview_id"].as_str().unwrap().into(),
            request_id: "mistaken-retarget".into(),
        },
    );
    assert_eq!(rejected["error"]["code"], "PREVIEW_NOT_FOUND");
    assert_eq!(editor.project, before);
    assert_eq!(editor.revision, 0);
    assert!(editor.undo.is_empty());
}

#[test]
fn owned_write_sequence_advances_authority_but_reads_and_reprepare_after_manual_edit_do_not() {
    let (mut editor, mut service, target, batch) = setup();
    for request in ["owned-first", "owned-second"] {
        let preview = service.prepare(
            &editor,
            Prepare {
                target: target.clone(),
                expected_revision: editor.revision,
                change: batch.clone(),
            },
        );
        assert!(preview.get("result").is_some(), "{preview}");
        let result = service.commit(
            &mut editor,
            Commit {
                target: target.clone(),
                preview_id: preview["result"]["preview_id"].as_str().unwrap().into(),
                request_id: request.into(),
            },
        );
        assert_eq!(result["result"]["status"], "committed");
        assert!(service.write_authorization(&editor).is_ok());
    }
    let preview = service.prepare(
        &editor,
        Prepare {
            target: target.clone(),
            expected_revision: editor.revision,
            change: batch.clone(),
        },
    );
    let preview_id = preview["result"]["preview_id"]
        .as_str()
        .unwrap()
        .to_string();
    let id = editor.project.elements[0].id;
    editor
        .apply(ModelCommand::MoveElements {
            ids: vec![id],
            offset: Point::new(0.25, 0.),
        })
        .unwrap();
    let inserted = editor.project.clone();
    let history = editor.undo.len();
    let read = ReadService::new("a".into());
    for _ in 0..2 {
        let context = read.context(&editor, &target).unwrap();
        assert_eq!(context["revision"], editor.revision);
        let denied = service.prepare(
            &editor,
            Prepare {
                target: target.clone(),
                expected_revision: editor.revision,
                change: batch.clone(),
            },
        );
        assert_eq!(denied["error"]["code"], "WRITE_AUTHORIZATION_SUSPENDED");
    }
    // The original operation remains inspectable; no identity replacement, replay or user Undo.
    let original = service.operation(
        &editor,
        Operation {
            target: target.clone(),
            request_id: "owned-second".into(),
            original_target: None,
        },
    );
    assert_eq!(original["result"]["status"], "committed");
    assert_eq!(
        service.cancel(
            &editor,
            Cancel {
                target: target.clone(),
                preview_id
            }
        )["result"]["status"],
        "cancelled"
    );
    assert_eq!(editor.project, inserted);
    assert_eq!(editor.undo.len(), history);
    assert_eq!(service.authorization_status(&editor)["status"], "suspended");
    service.authorize_current(&editor);
    assert!(service.write_authorization(&editor).is_ok());
    assert_eq!(editor.project, inserted);
    assert_eq!(editor.undo.len(), history);
}

#[test]
fn cancel_cannot_retarget_a_preview_from_a_different_document_session() {
    let (editor, mut service, target, batch) = setup();
    let preview = service.prepare(
        &editor,
        Prepare {
            target: target.clone(),
            expected_revision: 0,
            change: batch,
        },
    );
    let id = preview["result"]["preview_id"]
        .as_str()
        .unwrap()
        .to_string();
    let other = Editor::new(editor.project.clone());
    let foreign = ReadService::new("a".into()).target(&other);
    assert_eq!(
        service.cancel(
            &other,
            Cancel {
                target: foreign,
                preview_id: id.clone()
            }
        )["error"]["code"],
        "TARGET_MISMATCH"
    );
    assert_eq!(
        service.cancel(
            &editor,
            Cancel {
                target,
                preview_id: id
            }
        )["result"]["status"],
        "cancelled"
    );
    assert!(editor.undo.is_empty() && other.undo.is_empty());
}

#[test]
fn cancellation_does_not_report_an_old_document_commit_as_current() {
    let (mut editor, mut service, target, batch) = setup();
    let p = service.prepare(
        &editor,
        Prepare {
            target: target.clone(),
            expected_revision: 0,
            change: batch,
        },
    );
    let preview_id = p["result"]["preview_id"].as_str().unwrap().to_string();
    assert_eq!(
        service.commit(
            &mut editor,
            Commit {
                target: target.clone(),
                preview_id: preview_id.clone(),
                request_id: "owned-original".into()
            }
        )["result"]["status"],
        "committed"
    );
    let other = Editor::new(Project::default());
    assert_eq!(
        service.cancel(
            &other,
            Cancel {
                target: ReadService::new("a".into()).target(&other),
                preview_id: preview_id.clone()
            }
        )["error"]["code"],
        "TARGET_MISMATCH"
    );
    assert_eq!(
        service.cancel(&editor, Cancel { target, preview_id })["result"]["status"],
        "already_committed"
    );
    assert_eq!(editor.project.elements.len(), 1);
    assert!(other.project.elements.is_empty());
}
