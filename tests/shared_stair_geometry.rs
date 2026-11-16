use atelier_bim::{
    derived_geometry::inspect_closed_surface,
    model::Point,
    stairs::{self, StairSpec},
};
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
fn expected(s: &StairSpec) -> f64 {
    let run = s.start.distance(s.end);
    s.width
        * run
        * (s.slab_thickness * run.hypot(s.total_rise) / run
            + s.total_rise / (2. * f64::from(s.risers)))
}
#[test]
fn stair_source_has_exact_oriented_closed_body_and_independent_analytic_volume() {
    for origin in [0., 99900., -99900.] {
        for angle in [0_f64, 0.37, 2.3] {
            let spec = StairSpec {
                start: p(origin, origin),
                end: p(origin + 4.8 * angle.cos(), origin + 4.8 * angle.sin()),
                ..Default::default()
            };
            let g = stairs::generate(&spec, 3.).unwrap();
            let m = inspect_closed_surface(&g.triangles).unwrap();
            assert!(
                (m.signed_volume_m3 - expected(&spec)).abs() < 1e-8,
                "{:?}",
                m
            );
        }
    }
}
#[test]
fn stair_clip_is_globally_closed_after_convex_and_concave_partitions() {
    let spec = StairSpec::default();
    let clips = [
        vec![p(-1., 0.2), p(6., 0.2), p(6., 0.7), p(-1., 0.7)],
        vec![
            p(-1., 0.),
            p(6., 0.),
            p(6., 0.4),
            p(2.4, 0.4),
            p(2.4, 0.8),
            p(-1., 0.8),
        ],
    ];
    for clip in clips {
        for range in [None, Some((0.4, 1.9)), Some((1.6, 2.))] {
            let (m, _) = stairs::clipped_geometry(&spec, 0., Some(&clip), range).unwrap();
            inspect_closed_surface(&m.triangles).unwrap();
        }
    }
}
#[test]
fn stair_minimum_spec_remains_closed_at_all_supported_riser_counts() {
    for risers in 2..=100 {
        let s = StairSpec {
            end: p(0.3, 0.),
            width: 0.5,
            total_rise: 0.1,
            slab_thickness: 0.05,
            handrail_height: 0.5,
            risers,
            ..Default::default()
        };
        let g = stairs::generate(&s, 0.).unwrap();
        let m = inspect_closed_surface(&g.triangles).unwrap();
        assert!((m.signed_volume_m3 - expected(&s)).abs() < 1e-10);
    }
}

#[test]
fn stair_query_preserves_parameters_history_and_file_binds_real_context() {
    use atelier_bim::{
        editor::Editor,
        model::{ModelAdapter, Project, Shape},
        storage::JsonAdapter,
    };
    let mut e = Editor::new(Project::default());
    let id = e.place_stair(StairSpec::default()).unwrap();
    let before = format!(
        "{:?}/{:?}/{:?}/{:?}",
        e.project,
        e.undo,
        e.redo,
        e.selection_ids()
    );
    let g = e.stair_geometry(id).unwrap();
    assert_eq!(g.identity.input_revision, Some(e.revision));
    assert_eq!(g.identity.document_session, Some(e.document_session()));
    assert_eq!(g.geometry.parameters(), &StairSpec::default());
    assert_eq!(
        g.geometry.source,
        stairs::generate(&StairSpec::default(), 0.).unwrap()
    );
    assert!(g.geometry.display_bounds[1].z > g.geometry.body_bounds[1].z);
    let rail_only = g.clipped(None, Some((3.5, 4.))).unwrap();
    assert!(rail_only.geometry.mesh.triangles.is_empty());
    assert_eq!(rail_only.geometry.body_bounds, None);
    assert!(!rail_only.geometry.handrails.is_empty());
    assert!(atelier_bim::geometry::element_overlaps_z_range(
        &e.project,
        &e.project.elements[0],
        (3.5, 4.)
    ));
    assert_eq!(
        before,
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
    assert_eq!(copy.stair_geometry(id).unwrap().geometry, g.geometry);
    assert_ne!(copy.document_session(), e.document_session());
    let mut changed = e.project.elements[0].clone();
    if let Shape::Stair { spec } = &mut changed.shape {
        spec.width = 1.4;
    }
    e.replace_element(changed).unwrap();
    assert_ne!(
        e.stair_geometry(id).unwrap().identity.input_fingerprint,
        g.identity.input_fingerprint
    );
    e.undo();
    assert_eq!(e.stair_geometry(id).unwrap().geometry, g.geometry);
    e.redo();
    assert!((e.stair_geometry(id).unwrap().geometry.plan_area_m2 - 4.8 * 1.4).abs() < 1e-12);
    let floor_before = e.stair_geometry(id).unwrap();
    let mut raised = e.project.elements[0].clone();
    raised.story = 1;
    let dz = e.project.elevation(1) - e.project.elevation(0);
    e.replace_element(raised).unwrap();
    let floor_after = e.stair_geometry(id).unwrap();
    assert_ne!(
        floor_before.identity.input_fingerprint,
        floor_after.identity.input_fingerprint
    );
    assert!(
        (floor_after.geometry.body_bounds[0].z - floor_before.geometry.body_bounds[0].z - dz).abs()
            < 1e-12
    );
    e.undo();
    assert_eq!(
        e.stair_geometry(id).unwrap().geometry,
        floor_before.geometry
    );
    assert_eq!(e.stair_geometry(999).unwrap_err().code, "ELEMENT_NOT_FOUND");
    assert_eq!(
        g.clipped(Some(&[]), None).unwrap_err().code,
        "STAIR_CLIP_FAILED"
    );
}

// Integrate independently from the mesh: flat tread ceiling minus linear soffit,
// split only at analytical intersections with the two horizontal clip planes.
fn integral(s: &StairSpec, xmin: f64, xmax: f64, zmin: f64, zmax: f64) -> f64 {
    let run = s.start.distance(s.end);
    let slope = s.total_rise / run;
    let drop = s.slab_thickness * run.hypot(s.total_rise) / run;
    let mut area = 0.;
    for i in 0..s.risers {
        let a = (run * f64::from(i) / f64::from(s.risers)).max(xmin);
        let b = (run * f64::from(i + 1) / f64::from(s.risers)).min(xmax);
        if b <= a {
            continue;
        }
        let top = (s.total_rise * f64::from(i + 1) / f64::from(s.risers)).min(zmax);
        let mut xs = vec![a, b];
        for z in [zmin, top] {
            let x = (z + drop) / slope;
            if x > a && x < b {
                xs.push(x);
            }
        }
        xs.sort_by(f64::total_cmp);
        for w in xs.windows(2) {
            let height = |x: f64| (top - (slope * x - drop).max(zmin)).max(0.);
            area += (w[1] - w[0]) * (height(w[0]) + height(w[1])) / 2.;
        }
    }
    area
}
#[test]
fn stair_partial_runs_and_z_caps_match_independent_exact_integral_at_far_origins() {
    for origin in [0., 99900., -99900.] {
        for angle in [0_f64, 0.37, 2.3] {
            let point = |x: f64, y: f64| {
                p(
                    origin + x * angle.cos() - y * angle.sin(),
                    origin + x * angle.sin() + y * angle.cos(),
                )
            };
            let s = StairSpec {
                start: point(0., 0.),
                end: point(4.8, 0.),
                ..Default::default()
            };
            let g = atelier_bim::stair_geometry::snapshot(&s, 0.).unwrap();
            for (xmin, xmax, lo, hi) in [
                (0.3, 3.7, 0.4, 1.9),
                (1.1, 4.4, 1.6, 2.),
                (0., 4.8, -0.1, 0.05),
            ] {
                let clip = [
                    point(xmin, 0.2),
                    point(xmax, 0.2),
                    point(xmax, 0.7),
                    point(xmin, 0.7),
                ];
                let out = g.clipped(Some(&clip), Some((lo, hi))).unwrap();
                let m = inspect_closed_surface(&out.mesh.triangles).unwrap();
                let expected = integral(&s, xmin, xmax, lo, hi) * 0.5;
                assert!(
                    (m.signed_volume_m3 - expected).abs() < 1e-8,
                    "origin {origin} angle {angle}: {} vs {expected}",
                    m.signed_volume_m3
                );
                assert!(out.normalization.max_displacement_m <= out.normalization.merge_distance_m);
            }
        }
    }
}
