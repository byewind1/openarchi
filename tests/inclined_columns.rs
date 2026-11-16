use atelier_bim::{
    column::{ColumnSection, ColumnSolid},
    column_inclination::InclinedColumnSolid,
    model::Point,
};
use glam::DVec3;
use std::collections::HashMap;

fn sample() -> InclinedColumnSolid {
    InclinedColumnSolid {
        column: ColumnSolid {
            section: ColumnSection::Rectangular {
                width: 0.3,
                depth: 0.6,
            },
            center: Point::new(2., -1.),
            bottom: -0.75,
            height: 2.8,
            angle_degrees: 30.,
        },
        slant_degrees: 60.,
        direction_degrees: -90.,
    }
}
fn fixture(name: &str) -> serde_json::Value {
    let path = format!(
        "{}/tests/fixtures/geometry/slanted-column/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}
#[test]
fn rectangular_bounds_match_three_native_samples() {
    for name in ["slant60", "two-point", "axis30"] {
        let api = fixture(&format!("{name}-api"));
        let d = &api["detailsOfElements"][0]["details"];
        let n = |key: &str| d[key].as_f64().unwrap();
        let solid = InclinedColumnSolid {
            column: ColumnSolid {
                section: ColumnSection::Rectangular {
                    width: n("width") as f64,
                    depth: n("depth") as f64,
                },
                center: Point::new(
                    d["origin"]["x"].as_f64().unwrap() as f64,
                    d["origin"]["y"].as_f64().unwrap() as f64,
                ),
                bottom: n("zCoordinate") as f64,
                height: n("height") as f64,
                angle_degrees: n("axisRotationAngle").to_degrees() as f64,
            },
            slant_degrees: n("slantAngle").to_degrees(),
            direction_degrees: n("slantDirectionAngle").to_degrees(),
        };
        let b = fixture(&format!("{name}-bbox"));
        let b = &b["boundingBoxes3D"][0]["boundingBox3D"];
        let [lo, hi] = solid.bounds().unwrap();
        for (actual, key) in [lo.x, lo.y, lo.z, hi.x, hi.y, hi.z]
            .into_iter()
            .zip(["xMin", "yMin", "zMin", "xMax", "yMax", "zMax"])
        {
            assert!(
                (f64::from(actual) - b[key].as_f64().unwrap()).abs() < 0.00002,
                "{name} {key}"
            );
        }
    }
}
#[test]
fn vertical_geometry_preserves_existing_mesh_sections_and_rays() {
    for section in [
        ColumnSection::Circular { diameter: 0.6 },
        sample().column.section,
    ] {
        let mut s = sample();
        s.column.section = section;
        s.slant_degrees = 90.;
        s.direction_degrees = 0.;
        assert_eq!(s.bounds().unwrap(), s.column.bounds().unwrap());
        assert_eq!(
            s.mesh().unwrap().triangles,
            s.column.mesh().unwrap().triangles
        );
        assert_eq!(s.mesh().unwrap().edges, s.column.mesh().unwrap().edges);
        assert_eq!(s.section_at(0.).unwrap(), s.column.outline().unwrap());
        for (o, d) in [
            (DVec3::new(2., -1., 5.), -DVec3::Z),
            (DVec3::new(5., -1., 0.), -2. * DVec3::X),
        ] {
            assert_eq!(s.ray_hit(o, d), s.column.ray_hit(o, d));
        }
    }
}
#[test]
fn inclined_mesh_is_closed_outward_and_has_expected_volume() {
    for section in [
        ColumnSection::Circular { diameter: 0.6 },
        sample().column.section,
    ] {
        for direction in [-90., 0., 47., 180.] {
            let mut s = sample();
            s.column.section = section;
            s.direction_degrees = direction;
            let m = s.mesh().unwrap();
            let [lo, hi] = s.bounds().unwrap();
            let center = (lo + hi) / 2.;
            let mut edges = HashMap::new();
            let mut volume = 0.;
            for [a, b, c] in &m.triangles {
                assert!((*b - *a).cross(*c - *a).dot((*a + *b + *c) / 3. - center) > 0.);
                let (a, b, c) = ((*a - center), (*b - center), (*c - center));
                volume += a.dot(b.cross(c)) / 6.;
            }
            for t in &m.triangles {
                for i in 0..3 {
                    let a = t[i].to_array().map(f64::to_bits);
                    let b = t[(i + 1) % 3].to_array().map(f64::to_bits);
                    *edges.entry((a, b)).or_insert(0) += 1;
                }
            }
            for ((a, b), count) in &edges {
                assert_eq!(*count, 1);
                assert_eq!(edges.get(&(*b, *a)), Some(&1));
            }
            assert!((volume / s.volume().unwrap() - 1.).abs() < 0.004);
            assert!(
                m.triangles
                    .iter()
                    .flatten()
                    .all(|p| p.z == s.column.bottom || p.z == s.column.bottom + s.column.height)
            );
        }
    }
}
#[test]
fn inclined_circle_sections_and_analytic_rays_reject_box_corners() {
    let mut s = sample();
    s.column.section = ColumnSection::Circular { diameter: 2. };
    s.column.center = Point::default();
    s.column.bottom = 0.;
    s.direction_degrees = 0.;
    s.slant_degrees = 30.;
    let center_y = 1.0_f64 / 30.0_f64.to_radians().tan();
    for p in s.section_at(1.).unwrap() {
        assert!((p.x * p.x + ((p.y - center_y) / 2.).powi(2) - 1.).abs() < 0.000002);
    }
    let t = s
        .ray_hit(DVec3::new(3., center_y, 1.), -2. * DVec3::X)
        .unwrap();
    assert!((t - 1.).abs() < 0.000001);
    let t = s.ray_hit(DVec3::new(0., center_y, 1.), DVec3::X).unwrap();
    assert!((t - 1.).abs() < 0.000001);
    assert!(
        s.ray_hit(DVec3::new(0.9, center_y + 1.9, 1.), DVec3::X)
            .is_none()
    );
    let bounds = s.bounds().unwrap();
    let mesh = s.mesh().unwrap();
    s.column.angle_degrees = 173.;
    assert_eq!(s.bounds().unwrap(), bounds);
    assert_eq!(s.mesh().unwrap().triangles, mesh.triangles);
    assert!(s.section_at(-0.01).is_err());
    assert!(s.section_at(3.).is_err());
}
#[test]
fn invalid_and_unrepresentable_inclinations_fail_before_meshing() {
    for angle in [0., -1., 90.1, f64::NAN, f64::INFINITY, 1e-300] {
        let s = InclinedColumnSolid {
            slant_degrees: angle,
            ..sample()
        };
        assert!(s.bounds().is_err());
        assert!(s.mesh().is_err());
        assert!(s.ray_hit(DVec3::ZERO, DVec3::Z).is_none());
    }
    let s = InclinedColumnSolid {
        direction_degrees: f64::NAN,
        ..sample()
    };
    assert!(s.mesh().is_err());
}

#[test]
fn native_circle_cut_and_world_space_chords_match_ellipse() {
    let api = fixture("true-circle-api");
    let d = &api["detailsOfElements"][0]["details"];
    let a = d["slantAngle"].as_f64().unwrap();
    let x = d["origin"]["x"].as_f64().unwrap();
    let y = d["origin"]["y"].as_f64().unwrap();
    let radius = d["width"].as_f64().unwrap() / 2.;
    let cx = x + 1.1 / a.tan();
    let plan = fixture("true-circle-plan");
    for p in plan["detailsOfElements"][0]["floorPlanPolygons"][0]["coordinates"]
        .as_array()
        .unwrap()
    {
        let dx = (p["x"].as_f64().unwrap() - cx) * a.sin() / radius;
        let dy = (p["y"].as_f64().unwrap() - y) / radius;
        assert!((dx * dx + dy * dy - 1.).abs() < 1e-7);
    }
    let s = InclinedColumnSolid {
        column: ColumnSolid {
            section: ColumnSection::Circular {
                diameter: (radius * 2.) as f64,
            },
            center: Point::new(x as f64, y as f64),
            bottom: 0.,
            height: 2.8,
            angle_degrees: 30.,
        },
        slant_degrees: a.to_degrees(),
        direction_degrees: -90.,
    };
    let ring = s.section_at(1.1).unwrap();
    for (i, p) in ring.iter().enumerate() {
        let next = ring[(i + 1) % ring.len()];
        let dx = (f64::from(p.x) - cx) * a.sin() / radius;
        let dy = (f64::from(p.y) - y) / radius;
        assert!((dx * dx + dy * dy - 1.).abs() < 0.00002);
        let mx = (f64::from(p.x) + f64::from(next.x)) / 2. - cx;
        let my = (f64::from(p.y) + f64::from(next.y)) / 2. - y;
        let scale = radius / (mx * a.sin()).hypot(my);
        let chord_error = (mx * (scale - 1.)).hypot(my * (scale - 1.));
        assert!(chord_error <= ColumnSection::CHORD_ERROR + 0.000003);
    }
}

#[test]
fn two_point_axis_matches_native_sample_and_roundtrips_all_quadrants() {
    use atelier_bim::column_inclination::ColumnInclination;
    let api = fixture("two-point-api");
    let d = &api["detailsOfElements"][0]["details"];
    let base = Point::new(
        d["origin"]["x"].as_f64().unwrap() as f64,
        d["origin"]["y"].as_f64().unwrap() as f64,
    );
    let inclination =
        ColumnInclination::from_plan_axis(base, Point::new(base.x + 3.83459408, base.y), 2.8)
            .unwrap();
    assert!(
        (inclination.slant_degrees.to_radians() - d["slantAngle"].as_f64().unwrap()).abs() < 1e-7
    );
    assert!((inclination.direction_degrees + 90.).abs() < 1e-10);
    for (dx, dy) in [
        (3., 4.),
        (-3., 4.),
        (-3., -4.),
        (3., -4.),
        (0., 5.),
        (0., -5.),
        (5., 0.),
    ] {
        let top = Point::new(base.x + dx, base.y + dy);
        let i = ColumnInclination::from_plan_axis(base, top, 2.8).unwrap();
        let distance = 2.8_f64 / i.slant_degrees.to_radians().tan();
        let direction = i.direction_degrees.to_radians();
        assert!((distance * -direction.sin() - f64::from(top.x - base.x)).abs() < 1e-6);
        assert!((distance * direction.cos() - f64::from(top.y - base.y)).abs() < 1e-6);
    }
}

#[test]
fn two_point_axis_rejects_degenerate_and_nonfinite_input() {
    use atelier_bim::column_inclination::ColumnInclination;
    let base = Point::default();
    let top = Point::new(2., 3.);
    assert!(ColumnInclination::from_plan_axis(base, base, 3.).is_err());
    for height in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(ColumnInclination::from_plan_axis(base, top, height).is_err());
    }
    for point in [Point::new(f64::NAN, 0.), Point::new(0., f64::INFINITY)] {
        assert!(ColumnInclination::from_plan_axis(point, top, 3.).is_err());
        assert!(ColumnInclination::from_plan_axis(base, point, 3.).is_err());
    }
}
