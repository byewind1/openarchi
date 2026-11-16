use atelier_bim::{
    derived_geometry::{VolumeSource, inspect_closed_surface},
    editor::Editor,
    geometry,
    model::{ModelAdapter, Point, Project, Shape},
    roof::{self, RoofSpec},
    roof_contour::RoofContour,
    roof_geometry,
    storage::JsonAdapter,
};
use glam::DVec3;
use std::collections::BTreeMap;
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
fn rect(x: f64, y: f64, w: f64, h: f64) -> Vec<Point> {
    vec![p(x, y), p(x + w, y), p(x + w, y + h), p(x, y + h)]
}
fn spec() -> RoofSpec {
    RoofSpec {
        contour: None,
        pivot_start: p(0., 0.),
        pivot_end: p(8., 0.),
        rise_left: true,
        min: p(0., 0.),
        max: p(8., 6.),
        pitch: 30.,
        thickness: 0.3,
        pivot_offset: 2.,
    }
}
fn complex() -> RoofSpec {
    let mut s = spec();
    s.contour = Some(RoofContour {
        outer: vec![
            p(0., 0.),
            p(8., 0.),
            p(8., 3.),
            p(4., 3.),
            p(4., 6.),
            p(0., 6.),
        ],
        holes: vec![rect(1., 1., 1., 1.), rect(1., 3., 1., 2.)],
    });
    s
}
// Independent exact edge incidence and locally-originated divergence integral.
fn verify(mesh: &roof::RoofMesh, expected: f64, tol: f64) {
    let origin = mesh.triangles[0][0];
    let mut v = 0.;
    let mut edges = BTreeMap::new();
    for &[a, b, c] in &mesh.triangles {
        assert!((b - a).cross(c - a).length_squared() > 0.);
        v += (a - origin).dot((b - origin).cross(c - origin)) / 6.;
        for (a, b) in [(a, b), (b, c), (c, a)] {
            let key = |p: DVec3| {
                p.to_array().map(|x| {
                    if x == 0. {
                        0.0_f64.to_bits()
                    } else {
                        x.to_bits()
                    }
                })
            };
            let (a, b) = (key(a), key(b));
            let (k, d) = if a < b { ((a, b), 1) } else { ((b, a), -1) };
            let x = edges.entry(k).or_insert((0, 0));
            x.0 += 1;
            x.1 += d;
        }
    }
    assert!(
        edges.values().all(|&(n, d)| n == 2 && d == 0),
        "surface must be closed and consistently oriented"
    );
    assert!(
        (v - expected).abs() < tol,
        "actual {v}, expected {expected}"
    );
}

#[test]
fn shared_roof_preserves_concavity_holes_winding_slope_story_and_origin() {
    for offset in [0., 99900., -99900.] {
        for reverse in [false, true] {
            for pitch in [-30., 0., 30.] {
                let mut s = complex();
                s.pitch = pitch;
                if reverse {
                    let c = s.contour.as_mut().unwrap();
                    c.outer.reverse();
                    for h in &mut c.holes {
                        h.reverse();
                    }
                }
                s.translate(p(offset, offset));
                let before = s.clone();
                let g = roof_geometry::geometry(&s, 3.2).unwrap();
                assert_eq!(s, before);
                assert_eq!(g.plan_area_m2, 33.);
                assert!((g.slope_area_m2 - 33. / pitch.to_radians().cos()).abs() < 1e-10);
                assert_eq!(g.volume_source, VolumeSource::AnalyticPrism);
                verify(&g.mesh, 9.9, 1e-8);
                assert!((g.volume_m3 - g.sampled_volume_m3).abs() < 1e-8);
                for t in &g.plan_triangles {
                    let q = p(
                        (t[0].x + t[1].x + t[2].x) / 3.,
                        (t[0].y + t[1].y + t[2].y) / 3.,
                    );
                    assert!(s.footprint_contour().contains(q));
                }
                assert_eq!(roof::mesh(&s, 3.2, None, None).unwrap(), g.mesh);
                assert_eq!(g.bounds[0].x, offset);
                assert_eq!(g.bounds[1].x, offset + 8.);
            }
        }
    }
}

#[test]
fn roof_clips_have_closed_analytic_volumes_and_explicit_empty_or_error_results() {
    let mut s = spec();
    s.pitch = 45.;
    s.pivot_offset = 3.;
    s.thickness = 0.2;
    for offset in [0., 99900., -99900.] {
        let mut s = s.clone();
        s.translate(p(offset, offset));
        let g = roof_geometry::geometry(&s, 0.).unwrap();
        let clip = rect(offset + 1., offset + 1., 2., 1.);
        verify(&g.clipped(Some(&clip), None).unwrap(), 0.4, 1e-8);
        verify(
            &g.clipped(Some(&clip), Some((4.4, 4.6))).unwrap(),
            0.08,
            1e-8,
        );
        assert!(
            g.clipped(Some(&rect(offset + 20., offset, 1., 1.)), None)
                .unwrap()
                .triangles
                .is_empty()
        );
        assert!(
            g.clipped(None, Some((100., 101.)))
                .unwrap()
                .triangles
                .is_empty()
        );
        assert!(g.clipped(Some(&[]), None).is_err());
        assert!(g.clipped(None, Some((f64::NAN, 1.))).is_err());
    }
}

#[test]
fn roof_collinear_boundary_nodes_are_shared_by_caps_and_sides() {
    let mut s = spec();
    s.contour = Some(RoofContour {
        outer: vec![p(0., 0.), p(4., 0.), p(8., 0.), p(8., 6.), p(0., 6.)],
        holes: vec![],
    });
    let g = roof_geometry::geometry(&s, 0.).unwrap();
    verify(&g.mesh, 14.4, 1e-10);
    assert!(
        g.mesh
            .triangles
            .iter()
            .flatten()
            .any(|v| v.x == 4. && v.y == 0.)
    );
    let c = g
        .clipped(Some(&rect(1., 0., 6., 5.)), Some((3., 4.)))
        .unwrap();
    inspect_closed_surface(&c.triangles).unwrap();
}

#[test]
fn fixed_seed_oblique_roof_slices_match_independent_double_integral() {
    // Integral of positive-part affine height over an axis-aligned rectangle.
    // The four differences of cubic antiderivatives are independent of meshing.
    fn hinge(a: f64, b: f64, c: f64) -> f64 {
        let f = |x: f64, y: f64| (a * x + b * y + c).max(0.).powi(3) / (6. * a * b);
        f(7., 5.) - f(1., 5.) - f(7., 1.) + f(1., 1.)
    }
    let mut seed = 0x29_2026_u64;
    for offset in [0., 99900., -99900.] {
        for _ in 0..16 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let angle = 0.2 + (seed >> 32) as f64 / u32::MAX as f64 * 0.9;
            let mut s = spec();
            s.pitch = 20.;
            s.pivot_offset = 2.;
            s.pivot_end = p(8. * angle.cos(), 8. * angle.sin());
            let a = -angle.sin() * 20_f64.to_radians().tan();
            let b = angle.cos() * 20_f64.to_radians().tan();
            let (lo, hi) = (1.5, 2.7);
            let expected =
                hinge(a, b, 2. + 0.3 - lo) - hinge(a, b, 2. - lo) - hinge(a, b, 2. + 0.3 - hi)
                    + hinge(a, b, 2. - hi);
            s.translate(p(offset, offset));
            let g = roof_geometry::geometry(&s, 0.).unwrap();
            let m = g
                .clipped(
                    Some(&rect(offset + 1., offset + 1., 6., 4.)),
                    Some((lo, hi)),
                )
                .unwrap_or_else(|e| panic!("seed {seed} offset {offset}: {e}"));
            verify(&m, expected, 1e-8);
        }
    }
}

#[test]
fn roof_minimum_solid_and_coincident_cut_planes_remain_closed_at_large_offsets() {
    for offset in [0., 99900., -99900.] {
        for pitch in [0., 80., -80.] {
            let mut s = spec();
            s.max = p(0.01, 0.01);
            s.pivot_end = p(0.01, 0.);
            s.thickness = 0.01;
            s.pitch = pitch;
            s.translate(p(offset, offset));
            let g = roof_geometry::geometry(&s, 0.).unwrap();
            verify(&g.mesh, 1e-6, 1e-12);
            let full = g
                .clipped(None, Some((g.bounds[0].z, g.bounds[1].z)))
                .unwrap_or_else(|e| panic!("offset {offset} pitch {pitch} full: {e}"));
            verify(&full, 1e-6, 1e-12);
            assert!(
                g.clipped(None, Some((g.bounds[1].z, g.bounds[1].z + 1.)))
                    .unwrap_or_else(|e| panic!("offset {offset} pitch {pitch} tangent: {e}"))
                    .triangles
                    .is_empty()
            );
        }
    }
}

#[test]
fn complementary_cuts_at_every_source_corner_conserve_volume_without_sliver_faces() {
    for offset in [0., 99900., -99900.] {
        for pitch in [-80., -30., 0., 30., 80.] {
            let mut s = complex();
            s.pitch = pitch;
            s.pivot_end = p(6., 4.);
            s.translate(p(offset, offset));
            let g = roof_geometry::geometry(&s, 0.).unwrap();
            for z in g.mesh.triangles.iter().flatten().map(|p| p.z) {
                let low = g
                    .clipped(None, Some((g.bounds[0].z - 1., z)))
                    .unwrap_or_else(|e| panic!("low: offset {offset}, pitch {pitch}, z {z}: {e}"));
                let high = g
                    .clipped(None, Some((z, g.bounds[1].z + 1.)))
                    .unwrap_or_else(|e| panic!("high: offset {offset}, pitch {pitch}, z {z}: {e}"));
                let volume = |m: roof::RoofMesh| {
                    if m.triangles.is_empty() {
                        0.
                    } else {
                        inspect_closed_surface(&m.triangles)
                            .unwrap()
                            .signed_volume_m3
                    }
                };
                assert!((volume(low) + volume(high) - g.volume_m3).abs() < 1e-8);
            }
        }
    }
}

#[test]
fn editor_roof_query_binds_real_identity_without_modifying_state_and_survives_history_and_file() {
    let mut e = Editor::new(Project::default());
    let s = complex();
    let preview = e.roof_creation_candidate(s.clone()).unwrap();
    let before = e.project.clone();
    let fingerprint = roof_geometry::derive(&preview, 0.)
        .unwrap()
        .identity
        .input_fingerprint;
    assert_eq!(e.project, before);
    let id = e.place_roof(s).unwrap();
    let created = e.project.clone();
    let undo = format!("{:?}", e.undo);
    let status = e.status.clone();
    let g = e.roof_geometry(id).unwrap();
    assert_eq!(g.identity.input_fingerprint, fingerprint);
    assert_eq!(g.identity.input_revision, Some(e.revision));
    assert_eq!(g.identity.document_session, Some(e.document_session()));
    assert_eq!(e.roof_geometry(id).unwrap(), g);
    assert_eq!(e.project, created);
    assert_eq!(format!("{:?}", e.undo), undo);
    assert_eq!(e.status, status);
    assert_eq!(e.roof_geometry(999).unwrap_err().code, "ELEMENT_NOT_FOUND");
    assert_eq!(
        geometry::corners(&e.project, &e.project.elements[0]),
        g.geometry.compatibility_corners()
    );
    let original = e.project.elements[0].clone();
    let mut changed = original.clone();
    if let Shape::Roof { spec } = &mut changed.shape {
        spec.pitch = -25.;
    }
    e.replace_element(changed).unwrap();
    assert_ne!(
        e.roof_geometry(id).unwrap().identity.input_fingerprint,
        fingerprint
    );
    e.undo();
    assert_eq!(
        e.roof_geometry(id).unwrap().identity.input_fingerprint,
        fingerprint
    );
    e.redo();
    let loaded = JsonAdapter
        .import(&JsonAdapter.export(&e.project).unwrap())
        .unwrap();
    let reopened = Editor::new(loaded);
    assert_ne!(e.document_session(), reopened.document_session());
    assert_eq!(
        e.roof_geometry(id).unwrap().geometry,
        reopened.roof_geometry(id).unwrap().geometry
    );
    let mut invalid = original;
    if let Shape::Roof { spec } = &mut invalid.shape {
        spec.pitch = f64::NAN;
    }
    let d = roof_geometry::derive(&invalid, 0.).unwrap_err();
    assert_eq!(d.element_id, id);
    assert_eq!(d.code, "INVALID_ROOF");
}

#[test]
fn compatibility_mesh_accessors_distinguish_invalid_inputs_from_a_valid_empty_intersection() {
    let s = atelier_bim::roof::RoofSpec {
        contour: None,
        pivot_start: Point::new(0., 0.),
        pivot_end: Point::new(4., 0.),
        rise_left: true,
        min: Point::new(0., 0.),
        max: Point::new(4., 3.),
        pitch: 20.,
        thickness: 0.2,
        pivot_offset: 3.,
    };
    assert!(
        roof::mesh(&s, 0., None, Some((100., 101.)))
            .unwrap()
            .triangles
            .is_empty()
    );
    assert!(roof::mesh(&s, 0., None, Some((f64::NAN, 101.))).is_err());
    let mut bad = s.clone();
    bad.thickness = 0.;
    assert!(roof::mesh(&bad, 0., None, None).is_err());
    let invalid = atelier_bim::roof_contour::RoofContour {
        outer: vec![],
        holes: vec![],
    };
    assert!(roof::mesh_contour(&s, &invalid, 0., None, None).is_err());
}
