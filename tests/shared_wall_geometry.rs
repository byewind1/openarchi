use atelier_bim::{
    derived_geometry::{VolumeSource, inspect_closed_surface},
    editor::Editor,
    model::{ArcSegment, ModelAdapter, OpeningKind, Point, Project, Shape, WallContour},
    storage::JsonAdapter,
    wall_solid,
};
use geo::Area;
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
fn straight(offset: f64) -> Editor {
    let mut e = Editor::new(Project::default());
    e.height = 3.;
    e.thickness = 0.3;
    e.place_wall_segments(vec![(p(offset, offset), p(offset + 6., offset))])
        .unwrap();
    e
}
fn openings(e: &mut Editor) {
    let mut d = OpeningKind::Door.defaults();
    d.width = 0.9;
    d.height = 2.1;
    d.sill = 0.;
    e.place_opening_on_wall(OpeningKind::Door, 1, 1.5, d)
        .unwrap();
    let mut w = OpeningKind::Window.defaults();
    w.width = 1.2;
    w.height = 1.4;
    w.sill = 0.9;
    e.place_opening_on_wall(OpeningKind::Window, 1, 4.5, w)
        .unwrap();
}
#[test]
fn shared_straight_wall_holes_have_closed_material_and_independent_analytic_volume() {
    for offset in [0., 99900., -99900.] {
        let mut e = straight(offset);
        openings(&mut e);
        let before = format!(
            "{:?}/{:?}/{:?}/{:?}/{:?}",
            e.project,
            e.undo,
            e.redo,
            e.revision,
            e.selection_ids()
        );
        let g = e.wall_geometry(1).unwrap();
        let expected = 6. * 0.3 * 3. - 0.9 * 2.1 * 0.3 - 1.2 * 1.4 * 0.3;
        assert!((g.geometry.material_volume_m3 - expected).abs() < 1e-8);
        assert!(
            (inspect_closed_surface(&g.geometry.mesh.triangles)
                .unwrap()
                .signed_volume_m3
                - expected)
                .abs()
                < 1e-8
        );
        assert_eq!(g.geometry.volume_source, VolumeSource::AnalyticPrism);
        assert_eq!(g.geometry.voids.len(), 2);
        assert_eq!(g.identity.input_revision, Some(e.revision));
        assert_eq!(g.identity.document_session, Some(e.document_session()));
        assert_eq!(
            before,
            format!(
                "{:?}/{:?}/{:?}/{:?}/{:?}",
                e.project,
                e.undo,
                e.redo,
                e.revision,
                e.selection_ids()
            )
        );
        assert_eq!(g, e.wall_geometry(1).unwrap());
        let reopened = Editor::new(
            JsonAdapter
                .import(&JsonAdapter.export(&e.project).unwrap())
                .unwrap(),
        );
        assert_eq!(g.geometry, reopened.wall_geometry(1).unwrap().geometry);
    }
}
#[test]
fn wall_material_clips_keep_voids_and_analytic_z_intersections() {
    let mut e = straight(0.);
    openings(&mut e);
    let g = e.wall_geometry(1).unwrap();
    let clip = [p(0., -2.), p(6., -2.), p(6., 2.), p(0., 2.)];
    let m = g.geometry.clipped(Some(&clip), Some((1., 2.))).unwrap();
    let expected = (6. - 0.9 - 1.2) * 0.3;
    assert!(
        (inspect_closed_surface(&m.triangles)
            .unwrap()
            .signed_volume_m3
            - expected)
            .abs()
            < 1e-8
    );
    assert!(
        g.geometry
            .clipped(None, Some((4., 5.)))
            .unwrap()
            .triangles
            .is_empty()
    );
    assert!(g.geometry.clipped(Some(&[]), None).is_err());
}
#[test]
fn curved_wall_and_window_use_one_station_grid_without_chord_slivers_in_void() {
    for offset in [0., 99900., -99900.] {
        let mut e = Editor::new(Project::default());
        e.height = 3.;
        e.thickness = 0.3;
        e.place_wall_segments(vec![(p(offset + 4., offset), p(offset, offset + 4.))])
            .unwrap();
        let mut host = e.project.elements[0].clone();
        host.wall_contour = Some(WallContour::Arc {
            arc: ArcSegment {
                radius: 4.,
                clockwise: false,
                large_arc: false,
            },
        });
        e.replace_element(host).unwrap();
        let length = 4. * std::f64::consts::FRAC_PI_2;
        let mut d = OpeningKind::Window.defaults();
        d.width = 1.;
        d.height = 1.;
        d.sill = 1.;
        e.place_opening_on_wall(OpeningKind::Window, 1, length / 2., d)
            .unwrap();
        let g = e.wall_geometry(1).unwrap();
        assert_eq!(g.geometry.volume_source, VolumeSource::SampledSurface);
        assert!(g.geometry.max_chord_error_m <= 0.0021);
        let analytic = 4. * std::f64::consts::FRAC_PI_2 * 0.3 * 3. - 0.3;
        assert!((g.geometry.material_volume_m3 - analytic).abs() < 0.005);
        for i in 1..20 {
            let s = length / 2. - 0.5 + i as f64 / 20.;
            let angle = s / 4.;
            for radius in [3.86, 4., 4.14] {
                let q =
                    geo::Point::new(offset + radius * angle.cos(), offset + radius * angle.sin());
                assert!(
                    !g.geometry.contains_at(q, 1.5),
                    "window must have no sampled wall sliver"
                );
            }
        }
        inspect_closed_surface(&g.geometry.mesh.triangles).unwrap();
        assert!(g.geometry.gross_footprint.unsigned_area() > 0.);
    }
}
#[test]
fn minimum_wall_and_fully_consuming_opening_have_explicit_material_states() {
    for offset in [0., 99900., -99900.] {
        let mut e = Editor::new(Project::default());
        e.height = 0.01;
        e.thickness = 0.01;
        e.place_wall_segments(vec![(p(offset, offset), p(offset + 0.01, offset))])
            .unwrap();
        let g = e.wall_geometry(1).unwrap();
        assert!((g.geometry.material_volume_m3 - 1e-6).abs() < 1e-12);
        inspect_closed_surface(&g.geometry.mesh.triangles).unwrap();
    }
    let mut e = straight(0.);
    let mut d = OpeningKind::Door.defaults();
    d.width = 6.;
    d.height = 3.;
    d.sill = 0.;
    e.place_opening_on_wall(OpeningKind::Door, 1, 3., d)
        .unwrap();
    let g = e.wall_geometry(1).unwrap();
    assert!(g.geometry.empty_by_openings);
    assert!(g.geometry.mesh.triangles.is_empty());
    assert_eq!(g.geometry.bounds, None);
    assert_eq!(g.geometry.material_volume_m3, 0.);
}
#[test]
fn join_context_is_physical_and_independent_of_color_and_layer_visibility() {
    let mut e = straight(0.);
    e.place_wall_segments(vec![(p(6., 0.), p(6., 3.)), (p(3., 0.), p(3., 3.))])
        .unwrap();
    let layer = e.project.layers.new_layer("Physical hidden peer", None);
    let mut wall = e.project.elements[1].clone();
    wall.layer = atelier_bim::layers::ElementLayer(layer);
    e.replace_element(wall).unwrap();
    let before = e.wall_geometry(1).unwrap();
    for wall in &e.project.elements {
        inspect_closed_surface(
            &wall_solid::derive(&e.project, wall)
                .unwrap()
                .geometry
                .mesh
                .triangles,
        )
        .unwrap();
    }
    let mut project = e.project.clone();
    project.layers.layer_mut(layer).unwrap().status.visible = false;
    project.elements[1].material = atelier_bim::model::Material::Timber;
    let after = wall_solid::derive(&project, &project.elements[0]).unwrap();
    assert_eq!(
        before.identity.input_fingerprint,
        after.identity.input_fingerprint
    );
    assert_eq!(before.geometry, after.geometry);
    let mut wall = e.project.elements[0].clone();
    if let Shape::Wall { height, .. } = &mut wall.shape {
        *height = 4.;
    }
    e.replace_element(wall).unwrap();
    assert_ne!(
        e.wall_geometry(1).unwrap().identity.input_fingerprint,
        before.identity.input_fingerprint
    );
    e.undo();
    assert_eq!(e.wall_geometry(1).unwrap().geometry, before.geometry);
    e.redo();
}

#[test]
fn compatibility_bounds_cover_joined_material_but_full_void_is_not_a_physical_z_overlap() {
    use atelier_bim::geometry;
    let mut e = Editor::new(Project::default());
    e.height = 3.;
    e.thickness = 0.4;
    e.place_wall_segments(vec![(p(0., 0.), p(4., 0.)), (p(4., 0.), p(4., 3.))])
        .unwrap();
    let g = e.wall_geometry(1).unwrap();
    let c = geometry::corners(&e.project, &e.project.elements[0]);
    let [lo, hi] = g.geometry.bounds.unwrap();
    for v in g.geometry.mesh.triangles.iter().flatten() {
        assert!(v.cmpge(lo).all() && v.cmple(hi).all());
    }
    assert!((c[2].x - hi.x).abs() < 1e-12);
    assert!(hi.x > 4.1);
    let mut empty = straight(0.);
    let mut d = OpeningKind::Door.defaults();
    d.width = 6.;
    d.height = 3.;
    d.sill = 0.;
    empty
        .place_opening_on_wall(OpeningKind::Door, 1, 3., d)
        .unwrap();
    assert_eq!(empty.wall_geometry(1).unwrap().geometry.bounds, None);
    assert!(!geometry::element_overlaps_z_range(
        &empty.project,
        &empty.project.elements[0],
        (0., 3.)
    ));
    assert!(geometry::element_overlaps_z_range(
        &empty.project,
        &empty.project.elements[1],
        (0., 3.)
    ));
    assert_eq!(
        wall_solid::vertical_range(&empty.project, &empty.project.elements[0]),
        Some((0., 3.))
    );
}
