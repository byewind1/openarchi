use atelier_bim::{
    automation::{ReadService, Target},
    automation_change::{ChangeService, Commit, Operation, Prepare, Recover},
    editor::Editor,
    model::{Point, Project},
    model_service::{BatchChange, ElementTemplate, NamedElement},
    storage,
};
use std::path::PathBuf;
fn setup() -> (PathBuf, PathBuf, Editor, BatchChange) {
    let root = std::env::temp_dir().join(format!(
        "oa-durable-service-{}",
        storage::new_document_identity()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("model.json");
    let e = Editor::new(Project::default());
    storage::save_document(&e.project, &path, e.project_identity()).unwrap();
    let e = Editor::from_stored_document(&path).unwrap();
    let mut wall = e
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
    (root, path, e, batch)
}
fn commit(
    service: &mut ChangeService,
    editor: &mut Editor,
    target: &Target,
    batch: BatchChange,
) -> serde_json::Value {
    let preview = service.prepare(
        editor,
        Prepare {
            target: target.clone(),
            expected_revision: editor.revision,
            change: batch,
        },
    );
    let id = preview["result"]["preview_id"].as_str().unwrap();
    service.commit(
        editor,
        Commit {
            target: target.clone(),
            preview_id: id.into(),
            request_id: "original-op".into(),
        },
    )
}
#[test]
fn restart_lookup_restore_and_safe_undo_share_model_service() {
    let (root, path, mut editor, batch) = setup();
    let before = editor.project.clone();
    let target = ReadService::new("first".into()).target(&editor);
    let mut service =
        ChangeService::new("first".into(), true).with_journal_directory(root.join("journal"));
    let receipt = commit(&mut service, &mut editor, &target, batch);
    assert_eq!(receipt["result"]["status"], "committed");
    let after = editor.project.clone();
    drop(service);
    drop(editor);
    let mut editor = Editor::from_stored_document(&path).unwrap();
    let current = ReadService::new("second".into()).target(&editor);
    let mut service =
        ChangeService::new("second".into(), true).with_journal_directory(root.join("journal"));
    let lookup = service.operation(
        &editor,
        Operation {
            target: current.clone(),
            request_id: "original-op".into(),
            original_target: Some(target.clone()),
        },
    );
    assert_eq!(lookup["result"]["status"], "committed");
    assert_eq!(lookup["result"]["current_effect"], "BeforeImage");
    let blocked = service.recover(
        &mut editor,
        Recover {
            target: current.clone(),
            expected_revision: 0,
            original_target: target.clone(),
            request_id: "original-op".into(),
        },
    );
    assert_eq!(blocked["error"]["code"], "RECOVERY_DOCUMENT_REQUIRED");
    let mut editor = Editor::new(Project::default());
    let current = ReadService::new("second".into()).target(&editor);
    let restored = service.recover_document(
        &mut editor,
        Recover {
            target: current,
            expected_revision: 0,
            original_target: target.clone(),
            request_id: "original-op".into(),
        },
    );
    let current = ReadService::new("second".into()).target(&editor);
    assert_eq!(restored["result"]["status"], "committed");
    assert_eq!(editor.project, after);
    assert_eq!(editor.undo.len(), 1);
    let undone = service.undo(
        &mut editor,
        Operation {
            target: current,
            request_id: "original-op".into(),
            original_target: Some(target),
        },
    );
    assert_eq!(undone["result"]["status"], "undone");
    assert_eq!(editor.project, before);
    drop(service);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn user_undo_and_save_even_same_parameters_prevents_replay() {
    let (root, path, mut editor, batch) = setup();
    let before = editor.project.clone();
    let target = ReadService::new("first".into()).target(&editor);
    let mut service =
        ChangeService::new("first".into(), true).with_journal_directory(root.join("journal"));
    assert_eq!(
        commit(&mut service, &mut editor, &target, batch)["result"]["status"],
        "committed"
    );
    editor.undo();
    assert_eq!(editor.project, before);
    storage::save_document(&editor.project, &path, editor.project_identity()).unwrap();
    drop(service);
    drop(editor);
    let mut editor = Editor::from_stored_document(&path).unwrap();
    let current = ReadService::new("second".into()).target(&editor);
    let mut service =
        ChangeService::new("second".into(), true).with_journal_directory(root.join("journal"));
    let result = service.recover(
        &mut editor,
        Recover {
            target: current,
            expected_revision: 0,
            original_target: target,
            request_id: "original-op".into(),
        },
    );
    assert_eq!(result["error"]["code"], "RECOVERY_DOCUMENT_REQUIRED");
    assert_eq!(editor.project, before);
    assert!(editor.undo.is_empty());
    drop(service);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn journal_unavailable_never_falls_back_to_volatile_write() {
    let (root, _, mut editor, batch) = setup();
    let before = editor.project.clone();
    std::fs::write(root.join("blocked"), b"not a directory").unwrap();
    let target = ReadService::new("first".into()).target(&editor);
    let mut service =
        ChangeService::new("first".into(), true).with_journal_directory(root.join("blocked"));
    assert_eq!(
        commit(&mut service, &mut editor, &target, batch)["error"]["code"],
        "JOURNAL_UNAVAILABLE"
    );
    assert_eq!(editor.project, before);
    assert!(editor.undo.is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn external_file_writer_is_detected_before_recovery_model_change() {
    let (root, path, mut editor, batch) = setup();
    let target = ReadService::new("first".into()).target(&editor);
    let mut service =
        ChangeService::new("first".into(), true).with_journal_directory(root.join("journal"));
    commit(&mut service, &mut editor, &target, batch);
    drop(service);
    drop(editor);
    let mut editor = Editor::from_stored_document(&path).unwrap();
    let before = editor.project.clone();
    let current = ReadService::new("second".into()).target(&editor);
    let mut external = before.clone();
    external.name = "later external writer".into();
    storage::save_document(&external, &path, editor.project_identity()).unwrap();
    let mut service =
        ChangeService::new("second".into(), true).with_journal_directory(root.join("journal"));
    let result = service.recover(
        &mut editor,
        Recover {
            target: current,
            expected_revision: 0,
            original_target: target,
            request_id: "original-op".into(),
        },
    );
    assert_eq!(result["error"]["code"], "RECOVERY_DOCUMENT_REQUIRED");
    assert_eq!(editor.project, before);
    assert_eq!(storage::load(&path).unwrap(), external);
    drop(service);
    std::fs::remove_dir_all(root).unwrap();
}
