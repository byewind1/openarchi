use atelier_bim::{
    column::{ColumnSection, ColumnSolid},
    geometry,
    model::Point,
};
use glam::DVec3;
use std::collections::HashMap;

fn circle(diameter: f64) -> ColumnSolid {
    ColumnSolid {
        section: ColumnSection::Circular { diameter },
        center: Point::default(),
        bottom: -0.75,
        height: 3.,
        angle_degrees: 0.,
    }
}

#[test]
fn circular_sections_keep_exact_area_and_bounded_chord_error() {
    for diameter in [0.01, 0.6, 3., 100.] {
        let solid = circle(diameter);
        let ring = solid.outline().unwrap();
        assert!(ring.len() >= 32 && ring.len() % 4 == 0);
        let radius = diameter / 2.;
        let expected = std::f64::consts::PI * radius * radius;
        assert!((solid.section.area().unwrap() - expected).abs() <= expected * 1e-6);
        for i in 0..ring.len() {
            let a = ring[i];
            let b = ring[(i + 1) % ring.len()];
            let midpoint = Point::new((a.x + b.x) / 2., (a.y + b.y) / 2.);
            assert!((a.distance(Point::default()) - radius).abs() < diameter * 1e-6);
            assert!(
                f64::from(radius - midpoint.distance(Point::default()))
                    <= ColumnSection::CHORD_ERROR + 1e-5
            );
        }
        let mut rotated = solid;
        rotated.angle_degrees = 137.;
        assert_eq!(rotated.outline().unwrap(), ring);
        assert_eq!(rotated.bounds().unwrap(), solid.bounds().unwrap());
        assert_eq!(
            serde_json::from_str::<ColumnSection>(&serde_json::to_string(&solid.section).unwrap())
                .unwrap(),
            solid.section
        );
    }
}

#[test]
fn column_mesh_is_closed_outward_and_matches_section_volume() {
    for section in [
        ColumnSection::Circular { diameter: 0.6 },
        ColumnSection::Rectangular {
            width: 0.8,
            depth: 0.4,
        },
    ] {
        let solid = ColumnSolid {
            section,
            center: Point::new(2., -1.),
            angle_degrees: 30.,
            ..circle(0.6)
        };
        let mesh = solid.mesh().unwrap();
        let center = DVec3::new(2., -1., solid.bottom + solid.height / 2.);
        let mut edges: HashMap<([u64; 3], [u64; 3]), usize> = HashMap::new();
        let mut volume = 0.;
        for [a, b, c] in &mesh.triangles {
            let normal = (*b - *a).cross(*c - *a);
            assert!(normal.length_squared() > 0.);
            assert!(normal.dot((*a + *b + *c) / 3. - center) > 0.);
            let (a, b, c) = (*a - center, *b - center, *c - center);
            volume += a.dot(b.cross(c)) / 6.;
        }
        for triangle in &mesh.triangles {
            for i in 0..3 {
                let a = triangle[i].to_array().map(f64::to_bits);
                let b = triangle[(i + 1) % 3].to_array().map(f64::to_bits);
                *edges.entry((a, b)).or_default() += 1;
            }
        }
        for ((a, b), count) in &edges {
            assert_eq!(*count, 1);
            assert_eq!(edges.get(&(*b, *a)), Some(&1));
        }
        let exact = section.area().unwrap() * solid.height;
        assert!((volume - exact).abs() / exact < 0.004);
    }
}

#[test]
fn analytic_column_rays_hit_caps_sides_tangents_and_not_box_corners() {
    let s = circle(2.);
    let cases = [
        (DVec3::new(3., 0., 0.), -DVec3::X, Some(2.)),
        (DVec3::new(0., 0., 5.), -DVec3::Z, Some(2.75)),
        (DVec3::new(0., 0., -2.), DVec3::Z, Some(1.25)),
        (DVec3::new(0., 0., 0.), DVec3::X, Some(1.)),
        (DVec3::new(1., -2., 0.), DVec3::Y, Some(2.)),
        (DVec3::new(0.9, 0.9, 5.), -DVec3::Z, None),
        (DVec3::new(3., 0., 0.), DVec3::Z, None),
        (DVec3::new(3., 0., 0.), DVec3::X, None),
        (DVec3::ZERO, DVec3::ZERO, None),
        (DVec3::new(f64::NAN, 0., 0.), DVec3::X, None),
    ];
    for (o, d, expected) in cases {
        assert_eq!(s.ray_hit(o, d), expected);
    }
    assert_eq!(s.ray_hit(DVec3::new(3., 0., 0.), -DVec3::X * 2.), Some(1.));
}

#[test]
fn rectangular_section_reuses_rotation_and_invalid_sections_are_rejected() {
    let s = ColumnSolid {
        section: ColumnSection::Rectangular {
            width: 2.,
            depth: 1.,
        },
        angle_degrees: 90.,
        ..circle(1.)
    };
    assert_eq!(
        s.outline().unwrap(),
        geometry::rotated_column_footprint(Point::default(), 2., 1., 90.)
    );
    assert!((s.ray_hit(DVec3::new(3., 0., 0.), -DVec3::X).unwrap() - 2.5).abs() < 1e-5);
    assert_eq!(s.ray_hit(DVec3::new(0.6, 0.9, 5.), -DVec3::Z), None);
    for bad in [0., -1., f64::NAN, f64::INFINITY, 101.] {
        let s = circle(bad);
        assert!(s.mesh().is_err());
        assert!(s.bounds().is_err());
        assert!(s.section.area().is_err());
        assert_eq!(s.ray_hit(DVec3::ZERO, DVec3::X), None);
    }
    let mut far = circle(0.01);
    far.center = Point::new(99999., 99999.);
    // The old f32 model could not represent a 10 mm circle here. The f64
    // model must retain this legal local-frame solid; exact range endpoints
    // remain forbidden.
    assert!(far.mesh().is_ok());
    far.center = Point::new(100000., 99999.);
    assert!(far.mesh().is_err());
    let mut s = circle(1.);
    s.angle_degrees = f64::NAN;
    assert!(s.mesh().is_err());
    let mut s = circle(1.);
    s.bottom = f64::MAX;
    assert!(s.mesh().is_err());
}

#[test]
fn section_creation_switch_uses_observed_depth_and_never_restores_old_width() {
    for (width, depth) in [(0.8, 0.4), (0.4, 0.8)] {
        let rectangle = ColumnSection::Rectangular { width, depth };
        assert_eq!(rectangle.with_circular(false), rectangle);
        let circle = rectangle.with_circular(true);
        assert_eq!(circle, ColumnSection::Circular { diameter: depth });
        assert_eq!(circle.with_circular(true), circle);
        assert_eq!(
            circle.with_circular(false),
            ColumnSection::Rectangular {
                width: depth,
                depth
            }
        );
    }
    assert_eq!(
        ColumnSection::Circular { diameter: 0.6 }.with_circular(false),
        ColumnSection::Rectangular {
            width: 0.6,
            depth: 0.6
        }
    );
}
