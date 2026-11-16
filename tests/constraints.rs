use atelier_bim::{
    command::ModelCommand,
    concept_mass::ConceptMassSpec,
    constraints::{self, Basis, Check, Definition, Domain, Edit, Metric, Relation, Scope, State},
    editor::Editor,
    model::{Material, ModelAdapter, Point, Project, Shape},
    storage::JsonAdapter,
};
fn ring(x: f64, y: f64, w: f64, h: f64) -> Vec<Point> {
    vec![
        Point::new(x, y),
        Point::new(x + w, y),
        Point::new(x + w, y + h),
        Point::new(x, y + h),
    ]
}
fn fixture() -> (Project, u64, u64) {
    let mut p = Project::default();
    let a = p.add(
        0,
        Material::Plaster,
        Shape::ConceptMass {
            spec: ConceptMassSpec {
                arcs: vec![],
                hole_arcs: vec![],
                outline: ring(0., 0., 8., 6.),
                holes: vec![ring(2., 2., 2., 2.)],
                height: 5.,
                bottom_offset: 1.,
            },
        },
    );
    let b = p.add(
        0,
        Material::Concrete,
        Shape::ConceptMass {
            spec: ConceptMassSpec {
                arcs: vec![],
                hole_arcs: vec![],
                outline: ring(10., 0., 2., 3.),
                holes: vec![],
                height: 3.,
                bottom_offset: 0.,
            },
        },
    );
    (p, a, b)
}
fn definition(metric: Metric, ids: Vec<u64>, limit: Option<f64>, relation: Relation) -> Definition {
    Definition {
        id: "design-check".into(),
        title: "Authored numerical requirement".into(),
        domain: Domain::ConceptMasses,
        scope: Scope::Objects { ids },
        metric,
        relation,
        limit,
        basis: Basis::DesignAssumption {
            note: "Controlled design assumption; no client or code authority".into(),
        },
    }
}
fn put_run(_p: &Project, d: Definition) -> Vec<ModelCommand> {
    let fresh = Check {
        definition: d.clone(),
        report: None,
    };
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
                expected_sha256: constraints::expected(&fresh),
            },
        },
    ]
}
// Numerical/unit oracle only: intentionally no source IO or product admission.
// Actual host/default entry admission is exercised in cited_originals and GUI tests.
fn numerical_fixture(p: Project, d: Definition) -> Editor {
    let mut e = Editor::new(p);
    for c in put_run(&e.project, d) {
        e.apply(c).unwrap();
    }
    e
}
fn execute(p: Project, d: Definition) -> Editor {
    let mut e = Editor::new(p);
    let c = e.prepare_change(0, put_run(&e.project, d)).unwrap();
    e.commit_change("controlled-check", &c).unwrap();
    e
}
#[test]
fn numerical_positive_negative_exact_boundary_unknown_and_true_hole_area() {
    let (p, a, b) = fixture();
    for (metric, ids, limit, relation, value, state) in [
        (
            Metric::Height,
            vec![a],
            Some(5.),
            Relation::AtMost,
            5.,
            State::Pass,
        ),
        (
            Metric::Height,
            vec![a],
            Some(4.999999),
            Relation::AtMost,
            5.,
            State::Fail,
        ),
        (
            Metric::Height,
            vec![a],
            Some(5.000001),
            Relation::AtLeast,
            5.,
            State::Fail,
        ),
        (
            Metric::Height,
            vec![a],
            None,
            Relation::AtMost,
            5.,
            State::Unknown,
        ),
        (
            Metric::FootprintAreaSum,
            vec![a, b],
            Some(50.),
            Relation::AtMost,
            50.,
            State::Pass,
        ),
        (
            Metric::FootprintAreaSum,
            vec![a, b],
            Some(49.),
            Relation::AtMost,
            50.,
            State::Fail,
        ),
        (
            Metric::HorizontalSpacing,
            vec![a, b],
            Some(2.),
            Relation::AtLeast,
            2.,
            State::Pass,
        ),
        (
            Metric::HorizontalSpacing,
            vec![a, b],
            Some(2.000001),
            Relation::AtLeast,
            2.,
            State::Fail,
        ),
    ] {
        let e = execute(p.clone(), definition(metric, ids, limit, relation));
        let c = &e.project.rules.checks[0];
        assert_eq!(c.state(&e.project), state);
        assert_eq!(
            constraints::entry(&e.project, c)["unit"],
            metric.unit_code()
        );
        assert!((c.report.as_ref().unwrap().rows[0].value.unwrap() - value).abs() < 1e-8);
        assert_eq!(
            constraints::report(&e.project)["compliance_verdict"],
            "not_evaluated"
        );
    }
}
#[test]
fn inside_courtyard_spacing_is_to_real_polygon_not_its_bounding_box() {
    let (mut p, a, b) = fixture();
    if let Shape::ConceptMass { spec } =
        &mut p.elements.iter_mut().find(|e| e.id == b).unwrap().shape
    {
        spec.outline = ring(2.5, 2.5, 1., 1.);
    }
    let e = execute(
        p,
        definition(
            Metric::HorizontalSpacing,
            vec![a, b],
            Some(0.5),
            Relation::AtLeast,
        ),
    );
    let row = &e.project.rules.checks[0].report.as_ref().unwrap().rows[0];
    assert_eq!(row.value, Some(0.5));
    assert_eq!(row.state, State::Pass);
}
#[test]
fn shared_transaction_cancel_stale_refresh_undo_redo_and_native19_refuse_downgrade() {
    let (p, a, _) = fixture();
    let mut e = Editor::new(p);
    let baseline = e.project.clone();
    let proposal = e
        .prepare_change(
            0,
            put_run(
                &e.project,
                definition(Metric::Height, vec![a], Some(5.), Relation::AtMost),
            ),
        )
        .unwrap();
    assert_eq!(e.project, baseline);
    drop(proposal);
    assert!(e.undo.is_empty());
    let proposal = e
        .prepare_change(
            0,
            put_run(
                &e.project,
                definition(Metric::Height, vec![a], Some(5.), Relation::AtMost),
            ),
        )
        .unwrap();
    e.commit_change("record-check", &proposal).unwrap();
    let after = e.project.clone();
    assert_eq!(after.elements, baseline.elements);
    e.undo();
    assert_eq!(e.project, baseline);
    e.redo();
    assert_eq!(e.project, after);
    let mut changed = e
        .project
        .elements
        .iter()
        .find(|e| e.id == a)
        .unwrap()
        .clone();
    if let Shape::ConceptMass { spec } = &mut changed.shape {
        spec.height = 6.;
    }
    e.apply(ModelCommand::UpdateElement { element: changed })
        .unwrap();
    assert_eq!(
        e.project.rules.checks[0].freshness(&e.project),
        "Check result is out of date"
    );
    assert_eq!(e.project.rules.checks[0].state(&e.project), State::Unknown);
    let c = &e.project.rules.checks[0];
    let command = ModelCommand::Constraint {
        edit: Edit::Run {
            id: c.definition.id.clone(),
            expected_sha256: constraints::expected(c),
        },
    };
    let proposal = e.prepare_change(e.revision, vec![command]).unwrap();
    e.commit_change("refresh-check", &proposal).unwrap();
    assert_eq!(e.project.rules.checks[0].state(&e.project), State::Fail);
    assert_eq!(
        e.project.rules.checks[0]
            .report
            .as_ref()
            .unwrap()
            .generation,
        2
    );
    let bytes = JsonAdapter.export(&e.project).unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["schema_version"], 19);
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), e.project);
    v["schema_version"] = serde_json::json!(18);
    assert!(
        JsonAdapter
            .import(&serde_json::to_vec(&v).unwrap())
            .unwrap_err()
            .contains("version 19")
    );
    let mut forged: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    forged["rules"]["checks"][0]["report"]["rows"][0]["state"] = serde_json::json!("pass");
    assert!(
        JsonAdapter
            .import(&serde_json::to_vec(&forged).unwrap())
            .unwrap_err()
            .contains("does not match")
    );
}
#[test]
fn missing_boundary_cited_requirement_and_deleted_object_never_pass() {
    let (p, a, _) = fixture();
    let e = execute(
        p.clone(),
        definition(
            Metric::ReferenceBoundaryClearance,
            vec![a],
            Some(3.),
            Relation::AtLeast,
        ),
    );
    assert_eq!(e.project.rules.checks[0].state(&e.project), State::Unknown);
    let mut d = definition(Metric::Height, vec![a], Some(100.), Relation::AtMost);
    d.basis = Basis::CitedRequirement {
        id: "missing-original-requirement".into(),
    };
    let e = numerical_fixture(p.clone(), d);
    assert_eq!(e.project.rules.checks[0].state(&e.project), State::Unknown);
    let mut e = execute(
        p,
        definition(Metric::Height, vec![a], Some(5.), Relation::AtMost),
    );
    e.apply(ModelCommand::RemoveElements { ids: vec![a] })
        .unwrap();
    assert_eq!(e.project.rules.checks[0].state(&e.project), State::Unknown);
}
#[test]
fn unsupported_or_mixed_scope_refuses_and_empty_candidate_can_remain_unknown() {
    let (p, a, _) = fixture();
    let mut d = definition(Metric::Height, vec![a], Some(f64::NAN), Relation::AtMost);
    assert!(d.validate().is_err());
    d.limit = Some(5.);
    d.scope = Scope::Candidate {
        story: 0,
        option: None,
    };
    let e = execute(Project::default(), d);
    assert_eq!(e.project.rules.checks[0].state(&e.project), State::Unknown);
    let mut wrong = p.clone();
    wrong.elements[0].shape = Shape::Wall {
        start: Point::new(0., 0.),
        end: Point::new(8., 0.),
        thickness: 0.2,
        height: 3.,
    };
    let e = Editor::new(wrong);
    assert!(
        e.prepare_change(
            0,
            put_run(
                &e.project,
                definition(Metric::Height, vec![a], Some(5.), Relation::AtMost)
            )
        )
        .is_err()
    );
}
#[test]
fn default_native_and_progressive_adapters_resolve_same_typed_check_transaction() {
    let (p, a, _) = fixture();
    let e = Editor::new(p);
    let d = definition(Metric::Height, vec![a], Some(5.), Relation::AtMost);
    let check = Check {
        definition: d.clone(),
        report: None,
    };
    let value = serde_json::json!({"version":1,"creates":[],"edits":[{"operation":"constraint","edit":{"action":"put","expected_sha256":null,"definition":d}},{"operation":"constraint","edit":{"action":"run","id":"design-check","expected_sha256":constraints::expected(&check)}}]});
    let native: atelier_bim::automation_registry::NativeBatch =
        serde_json::from_value(value.clone()).unwrap();
    let prepared = e
        .prepare_batch_change(0, &native.lower(&e).unwrap())
        .unwrap();
    assert_eq!(
        prepared.project().rules.checks[0].state(prepared.project()),
        State::Pass
    );
    let discovered: atelier_bim::automation_registry::DiscoveredBatch =
        serde_json::from_value(value).unwrap();
    let prepared = e
        .prepare_batch_change(0, &discovered.resolve().unwrap().lower(&e).unwrap())
        .unwrap();
    assert_eq!(
        prepared.project().rules.checks[0].state(prepared.project()),
        State::Pass
    );
}

#[test]
fn native_host_standalone_checks_are_durable_but_mixed_geometry_cannot_forge_the_marker() {
    use atelier_bim::application_host::ApplicationHost;
    use std::rc::Rc;
    let (p, a, _) = fixture();
    let dir = std::env::temp_dir().join(format!(
        "oa-check-host-{}",
        atelier_bim::storage::new_document_identity()
    ));
    std::fs::create_dir(&dir).unwrap();
    for mixed in [false, true] {
        let mut host = ApplicationHost::new(p.clone());
        host.configure_operation_journal(Some(dir.join(if mixed { "mixed" } else { "pure" })))
            .unwrap();
        let before = host.document.editor.project.clone();
        let mut commands = put_run(
            &before,
            definition(Metric::Height, vec![a], Some(5.), Relation::AtMost),
        );
        if mixed {
            commands.push(ModelCommand::MoveElements {
                ids: vec![a],
                offset: Point::new(1., 0.),
            });
        }
        let candidate = host.document.editor.prepare_change(0, commands).unwrap();
        let result = host.confirm_task_prepared(
            host.context(),
            "standalone-check",
            "Edit quantified checks",
            Rc::new(candidate),
        );
        if mixed {
            assert_eq!(result["error"]["code"], "DESIGN_TASK_MISMATCH");
            assert_eq!(host.document.editor.project, before);
        } else {
            assert_eq!(result["result"]["status"], "committed", "{result}");
            assert_eq!(host.document.editor.project.elements, before.elements);
            assert_eq!(
                host.document.editor.project.rules.checks[0].state(&host.document.editor.project),
                State::Pass
            );
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn optional_current_saved_case_check_keeps_model_sources_tasks_and_native19_archive_roundtrip() {
    let supplied = std::env::var_os("OPENARCHI_CONSTRAINTS_INPUT").map(std::path::PathBuf::from);
    let mut e = if let Some(path) = &supplied {
        Editor::from_stored_document(path).unwrap()
    } else {
        Editor::new(atelier_bim::storage::startup_example())
    };
    let p = e.project.clone();
    let bytes = supplied.as_ref().map(|p| std::fs::read(p).unwrap());
    let option = p.plan_options.active_design_option;
    let mass = p
        .elements
        .iter()
        .find(|e| e.design_option == option && matches!(e.shape, Shape::ConceptMass { .. }));
    let (domain, id) = if let Some(e) = mass {
        (Domain::ConceptMasses, e.id)
    } else {
        (
            Domain::Zones,
            p.elements
                .iter()
                .find(|e| matches!(e.shape, Shape::Zone { .. }))
                .unwrap()
                .id,
        )
    };
    let baseline = p.clone();
    let mut d = definition(Metric::Height, vec![id], Some(100.), Relation::AtMost);
    d.domain = domain;
    let added_check = d.id.clone();
    let prepared = e
        .prepare_change(e.revision, put_run(&e.project, d))
        .unwrap();
    e.commit_change("saved-case-quantified-check", &prepared)
        .unwrap();
    assert_eq!(e.project.elements, baseline.elements);
    assert_eq!(e.project.drawings, baseline.drawings);
    assert_eq!(e.project.design_process, baseline.design_process);
    assert_eq!(e.project.job_archive, baseline.job_archive);
    assert_eq!(e.project.rules.sources, baseline.rules.sources);
    assert_eq!(e.project.rules.requirements, baseline.rules.requirements);
    let stored = JsonAdapter.export(&e.project).unwrap();
    let reopened = JsonAdapter.import(&stored).unwrap();
    assert_eq!(reopened, e.project);
    assert_eq!(
        reopened
            .rules
            .checks
            .iter()
            .find(|c| c.definition.id == added_check)
            .unwrap()
            .state(&reopened),
        State::Pass
    );
    assert_eq!(reopened.rules.checks.len(), baseline.rules.checks.len() + 1);
    assert_eq!(
        &reopened.rules.checks[..baseline.rules.checks.len()],
        &baseline.rules.checks
    );
    if let Some(path) = &supplied {
        assert_eq!(std::fs::read(path).unwrap(), bytes.unwrap());
    }
}

fn reference_boundary(p: &mut Project, outline: Vec<Point>) {
    use atelier_bim::site::boundary::{BoundaryOrigin, Placement, SiteBoundary};
    let mut closed = outline.clone();
    closed.push(closed[0]);
    let id = p.add(
        0,
        Material::Concrete,
        Shape::Polyline {
            points: closed,
            arcs: vec![],
        },
    );
    p.site.boundary = Some(SiteBoundary {
        name: "Synthetic declared reference; not legal approval".into(),
        element_id: id,
        imported_outline_m: outline.iter().map(|p| [p.x, p.y]).collect(),
        origin: BoundaryOrigin {
            source_sha256: "a".repeat(64),
            entity_handle: "1A".into(),
            record_sha256: "b".repeat(64),
            unit: atelier_bim::site::LengthUnit::Metre,
            placement: Placement::ModelAxes,
            centerline_width_native: None,
        },
    });
}
#[test]
fn concave_reference_boundary_checks_actual_inside_geometry_and_minimum_distance() {
    for (x, y, limit, state, value) in [
        (1., 1., 1., State::Pass, Some(1.)),
        (1., 1., 1.000001, State::Fail, Some(1.)),
        (5., 5., 1., State::Fail, None),
    ] {
        let mut p = Project::default();
        let a = p.add(
            0,
            Material::Plaster,
            Shape::ConceptMass {
                spec: ConceptMassSpec {
                    arcs: vec![],
                    hole_arcs: vec![],
                    outline: ring(x, y, 1., 1.),
                    holes: vec![],
                    height: 3.,
                    bottom_offset: 0.,
                },
            },
        );
        reference_boundary(
            &mut p,
            vec![
                Point::new(0., 0.),
                Point::new(10., 0.),
                Point::new(10., 4.),
                Point::new(4., 4.),
                Point::new(4., 10.),
                Point::new(0., 10.),
            ],
        );
        let e = execute(
            p,
            definition(
                Metric::ReferenceBoundaryClearance,
                vec![a],
                Some(limit),
                Relation::AtLeast,
            ),
        );
        let row = &e.project.rules.checks[0].report.as_ref().unwrap().rows[0];
        assert_eq!(row.state, state);
        assert_eq!(row.value, value);
    }
}
#[test]
fn cited_applicability_pending_na_and_changed_context_are_separate_from_numeric_compliance() {
    use atelier_bim::rules::*;
    let (mut p, a, _) = fixture();
    p.rules.context = ProjectContext {
        jurisdiction: "Synthetic fixture".into(),
        uses: "Synthetic test use".into(),
        assessment_date: "2026-11-11".into(),
        stage: "Concept fixture".into(),
    };
    p.rules.sources.push(RuleSource {
        id: "synthetic".into(),
        title: "Authored test document, not regulation".into(),
        code: "TEST-ONLY".into(),
        edition: "1".into(),
        publisher: "Test fixture".into(),
        jurisdiction: "Synthetic fixture".into(),
        published_on: "2026-01-01".into(),
        effective_on: "2026-01-01".into(),
        original_sha256: "a".repeat(64),
        origin_url: String::new(),
        origin: Origin::Unverified,
        verification_note: "Synthetic; no authority or project approval".into(),
        replaces: vec![],
        same_edition_as: None,
    });
    for (app, adoption, state) in [
        (Applicability::Unknown, Adoption::Pending, State::Unknown),
        (
            Applicability::NotApplicable,
            Adoption::NotAdopted,
            State::NotApplicable,
        ),
        (Applicability::Applicable, Adoption::Adopted, State::Pass),
    ] {
        let mut current = p.clone();
        current.rules.requirements.push(Requirement {
            id: "fixture-check".into(),
            title: "Synthetic height interpretation".into(),
            topic: Topic::AreaMassing,
            citation: Citation {
                source_id: "synthetic".into(),
                locator: "text/line[1]".into(),
                text: "Synthetic test maximum parameter height 5 m.".into(),
            },
            interpretation: "Authored test only".into(),
            decision: Decision {
                applicability: app,
                adoption,
                rationale: "Explicit controlled declaration; not legal approval".into(),
                missing: if app == Applicability::Unknown {
                    "No approval".into()
                } else {
                    String::new()
                },
                basis_sha256: current.rules.basis("synthetic").unwrap(),
            },
            links: vec![],
        });
        let mut d = definition(Metric::Height, vec![a], Some(5.), Relation::AtMost);
        d.basis = Basis::CitedRequirement {
            id: "fixture-check".into(),
        };
        let mut e = numerical_fixture(current, d);
        assert_eq!(e.project.rules.checks[0].numerical_state(&e.project), state);
        assert_eq!(e.project.rules.checks[0].state(&e.project), State::Unknown);
        assert_eq!(
            constraints::report(&e.project)["compliance_verdict"],
            "not_evaluated"
        );
        e.project.rules.context.stage = "Changed design stage".into();
        assert_eq!(e.project.rules.checks[0].state(&e.project), State::Unknown);
    }
}

#[test]
fn definition_edit_keeps_original_report_definition_units_and_monotonic_generation() {
    let (p, a, _) = fixture();
    let mut e = execute(
        p,
        definition(Metric::Height, vec![a], Some(5.), Relation::AtMost),
    );
    let old = e.project.rules.checks[0].clone();
    let mut d = old.definition.clone();
    d.metric = Metric::FootprintAreaSum;
    d.limit = None;
    e.apply(ModelCommand::Constraint {
        edit: Edit::Put {
            expected_sha256: Some(constraints::expected(&old)),
            definition: d,
        },
    })
    .unwrap();
    let c = &e.project.rules.checks[0];
    assert_eq!(c.state(&e.project), State::Unknown);
    assert_eq!(c.report.as_ref().unwrap().input_definition, old.definition);
    assert_eq!(
        c.report.as_ref().unwrap().input_definition.metric.unit(),
        "m"
    );
    let edit = Edit::Run {
        id: c.definition.id.clone(),
        expected_sha256: constraints::expected(c),
    };
    e.apply(ModelCommand::Constraint { edit }).unwrap();
    let report = e.project.rules.checks[0].report.as_ref().unwrap();
    assert_eq!(report.generation, 2);
    assert_eq!(report.input_definition.metric.unit(), "m²");
    assert_eq!(report.rows[0].value, Some(44.));
    assert_eq!(report.rows[0].state, State::Unknown);
}

#[test]
fn edge_setbacks_are_signed_independent_thresholds_not_uniform_boundary_clearance() {
    let (mut p, a, _) = fixture();
    reference_boundary(&mut p, ring(-10., -10., 40., 40.));
    for (edge, expected) in [(0, 10.), (1, 22.), (2, 24.), (3, 10.)] {
        for (limit, state) in [
            (None, State::Unknown),
            (Some(expected), State::Pass),
            (Some(expected + 0.001), State::Fail),
        ] {
            let e = execute(
                p.clone(),
                definition(
                    Metric::ReferenceBoundaryEdgeClearance { edge },
                    vec![a],
                    limit,
                    Relation::AtLeast,
                ),
            );
            let r = &e.project.rules.checks[0].report.as_ref().unwrap().rows[0];
            assert_eq!(r.value, Some(expected));
            assert_eq!(r.state, state);
        }
    }
    let Shape::ConceptMass { spec } = &mut p.elements.iter_mut().find(|e| e.id == a).unwrap().shape
    else {
        unreachable!()
    };
    for q in spec
        .outline
        .iter_mut()
        .chain(spec.holes.iter_mut().flatten())
    {
        q.x -= 12.;
    }
    let e = execute(
        p,
        definition(
            Metric::ReferenceBoundaryEdgeClearance { edge: 3 },
            vec![a],
            Some(0.),
            Relation::AtLeast,
        ),
    );
    let r = &e.project.rules.checks[0].report.as_ref().unwrap().rows[0];
    assert_eq!(r.value, Some(-2.));
    assert_eq!(
        r.state,
        State::Fail,
        "outside must fail even a zero setback"
    );
}

#[test]
fn edge_setbacks_preserve_physical_distance_for_winding_rotation_and_large_origin() {
    for reverse in [false, true] {
        let (mut p, a, _) = fixture();
        let transform = |q: &mut Point| {
            let (x, y) = (q.x, q.y);
            q.x = 54_000. + 0.6 * x - 0.8 * y;
            q.y = 50_000. + 0.8 * x + 0.6 * y;
        };
        for e in &mut p.elements {
            if let Shape::ConceptMass { spec } = &mut e.shape {
                for q in spec
                    .outline
                    .iter_mut()
                    .chain(spec.holes.iter_mut().flatten())
                {
                    transform(q);
                }
            }
        }
        let mut outline = ring(-10., -10., 40., 40.);
        outline.iter_mut().for_each(transform);
        if reverse {
            outline.reverse();
        }
        reference_boundary(&mut p, outline);
        let e = execute(
            p,
            definition(
                Metric::ReferenceBoundaryEdgeClearance {
                    edge: if reverse { 2 } else { 0 },
                },
                vec![a],
                Some(9.99),
                Relation::AtLeast,
            ),
        );
        let r = &e.project.rules.checks[0].report.as_ref().unwrap().rows[0];
        assert!((r.value.unwrap() - 10.).abs() < 1e-8);
        assert_eq!(r.state, State::Pass);
    }
}

#[test]
fn edge_setbacks_missing_concave_or_missing_edge_are_unknown_and_never_uniform_fallback() {
    let (p, a, _) = fixture();
    let missing = execute(
        p.clone(),
        definition(
            Metric::ReferenceBoundaryEdgeClearance { edge: 0 },
            vec![a],
            Some(0.),
            Relation::AtLeast,
        ),
    );
    assert_eq!(
        missing.project.rules.checks[0].state(&missing.project),
        State::Unknown
    );
    let mut convex = p.clone();
    reference_boundary(&mut convex, ring(-10., -10., 40., 40.));
    let missing_edge = execute(
        convex,
        definition(
            Metric::ReferenceBoundaryEdgeClearance { edge: 4 },
            vec![a],
            Some(0.),
            Relation::AtLeast,
        ),
    );
    assert_eq!(
        missing_edge.project.rules.checks[0].state(&missing_edge.project),
        State::Unknown
    );
    let mut concave = p;
    reference_boundary(
        &mut concave,
        vec![
            Point::new(-10., -10.),
            Point::new(30., -10.),
            Point::new(30., 10.),
            Point::new(10., 10.),
            Point::new(10., 30.),
            Point::new(-10., 30.),
        ],
    );
    let e = execute(
        concave,
        definition(
            Metric::ReferenceBoundaryEdgeClearance { edge: 0 },
            vec![a],
            Some(0.),
            Relation::AtLeast,
        ),
    );
    let r = &e.project.rules.checks[0].report.as_ref().unwrap().rows[0];
    assert_eq!(r.state, State::Unknown);
    assert!(r.reason.contains("convex"));
    assert!(r.value.is_none());
}

#[test]
fn edge_setbacks_native23_reopen_undo_and_boundary_staleness_preserve_old_reports() {
    let (mut p, a, _) = fixture();
    reference_boundary(&mut p, ring(-10., -10., 40., 40.));
    let before = p.clone();
    let d = definition(
        Metric::ReferenceBoundaryEdgeClearance { edge: 1 },
        vec![a],
        Some(22.),
        Relation::AtLeast,
    );
    let mut e = execute(p, d);
    let after = e.project.clone();
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project, after);
    let bytes = JsonAdapter.export(&e.project).unwrap();
    let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["schema_version"], 23);
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), after);
    for version in [19, 20, 21, 22] {
        json["schema_version"] = version.into();
        assert!(
            JsonAdapter
                .import(&serde_json::to_vec(&json).unwrap())
                .unwrap_err()
                .contains("version 23")
        );
    }
    let boundary = e.project.site.boundary.as_ref().unwrap().element_id;
    let Shape::Polyline { points, .. } = &mut e
        .project
        .elements
        .iter_mut()
        .find(|x| x.id == boundary)
        .unwrap()
        .shape
    else {
        unreachable!()
    };
    points[1].x += 1.;
    let c = &e.project.rules.checks[0];
    assert_eq!(c.freshness(&e.project), "Check result is out of date");
    assert_eq!(c.state(&e.project), State::Unknown);
    assert_eq!(c.report, after.rules.checks[0].report);
    // The retained previous input still needs v23 after editing the metric.
    e.project.rules.checks[0].definition.metric = Metric::Height;
    let bytes = JsonAdapter.export(&e.project).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["schema_version"],
        23
    );
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), e.project);
}

#[test]
fn signed_edge_reports_do_not_relax_legacy_nonnegative_metrics() {
    let (p, a, _) = fixture();
    let mut e = execute(
        p,
        definition(Metric::Height, vec![a], Some(5.), Relation::AtMost),
    );
    e.project.rules.checks[0].report.as_mut().unwrap().rows[0].value = Some(-1.);
    assert!(
        JsonAdapter
            .export(&e.project)
            .unwrap_err()
            .contains("Invalid stored quantified check report")
    );
}

#[test]
fn optional_saved_case_edge_unknown_keeps_all_existing_content_and_reopens_native23() {
    let supplied = std::env::var_os("OPENARCHI_CONSTRAINTS_INPUT").map(std::path::PathBuf::from);
    let mut e = if let Some(path) = &supplied {
        Editor::from_stored_document(path).unwrap()
    } else {
        Editor::new(atelier_bim::storage::startup_example())
    };
    let bytes = supplied.as_ref().map(|p| std::fs::read(p).unwrap());
    let before = e.project.clone();
    let mass = before
        .elements
        .iter()
        .find(|e| matches!(e.shape, Shape::ConceptMass { .. }))
        .unwrap();
    let d = definition(
        Metric::ReferenceBoundaryEdgeClearance { edge: 0 },
        vec![mass.id],
        None,
        Relation::AtLeast,
    );
    let prepared = e.prepare_change(e.revision, put_run(&before, d)).unwrap();
    e.commit_change("saved-case-edge-unknown", &prepared)
        .unwrap();
    let mut restored = e.project.clone();
    restored.rules.checks.pop();
    assert_eq!(restored, before);
    assert_eq!(
        e.project.rules.checks.last().unwrap().state(&e.project),
        State::Unknown
    );
    let stored = JsonAdapter.export(&e.project).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&stored).unwrap()["schema_version"],
        23
    );
    assert_eq!(JsonAdapter.import(&stored).unwrap(), e.project);
    if let Some(path) = supplied {
        assert_eq!(std::fs::read(path).unwrap(), bytes.unwrap());
    }
}

#[test]
fn proposed_check_preview_uses_shared_numbers_without_saving_reports_and_filters_scope() {
    let (p, a, b) = fixture();
    let mut e = execute(
        p,
        definition(Metric::Height, vec![a], Some(5.), Relation::AtMost),
    );
    let mut other = definition(Metric::Height, vec![b], Some(4.), Relation::AtMost);
    other.id = "unrelated".into();
    e.apply(ModelCommand::Constraint {
        edit: Edit::Put {
            expected_sha256: None,
            definition: other,
        },
    })
    .unwrap();
    let baseline = e.project.clone();
    let mut element = e
        .project
        .elements
        .iter()
        .find(|x| x.id == a)
        .unwrap()
        .clone();
    let Shape::ConceptMass { spec } = &mut element.shape else {
        unreachable!()
    };
    spec.height = 6.;
    let change = e
        .prepare_change(e.revision, vec![ModelCommand::UpdateElement { element }])
        .unwrap();
    let preview = change.declared_check_previews();
    assert_eq!(preview.len(), 1);
    assert_eq!(preview[0].state, State::Fail);
    assert_eq!(preview[0].rows[0].value, Some(6.));
    assert_eq!(preview[0].rows[0].ids, vec![a]);
    assert_eq!(e.project, baseline);
    assert_eq!(change.project().rules, baseline.rules);
    let run = constraints::apply(
        change.project(),
        &Edit::Run {
            id: preview[0].definition.id.clone(),
            expected_sha256: constraints::expected(&baseline.rules.checks[0]),
        },
    )
    .unwrap();
    assert_eq!(preview[0].rows, run[0].report.as_ref().unwrap().rows);
    assert_eq!(
        preview[0].source_sha256.as_ref(),
        Some(&run[0].report.as_ref().unwrap().source_sha256)
    );
    e.commit_change("confirm-proposal-only", &change).unwrap();
    assert_eq!(e.project.rules, baseline.rules);
    assert_eq!(
        e.project.rules.checks[0].freshness(&e.project),
        "Check result is out of date"
    );
    e.undo();
    assert_eq!(e.project, baseline);
    e.redo();
    assert_eq!(e.project, *change.project());
    let bytes = JsonAdapter.export(&e.project).unwrap();
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), e.project);
}
#[test]
fn proposed_check_missing_inputs_and_citation_are_not_promoted_to_verified_pass() {
    let (p, a, _) = fixture();
    let mut d = definition(Metric::Height, vec![a], None, Relation::AtMost);
    let v = constraints::preview(&p, &d);
    assert_eq!(v.state, State::Unknown);
    assert_eq!(v.rows[0].value, Some(5.));
    d.limit = Some(8.);
    d.metric = Metric::ReferenceControlRegionClearance;
    d.relation = Relation::AtLeast;
    let v = constraints::preview(&p, &d);
    assert_eq!(v.state, State::Unknown);
    assert_eq!(v.rows[0].value, None);
    assert!(v.rows[0].reason.contains("missing"));
    d.metric = Metric::Height;
    d.relation = Relation::AtMost;
    d.basis = Basis::CitedRequirement {
        id: "missing-original".into(),
    };
    let v = constraints::preview(&p, &d);
    assert_eq!(v.state, State::Unknown);
    assert_eq!(v.rows[0].value, Some(5.));
    d.limit = Some(f64::NAN);
    let v = constraints::preview(&p, &d);
    assert_eq!(v.state, State::Unknown);
    assert!(v.source_sha256.is_none());
}

#[test]
#[ignore = "explicit read-only protected parametric project; no model commit or original write"]
fn proposed_check_real_parameter_preparation_profile() {
    use atelier_bim::design_process::parametric as pm;
    use sha2::{Digest, Sha256};
    let path = std::path::PathBuf::from(
        std::env::var_os("OPENARCHI_PARAMETER_CHECK_INPUT").expect("protected input"),
    );
    let input = std::fs::read(&path).unwrap();
    let e = Editor::from_stored_document(&path).unwrap();
    let before = e.project.clone();
    let task = e.project.design_process.task().unwrap();
    let option = e.project.plan_options.active_design_option.unwrap();
    let mut parameters = pm::current_parameters(&e.project, &task.id, option).unwrap();
    parameters.floor_height_mm += 100;
    let mut samples = vec![];
    for _ in 0..11 {
        let start = std::time::Instant::now();
        let change = pm::prepare_update(
            &e,
            &atelier_bim::storage::new_document_identity(),
            &task.id,
            parameters.clone(),
            "Protected read-only performance probe; not design adoption".into(),
        )
        .unwrap();
        let prepare = start.elapsed().as_nanos();
        let t = std::time::Instant::now();
        let checks = change.declared_check_previews();
        let check = t.elapsed().as_nanos();
        assert!(!checks.is_empty());
        assert!(checks.iter().all(|c| c.source_sha256.is_some()));
        assert_eq!(change.project().rules, e.project.rules);
        samples.push((prepare, check));
        std::hint::black_box(checks);
    }
    assert_eq!(e.project, before);
    assert_eq!(std::fs::read(&path).unwrap(), input);
    let cold = samples.remove(0);
    let mut prep = samples.iter().map(|v| v.0).collect::<Vec<_>>();
    let mut checks = samples.iter().map(|v| v.1).collect::<Vec<_>>();
    prep.sort();
    checks.sort();
    eprintln!(
        "PARAMETER_CHECK_CPU {}",
        serde_json::json!({"input_sha256":format!("{:x}",Sha256::digest(&input)),"objects":e.project.elements.len(),"cold_parameter_prepare_ns":cold.0,"cold_check_projection_ns":cold.1,"hot_median_parameter_prepare_ns":prep[5],"hot_median_check_projection_ns":checks[5],"boundary":"Actual pm::prepare_update immutable host proposal + declared_check_previews; no commit, no GPU or AI, original bytes unchanged"})
    );
}
