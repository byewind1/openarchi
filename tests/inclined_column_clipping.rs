use atelier_bim::{
    column::{self, ColumnSection},
    column_inclination::{ColumnInclination, InclinedColumnSolid},
    editor::Editor,
    geometry::{self, Camera},
    model::{Point, Project},
};
use glam::DVec3;
fn scene() -> (Editor, u64) {
    let mut e = Editor::new(Project::default());
    let id = e
        .place_column_with_inclination(
            Point::default(),
            ColumnSection::Rectangular {
                width: 2.,
                depth: 2_f64.sqrt(),
            },
            2.,
            Some(ColumnInclination {
                slant_degrees: 45.,
                direction_degrees: 0.,
            }),
        )
        .unwrap();
    (e, id)
}
fn solid() -> InclinedColumnSolid {
    let (e, _) = scene();
    column::inclined_solid(&e.project, &e.project.elements[0]).unwrap()
}
fn volume(t: &[[DVec3; 3]]) -> f64 {
    t.iter()
        .map(|[a, b, c]| a.dot(b.cross(*c)) / 6.)
        .sum::<f64>()
}
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 0.0001, "{a} != {b}");
}
#[test]
fn horizontal_and_vertical_clips_keep_true_shear_and_sealed_volume() {
    let s = solid();
    let whole = s.mesh().unwrap();
    near(volume(&whole.triangles), 8.);
    assert_eq!(
        s.clipped_mesh(None, None).unwrap().triangles,
        whole.triangles
    );
    let half = [p(0., -5.), p(5., -5.), p(5., 5.), p(0., 5.)];
    let m = s.clipped_mesh(Some(&half), None).unwrap();
    near(volume(&m.triangles), 4.);
    assert!(m.triangles.iter().flatten().all(|p| p.x >= -1e-6));
    let m = s.clipped_mesh(Some(&half), Some((1., 2.))).unwrap();
    near(volume(&m.triangles), 2.);
    assert!(m.triangles.iter().flatten().all(|p| p.z >= 1. - 1e-6));
    near(
        volume(&s.clipped_mesh(None, Some((1.5, 0.5))).unwrap().triangles),
        4.,
    );
    let empty = [p(-5., 2.1), p(5., 2.1), p(5., 5.), p(-5., 5.)];
    assert!(
        s.clipped_mesh(Some(&empty), Some((0., 1.)))
            .unwrap()
            .triangles
            .is_empty()
    );
}
#[test]
fn concave_clip_has_no_partition_faces_or_artificial_diagonal_edges() {
    let s = solid();
    let shape = [
        p(-5., -5.),
        p(5., -5.),
        p(5., 2.),
        p(0., 2.),
        p(0., 5.),
        p(-5., 5.),
    ];
    near(
        volume(&s.clipped_mesh(Some(&shape), None).unwrap().triangles),
        7.5,
    );
    let cover = [p(-5., -5.), p(5., -5.), p(5., 5.), p(-5., 5.)];
    let m = s.clipped_mesh(Some(&cover), None).unwrap();
    near(volume(&m.triangles), 8.);
    let original = s.mesh().unwrap();
    for [a, b] in m.edges {
        assert!(
            original.edges.iter().any(|[u, v]| {
                let d = *v - *u;
                let on = |q: DVec3| {
                    (q - *u).cross(d).length() < 1e-5
                        && (q - *u).dot(d) >= -1e-5
                        && (q - *u).dot(d) <= d.length_squared() + 1e-5
                };
                on(a) && on(b)
            }),
            "unexpected partition edge {a:?} {b:?}"
        );
    }
}
#[test]
fn circular_clipping_and_invalid_ranges() {
    let mut s = solid();
    s.column.section = ColumnSection::Circular { diameter: 2. };
    let whole = volume(&s.mesh().unwrap().triangles);
    let half = [p(0., -5.), p(5., -5.), p(5., 5.), p(0., 5.)];
    near(
        volume(&s.clipped_mesh(Some(&half), None).unwrap().triangles),
        whole / 2.,
    );
    assert!(s.clipped_mesh(None, Some((f64::NAN, 2.))).is_err());
    assert!(s.clipped_mesh(Some(&[]), None).is_err());
    assert!(
        s.clipped_mesh(None, Some((1., 1.)))
            .unwrap()
            .triangles
            .is_empty()
    );
}
#[test]
fn model_spatial_bounds_marquee_and_camera_pick_follow_slanted_top() {
    let (e, id) = scene();
    let elem = &e.project.elements[0];
    let corners = geometry::corners(&e.project, elem);
    near(corners[6].y as f64, 3.);
    let top = [p(-0.2, 2.5), p(0.2, 2.5), p(0.2, 2.9), p(-0.2, 2.9)];
    assert!(geometry::element_intersects_polygon(&e.project, elem, &top));
    assert!(!geometry::element_fully_inside_polygon(
        &e.project, elem, &top
    ));
    let whole = [p(-2., -2.), p(2., -2.), p(2., 4.), p(-2., 4.)];
    assert!(geometry::element_fully_inside_polygon(
        &e.project, elem, &whole
    ));
    let camera = Camera {
        yaw: 0.,
        pitch: 1.3,
        span: 10.,
        target: DVec3::new(0., 1., 1.),
    };
    let ndc = camera.matrix(1.).project_point3(DVec3::new(0., 2., 2.));
    assert_eq!(camera.pick(&e.project, [ndc.x, ndc.y], 1., None), Some(id));
}

#[test]
fn horizontal_clip_volume_is_independent_of_direction_axis_and_world_origin() {
    for section in [
        ColumnSection::Rectangular {
            width: 0.6,
            depth: 0.4,
        },
        ColumnSection::Circular { diameter: 0.6 },
    ] {
        for slant in [15_f64, 45., 89., 90.] {
            for direction in [-135., 0., 75., 180.] {
                for axis in [0., 37.] {
                    let mut s = solid();
                    s.column.section = section;
                    s.column.center = p(12., -9.);
                    s.column.bottom = -3.5;
                    s.column.height = 6.;
                    s.column.angle_degrees = axis;
                    s.slant_degrees = slant;
                    s.direction_degrees = direction;
                    let before = s.mesh().unwrap();
                    // A constant section has a constant horizontal area. Comparing
                    // to the whole independently checks slice placement/closure even
                    // for the circle's finite tessellation.
                    let whole_volume = volume(&before.triangles);
                    let clipped = s.clipped_mesh(None, Some((-2.25, 0.75))).unwrap();
                    let got = volume(&clipped.triangles);
                    assert!(
                        (got - whole_volume / 2.).abs() < 0.0002,
                        "{section:?} slant={slant} direction={direction} axis={axis}: {got} vs {}",
                        whole_volume / 2.
                    );
                    let area = match section {
                        ColumnSection::Rectangular { width, depth } => {
                            f64::from(width) * f64::from(depth)
                        }
                        ColumnSection::Circular { diameter } => {
                            let n = s.section_at(s.column.bottom).unwrap().len() as f64;
                            // Independent regular inscribed polygon area.
                            n * 0.5
                                * (f64::from(diameter) / 2.).powi(2)
                                * (std::f64::consts::TAU / n).sin()
                        }
                    };
                    let expected = area * 3. / slant.to_radians().sin();
                    assert!((got - expected).abs() < 0.0002, "{got} vs {expected}");
                    assert!(
                        clipped
                            .triangles
                            .iter()
                            .flatten()
                            .all(|v| v.is_finite() && v.z >= -2.25001 && v.z <= 0.75001)
                    );
                    for range in [(-5., -4.), (3., 4.)] {
                        let empty = s.clipped_mesh(None, Some(range)).unwrap();
                        assert!(empty.triangles.is_empty());
                        assert!(empty.edges.is_empty());
                    }
                    assert_eq!(s.mesh().unwrap().triangles, before.triangles);
                }
            }
        }
    }
}

#[test]
fn tangent_column_range_has_no_zero_volume_faces() {
    let s = solid();
    let m = s.clipped_mesh(None, Some((2., 3.))).unwrap();
    assert!(m.triangles.is_empty() && m.edges.is_empty());
    let m = s.clipped_mesh(None, Some((-1., 0.))).unwrap();
    assert!(m.triangles.is_empty() && m.edges.is_empty());
}
