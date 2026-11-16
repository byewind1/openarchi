use atelier_bim::{
    application_host::{ApplicationHost, HostCommand},
    command::ModelCommand,
    editor::Editor,
    model::Point,
    storage,
    workbench::{EnvironmentState, WorkMode},
};

#[test]
fn host_transaction_matches_legacy_and_preserves_real_case_through_modes_and_history() {
    let project = storage::startup_example();
    let mut host = ApplicationHost::new(project.clone());
    let mut legacy = Editor::new(project.clone());
    let id = project
        .elements
        .iter()
        .find(|e| matches!(e.shape, atelier_bim::model::Shape::Zone { .. }))
        .unwrap()
        .id;
    let command = ModelCommand::MoveElements {
        ids: vec![id],
        offset: Point::new(0.25, 0.),
    };
    legacy.apply(command.clone()).unwrap();
    host.document.editor.set_selection(vec![id]);
    let receipt = host
        .dispatch(
            host.context(),
            HostCommand::Apply {
                request_id: "host-real-case-move".into(),
                commands: vec![command],
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(host.document.editor.project, legacy.project);
    assert_eq!(host.document.editor.undo.len(), legacy.undo.len());
    assert_eq!(receipt.changed_ids, vec![id]);
    let context = host.context();
    let before = host.document.editor.project.clone();
    let mut environment = EnvironmentState::default();
    for mode in [WorkMode::SiteConcept, WorkMode::Architecture] {
        environment.switch_mode(mode);
    }
    assert_eq!(host.context(), context);
    assert_eq!(host.document.editor.project, before);
    assert_eq!(host.document.editor.selection_ids(), vec![id]);
    assert!(host.document.editor.dirty());
    host.dispatch(host.context(), HostCommand::Undo).unwrap();
    assert_eq!(host.document.editor.project, project);
    assert!(!host.document.editor.dirty());
    host.dispatch(host.context(), HostCommand::Redo).unwrap();
    assert_eq!(host.document.editor.project, legacy.project);
}

#[test]
fn host_rejects_foreign_stale_and_invalid_requests_atomically() {
    let mut host = ApplicationHost::new(storage::startup_example());
    let foreign = ApplicationHost::new(storage::startup_example()).context();
    let before = host.document.editor.project.clone();
    assert!(host.dispatch(foreign, HostCommand::Undo).is_err());
    let mut stale = host.context();
    stale.revision += 1;
    assert!(host.dispatch(stale, HostCommand::Redo).is_err());
    assert!(
        host.dispatch(
            host.context(),
            HostCommand::Apply {
                request_id: "invalid".into(),
                commands: vec![ModelCommand::MoveElements {
                    ids: vec![u64::MAX],
                    offset: Point::new(f64::NAN, 0.)
                }]
            }
        )
        .is_err()
    );
    assert_eq!(host.document.editor.project, before);
    assert!(host.document.editor.undo.is_empty());
    assert!(!host.document.editor.dirty());
}

#[test]
fn bundled_profiles_resolve_unique_commands_without_optional_services() {
    use atelier_bim::workbench::WorkbenchCommand;
    let mut ids = std::collections::HashSet::new();
    for &command in WorkbenchCommand::ALL {
        assert!(ids.insert(command.id()));
    }
    for mode in [WorkMode::Architecture, WorkMode::SiteConcept] {
        let profile = mode.profile();
        assert_eq!(profile.schema_version, 1);
        assert_eq!(profile.mode, mode);
        assert!(profile.commands.contains(&WorkbenchCommand::Zone));
        assert!(profile.commands.contains(&WorkbenchCommand::Save));
        for command in profile.commands {
            assert!(WorkbenchCommand::ALL.contains(&command));
        }
    }
}

#[test]
fn work_mode_preference_roundtrip_and_invalid_file_preservation() {
    use atelier_bim::workbench::preferences;
    let directory = std::env::temp_dir().join(format!(
        "oa-mode-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = directory.join("work-mode.json");
    assert_eq!(
        preferences::load(&path).unwrap().mode,
        WorkMode::Architecture
    );
    assert!(!directory.exists());
    preferences::save(&path, WorkMode::SiteConcept).unwrap();
    assert_eq!(
        preferences::load(&path).unwrap().mode,
        WorkMode::SiteConcept
    );
    preferences::save(&path, WorkMode::Architecture).unwrap();
    assert_eq!(
        preferences::load(&path).unwrap().mode,
        WorkMode::Architecture
    );
    for bad in [
        b"{truncated".to_vec(),
        br#"{"schema_version":99,"mode":"SiteConcept"}"#.to_vec(),
        vec![b' '; 17000],
    ] {
        std::fs::write(&path, &bad).unwrap();
        assert!(preferences::load(&path).is_err());
        assert!(preferences::save(&path, WorkMode::SiteConcept).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bad);
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn work_mode_write_failure_preserves_obstructing_data_and_registry_is_current() {
    use atelier_bim::workbench::{CommandEffect, CommandRegistry, WorkbenchCommand, preferences};
    let path = std::env::temp_dir().join(format!(
        "oa-mode-file-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, b"preserve existing data").unwrap();
    assert!(preferences::save(&path.join("mode.json"), WorkMode::SiteConcept).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"preserve existing data");
    std::fs::remove_file(path).unwrap();
    let mut editor = Editor::new(storage::startup_example());
    let undo = CommandRegistry::resolve("model.undo").unwrap();
    assert_eq!(undo, WorkbenchCommand::Undo);
    assert_eq!(
        CommandRegistry::describe(undo, &editor).disabled_reason,
        Some("Nothing to undo")
    );
    editor.place_column(Point::new(30., 30.)).unwrap();
    let descriptor = CommandRegistry::describe(undo, &editor);
    assert_eq!(descriptor.disabled_reason, None);
    assert_eq!(descriptor.effect, CommandEffect::ModelHistory);
    assert_eq!(descriptor.parameter_schema["additionalProperties"], false);
    assert!(CommandRegistry::resolve("unknown.command").is_none());
}

#[test]
fn native_drawing_document_uses_shared_receipt_and_one_undo_without_geometry_changes() {
    use atelier_bim::{command::ModelCommand, drawings::Worksheet};
    let original = atelier_bim::storage::startup_example();
    let mut host = ApplicationHost::new(original.clone());
    let mut document = original.drawings.clone();
    document.worksheets.push(Worksheet {
        id: document.next_id().unwrap(),
        name: "Source paper".into(),
    });
    let receipt = host
        .dispatch(
            host.context(),
            HostCommand::Apply {
                request_id: "paper-header".into(),
                commands: vec![ModelCommand::SetDrawings {
                    document: document.clone(),
                }],
            },
        )
        .unwrap()
        .unwrap();
    assert!(receipt.drawings_changed);
    assert!(!receipt.site_context_changed && !receipt.program_changed && !receipt.evidence_changed);
    assert_eq!(host.document.editor.project.elements, original.elements);
    assert_eq!(host.document.editor.undo.len(), 1);
    host.dispatch(host.context(), HostCommand::Undo).unwrap();
    assert_eq!(host.document.editor.project, original);
    host.dispatch(host.context(), HostCommand::Redo).unwrap();
    assert_eq!(host.document.editor.project.drawings, document);
}

#[test]
fn creation_preset_host_preserves_model_and_authority_and_refuses_active_input() {
    use atelier_bim::workbench::creation_presets::CreationPresets;
    let mut host = ApplicationHost::new(storage::startup_example());
    let before = host.document.editor.project.clone();
    let context = host.context();
    let authority = host.object_task_authorization();
    let presets = CreationPresets {
        mass_height_mm: 6850,
        zone_height_mm: 3250,
        zone_purpose: "Concept teaching".into(),
    };
    host.apply_creation_presets(&presets).unwrap();
    assert_eq!(
        host.creation_defaults(),
        (6.85, 3.25, "Concept teaching".into())
    );
    assert_eq!(host.context(), context);
    assert_eq!(host.document.editor.project, before);
    assert_eq!(host.object_task_authorization(), authority);
    assert!(host.document.editor.undo.is_empty());
    host.document.editor.anchor = Some(Point::new(4., 5.));
    let defaults = host.creation_defaults();
    let mut other = presets.clone();
    other.zone_height_mm = 3600;
    assert!(host.apply_creation_presets(&other).is_err());
    assert_eq!(host.creation_defaults(), defaults);
    assert_eq!(host.document.editor.anchor, Some(Point::new(4., 5.)));
    assert_eq!(host.document.editor.project, before);
}
