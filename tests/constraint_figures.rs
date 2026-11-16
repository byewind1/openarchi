use atelier_bim::{
    command::ModelCommand,
    concept_mass::ConceptMassSpec,
    constraints::{
        self, Basis, Check, Definition as CheckDefinition, Domain as CheckDomain,
        Edit as CheckEdit, Metric, Relation, Scope, State,
    },
    diagrams::{self, Edit, Status},
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
fn fixture() -> (Editor, u64, u64) {
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
                bottom_offset: 0.,
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
                height: 7.,
                bottom_offset: 0.,
            },
        },
    );
    (Editor::new(p), a, b)
}
fn apply(e: &mut Editor, id: &str, commands: Vec<ModelCommand>) {
    let prepared = e.prepare_change(e.revision, commands).unwrap();
    e.commit_change(id, &prepared).unwrap();
}
fn check(e: &mut Editor, id: &str, metric: Metric, ids: Vec<u64>, limit: f64) {
    let definition = CheckDefinition {
        id: id.into(),
        title: format!("Explicit test assumption {id}"),
        domain: CheckDomain::ConceptMasses,
        scope: Scope::Objects { ids },
        metric,
        relation: if metric == Metric::HorizontalSpacing || metric.uses_boundary() {
            Relation::AtLeast
        } else {
            Relation::AtMost
        },
        limit: Some(limit),
        basis: Basis::DesignAssumption {
            note: "Independent numerical fixture; no survey or approval".into(),
        },
    };
    let fresh = Check {
        definition: definition.clone(),
        report: None,
    };
    apply(
        e,
        &format!("check-{id}"),
        vec![
            ModelCommand::Constraint {
                edit: CheckEdit::Put {
                    expected_sha256: None,
                    definition,
                },
            },
            ModelCommand::Constraint {
                edit: CheckEdit::Run {
                    id: id.into(),
                    expected_sha256: constraints::expected(&fresh),
                },
            },
        ],
    );
}
fn create(e: &mut Editor) -> u64 {
    use atelier_bim::automation_registry::{DiscoveredBatch, NativeBatch};
    let raw = serde_json::json!({"version":1,"edits":[{"operation":"diagram","edit":{"action":"create","name":"Linked constraints","definition":{"story":0,"option":null,"domain":"site","presentation":{"type":"constraint_overlay"}}}}]});
    let native: NativeBatch = serde_json::from_value(raw.clone()).unwrap();
    let discovered: DiscoveredBatch = serde_json::from_value(raw).unwrap();
    assert_eq!(
        serde_json::to_value(discovered.resolve().unwrap()).unwrap(),
        serde_json::to_value(&native).unwrap()
    );
    let before = e.project.clone();
    let prepared = e
        .prepare_batch_change(e.revision, &native.lower(e).unwrap())
        .unwrap();
    assert_eq!(e.project, before);
    e.commit_change("create-overlay", &prepared).unwrap();
    e.project.drawings.diagrams.last().unwrap().id
}

fn refresh(e: &mut Editor, id: u64) {
    let expected = diagrams::expected(
        e.project
            .drawings
            .diagrams
            .iter()
            .find(|f| f.id == id)
            .unwrap(),
    );
    apply(
        e,
        &format!("refresh-{}", e.revision),
        vec![ModelCommand::Diagram {
            edit: Edit::Refresh {
                id,
                expected_sha256: expected,
            },
        }],
    );
}
#[test]
fn exact_rows_real_hole_spacing_missing_boundary_and_svg_object_identity() {
    let (mut e, a, b) = fixture();
    check(&mut e, "height", Metric::Height, vec![a, b], 6.);
    let id = create(&mut e);
    let f = &e.project.drawings.diagrams[0];
    let s = diagrams::checks::Source::read(&f.snapshot).unwrap();
    assert_eq!(s.element_state(a), Some(State::Pass));
    assert_eq!(s.element_state(b), Some(State::Fail));
    check(&mut e, "area", Metric::FootprintAreaSum, vec![a, b], 49.);
    check(&mut e, "gap", Metric::HorizontalSpacing, vec![a, b], 2.);
    check(
        &mut e,
        "boundary",
        Metric::ReferenceBoundaryClearance,
        vec![a],
        3.,
    );
    assert_eq!(
        e.project.drawings.diagrams[0].status(&e.project),
        Status::Stale
    );
    assert!(e.project.drawings.diagrams[0].svg(&e.project).is_err());
    refresh(&mut e, id);
    let f = &e.project.drawings.diagrams[0];
    let s = diagrams::checks::Source::read(&f.snapshot).unwrap();
    let entry = |name: &str| {
        s.entries
            .iter()
            .find(|v| v.check.definition.id == name)
            .unwrap()
    };
    assert_eq!(
        entry("area").check.report.as_ref().unwrap().rows[0].value,
        Some(50.)
    ); //8*6-2*2+2*3
    assert_eq!(
        entry("gap").check.report.as_ref().unwrap().rows[0].value,
        Some(2.)
    );
    assert_eq!(entry("boundary").state, State::Unknown);
    assert_eq!(
        entry("boundary").check.report.as_ref().unwrap().rows[0].value,
        None
    );
    assert_eq!(s.element_state(a), Some(State::Fail));
    assert_eq!(s.element_state(b), Some(State::Fail));
    let page = diagrams::site::Page::new(f).unwrap();
    assert_eq!(
        page.paths
            .iter()
            .find(|p| p.element_id == Some(a))
            .unwrap()
            .rings
            .len(),
        2
    );
    let svg = f.svg(&e.project).unwrap();
    assert!(svg.contains(&format!("data-element-id=\"{a}\"")));
    assert!(svg.contains("fill-rule=\"evenodd\""));
    assert!(svg.contains("not project approval"));
    assert!(
        diagrams::query(&e.project, 0, 1).unwrap()["figures"][0]["constraint_overlay"].is_object()
    );
}
#[test]
fn condition_change_preserves_old_value_refresh_never_reexecutes_check_and_storage_is_versioned() {
    let (mut e, a, _) = fixture();
    check(&mut e, "height", Metric::Height, vec![a], 6.);
    let id = create(&mut e);
    let geometry = e.project.elements.clone();
    let old = e.project.drawings.diagrams[0].clone();
    let c = e.project.rules.checks[0].clone();
    let mut definition = c.definition.clone();
    definition.limit = Some(4.);
    apply(
        &mut e,
        "edit-limit",
        vec![ModelCommand::Constraint {
            edit: CheckEdit::Put {
                definition,
                expected_sha256: Some(constraints::expected(&c)),
            },
        }],
    );
    assert_eq!(
        e.project.drawings.diagrams[0].status(&e.project),
        Status::Stale
    );
    refresh(&mut e, id);
    let f = &e.project.drawings.diagrams[0];
    assert_eq!(f.id, id);
    assert_eq!(f.generation, 2);
    let s = diagrams::checks::Source::read(&f.snapshot).unwrap();
    assert_eq!(s.entries[0].state, State::Unknown);
    assert_eq!(
        s.entries[0].check.report.as_ref().unwrap().rows[0].value,
        Some(5.)
    );
    assert_eq!(
        e.project.rules.checks[0]
            .report
            .as_ref()
            .unwrap()
            .generation,
        1
    );
    let c = e.project.rules.checks[0].clone();
    apply(
        &mut e,
        "explicit-run",
        vec![ModelCommand::Constraint {
            edit: CheckEdit::Run {
                id: c.definition.id.clone(),
                expected_sha256: constraints::expected(&c),
            },
        }],
    );
    assert_eq!(
        e.project.drawings.diagrams[0].status(&e.project),
        Status::Stale
    );
    refresh(&mut e, id);
    assert_eq!(
        diagrams::checks::Source::read(&e.project.drawings.diagrams[0].snapshot)
            .unwrap()
            .entries[0]
            .state,
        State::Fail
    );
    let after = e.project.clone();
    e.undo();
    assert_eq!(
        e.project.drawings.diagrams[0].status(&e.project),
        Status::Stale
    );
    e.redo();
    assert_eq!(e.project, after);
    assert_eq!(e.project.elements, geometry);
    assert_ne!(e.project.drawings.diagrams[0], old);
    let bytes = JsonAdapter.export(&e.project).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["schema_version"], 20);
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), e.project);
    let mut downgraded = value;
    downgraded["schema_version"] = 19.into();
    assert!(
        JsonAdapter
            .import(&serde_json::to_vec(&downgraded).unwrap())
            .unwrap_err()
            .contains("version 20")
    );
}
#[test]
fn cited_portable_results_remain_unknown_and_cannot_be_relabelled_pass() {
    let (mut e, a, _) = fixture();
    check(&mut e, "height", Metric::Height, vec![a], 6.);
    let old = e.project.rules.checks[0].clone();
    let mut definition = old.definition.clone();
    definition.basis = Basis::CitedRequirement {
        id: "missing-original-clause".into(),
    };
    apply(
        &mut e,
        "definition-without-run",
        vec![ModelCommand::Constraint {
            edit: CheckEdit::Put {
                definition,
                expected_sha256: Some(constraints::expected(&old)),
            },
        }],
    );
    create(&mut e);
    let f = &e.project.drawings.diagrams[0];
    let s = diagrams::checks::Source::read(&f.snapshot).unwrap();
    assert_eq!(s.entries[0].state, State::Unknown);
    assert_eq!(s.element_state(a), Some(State::Unknown));
    assert_eq!(
        s.entries[0].check.report.as_ref().unwrap().rows[0].value,
        Some(5.)
    );
    let mut bytes: serde_json::Value =
        serde_json::from_slice(&JsonAdapter.export(&e.project).unwrap()).unwrap();
    bytes["drawings"]["diagrams"][0]["snapshot"]["context"]["constraint_overlay"]["entries"][0]["state"] =
        "pass".into();
    assert!(
        JsonAdapter
            .import(&serde_json::to_vec(&bytes).unwrap())
            .unwrap_err()
            .contains("contradicts")
    );
}

#[test]
fn removed_check_targets_remain_explicit_while_other_checks_still_draw() {
    let (mut e, a, b) = fixture();
    check(&mut e, "missing", Metric::Height, vec![a], 6.);
    check(&mut e, "live", Metric::Height, vec![b], 8.);
    let id = create(&mut e);
    apply(
        &mut e,
        "remove-original-object",
        vec![ModelCommand::RemoveElements { ids: vec![a] }],
    );
    assert_eq!(
        e.project.drawings.diagrams[0].status(&e.project),
        Status::Stale
    );
    refresh(&mut e, id);
    let s = diagrams::checks::Source::read(&e.project.drawings.diagrams[0].snapshot).unwrap();
    assert_eq!(s.unlocated_ids, vec!["missing"]);
    assert_eq!(s.entries.len(), 1);
    assert_eq!(s.entries[0].check.definition.id, "live");
    assert_eq!(s.element_state(b), Some(State::Pass));
}
#[test]
fn overlapping_space_does_not_hide_failing_mass_and_labels_are_distinct() {
    let (mut e, a, _) = fixture();
    let z = e.project.add(
        0,
        Material::Plaster,
        Shape::Zone {
            spec: atelier_bim::zones::ZoneSpec {
                surface: Default::default(),
                outline: ring(0., 0., 8., 6.),
                holes: vec![ring(2., 2., 2., 2.)],
                height: 2.8,
                bottom_offset: 0.,
                purpose: "Unknown use".into(),
                label_position: None,
            },
        },
    );
    check(&mut e, "mass-fails", Metric::Height, vec![a], 4.);
    let mut definition = e.project.rules.checks[0].definition.clone();
    definition.id = "space-passes".into();
    definition.domain = CheckDomain::Zones;
    definition.scope = Scope::Objects { ids: vec![z] };
    definition.limit = Some(3.);
    let fresh = Check {
        definition: definition.clone(),
        report: None,
    };
    apply(
        &mut e,
        "space-check",
        vec![
            ModelCommand::Constraint {
                edit: CheckEdit::Put {
                    definition,
                    expected_sha256: None,
                },
            },
            ModelCommand::Constraint {
                edit: CheckEdit::Run {
                    id: "space-passes".into(),
                    expected_sha256: constraints::expected(&fresh),
                },
            },
        ],
    );
    create(&mut e);
    let page = diagrams::site::Page::new(&e.project.drawings.diagrams[0]).unwrap();
    let failed = page
        .paths
        .iter()
        .position(|p| p.element_id == Some(a))
        .unwrap();
    let passed = page
        .paths
        .iter()
        .position(|p| p.element_id == Some(z))
        .unwrap();
    assert!(
        failed > passed,
        "draw the known failed footprint last, even if its linked space passes"
    );
    let labels = page.labels();
    assert_ne!(
        labels.iter().find(|l| l.0 == a).unwrap().1,
        labels.iter().find(|l| l.0 == z).unwrap().1
    );
}

#[test]
fn linked_space_review_is_separate_from_its_numeric_row_and_link_refresh_invalidates_figure() {
    use atelier_bim::mass_spaces::{self, Edit as SpaceEdit};
    let (mut e, a, _) = fixture();
    let mass = e
        .project
        .elements
        .iter()
        .find(|v| v.id == a)
        .unwrap()
        .clone();
    let stamp = mass_spaces::element_stamp(&e.project, &mass).unwrap();
    apply(
        &mut e,
        "derive-space",
        vec![ModelCommand::MassSpaces {
            edit: SpaceEdit::Create {
                mass: a,
                expected_source_sha256: stamp,
                name: "Linked space".into(),
                purpose: "Unknown".into(),
                height: 3.,
                assumption: "One concept storey with explicit fixed height".into(),
            },
        }],
    );
    let z = e.project.design_process.mass_spaces[0].zone;
    let definition = CheckDefinition {
        id: "zone-height".into(),
        title: "Zone numeric height".into(),
        domain: CheckDomain::Zones,
        scope: Scope::Objects { ids: vec![z] },
        metric: Metric::Height,
        relation: Relation::AtMost,
        limit: Some(3.1),
        basis: Basis::DesignAssumption {
            note: "Declared concept height".into(),
        },
    };
    let fresh = Check {
        definition: definition.clone(),
        report: None,
    };
    apply(
        &mut e,
        "zone-check",
        vec![
            ModelCommand::Constraint {
                edit: CheckEdit::Put {
                    definition,
                    expected_sha256: None,
                },
            },
            ModelCommand::Constraint {
                edit: CheckEdit::Run {
                    id: "zone-height".into(),
                    expected_sha256: constraints::expected(&fresh),
                },
            },
        ],
    );
    let id = create(&mut e);
    assert_eq!(
        diagrams::checks::Source::read(&e.project.drawings.diagrams[0].snapshot)
            .unwrap()
            .element_state(z),
        Some(State::Pass)
    );
    let mut changed = mass;
    if let Shape::ConceptMass { spec } = &mut changed.shape {
        spec.height += 0.5;
    }
    apply(
        &mut e,
        "change-mass-height",
        vec![ModelCommand::UpdateElement { element: changed }],
    );
    assert_eq!(
        e.project.rules.checks[0].state(&e.project),
        State::Pass,
        "unchanged Zone geometry retains its actual numerical result"
    );
    assert_eq!(
        e.project.drawings.diagrams[0].status(&e.project),
        Status::Stale
    );
    refresh(&mut e, id);
    let source = diagrams::checks::Source::read(&e.project.drawings.diagrams[0].snapshot).unwrap();
    assert_eq!(source.entries[0].state, State::Pass);
    assert_eq!(source.lineage[0].status, "Mass-space source changed");
    assert_eq!(source.element_state(z), Some(State::Unknown));
    let geometry = e.project.elements.clone();
    let link = e.project.design_process.mass_spaces[0].clone();
    apply(
        &mut e,
        "explicit-link-refresh",
        vec![ModelCommand::MassSpaces {
            edit: SpaceEdit::Refresh {
                zone: z,
                expected_sha256: mass_spaces::expected(&link),
                height: None,
            },
        }],
    );
    assert_eq!(e.project.elements, geometry);
    assert_eq!(
        e.project.drawings.diagrams[0].status(&e.project),
        Status::Stale,
        "metadata-only lineage refresh is also a figure dependency"
    );
    refresh(&mut e, id);
    assert_eq!(
        diagrams::checks::Source::read(&e.project.drawings.diagrams[0].snapshot)
            .unwrap()
            .element_state(z),
        Some(State::Pass)
    );
}

#[test]
fn retained_edge_check_figure_requires_native23_even_after_live_check_is_removed() {
    let (mut e, a, _) = fixture();
    // No boundary supplied: a real unknown, retained visibly in the shared figure.
    check(
        &mut e,
        "edge-unknown",
        Metric::ReferenceBoundaryEdgeClearance { edge: 2 },
        vec![a],
        8.,
    );
    assert_eq!(e.project.rules.checks[0].state(&e.project), State::Unknown);
    create(&mut e);
    let c = e.project.rules.checks[0].clone();
    apply(
        &mut e,
        "remove-live-check",
        vec![ModelCommand::Constraint {
            edit: CheckEdit::Remove {
                id: c.definition.id.clone(),
                expected_sha256: constraints::expected(&c),
            },
        }],
    );
    assert!(e.project.rules.checks.is_empty());
    let bytes = JsonAdapter.export(&e.project).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["schema_version"], 23);
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), e.project);
    value["schema_version"] = 20.into();
    assert!(
        JsonAdapter
            .import(&serde_json::to_vec(&value).unwrap())
            .unwrap_err()
            .contains("version 23")
    );
}

#[test]
fn edge_reference_is_visible_in_portable_figure_and_bound_to_its_snapshot() {
    use atelier_bim::site::{
        LengthUnit,
        boundary::{BoundaryOrigin, Placement, SiteBoundary},
    };
    let (mut e, a, _) = fixture();
    let outline = ring(-10., -10., 40., 40.);
    let mut closed = outline.clone();
    closed.push(closed[0]);
    let border = e.project.add(
        0,
        Material::Concrete,
        Shape::Polyline {
            points: closed,
            arcs: vec![],
        },
    );
    e.project.site.boundary = Some(SiteBoundary {
        name: "Declared test only".into(),
        element_id: border,
        imported_outline_m: outline.iter().map(|p| [p.x, p.y]).collect(),
        origin: BoundaryOrigin {
            source_sha256: "a".repeat(64),
            entity_handle: "1A".into(),
            record_sha256: "b".repeat(64),
            unit: LengthUnit::Metre,
            placement: Placement::ModelAxes,
            centerline_width_native: None,
        },
    });
    for i in 0..9 {
        check(
            &mut e,
            &format!("edge-{i}"),
            Metric::ReferenceBoundaryEdgeClearance { edge: 1 },
            vec![a],
            22.,
        );
    }
    create(&mut e);
    let figure = e.project.drawings.diagrams[0].clone();
    let lines = figure.text_lines();
    assert!(
        lines
            .iter()
            .any(|(_, _, s, _)| s == "Reference edge 2: (30.000, -10.000)")
    );
    assert!(lines.iter().any(|(_, _, s, _)| s == "→ (30.000, 30.000) m"));
    assert!(lines.iter().any(|(_, _, s, _)| s == "+3 · Full check list"));
    assert!(
        lines
            .iter()
            .filter(|(x, _, s, _)| *x == 835.
                && (s.starts_with("Reference edge") || s.starts_with('→')))
            .all(|(_, y, _, _)| *y < 580.)
    );
    let svg = figure.svg(&e.project).unwrap();
    assert!(svg.contains("Reference edge 2: (30.000, -10.000)"));
    assert_eq!(
        diagrams::checks::Source::read(&figure.snapshot)
            .unwrap()
            .entries
            .len(),
        9
    );
    apply(
        &mut e,
        "move-reference",
        vec![ModelCommand::MoveElements {
            ids: vec![border],
            offset: Point::new(1., 0.),
        }],
    );
    assert_eq!(
        e.project.drawings.diagrams[0].text_lines(),
        lines,
        "portable endpoints must remain from old snapshot until explicit update"
    );
    let bytes = JsonAdapter.export(&e.project).unwrap();
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), e.project);
}
