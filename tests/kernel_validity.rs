use atelier_bim::{
    beam::BeamGeometry,
    beam_geometry,
    command::ModelCommand,
    derived_geometry::inspect_closed_surface,
    editor::Editor,
    model::{ArcSegment, Point, Project},
};
use glam::DVec3;
fn cube(origin: DVec3, size: f64) -> Vec<[DVec3; 3]> {
    let v: Vec<_> = (0..8)
        .map(|i| {
            origin
                + DVec3::new(
                    if matches!(i % 4, 1 | 2) { size } else { 0. },
                    if matches!(i % 4, 2 | 3) { size } else { 0. },
                    if i < 4 { 0. } else { size },
                )
        })
        .collect();
    atelier_bim::geometry::FACES
        .iter()
        .flat_map(|f| [[v[f[0]], v[f[1]], v[f[2]]], [v[f[0]], v[f[2]], v[f[3]]]])
        .collect()
}
#[test]
fn disconnected_inverted_material_shell_cannot_hide_behind_a_larger_positive_shell() {
    let mut mesh = cube(DVec3::ZERO, 2.);
    let mut wrong = cube(DVec3::new(10., 0., 0.), 1.);
    for t in &mut wrong {
        t.swap(1, 2);
    }
    mesh.extend(wrong);
    assert!(inspect_closed_surface(&mesh).is_err());
}
#[test]
fn disconnected_positive_bodies_use_per_component_local_volume_origins() {
    let mut mesh = cube(DVec3::new(-99900., -99900., 0.), 0.01);
    mesh.extend(cube(DVec3::new(99900., 99900., 0.), 0.01));
    let m = inspect_closed_surface(&mesh).unwrap();
    assert!((m.signed_volume_m3 - 2e-6).abs() < 1e-12);
}
#[test]
fn positive_thin_beam_clip_keeps_its_independent_volume() {
    let g = beam_geometry::snapshot(BeamGeometry {
        start: Point::default(),
        end: Point::new(4., 0.),
        start_top: 3.,
        width: 0.3,
        height: 0.5,
        slant_angle: 0.,
        arc: None,
    })
    .unwrap();
    let mesh = g.clipped(None, Some((2.7, 2.700001))).unwrap();
    let metrics = inspect_closed_surface(&mesh.triangles).unwrap();
    assert!((metrics.signed_volume_m3 - 1.2e-6).abs() < 1e-12);
}
#[test]
fn finite_but_unrepresentable_arc_is_rejected_without_consuming_real_redo() {
    let mut e = Editor::new(Project::default());
    e.place_wall_segments(vec![(Point::default(), Point::new(4., 0.))])
        .unwrap();
    e.undo();
    let before = format!(
        "{:?}/{:?}/{:?}/{:?}/{:?}",
        e.project,
        e.revision,
        e.undo,
        e.redo,
        e.selection_ids()
    );
    let bad = e.polyline_creation_candidate(
        vec![Point::default(), Point::new(1., 0.)],
        vec![Some(ArcSegment {
            radius: 1e308,
            clockwise: false,
            large_arc: false,
        })],
    );
    assert!(e.apply(ModelCommand::AddElement { element: bad }).is_err());
    assert_eq!(
        before,
        format!(
            "{:?}/{:?}/{:?}/{:?}/{:?}",
            e.project,
            e.revision,
            e.undo,
            e.redo,
            e.selection_ids()
        )
    );
    e.redo();
    assert_eq!(e.project.elements.len(), 1);
}

fn impossible_arc(e: &Editor) -> atelier_bim::model::Element {
    e.polyline_creation_candidate(
        vec![Point::default(), Point::new(1., 0.)],
        vec![Some(ArcSegment {
            radius: 1e308,
            clockwise: false,
            large_arc: false,
        })],
    )
}
#[test]
fn bad_source_is_blocked_in_prepare_legacy_and_json_while_query_remains_readonly() {
    use atelier_bim::{model::ModelAdapter, storage::JsonAdapter};
    let mut e = Editor::new(Project::default());
    e.place_wall_segments(vec![(Point::default(), Point::new(4., 0.))])
        .unwrap();
    e.undo();
    let before = e.project.clone();
    let revision = e.revision;
    let undo = format!("{:?}", e.undo);
    let redo = format!("{:?}", e.redo);
    let selection = e.selection_ids();
    let bad = impossible_arc(&e);
    assert!(
        e.prepare_change(
            revision,
            vec![ModelCommand::AddElement {
                element: bad.clone()
            }]
        )
        .is_err()
    );
    assert_eq!(e.project, before);
    assert_eq!(e.revision, revision);
    e.project.elements.push(bad.clone());
    e.project.next_id = bad.id + 1;
    let raw = e.project.clone();
    let report = e.geometry_report();
    assert_eq!(e.project, raw);
    assert_eq!(report.input_revision, Some(revision));
    let d = report.first_error().unwrap();
    assert_eq!(d.element_id, bad.id);
    assert_eq!(d.code, "INVALID_POLYLINE_GEOMETRY");
    assert!(!d.feature.is_empty());
    assert!(!d.message.is_empty());
    assert!(JsonAdapter.export(&raw).is_err());
    let bytes = serde_json::to_vec(&raw).unwrap();
    assert!(JsonAdapter.import(&bytes).is_err());
    assert!(!e.commit_checked(before.clone()));
    assert_eq!(e.project, before);
    assert_eq!(e.revision, revision);
    assert_eq!(format!("{:?}", e.undo), undo);
    assert_eq!(format!("{:?}", e.redo), redo);
    assert_eq!(e.selection_ids(), selection);
    let good = e.polyline_creation_candidate(vec![Point::default(), Point::new(1., 0.)], vec![]);
    let prepared = e
        .prepare_change(revision, vec![ModelCommand::AddElement { element: good }])
        .unwrap();
    let first = e.commit_change("after-source-error", &prepared).unwrap();
    assert_eq!(
        e.commit_change("after-source-error", &prepared).unwrap(),
        first
    );
    assert_eq!(e.project.elements.len(), 1);
    assert_eq!(e.undo.len(), 1);
}
#[test]
fn legacy_diameter_compatibility_has_explicit_warning_and_keeps_parameters() {
    use atelier_bim::derived_geometry::DiagnosticSeverity;
    let mut e = Editor::new(Project::default());
    let points = vec![Point::new(-1.000025, 0.), Point::new(1.000025, 0.)];
    let arcs = vec![Some(ArcSegment {
        radius: 1.,
        clockwise: false,
        large_arc: false,
    })];
    let id = e.place_polyline(points.clone(), arcs.clone()).unwrap();
    let before = e.project.clone();
    e.project.validate().unwrap();
    let report = e.geometry_report();
    assert!(report.first_error().is_none());
    let d = report
        .diagnostics
        .iter()
        .find(|d| d.code == "ARC_ENDPOINT_RADIUS_TOLERANCE")
        .unwrap();
    assert_eq!(d.severity, DiagnosticSeverity::Warning);
    assert_eq!(d.element_id, id);
    assert_eq!(
        e.polyline_geometry(id).unwrap().geometry.parameters(),
        (points.as_slice(), arcs.as_slice())
    );
    assert_eq!(e.project, before);
}
#[test]
fn fully_consumed_wall_is_explicit_empty_material_without_false_solid_error() {
    use atelier_bim::model::OpeningKind;
    let mut e = Editor::new(Project::default());
    e.height = 3.;
    e.place_wall_segments(vec![(Point::default(), Point::new(6., 0.))])
        .unwrap();
    let mut d = OpeningKind::Door.defaults();
    d.width = 6.;
    d.height = 3.;
    e.place_opening_on_wall(OpeningKind::Door, 1, 3., d)
        .unwrap();
    e.project.validate().unwrap();
    let report = e.geometry_report();
    assert!(report.first_error().is_none());
    assert_eq!(report.explicit_empty_material, vec![1]);
    assert_eq!(report.inspected_elements, 2);
}

#[test]
fn rotated_minimum_wall_projects_endpoint_stations_with_coordinate_roundoff() {
    for origin in [0., 99900., -99900.] {
        let mut e = Editor::new(Project::default());
        e.place_wall_segments(vec![(
            Point::new(origin, origin),
            Point::new(origin + 0.01, origin),
        )])
        .unwrap();
        let mut wall = e.project.elements[0].clone();
        if let atelier_bim::model::Shape::Wall { end, .. } = &mut wall.shape {
            let x = end.x - origin;
            let (sin, cos) = 30_f64.to_radians().sin_cos();
            *end = Point::new(origin + x * cos, origin + x * sin);
        }
        e.project.elements[0] = wall;
        let g = e.wall_geometry(1).unwrap_or_else(|d| {
            panic!("origin {origin}: {d:?}; {:?}", e.project.elements[0].shape)
        });
        let volume = inspect_closed_surface(&g.geometry.mesh.triangles)
            .unwrap()
            .signed_volume_m3;
        assert!((volume - 0.01 * e.thickness * e.height).abs() < 1e-10);
    }
}

#[test]
fn slab_aggregate_clip_is_closed_and_matches_independent_prism_with_hole_volume() {
    use atelier_bim::model::{Material, Shape, SlabContour};
    let p = |x, y| Point::new(x, y);
    let mut project = Project::default();
    project.add(
        0,
        Material::Concrete,
        Shape::Slab {
            min: p(0., 0.),
            max: p(4., 4.),
            thickness: 0.3,
        },
    );
    project.elements[0].slab_contour = Some(SlabContour {
        points: vec![p(0., 0.), p(4., 0.), p(4., 4.), p(0., 4.)],
        holes: vec![SlabContour {
            points: vec![p(1., 1.), p(2., 1.), p(2., 2.), p(1., 2.)],
            ..Default::default()
        }],
        ..Default::default()
    });
    project.validate().unwrap();
    let e = Editor::new(project);
    let g = e.slab_geometry(1).unwrap();
    let clip = [p(0.4, 0.2), p(3.3, 0.2), p(3.3, 2.7), p(0.4, 2.7)];
    let out = g.clipped(&clip, Some((-0.22, -0.08))).unwrap();
    let triangles: Vec<_> = out.triangles().collect();
    let metrics = inspect_closed_surface(&triangles).unwrap();
    let expected = ((3.3 - 0.4) * (2.7 - 0.2) - 1.) * 0.14;
    assert!(
        (metrics.signed_volume_m3 - expected).abs() < 1e-8,
        "{} vs {expected}",
        metrics.signed_volume_m3
    );
}

#[test]
fn slab_aggregate_curved_hole_and_slope_remain_closed_under_rotated_concave_cuts() {
    use atelier_bim::model::{SlabContour, SlabEdgeSettings};
    for origin in [0., 99900., -99900.] {
        for rotation in [0_f64, 0.37] {
            for slope in [90., 45., 135.] {
                let point = |x: f64, y: f64| {
                    Point::new(
                        origin + x * rotation.cos() - y * rotation.sin(),
                        origin + x * rotation.sin() + y * rotation.cos(),
                    )
                };
                let edge = SlabEdgeSettings {
                    angle_degrees: slope,
                    surface: None,
                };
                let c = SlabContour {
                    points: vec![point(0., 0.), point(4., 0.), point(4., 4.), point(0., 4.)],
                    edge_settings: vec![edge.clone(); 4],
                    holes: vec![SlabContour {
                        points: vec![
                            point(2.5, 2.),
                            point(2., 2.5),
                            point(1.5, 2.),
                            point(2., 1.5),
                        ],
                        arcs: vec![
                            Some(ArcSegment {
                                radius: 0.5,
                                clockwise: false,
                                large_arc: false
                            });
                            4
                        ],
                        edge_settings: vec![edge.clone(); 4],
                        ..Default::default()
                    }],
                    ..Default::default()
                };
                let mut e = Editor::new(Project::default());
                e.thickness = 0.3;
                e.place_slab_contour(c).unwrap();
                let g = e.slab_geometry(1).unwrap();
                for concave in [false, true] {
                    let clip = if concave {
                        vec![
                            point(0.4, 0.2),
                            point(3.3, 0.2),
                            point(3.3, 1.7),
                            point(2.8, 1.7),
                            point(2.8, 3.6),
                            point(0.4, 3.6),
                        ]
                    } else {
                        vec![
                            point(0.4, 0.2),
                            point(3.3, 0.2),
                            point(3.3, 3.6),
                            point(0.4, 3.6),
                        ]
                    };
                    for range in [(-0.22, -0.08), (-0.3, -0.15)] {
                        let out = g.clipped(&clip, Some(range)).unwrap();
                        let report = out
                            .normalization
                            .as_ref()
                            .expect("clipped aggregate must report normalization");
                        assert!(report.max_displacement_m <= report.merge_distance_m);
                        assert!(report.max_edge_deviation_m <= report.merge_distance_m);
                        assert_eq!(
                            report.output_triangles,
                            out.top.len() + out.bottom.len() + out.sides.len()
                        );
                        let recorded = out
                            .metrics
                            .as_ref()
                            .expect("nonempty clipped material must have metrics");
                        assert_eq!(recorded.triangle_count, report.output_triangles);
                        assert!(recorded.component_count > 0);
                        let triangles: Vec<_> = out.triangles().collect();
                        inspect_closed_surface(&triangles).unwrap_or_else(|m|panic!("origin={origin},rotation={rotation},slope={slope},concave={concave},range={range:?}: {m}"));
                    }
                }
            }
        }
    }
}

#[test]
fn unified_report_identifies_invalid_header_element_and_host_before_source_math() {
    use atelier_bim::{
        geometry_validation::GeometryReport,
        model::{Material, OpeningKind, Shape},
    };
    let mut project = Project::default();
    project.schema_version = 999;
    let r = project.validation_report();
    assert!(!r.input_checked);
    assert_eq!(r.inspected_elements, 0);
    assert_eq!(r.first_error().unwrap().code, "INVALID_PROJECT_INPUT");
    let mut project = Project::default();
    let id = project.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: Point::new(f64::NAN, 0.),
            end: Point::new(4., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    let before = format!("{project:?}");
    let r = project.validation_report();
    assert_eq!(format!("{project:?}"), before);
    assert!(!r.input_checked);
    assert_eq!(r.inspected_elements, 0);
    let d = r.first_error().unwrap();
    assert_eq!(d.element_id, id);
    assert_eq!(d.feature, "parameters");
    assert_eq!(d.code, "INVALID_ELEMENT_INPUT");
    let bytes = serde_json::to_vec(&r).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["diagnostics"][0]["severity"], "error");
    assert_eq!(serde_json::from_slice::<GeometryReport>(&bytes).unwrap(), r);
    let mut project = Project::default();
    let id = project.add(
        0,
        Material::Concrete,
        Shape::Opening {
            kind: OpeningKind::Door,
            host_wall: 999,
            offset: 1.,
            width: 0.9,
            height: 2.1,
            sill: 0.,
        },
    );
    let e = Editor::new(project);
    let r = e.geometry_report();
    assert!(!r.input_checked);
    assert_eq!(r.document_session, Some(e.document_session()));
    assert_eq!(r.input_revision, Some(e.revision));
    assert_eq!(r.first_error().unwrap().element_id, id);
    assert_eq!(r.first_error().unwrap().feature, "host");
}

#[test]
fn shared_command_failures_return_the_original_proposal_diagnostic_after_rollback() {
    let mut e = Editor::new(Project::default());
    e.place_wall_segments(vec![(Point::default(), Point::new(4., 0.))])
        .unwrap();
    e.undo();
    let before = format!(
        "{:?}/{:?}/{:?}/{:?}/{:?}",
        e.project,
        e.revision,
        e.undo,
        e.redo,
        e.selection_ids()
    );
    for batch in [false, true] {
        let mut bad = impossible_arc(&e);
        bad.id += 1;
        let good = e
            .wall_creation_candidates(vec![(Point::default(), Point::new(4., 0.))])
            .unwrap()
            .remove(0);
        let commands = if batch {
            vec![ModelCommand::AddElements {
                elements: vec![good, bad.clone()],
                group: false,
            }]
        } else {
            vec![
                ModelCommand::AddElement { element: good },
                ModelCommand::AddElement {
                    element: bad.clone(),
                },
            ]
        };
        let failure = e.prepare_change(e.revision, commands).err().unwrap();
        assert_eq!(failure.code, "VALIDATION_FAILED");
        assert_eq!(failure.diagnostics.len(), 1);
        let d = &failure.diagnostics[0];
        assert_eq!(d.element_id, bad.id);
        assert_eq!(d.code, "INVALID_POLYLINE_GEOMETRY");
        assert_eq!(d.feature, "curve");
        let value = serde_json::to_value(&failure).unwrap();
        assert_eq!(value["diagnostics"][0]["element_id"], bad.id);
        assert_eq!(
            before,
            format!(
                "{:?}/{:?}/{:?}/{:?}/{:?}",
                e.project,
                e.revision,
                e.undo,
                e.redo,
                e.selection_ids()
            )
        );
    }
    let mut bad = e
        .wall_creation_candidates(vec![(Point::default(), Point::new(4., 0.))])
        .unwrap()
        .remove(0);
    if let atelier_bim::model::Shape::Wall { height, .. } = &mut bad.shape {
        *height = f64::NAN;
    }
    let failure = e
        .prepare_change(
            e.revision,
            vec![ModelCommand::AddElements {
                elements: vec![bad.clone()],
                group: false,
            }],
        )
        .err()
        .unwrap();
    assert_eq!(failure.diagnostics[0].element_id, bad.id);
    assert_eq!(failure.diagnostics[0].code, "INVALID_ELEMENT_INPUT");
    assert!(failure.message.starts_with("Invalid geometry:"));
    assert_eq!(
        before,
        format!(
            "{:?}/{:?}/{:?}/{:?}/{:?}",
            e.project,
            e.revision,
            e.undo,
            e.redo,
            e.selection_ids()
        )
    );
    e.redo();
    assert_eq!(e.project.elements.len(), 1);
}

#[test]
fn small_marquee_uses_local_area_at_both_coordinate_extremes() {
    use atelier_bim::editor::{MarqueeArea, MarqueeScope};
    for origin in [0., 99900., -99900.] {
        let s = 0.00225;
        let points = vec![
            Point::new(origin, origin),
            Point::new(origin + s, origin),
            Point::new(origin + s, origin + s),
            Point::new(origin, origin + s),
        ];
        assert!(
            MarqueeArea::new(points, MarqueeScope::CurrentStory, 0).is_some(),
            "origin {origin}"
        );
    }
}

#[test]
fn concave_clip_of_window_wall_matches_equivalent_rectangle_and_independent_volume() {
    use atelier_bim::model::{Material, OpeningKind, Shape};
    for joined in [false, true] {
        for window in [false, true] {
            let mut project = Project::default();
            let q = Point::new;
            let wall = project.add(
                0,
                Material::Concrete,
                Shape::Wall {
                    start: q(0., 0.),
                    end: q(4., 0.),
                    thickness: 0.3,
                    height: 3.,
                },
            );
            if joined {
                project.add(
                    0,
                    Material::Concrete,
                    Shape::Wall {
                        start: q(4., 0.),
                        end: q(4., 3.),
                        thickness: 0.3,
                        height: 3.,
                    },
                );
            }
            if window {
                project.add(
                    0,
                    Material::Concrete,
                    Shape::Opening {
                        host_wall: wall,
                        kind: OpeningKind::Window,
                        offset: 3.,
                        width: 1.,
                        sill: 1.,
                        height: 1.,
                    },
                );
            }
            project.validate().unwrap();
            let g = atelier_bim::wall_solid::derive(&project, &project.elements[0]).unwrap();
            for concave in [false, true] {
                let clip = if concave {
                    vec![
                        q(0.5, -1.),
                        q(3.5, -1.),
                        q(3.5, 3.),
                        q(2., 3.),
                        q(2., 1.),
                        q(0.5, 1.),
                    ]
                } else {
                    vec![q(0.5, -1.), q(3.5, -1.), q(3.5, 1.), q(0.5, 1.)]
                };
                for range in [
                    None,
                    Some((0., 1.)),
                    Some((1., 2.)),
                    Some((2., 3.)),
                    Some((0., 2.)),
                    Some((1., 3.)),
                ] {
                    let mesh = g
                        .geometry
                        .clipped(Some(&clip), range)
                        .unwrap_or_else(|message| {
                            panic!("joined={joined} window={window} concave={concave}: {message}")
                        });
                    let metrics = inspect_closed_surface(&mesh.triangles).unwrap();
                    let (lo, hi) = range.unwrap_or((0., 3.));
                    let expected = 3. * 0.3 * (hi - lo)
                        - if window {
                            0.3 * (hi.min(2.) - lo.max(1.)).max(0.)
                        } else {
                            0.
                        };
                    assert!(
                        (metrics.signed_volume_m3 - expected).abs() < 1e-8,
                        "joined={joined} window={window} concave={concave} volume={}",
                        metrics.signed_volume_m3
                    );
                }
            }
        }
    }
}
