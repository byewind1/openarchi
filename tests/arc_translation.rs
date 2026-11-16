use atelier_bim::{
    editor::Editor,
    geometry,
    model::{Point, Project},
};

#[test]
fn three_point_arcs_keep_radius_branch_and_creation_under_translation() {
    for offset in [0., 10000., 90000., -90000.] {
        for large in [false, true] {
            for reversed in [false, true] {
                let p = |x: f64, y: f64| Point::new(x + offset, y - offset);
                let (a, b) = if reversed {
                    (p(0., 5.), p(5., 0.))
                } else {
                    (p(5., 0.), p(0., 5.))
                };
                let through = if large { p(-5., 0.) } else { p(3., 4.) };
                let arc = geometry::arc_segment_through(a, b, through).unwrap();
                assert!(
                    (arc.radius - 5.).abs() < 1e-5,
                    "offset={offset}, arc={arc:?}"
                );
                assert_eq!(arc.large_arc, large);
                assert_eq!(arc.clockwise, large != reversed);
                let (center, sweep) = geometry::arc_center_sweep(a, b, arc).unwrap();
                assert!(center.distance(p(0., 0.)) < 1e-4);
                assert!((through.distance(center) - 5.).abs() < 1e-4);
                let expected = std::f64::consts::FRAC_PI_2 * if large { 3. } else { 1. };
                assert!((sweep.abs() - expected).abs() < 1e-5);
                let mut e = Editor::new(Project::default());
                let before = e.project.clone();
                e.place_polyline(vec![a, b], vec![Some(arc)]).unwrap();
                let after = e.project.clone();
                let loaded: Project =
                    serde_json::from_str(&serde_json::to_string(&after).unwrap()).unwrap();
                loaded.validate().unwrap();
                assert_eq!(loaded, after);
                e.undo();
                assert_eq!(e.project, before);
                e.redo();
                assert_eq!(e.project, after);
            }
        }
    }
}

#[test]
fn complementary_semicircles_keep_closed_offset() {
    let a = Point::new(-1., 0.);
    let b = Point::new(1., 0.);
    let upper = geometry::arc_segment_through(a, b, Point::new(0., 1.)).unwrap();
    let lower = geometry::arc_segment_through(b, a, Point::new(0., -1.)).unwrap();
    assert!(upper.clockwise, "upper={upper:?}");
    assert!(lower.clockwise, "lower={lower:?}");
    assert!(!upper.large_arc && !lower.large_arc, "{upper:?} {lower:?}");
    assert!(geometry::arc_center_sweep(a, b, upper).is_some());
    assert!(
        geometry::arc_center_sweep(b, a, lower).is_some(),
        "lower={lower:?}"
    );
    assert!(
        geometry::offset_polyline_with_arcs(&[a, b, a], &[Some(upper), Some(lower)], 0.25)
            .is_some()
    );
}

#[test]
fn arc_geometry_rejects_nonfinite_and_degenerate_inputs() {
    use atelier_bim::model::ArcSegment;
    let a = Point::new(-1., 0.);
    let b = Point::new(1., 0.);
    let through = Point::new(0., 1.);
    let arc = ArcSegment {
        radius: 1.,
        clockwise: true,
        large_arc: false,
    };
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for invalid in [Point::new(value, 0.), Point::new(0., value)] {
            assert!(geometry::arc_segment_through(invalid, b, through).is_none());
            assert!(geometry::arc_segment_through(a, invalid, through).is_none());
            assert!(geometry::arc_segment_through(a, b, invalid).is_none());
            assert!(
                geometry::arc_center_sweep(invalid, b, arc).is_none(),
                "invalid start {invalid:?}"
            );
            assert!(
                geometry::arc_center_sweep(a, invalid, arc).is_none(),
                "invalid end {invalid:?}"
            );
        }
    }
    assert!(geometry::arc_segment_through(a, a, through).is_none());
    assert!(geometry::arc_segment_through(a, b, Point::new(0., 0.)).is_none());
    assert!(geometry::arc_center_sweep(a, a, arc).is_none());
    for radius in [0., -1., 0.5, f64::NAN, f64::INFINITY] {
        assert!(geometry::arc_center_sweep(a, b, ArcSegment { radius, ..arc }).is_none());
    }
}

#[test]
fn rejected_arc_creation_preserves_real_redo_and_allows_correction() {
    use atelier_bim::model::ArcSegment;
    let mut e = Editor::new(Project::default());
    e.place_column(Point::new(4., 4.)).unwrap();
    let created = e.project.clone();
    e.undo();
    let before = e.project.clone();
    let revision = e.revision;
    let selected = e.selected;
    let dirty = e.dirty();
    let a = Point::new(-1., 0.);
    let b = Point::new(1., 0.);
    for (points, radius) in [
        (vec![Point::new(f64::NAN, 0.), b], 1.),
        (vec![a, Point::new(0., f64::INFINITY)], 1.),
        (vec![a, b], 0.5),
        (vec![a, b], f64::NAN),
        (vec![a, a], 1.),
    ] {
        let arc = ArcSegment {
            radius,
            clockwise: true,
            large_arc: false,
        };
        assert!(e.place_polyline(points, vec![Some(arc)]).is_err());
        assert_eq!(e.project, before);
        assert_eq!(e.revision, revision);
        assert_eq!(e.selected, selected);
        assert_eq!(e.dirty(), dirty);
        assert!(e.undo.is_empty());
    }
    e.redo();
    assert_eq!(e.project, created);
    e.undo();
    assert_eq!(e.project, before);
    let arc = geometry::arc_segment_through(a, b, Point::new(0., 1.)).unwrap();
    e.place_polyline(vec![a, b], vec![Some(arc)]).unwrap();
    let corrected = e.project.clone();
    assert_eq!(e.undo.len(), 1);
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project, corrected);
}
