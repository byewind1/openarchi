//! Numerical authored geometry only; not surveyed retained buildings or native acceptance.
use atelier_bim::{
    command::ModelCommand,
    concept_mass::ConceptMassSpec,
    constraints::{self, Basis, Check, Definition, Domain, Edit, Metric, Relation, Scope, State},
    editor::Editor,
    model::{Material, ModelAdapter, Point, Project, Shape},
    storage::JsonAdapter,
};

fn ring(x: f64, y: f64, size: f64) -> Vec<Point> {
    vec![
        Point::new(x, y),
        Point::new(x + size, y),
        Point::new(x + size, y + size),
        Point::new(x, y + size),
    ]
}
fn fixture() -> (Project, u64, u64, u64) {
    let mut p = Project::default();
    let a = p.design_options.create("A".into());
    let b = p.design_options.create("B".into());
    let common = p.add(
        0,
        Material::Concrete,
        Shape::ConceptMass {
            spec: ConceptMassSpec {
                arcs: vec![],
                hole_arcs: vec![],
                outline: ring(0., 0., 10.),
                holes: vec![ring(2., 2., 6.)],
                height: 3.,
                bottom_offset: 0.,
            },
        },
    );
    let candidate = p.add(
        0,
        Material::Plaster,
        Shape::ConceptMass {
            spec: ConceptMassSpec {
                arcs: vec![],
                hole_arcs: vec![],
                outline: ring(4., 4., 2.),
                holes: vec![],
                height: 6.,
                bottom_offset: 0.,
            },
        },
    );
    p.elements
        .iter_mut()
        .find(|e| e.id == candidate)
        .unwrap()
        .design_option = Some(a);
    let other = p.add(
        0,
        Material::Plaster,
        Shape::ConceptMass {
            spec: ConceptMassSpec {
                arcs: vec![],
                hole_arcs: vec![],
                outline: ring(5., 5., 1.),
                holes: vec![],
                height: 7.,
                bottom_offset: 0.,
            },
        },
    );
    p.elements
        .iter_mut()
        .find(|e| e.id == other)
        .unwrap()
        .design_option = Some(b);
    (p, common, candidate, other)
}
fn def(a: u64, b: u64) -> Definition {
    Definition {
        id: "common-gap".into(),
        title: "Explicit shared context clearance".into(),
        domain: Domain::ConceptMasses,
        scope: Scope::Objects { ids: vec![a, b] },
        metric: Metric::HorizontalSpacing,
        relation: Relation::AtLeast,
        limit: Some(2.),
        basis: Basis::DesignAssumption {
            note: "Authored footprints only, not a surveyed retained building or a legal limit"
                .into(),
        },
    }
}
fn commit(e: &mut Editor, commands: Vec<ModelCommand>) {
    let c = e.prepare_change(e.revision, commands).unwrap();
    e.commit_change(&format!("fixture-{}", e.revision), &c)
        .unwrap();
}
fn put_run(e: &mut Editor, d: Definition) {
    let stamp = constraints::expected(&Check {
        definition: d.clone(),
        report: None,
    });
    commit(
        e,
        vec![
            ModelCommand::Constraint {
                edit: Edit::Put {
                    expected_sha256: None,
                    definition: d.clone(),
                },
            },
            ModelCommand::Constraint {
                edit: Edit::Run {
                    id: d.id,
                    expected_sha256: stamp,
                },
            },
        ],
    );
}
#[test]
fn common_mass_gap_uses_actual_courtyard_geometry_and_shared_transactions() {
    let (p, common, candidate, other) = fixture();
    let d = def(common, candidate);
    let original = p.clone();
    let preview = constraints::preview(&p, &d);
    assert_eq!(preview.state, State::Pass);
    assert_eq!(preview.rows[0].value, Some(2.)); // Overlapping bounding boxes, separated real polygons.
    assert_eq!(p, original);
    let mut e = Editor::new(p);
    put_run(&mut e, d.clone());
    let c = &e.project.rules.checks[0];
    assert_eq!(c.report.as_ref().unwrap().rows, preview.rows);
    assert_eq!(
        c.report.as_ref().unwrap().source_sha256,
        preview.source_sha256.unwrap()
    );
    let before = e.project.clone();
    let mut changed = e
        .project
        .elements
        .iter()
        .find(|e| e.id == candidate)
        .unwrap()
        .clone();
    if let Shape::ConceptMass { spec } = &mut changed.shape {
        for q in &mut spec.outline {
            q.x += 1.;
        }
    }
    let change = e
        .prepare_change(
            e.revision,
            vec![ModelCommand::UpdateElement { element: changed }],
        )
        .unwrap();
    let proposed = change.declared_check_previews();
    assert_eq!(proposed.len(), 1);
    assert_eq!(proposed[0].state, State::Fail);
    assert_eq!(proposed[0].rows[0].value, Some(1.));
    assert_eq!(e.project, before); // Preparing/cancelling has no side effects.
    e.commit_change("explicit-candidate-move", &change).unwrap();
    assert_eq!(e.project.rules, before.rules);
    assert_eq!(
        e.project.rules.checks[0].freshness(&e.project),
        "Check result is out of date"
    );
    for id in [common, other] {
        assert_eq!(
            e.project.elements.iter().find(|e| e.id == id),
            before.elements.iter().find(|e| e.id == id)
        );
    }
    let after = e.project.clone();
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project, after);
    let expected = constraints::expected(&e.project.rules.checks[0]);
    commit(
        &mut e,
        vec![ModelCommand::Constraint {
            edit: Edit::Run {
                id: d.id,
                expected_sha256: expected,
            },
        }],
    );
    assert_eq!(e.project.rules.checks[0].state(&e.project), State::Fail);
    assert_eq!(
        JsonAdapter
            .import(&JsonAdapter.export(&e.project).unwrap())
            .unwrap(),
        e.project
    );
}
#[test]
fn mixed_alternatives_and_other_domains_or_metrics_stay_rejected() {
    let (p, common, candidate, other) = fixture();
    for d in [
        def(candidate, other),
        Definition {
            metric: Metric::FootprintAreaSum,
            ..def(common, candidate)
        },
        Definition {
            metric: Metric::Height,
            ..def(common, candidate)
        },
    ] {
        assert_eq!(constraints::preview(&p, &d).state, State::Unknown);
        let e = Editor::new(p.clone());
        assert!(
            e.prepare_change(
                0,
                vec![ModelCommand::Constraint {
                    edit: Edit::Put {
                        expected_sha256: None,
                        definition: d
                    }
                }]
            )
            .is_err()
        );
    }
    let mut q = p.clone();
    q.elements
        .iter_mut()
        .find(|e| e.id == common)
        .unwrap()
        .story = 99;
    assert_eq!(
        constraints::preview(&q, &def(common, candidate)).state,
        State::Unknown
    );
    let missing = constraints::preview(&p, &def(candidate, 999));
    assert_eq!(missing.state, State::Unknown);
    assert!(missing.rows[0].value.is_none());
    for basis in [
        Basis::DesignAssumption {
            note: "Unknown limit".into(),
        },
        Basis::CitedRequirement {
            id: "unavailable".into(),
        },
    ] {
        let d = Definition {
            limit: None,
            basis,
            ..def(common, candidate)
        };
        assert_eq!(constraints::preview(&p, &d).state, State::Unknown);
    }
}

#[test]
fn common_context_change_invalidates_only_bound_candidate_report_and_figure() {
    use atelier_bim::diagrams::{
        self, Definition as FigureDefinition, Domain as FigureDomain, Presentation,
    };
    let (p, common, candidate, other) = fixture();
    let mut e = Editor::new(p);
    put_run(&mut e, def(common, candidate));
    let option = e
        .project
        .elements
        .iter()
        .find(|e| e.id == candidate)
        .unwrap()
        .design_option;
    let other_option = e
        .project
        .elements
        .iter()
        .find(|e| e.id == other)
        .unwrap()
        .design_option;
    let definition = FigureDefinition {
        story: 0,
        option,
        domain: FigureDomain::Site,
        presentation: Presentation::ConstraintOverlay,
    };
    let candidate_elements = e
        .project
        .elements
        .iter()
        .filter(|e| e.design_option == option)
        .cloned()
        .collect::<Vec<_>>();
    let source =
        diagrams::checks::Source::capture(&e.project, &definition, &candidate_elements).unwrap();
    assert_eq!(source.entries.len(), 1);
    assert_eq!(source.entries[0].active_ids, vec![candidate]);
    let b_elements = e
        .project
        .elements
        .iter()
        .filter(|e| e.design_option == other_option)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        diagrams::checks::Source::capture_if_present(
            &e.project,
            &FigureDefinition {
                option: other_option,
                ..definition.clone()
            },
            &b_elements
        )
        .unwrap()
        .is_none()
    );
    commit(
        &mut e,
        vec![ModelCommand::Diagram {
            edit: diagrams::Edit::Create {
                name: "Authored common clearance".into(),
                definition,
            },
        }],
    );
    let original = e.project.clone();
    let figure = &original.drawings.diagrams[0];
    let page = diagrams::site::Page::new(figure).unwrap();
    assert!(
        page.paths.iter().any(|p| p.element_id == Some(common)),
        "A clearance figure must show its explicitly measured common footprint"
    );
    assert!(
        figure
            .svg(&original)
            .unwrap()
            .contains(&format!("data-element-id=\"{common}\""))
    );
    assert_eq!(
        figure.snapshot.elements.len(),
        1,
        "common context is not candidate quantity"
    );
    let source = diagrams::checks::Source::read(&figure.snapshot).unwrap();
    assert_eq!(source.version, 2);
    assert_eq!(
        source.common_context,
        vec![
            original
                .elements
                .iter()
                .find(|e| e.id == common)
                .unwrap()
                .clone()
        ]
    );
    assert_eq!(source.entries[0].active_ids, vec![candidate]);
    let candidate_element = original
        .elements
        .iter()
        .find(|e| e.id == candidate)
        .unwrap();
    assert_eq!(figure.total(), diagrams::quantities(candidate_element));
    assert_eq!(
        diagrams::site::figure_summary(figure)["common_context_ids"],
        serde_json::json!([common])
    );
    assert!(
        diagrams::site::legend(&page)
            .iter()
            .any(|(_, _, s)| *s == "Common context · excluded from candidate quantities")
    );
    for mutation in 0..4 {
        let mut invalid = source.clone();
        match mutation {
            0 => invalid.common_context[0].design_option = other_option,
            1 => invalid.common_context[0].id = 99999,
            2 => invalid
                .common_context
                .push(invalid.common_context[0].clone()),
            _ => invalid.common_context[0].story = 99,
        }
        assert!(
            invalid
                .validate(&figure.definition, &figure.snapshot.elements)
                .is_err()
        );
    }
    // Legacy snapshots remain readable; adding explicit common geometry makes
    // only these mixed-scope overlays stale until a normal, explicit refresh.
    let mut legacy = source.clone();
    legacy.version = 1;
    legacy.common_context.clear();
    legacy
        .validate(&figure.definition, &figure.snapshot.elements)
        .unwrap();
    let mut changed = e
        .project
        .elements
        .iter()
        .find(|e| e.id == common)
        .unwrap()
        .clone();
    if let Shape::ConceptMass { spec } = &mut changed.shape {
        for q in &mut spec.holes[0] {
            q.x -= 1.;
        }
    }
    let change = e
        .prepare_change(
            e.revision,
            vec![ModelCommand::UpdateElement { element: changed }],
        )
        .unwrap();
    let proposed = change.declared_check_previews();
    assert_eq!(proposed[0].rows[0].value, Some(1.));
    e.commit_change("explicit-common-user-edit", &change)
        .unwrap();
    assert_eq!(e.project.rules, original.rules);
    assert_eq!(e.project.rules.checks[0].state(&e.project), State::Unknown);
    assert_eq!(
        e.project.drawings.diagrams[0].status(&e.project),
        diagrams::Status::Stale
    );
    let check_stamp = constraints::expected(&e.project.rules.checks[0]);
    commit(
        &mut e,
        vec![ModelCommand::Constraint {
            edit: Edit::Run {
                id: "common-gap".into(),
                expected_sha256: check_stamp,
            },
        }],
    );
    let figure = &e.project.drawings.diagrams[0];
    let (id, stamp) = (figure.id, diagrams::expected(figure));
    commit(
        &mut e,
        vec![ModelCommand::Diagram {
            edit: diagrams::Edit::Refresh {
                id,
                expected_sha256: stamp,
            },
        }],
    );
    assert_eq!(
        e.project.drawings.diagrams[0].status(&e.project),
        diagrams::Status::Current
    );
    let source = diagrams::checks::Source::read(&e.project.drawings.diagrams[0].snapshot).unwrap();
    assert_eq!(source.element_state(candidate), Some(State::Fail));
    assert_eq!(
        JsonAdapter
            .import(&JsonAdapter.export(&e.project).unwrap())
            .unwrap(),
        e.project
    );
}

#[test]
#[ignore = "explicit protected saved input and prior endpoint check observations; original is read-only"]
fn protected_existing_check_semantics_and_complete_project_stay_unchanged() {
    let input = std::path::PathBuf::from(std::env::var_os("OPENARCHI_COMMON_GAP_INPUT").unwrap());
    let prior =
        std::path::PathBuf::from(std::env::var_os("OPENARCHI_COMMON_GAP_PRIOR_CHECKS").unwrap());
    let output = std::path::PathBuf::from(std::env::var_os("OPENARCHI_COMMON_GAP_OUTPUT").unwrap());
    let bytes = std::fs::read(&input).unwrap();
    let e = Editor::from_stored_document(&input).unwrap();
    let before = e.project.clone();
    let prior: serde_json::Value = serde_json::from_slice(&std::fs::read(prior).unwrap()).unwrap();
    let entries = prior["result"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), before.rules.checks.len());
    for c in &before.rules.checks {
        let entry = entries
            .iter()
            .find(|v| v["definition"]["id"] == c.definition.id)
            .unwrap();
        assert_eq!(constraints::entry(&before, c), *entry);
        let p = constraints::preview(&before, &c.definition);
        if c.freshness(&before) == "Check inputs unchanged" {
            assert_eq!(p.state, c.state(&before));
        }
    }
    assert_eq!(e.project, before);
    assert_eq!(std::fs::read(&input).unwrap(), bytes);
    let result = serde_json::json!({"status":"pass","entry":"same current engine compared against saved prior default endpoint observations","existing_checks_unchanged":entries.len(),"complete_project_unchanged":true,"original_file_unchanged":true,"actual_retained_geometry_provided":false,"real_retained_clearance":"unknown","native_package":"not_run_for_current_source"});
    use std::io::Write;
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output)
        .unwrap()
        .write_all(&serde_json::to_vec_pretty(&result).unwrap())
        .unwrap();
}
