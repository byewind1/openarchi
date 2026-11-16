use atelier_bim::{
    beam::BeamGeometry,
    command::ModelCommand,
    editor::Editor,
    geometry,
    model::{ArcSegment, Point, Project, Shape},
    storage,
};
use glam::DVec3;
fn arc(large_arc: bool) -> ArcSegment {
    ArcSegment {
        radius: 19.172970401,
        clockwise: false,
        large_arc,
    }
}
fn native(large: bool) -> BeamGeometry {
    BeamGeometry {
        start: Point::new(25.379727801, 16.271672365),
        end: Point::new(6.2067574, if large { -2.901298037 } else { 35.444642766 }),
        width: 0.3,
        height: 0.4,
        start_top: 3.,
        slant_angle: 0.,
        arc: Some(arc(large)),
    }
}
fn volume(t: &[[DVec3; 3]]) -> f64 {
    t.iter()
        .map(|t| t[0].dot(t[1].cross(t[2])) / 6.)
        .sum::<f64>()
        .abs()
}
#[test]
fn native_curved_beam_bounds_area_mesh_and_reverse() {
    for large in [false, true] {
        let g = native(large);
        let s = g.solid().unwrap();
        let sweep = if large { 1.5 } else { 0.5 } * std::f64::consts::PI;
        assert!((s.axis_length - 19.172970401 * sweep).abs() < 1e-4);
        let m = g.mesh(None, None).unwrap();
        assert!((volume(&m.triangles) - s.volume).abs() < s.volume * 0.0002);
        if large {
            for (i, lo, hi) in [
                (0, -13.116213002, 25.529727801),
                (1, -3.051298037, 35.594642766),
                (2, 2.6, 3.),
            ] {
                let actual_lo = s.corners.iter().map(|v| v[i]).fold(f64::INFINITY, f64::min);
                let actual_hi = s
                    .corners
                    .iter()
                    .map(|v| v[i])
                    .fold(f64::NEG_INFINITY, f64::max);
                assert!((actual_lo - lo).abs() < 1e-4);
                assert!((actual_hi - hi).abs() < 1e-4);
            }
        }
        let reversed = BeamGeometry {
            start: g.end,
            end: g.start,
            arc: Some(ArcSegment {
                clockwise: true,
                ..g.arc.unwrap()
            }),
            ..g
        };
        assert!((reversed.solid().unwrap().volume - s.volume).abs() < 1e-4);
        let clipped = g.mesh(None, Some((2.7, 2.9))).unwrap();
        assert!((volume(&clipped.triangles) - s.volume * 0.5).abs() < s.volume * 0.0002);
        assert!(g.mesh(None, Some((4., 5.))).unwrap().triangles.is_empty());
        let clip = [
            Point::new(0., 0.),
            Point::new(30., 0.),
            Point::new(30., 40.),
            Point::new(0., 40.),
        ];
        let clipped = g.mesh(Some(&clip), None).unwrap();
        assert!(!clipped.triangles.is_empty());
        assert!(
            clipped
                .triangles
                .iter()
                .flatten()
                .all(|p| p.x >= -1e-4 && p.x <= 30.0001 && p.y >= -1e-4 && p.y <= 40.0001)
        );
    }
}
#[test]
fn curved_beam_typed_history_file_picking_and_legacy() {
    let mut e = Editor::new(Project::default());
    e.beam_size = [0.3, 0.4, 3.];
    let g = native(true);
    let id = e.place_curved_beam(g.start, g.end, g.arc.unwrap()).unwrap();
    let before = e.project.clone();
    let b = &before.elements[0];
    assert!((b.area() as f64 - g.solid().unwrap().axis_length * 0.3).abs() < 1e-4);
    assert_eq!(
        geometry::hit_plan(&before, 0, Point::new(-12.966213, 16.271673), 0.01),
        Some(id)
    );
    assert_eq!(
        geometry::hit_plan(&before, 0, Point::new(6.2067574, 16.271673), 0.01),
        None
    );
    let camera = geometry::Camera {
        target: DVec3::new(6.2067574, 16.271673, 3.),
        yaw: 0.,
        pitch: 89_f64.to_radians(),
        span: 50.,
        ..Default::default()
    };
    for (point, expected) in [
        (DVec3::new(-12.966213, 16.271673, 3.), Some(id)),
        (DVec3::new(6.2067574, 16.271673, 3.), None),
    ] {
        let n = camera.matrix(1.).project_point3(point);
        assert_eq!(camera.pick(&before, [n.x, n.y], 1., Some(0)), expected);
    }
    e.apply(ModelCommand::MoveElements {
        ids: vec![id],
        offset: Point::new(4., -2.),
    })
    .unwrap();
    let moved = e.project.clone();
    e.undo();
    assert_eq!(e.project, before);
    e.undo();
    assert!(e.project.elements.is_empty());
    e.redo();
    e.redo();
    assert_eq!(e.project, moved);
    let path =
        std::env::temp_dir().join(format!("openarchi-curved-beam-{}.json", std::process::id()));
    storage::save(&moved, &path).unwrap();
    assert_eq!(storage::load(&path).unwrap(), moved);
    std::fs::remove_file(path).unwrap();
    let mut legacy = serde_json::to_value(&before).unwrap();
    legacy["elements"][0]["shape"]
        .as_object_mut()
        .unwrap()
        .remove("arc");
    let p: Project = serde_json::from_value(legacy).unwrap();
    p.validate().unwrap();
    assert!(matches!(p.elements[0].shape, Shape::Beam { arc: None, .. }));
}
#[test]
fn invalid_curved_creation_preserves_identity_redo_and_project() {
    let mut e = Editor::new(Project::default());
    e.place_beam(Point::new(0., 0.), Point::new(2., 0.))
        .unwrap();
    e.undo();
    let before = e.project.clone();
    let rev = e.revision;
    let redo = e.redo.len();
    for radius in [0., f64::NAN, 0.1, 1.] {
        assert!(
            e.place_curved_beam(
                Point::new(10., 0.),
                Point::new(0., 10.),
                ArcSegment {
                    radius,
                    clockwise: false,
                    large_arc: false
                }
            )
            .is_err()
        );
        assert_eq!(e.project, before);
        assert_eq!(e.revision, rev);
        assert_eq!(e.redo.len(), redo);
    }
    assert!(
        BeamGeometry {
            slant_angle: 10.,
            ..native(false)
        }
        .solid()
        .is_err()
    );
}

#[test]
fn curved_side_edges_keep_end_caps_and_true_cuts_without_facet_seams() {
    let g = BeamGeometry {
        start: Point::new(4., 0.),
        end: Point::new(0., -4.),
        width: 0.6,
        height: 0.8,
        start_top: 3.,
        slant_angle: 0.,
        arc: Some(ArcSegment {
            radius: 4.,
            clockwise: false,
            large_arc: true,
        }),
    };
    let vertical_positions = |mesh: &atelier_bim::beam::BeamMesh| {
        let mut points: Vec<Point> = vec![];
        for [a, b] in &mesh.edges {
            let point = Point::new(a.x, a.y);
            if point.distance(Point::new(b.x, b.y)) < 1e-6
                && (a.z - b.z).abs() > 0.01
                && !points.iter().any(|p| p.distance(point) < 1e-4)
            {
                points.push(point);
            }
        }
        points
    };
    for spec in [
        g,
        BeamGeometry {
            start: g.end,
            end: g.start,
            arc: Some(ArcSegment {
                clockwise: true,
                ..g.arc.unwrap()
            }),
            ..g
        },
    ] {
        for z in [None, Some((2.4, 2.8))] {
            let mesh = spec.mesh(None, z).unwrap();
            let corners = vertical_positions(&mesh);
            assert_eq!(corners.len(), 4, "{corners:?}");
            for expected in [
                Point::new(4.3, 0.),
                Point::new(3.7, 0.),
                Point::new(0., -4.3),
                Point::new(0., -3.7),
            ] {
                assert!(
                    corners.iter().any(|p| p.distance(expected) < 1e-4),
                    "missing {expected:?}"
                );
            }
            let expected_volume =
                4. * std::f64::consts::PI * 1.5 * 0.6 * if z.is_some() { 0.4 } else { 0.8 };
            assert!((volume(&mesh.triangles) - expected_volume).abs() < expected_volume * 0.0002);
        }
    }
    // x=0 crosses cardinal samples exactly: their new vertical cut edges must survive.
    let clip = [
        Point::new(0., -10.),
        Point::new(10., -10.),
        Point::new(10., 10.),
        Point::new(0., 10.),
    ];
    let mesh = g.mesh(Some(&clip), Some((2.4, 2.8))).unwrap();
    let corners = vertical_positions(&mesh);
    // The source end on x=0 can survive as a nearly zero-width boundary
    // fragment at f64 precision; it is also a genuine radial-end corner.
    assert!(corners.iter().all(|p| p.x.abs() < 1e-4 || p.y.abs() < 1e-4));
    for expected in [
        Point::new(4.3, 0.),
        Point::new(3.7, 0.),
        Point::new(0., 4.3),
        Point::new(0., 3.7),
    ] {
        assert!(
            corners.iter().any(|p| p.distance(expected) < 1e-4),
            "missing cut {expected:?}: {corners:?}"
        );
    }
}
