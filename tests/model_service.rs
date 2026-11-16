use atelier_bim::{
    command::ModelCommand,
    editor::Editor,
    model::{OpeningKind, Point, Project, SlabContour},
    storage,
};
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
fn commands(e: &Editor) -> Vec<ModelCommand> {
    let mut batch = e
        .wall_creation_candidates(vec![(p(0., 0.), p(8., 0.))])
        .unwrap();
    let door = e
        .opening_in_batch_candidate(
            &batch,
            OpeningKind::Door,
            batch[0].id,
            2.,
            OpeningKind::Door.defaults(),
        )
        .unwrap();
    batch.push(door);
    let mut slab = e
        .slab_creation_candidate(SlabContour {
            points: vec![p(0., 0.), p(8., 0.), p(8., 6.), p(0., 6.)],
            ..Default::default()
        })
        .unwrap();
    slab.id = batch.iter().map(|x| x.id).max().unwrap() + 1;
    batch.push(slab);
    vec![ModelCommand::AddElements {
        elements: batch,
        group: false,
    }]
}
fn state(e: &Editor) -> String {
    format!(
        "{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}",
        e.project,
        e.undo,
        e.redo,
        e.selection_ids(),
        e.revision,
        e.status,
        e.dirty(),
        e.document_session()
    )
}
#[test]
fn preview_commit_recovery_and_real_history_share_one_editor() {
    let mut e = Editor::new(Project::default());
    let initial = e.project.clone();
    let before = state(&e);
    let prepared = e.prepare_change(e.revision, commands(&e)).unwrap();
    assert_eq!(state(&e), before);
    assert_eq!(prepared.difference().affected_ids().len(), 3);
    let expected = prepared.project().clone();
    let receipt = e.commit_change("house-1", &prepared).unwrap();
    assert_eq!(e.project, expected);
    assert_eq!(e.undo.len(), 1);
    assert_eq!(receipt.revision_after, 1);
    assert_eq!(e.change_receipt("house-1"), Some(&receipt));
    let committed = state(&e);
    assert_eq!(e.commit_change("house-1", &prepared).unwrap(), receipt);
    assert_eq!(state(&e), committed);
    let path = std::env::temp_dir().join(format!("oa-service-{}.json", std::process::id()));
    storage::save(&e.project, &path).unwrap();
    assert_eq!(storage::load(&path).unwrap(), expected);
    std::fs::remove_file(path).unwrap();
    e.undo();
    assert_eq!(e.project, initial);
    let undone = state(&e);
    assert_eq!(e.commit_change("house-1", &prepared).unwrap(), receipt);
    assert_eq!(state(&e), undone);
    e.redo();
    assert_eq!(e.project, expected);
}
#[test]
fn late_failure_and_prepare_preserve_selection_dirty_and_real_redo() {
    let mut e = Editor::new(Project::default());
    e.place_column(p(10., 10.)).unwrap();
    let redo = e.project.clone();
    e.undo();
    let before = state(&e);
    let mut cmds = commands(&e);
    cmds.push(ModelCommand::MoveElements {
        ids: vec![1],
        offset: p(f64::NAN, 0.),
    });
    assert_eq!(
        e.prepare_change(e.revision, cmds).err().unwrap().code,
        "VALIDATION_FAILED"
    );
    assert_eq!(state(&e), before);
    e.redo();
    assert_eq!(e.project, redo);
}
#[test]
fn stale_and_foreign_candidates_cannot_overwrite_user_edits() {
    let mut e = Editor::new(Project::default());
    let prepared = e.prepare_change(0, commands(&e)).unwrap();
    e.place_column(p(10., 10.)).unwrap();
    let before = state(&e);
    assert_eq!(
        e.commit_change("old", &prepared).unwrap_err().code,
        "REVISION_CONFLICT"
    );
    assert_eq!(state(&e), before);
    let mut reopened = Editor::new(prepared.project().clone());
    assert_eq!(
        reopened.commit_change("old", &prepared).unwrap_err().code,
        "DOCUMENT_CONFLICT"
    );
    assert!(e.prepare_change(0, commands(&e)).is_err());
}
#[test]
fn mixed_commands_commit_once_and_identity_reuse_is_rejected() {
    let mut e = Editor::new(Project::default());
    let mut cmds = commands(&e);
    cmds.push(ModelCommand::MoveElements {
        ids: vec![1],
        offset: p(1., 0.),
    });
    let prepared = e.prepare_change(0, cmds).unwrap();
    let expected = prepared.project().clone();
    e.commit_change("mixed", &prepared).unwrap();
    assert_eq!(e.undo.len(), 1);
    assert_eq!(e.project, expected);
    let other = e
        .prepare_change(
            e.revision,
            vec![ModelCommand::RemoveElements { ids: vec![1] }],
        )
        .unwrap();
    let before = state(&e);
    assert_eq!(
        e.commit_change("mixed", &other).unwrap_err().code,
        "REQUEST_ID_CONFLICT"
    );
    assert_eq!(state(&e), before);
    e.undo();
    assert!(e.project.elements.is_empty());
    e.redo();
    assert_eq!(e.project, expected);
}
#[test]
fn noops_internal_commands_and_invalid_request_identity_are_rejected() {
    let mut e = Editor::new(Project::default());
    let before = state(&e);
    assert_eq!(e.prepare_change(0, vec![]).err().unwrap().code, "NO_CHANGE");
    assert_eq!(
        e.prepare_change(0, vec![ModelCommand::ChangeSet])
            .err()
            .unwrap()
            .code,
        "UNSUPPORTED_COMMAND"
    );
    let prepared = e.prepare_change(0, commands(&e)).unwrap();
    assert_eq!(
        e.commit_change("", &prepared).unwrap_err().code,
        "INVALID_REQUEST_ID"
    );
    assert_eq!(state(&e), before);
    e.commit_change("fixed", &prepared).unwrap();
}

#[test]
fn request_identity_includes_base_revision_and_revision_cannot_wrap() {
    let mut e = Editor::new(Project::default());
    e.place_column(p(0., 0.)).unwrap();
    let movement = vec![ModelCommand::MoveElements {
        ids: vec![1],
        offset: p(1., 0.),
    }];
    let prepared = e.prepare_change(e.revision, movement.clone()).unwrap();
    e.commit_change("move", &prepared).unwrap();
    let next = e.prepare_change(e.revision, movement.clone()).unwrap();
    let before = state(&e);
    assert_eq!(
        e.commit_change("move", &next).unwrap_err().code,
        "REQUEST_ID_CONFLICT"
    );
    assert_eq!(state(&e), before);
    e.revision = u64::MAX;
    let prepared = e.prepare_change(e.revision, movement).unwrap();
    let before = state(&e);
    assert_eq!(
        e.commit_change("overflow", &prepared).unwrap_err().code,
        "REVISION_EXHAUSTED"
    );
    assert_eq!(state(&e), before);
}
#[test]
fn receipt_capacity_rejects_new_writes_without_forgetting_old_requests() {
    let mut e = Editor::new(Project::default());
    e.place_column(p(0., 0.)).unwrap();
    let movement = vec![ModelCommand::MoveElements {
        ids: vec![1],
        offset: p(1., 0.),
    }];
    let first = e.prepare_change(e.revision, movement.clone()).unwrap();
    let receipt = e.commit_change("first", &first).unwrap();
    for i in 1..1024 {
        let prepared = e.prepare_change(e.revision, movement.clone()).unwrap();
        e.commit_change(&format!("move-{i}"), &prepared).unwrap();
    }
    let next = e.prepare_change(e.revision, movement).unwrap();
    let before = state(&e);
    assert_eq!(
        e.commit_change("excess", &next).unwrap_err().code,
        "SESSION_CAPACITY"
    );
    assert_eq!(e.commit_change("first", &first).unwrap(), receipt);
    assert_eq!(state(&e), before);
}
