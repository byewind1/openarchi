use atelier_bim::{
    command::ModelCommand,
    derived_geometry::inspect_closed_surface,
    editor::Editor,
    model::{ArcSegment, ModelAdapter, OpeningKind, Point, Project, Shape, WallContour},
    opening_geometry::{self, PaneRepresentation},
    storage::JsonAdapter,
    wall_solid,
};
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
fn fixture(kind: OpeningKind, curved: bool, origin: f64) -> (Editor, u64) {
    let mut e = Editor::new(Project::default());
    e.height = 3.;
    e.thickness = 0.3;
    let (a, b) = if curved {
        (p(origin + 4., origin), p(origin, origin + 4.))
    } else {
        (p(origin, origin), p(origin + 6., origin))
    };
    e.place_wall_segments(vec![(a, b)]).unwrap();
    if curved {
        let mut host = e.project.elements[0].clone();
        host.wall_contour = Some(WallContour::Arc {
            arc: ArcSegment {
                radius: 4.,
                clockwise: false,
                large_arc: false,
            },
        });
        e.replace_element(host).unwrap();
    }
    let length = atelier_bim::wall_geometry::reference_length(&e.project.elements[0]);
    let id = e
        .place_opening_on_wall(kind, 1, length / 2., kind.defaults())
        .unwrap();
    (e, id)
}
#[test]
fn opening_queries_are_revision_bound_void_proxies_and_glazing_is_display_only() {
    for origin in [0., 99900., -99900.] {
        for kind in [OpeningKind::Door, OpeningKind::Window] {
            let (e, id) = fixture(kind, false, origin);
            let state = format!(
                "{:?}/{:?}/{:?}/{:?}/{:?}",
                e.project,
                e.undo,
                e.redo,
                e.revision,
                e.selection_ids()
            );
            let g = e.opening_geometry(id).unwrap();
            let d = kind.defaults();
            let expected = d.width * d.height * 0.3;
            assert_eq!(g.geometry.host_wall, 1);
            assert_eq!(g.geometry.nominal_area_m2, d.width * d.height);
            assert!((g.geometry.sampled_cut_volume_m3 - expected).abs() < 1e-8);
            assert!(
                (inspect_closed_surface(&g.geometry.selectable_proxy.triangles)
                    .unwrap()
                    .signed_volume_m3
                    - expected)
                    .abs()
                    < 1e-8
            );
            assert_eq!(g.identity.input_revision, Some(e.revision));
            assert_eq!(g.host_identity.document_session, Some(e.document_session()));
            assert_eq!(
                g.host_identity.input_fingerprint,
                e.wall_geometry(1).unwrap().identity.input_fingerprint
            );
            if kind == OpeningKind::Door {
                assert!(g.geometry.pane.is_none());
            } else {
                let pane = g.geometry.pane.as_ref().unwrap();
                assert_eq!(pane.representation, PaneRepresentation::DisplayBox);
                let visual = inspect_closed_surface(&pane.mesh.triangles)
                    .unwrap()
                    .signed_volume_m3;
                assert!(
                    (visual - (d.width - 0.1) * 0.016 * (d.height - 0.1)).abs() < 1e-8,
                    "glazing must fit inside the 50mm frame, not occupy the old unframed void"
                );
                assert_eq!(g.geometry.joinery.len(), 4);
                let wall = e.wall_geometry(1).unwrap();
                assert!(
                    (wall.geometry.gross_volume_m3 - wall.geometry.material_volume_m3 - expected)
                        .abs()
                        < 1e-8
                );
            }
            assert_eq!(
                state,
                format!(
                    "{:?}/{:?}/{:?}/{:?}/{:?}",
                    e.project,
                    e.undo,
                    e.redo,
                    e.revision,
                    e.selection_ids()
                )
            );
            let reopened = Editor::new(
                JsonAdapter
                    .import(&JsonAdapter.export(&e.project).unwrap())
                    .unwrap(),
            );
            assert_eq!(g.geometry, reopened.opening_geometry(id).unwrap().geometry);
            assert_ne!(e.document_session(), reopened.document_session());
        }
    }
}
#[test]
fn curved_host_void_closed_proxy_and_open_glazing_use_the_same_local_sections() {
    for origin in [0., 99900., -99900.] {
        let (e, id) = fixture(OpeningKind::Window, true, origin);
        let g = e.opening_geometry(id).unwrap();
        inspect_closed_surface(&g.geometry.selectable_proxy.triangles).unwrap();
        let pane = g.geometry.pane.as_ref().unwrap();
        assert_eq!(pane.representation, PaneRepresentation::OpenDisplaySurface);
        assert!(!pane.mesh.triangles.is_empty());
        for &[a, b, c] in &pane.mesh.triangles {
            assert!((b - a).cross(c - a).length_squared() > 0.);
        }
        let proxy = g.geometry.clipped_proxy(None, Some((1., 2.))).unwrap();
        let metrics = inspect_closed_surface(&proxy.triangles).unwrap();
        assert!(metrics.bounds[0].z >= 1. && metrics.bounds[1].z <= 2.);
        let rendered = pane.clipped(None, Some((1., 2.))).unwrap();
        assert!(!rendered.triangles.is_empty());
        assert!(
            rendered
                .triangles
                .iter()
                .flatten()
                .all(|p| p.z >= 1. && p.z <= 2.)
        );
    }
}
#[test]
fn host_motion_deletion_and_undo_redo_keep_query_dependencies_and_child_records() {
    let (mut e, id) = fixture(OpeningKind::Door, false, 0.);
    let before = e.opening_geometry(id).unwrap();
    let child = e
        .project
        .elements
        .iter()
        .find(|e| e.id == id)
        .unwrap()
        .clone();
    e.apply(ModelCommand::MoveElements {
        ids: vec![1],
        offset: p(2., -1.),
    })
    .unwrap();
    let moved = e.opening_geometry(id).unwrap();
    assert_eq!(
        e.project.elements.iter().find(|e| e.id == id).unwrap(),
        &child
    );
    assert_ne!(
        before.identity.input_fingerprint,
        moved.identity.input_fingerprint
    );
    assert!(
        (moved.geometry.bounds.unwrap()[0].x - before.geometry.bounds.unwrap()[0].x - 2.).abs()
            < 1e-12
    );
    e.undo();
    assert_eq!(e.opening_geometry(id).unwrap().geometry, before.geometry);
    e.redo();
    e.apply(ModelCommand::RemoveElements { ids: vec![1] })
        .unwrap();
    assert_eq!(
        e.opening_geometry(id).unwrap_err().code,
        "ELEMENT_NOT_FOUND"
    );
    e.undo();
    assert_eq!(e.opening_geometry(id).unwrap().geometry, moved.geometry);
    e.redo();
    assert!(e.project.elements.is_empty());
}
#[test]
fn stale_inputs_missing_hosts_and_invalid_clip_are_explicit_errors() {
    let (e, id) = fixture(OpeningKind::Window, false, 0.);
    let mut stale = e
        .project
        .elements
        .iter()
        .find(|e| e.id == id)
        .unwrap()
        .clone();
    if let Shape::Opening { width, .. } = &mut stale.shape {
        *width = 0.6;
    }
    assert_eq!(
        opening_geometry::derive(&e.project, &stale)
            .unwrap_err()
            .code,
        "INVALID_OPENING_GEOMETRY"
    );
    if let Shape::Opening { host_wall, .. } = &mut stale.shape {
        *host_wall = 999;
    }
    assert_eq!(
        opening_geometry::derive(&e.project, &stale)
            .unwrap_err()
            .code,
        "OPENING_HOST_MISSING"
    );
    let g = e.opening_geometry(id).unwrap();
    assert!(g.geometry.clipped_proxy(Some(&[]), None).is_err());
    assert!(
        g.geometry
            .pane
            .unwrap()
            .clipped(None, Some((f64::NAN, 2.)))
            .is_err()
    );
}
#[test]
fn shared_batch_preview_and_commit_have_identical_joined_host_and_void_geometry() {
    let mut e = Editor::new(Project::default());
    e.height = 3.;
    e.thickness = 0.3;
    let mut pending = e
        .wall_creation_candidates(vec![(p(0., 0.), p(4., 0.)), (p(4., 0.), p(4., 3.))])
        .unwrap();
    let door = e
        .opening_in_batch_candidate(
            &pending,
            OpeningKind::Door,
            pending[0].id,
            2.,
            OpeningKind::Door.defaults(),
        )
        .unwrap();
    let door_id = door.id;
    pending.push(door);
    let prepared = e
        .prepare_change(
            e.revision,
            vec![ModelCommand::AddElements {
                elements: pending,
                group: false,
            }],
        )
        .unwrap();
    let preview = prepared.project();
    let host_preview = wall_solid::derive(preview, &preview.elements[0]).unwrap();
    let opening_preview = opening_geometry::derive(
        preview,
        preview.elements.iter().find(|e| e.id == door_id).unwrap(),
    )
    .unwrap();
    assert!(e.project.elements.is_empty());
    assert!(e.undo.is_empty());
    e.commit_change("joined-void-geometry", &prepared).unwrap();
    assert_eq!(e.wall_geometry(1).unwrap().geometry, host_preview.geometry);
    assert_eq!(
        e.opening_geometry(door_id).unwrap().geometry,
        opening_preview.geometry
    );
    assert_eq!(
        e.opening_geometry(door_id)
            .unwrap()
            .identity
            .input_fingerprint,
        opening_preview.identity.input_fingerprint
    );
    assert_eq!(e.undo.len(), 1);
    e.undo();
    assert!(e.project.elements.is_empty());
    e.redo();
    assert_eq!(
        e.opening_geometry(door_id).unwrap().geometry,
        opening_preview.geometry
    );
}
