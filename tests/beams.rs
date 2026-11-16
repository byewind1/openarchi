use atelier_bim::{
    command::ModelCommand,
    editor::{Editor, Tool},
    geometry,
    model::{Point, Project, Shape},
    storage,
};
#[test]
fn diagonal_beam_footprint_story_elevation_edit_history_and_file_roundtrip() {
    let mut e = Editor::new(Project::default());
    e.story = 1;
    e.beam_size = [0.6, 0.5, 3.];
    let id = e
        .place_beam(Point::new(0., 0.), Point::new(3., 4.))
        .unwrap();
    let created = e.project.clone();
    let beam = &created.elements[0];
    assert_eq!(beam.kind(), "Beam");
    assert!((beam.area() - 3.).abs() < 1e-5);
    let corners = geometry::corners(&created, beam);
    assert!((corners[0].x + 0.24).abs() < 1e-5);
    assert!((corners[0].y - 0.18).abs() < 1e-5);
    assert_eq!(corners[0].z, created.elevation(1) + 2.5);
    assert_eq!(corners[4].z, created.elevation(1) + 3.);
    assert_eq!(
        geometry::hit_plan(&created, 1, Point::new(1.5, 2.), 0.01),
        Some(id)
    );
    let mut update = beam.clone();
    if let Shape::Beam {
        height, top_offset, ..
    } = &mut update.shape
    {
        *height = 0.8;
        *top_offset = 3.5;
    }
    e.replace_element(update).unwrap();
    e.apply(ModelCommand::MoveElements {
        ids: vec![id],
        offset: Point::new(1., -1.),
    })
    .unwrap();
    let moved = e.project.clone();
    assert!(matches!(
        moved.elements[0].shape,
        Shape::Beam {
            start: Point { x: 1., y: -1. },
            end: Point { x: 4., y: 3. },
            height: 0.8,
            top_offset: 3.5,
            ..
        }
    ));
    e.undo();
    e.undo();
    assert_eq!(e.project, created);
    e.redo();
    e.redo();
    assert_eq!(e.project, moved);
    let path = std::env::temp_dir().join(format!("openarchi-beam-{}.json", std::process::id()));
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
fn beam_invalid_geometry_and_locked_layer_preserve_redo_and_revision() {
    let mut e = Editor::new(Project::default());
    e.place_beam(Point::default(), Point::new(2., 0.)).unwrap();
    e.undo();
    let before = e.project.clone();
    let revision = e.revision;
    let redo = e.redo.len();
    for (end, size) in [
        (Point::default(), [0.3, 0.5, 3.]),
        (Point::new(2., 0.), [0., 0.5, 3.]),
        (Point::new(2., 0.), [0.3, f64::NAN, 3.]),
        (Point::new(2., 0.), [0.3, 0.5, f64::INFINITY]),
    ] {
        e.beam_size = size;
        assert!(e.place_beam(Point::default(), end).is_err());
        assert_eq!(e.project, before);
        assert_eq!(e.revision, revision);
        assert_eq!(e.redo.len(), redo);
    }
    e.beam_size = [0.3, 0.5, 3.];
    let layer = e
        .project
        .layers
        .layers
        .iter_mut()
        .find(|l| l.id != 0)
        .unwrap();
    layer.status.locked = true;
    e.beam_layer = atelier_bim::layers::ElementLayer(layer.id);
    let locked = e.project.clone();
    assert!(e.place_beam(Point::default(), Point::new(2., 0.)).is_err());
    assert_eq!(e.project, locked);
    assert_eq!(e.redo.len(), redo);
}
#[test]
fn two_click_beam_draft_invalid_retry_and_tool_switch_are_atomic() {
    let mut e = Editor::new(Project::default());
    e.set_tool(Tool::Beam);
    e.click(Point::default(), 0.01);
    assert_eq!(e.anchor, Some(Point::default()));
    assert!(e.project.elements.is_empty());
    assert!(e.undo.is_empty());
    e.click(Point::default(), 0.01);
    assert_eq!(e.anchor, Some(Point::default()));
    assert!(e.project.elements.is_empty());
    assert_eq!(e.status, "Invalid beam dimensions or position");
    e.click(Point::new(2., 1.), 0.01);
    assert_eq!(e.project.elements.len(), 1);
    assert_eq!(e.undo.len(), 1);
    assert_eq!(e.anchor, None);
    e.click(Point::new(3., 1.), 0.01);
    e.set_tool(Tool::Column);
    assert_eq!(e.anchor, None);
    assert_eq!(e.project.elements.len(), 1);
    let id = e.project.elements[0].id;
    e.project.edit_state.locked.insert(id);
    let before = e.project.clone();
    let mut bad = before.elements[0].clone();
    if let Shape::Beam { height, .. } = &mut bad.shape {
        *height = 0.;
    }
    assert!(e.replace_element(bad).is_err());
    assert!(
        e.apply(ModelCommand::RemoveElements { ids: vec![id] })
            .is_err()
    );
    assert_eq!(e.project, before);
}

#[test]
fn slanted_beam_creation_persistence_translation_history_and_rejection() {
    let mut e = Editor::new(Project::default());
    e.story = 1;
    e.beam_size = [0.3, 0.4, 2.];
    let id = e
        .place_slanted_beam(Point::default(), Point::new(4., 0.), 30.)
        .unwrap();
    let original = e.project.clone();
    let corners = geometry::corners(&original, &original.elements[0]);
    assert!((corners[5].z - corners[4].z - 4. * 30_f64.to_radians().tan()).abs() < 1e-5);
    let json = serde_json::to_value(&original).unwrap();
    assert_eq!(json["elements"][0]["shape"]["slant_angle"], 30.);
    let path = std::env::temp_dir().join(format!("openarchi-slant-{}.json", std::process::id()));
    storage::save(&original, &path).unwrap();
    assert_eq!(storage::load(&path).unwrap(), original);
    std::fs::remove_file(path).unwrap();
    e.apply(ModelCommand::MoveElements {
        ids: vec![id],
        offset: Point::new(7., -2.),
    })
    .unwrap();
    let moved = e.project.clone();
    let after = geometry::corners(&moved, &moved.elements[0]);
    for (a, b) in corners.into_iter().zip(after) {
        assert!((b - a - glam::DVec3::new(7., -2., 0.)).length() < 1e-5);
    }
    e.undo();
    assert_eq!(e.project, original);
    let redo = e.redo[0].command.clone();
    for angle in [90., f64::NAN, f64::INFINITY] {
        assert!(
            e.place_slanted_beam(Point::default(), Point::new(4., 0.), angle)
                .is_err()
        );
        assert_eq!(e.project, original);
        assert_eq!(e.redo[0].command, redo);
    }
    e.redo();
    assert_eq!(e.project, moved);
    e.undo();
    e.undo();
    assert!(e.project.elements.is_empty());
    e.redo();
    assert_eq!(e.project, original);
    let mut legacy = json;
    legacy["elements"][0]["shape"]
        .as_object_mut()
        .unwrap()
        .remove("slant_angle");
    let old: Project = serde_json::from_value(legacy).unwrap();
    old.validate().unwrap();
    let saved = serde_json::to_value(&old).unwrap();
    assert!(saved["elements"][0]["shape"].get("slant_angle").is_none());
    let c = geometry::corners(&old, &old.elements[0]);
    assert_eq!(c[4].z, c[5].z);
}

#[test]
fn slanted_beam_camera_picks_solid_not_its_vertical_bounding_box() {
    let mut e = Editor::new(Project::default());
    e.beam_size = [1., std::f64::consts::FRAC_1_SQRT_2, 0.];
    let id = e
        .place_slanted_beam(Point::default(), Point::new(4., 0.), 45.)
        .unwrap();
    let camera = geometry::Camera {
        yaw: -std::f64::consts::FRAC_PI_2,
        pitch: 0.,
        span: 10.,
        target: glam::DVec3::new(2., 0., 1.5),
    };
    for (point, expected) in [
        (glam::DVec3::new(2., 0., 1.5), Some(id)),
        (glam::DVec3::new(0.5, 0., 3.5), None),
        (glam::DVec3::new(3.5, 0., 0.), None),
    ] {
        let ndc = camera.matrix(1.).project_point3(point);
        assert_eq!(
            camera.pick(&e.project, [ndc.x, ndc.y], 1., Some(0)),
            expected,
            "{point:?}"
        );
    }
}
