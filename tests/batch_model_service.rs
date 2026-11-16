use atelier_bim::{
    column::ColumnSection,
    editor::Editor,
    model::{ArcSegment, Material, ModelAdapter, OpeningKind, Point, Project, Shape, SlabContour},
    model_service::{
        BatchChange, ElementReference as R, ElementTemplate, NamedElement, ReferencedCommand as C,
    },
    storage::JsonAdapter,
};
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
fn template(shape: Shape) -> ElementTemplate {
    let mut project = Project::default();
    project.add(0, Material::Concrete, shape);
    let mut element = project.elements.pop().unwrap();
    element.id = 0;
    ElementTemplate {
        element,
        host: None,
    }
}
fn t(name: &str) -> R {
    R::Temporary(name.into())
}
fn eight() -> BatchChange {
    let mut opening = template(Shape::Opening {
        host_wall: 0,
        kind: OpeningKind::Window,
        width: 1.,
        height: 1.,
        sill: 1.,
        offset: 2.,
    });
    opening.host = Some(t("wall"));
    let wall = template(Shape::Wall {
        start: p(0., 0.),
        end: p(4., 0.),
        thickness: 0.3,
        height: 3.,
    });
    let mut slab = template(Shape::Slab {
        min: p(0., 1.),
        max: p(4., 5.),
        thickness: 0.3,
    });
    slab.element.slab_contour = Some(SlabContour {
        points: vec![p(0., 1.), p(4., 1.), p(4., 5.), p(0., 5.)],
        holes: vec![SlabContour {
            points: vec![p(1., 2.), p(2., 2.), p(2., 3.), p(1., 3.)],
            ..Default::default()
        }],
        ..Default::default()
    });
    let column = template(Shape::Column {
        center: p(6., 2.),
        section: ColumnSection::Circular { diameter: 0.5 },
        height: 3.,
        bottom_offset: 0.,
        axis_rotation: 30.,
        inclination: Some(atelier_bim::column_inclination::ColumnInclination {
            slant_degrees: 75.,
            direction_degrees: 30.,
        }),
        plan: Default::default(),
    });
    let beam = template(Shape::Beam {
        start: p(0., 6.),
        end: p(4., 6.),
        width: 0.3,
        height: 0.5,
        top_offset: 3.,
        slant_angle: 20.,
        arc: None,
    });
    let roof = template(Shape::Roof {
        spec: atelier_bim::roof::RoofSpec {
            contour: None,
            pivot_start: p(0., 1.),
            pivot_end: p(4., 1.),
            rise_left: true,
            min: p(0., 1.),
            max: p(4., 5.),
            pitch: 25.,
            thickness: 0.2,
            pivot_offset: 3.,
        },
    });
    let stair = template(Shape::Stair {
        spec: Default::default(),
    });
    let line = template(Shape::Polyline {
        points: vec![p(0., 8.), p(2., 8.)],
        arcs: vec![Some(ArcSegment {
            radius: 1.,
            clockwise: false,
            large_arc: false,
        })],
    });
    BatchChange {
        creates: vec![
            ("opening", opening),
            ("wall", wall),
            ("slab", slab),
            ("column", column),
            ("beam", beam),
            ("roof", roof),
            ("stair", stair),
            ("line", line),
        ]
        .into_iter()
        .map(|(name, template)| NamedElement {
            name: name.into(),
            template,
        })
        .collect(),
        ..Default::default()
    }
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
fn symbolic_forward_host_eight_types_update_move_remove_and_file_use_real_atomic_history() {
    let mut e = Editor::new(Project::default());
    let initial = e.project.clone();
    let before = state(&e);
    let request = eight();
    assert_eq!(
        serde_json::from_str::<BatchChange>(&serde_json::to_string(&request).unwrap()).unwrap(),
        request
    );
    let prepared = e.prepare_batch_change(0, &request).unwrap();
    assert_eq!(before, state(&e));
    assert_eq!(prepared.resolved_references()["opening"], 1);
    assert_eq!(prepared.resolved_references()["wall"], 2);
    let expected = prepared.project().clone();
    let receipt = e.commit_change("eight", &prepared).unwrap();
    assert_eq!(
        receipt.resolved_references,
        prepared.resolved_references().clone()
    );
    assert_eq!(receipt.changed_ids.len(), 8);
    assert_eq!(e.undo.len(), 1);
    assert_eq!(e.project, expected);
    assert_eq!(
        JsonAdapter
            .import(&JsonAdapter.export(&expected).unwrap())
            .unwrap(),
        expected
    );
    e.undo();
    assert_eq!(e.project, initial);
    e.redo();
    assert_eq!(e.project, expected);
    let mut edits = Vec::new();
    for current in &e.project.elements {
        let mut element = current.clone();
        element.id = 0;
        element.name.push_str(" updated");
        let mut host = None;
        match &mut element.shape {
            Shape::ConceptMass { spec } => spec.height += 0.1,
            Shape::Zone { spec } => spec.height += 0.1,
            Shape::Wall { height, .. }
            | Shape::Column { height, .. }
            | Shape::Beam { height, .. } => *height += 0.1,
            Shape::Opening {
                host_wall, width, ..
            } => {
                host = Some(R::Existing(*host_wall));
                *host_wall = 0;
                *width = 0.9;
            }
            Shape::Slab { thickness, .. } => *thickness += 0.05,
            Shape::Roof { spec } => spec.pitch = 30.,
            Shape::Stair { spec } => spec.width = 1.2,
            Shape::Polyline { arcs, .. } => arcs[0].as_mut().unwrap().clockwise = true,
        }
        edits.push(C::Update {
            target: R::Existing(current.id),
            template: ElementTemplate { element, host },
        });
    }
    let updated = e
        .prepare_batch_change(
            e.revision,
            &BatchChange {
                edits,
                ..Default::default()
            },
        )
        .unwrap();
    let updated_model = updated.project().clone();
    e.commit_change("update-eight", &updated).unwrap();
    assert_eq!(e.undo.len(), 2);
    assert_eq!(updated.difference().affected_ids().len(), 8);
    e.undo();
    assert_eq!(e.project, expected);
    e.redo();
    assert_eq!(e.project, updated_model);
    let moved = e
        .prepare_batch_change(
            e.revision,
            &BatchChange {
                edits: vec![C::Move {
                    targets: e
                        .project
                        .elements
                        .iter()
                        .map(|x| R::Existing(x.id))
                        .collect(),
                    offset: p(1000., -1000.),
                }],
                ..Default::default()
            },
        )
        .unwrap();
    e.commit_change("move-eight", &moved).unwrap();
    let translated = e.project.clone();
    assert_ne!(translated, updated_model);
    e.undo();
    assert_eq!(e.project, updated_model);
    e.redo();
    assert_eq!(e.project, translated);
    let deleted = e
        .prepare_batch_change(
            e.revision,
            &BatchChange {
                edits: vec![C::Remove {
                    targets: vec![R::Existing(receipt.resolved_references["wall"])],
                }],
                ..Default::default()
            },
        )
        .unwrap();
    e.commit_change("remove-host", &deleted).unwrap();
    assert_eq!(e.project.elements.len(), 6);
    e.undo();
    assert_eq!(e.project, translated);
    e.redo();
    assert_eq!(e.project.elements.len(), 6);
    assert_eq!(
        JsonAdapter
            .import(&JsonAdapter.export(&e.project).unwrap())
            .unwrap(),
        e.project
    );
}
#[test]
fn symbolic_validation_late_failure_cancel_and_recovery_preserve_real_redo_and_ids() {
    let mut e = Editor::new(Project::default());
    e.place_column(p(20., 20.)).unwrap();
    let redo = e.project.clone();
    e.undo();
    let before = state(&e);
    let mut request = eight();
    request.edits.push(C::Move {
        targets: vec![t("wall")],
        offset: p(f64::NAN, 0.),
    });
    assert_eq!(
        e.prepare_batch_change(e.revision, &request)
            .err()
            .unwrap()
            .code,
        "VALIDATION_FAILED"
    );
    assert_eq!(state(&e), before);
    request.edits.clear();
    let cancelled = e.prepare_batch_change(e.revision, &request).unwrap();
    drop(cancelled);
    assert_eq!(state(&e), before);
    e.redo();
    assert_eq!(e.project, redo);
    e.undo();
    let prepared = e.prepare_batch_change(e.revision, &request).unwrap();
    e.commit_change("recover", &prepared).unwrap();
    assert_eq!(e.project.elements.len(), 8);
}
#[test]
fn symbolic_names_missing_references_hosts_templates_and_versions_are_rejected_atomically() {
    let e = Editor::new(Project::default());
    let before = state(&e);
    let check = |request: BatchChange, code: &str| {
        assert_eq!(
            e.prepare_batch_change(e.revision, &request)
                .err()
                .unwrap()
                .code,
            code
        );
        assert_eq!(state(&e), before);
    };
    let mut r = eight();
    r.version = 999;
    check(r, "UNSUPPORTED_BATCH_VERSION");
    let mut r = eight();
    r.creates[1].name = "opening".into();
    check(r, "DUPLICATE_TEMPORARY_NAME");
    for bad in ["", "has space", "中文", &"a".repeat(65)] {
        let mut r = eight();
        r.creates[1].name = bad.into();
        check(r, "INVALID_TEMPORARY_NAME");
    }
    let mut r = eight();
    r.creates[0].template.host = Some(t("missing"));
    check(r, "REFERENCE_NOT_FOUND");
    let mut r = eight();
    r.creates[0].template.host = Some(R::Existing(2));
    check(r, "REFERENCE_NOT_FOUND");
    let mut r = eight();
    r.creates[0].template.host = None;
    check(r, "INVALID_HOST_REFERENCE");
    let mut r = eight();
    r.creates[1].template.host = Some(t("wall"));
    check(r, "INVALID_HOST_REFERENCE");
    let mut r = eight();
    r.creates[1].template.element.id = 42;
    check(r, "INVALID_TEMPLATE_ID");
    for name in ["opening", "line"] {
        let mut r = eight();
        r.creates[0].template.host = Some(t(name));
        check(r, "VALIDATION_FAILED");
    }
    let mut r = eight();
    r.edits.push(C::Remove {
        targets: vec![t("unknown")],
    });
    check(r, "REFERENCE_NOT_FOUND");
    check(BatchChange::default(), "NO_CHANGE");
}
#[test]
fn symbolic_retry_identity_keeps_original_names_payload_and_undone_state() {
    let mut e = Editor::new(Project::default());
    let r = eight();
    let first = e.prepare_batch_change(0, &r).unwrap();
    let same = e.prepare_batch_change(0, &r).unwrap();
    let mut changed = r.clone();
    changed.creates[7].name = "renamed".into();
    let other = e.prepare_batch_change(0, &changed).unwrap();
    assert_eq!(first.project(), other.project());
    let receipt = e.commit_change("retry", &first).unwrap();
    let before = state(&e);
    assert_eq!(e.change_receipt("retry"), Some(&receipt));
    assert_eq!(e.commit_change("retry", &same).unwrap(), receipt);
    assert_eq!(state(&e), before);
    assert_eq!(
        e.commit_change("retry", &other).unwrap_err().code,
        "REQUEST_ID_CONFLICT"
    );
    assert_eq!(state(&e), before);
    e.undo();
    let before = state(&e);
    assert_eq!(e.commit_change("retry", &same).unwrap(), receipt);
    assert_eq!(state(&e), before);
    e.redo();
    assert_eq!(&e.project, first.project());
    let mut foreign = Editor::new(e.project.clone());
    assert_eq!(
        foreign.commit_change("retry", &first).unwrap_err().code,
        "DOCUMENT_CONFLICT"
    );
}
#[test]
fn symbolic_existing_locked_host_id_exhaustion_and_stale_baselines_do_not_write() {
    let mut e = Editor::new(Project::default());
    e.place_wall_segments(vec![(p(0., 0.), p(4., 0.))]).unwrap();
    let layer = e.project.layers.new_layer("locked host", None);
    e.project.elements[0].layer = atelier_bim::layers::ElementLayer(layer);
    e.project.layers.layer_mut(layer).unwrap().status.locked = true;
    let mut r = eight();
    r.creates.truncate(1);
    r.creates[0].template.host = Some(R::Existing(1));
    let before = state(&e);
    assert!(e.prepare_batch_change(e.revision, &r).is_err());
    assert_eq!(state(&e), before);
    e.project.layers.layer_mut(layer).unwrap().status.locked = false;
    let prepared = e.prepare_batch_change(e.revision, &r).unwrap();
    e.place_column(p(10., 10.)).unwrap();
    let before = state(&e);
    assert_eq!(
        e.commit_change("stale", &prepared).unwrap_err().code,
        "REVISION_CONFLICT"
    );
    assert_eq!(state(&e), before);
    assert_eq!(
        e.prepare_batch_change(0, &r).err().unwrap().code,
        "REVISION_CONFLICT"
    );
    let mut bounded = Editor::new(Project::default());
    bounded.project.next_id = u64::MAX;
    let before = state(&bounded);
    assert_eq!(
        bounded
            .prepare_batch_change(0, &eight())
            .err()
            .unwrap()
            .code,
        "ID_EXHAUSTED"
    );
    assert_eq!(state(&bounded), before);
}
