use atelier_bim::{
    editor::Editor,
    model::{OpeningKind, Point, Project},
    opening_joinery::{self, DoorStyle, OpeningJoinery, PartRole, ResizeEdge},
};
fn fixture(kind: OpeningKind) -> (Editor, u64) {
    let mut e = Editor::new(Project::default());
    let wall = e
        .place_wall_segments(vec![(Point::new(0., 0.), Point::new(6., 0.))])
        .unwrap()[0];
    let id = e
        .place_opening_on_wall(kind, wall, 3., kind.defaults())
        .unwrap();
    (e, id)
}
#[test]
fn single_double_quarter_arcs_follow_host_basis_and_hinges() {
    for b in [
        Point::new(3., 0.),
        Point::new(-3., 0.),
        Point::new(0., 3.),
        Point::new(0., -3.),
        Point::new(2., 2.),
    ] {
        let a = Point::new(0., 0.);
        let len = a.distance(b);
        let u = Point::new(b.x / len, b.y / len);
        for style in [DoorStyle::Single, DoorStyle::Double] {
            for positive in [false, true] {
                for start in [false, true] {
                    let s = OpeningJoinery {
                        door_style: style,
                        opens_positive: positive,
                        hinge_at_start: start,
                        ..Default::default()
                    };
                    let g = opening_joinery::generate(a, b, 0., 2.1, OpeningKind::Door, s);
                    let arcs: Vec<_> = g.plan.iter().filter(|p| p.arc).collect();
                    let leaves = if style == DoorStyle::Double { 2 } else { 1 };
                    assert_eq!(arcs.len(), leaves);
                    for (i, path) in arcs.iter().enumerate() {
                        let hinge_start = if leaves == 2 { i == 0 } else { start };
                        let x = if hinge_start {
                            s.frame_width
                        } else {
                            len - s.frame_width
                        };
                        let y = if positive {
                            s.frame_depth / 2.
                        } else {
                            -s.frame_depth / 2.
                        };
                        let hinge = Point::new(u.x * x - u.y * y, u.y * x + u.x * y);
                        let radius = (len - 2. * s.frame_width) / leaves as f64;
                        for p in &path.points {
                            assert!((hinge.distance(*p) - radius).abs() < 1e-10);
                        }
                        let p = path.points[0];
                        let q = *path.points.last().unwrap();
                        assert!(
                            ((p.x - hinge.x) * (q.x - hinge.x) + (p.y - hinge.y) * (q.y - hinge.y))
                                .abs()
                                < 1e-10,
                            "must be a quarter circle, not a half circle"
                        );
                        assert_eq!(path.points.len(), 25);
                    }
                }
            }
        }
    }
    assert_eq!(OpeningJoinery::default().default_width(), 0.9);
    assert_eq!(
        OpeningJoinery {
            door_style: DoorStyle::Double,
            ..Default::default()
        }
        .default_width(),
        1.5
    );
}
#[test]
fn frames_leaves_pane_cache_identity_and_clip_share_one_opening() {
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        let (mut e, id) = fixture(kind);
        let original = e.project.clone();
        let before =
            atelier_bim::opening_geometry::derive(&e.project, &e.project.elements[1]).unwrap();
        assert_eq!(
            before
                .geometry
                .joinery
                .iter()
                .filter(|p| p.0 == PartRole::Frame)
                .count(),
            if kind == OpeningKind::Door { 3 } else { 4 }
        );
        for (_, p) in &before.geometry.joinery {
            atelier_bim::derived_geometry::inspect_closed_surface(&p.mesh.triangles).unwrap();
            let clipped = p.clipped(None, Some((0.5, 1.))).unwrap();
            assert!(
                clipped
                    .triangles
                    .iter()
                    .flatten()
                    .all(|v| v.z >= 0.5 - 1e-9 && v.z <= 1. + 1e-9)
            );
        }
        let mut elem = e.project.elements[1].clone();
        elem.opening_joinery = Some(OpeningJoinery {
            door_style: DoorStyle::Double,
            opens_positive: false,
            ..Default::default()
        });
        e.replace_element(elem).unwrap();
        let after =
            atelier_bim::opening_geometry::derive(&e.project, &e.project.elements[1]).unwrap();
        assert_ne!(
            before.identity.input_fingerprint,
            after.identity.input_fingerprint
        );
        assert_eq!(before.host_identity, after.host_identity);
        assert_eq!(
            before.geometry.sampled_cut_volume_m3,
            after.geometry.sampled_cut_volume_m3
        );
        if kind == OpeningKind::Door {
            assert_eq!(
                after
                    .geometry
                    .joinery
                    .iter()
                    .filter(|p| p.0 == PartRole::Leaf)
                    .count(),
                2
            );
            assert!(after.geometry.bounds.unwrap()[0].y < -0.1);
        } else {
            let pane = after.geometry.pane.as_ref().unwrap();
            assert!(
                pane.mesh
                    .triangles
                    .iter()
                    .flatten()
                    .all(|p| p.x > 2.4 && p.x < 3.6 && p.z >= 0.95 - 1e-9)
            );
        }
        let bytes = serde_json::to_vec(&e.project).unwrap();
        assert_eq!(
            serde_json::from_slice::<Project>(&bytes).unwrap(),
            e.project
        );
        e.undo();
        assert_eq!(e.project, original);
        assert!(e.project.elements.iter().any(|v| v.id == id));
    }
}
#[test]
fn resizing_keeps_opposite_edge_checks_host_and_is_one_transaction() {
    let (mut e, _) = fixture(OpeningKind::Window);
    let before = e.project.clone();
    let item = e.project.elements[1].clone();
    let resized = opening_joinery::resized(&e.project, &item, ResizeEdge::Right, 0.4).unwrap();
    let old = e.undo.len();
    e.replace_element(resized).unwrap();
    assert_eq!(e.undo.len(), old + 1);
    if let atelier_bim::model::Shape::Opening { width, offset, .. } = e.project.elements[1].shape {
        assert!((width - 1.6).abs() < 1e-9);
        assert!((offset - 3.2).abs() < 1e-9);
    }
    let after = e.project.clone();
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project, after);
    let original = e.project.clone();
    assert!(opening_joinery::resized(&e.project, &item, ResizeEdge::Right, 20.).is_err());
    assert_eq!(e.project, original);
    let bottom = opening_joinery::resized(&before, &item, ResizeEdge::Bottom, 0.2).unwrap();
    if let atelier_bim::model::Shape::Opening { sill, height, .. } = bottom.shape {
        assert!((sill - 1.1).abs() < 1e-9);
        assert!((height - 1.2).abs() < 1e-9);
    }
    let mut bad = item.clone();
    bad.opening_joinery = Some(OpeningJoinery {
        frame_depth: f64::NAN,
        ..Default::default()
    });
    assert!(e.replace_element(bad).is_err());
    assert_eq!(e.project, original);
}

#[test]
fn selectable_open_leaf_and_display_bounds_are_not_wall_material() {
    let (e, id) = fixture(OpeningKind::Door);
    let g = atelier_bim::opening_geometry::derive(&e.project, &e.project.elements[1]).unwrap();
    let leaf = &g
        .geometry
        .symbol
        .iter()
        .find(|p| p.closed && p.points.iter().any(|v| v.y > 0.3))
        .unwrap()
        .points;
    let q = Point::new(
        leaf.iter().map(|p| p.x).sum::<f64>() / 4.,
        leaf.iter().map(|p| p.y).sum::<f64>() / 4.,
    );
    assert_eq!(
        atelier_bim::geometry::hit_plan(&e.project, 0, q, 0.01),
        Some(id)
    );
    assert!(g.geometry.bounds.unwrap()[1].y > 0.7);
    assert!(
        (g.geometry.sampled_cut_volume_m3 - 0.9 * 2.1 * e.project.elements[0].thickness()).abs()
            < 1e-8
    );
}

#[test]
fn explicit_joinery_invert_and_mirror_keep_physical_hinge_semantics() {
    use atelier_bim::editing::EditCommand;
    use std::collections::BTreeSet;
    let (mut e, _) = fixture(OpeningKind::Door);
    let mut host = e.project.elements[0].clone();
    host.wall_settings
        .as_mut()
        .unwrap()
        .geometry
        .reference_offset = 0.17;
    e.replace_element(host).unwrap();
    let mut door = e.project.elements[1].clone();
    door.opening_joinery = Some(Default::default());
    e.replace_element(door).unwrap();
    let mesh = |p: &Project| {
        let g = atelier_bim::opening_geometry::derive(p, &p.elements[1]).unwrap();
        g.geometry
            .joinery
            .iter()
            .flat_map(|(_, x)| x.mesh.triangles.iter().flatten())
            .map(|q| {
                [
                    (q.x * 1e8).round() as i64,
                    (q.y * 1e8).round() as i64,
                    (q.z * 1e8).round() as i64,
                ]
            })
            .collect::<BTreeSet<_>>()
    };
    let original = mesh(&e.project);
    let span =
        atelier_bim::geometry::opening_frame_span(&e.project, &e.project.elements[1]).unwrap();
    assert!((span.0.y - 0.17).abs() < 1e-9);
    let wall = e.project.elements[0].id;
    e.set_selection(vec![wall]);
    e.edit(EditCommand::InvertWall).unwrap();
    assert_eq!(mesh(&e.project), original);
    e.set_selection(vec![wall]);
    e.edit(EditCommand::Transform {
        origin: Point::new(0., 0.),
        angle: 0.,
        scale: 1.,
        mirror: true,
        offset: Point::new(0., 0.),
        copies: 0,
    })
    .unwrap();
    assert_eq!(
        mesh(&e.project),
        original.into_iter().map(|[x, y, z]| [-x, y, z]).collect()
    );
}
