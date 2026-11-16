use atelier_bim::{
    command::ModelCommand,
    editor::{Editor, Tool},
    geometry,
    layers::ElementLayer,
    model::{ArcSegment, Point, Project, Shape, SlabContour},
    slab_creation::{SlabDraft, SlabGeometryMethod},
    slab_geometry, storage,
};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
fn contour(points: &[(f64, f64)]) -> SlabContour {
    SlabContour {
        points: points.iter().map(|&(x, y)| p(x, y)).collect(),
        ..Default::default()
    }
}

fn redo_payload(e: &Editor) -> Vec<(ModelCommand, ModelCommand)> {
    e.redo
        .iter()
        .map(|r| (r.command.clone(), r.inverse.clone()))
        .collect()
}

#[test]
fn rotated_slab_three_points_preview_exact_footprint_height_history_and_disk() {
    let mut e = Editor::new(Project::default());
    e.story = 1;
    e.thickness = 0.3;
    e.slab_method = SlabGeometryMethod::RotatedRectangle;
    e.set_tool(Tool::Slab);
    let before = e.project.clone();
    e.click(p(0., 0.), 0.);
    e.click(p(3., 4.), 0.);
    assert_eq!(e.project, before);
    assert!(e.undo.is_empty());
    let preview = e.rotated_slab_preview(p(7., 1.)).unwrap();
    assert!((preview.area() - 25.).abs() < 1e-4);
    e.click(p(7., 1.), 0.);
    assert!(e.slab_draft.is_none());
    let slab = &e.project.elements[0];
    assert_eq!(
        slab.slab_contour.as_ref().unwrap().points,
        vec![p(0., 0.), p(3., 4.), p(7., 1.), p(4., -3.)]
    );
    assert_eq!(
        slab.shape,
        Shape::Slab {
            min: p(0., -3.),
            max: p(7., 4.),
            thickness: 0.3
        }
    );
    assert!((slab.area() - 25.).abs() < 1e-4);
    assert_eq!(
        geometry::hit_plan(&e.project, 1, p(3.5, 0.5), 0.01),
        Some(slab.id)
    );
    assert_eq!(
        geometry::hit_plan(&e.project, 1, p(0.2, 3.8), 0.01),
        None,
        "the bounding box is not the slab"
    );
    let corners = geometry::corners(&e.project, slab);
    assert!((corners[0].z - 2.9).abs() < 1e-4);
    assert!((corners[4].z - 3.2).abs() < 1e-4);
    let after = e.project.clone();
    assert_eq!(e.undo.len(), 1);
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project, after);
    let file = std::env::temp_dir().join(format!("oa-slab-creation-{}.json", std::process::id()));
    storage::save(&after, &file).unwrap();
    assert_eq!(storage::load(&file).unwrap(), after);
    std::fs::remove_file(file).unwrap();
}

#[test]
fn shared_slab_creation_accepts_concave_contour_hole_and_exact_arc() {
    let mut e = Editor::new(Project::default());
    let mut c = contour(&[(0., 0.), (6., 0.), (6., 2.), (2., 2.), (2., 6.), (0., 6.)]);
    c.holes
        .push(contour(&[(0.5, 0.5), (1.5, 0.5), (1.5, 1.5), (0.5, 1.5)]));
    let id = e.place_slab_contour(c.clone()).unwrap();
    let slab = &e.project.elements[0];
    assert_eq!(slab.id, id);
    assert_eq!(slab.slab_contour.as_ref(), Some(&c));
    assert!((slab.area() - 19.).abs() < 1e-4);
    let triangle_area: f64 = slab_geometry::triangles(slab)
        .iter()
        .map(|t| slab_geometry::area(t))
        .sum();
    assert!((triangle_area - 19.).abs() < 1e-4);
    assert_eq!(
        geometry::hit_plan(&e.project, 0, p(1., 1.), 0.01),
        None,
        "hole stays empty"
    );
    assert_eq!(
        geometry::hit_plan(&e.project, 0, p(4., 4.), 0.01),
        None,
        "concavity stays empty"
    );
    let mut curved = contour(&[(10., 0.), (14., 0.), (14., 4.), (10., 4.)]);
    curved.arcs = vec![
        None,
        Some(ArcSegment {
            radius: 2.,
            clockwise: false,
            large_arc: false,
        }),
        None,
        None,
    ];
    let id = e.place_slab_contour(curved.clone()).unwrap();
    let arc = e.project.elements.iter().find(|x| x.id == id).unwrap();
    assert_eq!(arc.slab_contour.as_ref(), Some(&curved));
    assert!((arc.area() - (16. + 2. * std::f64::consts::PI)).abs() < 1e-4);
    assert!(matches!(arc.shape, Shape::Slab { max, .. } if (max.x-16.).abs()<1e-4));
    let encoded = serde_json::to_vec(&e.project).unwrap();
    assert_eq!(
        serde_json::from_slice::<Project>(&encoded).unwrap(),
        e.project
    );
    e.project.validate().unwrap();
}

#[test]
fn slab_creation_rejections_preserve_draft_identity_and_nonempty_redo() {
    let mut e = Editor::new(Project::default());
    e.place_column(p(20., 20.)).unwrap();
    e.undo();
    let initial = e.project.clone();
    let revision = e.revision;
    let redo = redo_payload(&e);
    assert!(
        e.place_slab_contour(contour(&[(0., 0.), (3., 3.), (0., 3.), (3., 0.)]))
            .is_err()
    );
    assert_eq!(e.project, initial);
    assert_eq!(e.revision, revision);
    assert_eq!(redo_payload(&e), redo);
    e.slab_method = SlabGeometryMethod::RotatedRectangle;
    e.set_tool(Tool::Slab);
    e.click(p(0., 0.), 0.);
    e.click(p(0., 0.), 0.);
    assert_eq!(
        e.slab_draft,
        Some(SlabDraft::BaselineEnd { start: p(0., 0.) })
    );
    e.click(p(3., 4.), 0.);
    let draft = e.slab_draft;
    e.click(p(6., 8.), 0.);
    assert_eq!(e.slab_draft, draft);
    assert_eq!(e.project, initial);
    assert_eq!(redo_payload(&e), redo);
    let layer = e
        .project
        .layers
        .layers
        .iter_mut()
        .find(|l| l.id != 0)
        .unwrap();
    layer.status.locked = true;
    e.slab_layer = ElementLayer(layer.id);
    let locked = e.project.clone();
    e.click(p(7., 1.), 0.);
    assert_eq!(e.project, locked);
    assert_eq!(redo_payload(&e), redo);
    assert_eq!(e.slab_draft, draft);
    assert!(!e.status.starts_with("Created"));
    e.project
        .layers
        .layers
        .iter_mut()
        .find(|l| l.id == e.slab_layer.0)
        .unwrap()
        .status
        .locked = false;
    e.click(p(7., 1.), 0.);
    assert_eq!(e.project.elements.len(), 1);
    assert!(e.slab_draft.is_none());
    let mut exhausted = Editor::new(Project::default());
    exhausted.project.next_id = u64::MAX;
    let before = exhausted.project.clone();
    assert!(
        exhausted
            .place_slab_contour(contour(&[(0., 0.), (2., 0.), (2., 2.), (0., 2.)]))
            .is_err()
    );
    assert_eq!(exhausted.project, before);
}

#[test]
fn rectangular_slab_keeps_legacy_shape_and_rejects_locked_layer_atomically() {
    let mut e = Editor::new(Project::default());
    e.set_tool(Tool::Slab);
    e.click(p(0., 0.), 0.);
    e.click(p(4., 3.), 0.);
    assert!(e.project.elements[0].slab_contour.is_none());
    assert_eq!(
        e.project.elements[0].shape,
        Shape::Slab {
            min: p(0., 0.),
            max: p(4., 3.),
            thickness: 0.22
        }
    );
    e.undo();
    let redo = redo_payload(&e);
    let layer = e
        .project
        .layers
        .layers
        .iter_mut()
        .find(|l| l.id != 0)
        .unwrap();
    layer.status.locked = true;
    e.slab_layer = ElementLayer(layer.id);
    let before = e.project.clone();
    e.click(p(0., 0.), 0.);
    e.click(p(4., 3.), 0.);
    assert_eq!(e.project, before);
    assert_eq!(redo_payload(&e), redo);
    assert_eq!(e.anchor, Some(p(0., 0.)));
    assert!(!e.status.starts_with("Created"));
}

#[test]
fn slab_draft_cleanup_covers_cancel_tool_story_worksheet_undo_and_redo() {
    for action in 0..6 {
        let mut e = Editor::new(Project::default());
        e.slab_method = SlabGeometryMethod::RotatedRectangle;
        e.set_tool(Tool::Slab);
        e.click(p(0., 0.), 0.);
        e.click(p(3., 4.), 0.);
        let before = e.project.clone();
        match action {
            0 => e.cancel_wall_draft(),
            1 => e.set_tool(Tool::Wall),
            2 => e.switch_story(1),
            3 => e.switch_worksheet(None),
            4 => e.undo(),
            _ => e.redo(),
        }
        assert!(e.slab_draft.is_none(), "action{action}");
        assert_eq!(e.project, before);
    }
}

#[test]
fn rejected_slab_on_locked_layer_preserves_actual_redo_result() {
    let mut e = Editor::new(Project::default());
    let layer = e
        .project
        .layers
        .layers
        .iter_mut()
        .find(|l| l.id != 0)
        .unwrap();
    layer.status.locked = true;
    e.slab_layer = ElementLayer(layer.id);
    e.place_column(p(20., 20.)).unwrap();
    let with_column = e.project.clone();
    e.undo();
    let before = e.project.clone();
    assert!(
        e.place_slab_contour(contour(&[(0., 0.), (4., 0.), (4., 3.), (0., 3.)]))
            .is_err()
    );
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project, with_column);
}

#[test]
fn slab_creation_offsets_keep_story_relative_surfaces_history_and_json() {
    for method in [
        SlabGeometryMethod::Rectangular,
        SlabGeometryMethod::RotatedRectangle,
    ] {
        for offset in [0., -0.75, 1.25] {
            let mut e = Editor::new(Project::default());
            e.story = 1;
            e.thickness = 0.3;
            e.slab_elevation = offset;
            e.slab_method = method;
            e.set_tool(Tool::Slab);
            let before = e.project.clone();
            e.click(p(0., 0.), 0.);
            if method == SlabGeometryMethod::RotatedRectangle {
                e.click(p(3., 4.), 0.);
                let preview = e.rotated_slab_preview(p(7., 1.)).unwrap();
                assert_eq!(slab_geometry::contour(&preview).elevation, offset);
            }
            e.click(p(7., 1.), 0.);
            assert_eq!(e.project.elements.len(), 1, "{}", e.status);
            let slab = &e.project.elements[0];
            assert_eq!(slab_geometry::contour(slab).elevation, offset);
            if method == SlabGeometryMethod::Rectangular {
                assert_eq!(slab.slab_contour.is_none(), offset == 0.);
            }
            let triangles = slab_geometry::cap_triangles(slab, e.project.elevation(1)).unwrap();
            let zs: Vec<_> = triangles.iter().flatten().map(|v| v.z).collect();
            let top = e.project.elevation(1) + offset;
            assert!(
                zs.iter()
                    .all(|z| (*z - top).abs() < 1e-5 || (*z - (top - 0.3)).abs() < 1e-5)
            );
            assert!(zs.iter().any(|z| (*z - top).abs() < 1e-5));
            assert!(zs.iter().any(|z| (*z - (top - 0.3)).abs() < 1e-5));
            let after = e.project.clone();
            let encoded = serde_json::to_string(&after).unwrap();
            let decoded: Project = serde_json::from_str(&encoded).unwrap();
            assert_eq!(decoded, after);
            e.undo();
            assert_eq!(e.project, before);
            e.redo();
            assert_eq!(e.project, after);
        }
    }
}

#[test]
fn slab_creation_invalid_offset_preserves_redo_and_explicit_api_offset_wins() {
    for method in [
        SlabGeometryMethod::Rectangular,
        SlabGeometryMethod::RotatedRectangle,
    ] {
        let mut e = Editor::new(Project::default());
        e.place_column(p(20., 20.)).unwrap();
        let with_column = e.project.clone();
        e.undo();
        let before = e.project.clone();
        let redo = redo_payload(&e);
        e.slab_method = method;
        e.slab_elevation = f64::NAN;
        e.set_tool(Tool::Slab);
        e.click(p(0., 0.), 0.);
        if method == SlabGeometryMethod::RotatedRectangle {
            e.click(p(3., 4.), 0.);
        }
        e.click(p(7., 1.), 0.);
        assert_eq!(e.project, before);
        assert_eq!(redo_payload(&e), redo);
        assert!(!e.status.is_empty());
        e.redo();
        assert_eq!(e.project, with_column);
        let mut c = contour(&[(0., 0.), (2., 0.), (2., 2.), (0., 2.)]);
        c.elevation = -2.;
        let id = e.place_slab_contour(c).unwrap();
        assert_eq!(
            slab_geometry::contour(e.project.elements.iter().find(|v| v.id == id).unwrap())
                .elevation,
            -2.
        );
    }
}

#[test]
fn rotated_slab_numeric_targets_share_signed_width_geometry() {
    use atelier_bim::slab_creation::SlabDraft;
    for angle in [-140_f64, -30., 0., 45., 135.] {
        for width in [-2_f64, 2.] {
            let mut e = Editor::new(Project::default());
            let start = Point::new(2., -3.);
            let baseline = SlabDraft::BaselineEnd { start };
            let end = baseline.numeric_target(5., angle).unwrap();
            assert!((start.distance(end) - 5.).abs() < 1e-5);
            let draft = SlabDraft::Width { start, end };
            let target = draft.numeric_target(width, f64::NAN).unwrap();
            let values = draft.numeric_values(target);
            assert!((values.0 - width).abs() < 1e-5);
            e.slab_draft = Some(draft);
            e.slab_elevation = 1.25;
            let preview = e.rotated_slab_preview(target).unwrap();
            assert!((preview.area() - 10.).abs() < 1e-4);
            assert_eq!(preview.slab_contour.as_ref().unwrap().elevation, 1.25);
            let before = e.project.clone();
            e.place_slab_contour(preview.slab_contour.clone().unwrap())
                .unwrap();
            let after = e.project.clone();
            e.undo();
            assert_eq!(e.project, before);
            e.redo();
            assert_eq!(e.project, after);
        }
    }
}
#[test]
fn rotated_slab_numeric_invalid_values_do_not_produce_candidates() {
    use atelier_bim::slab_creation::SlabDraft;
    let a = SlabDraft::BaselineEnd {
        start: Point::default(),
    };
    for value in [f64::NAN, f64::INFINITY, -1., 0., 0.001, 100000.] {
        assert!(a.numeric_target(value, 0.).is_err());
    }
    assert!(a.numeric_target(5., f64::NAN).is_err());
    let b = SlabDraft::Width {
        start: Point::default(),
        end: Point::new(5., 0.),
    };
    for width in [f64::NAN, f64::INFINITY, 0., 0.001, -0.001, 100000.] {
        assert!(b.numeric_target(width, 0.).is_err());
    }
}

#[test]
fn rectangular_preview_matches_pointer_commit_without_consuming_redo() {
    use atelier_bim::{model::Material, view_settings::RenovationStatus};
    for elevation in [0., 1.25, -0.75] {
        for reversed in [false, true] {
            let mut e = Editor::new(Project::default());
            e.place_column(p(9., 9.)).unwrap();
            let redo_project = e.project.clone();
            e.undo();
            e.story = 1;
            e.material = Material::Timber;
            e.wall_renovation = RenovationStatus::Existing;
            e.slab_layer = ElementLayer(
                e.project
                    .layers
                    .layers
                    .iter()
                    .find(|l| l.id != 0 && !l.status.locked)
                    .unwrap()
                    .id,
            );
            e.slab_elevation = elevation;
            e.thickness = 0.45;
            let (a, b) = if reversed {
                (p(6., 4.), p(0., 0.))
            } else {
                (p(0., 0.), p(6., 4.))
            };
            let before = e.project.clone();
            let revision = e.revision;
            let selection = e.selection_ids();
            let preview = e.rectangular_slab_preview(a, b).unwrap();
            assert_eq!(preview.layer, e.slab_layer);
            assert_eq!(preview.material, e.material);
            assert_eq!(preview.story, 1);
            assert_eq!(preview.renovation, e.wall_renovation);
            assert_eq!(preview.slab_contour.is_none(), elevation == 0.);
            assert_eq!(slab_geometry::contour(&preview).elevation, elevation);
            assert!(e.rectangular_slab_preview(a, a).is_err());
            assert_eq!(e.project, before);
            assert_eq!(e.revision, revision);
            assert_eq!(e.selection_ids(), selection);
            e.redo();
            assert_eq!(e.project, redo_project);
            e.undo();
            assert_eq!(e.project, before);
            let expected = e
                .preview_add_elements(vec![preview.clone()], false)
                .unwrap();
            e.set_tool(Tool::Slab);
            e.click(a, 0.);
            e.click(b, 0.);
            assert_eq!(e.project, expected);
            assert_eq!(e.project.elements.last(), Some(&preview));
            let loaded: Project =
                serde_json::from_str(&serde_json::to_string(&e.project).unwrap()).unwrap();
            loaded.validate().unwrap();
            assert_eq!(loaded, e.project);
            e.undo();
            assert_eq!(e.project, before);
            e.redo();
            assert_eq!(e.project, expected);
        }
    }
}
