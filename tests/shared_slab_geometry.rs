use atelier_bim::{
    editor::Editor,
    geometry,
    model::{ArcSegment, ModelAdapter, Point, Project, SlabContour, SlabEdgeSettings},
    slab_geometry::{self, VolumeSource},
    storage::JsonAdapter,
};
use std::collections::BTreeMap;
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
fn rectangle(w: f64, h: f64) -> Vec<Point> {
    vec![p(0., 0.), p(w, 0.), p(w, h), p(0., h)]
}
fn editor(mut c: SlabContour, origin: f64) -> Editor {
    for point in c
        .points
        .iter_mut()
        .chain(c.holes.iter_mut().flat_map(|h| h.points.iter_mut()))
    {
        point.x += origin;
        point.y += origin;
    }
    let mut e = Editor::new(Project::default());
    e.thickness = 0.3;
    e.place_slab_contour(c).unwrap();
    e
}
fn closed_and_oriented(g: &slab_geometry::DerivedSlab) {
    let origin = g.bounds[0];
    let mut volume = 0.;
    let mut edges: BTreeMap<([u64; 3], [u64; 3]), (usize, i32)> = BTreeMap::new();
    for [a, b, c] in g.triangles() {
        volume += (a - origin).dot((b - origin).cross(c - origin)) / 6.;
        for (a, b) in [(a, b), (b, c), (c, a)] {
            let a = a.to_array().map(|v| if v == 0. { 0. } else { v }.to_bits());
            let b = b.to_array().map(|v| if v == 0. { 0. } else { v }.to_bits());
            let (key, sign) = if a < b { ((a, b), 1) } else { ((b, a), -1) };
            let x = edges.entry(key).or_default();
            x.0 += 1;
            x.1 += sign;
        }
    }
    assert!(volume > 0.);
    assert!(
        edges
            .values()
            .all(|&(count, direction)| count == 2 && direction == 0),
        "mesh edges must have two opposite neighbours"
    );
}
#[test]
fn shared_holed_slab_has_closed_oriented_surface_and_revision_bound_readonly_identity() {
    for reverse in [false, true] {
        for origin in [0., 99900., -99900.] {
            let mut c = SlabContour {
                points: rectangle(8., 6.),
                holes: vec![
                    SlabContour {
                        points: vec![p(1., 1.), p(2., 1.), p(2., 2.), p(1., 2.)],
                        ..Default::default()
                    },
                    SlabContour {
                        points: vec![p(4., 3.), p(6., 3.), p(6., 4.), p(4., 4.)],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            };
            if reverse {
                c.points.reverse();
                for h in &mut c.holes {
                    h.points.reverse();
                }
            }
            let e = editor(c, origin);
            let before = format!(
                "{:?}/{:?}/{:?}/{:?}/{:?}",
                e.project,
                e.undo,
                e.redo,
                e.revision,
                e.selection_ids()
            );
            let g = e.slab_geometry(1).unwrap();
            closed_and_oriented(&g);
            assert_eq!(g.plan_area_m2, 45.);
            assert!((g.volume_m3 - 13.5).abs() < 1e-10);
            assert_eq!(g.volume_source, VolumeSource::AnalyticPrism);
            assert_eq!(g.identity.input_revision, Some(e.revision));
            assert_eq!(g.identity.document_session, Some(e.document_session()));
            assert_eq!(g, e.slab_geometry(1).unwrap());
            assert_eq!(
                format!(
                    "{:?}/{:?}/{:?}/{:?}/{:?}",
                    e.project,
                    e.undo,
                    e.redo,
                    e.revision,
                    e.selection_ids()
                ),
                before
            );
            let restored = JsonAdapter
                .import(&JsonAdapter.export(&e.project).unwrap())
                .unwrap();
            assert_eq!(
                slab_geometry::derive(&restored.elements[0], 0.)
                    .unwrap()
                    .identity
                    .input_fingerprint,
                g.identity.input_fingerprint
            );
            assert_eq!(e.slab_geometry(999).unwrap_err().code, "ELEMENT_NOT_FOUND");
        }
    }
}
#[test]
fn tapered_slabs_use_true_bottom_extents_and_independent_polynomial_volume() {
    for (angle, sign) in [(45., -1.), (135., 1.)] {
        for origin in [0., 99900., -99900.] {
            let c = SlabContour {
                points: rectangle(4., 3.),
                edge_settings: vec![
                    SlabEdgeSettings {
                        angle_degrees: angle,
                        surface: None
                    };
                    4
                ],
                ..Default::default()
            };
            let e = editor(c, origin);
            let g = e.slab_geometry(1).unwrap();
            closed_and_oriented(&g);
            let expected = 12. * 0.3 + sign * 7. * 0.3_f64.powi(2) + 4. * 0.3_f64.powi(3) / 3.;
            assert!(
                (g.volume_m3 - expected).abs() < 1e-9,
                "angle={angle} origin={origin}: {} != {expected}",
                g.volume_m3
            );
            assert_eq!(g.volume_source, VolumeSource::SampledSurface);
            let outward = (sign * 0.3).max(0.);
            assert!((g.bounds[0].x - (origin - outward)).abs() < 1e-10);
            assert!((g.bounds[1].x - (origin + 4. + outward)).abs() < 1e-10);
            for corner in geometry::corners(&e.project, &e.project.elements[0]) {
                assert!(corner.cmpge(g.bounds[0]).all() && corner.cmple(g.bounds[1]).all());
            }
        }
    }
}
#[test]
fn circular_arc_area_stays_parametric_while_mesh_sampling_is_explicit() {
    let e = editor(
        SlabContour {
            points: rectangle(4., 3.),
            arcs: vec![
                Some(ArcSegment {
                    radius: 2.,
                    clockwise: false,
                    large_arc: false,
                }),
                None,
                None,
                None,
            ],
            ..Default::default()
        },
        0.,
    );
    let g = e.slab_geometry(1).unwrap();
    closed_and_oriented(&g);
    assert!((g.plan_area_m2 - (12. + 2. * std::f64::consts::PI)).abs() < 1e-10);
    assert_eq!(g.volume_source, VolumeSource::AnalyticPrism);
    assert!((g.bounds[0].y + 2.).abs() < 1e-12);
    assert_eq!(g.top_contour.arcs[0].unwrap().radius, 2.);
}

#[test]
fn sloped_slab_marquee_picking_must_use_the_actual_tapered_solid() {
    use atelier_bim::{
        editor::{MarqueeArea, MarqueeScope},
        geometry::{Camera, ModelViewFilter},
    };
    let e = editor(
        SlabContour {
            points: rectangle(4., 3.),
            edge_settings: vec![
                SlabEdgeSettings {
                    angle_degrees: 45.,
                    surface: None
                };
                4
            ],
            ..Default::default()
        },
        0.,
    );
    let mut area = MarqueeArea::new(
        vec![p(-1., -1.), p(5., -1.), p(5., 4.), p(-1., 4.)],
        MarqueeScope::CurrentStory,
        0,
    )
    .unwrap();
    area.z_range = Some((-0.3, -0.15));
    for (x, expected) in [(0.05, None), (1., Some(1))] {
        let camera = Camera {
            target: glam::DVec3::new(x, 1., -0.15),
            pitch: 1.45,
            yaw: 0.,
            span: 5.,
        };
        assert_eq!(
            camera.pick_in_view(
                &e.project,
                [0., 0.],
                1.,
                ModelViewFilter {
                    marquee: Some(&area),
                    ..Default::default()
                }
            ),
            expected,
            "x={x}"
        );
    }
}

fn clipped_volume(mesh: slab_geometry::ClippedSlabMesh) -> f64 {
    let triangles: Vec<_> = mesh.triangles().collect();
    let origin = triangles.first().map(|t| t[0]).unwrap_or(glam::DVec3::ZERO);
    triangles
        .into_iter()
        .map(|[a, b, c]| (a - origin).dot((b - origin).cross(c - origin)) / 6.)
        .sum()
}
#[test]
fn clipping_preserves_holes_concavity_taper_and_independent_slice_volume() {
    let clip = vec![p(-1., -1.), p(5., -1.), p(5., 4.), p(-1., 4.)];
    for (angle, sign) in [(45., -1.), (135., 1.)] {
        let e = editor(
            SlabContour {
                points: rectangle(4., 3.),
                edge_settings: vec![
                    SlabEdgeSettings {
                        angle_degrees: angle,
                        surface: None
                    };
                    4
                ],
                ..Default::default()
            },
            0.,
        );
        let g = e.slab_geometry(1).unwrap();
        let integral = |t: f64| 12. * t + sign * 7. * t * t + 4. * t * t * t / 3.;
        let cut = g.clipped(&clip, Some((-0.3, -0.15))).unwrap();
        assert_eq!(cut.identity, Some(g.identity.clone()));
        assert!((clipped_volume(cut) - (integral(0.3) - integral(0.15))).abs() < 1e-9);
        assert!(g.clipped(&clip, Some((f64::NAN, 0.))).is_err());
        assert!(g.clipped(&clip, Some((0., 0.))).unwrap().top.is_empty());
    }
    let e = editor(
        SlabContour {
            points: vec![
                p(0., 0.),
                p(4., 0.),
                p(4., 2.),
                p(2., 2.),
                p(2., 4.),
                p(0., 4.),
            ],
            holes: vec![SlabContour {
                points: vec![p(0.5, 0.5), p(1.5, 0.5), p(1.5, 1.5), p(0.5, 1.5)],
                ..Default::default()
            }],
            ..Default::default()
        },
        0.,
    );
    let g = e.slab_geometry(1).unwrap();
    let cut = g.clipped(&clip, None).unwrap();
    assert!((clipped_volume(cut) - 3.3).abs() < 1e-9);
    let hole = vec![p(0.6, 0.6), p(1.4, 0.6), p(1.4, 1.4), p(0.6, 1.4)];
    assert!(g.clipped(&hole, None).unwrap().triangles().next().is_none());
}
#[test]
fn collinear_boundary_nodes_remain_conforming_in_caps_and_sides() {
    let e = editor(
        SlabContour {
            points: vec![p(0., 0.), p(2., 0.), p(4., 0.), p(4., 3.), p(0., 3.)],
            ..Default::default()
        },
        0.,
    );
    let g = e.slab_geometry(1).unwrap();
    closed_and_oriented(&g);
    let clip = vec![p(-1., -1.), p(5., -1.), p(5., 4.), p(-1., 4.)];
    assert!((clipped_volume(g.clipped(&clip, None).unwrap()) - 3.6).abs() < 1e-9);
}

#[test]
fn holed_taper_clipping_matches_polynomial_slice_in_both_windings() {
    for angle in [45., 135.] {
        for reverse in [false, true] {
            let mut outer = rectangle(8., 6.);
            let mut hole = vec![p(2., 2.), p(3., 2.), p(3., 3.), p(2., 3.)];
            if reverse {
                outer.reverse();
                hole.reverse();
            }
            let setting = SlabEdgeSettings {
                angle_degrees: angle,
                surface: None,
            };
            let e = editor(
                SlabContour {
                    points: outer,
                    edge_settings: vec![setting.clone(); 4],
                    holes: vec![SlabContour {
                        points: hole,
                        edge_settings: vec![setting; 4],
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                0.,
            );
            let g = e.slab_geometry(1).unwrap();
            closed_and_oriented(&g);
            let clip = vec![p(-1., -1.), p(9., -1.), p(9., 7.), p(-1., 7.)];
            let sign = if angle < 90. { -1. } else { 1. };
            let integral = |t: f64| 47. * t + sign * 16. * t * t;
            assert!(
                (clipped_volume(g.clipped(&clip, Some((-0.3, -0.15))).unwrap())
                    - (integral(0.3) - integral(0.15)))
                .abs()
                    < 1e-8
            );
        }
    }
}
#[test]
fn curved_partition_keeps_the_sampled_solid_and_structured_rejections() {
    let e = editor(
        SlabContour {
            points: rectangle(4., 3.),
            arcs: vec![
                Some(ArcSegment {
                    radius: 2.,
                    clockwise: false,
                    large_arc: false,
                }),
                None,
                None,
                None,
            ],
            ..Default::default()
        },
        0.,
    );
    let g = e.slab_geometry(1).unwrap();
    let clip = vec![p(-1., -3.), p(5., -3.), p(5., 4.), p(-1., 4.)];
    assert!((clipped_volume(g.clipped(&clip, None).unwrap()) - g.sampled_volume_m3).abs() < 1e-8);
    let before = e.project.clone();
    let mut invalid = before.elements[0].clone();
    if let atelier_bim::model::Shape::Slab {
        ref mut thickness, ..
    } = invalid.shape
    {
        *thickness = f64::NAN;
    }
    let error = slab_geometry::derive(&invalid, 0.).unwrap_err();
    assert_eq!(error.code, "INVALID_SLAB_PARAMETERS");
    assert_eq!(error.element_id, 1);
    assert_eq!(error.feature, "parameters");
    assert_eq!(error.severity, slab_geometry::DiagnosticSeverity::Error);
    assert_eq!(e.project, before);
}

#[test]
fn curved_taper_slice_has_independently_integrated_sampled_volume() {
    let half_circle_factor = 0.5 * 36. * (std::f64::consts::PI / 36.).sin();
    for angle in [45., 135.] {
        let e = editor(
            SlabContour {
                points: rectangle(4., 3.),
                arcs: vec![
                    Some(ArcSegment {
                        radius: 2.,
                        clockwise: false,
                        large_arc: false,
                    }),
                    None,
                    None,
                    None,
                ],
                edge_settings: vec![
                    SlabEdgeSettings {
                        angle_degrees: angle,
                        surface: None
                    };
                    4
                ],
                ..Default::default()
            },
            0.,
        );
        let g = e.slab_geometry(1).unwrap();
        closed_and_oriented(&g);
        let sign = if angle < 90. { -1. } else { 1. };
        let integral = |t: f64| {
            (12. + 4. * half_circle_factor) * t
                + sign * (10. + 4. * half_circle_factor) * t * t / 2.
                + (2. + half_circle_factor) * t * t * t / 3.
        };
        assert!(
            (g.sampled_volume_m3 - integral(0.3)).abs() < 1e-8,
            "angle={angle}"
        );
        let clip = vec![p(-1., -3.), p(5., -3.), p(5., 4.), p(-1., 4.)];
        assert!(
            (clipped_volume(g.clipped(&clip, Some((-0.3, -0.15))).unwrap())
                - (integral(0.3) - integral(0.15)))
            .abs()
                < 1e-8
        );
    }
}
#[test]
fn circular_hole_side_slope_moves_the_hole_boundary_radially() {
    let arcs = vec![
        Some(ArcSegment {
            radius: 1.,
            clockwise: false,
            large_arc: false
        });
        4
    ];
    let setting = SlabEdgeSettings {
        angle_degrees: 45.,
        surface: None,
    };
    let e = editor(
        SlabContour {
            points: rectangle(8., 6.),
            edge_settings: vec![setting.clone(); 4],
            holes: vec![SlabContour {
                points: vec![p(3., 3.), p(4., 2.), p(5., 3.), p(4., 4.)],
                arcs,
                edge_settings: vec![setting; 4],
                ..Default::default()
            }],
            ..Default::default()
        },
        0.,
    );
    let g = e.slab_geometry(1).unwrap();
    closed_and_oriented(&g);
    for point in &g.bottom_contour.holes[0].points {
        assert!(
            (point.distance(p(4., 3.)) - 1.3).abs() < 1e-10,
            "bottom node {point:?}"
        );
    }
    let circle_factor = 0.5 * 72. * (std::f64::consts::TAU / 72.).sin();
    let integral = |t: f64| {
        (48. - circle_factor) * t - (28. + 2. * circle_factor) * t * t / 2.
            + (4. - circle_factor) * t * t * t / 3.
    };
    assert!((g.sampled_volume_m3 - integral(0.3)).abs() < 1e-8);
    let clip = vec![p(-1., -1.), p(9., -1.), p(9., 7.), p(-1., 7.)];
    assert!(
        (clipped_volume(g.clipped(&clip, Some((-0.3, -0.15))).unwrap())
            - (integral(0.3) - integral(0.15)))
        .abs()
            < 1e-8
    );
}

#[test]
fn preview_commit_and_large_offset_clipping_share_geometry_and_input_identity() {
    use atelier_bim::command::ModelCommand;
    for origin in [0., 99900., -99900.] {
        let mut e = Editor::new(Project::default());
        e.thickness = 0.3;
        let mut c = SlabContour {
            points: rectangle(4., 3.),
            edge_settings: vec![
                SlabEdgeSettings {
                    angle_degrees: 45.,
                    surface: None
                };
                4
            ],
            ..Default::default()
        };
        for p in &mut c.points {
            p.x += origin;
            p.y += origin;
        }
        let element = e.slab_creation_candidate(c).unwrap();
        let candidate = e
            .prepare_change(e.revision, vec![ModelCommand::AddElement { element }])
            .unwrap();
        let preview = slab_geometry::derive(&candidate.project().elements[0], 0.).unwrap();
        e.commit_change("preview-solid", &candidate).unwrap();
        let actual = e.slab_geometry(1).unwrap();
        assert_eq!(
            preview.identity.input_fingerprint,
            actual.identity.input_fingerprint
        );
        assert_eq!(
            preview.triangles().collect::<Vec<_>>(),
            actual.triangles().collect::<Vec<_>>()
        );
        assert_eq!(preview.bounds, actual.bounds);
        let clip = vec![
            p(origin - 1., origin - 1.),
            p(origin + 5., origin - 1.),
            p(origin + 5., origin + 4.),
            p(origin - 1., origin + 4.),
        ];
        let integral = |t: f64| 12. * t - 7. * t * t + 4. * t * t * t / 3.;
        assert!(
            (clipped_volume(actual.clipped(&clip, Some((-0.3, -0.15))).unwrap())
                - (integral(0.3) - integral(0.15)))
            .abs()
                < 1e-8,
            "origin={origin}"
        );
        let before = e.project.clone();
        e.project.stories[0].elevation = 3.2;
        e.commit(before);
        let raised = e.slab_geometry(1).unwrap();
        assert_eq!(raised.identity.input_revision, Some(e.revision));
        assert_ne!(
            raised.identity.input_fingerprint,
            actual.identity.input_fingerprint
        );
        assert!((raised.bounds[0].z - actual.bounds[0].z - 3.2).abs() < 1e-12);
    }
}
