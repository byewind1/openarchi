use atelier_bim::{
    command::ModelCommand,
    editor::{Editor, Tool},
    geometry,
    model::{Point, Project, Shape},
    roof::{self, RoofSpec},
    storage,
};
fn spec() -> RoofSpec {
    RoofSpec {
        contour: None,
        pivot_start: Point::default(),
        pivot_end: Point::new(4., 0.),
        rise_left: true,
        min: Point::default(),
        max: Point::new(4., 3.),
        pitch: 45.,
        thickness: 0.2,
        pivot_offset: 3.,
    }
}
fn volume(m: &roof::RoofMesh) -> f64 {
    m.triangles
        .iter()
        .map(|t| t[0].dot(t[1].cross(t[2])) / 6.)
        .sum::<f64>()
        .abs()
}
#[test]
fn roof_bottom_pivot_vertical_thickness_story_and_reversed_direction() {
    let mut e = Editor::new(Project::default());
    e.story = 1;
    let id = e.place_roof(spec()).unwrap();
    let c = geometry::corners(&e.project, &e.project.elements[0]);
    let base = e.project.elevation(1);
    for i in 0..4 {
        assert!((c[i].z - (base + 3. + c[i].y)).abs() < 1e-5);
        assert!((c[i + 4].z - c[i].z - 0.2).abs() < 1e-5);
    }
    assert_eq!(
        geometry::hit_plan(&e.project, 1, Point::new(2., 1.), 0.01),
        Some(id)
    );
    let mut s = spec();
    s.rise_left = false;
    assert!((s.bottom(Point::new(2., 2.), 0.) - 1.).abs() < 1e-5);
    s.pitch = -45.;
    assert!((s.bottom(Point::new(2., 2.), 0.) - 5.).abs() < 1e-5);
    s.pivot_start = Point::new(4., 0.);
    s.pivot_end = Point::default();
    s.rise_left = true;
    assert!((s.bottom(Point::new(2., 2.), 0.) - 5.).abs() < 1e-5);
}
#[test]
fn roof_mesh_retains_slope_when_clipped_in_xy_and_z_and_is_closed() {
    let s = spec();
    let all = roof::mesh(&s, 0., None, None).unwrap();
    assert!((volume(&all) - 2.4).abs() < 1e-4);
    let clip = [
        Point::new(1., 1.),
        Point::new(3., 1.),
        Point::new(3., 2.),
        Point::new(1., 2.),
    ];
    let xy = roof::mesh(&s, 0., Some(&clip), None).unwrap();
    assert!((volume(&xy) - 0.4).abs() < 1e-4);
    for v in xy.triangles.iter().flatten() {
        assert!((1. - 1e-5..=3. + 1e-5).contains(&v.x));
        assert!((1. - 1e-5..=2. + 1e-5).contains(&v.y));
        assert!((v.z - (3. + v.y)).abs() < 1e-4 || (v.z - (3.2 + v.y)).abs() < 1e-4);
    }
    let cut = roof::mesh(&s, 0., Some(&clip), Some((4.4, 4.6))).unwrap();
    assert!((volume(&cut) - 0.08).abs() < 2e-4);
    for v in cut.triangles.iter().flatten() {
        assert!((4.4 - 1e-5..=4.6 + 1e-5).contains(&v.z));
        assert!((v.y - 1.2) >= -1e-4 && v.y <= 1.6 + 1e-4);
    }
    let mut flat = s;
    flat.pitch = 0.;
    let m = roof::mesh(&flat, 0., None, Some((3., 3.2))).unwrap();
    assert!((volume(&m) - 2.4).abs() < 1e-4);
    assert!(
        roof::mesh(&flat, 0., None, Some((5., 6.)))
            .unwrap()
            .triangles
            .is_empty()
    );
}
#[test]
fn roof_property_translation_delete_history_and_file_roundtrip() {
    let mut e = Editor::new(Project::default());
    let id = e.place_roof(spec()).unwrap();
    let created = e.project.clone();
    let mut update = created.elements[0].clone();
    if let Shape::Roof { spec } = &mut update.shape {
        spec.pitch = 30.;
        spec.pivot_offset = 4.;
    }
    e.replace_element(update).unwrap();
    e.apply(ModelCommand::MoveElements {
        ids: vec![id],
        offset: Point::new(2., -1.),
    })
    .unwrap();
    let moved = e.project.clone();
    if let Shape::Roof { spec } = &moved.elements[0].shape {
        assert_eq!(spec.pivot_start, Point::new(2., -1.));
        assert_eq!(spec.min, Point::new(2., -1.));
        assert!((spec.bottom(Point::new(2., -1.), 0.) - 4.).abs() < 1e-5);
    } else {
        panic!()
    }
    e.undo();
    e.undo();
    assert_eq!(e.project, created);
    e.redo();
    e.redo();
    assert_eq!(e.project, moved);
    let path = std::env::temp_dir().join(format!("openarchi-roof-{}.json", std::process::id()));
    storage::save(&moved, &path).unwrap();
    assert_eq!(storage::load(&path).unwrap(), moved);
    std::fs::remove_file(path).unwrap();
    e.apply(ModelCommand::RemoveElements { ids: vec![id] })
        .unwrap();
    assert!(e.project.elements.is_empty());
    e.undo();
    assert_eq!(e.project, moved);
}
#[test]
fn roof_invalid_drafts_parameters_and_locks_preserve_project_and_history() {
    let mut e = Editor::new(Project::default());
    e.set_tool(Tool::Roof);
    let before = e.project.clone();
    for p in [Point::default(), Point::default()] {
        e.click(p, 0.01);
    }
    assert_eq!(e.status, "Roof pivot line is too short");
    assert_eq!(e.project, before);
    e.click(Point::new(4., 0.), 0.01);
    e.click(Point::new(2., 0.), 0.01);
    assert_eq!(e.status, "Choose a side away from the roof pivot line");
    for p in [Point::new(2., 1.), Point::default()] {
        e.click(p, 0.01);
    }
    assert!(e.undo.is_empty());
    e.click(Point::default(), 0.01);
    assert_eq!(e.project, before);
    assert!(e.roof_draft.is_some());
    e.click(Point::new(4., 3.), 0.01);
    assert_eq!(e.project.elements.len(), 1);
    assert_eq!(e.undo.len(), 1);
    assert!(e.roof_draft.is_none());
    let created = e.project.clone();
    let mut invalid = created.elements[0].clone();
    if let Shape::Roof { spec } = &mut invalid.shape {
        spec.pitch = f64::NAN;
    }
    assert!(e.replace_element(invalid).is_err());
    assert_eq!(e.project, created);
    e.undo();
    let before = e.project.clone();
    let revision = e.revision;
    let redo = e.redo.len();
    for value in [f64::NAN, f64::INFINITY, 90.] {
        let mut s = spec();
        s.pitch = value;
        assert!(e.place_roof(s).is_err());
        assert_eq!(e.project, before);
        assert_eq!(e.revision, revision);
        assert_eq!(e.redo.len(), redo);
    }
    let l = e
        .project
        .layers
        .layers
        .iter_mut()
        .find(|l| l.id != 0)
        .unwrap();
    l.status.locked = true;
    e.roof_layer = atelier_bim::layers::ElementLayer(l.id);
    let locked = e.project.clone();
    assert!(e.place_roof(spec()).is_err());
    assert_eq!(e.project, locked);
    assert_eq!(e.redo.len(), redo);
    e.click(Point::default(), 0.01);
    e.set_tool(Tool::Column);
    assert!(e.roof_draft.is_none());
    assert_eq!(e.anchor, None);
}

#[test]
fn roof_disconnected_xy_clips_keep_both_components_and_empty_history_cancels_draft() {
    let s = spec();
    let clip = [
        Point::new(0., 0.),
        Point::new(1., 0.),
        Point::new(1., 4.),
        Point::new(3., 4.),
        Point::new(3., 0.),
        Point::new(4., 0.),
        Point::new(4., 5.),
        Point::new(0., 5.),
    ];
    let m = roof::mesh(&s, 0., Some(&clip), None).unwrap();
    assert!((volume(&m) - 1.2).abs() < 1e-4);
    assert!(
        m.triangles
            .iter()
            .flatten()
            .all(|v| v.x <= 1.0001 || v.x >= 2.9999)
    );
    let mut e = Editor::new(Project::default());
    e.set_tool(Tool::Roof);
    e.click(Point::default(), 0.01);
    e.undo();
    assert!(e.roof_draft.is_none());
    assert!(e.anchor.is_none());
    assert!(e.project.elements.is_empty());
}
