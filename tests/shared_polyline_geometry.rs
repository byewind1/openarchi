use atelier_bim::{
    editor::Editor,
    geometry,
    model::{ArcSegment, Point, Project},
};
#[test]
fn polyline_actual_bounds_include_non_endpoint_arc_extrema() {
    let mut e = Editor::new(Project::default());
    let at = |degrees: f64| {
        Point::new(
            4. * degrees.to_radians().cos(),
            4. * degrees.to_radians().sin(),
        )
    };
    let id = e
        .place_polyline(
            vec![at(11.), at(107.)],
            vec![Some(ArcSegment {
                radius: 4.,
                clockwise: false,
                large_arc: false,
            })],
        )
        .unwrap();
    let element = e.project.elements.iter().find(|e| e.id == id).unwrap();
    let c = geometry::corners(&e.project, element);
    let ymax = c.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
    assert!((ymax - 4.).abs() < 1e-10, "{ymax}");
}
#[test]
fn plan_only_polyline_does_not_have_an_invisible_pickable_box_in_3d() {
    use glam::DVec3;
    let mut e = Editor::new(Project::default());
    e.place_polyline(vec![Point::new(-2., -2.), Point::new(2., 2.)], vec![])
        .unwrap();
    let camera = geometry::Camera {
        target: DVec3::ZERO,
        ..Default::default()
    };
    let selected = camera.pick(&e.project, [0., 0.], 4. / 3., None);
    assert_eq!(selected, None);
}

#[test]
fn curve_query_keeps_analytic_lengths_cardinal_bounds_and_sampling_separate() {
    use atelier_bim::polyline_geometry;
    for origin in [0., 99900., -99900.] {
        for major in [false, true] {
            for reverse in [false, true] {
                let at = |deg: f64| {
                    Point::new(
                        origin + 4. * deg.to_radians().cos(),
                        origin + 4. * deg.to_radians().sin(),
                    )
                };
                let (a, b) = if reverse {
                    (at(107.), at(11.))
                } else {
                    (at(11.), at(107.))
                };
                let arc = ArcSegment {
                    radius: 4.,
                    clockwise: major != reverse,
                    large_arc: major,
                };
                let g = polyline_geometry::snapshot(&[a, b], &[Some(arc)], 2.5).unwrap();
                let minor = 96_f64.to_radians();
                let sweep = if major {
                    std::f64::consts::TAU - minor
                } else {
                    minor
                };
                assert!((g.length_m - 4. * sweep).abs() < 1e-8);
                assert!(!g.closed);
                assert_eq!(g.plan_path.first(), Some(&a));
                assert_eq!(g.plan_path.last(), Some(&b));
                let n = (g.plan_path.len() - 1) as f64;
                let chords = n * 8. * (sweep / (2. * n)).sin();
                assert!((g.sampled_length_m - chords).abs() < 1e-8);
                assert!(g.sampled_length_m < g.length_m);
                assert_eq!(g.bounds[0].z, 2.5);
                assert_eq!(g.bounds[1].z, 2.5);
                let (expected_lo, expected_hi) = if major {
                    (
                        Point::new(origin - 4., origin - 4.),
                        Point::new(origin + 4., at(107.).y),
                    )
                } else {
                    (
                        Point::new(at(107.).x, at(11.).y),
                        Point::new(at(11.).x, origin + 4.),
                    )
                };
                assert!((g.bounds[0].x - expected_lo.x).abs() < 1e-9);
                assert!((g.bounds[0].y - expected_lo.y).abs() < 1e-9);
                assert!((g.bounds[1].x - expected_hi.x).abs() < 1e-9);
                assert!((g.bounds[1].y - expected_hi.y).abs() < 1e-9);
                if !major {
                    assert!(g.sampled_bounds[1].y < g.bounds[1].y - 1e-5);
                }
                for p in &g.plan_path {
                    assert!(
                        p.x >= g.bounds[0].x - 1e-9
                            && p.x <= g.bounds[1].x + 1e-9
                            && p.y >= g.bounds[0].y - 1e-9
                            && p.y <= g.bounds[1].y + 1e-9
                    );
                }
            }
        }
    }
}
#[test]
fn closed_semicircles_and_mixed_segments_keep_stored_endpoints_and_parameter_length() {
    use atelier_bim::polyline_geometry;
    let a = Point::new(-1., 0.);
    let b = Point::new(1., 0.);
    let c = ArcSegment {
        radius: 1.,
        clockwise: true,
        large_arc: false,
    };
    let g = polyline_geometry::snapshot(&[a, b, a], &[Some(c), Some(c)], 0.).unwrap();
    assert!(g.closed);
    assert_eq!(g.plan_path.first(), g.plan_path.last());
    assert!((g.length_m - std::f64::consts::TAU).abs() < 1e-12);
    assert!((g.bounds[0].y + 1.).abs() < 1e-12);
    assert!((g.bounds[1].y - 1.).abs() < 1e-12);
    let g = polyline_geometry::snapshot(&[a, b, Point::new(1., 3.)], &[Some(c), None], 0.).unwrap();
    assert!((g.length_m - std::f64::consts::PI - 3.).abs() < 1e-12);
    assert_eq!(g.segments[1].endpoints(), [b, Point::new(1., 3.)]);
}
#[test]
fn curve_service_preview_commit_context_history_and_file_are_the_same_model() {
    use atelier_bim::{
        command::ModelCommand, model::ModelAdapter, polyline_geometry, storage::JsonAdapter,
    };
    let mut e = Editor::new(Project::default());
    let candidate = e.polyline_creation_candidate(
        vec![Point::new(1., 0.), Point::new(0., 1.)],
        vec![Some(ArcSegment {
            radius: 1.,
            clockwise: true,
            large_arc: true,
        })],
    );
    let id = candidate.id;
    let before = e.project.clone();
    let prepared = e
        .prepare_change(
            e.revision,
            vec![ModelCommand::AddElement {
                element: candidate.clone(),
            }],
        )
        .unwrap();
    let preview = polyline_geometry::derive(prepared.project(), &candidate).unwrap();
    assert_eq!(e.project, before);
    assert!(e.undo.is_empty());
    e.commit_change("curve-source", &prepared).unwrap();
    let g = e.polyline_geometry(id).unwrap();
    assert_eq!(g.geometry, preview.geometry);
    assert_eq!(
        g.identity.input_fingerprint,
        preview.identity.input_fingerprint
    );
    assert_eq!(g.identity.input_revision, Some(e.revision));
    assert_eq!(g.identity.document_session, Some(e.document_session()));
    assert_eq!(e.project.elements[0].area(), g.geometry.length_m);
    let state = format!(
        "{:?}/{:?}/{:?}/{:?}",
        e.project,
        e.undo,
        e.redo,
        e.selection_ids()
    );
    e.polyline_geometry(id).unwrap();
    assert_eq!(
        state,
        format!(
            "{:?}/{:?}/{:?}/{:?}",
            e.project,
            e.undo,
            e.redo,
            e.selection_ids()
        )
    );
    let copy = Editor::new(
        JsonAdapter
            .import(&JsonAdapter.export(&e.project).unwrap())
            .unwrap(),
    );
    assert_eq!(copy.polyline_geometry(id).unwrap().geometry, g.geometry);
    assert_ne!(copy.document_session(), e.document_session());
    e.apply(ModelCommand::MoveElements {
        ids: vec![id],
        offset: Point::new(2., -1.),
    })
    .unwrap();
    let moved = e.polyline_geometry(id).unwrap();
    assert_ne!(
        moved.identity.input_fingerprint,
        g.identity.input_fingerprint
    );
    assert!((moved.geometry.bounds[0].x - g.geometry.bounds[0].x - 2.).abs() < 1e-12);
    e.undo();
    assert_eq!(e.polyline_geometry(id).unwrap().geometry, g.geometry);
    e.redo();
    assert_eq!(e.polyline_geometry(id).unwrap().geometry, moved.geometry);
    assert_eq!(
        e.polyline_geometry(999).unwrap_err().code,
        "ELEMENT_NOT_FOUND"
    );
}
#[test]
fn invalid_curve_metadata_and_unrepresentable_arcs_are_not_straight_line_successes() {
    use atelier_bim::polyline_geometry;
    let p = [Point::default(), Point::new(1., 0.)];
    for arcs in [
        vec![None, None],
        vec![Some(ArcSegment {
            radius: 0.,
            clockwise: false,
            large_arc: false,
        })],
        vec![Some(ArcSegment {
            radius: 0.1,
            clockwise: false,
            large_arc: false,
        })],
        vec![Some(ArcSegment {
            radius: 1e308,
            clockwise: false,
            large_arc: false,
        })],
    ] {
        assert!(polyline_geometry::snapshot(&p, &arcs, 0.).is_err());
        assert!(geometry::polyline_path(&p, &arcs).is_empty());
    }
    assert!(polyline_geometry::snapshot(&p, &[], f64::NAN).is_err());
    assert!(polyline_geometry::snapshot(&p[..1], &[], 0.).is_err());
}
