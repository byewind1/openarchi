use atelier_bim::{automation::ReadService, editor::Editor, model::Project, storage};
#[test]
fn project_identity_persists_outside_parameter_model_and_session_rotates() {
    let editor = Editor::new(Project::default());
    let other = Editor::new(Project::default());
    assert_eq!(editor.project, other.project);
    assert_ne!(editor.project_identity(), other.project_identity());
    let path = std::env::temp_dir().join(format!("oa-document-{}.json", editor.project_identity()));
    storage::save_document(&editor.project, &path, editor.project_identity()).unwrap();
    let (project, version, id) = storage::load_document(&path).unwrap();
    assert_eq!(project, editor.project);
    assert_eq!(id.as_deref(), Some(editor.project_identity()));
    let mut reopened = Editor::new(project);
    reopened.adopt_document_identity(id.unwrap()).unwrap();
    assert_ne!(reopened.document_session(), editor.document_session());
    assert_eq!(reopened.project_identity(), editor.project_identity());
    let service = ReadService::new("instance".into());
    assert_ne!(service.target(&editor), service.target(&reopened));
    let stale = version.clone();
    storage::save_document(&other.project, &path, other.project_identity()).unwrap();
    assert!(
        storage::save_document_if_unchanged(
            &editor.project,
            &path,
            &stale,
            editor.project_identity()
        )
        .is_err()
    );
    std::fs::remove_file(&path).unwrap();
    let lock = path.with_file_name(format!(
        ".{}.openarchi-lock",
        path.file_name().unwrap().to_str().unwrap()
    ));
    let _ = std::fs::remove_file(lock);
}
#[test]
fn legacy_files_remain_parameter_equivalent_and_do_not_invent_persisted_identity() {
    let project = Project::default();
    let id = storage::new_document_identity();
    let path = std::env::temp_dir().join(format!("oa-legacy-{id}.json"));
    storage::save(&project, &path).unwrap();
    let (restored, _, identity) = storage::load_document(&path).unwrap();
    assert_eq!(restored, project);
    assert!(identity.is_none());
    std::fs::remove_file(path).unwrap();
}
