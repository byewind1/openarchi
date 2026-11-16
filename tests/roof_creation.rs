use atelier_bim::{
    editor::Editor,
    model::{Point, Project, Shape},
    roof::RoofDraft,
    roof_creation::RoofPolygonDraft,
};
fn draft() -> RoofPolygonDraft {
    RoofPolygonDraft::from_contour_start(RoofDraft::ContourStart {
        start: Point::new(-2., 1.),
        end: Point::new(8., 1.),
        rise_left: true,
    })
    .unwrap()
}
fn points() -> Vec<Point> {
    [(0., 0.), (6., 0.), (6., 2.), (2., 2.), (2., 6.), (0., 6.)]
        .map(|(x, y)| Point::new(x, y))
        .to_vec()
}

#[test]
fn regularized_roof_regions_keep_pivot_holes_and_explicit_selection_history() {
    let cases = [
        (vec![(0., 0.), (4., 4.), (0., 4.), (4., 0.)], 2, 0, 8.),
        (
            vec![
                (0., 0.),
                (10., 0.),
                (10., 10.),
                (0., 10.),
                (0., 0.),
                (2., 2.),
                (2., 8.),
                (8., 8.),
                (8., 2.),
                (2., 2.),
                (0., 0.),
            ],
            1,
            1,
            64.,
        ),
    ];
    for (path, count, holes, area) in cases {
        for chosen in 0..count {
            let mut editor = Editor::new(Project::default());
            editor.set_tool(atelier_bim::editor::Tool::Roof);
            for p in [Point::new(-2., 1.), Point::new(8., 1.), Point::new(0., 3.)] {
                editor.click(p, 0.01);
            }
            editor.begin_roof_polygon().unwrap();
            editor.roof_size = [30., 0.25, 2.5];
            let last = path.len() - 1;
            for &(x, y) in &path[..last] {
                editor.accept_roof_polygon_vertex(Point::new(x, y)).unwrap();
            }
            let frozen = editor.roof_polygon_draft().unwrap().clone();
            let cursor = Point::new(path[last].0, path[last].1);
            let preview = editor.roof_polygon_candidates(Some(cursor)).unwrap();
            assert_eq!(editor.roof_polygon_draft(), Some(&frozen));
            editor.accept_roof_polygon_vertex(cursor).unwrap();
            let regions = editor.roof_polygon_candidates(None).unwrap();
            assert_eq!(preview, regions);
            assert_eq!(regions.len(), count);
            assert!(
                (regions
                    .iter()
                    .map(|r| r.footprint_contour().area())
                    .sum::<f64>()
                    - area)
                    .abs()
                    < 1e-6
            );
            for spec in &regions {
                spec.validate().unwrap();
                assert_eq!(spec.pivot_start, Point::new(-2., 1.));
                assert_eq!(spec.pivot_end, Point::new(8., 1.));
                assert!(spec.rise_left);
                assert_eq!(spec.contour.as_ref().unwrap().holes.len(), holes);
                assert!(
                    (spec.bottom(Point::new(0., 3.), 0.) - (2.5 + 2. * 30_f64.to_radians().tan()))
                        .abs()
                        < 1e-5
                );
            }
            let before = editor.project.clone();
            let draft = editor.roof_polygon_draft().cloned();
            editor.roof_size[0] = 20.;
            assert!(editor.commit_roof_polygon_region(&regions[chosen]).is_err());
            assert_eq!(editor.project, before);
            assert_eq!(editor.roof_polygon_draft(), draft.as_ref());
            assert!(editor.undo.is_empty());
            editor.roof_size[0] = 30.;
            editor.commit_roof_polygon_region(&regions[chosen]).unwrap();
            assert_eq!(editor.undo.len(), 1);
            assert!(editor.roof_polygon_draft().is_none() && editor.anchor.is_none());
            assert!(
                matches!(&editor.project.elements[0].shape, Shape::Roof{spec} if *spec == regions[chosen])
            );
            let created = editor.project.clone();
            let loaded: Project =
                serde_json::from_str(&serde_json::to_string(&created).unwrap()).unwrap();
            loaded.validate().unwrap();
            assert_eq!(created, loaded);
            editor.undo();
            assert_eq!(editor.project, before);
            editor.redo();
            assert_eq!(editor.project, created);
        }
    }
}

#[test]
fn stale_regularized_roof_region_does_not_consume_real_redo() {
    let mut editor = Editor::new(Project::default());
    editor.place_column(Point::new(20., 20.)).unwrap();
    let redo = editor.project.clone();
    editor.undo();
    editor.set_tool(atelier_bim::editor::Tool::Roof);
    for p in [Point::new(-2., 1.), Point::new(8., 1.), Point::new(0., 3.)] {
        editor.click(p, 0.01);
    }
    editor.begin_roof_polygon().unwrap();
    for p in [
        Point::new(0., 0.),
        Point::new(4., 4.),
        Point::new(0., 4.),
        Point::new(4., 0.),
    ] {
        editor.accept_roof_polygon_vertex(p).unwrap();
    }
    let mut region = editor.roof_polygon_candidates(None).unwrap().remove(0);
    region.pivot_offset += 1.;
    let before = editor.project.clone();
    let draft = editor.roof_polygon_draft().cloned();
    let revision = editor.revision;
    let selected = editor.selected;
    assert!(editor.commit_roof_polygon_region(&region).is_err());
    assert_eq!(editor.project, before);
    assert_eq!(editor.roof_polygon_draft(), draft.as_ref());
    assert_eq!(editor.revision, revision);
    assert_eq!(editor.selected, selected);
    editor.redo();
    assert_eq!(editor.project, redo);
}

#[test]
fn polygon_candidate_and_commit_share_geometry_parameters_history_and_roundtrip() {
    let mut editor = Editor::new(Project::default());
    editor.place_column(Point::new(20., 20.)).unwrap();
    let with_column = editor.project.clone();
    editor.undo();
    let before = editor.project.clone();
    let revision = editor.revision;
    let selected = editor.selected;
    editor.roof_size = [30., 0.25, 2.5];
    let mut draft = draft();
    let points = points();
    for point in &points[..5] {
        draft.accept_vertex(*point).unwrap();
    }
    let frozen = draft.clone();
    let preview = editor
        .roof_polygon_candidate(&draft, Some(points[5]))
        .unwrap();
    assert_eq!(draft, frozen);
    assert_eq!(preview.footprint_contour().outer, points);
    assert_eq!(preview.min, Point::default());
    assert_eq!(preview.max, Point::new(6., 6.));
    assert_eq!(
        (preview.pitch, preview.thickness, preview.pivot_offset),
        (30., 0.25, 2.5)
    );
    assert!((preview.footprint_contour().area() - 20.).abs() < 1e-6);
    assert_eq!(editor.project, before);
    assert_eq!(editor.revision, revision);
    assert_eq!(editor.selected, selected);
    assert!(editor.undo.is_empty());
    editor.redo();
    assert_eq!(editor.project, with_column);
    editor.undo();
    draft.accept_vertex(points[5]).unwrap();
    // Explicit repeated closure data is not a second polygon vertex.
    draft.accept_vertex(points[0]).unwrap();
    let id = editor.place_roof_polygon(&draft).unwrap();
    assert_eq!(editor.undo.len(), 1);
    assert!(matches!(&editor.project.elements[0].shape,Shape::Roof { spec } if *spec==preview));
    assert_eq!(editor.project.elements[0].id, id);
    let created = editor.project.clone();
    let loaded: Project = serde_json::from_str(&serde_json::to_string(&created).unwrap()).unwrap();
    loaded.validate().unwrap();
    assert_eq!(loaded, created);
    let path =
        std::env::temp_dir().join(format!("oa-roof-polygon-draft-{}.json", std::process::id()));
    atelier_bim::storage::save(&created, &path).unwrap();
    assert_eq!(atelier_bim::storage::load(&path).unwrap(), created);
    std::fs::remove_file(path).unwrap();
    editor.undo();
    assert_eq!(editor.project, before);
    editor.redo();
    assert_eq!(editor.project, created);
}
#[test]
fn incomplete_and_crossing_drafts_are_retained_and_rejection_is_atomic() {
    let mut editor = Editor::new(Project::default());
    editor.place_column(Point::default()).unwrap();
    let with_column = editor.project.clone();
    editor.undo();
    let before = editor.project.clone();
    let revision = editor.revision;
    let mut d = draft();
    assert!(editor.place_roof_polygon(&d).is_err());
    for point in [
        Point::new(0., 0.),
        Point::new(4., 4.),
        Point::new(0., 4.),
        Point::new(4., 0.),
    ] {
        d.accept_vertex(point).unwrap();
    }
    let frozen = d.clone();
    assert!(editor.place_roof_polygon(&d).is_err());
    assert_eq!(d, frozen);
    for p in [
        Point::new(f64::NAN, 0.),
        Point::new(0., f64::INFINITY),
        Point::new(100_000., 0.),
    ] {
        assert!(d.accept_vertex(p).is_err());
        assert_eq!(d, frozen);
    }
    assert_eq!(editor.project, before);
    assert_eq!(editor.revision, revision);
    assert!(editor.undo.is_empty());
    editor.redo();
    assert_eq!(editor.project, with_column);
    assert!(
        RoofPolygonDraft::from_contour_start(RoofDraft::PivotEnd {
            start: Point::default()
        })
        .is_none()
    );
}
#[test]
fn polygon_input_capacity_and_invalid_parameters_do_not_consume_accepted_points() {
    let mut d = draft();
    for _ in 0..10_000 {
        d.accept_vertex(Point::default()).unwrap();
    }
    let frozen = d.clone();
    assert!(d.accept_vertex(Point::new(1., 1.)).is_err());
    assert_eq!(d, frozen);
    let mut d = draft();
    for p in points() {
        d.accept_vertex(p).unwrap();
    }
    let frozen = d.clone();
    assert!(d.candidate(None, [f64::NAN, 0.2, 3.]).is_err());
    assert!(
        d.candidate(Some(Point::new(f64::INFINITY, 0.)), [30., 0.2, 3.])
            .is_err()
    );
    assert_eq!(d, frozen);
}

fn session() -> Editor {
    let mut e = Editor::new(Project::default());
    e.set_tool(atelier_bim::editor::Tool::Roof);
    for p in [Point::new(-2., 0.), Point::new(8., 0.), Point::new(0., 2.)] {
        e.click(p, 0.01);
    }
    e.begin_roof_polygon().unwrap();
    e
}
#[test]
fn polygon_session_clicks_commit_and_failure_keep_one_consistent_state() {
    let mut e = session();
    let before = e.project.clone();
    for p in points() {
        e.click(p, 0.01);
    }
    let frozen = e.roof_polygon_draft().unwrap().clone();
    e.begin_roof_polygon().unwrap();
    assert_eq!(e.roof_polygon_draft(), Some(&frozen));
    e.roof_size[1] = 0.;
    assert!(e.commit_roof_polygon().is_err());
    assert_eq!(e.roof_polygon_draft(), Some(&frozen));
    assert_eq!(e.project, before);
    assert!(e.undo.is_empty());
    e.roof_size[1] = 0.2;
    assert_eq!(
        e.roof_preview(*frozen.points().last().unwrap()).unwrap(),
        e.roof_polygon_candidate(&frozen, None).unwrap()
    );
    let id = e.commit_roof_polygon().unwrap();
    assert_eq!(e.selected, Some(id));
    assert!(e.roof_polygon_draft().is_none() && e.roof_draft.is_none() && e.anchor.is_none());
    assert_eq!(e.undo.len(), 1);
    let created = e.project.clone();
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project, created);
}
#[test]
fn polygon_session_lifecycle_clears_nodes_even_without_history() {
    for action in 0..6 {
        let mut e = session();
        e.click(Point::new(1., 1.), 0.01);
        let before = e.project.clone();
        match action {
            0 => e.cancel_wall_draft(),
            1 => e.set_tool(atelier_bim::editor::Tool::Wall),
            2 => e.switch_story(1),
            3 => e.switch_worksheet(None),
            4 => e.undo(),
            _ => e.redo(),
        }
        assert!(e.roof_polygon_draft().is_none() && e.roof_draft.is_none() && e.anchor.is_none());
        assert_eq!(e.project, before);
        assert!(e.undo.is_empty());
        assert!(e.begin_roof_polygon().is_err());
    }
}

#[test]
fn roof_numeric_targets_cover_signed_rectangles_and_validate_stages() {
    let start = Point::new(1., 2.);
    for angle in [-150_f64, -30., 0., 45., 120.] {
        let pivot = RoofDraft::PivotEnd { start };
        let end = pivot.numeric_target(6., angle).unwrap();
        let values = pivot.numeric_values(end).unwrap();
        assert!((values.0 - 6.).abs() < 1e-5);
        assert!((values.1 - angle).abs() < 1e-4);
        let side = RoofDraft::RiseSide { start, end };
        assert!(side.numeric_values(start).is_none());
        assert!(side.numeric_target(1., 1.).is_err());
        for dx in [-4_f64, 4.] {
            for dy in [-3_f64, 3.] {
                let first = Point::new(5., 5.);
                let draft = RoofDraft::ContourEnd {
                    start,
                    end,
                    rise_left: true,
                    first,
                };
                let point = draft.numeric_target(dx, dy).unwrap();
                assert_eq!(draft.numeric_values(point), Some((dx, dy)));
                let mut editor = Editor::new(Project::default());
                editor.roof_draft = Some(draft);
                let candidate = editor.roof_preview(point).unwrap();
                candidate.validate().unwrap();
                assert!((candidate.footprint_contour().area() - 12.).abs() < 1e-5);
                editor.place_roof(candidate.clone()).unwrap();
                let actual = editor.project.clone();
                let loaded: Project =
                    serde_json::from_str(&serde_json::to_string(&actual).unwrap()).unwrap();
                assert_eq!(loaded, actual);
                editor.undo();
                assert!(editor.project.elements.is_empty());
                editor.redo();
                assert_eq!(editor.project, actual);
            }
        }
    }
    let pivot = RoofDraft::PivotEnd { start };
    for (a, b) in [
        (f64::NAN, 0.),
        (6., f64::INFINITY),
        (-1., 0.),
        (0., 0.),
        (100000., 0.),
    ] {
        assert!(pivot.numeric_target(a, b).is_err());
    }
    let rect = RoofDraft::ContourEnd {
        start,
        end: Point::new(7., 2.),
        rise_left: true,
        first: Point::default(),
    };
    for (a, b) in [
        (0., 3.),
        (4., 0.),
        (f64::NAN, 3.),
        (4., f64::INFINITY),
        (100000., 3.),
    ] {
        assert!(rect.numeric_target(a, b).is_err());
    }
}

#[test]
fn rotated_roof_creation_keeps_pivot_independent_and_roundtrips() {
    use atelier_bim::{editor::Tool, roof::RoofGeometryMethod};
    for angle in [-120_f64, 0., 35., 90.] {
        for width in [-3_f64, 3.] {
            let mut e = Editor::new(Project::default());
            e.set_tool(Tool::Roof);
            e.roof_method = RoofGeometryMethod::RotatedRectangle;
            let before = e.project.clone();
            for p in [
                Point::new(0., 0.),
                Point::new(6., 0.),
                Point::new(0., 1.),
                Point::new(1., 2.),
            ] {
                e.click(p, 0.);
            }
            let baseline = e.roof_draft.unwrap();
            let end = baseline.numeric_target(5., angle).unwrap();
            e.click(end, 0.);
            assert_eq!(e.project, before);
            assert!(e.undo.is_empty());
            let draft = e.roof_draft.unwrap();
            let target = draft.numeric_target(width, 0.).unwrap();
            assert!((draft.numeric_values(target).unwrap().0 - width).abs() < 1e-5);
            assert!(draft.numeric_target(0., 0.).is_err());
            assert!(draft.numeric_target(f64::NAN, 0.).is_err());
            let spec = e.roof_preview(target).unwrap();
            spec.validate().unwrap();
            assert_eq!(spec.pivot_start, Point::new(0., 0.));
            assert_eq!(spec.pivot_end, Point::new(6., 0.));
            assert!(spec.rise_left);
            let contour = spec.contour.as_ref().unwrap();
            assert_eq!(contour.outer.len(), 4);
            assert!((contour.area() - 15.).abs() < 1e-4);
            assert_eq!(contour.outer[0], Point::new(1., 2.));
            assert_eq!(contour.outer[1], end);
            for point in &contour.outer {
                let expected = 3. + point.y * 20_f64.to_radians().tan();
                assert!((spec.bottom(*point, 0.) - expected).abs() < 1e-5);
            }
            e.click(target, 0.);
            assert_eq!(e.undo.len(), 1);
            assert!(
                matches!(&e.project.elements[0].shape,Shape::Roof{spec: actual} if actual == &spec)
            );
            let after = e.project.clone();
            let restored: Project =
                serde_json::from_str(&serde_json::to_string(&after).unwrap()).unwrap();
            assert_eq!(restored, after);
            e.undo();
            assert_eq!(e.project, before);
            e.redo();
            assert_eq!(e.project, after);
        }
    }
}

#[test]
fn rotated_roof_invalid_pointer_preserves_draft_and_real_redo() {
    use atelier_bim::{editor::Tool, roof::RoofGeometryMethod};
    let mut e = Editor::new(Project::default());
    e.place_column(Point::new(20., 20.)).unwrap();
    let redo = e.project.clone();
    e.undo();
    let before = e.project.clone();
    e.set_tool(Tool::Roof);
    e.roof_method = RoofGeometryMethod::RotatedRectangle;
    for p in [(0., 0.), (6., 0.), (0., 1.), (1., 2.)] {
        e.click(Point::new(p.0, p.1), 0.);
    }
    let baseline = e.roof_draft;
    e.click(Point::new(1., 2.), 0.);
    assert_eq!(e.roof_draft, baseline);
    e.click(Point::new(5., 4.), 0.);
    let width = e.roof_draft;
    e.click(Point::new(5., 4.), 0.);
    assert_eq!(e.roof_draft, width);
    assert_eq!(e.project, before);
    e.cancel_wall_draft();
    assert!(e.roof_draft.is_none());
    e.redo();
    assert_eq!(e.project, redo);
}

#[test]
fn numeric_polygon_nodes_share_pointer_candidate_and_single_commit() {
    let mut e = session();
    let before = e.project.clone();
    e.accept_roof_polygon_vertex(Point::new(0., 0.)).unwrap();
    for (length, angle, expected) in [
        (6., 0., Point::new(6., 0.)),
        (2., 450., Point::new(6., 2.)),
        (4., 180., Point::new(2., 2.)),
        (4., 90., Point::new(2., 6.)),
        (2., -180., Point::new(0., 6.)),
    ] {
        let frozen = e.roof_polygon_draft().unwrap().clone();
        let point = frozen.numeric_target(length, angle).unwrap();
        assert!(point.distance(expected) < 1e-5);
        assert_eq!(e.roof_polygon_draft(), Some(&frozen));
        let preview = e.roof_polygon_candidate(&frozen, Some(point));
        e.accept_roof_polygon_numeric_vertex(length, angle).unwrap();
        assert_eq!(e.anchor, Some(point));
        if let Ok(preview) = preview {
            assert_eq!(
                e.roof_polygon_candidate(e.roof_polygon_draft().unwrap(), None)
                    .unwrap(),
                preview
            );
        }
        assert_eq!(e.project, before);
        assert!(e.undo.is_empty());
    }
    let expected = e
        .roof_polygon_candidate(e.roof_polygon_draft().unwrap(), None)
        .unwrap();
    assert!((expected.footprint_contour().area() - 20.).abs() < 1e-4);
    e.commit_roof_polygon().unwrap();
    assert_eq!(e.undo.len(), 1);
    assert!(matches!(&e.project.elements[0].shape, Shape::Roof { spec } if *spec == expected));
    let created = e.project.clone();
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project, created);
}

#[test]
fn numeric_polygon_rejection_preserves_nodes_anchor_and_actual_redo() {
    let mut e = Editor::new(Project::default());
    e.place_column(Point::new(20., 20.)).unwrap();
    let redo = e.project.clone();
    e.undo();
    let before = e.project.clone();
    e.set_tool(atelier_bim::editor::Tool::Roof);
    for p in [Point::new(-2., 0.), Point::new(8., 0.), Point::new(0., 2.)] {
        e.click(p, 0.01);
    }
    e.begin_roof_polygon().unwrap();
    assert!(e.accept_roof_polygon_numeric_vertex(2., 0.).is_err());
    assert!(e.roof_polygon_draft().unwrap().points().is_empty());
    e.accept_roof_polygon_vertex(Point::new(99999., 2.))
        .unwrap();
    let frozen = e.roof_polygon_draft().unwrap().clone();
    let anchor = e.anchor;
    let revision = e.revision;
    for (length, angle) in [
        (0., 0.),
        (-1., 0.),
        (f64::NAN, 0.),
        (1., f64::INFINITY),
        (2., 0.),
    ] {
        assert!(e.accept_roof_polygon_numeric_vertex(length, angle).is_err());
        assert_eq!(e.roof_polygon_draft(), Some(&frozen));
        assert_eq!(e.anchor, anchor);
        assert_eq!(e.project, before);
        assert_eq!(e.revision, revision);
    }
    e.accept_roof_polygon_numeric_vertex(2., 180.).unwrap();
    assert!(e.anchor.unwrap().distance(Point::new(99997., 2.)) < 1e-5);
    e.cancel_wall_draft();
    e.redo();
    assert_eq!(e.project, redo);
}

#[test]
fn shared_roof_candidate_preserves_history_defaults_and_full_validation_boundary() {
    use atelier_bim::{
        layers::ElementLayer, model::Material, roof::RoofSpec, roof_contour::RoofContour,
        view_settings::RenovationStatus,
    };
    for with_hole in [false, true] {
        let mut e = Editor::new(Project::default());
        e.place_column(Point::new(9., 9.)).unwrap();
        let redo_project = e.project.clone();
        e.undo();
        e.story = 1;
        e.roof_material = Material::Timber;
        e.wall_renovation = RenovationStatus::Existing;
        e.roof_layer = ElementLayer(
            e.project
                .layers
                .layers
                .iter()
                .find(|l| l.id != 0 && !l.status.locked)
                .unwrap()
                .id,
        );
        let ring = |a: f64, b: f64, c: f64, d: f64| {
            vec![
                Point::new(a, b),
                Point::new(c, b),
                Point::new(c, d),
                Point::new(a, d),
            ]
        };
        let spec = RoofSpec {
            min: Point::new(0., 0.),
            max: Point::new(8., 6.),
            pivot_start: Point::new(-2., 1.),
            pivot_end: Point::new(10., 1.),
            rise_left: true,
            pitch: 30.,
            thickness: 0.25,
            pivot_offset: 2.,
            contour: with_hole.then(|| RoofContour {
                outer: ring(0., 0., 8., 6.),
                holes: vec![ring(2., 2., 4., 4.)],
            }),
        };
        let before = e.project.clone();
        let revision = e.revision;
        let selection = e.selection_ids();
        let candidate = e.roof_creation_candidate(spec.clone()).unwrap();
        assert_eq!(candidate.story, 1);
        assert_eq!(candidate.material, Material::Timber);
        assert_eq!(candidate.layer, e.roof_layer);
        assert_eq!(candidate.renovation, RenovationStatus::Existing);
        let mut invalid = spec.clone();
        invalid.pitch = f64::NAN;
        assert!(e.roof_creation_candidate(invalid).is_err());
        assert_eq!(e.project, before);
        assert_eq!(e.revision, revision);
        assert_eq!(e.selection_ids(), selection);
        e.redo();
        assert_eq!(e.project, redo_project);
        e.undo();
        assert_eq!(e.project, before);
        e.project
            .layers
            .layers
            .iter_mut()
            .find(|l| l.id == e.roof_layer.0)
            .unwrap()
            .status
            .locked = true;
        let locked = e.project.clone();
        assert_eq!(e.roof_creation_candidate(spec.clone()).unwrap(), candidate);
        assert!(
            e.preview_add_elements(vec![candidate.clone()], false)
                .is_err()
        );
        assert!(e.place_roof(spec.clone()).is_err());
        assert_eq!(e.project, locked);
        e.project
            .layers
            .layers
            .iter_mut()
            .find(|l| l.id == e.roof_layer.0)
            .unwrap()
            .status
            .locked = false;
        assert_eq!(e.project, before);
        e.redo();
        assert_eq!(e.project, redo_project);
        e.undo();
        assert_eq!(e.project, before);
        let preview = e
            .preview_add_elements(vec![candidate.clone()], false)
            .unwrap();
        let id = e.place_roof(spec).unwrap();
        assert_eq!(e.project, preview);
        assert_eq!(
            e.project.elements.iter().find(|x| x.id == id),
            Some(&candidate)
        );
        let loaded: Project =
            serde_json::from_str(&serde_json::to_string(&e.project).unwrap()).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded, e.project);
        e.undo();
        assert_eq!(e.project, before);
        e.redo();
        assert_eq!(e.project, preview);
    }
}
