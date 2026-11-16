use atelier_bim::{
    model::{ModelAdapter, OpeningKind, Shape},
    storage::{self, JsonAdapter},
};

#[test]
fn synthetic_example_is_complete_valid_and_independent() {
    let p = storage::startup_example();
    p.validate().unwrap();
    assert_eq!(p.elements.len(), 130);
    assert_eq!(
        p.elements
            .iter()
            .filter(|e| matches!(e.shape, Shape::Zone { .. }))
            .count(),
        36
    );
    assert!(p.name.contains("Synthetic"));
    assert_eq!(
        p.elements
            .iter()
            .filter(|e| matches!(
                e.shape,
                Shape::Opening {
                    kind: OpeningKind::Door,
                    ..
                }
            ))
            .count(),
        16
    );
    assert_eq!(
        p.elements
            .iter()
            .filter(|e| matches!(
                e.shape,
                Shape::Opening {
                    kind: OpeningKind::Window,
                    ..
                }
            ))
            .count(),
        21
    );
    assert!(
        p.elements
            .iter()
            .any(|e| matches!(e.shape, Shape::Stair { .. }))
    );
    assert!(
        p.elements
            .iter()
            .any(|e| matches!(e.shape, Shape::Roof { .. }))
    );
    for e in &p.elements {
        if matches!(e.shape, Shape::Opening { .. }) {
            assert!(e.opening_joinery.is_some());
        }
    }
    let restored = JsonAdapter
        .import(&JsonAdapter.export(&p).unwrap())
        .unwrap();
    assert_eq!(restored, p);
    let mut edited = p;
    edited.elements.clear();
    assert_eq!(storage::startup_example().elements.len(), 130);
}

#[test]
fn public_example_is_self_contained_and_checks_survive_edit_history_and_reopen() {
    use atelier_bim::{command::ModelCommand, constraints::State, editor::Editor};
    let raw: serde_json::Value =
        serde_json::from_slice(&JsonAdapter.export(&storage::startup_example()).unwrap()).unwrap();
    for private in [
        "_openarchi_document_id",
        "_openarchi_save_id",
        "design_process",
        "job_archive",
    ] {
        assert!(
            raw.get(private).is_none(),
            "private execution data: {private}"
        );
    }
    let p = storage::startup_example();
    assert!(p.design_process.is_empty());
    assert!(p.job_archive.is_empty());
    assert!(p.drawings.pages.is_empty());
    assert_eq!(p.drawings.diagrams.len(), 1);
    assert_eq!(raw["schema_version"], 20);
    let figure = &p.drawings.diagrams[0];
    assert!(matches!(
        figure.definition.presentation,
        atelier_bim::diagrams::Presentation::ConstraintOverlay
    ));
    assert_eq!(figure.status(&p), atelier_bim::diagrams::Status::Current);
    let overlay = atelier_bim::diagrams::checks::Source::read(&figure.snapshot).unwrap();
    assert_eq!(overlay.entries.len(), 6);
    assert!(overlay.lineage.is_empty());
    assert!(overlay.entries.iter().all(|e| e.citation.is_none()));
    assert!(figure.snapshot.context["task"].is_null());
    assert!(
        figure.snapshot.context["updates"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(p.rules.sources.is_empty() && p.rules.requirements.is_empty());
    assert_eq!(p.plan_options.active_design_option, Some(5));
    assert_eq!(
        p.rules
            .checks
            .iter()
            .map(|c| c.state(&p))
            .collect::<Vec<_>>(),
        [
            State::Fail,
            State::Unknown,
            State::Pass,
            State::Fail,
            State::Pass,
            State::Pass
        ]
    );
    let area = p.rules.checks[2].report.as_ref().unwrap().rows[0]
        .value
        .unwrap();
    assert!((area - 65.4).abs() < 1e-9);
    let mut editor = Editor::new(p.clone());
    let mut changed = p.elements.iter().find(|e| e.id == 133).unwrap().clone();
    let Shape::ConceptMass { spec } = &mut changed.shape else {
        panic!("mass133 missing")
    };
    assert_eq!(spec.height, 5.05);
    spec.height = 5.15;
    editor
        .apply(ModelCommand::UpdateElement { element: changed })
        .unwrap();
    assert_eq!(
        editor.project.rules.checks[0].state(&editor.project),
        State::Unknown
    );
    assert_eq!(
        editor.project.rules.checks[4].state(&editor.project),
        State::Pass
    );
    editor.undo();
    assert_eq!(editor.project, p);
    editor.redo();
    assert_eq!(
        editor.project.rules.checks[0].state(&editor.project),
        State::Unknown
    );
    editor.undo();
    let root = std::env::temp_dir().join(format!("oa-public-{}", storage::new_document_identity()));
    std::fs::create_dir(&root).unwrap();
    let file = root.join("continued.json");
    storage::save(&editor.project, &file).unwrap();
    let reopened = Editor::from_stored_document(&file).unwrap();
    assert_eq!(reopened.project, p);
    assert_eq!(
        reopened.project.rules.checks[2].state(&reopened.project),
        State::Pass
    );
    std::fs::remove_dir_all(root).unwrap();
}
