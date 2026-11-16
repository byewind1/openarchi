use atelier_bim::{
    command::ModelCommand,
    concept_mass::ConceptMassSpec,
    constraints::{self, Basis, Check, Definition, Domain, Edit, Metric, Relation, Scope, State},
    diagrams::{self, Presentation},
    editor::Editor,
    model::{Material, ModelAdapter, Point, Project, Shape},
    project_repository::{
        LengthUnit, Repository,
        boundary::{BoundaryPlacement, BoundaryWidthPolicy},
    },
    site::boundary::Role,
    storage::{self, JsonAdapter},
};
use std::{fs, path::PathBuf};
struct Fixture {
    root: PathBuf,
    e: Editor,
    repo: Repository,
    source: String,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn drawing() -> Vec<u8> {
    let mut s = "0\nSECTION\n2\nENTITIES\n".to_string();
    for (id, points) in [
        (
            "A1",
            vec![(0, 0), (20, 0), (20, 8), (12, 8), (12, 20), (0, 20)],
        ),
        (
            "A2",
            vec![(2, 2), (10, 2), (10, 6), (18, 6), (18, 7), (8, 7), (2, 18)],
        ),
    ] {
        s += &format!(
            "0\nLWPOLYLINE\n5\n{id}\n8\nREFERENCE\n90\n{}\n70\n1\n",
            points.len()
        );
        for (x, y) in points {
            s += &format!("10\n{x}\n20\n{y}\n");
        }
    }
    s += "0\nENDSEC\n0\nEOF\n";
    s.into_bytes()
}
fn fixture() -> Fixture {
    let root =
        std::env::temp_dir().join(format!("oa-control-{}", storage::new_document_identity()));
    fs::create_dir(&root).unwrap();
    let e = Editor::new(Project::default());
    let mut repo = Repository::create_for_editor(&root.join("repo"), &e).unwrap();
    let path = root.join("declarations.dxf");
    fs::write(&path, drawing()).unwrap();
    let source = repo.add_source(&path).unwrap();
    repo.confirm_unit(&source, Some(LengthUnit::Metre)).unwrap();
    Fixture {
        root,
        e,
        repo,
        source,
    }
}
fn proposal(f: &Fixture, role: Role) -> atelier_bim::site::boundary::BoundaryProposal {
    f.repo
        .preview_reference_region(
            &f.e.project,
            &f.source,
            if role == Role::Site { "A1" } else { "A2" },
            BoundaryPlacement::ModelAxes,
            "Authored reference; no approval".into(),
            0,
            1,
            BoundaryWidthPolicy::RejectNonzero,
            role,
        )
        .unwrap()
}
fn apply(e: &mut Editor, commands: Vec<ModelCommand>) {
    let p = e.prepare_change(e.revision, commands).unwrap();
    e.commit_change(&storage::new_document_identity(), &p)
        .unwrap();
}
fn import(f: &mut Fixture, role: Role) {
    let p = proposal(f, role);
    let _guard = f.repo.guard_boundary_commit(&p).unwrap();
    apply(
        &mut f.e,
        vec![if role == Role::Site {
            ModelCommand::ApplySiteBoundary { proposal: p }
        } else {
            ModelCommand::ApplySiteControlRegion { proposal: p }
        }],
    );
}
fn mass(p: &mut Project, x: f64, y: f64) -> u64 {
    p.add(
        0,
        Material::Plaster,
        Shape::ConceptMass {
            spec: ConceptMassSpec {
                arcs: vec![],
                hole_arcs: vec![],
                outline: vec![
                    Point::new(x, y),
                    Point::new(x + 2., y),
                    Point::new(x + 2., y + 2.),
                    Point::new(x, y + 2.),
                ],
                holes: vec![],
                height: 3.,
                bottom_offset: 0.,
            },
        },
    )
}
fn check(e: &mut Editor, id: u64, metric: Metric, limit: Option<f64>) {
    let d = Definition {
        id: "control-check".into(),
        title: "Declared scope fixture".into(),
        domain: Domain::ConceptMasses,
        scope: Scope::Objects { ids: vec![id] },
        metric,
        relation: Relation::AtLeast,
        limit,
        basis: Basis::DesignAssumption {
            note: "Explicit reference geometry; no approved planning authority".into(),
        },
    };
    let fresh = Check {
        definition: d.clone(),
        report: None,
    };
    apply(
        e,
        vec![
            ModelCommand::Constraint {
                edit: Edit::Put {
                    expected_sha256: None,
                    definition: d,
                },
            },
            ModelCommand::Constraint {
                edit: Edit::Run {
                    id: "control-check".into(),
                    expected_sha256: constraints::expected(&fresh),
                },
            },
        ],
    );
}
#[test]
fn independent_concave_references_preview_cancel_commit_undo_and_source_guard() {
    let mut f = fixture();
    import(&mut f, Role::Site);
    let before = f.e.project.clone();
    let p = proposal(&f, Role::ControlRegion);
    let cmd = ModelCommand::ApplySiteControlRegion {
        proposal: p.clone(),
    };
    let prepared = f.e.prepare_change(f.e.revision, vec![cmd.clone()]).unwrap();
    assert_eq!(f.e.project, before, "preview is read-only");
    assert_eq!(prepared.project().site.boundary, before.site.boundary);
    drop(prepared);
    assert_eq!(f.e.project, before);
    let guard = atelier_bim::project_repository::conditions::guard_conditions(
        Some(&f.repo),
        f.e.project_identity(),
        &f.e.project,
        &[cmd.clone()],
    )
    .unwrap();
    apply(&mut f.e, vec![cmd]);
    drop(guard);
    let after = f.e.project.clone();
    assert_eq!(after.site.boundary, before.site.boundary);
    assert_eq!(after.elements[..before.elements.len()], before.elements);
    assert_ne!(
        after.site.boundary.as_ref().unwrap().element_id,
        after.site.control_region.as_ref().unwrap().element_id
    );
    assert_eq!(
        after
            .site
            .control_region
            .as_ref()
            .unwrap()
            .imported_outline_m
            .len(),
        7
    );
    f.e.undo();
    assert_eq!(f.e.project, before);
    f.e.redo();
    assert_eq!(f.e.project, after);
    let replacement = proposal(&f, Role::ControlRegion);
    f.repo
        .reject_source(&f.source, "Source rejected after preview")
        .unwrap();
    assert!(f.repo.guard_boundary_commit(&replacement).is_err());
    assert_eq!(f.e.project, after);
}
#[test]
fn editing_plot_does_not_block_control_import_but_editing_target_refuses_reimport() {
    let mut f = fixture();
    import(&mut f, Role::Site);
    let plot = f.e.project.site.boundary.as_ref().unwrap().element_id;
    apply(
        &mut f.e,
        vec![ModelCommand::MoveElements {
            ids: vec![plot],
            offset: Point::new(1., 0.),
        }],
    );
    let plot_element = f.e.project.elements[0].clone();
    assert!(atelier_bim::site::boundary::user_modified(&f.e.project).unwrap());
    import(&mut f, Role::ControlRegion);
    assert_eq!(f.e.project.elements[0], plot_element);
    let control = f.e.project.site.control_region.as_ref().unwrap().element_id;
    let p = proposal(&f, Role::ControlRegion);
    apply(
        &mut f.e,
        vec![ModelCommand::MoveElements {
            ids: vec![control],
            offset: Point::new(0., 1.),
        }],
    );
    let before = f.e.project.clone();
    assert!(
        f.repo
            .preview_reference_region(
                &before,
                &f.source,
                "A2",
                BoundaryPlacement::ModelAxes,
                "Replace".into(),
                0,
                1,
                BoundaryWidthPolicy::RejectNonzero,
                Role::ControlRegion
            )
            .unwrap_err()
            .contains("user edits")
    );
    assert!(
        f.e.prepare_change(
            f.e.revision,
            vec![ModelCommand::ApplySiteControlRegion { proposal: p }]
        )
        .is_err()
    );
    assert_eq!(f.e.project, before);
    let mut forged = before.site.clone();
    forged.control_region = None;
    assert!(
        f.e.apply(ModelCommand::SetSiteContext { context: forged })
            .is_err()
    );
    assert_eq!(f.e.project, before);
    assert!(
        f.e.prepare_change(
            f.e.revision,
            vec![ModelCommand::RemoveElements { ids: vec![control] }]
        )
        .is_err()
    );
    apply(&mut f.e, vec![ModelCommand::DetachSiteControlRegion]);
    assert!(f.e.project.site.control_region.is_none());
    assert!(f.e.project.elements.iter().any(|e| e.id == control));
    assert_eq!(f.e.project.elements[0], plot_element);
}
#[test]
fn concave_control_clearance_uses_full_region_and_is_not_plot_clearance() {
    for (x, y, limit, state, value) in [
        (4., 4., Some(2.), State::Pass, Some(2.)),
        (4., 4., Some(2.001), State::Fail, Some(2.)),
        (9., 10., Some(0.), State::Fail, None),
        (4., 4., None, State::Unknown, Some(2.)),
    ] {
        let mut f = fixture();
        import(&mut f, Role::Site);
        import(&mut f, Role::ControlRegion);
        let id = mass(&mut f.e.project, x, y);
        check(&mut f.e, id, Metric::ReferenceControlRegionClearance, limit);
        let c = &f.e.project.rules.checks[0];
        assert_eq!(c.state(&f.e.project), state);
        assert_eq!(c.report.as_ref().unwrap().rows[0].value, value);
        if x == 9. {
            let mut p = f.e.project.clone();
            p.rules.checks.clear();
            let mut e = Editor::new(p);
            check(&mut e, id, Metric::ReferenceBoundaryClearance, Some(0.));
            assert_eq!(
                e.project.rules.checks[0].state(&e.project),
                State::Pass,
                "inside plot is not inside control region"
            );
        }
    }
    let mut e = Editor::new(Project::default());
    let id = mass(&mut e.project, 0., 0.);
    check(
        &mut e,
        id,
        Metric::ReferenceControlRegionClearance,
        Some(0.),
    );
    assert_eq!(e.project.rules.checks[0].state(&e.project), State::Unknown);
}
#[test]
fn local_control_edit_expires_check_and_figure_then_refreshes_without_replacing_plot() {
    let mut f = fixture();
    import(&mut f, Role::Site);
    import(&mut f, Role::ControlRegion);
    let id = mass(&mut f.e.project, 4., 4.);
    check(
        &mut f.e,
        id,
        Metric::ReferenceControlRegionClearance,
        Some(2.),
    );
    apply(
        &mut f.e,
        vec![ModelCommand::Diagram {
            edit: diagrams::Edit::Create {
                name: "Local control feedback".into(),
                definition: diagrams::Definition {
                    story: 0,
                    option: None,
                    domain: diagrams::Domain::Site,
                    presentation: Presentation::ConstraintOverlay,
                },
            },
        }],
    );
    let before = f.e.project.clone();
    let control = before.site.control_region.as_ref().unwrap().element_id;
    let plot_id = before.site.boundary.as_ref().unwrap().element_id;
    let plot = before
        .elements
        .iter()
        .find(|e| e.id == plot_id)
        .unwrap()
        .clone();
    apply(
        &mut f.e,
        vec![ModelCommand::MoveElements {
            ids: vec![control],
            offset: Point::new(1., 0.),
        }],
    );
    assert_eq!(
        f.e.project.elements.iter().find(|e| e.id == plot_id),
        Some(&plot)
    );
    assert_eq!(
        f.e.project.rules.checks[0].state(&f.e.project),
        State::Unknown
    );
    assert_eq!(
        f.e.project.drawings.diagrams[0], before.drawings.diagrams[0],
        "stale snapshot must remain intact"
    );
    assert_eq!(
        f.e.project.drawings.diagrams[0].status(&f.e.project),
        diagrams::Status::Stale
    );
    assert!(f.e.project.drawings.diagrams[0].svg(&f.e.project).is_err());
    let changed = f.e.project.clone();
    let commands = vec![
        ModelCommand::Constraint {
            edit: Edit::Run {
                id: "control-check".into(),
                expected_sha256: constraints::expected(&f.e.project.rules.checks[0]),
            },
        },
        ModelCommand::Diagram {
            edit: diagrams::Edit::Refresh {
                id: before.drawings.diagrams[0].id,
                expected_sha256: diagrams::expected(&before.drawings.diagrams[0]),
            },
        },
    ];
    let preview = f.e.prepare_change(f.e.revision, commands.clone()).unwrap();
    assert_eq!(
        f.e.project, changed,
        "cancel refresh leaves old results and model untouched"
    );
    drop(preview);
    apply(&mut f.e, commands);
    let updated = f.e.project.clone();
    assert_eq!(
        updated.rules.checks[0].report.as_ref().unwrap().rows[0].value,
        Some(1.)
    );
    assert_eq!(updated.rules.checks[0].state(&updated), State::Fail);
    let figure = &updated.drawings.diagrams[0];
    assert_eq!(figure.id, before.drawings.diagrams[0].id);
    assert_eq!(
        figure.generation,
        before.drawings.diagrams[0].generation + 1
    );
    assert_eq!(figure.status(&updated), diagrams::Status::Current);
    assert_eq!(
        updated.elements, changed.elements,
        "refresh cannot move geometry"
    );
    f.e.undo();
    assert_eq!(f.e.project, changed);
    f.e.redo();
    assert_eq!(f.e.project, updated);
    assert_eq!(
        JsonAdapter
            .import(&JsonAdapter.export(&updated).unwrap())
            .unwrap(),
        updated
    );
}
#[test]
fn native24_preserves_detached_previous_snapshot_and_rejects_old_markers() {
    let mut f = fixture();
    import(&mut f, Role::Site);
    import(&mut f, Role::ControlRegion);
    let id = mass(&mut f.e.project, 4., 4.);
    check(
        &mut f.e,
        id,
        Metric::ReferenceControlRegionClearance,
        Some(2.),
    );
    apply(
        &mut f.e,
        vec![ModelCommand::Diagram {
            edit: diagrams::Edit::Create {
                name: "Actual reference identities".into(),
                definition: diagrams::Definition {
                    story: 0,
                    option: None,
                    domain: diagrams::Domain::Site,
                    presentation: Presentation::ConstraintOverlay,
                },
            },
        }],
    );
    let fig = f.e.project.drawings.diagrams[0].clone();
    assert_eq!(fig.snapshot.site.as_ref().unwrap().geometry_version, 2);
    let page = diagrams::site::Page::new(&fig).unwrap();
    assert!(
        page.paths
            .iter()
            .any(|p| p.kind == diagrams::site::Kind::Boundary)
    );
    assert!(
        page.paths
            .iter()
            .any(|p| p.kind == diagrams::site::Kind::ControlRegion)
    );
    assert!(
        fig.svg(&f.e.project)
            .unwrap()
            .contains("data-site-kind=\"ControlRegion\"")
    );
    assert!(
        fig.text_lines()
            .iter()
            .any(|(_, _, s, _)| s.contains("Declared control region"))
    );
    let before = f.e.project.clone();
    let bytes = JsonAdapter.export(&before).unwrap();
    let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["schema_version"], 24);
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), before);
    for version in [14, 19, 20, 22, 23] {
        json["schema_version"] = version.into();
        assert!(
            JsonAdapter
                .import(&serde_json::to_vec(&json).unwrap())
                .unwrap_err()
                .contains("version 24")
        );
    }
    apply(&mut f.e, vec![ModelCommand::DetachSiteControlRegion]);
    assert_eq!(
        f.e.project.rules.checks[0].state(&f.e.project),
        State::Unknown
    );
    assert_eq!(f.e.project.drawings.diagrams[0], fig);
    let c = f.e.project.rules.checks[0].clone();
    apply(
        &mut f.e,
        vec![ModelCommand::Constraint {
            edit: Edit::Remove {
                id: c.definition.id.clone(),
                expected_sha256: constraints::expected(&c),
            },
        }],
    );
    assert!(f.e.project.rules.checks.is_empty());
    let bytes = JsonAdapter.export(&f.e.project).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["schema_version"],
        24
    );
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), f.e.project);
}
#[test]
fn discovered_and_native_control_import_share_the_same_command_and_schema() {
    use atelier_bim::automation_registry::{DiscoveredBatch, NativeBatch};
    let mut f = fixture();
    import(&mut f, Role::Site);
    let proposal = proposal(&f, Role::ControlRegion);
    let v = serde_json::json!({"version":1,"edits":[{"operation":"apply_site_control_region","proposal":proposal}]});
    let a: NativeBatch = serde_json::from_value(v.clone()).unwrap();
    let b: DiscoveredBatch = serde_json::from_value(v).unwrap();
    let da = a.lower(&f.e).unwrap();
    let db = b.resolve().unwrap().lower(&f.e).unwrap();
    assert_eq!(
        serde_json::to_value(&da).unwrap(),
        serde_json::to_value(&db).unwrap()
    );
    let prepared = f.e.prepare_batch_change(f.e.revision, &da).unwrap();
    assert!(prepared.project().site.control_region.is_some());
    for name in ["apply_site_control_region", "detach_site_control_region"] {
        assert!(atelier_bim::automation_registry::describe(name).is_some());
    }
}

#[test]
fn proposed_control_clearance_matches_run_inside_outside_missing_and_keeps_saved_report() {
    let mut f = fixture();
    import(&mut f, Role::ControlRegion);
    let id = mass(&mut f.e.project, 4., 3.);
    check(
        &mut f.e,
        id,
        Metric::ReferenceControlRegionClearance,
        Some(0.),
    );
    let baseline = f.e.project.clone();
    let definition = &baseline.rules.checks[0].definition;
    let initial = constraints::preview(&baseline, definition);
    assert_eq!(initial.state, State::Pass);
    assert!(initial.rows[0].value.unwrap() > 0.);
    let change =
        f.e.prepare_change(
            f.e.revision,
            vec![ModelCommand::MoveElements {
                ids: vec![id],
                offset: Point::new(40., 0.),
            }],
        )
        .unwrap();
    let previews = change.declared_check_previews();
    assert_eq!(previews.len(), 1);
    assert_eq!(previews[0].state, State::Fail);
    assert_eq!(previews[0].rows[0].value, None);
    assert!(previews[0].rows[0].reason.contains("outside"));
    assert_eq!(f.e.project, baseline);
    assert_eq!(change.project().rules, baseline.rules);
    let run = constraints::apply(
        change.project(),
        &Edit::Run {
            id: definition.id.clone(),
            expected_sha256: constraints::expected(&baseline.rules.checks[0]),
        },
    )
    .unwrap();
    assert_eq!(previews[0].rows, run[0].report.as_ref().unwrap().rows);
    let mut absent = baseline.clone();
    absent.site.control_region = None;
    let v = constraints::preview(&absent, definition);
    assert_eq!(v.state, State::Unknown);
    assert_eq!(v.rows[0].value, None);
}
