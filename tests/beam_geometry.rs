use atelier_bim::{
    beam::BeamGeometry,
    geometry,
    model::{Material, Point, Project, Shape},
};
use glam::DVec3;
fn spec() -> BeamGeometry {
    BeamGeometry {
        start: Point::new(-12.966213002, 25.858157565),
        end: Point::new(25.379727801, 25.858157565),
        start_top: 3.,
        width: 0.3,
        height: 0.4,
        slant_angle: 30.,
        arc: None,
    }
}
#[test]
fn inclined_beam_matches_ac29_positive_and_negative_vertical_cut_bounds() {
    for (g, expected) in [
        (
            spec(),
            [
                -12.966213002,
                25.379727801,
                25.708157565,
                26.008157565,
                2.538119785,
                25.139039245,
            ],
        ),
        (
            BeamGeometry {
                end: Point::new(25.379727801, 45.031127967),
                slant_angle: -30.,
                ..spec()
            },
            [
                -13.033295041,
                25.44680984,
                25.723993487,
                45.165292045,
                -22.214078569,
                3.,
            ],
        ),
    ] {
        let solid = g.solid().unwrap();
        let min = solid
            .corners
            .iter()
            .copied()
            .fold(DVec3::splat(f64::INFINITY), DVec3::min);
        let max = solid
            .corners
            .iter()
            .copied()
            .fold(DVec3::splat(f64::NEG_INFINITY), DVec3::max);
        for (actual, expected) in [min.x, max.x, min.y, max.y, min.z, max.z]
            .into_iter()
            .zip(expected)
        {
            assert!((actual - expected).abs() < 1e-4, "{actual} != {expected}");
        }
        let axis = (solid.corners[5] - solid.corners[4]).normalize();
        let normal = DVec3::new(
            -axis.x * axis.z,
            -axis.y * axis.z,
            axis.x * axis.x + axis.y * axis.y,
        )
        .normalize();
        assert!(((solid.corners[4] - solid.corners[0]).dot(normal) - g.height).abs() < 1e-5);
        assert!(
            (solid.axis_length
                - g.start.distance(g.end) as f64 / (g.slant_angle as f64).to_radians().cos())
            .abs()
                < 1e-5
        );
        assert!((solid.volume - solid.axis_length * g.width as f64 * g.height as f64).abs() < 1e-9);
    }
}
#[test]
fn beam_core_keeps_horizontal_corners_and_rejects_invalid_or_collapsed_solids() {
    let g = BeamGeometry {
        start: Point::new(0., 0.),
        end: Point::new(3., 4.),
        slant_angle: 0.,
        ..spec()
    };
    let mut project = Project::default();
    project.add(
        0,
        Material::Concrete,
        Shape::Beam {
            start: g.start,
            end: g.end,
            width: g.width,
            height: g.height,
            top_offset: g.start_top,
            slant_angle: 0.,
            arc: None,
        },
    );
    let before = project.clone();
    assert_eq!(
        g.solid().unwrap().corners,
        geometry::corners(&project, &project.elements[0])
    );
    assert_eq!(project, before);
    for bad in [
        BeamGeometry {
            slant_angle: 90.,
            ..g
        },
        BeamGeometry {
            slant_angle: f64::NAN,
            ..g
        },
        BeamGeometry { width: 0., ..g },
        BeamGeometry {
            height: f64::INFINITY,
            ..g
        },
        BeamGeometry { end: g.start, ..g },
        BeamGeometry {
            start_top: 100000.,
            ..g
        },
        BeamGeometry {
            end: Point::new(99000., 0.),
            slant_angle: 89.,
            ..g
        },
    ] {
        assert!(bad.solid().is_err());
    }
    for angle in [-89., 89.] {
        assert!(
            BeamGeometry {
                end: Point::new(1., 0.),
                slant_angle: angle,
                ..g
            }
            .solid()
            .is_ok()
        );
    }
}

#[test]
fn slanted_beam_xy_z_clipping_keeps_slopes_caps_and_volume() {
    let g = BeamGeometry {
        start: Point::new(0., 0.),
        end: Point::new(4., 0.),
        start_top: 0.,
        width: 2.,
        height: std::f64::consts::FRAC_1_SQRT_2,
        slant_angle: 45.,
        arc: None,
    };
    let volume = |triangles: &[[DVec3; 3]]| -> f64 {
        triangles
            .iter()
            .map(|t| t[0].dot(t[1].cross(t[2])) / 6.)
            .sum::<f64>()
            .abs()
    };
    let clip = [
        Point::new(2., -0.5),
        Point::new(3., -0.5),
        Point::new(3., 0.5),
        Point::new(2., 0.5),
    ];
    for (xy, z, expected) in [
        (None, None, 8.),
        (Some(clip.as_slice()), None, 1.),
        (None, Some((1., 2.)), 2.),
        (Some(clip.as_slice()), Some((1., 2.)), 0.5),
    ] {
        let mesh = g.mesh(xy, z).unwrap();
        assert!(
            (volume(&mesh.triangles) - expected).abs() < 1e-5,
            "volume {} != {expected}",
            volume(&mesh.triangles)
        );
        assert!(!mesh.triangles.is_empty());
        for t in &mesh.triangles {
            for p in t {
                assert!(p.z <= p.x + 1e-5 && p.z >= p.x - 1. - 1e-5, "{p:?}");
                if let Some((lo, hi)) = z {
                    assert!(p.z >= lo - 1e-5 && p.z <= hi + 1e-5);
                }
            }
        }
    }
    assert!(g.mesh(None, Some((10., 11.))).unwrap().triangles.is_empty());
    let reversed = BeamGeometry {
        start: g.end,
        end: g.start,
        start_top: 4.,
        slant_angle: -45.,
        ..g
    };
    assert!(
        (volume(
            &reversed
                .mesh(Some(&clip), Some((1., 2.)))
                .unwrap()
                .triangles
        ) - 0.5)
            .abs()
            < 1e-5
    );
}
