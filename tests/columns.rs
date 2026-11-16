use atelier_bim::{
    command::ModelCommand,
    editor::Editor,
    geometry,
    model::{Point, Project, Shape},
    storage,
};
#[test]
fn rectangular_column_geometry_edit_move_history_and_persistence() {
    let mut editor = Editor::new(Project::default());
    editor.story = 1;
    editor.column_section = atelier_bim::column::ColumnSection::Rectangular {
        width: 0.6,
        depth: 0.4,
    };
    editor.column_height = 3.5;
    let id = editor.place_column(Point::new(2., 3.)).unwrap();
    let created = editor.project.clone();
    let element = &created.elements[0];
    assert_eq!(element.kind(), "Column");
    let bounds = geometry::corners(&created, element);
    assert!((bounds[0].x - 1.7).abs() < 1e-5);
    assert!((bounds[0].y - 2.8).abs() < 1e-5);
    assert_eq!(bounds[0].z, created.elevation(1));
    assert_eq!(bounds[4].z, bounds[0].z + 3.5);
    assert_eq!(
        geometry::hit_plan(&created, 1, Point::new(2., 3.), 0.01),
        Some(id)
    );
    let mut updated = element.clone();
    if let Shape::Column {
        section: atelier_bim::column::ColumnSection::Rectangular { width, .. },
        height,
        ..
    } = &mut updated.shape
    {
        *width = 0.8;
        *height = 4.;
    }
    editor.replace_element(updated).unwrap();
    editor
        .apply(ModelCommand::MoveElements {
            ids: vec![id],
            offset: Point::new(1., -1.),
        })
        .unwrap();
    let moved = editor.project.clone();
    assert!(matches!(
        moved.elements[0].shape,
        Shape::Column {
            center: Point { x: 3., y: 2. },
            section: atelier_bim::column::ColumnSection::Rectangular { width: 0.8, .. },
            height: 4.,
            ..
        }
    ));
    editor.undo();
    editor.undo();
    assert_eq!(editor.project, created);
    editor.redo();
    editor.redo();
    assert_eq!(editor.project, moved);
    let path = std::env::temp_dir().join(format!("openarchi-column-{}.json", std::process::id()));
    storage::save(&moved, &path).unwrap();
    assert_eq!(storage::load(&path).unwrap(), moved);
    std::fs::remove_file(path).unwrap();
}
#[test]
fn invalid_and_locked_column_placement_preserve_history() {
    let mut editor = Editor::new(Project::default());
    editor.place_column(Point::default()).unwrap();
    editor.undo();
    let before = editor.project.clone();
    let revision = editor.revision;
    let redo = editor.redo.len();
    for bad in [0., -1., f64::NAN, f64::INFINITY] {
        editor.column_section = atelier_bim::column::ColumnSection::Rectangular {
            width: bad,
            depth: 0.4,
        };
        editor.column_height = 3.;
        assert!(editor.place_column(Point::default()).is_err());
        assert_eq!(editor.project, before);
        assert_eq!(editor.revision, revision);
        assert_eq!(editor.redo.len(), redo);
    }
    editor.column_section = atelier_bim::column::ColumnSection::Rectangular {
        width: 0.4,
        depth: 0.4,
    };
    editor.column_height = 3.;
    let layer = editor
        .project
        .layers
        .layers
        .iter_mut()
        .find(|l| l.id != 0)
        .unwrap();
    layer.status.locked = true;
    editor.column_layer = atelier_bim::layers::ElementLayer(layer.id);
    let before = editor.project.clone();
    assert!(editor.place_column(Point::default()).is_err());
    assert_eq!(editor.project, before);
    assert_eq!(editor.redo.len(), redo);
}

#[test]
fn column_invalid_property_and_locked_update_delete_are_atomic() {
    let mut editor = Editor::new(Project::default());
    let id = editor.place_column(Point::default()).unwrap();
    let before = editor.project.clone();
    let revision = editor.revision;
    let mut invalid = before.elements[0].clone();
    if let Shape::Column { height, .. } = &mut invalid.shape {
        *height = 0.;
    }
    assert!(editor.replace_element(invalid).is_err());
    assert_eq!(editor.project, before);
    assert_eq!(editor.revision, revision);
    editor.project.edit_state.locked.insert(id);
    let locked = editor.project.clone();
    let mut update = locked.elements[0].clone();
    update.name = "changed".into();
    assert!(editor.replace_element(update).is_err());
    assert!(
        editor
            .apply(ModelCommand::RemoveElements { ids: vec![id] })
            .is_err()
    );
    assert_eq!(editor.project, locked);
    editor.project.edit_state.locked.clear();
    let before_delete = editor.project.clone();
    editor
        .apply(ModelCommand::RemoveElements { ids: vec![id] })
        .unwrap();
    assert!(editor.project.elements.is_empty());
    editor.undo();
    assert_eq!(editor.project, before_delete);
}

#[test]
fn column_bottom_offset_geometry_properties_history_and_legacy_json() {
    for offset in [-1.25, 0., 2.5] {
        let mut editor = Editor::new(Project::default());
        editor.story = 1;
        editor.column_bottom_offset = offset;
        let id = editor.place_column(Point::new(2., 3.)).unwrap();
        let created = editor.project.clone();
        let corners = geometry::corners(&created, &created.elements[0]);
        for i in 0..4 {
            assert_eq!(corners[i].z, created.elevation(1) + offset);
            assert_eq!(corners[i + 4].z, corners[i].z + 3.);
        }
        assert!(geometry::element_overlaps_z_range(
            &created,
            &created.elements[0],
            (corners[0].z + 1., corners[0].z + 2.)
        ));
        assert!(!geometry::element_overlaps_z_range(
            &created,
            &created.elements[0],
            (corners[0].z - 2., corners[0].z - 1.)
        ));
        let serialized = serde_json::to_value(&created).unwrap();
        assert_eq!(
            serde_json::from_value::<Project>(serialized.clone()).unwrap(),
            created
        );
        let mut legacy = serialized;
        legacy["elements"][0]["shape"]
            .as_object_mut()
            .unwrap()
            .remove("bottom_offset");
        let legacy: Project = serde_json::from_value(legacy).unwrap();
        assert_eq!(
            geometry::corners(&legacy, &legacy.elements[0])[0].z,
            legacy.elevation(1)
        );
        let mut changed = created.elements[0].clone();
        if let Shape::Column { bottom_offset, .. } = &mut changed.shape {
            *bottom_offset = offset + 1.;
        }
        editor.replace_element(changed).unwrap();
        let edited = editor.project.clone();
        editor
            .apply(ModelCommand::MoveElements {
                ids: vec![id],
                offset: Point::new(1., 2.),
            })
            .unwrap();
        assert_eq!(
            geometry::corners(&editor.project, &editor.project.elements[0])[0].z,
            corners[0].z + 1.
        );
        editor.undo();
        assert_eq!(editor.project, edited);
        editor.undo();
        assert_eq!(editor.project, created);
        editor.undo();
        assert!(editor.project.elements.is_empty());
        editor.redo();
        assert_eq!(editor.project, created);
        editor.redo();
        assert_eq!(editor.project, edited);
    }
}

#[test]
fn column_invalid_bottom_offset_preserves_nonempty_redo() {
    let mut editor = Editor::new(Project::default());
    editor.place_column(Point::default()).unwrap();
    let created = editor.project.clone();
    editor.undo();
    let before = editor.project.clone();
    let revision = editor.revision;
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        editor.column_bottom_offset = value;
        assert!(editor.place_column(Point::default()).is_err());
        assert_eq!(editor.project, before);
        assert_eq!(editor.revision, revision);
        assert_eq!(editor.redo.len(), 1);
    }
    editor.redo();
    assert_eq!(editor.project, created);
    let mut invalid = editor.project.elements[0].clone();
    if let Shape::Column { bottom_offset, .. } = &mut invalid.shape {
        *bottom_offset = f64::NAN;
    }
    assert!(editor.replace_element(invalid).is_err());
    assert_eq!(editor.project, created);
}

#[test]
fn rotated_column_matches_ac29_samples_and_preserves_height_history_json() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/geometry/column-rotation-runtime--both-details.json"
    ))
    .unwrap();
    for sample in capture["structuredContent"]["detailsOfElements"]
        .as_array()
        .unwrap()
    {
        let details = &sample["details"];
        let mut editor = Editor::new(Project::default());
        editor.column_section = atelier_bim::column::ColumnSection::Rectangular {
            width: 0.8,
            depth: 0.4,
        };
        editor.column_height = 2.8;
        editor.column_bottom_offset = 1.25;
        editor.column_axis_rotation =
            (details["axisRotationAngle"].as_f64().unwrap().to_degrees()) as f64;
        let center = Point::new(
            details["origin"]["x"].as_f64().unwrap() as f64,
            details["origin"]["y"].as_f64().unwrap() as f64,
        );
        let id = editor.place_column(center).unwrap();
        let created = editor.project.clone();
        let corners = geometry::corners(&created, &created.elements[0]);
        for (i, expected) in sample["floorPlanPolygons"][0]["coordinates"]
            .as_array()
            .unwrap()[..4]
            .iter()
            .enumerate()
        {
            assert!((corners[i].x as f64 - expected["x"].as_f64().unwrap()).abs() < 1e-5);
            assert!((corners[i].y as f64 - expected["y"].as_f64().unwrap()).abs() < 1e-5);
            assert_eq!(corners[i].z, 1.25);
            assert_eq!(corners[i + 4].z, 1.25 + 2.8);
        }
        assert_eq!(geometry::hit_plan(&created, 0, center, 0.001), Some(id));
        // Inside the bounding box, outside the actual rotated rectangle.
        let max_x = corners[..4]
            .iter()
            .map(|c| c.x)
            .fold(f64::NEG_INFINITY, f64::max);
        let max_y = corners[..4]
            .iter()
            .map(|c| c.y)
            .fold(f64::NEG_INFINITY, f64::max);
        assert_eq!(
            geometry::hit_plan(&created, 0, Point::new(max_x - 0.01, max_y - 0.01), 0.001),
            None
        );
        let mut modified = created.elements[0].clone();
        if let Shape::Column { axis_rotation, .. } = &mut modified.shape {
            *axis_rotation += 90.;
        }
        editor.replace_element(modified).unwrap();
        let changed = editor.project.clone();
        editor.undo();
        assert_eq!(editor.project, created);
        editor.redo();
        assert_eq!(editor.project, changed);
        assert_eq!(
            serde_json::from_str::<Project>(&serde_json::to_string(&changed).unwrap()).unwrap(),
            changed
        );
        let mut legacy = serde_json::to_value(&created).unwrap();
        legacy["elements"][0]["shape"]
            .as_object_mut()
            .unwrap()
            .remove("axis_rotation");
        let legacy: Project = serde_json::from_value(legacy).unwrap();
        assert_eq!(
            geometry::footprint(&legacy.elements[0]),
            geometry::column_footprint(center, 0.8, 0.4)
        );
    }
}

#[test]
fn column_invalid_axis_angle_keeps_real_redo_and_periodic_geometry() {
    let mut editor = Editor::new(Project::default());
    editor.place_column(Point::default()).unwrap();
    let created = editor.project.clone();
    editor.undo();
    let before = editor.project.clone();
    for angle in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        editor.column_axis_rotation = angle;
        assert!(editor.place_column(Point::default()).is_err());
        assert_eq!(editor.project, before);
        assert_eq!(editor.redo.len(), 1);
    }
    editor.redo();
    assert_eq!(editor.project, created);
    assert_eq!(
        geometry::rotated_column_footprint(Point::default(), 0.8, 0.4, 30.),
        geometry::rotated_column_footprint(Point::default(), 0.8, 0.4, 390.)
    );
}

#[test]
fn circular_column_model_history_files_and_exact_pick() {
    use atelier_bim::column::{ColumnSection, solid};
    use glam::DVec3;
    let mut editor = Editor::new(Project::default());
    editor.story = 1;
    editor.column_bottom_offset = -0.75;
    editor.column_axis_rotation = 37.;
    let id = editor
        .place_column_section(
            Point::default(),
            ColumnSection::Circular { diameter: 2. },
            3.,
        )
        .unwrap();
    let created = editor.project.clone();
    let e = &created.elements[0];
    assert!((e.area() - std::f64::consts::PI).abs() < 1e-6);
    assert_eq!(e.thickness(), 2.);
    let outline = geometry::element_plan_outline(&created, e);
    assert!(outline.len() >= 32);
    assert!(
        outline
            .iter()
            .all(|p| (p.distance(Point::default()) - 1.).abs() < 1e-6)
    );
    assert_eq!(
        geometry::hit_plan(&created, 1, Point::new(0.9, 0.9), 0.),
        None
    );
    assert_eq!(
        geometry::hit_plan(&created, 1, Point::new(0.7, 0.7), 0.),
        Some(id)
    );
    assert!(
        atelier_bim::guides::element_paths(
            e,
            atelier_bim::guides::DistanceReference::ReferenceLines
        )
        .is_empty()
    );
    editor.set_selection(vec![id]);
    assert!(
        editor
            .edit(atelier_bim::editing::EditCommand::Explode)
            .is_err()
    );
    assert_eq!(editor.project, created);
    let body = solid(&created, e).unwrap();
    assert_eq!(body.bottom, created.elevation(1) - 0.75);
    assert!(!body.mesh().unwrap().triangles.is_empty());
    // Real camera-picking entry point, aimed vertically through the AABB corner.
    for (x, y, expected) in [(0.9, 0.9, None), (0.7, 0.7, Some(id))] {
        let camera = geometry::Camera {
            target: DVec3::new(x, y, body.bottom + 1.5),
            yaw: 0.,
            pitch: std::f64::consts::FRAC_PI_2 - 0.00001,
            span: 5.,
        };
        assert_eq!(camera.pick(&created, [0., 0.], 1., Some(1)), expected);
    }
    // Rendering samples are never exposed as control nodes or endpoint snaps.
    assert!(atelier_bim::column::edit_nodes(e).is_empty());
    assert_ne!(
        geometry::snap(&created, 1, outline[1], 0.001, true).1,
        geometry::SnapKind::Endpoint
    );
    let mut changed = e.clone();
    if let Shape::Column { section, .. } = &mut changed.shape {
        *section = ColumnSection::Circular { diameter: 1.2 };
    }
    editor.replace_element(changed).unwrap();
    let modified = editor.project.clone();
    editor.undo();
    assert_eq!(editor.project, created);
    editor.redo();
    assert_eq!(editor.project, modified);
    editor
        .apply(ModelCommand::MoveElements {
            ids: vec![id],
            offset: Point::new(2., -1.),
        })
        .unwrap();
    assert_eq!(
        solid(&editor.project, &editor.project.elements[0])
            .unwrap()
            .section,
        ColumnSection::Circular { diameter: 1.2 }
    );
    let path = std::env::temp_dir().join(format!(
        "openarchi-round-column-{}.json",
        std::process::id()
    ));
    storage::save(&editor.project, &path).unwrap();
    assert_eq!(storage::load(&path).unwrap(), editor.project);
    let wire: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let shape = &wire["elements"][0]["shape"];
    assert_eq!(shape["section"], "Circular");
    assert_eq!(shape["diameter"].as_f64().unwrap() as f64, 1.2_f64);
    assert!(shape.get("width").is_none() && shape.get("depth").is_none());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn column_section_adapter_preserves_legacy_and_rejects_conflicts_atomically() {
    use atelier_bim::column::ColumnSection;
    let legacy = serde_json::json!({"kind":"Column","center":{"x":2.,"y":3.},"width":0.6,"depth":0.4,"height":3.});
    let old: Shape = serde_json::from_value(legacy.clone()).unwrap();
    assert!(matches!(
        old,
        Shape::Column {
            section: ColumnSection::Rectangular {
                width: 0.6,
                depth: 0.4
            },
            bottom_offset: 0.,
            axis_rotation: 0.,
            inclination: None,
            plan: _,
            ..
        }
    ));
    let saved = serde_json::to_value(&old).unwrap();
    assert!(saved.get("section").is_none());
    assert_eq!(saved["width"], serde_json::json!(0.6_f64));
    for patch in [
        serde_json::json!({"section":"Circular","diameter":0.6}),
        serde_json::json!({"diameter":0.6}),
        serde_json::json!({"section":"Unknown"}),
    ] {
        let mut value = legacy.clone();
        value
            .as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        assert!(serde_json::from_value::<Shape>(value).is_err());
    }
    let mut editor = Editor::new(Project::default());
    editor
        .place_column_section(
            Point::default(),
            ColumnSection::Circular { diameter: 0.6 },
            3.,
        )
        .unwrap();
    let created = editor.project.clone();
    editor.undo();
    let before = editor.project.clone();
    let revision = editor.revision;
    for diameter in [0., -1., f64::NAN, f64::INFINITY, 101.] {
        assert!(
            editor
                .place_column_section(Point::default(), ColumnSection::Circular { diameter }, 3.)
                .is_err()
        );
        assert_eq!(editor.project, before);
        assert_eq!(editor.revision, revision);
        assert_eq!(editor.redo.len(), 1);
    }
    editor.redo();
    assert_eq!(editor.project, created);
}
