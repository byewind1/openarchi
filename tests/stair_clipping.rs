use atelier_bim::{
    model::Point,
    stairs::{self, StairSpec},
};
use glam::DVec3;
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
fn volume(t: &[[DVec3; 3]]) -> f64 {
    t.iter().map(|[a, b, c]| a.dot(b.cross(*c)) / 6.).sum()
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 2e-4, "{a} != {b}");
}
fn body_volume(s: &StairSpec) -> f64 {
    let run = s.start.distance(s.end);
    let drop = s.slab_thickness * run.hypot(s.total_rise) / run;
    f64::from(s.width * run * (drop + s.total_rise / (2. * f64::from(s.risers))))
}
#[test]
fn unclipped_and_cover_preserve_body_and_only_real_edges() {
    let s = StairSpec::default();
    let g = stairs::generate(&s, 0.).unwrap();
    let (m, rails) = stairs::clipped_geometry(&s, 0., None, None).unwrap();
    assert_eq!(m.triangles, g.triangles);
    assert_eq!(m.edges, g.edges);
    assert_eq!(rails, g.handrails);
    let cover = [p(-1., -1.), p(6., -1.), p(6., 3.), p(-1., 3.)];
    let (m, rails) = stairs::clipped_geometry(&s, 0., Some(&cover), None).unwrap();
    near(volume(&m.triangles), body_volume(&s));
    assert_eq!(rails.len(), 2);
    for [a, b] in m.edges {
        assert!(
            g.edges.iter().any(|[u, v]| {
                let d = *v - *u;
                let on = |q: DVec3| {
                    (q - *u).cross(d).length() < 1e-5
                        && (q - *u).dot(d) >= -1e-5
                        && (q - *u).dot(d) <= d.length_squared() + 1e-5
                };
                on(a) && on(b)
            }),
            "internal edge {a:?} {b:?}"
        );
    }
}
#[test]
fn width_cut_is_closed_and_does_not_fill_stair_voids() {
    let s = StairSpec::default();
    let clip = [p(-1., 0.2), p(6., 0.2), p(6., 0.7), p(-1., 0.7)];
    let (m, rails) = stairs::clipped_geometry(&s, 3., Some(&clip), None).unwrap();
    near(
        volume(&m.triangles),
        body_volume(&s) * 0.5 / f64::from(s.width),
    );
    assert!(rails.is_empty());
    let run = s.start.distance(s.end);
    let drop = s.slab_thickness * run.hypot(s.total_rise) / run;
    for t in &m.triangles {
        for q in t {
            assert!(q.y >= 0.2 - 1e-5 && q.y <= 0.7 + 1e-5);
        }
        let q = (t[0] + t[1] + t[2]) / 3.;
        let upper = ((q.x / run * f64::from(s.risers) + 1e-5).floor() + 1.)
            .min(f64::from(s.risers))
            * s.total_rise
            / f64::from(s.risers)
            + 3.;
        assert!(
            q.z <= upper + 1e-5 && q.z >= 3. + q.x / run * s.total_rise - drop - 1e-5,
            "void filled {q:?}"
        );
    }
    // Width cut cap edges cannot expose side-profile triangulation diagonals.
    for [a, b] in m.edges {
        if (a.y - b.y).abs() < 1e-5 {
            let d = b - a;
            let is_step = d.x.abs() < 1e-5 || d.z.abs() < 1e-5;
            let is_soffit = (d.z * run - d.x * s.total_rise).abs() < 1e-4;
            assert!(is_step || is_soffit, "partition seam {a:?} {b:?}");
        }
    }
}
#[test]
fn z_limits_reverse_empty_and_rails_are_consistent() {
    let s = StairSpec::default();
    let (a, ra) = stairs::clipped_geometry(&s, 0., None, Some((1., 2.))).unwrap();
    let (b, rb) = stairs::clipped_geometry(&s, 0., None, Some((2., 1.))).unwrap();
    assert_eq!(a.triangles, b.triangles);
    assert_eq!(ra, rb);
    assert!(!ra.is_empty());
    for p in a.triangles.iter().flatten().chain(ra.iter().flatten()) {
        assert!(p.z >= 1. - 1e-5 && p.z <= 2. + 1e-5);
    }
    let (empty, rails) = stairs::clipped_geometry(&s, 0., None, Some((20., 21.))).unwrap();
    assert!(empty.triangles.is_empty() && empty.edges.is_empty() && rails.is_empty());
    assert!(stairs::clipped_geometry(&s, 0., None, Some((f64::NAN, 2.))).is_err());
}

#[test]
fn concave_region_volume_is_sum_of_disjoint_width_strips() {
    let s = StairSpec::default();
    let clip = [
        p(-1., 0.),
        p(6., 0.),
        p(6., 0.4),
        p(2.4, 0.4),
        p(2.4, 0.8),
        p(-1., 0.8),
    ];
    let (m, _) = stairs::clipped_geometry(&s, 0., Some(&clip), None).unwrap();
    let expected = body_volume(&s) * 0.6 / f64::from(s.width);
    near(volume(&m.triangles), expected);
    for q in m.triangles.iter().flatten() {
        assert!(q.y <= 0.4 + 1e-5 || q.x <= 2.4 + 1e-5);
    }
}
#[test]
fn rotated_translated_stair_retains_clipped_volume() {
    for angle in [0.3_f64, 1.5, 3.7] {
        let mut s = StairSpec::default();
        let run = s.start.distance(s.end);
        let point = |x: f64, y: f64| {
            p(
                10. + x * angle.cos() - y * angle.sin(),
                -6. + x * angle.sin() + y * angle.cos(),
            )
        };
        s.start = point(0., 0.);
        s.end = point(run, 0.);
        let clip = [
            point(-1., 0.2),
            point(6., 0.2),
            point(6., 0.7),
            point(-1., 0.7),
        ];
        let (m, _) = stairs::clipped_geometry(&s, 3., Some(&clip), None).unwrap();
        near(
            volume(&m.triangles),
            body_volume(&s) * 0.5 / f64::from(s.width),
        );
    }
}

#[test]
fn horizontal_caps_match_independent_integrated_section() {
    let s = StairSpec::default();
    let run = s.start.distance(s.end);
    let drop = s.slab_thickness * run.hypot(s.total_rise) / run;
    for (lo, hi) in [(0.4_f64, 1.9), (1.6, 2.0), (-0.1, 0.05)] {
        let (m, _) = stairs::clipped_geometry(&s, 0., None, Some((lo, hi))).unwrap();
        // Integrate each tread interval separately: clipped linear soffit under a flat tread.
        let mut expected = 0_f64;
        for i in 0..s.risers {
            let x0 = f64::from(run) * f64::from(i) / f64::from(s.risers);
            let x1 = f64::from(run) * f64::from(i + 1) / f64::from(s.risers);
            let top = f64::from(s.total_rise) * f64::from(i + 1) / f64::from(s.risers);
            for j in 0..1000 {
                let x = x0 + (x1 - x0) * (f64::from(j) + 0.5) / 1000.;
                let bottom = x / f64::from(run) * f64::from(s.total_rise) - f64::from(drop);
                expected += (top.min(f64::from(hi)) - bottom.max(f64::from(lo))).max(0.)
                    * (x1 - x0)
                    / 1000.
                    * f64::from(s.width);
            }
        }
        near(volume(&m.triangles), expected);
    }
}

#[test]
fn tangent_regions_do_not_leave_zero_volume_body_faces() {
    let s = StairSpec::default();
    for clip in [
        [
            p(-1., s.width),
            p(6., s.width),
            p(6., s.width + 1.),
            p(-1., s.width + 1.),
        ],
        [p(-1., -1.), p(0., -1.), p(0., 2.), p(-1., 2.)],
    ] {
        let (mesh, rails) = stairs::clipped_geometry(&s, 0., Some(&clip), None).unwrap();
        assert!(
            mesh.triangles.is_empty() && mesh.edges.is_empty(),
            "tangent contact produced {} triangles",
            mesh.triangles.len()
        );
        assert!(rails.is_empty());
    }
    let (mesh, _) =
        stairs::clipped_geometry(&s, 0., None, Some((s.total_rise, s.total_rise + 2.))).unwrap();
    assert!(mesh.triangles.is_empty() && mesh.edges.is_empty());
}

#[test]
fn concave_clip_can_split_handrails_without_bridging_gap() {
    let s = StairSpec::default();
    // Two fingers joined above the stair; each rail intersects both fingers.
    let clip = [
        p(0.5, -1.),
        p(1., -1.),
        p(1., 2.),
        p(3., 2.),
        p(3., -1.),
        p(4., -1.),
        p(4., 3.),
        p(0.5, 3.),
    ];
    let (_, rails) = stairs::clipped_geometry(&s, 0., Some(&clip), None).unwrap();
    assert_eq!(rails.len(), 4);
    for [a, b] in rails {
        assert!((a.x >= 0.5 - 1e-5 && b.x <= 1. + 1e-5) || (a.x >= 3. - 1e-5 && b.x <= 4. + 1e-5));
        let original = stairs::generate(&s, 0.).unwrap().handrails;
        assert!(
            original
                .iter()
                .any(|[u, v]| (a - *u).cross(*v - *u).length() < 1e-4
                    && (b - *u).cross(*v - *u).length() < 1e-4)
        );
    }
}
#[test]
fn thin_positive_volume_survives_boundary_contact_filter() {
    let s = StairSpec::default();
    let clip = [
        p(-1., s.width - 0.001),
        p(6., s.width - 0.001),
        p(6., s.width),
        p(-1., s.width),
    ];
    let (m, _) = stairs::clipped_geometry(&s, 0., Some(&clip), None).unwrap();
    assert!(!m.triangles.is_empty());
    let expected = body_volume(&s) * 0.001 / f64::from(s.width);
    assert!((volume(&m.triangles) - expected).abs() < 1e-6);
}
