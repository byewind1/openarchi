use atelier_bim::{
    beam::BeamGeometry,
    beam_geometry,
    derived_geometry::{VolumeSource, inspect_closed_surface},
    editor::Editor,
    geometry,
    model::{ArcSegment, ModelAdapter, Point, Project, Shape},
    storage::JsonAdapter,
};
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
fn parameters(offset: f64, angle: f64) -> BeamGeometry {
    BeamGeometry {
        start: p(offset, offset),
        end: p(offset + 3., offset + 4.),
        start_top: 3.,
        width: 0.4,
        height: 0.5,
        slant_angle: angle,
        arc: None,
    }
}
fn close_volume(g: &beam_geometry::BeamSnapshot, expected: f64) {
    let m = inspect_closed_surface(&g.mesh.triangles).unwrap();
    assert!(
        (m.signed_volume_m3 - expected).abs() < 1e-8,
        "{} != {expected}",
        m.signed_volume_m3
    );
    assert_eq!(g.bounds, m.bounds);
    assert_eq!(g.volume_source, VolumeSource::AnalyticPrism);
}
#[test]
fn straight_slanted_and_reversed_beams_share_true_surfaces_bounds_and_analytic_quantities() {
    for offset in [0., 99900., -99900.] {
        for angle in [-30., 0., 30.] {
            for reverse in [false, true] {
                let mut s = parameters(offset, angle);
                if reverse {
                    std::mem::swap(&mut s.start, &mut s.end);
                }
                let g = beam_geometry::snapshot(s).unwrap();
                let expected = 1. / angle.to_radians().cos();
                close_volume(&g, expected);
                assert!((g.volume_m3 - expected).abs() < 1e-12);
                assert!((g.plan_area_m2 - 2.).abs() < 1e-10);
                assert_eq!(s.mesh(None, None).unwrap(), g.mesh);
                for c in g.solid.corners {
                    assert!(
                        g.mesh.triangles.iter().flatten().any(|&v| v == c),
                        "compatibility corners must be real source vertices"
                    );
                }
            }
        }
    }
}
#[test]
fn curved_beams_keep_parametric_arc_quantities_and_explicit_sampling_error_in_both_directions() {
    for offset in [0., 99900., -99900.] {
        for clockwise in [false, true] {
            for large_arc in [false, true] {
                let s = BeamGeometry {
                    start: p(offset + 5., offset),
                    end: p(offset, offset + 5.),
                    arc: Some(ArcSegment {
                        radius: 5.,
                        clockwise,
                        large_arc,
                    }),
                    slant_angle: 0.,
                    ..parameters(offset, 0.)
                };
                let g = beam_geometry::snapshot(s).unwrap();
                let length = 5. * std::f64::consts::PI * if large_arc { 1.5 } else { 0.5 };
                let expected = length * 0.4 * 0.5;
                assert!((g.volume_m3 - expected).abs() < 1e-8);
                assert!((g.plan_area_m2 - length * 0.4).abs() < 1e-8);
                let sampled = inspect_closed_surface(&g.mesh.triangles)
                    .unwrap()
                    .signed_volume_m3;
                assert!(sampled < expected && (sampled - expected).abs() / expected < 0.0001);
                assert_eq!(g.parameters().arc, s.arc);
                assert_eq!(g, beam_geometry::snapshot(s).unwrap());
            }
        }
    }
}
#[test]
fn minimum_beam_surfaces_and_boundary_clips_stay_valid_after_large_translation() {
    for offset in [0., 99900., -99900.] {
        for angle in [-80., 0., 80.] {
            let s = BeamGeometry {
                end: p(offset + 0.01, offset),
                width: 0.01,
                height: 0.01,
                ..parameters(offset, angle)
            };
            let g = beam_geometry::snapshot(s).unwrap();
            let expected = 1e-6 / angle.to_radians().cos();
            close_volume(&g, expected);
            let full = g
                .clipped(None, Some((g.bounds[0].z, g.bounds[1].z)))
                .unwrap();
            assert_eq!(g.mesh, full);
            assert!(
                g.clipped(None, Some((g.bounds[1].z, g.bounds[1].z + 1.)))
                    .unwrap()
                    .triangles
                    .is_empty()
            );
        }
    }
}
#[test]
fn minimum_curved_beam_plan_and_solid_share_sampling_with_independent_chord_error_bound() {
    for offset in [0., 99900., -99900.] {
        let s = BeamGeometry {
            start: p(offset + 0.02, offset),
            end: p(offset, offset + 0.02),
            arc: Some(ArcSegment {
                radius: 0.02,
                clockwise: false,
                large_arc: false,
            }),
            width: 0.01,
            height: 0.01,
            slant_angle: 0.,
            ..parameters(offset, 0.)
        };
        let g = beam_geometry::snapshot(s).unwrap();
        let plan: f64 = g
            .plan_triangles
            .iter()
            .map(|t| {
                ((t[1].x - t[0].x) * (t[2].y - t[0].y) - (t[1].y - t[0].y) * (t[2].x - t[0].x))
                    .abs()
                    / 2.
            })
            .sum();
        let analytic = 0.02 * 0.01 * std::f64::consts::FRAC_PI_2;
        let step = std::f64::consts::PI / 180.;
        // Ceil at an exact degree boundary may use 90 or 91 facets after
        // translation roundoff. Bound sampling error independently, without
        // changing the maximum one-degree sampling rule to force a count.
        let lower = analytic * step.sin() / step;
        assert!(
            plan >= lower - 1e-11 && plan <= analytic + 1e-11,
            "offset={offset}, area={plan}"
        );
        if offset == 0. {
            assert!((plan - 0.02 * 0.01 * 90. * step.sin()).abs() < 1e-11);
        }
        assert!((g.sampled_volume_m3 - plan * 0.01).abs() < 1e-12);
        assert!((g.plan_area_m2 - analytic).abs() < 1e-11);
    }
}
#[test]
fn beam_clips_are_closed_and_empty_sets_are_distinct_from_invalid_input() {
    for offset in [0., 99900., -99900.] {
        let s = BeamGeometry {
            end: p(offset + 4., offset),
            ..parameters(offset, 0.)
        };
        let g = beam_geometry::snapshot(s).unwrap();
        let clip = [
            p(offset + 1., offset - 0.1),
            p(offset + 3., offset - 0.1),
            p(offset + 3., offset + 0.1),
            p(offset + 1., offset + 0.1),
        ];
        let m = g.clipped(Some(&clip), Some((2.6, 2.9))).unwrap();
        assert!(
            (inspect_closed_surface(&m.triangles)
                .unwrap()
                .signed_volume_m3
                - 0.12)
                .abs()
                < 1e-8
        );
        assert!(
            g.clipped(None, Some((30., 31.)))
                .unwrap()
                .triangles
                .is_empty()
        );
        assert!(g.clipped(Some(&[]), None).is_err());
        assert!(g.clipped(None, Some((0., f64::NAN))).is_err());
    }
}
#[test]
fn editor_beam_query_and_candidate_share_revision_identity_and_exact_history_and_file_results() {
    let mut e = Editor::new(Project::default());
    let s = parameters(0., 30.);
    e.beam_size = [s.width, s.height, s.start_top];
    let before = e.project.clone();
    let preview = e
        .beam_path_preview(s.start, s.end, s.slant_angle, None)
        .unwrap();
    let candidate = beam_geometry::derive(&e.project, &preview).unwrap();
    assert_eq!(e.project, before);
    let id = e.place_slanted_beam(s.start, s.end, s.slant_angle).unwrap();
    let state = format!(
        "{:?}/{:?}/{:?}/{:?}/{:?}/{:?}",
        e.project,
        e.undo,
        e.redo,
        e.revision,
        e.selection_ids(),
        e.status
    );
    let g = e.beam_geometry(id).unwrap();
    assert_eq!(
        g.identity.input_fingerprint,
        candidate.identity.input_fingerprint
    );
    assert_eq!(g.identity.input_revision, Some(e.revision));
    assert_eq!(g.identity.document_session, Some(e.document_session()));
    assert_eq!(g, e.beam_geometry(id).unwrap());
    assert_eq!(
        state,
        format!(
            "{:?}/{:?}/{:?}/{:?}/{:?}/{:?}",
            e.project,
            e.undo,
            e.redo,
            e.revision,
            e.selection_ids(),
            e.status
        )
    );
    assert_eq!(
        geometry::corners(&e.project, &e.project.elements[0]),
        g.geometry.solid.corners
    );
    assert_eq!(e.project.elements[0].area(), g.geometry.plan_area_m2);
    assert_eq!(e.beam_geometry(999).unwrap_err().code, "ELEMENT_NOT_FOUND");
    let mut edited = e.project.elements[0].clone();
    if let Shape::Beam { width, .. } = &mut edited.shape {
        *width = 0.8;
    }
    e.replace_element(edited).unwrap();
    assert_ne!(
        e.beam_geometry(id).unwrap().identity.input_fingerprint,
        g.identity.input_fingerprint
    );
    e.undo();
    assert_eq!(e.beam_geometry(id).unwrap().geometry, g.geometry);
    e.redo();
    let reopened = Editor::new(
        JsonAdapter
            .import(&JsonAdapter.export(&e.project).unwrap())
            .unwrap(),
    );
    assert_eq!(
        e.beam_geometry(id).unwrap().geometry,
        reopened.beam_geometry(id).unwrap().geometry
    );
    assert_ne!(e.document_session(), reopened.document_session());
    let mut next_story = e.project.elements[0].clone();
    next_story.story = 1;
    let previous = e.beam_geometry(id).unwrap();
    e.replace_element(next_story).unwrap();
    let shifted = e.beam_geometry(id).unwrap();
    assert_ne!(
        shifted.identity.input_fingerprint,
        previous.identity.input_fingerprint
    );
    assert!((shifted.geometry.bounds[0].z - previous.geometry.bounds[0].z - 3.2).abs() < 1e-12);
    e.undo();
    assert_eq!(e.beam_geometry(id).unwrap().geometry, previous.geometry);
    e.redo();
    let mut invalid = e.project.elements[0].clone();
    if let Shape::Beam { height, .. } = &mut invalid.shape {
        *height = 0.;
    }
    let d = beam_geometry::derive(&e.project, &invalid).unwrap_err();
    assert_eq!(d.element_id, id);
    assert_eq!(d.code, "INVALID_BEAM");
}
