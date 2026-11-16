use atelier_bim::{
    column::{self, ColumnSection},
    column_inclination::ColumnInclination,
    command::ModelCommand,
    editor::Editor,
    model::{Point, Project, Shape},
    storage,
};
fn tilt() -> ColumnInclination {
    ColumnInclination {
        slant_degrees: 60.,
        direction_degrees: -90.,
    }
}
#[test]
fn inclined_columns_share_atomic_commands_history_and_files() {
    for section in [
        ColumnSection::Rectangular {
            width: 0.3,
            depth: 0.6,
        },
        ColumnSection::Circular { diameter: 0.6 },
    ] {
        let mut e = Editor::new(Project::default());
        e.story = 1;
        e.column_bottom_offset = -0.75;
        e.column_axis_rotation = 30.;
        let before = e.project.clone();
        let id = e
            .place_column_with_inclination(Point::new(2., 3.), section, 2.8, Some(tilt()))
            .unwrap();
        let created = e.project.clone();
        let solid = column::inclined_solid(&created, &created.elements[0]).unwrap();
        assert_eq!(solid.column.bottom, created.elevation(1) - 0.75);
        assert_eq!(solid.column.angle_degrees, 30.);
        assert!(solid.mesh().is_ok());
        assert!(
            column::solid(&created, &created.elements[0]).is_none(),
            "vertical-only path must not silently flatten inclination"
        );
        e.apply(ModelCommand::MoveElements {
            ids: vec![id],
            offset: Point::new(4., -2.),
        })
        .unwrap();
        let moved = e.project.clone();
        let shifted = column::inclined_solid(&moved, &moved.elements[0]).unwrap();
        assert_eq!(shifted.slant_degrees, 60.);
        assert_eq!(shifted.direction_degrees, -90.);
        for (a, b) in solid
            .bounds()
            .unwrap()
            .into_iter()
            .zip(shifted.bounds().unwrap())
        {
            assert!((b - a - glam::DVec3::new(4., -2., 0.)).length() < 0.00001);
        }
        e.undo();
        assert_eq!(e.project, created);
        e.undo();
        assert_eq!(e.project, before);
        e.redo();
        e.redo();
        assert_eq!(e.project, moved);
        let path = std::env::temp_dir().join(format!(
            "oa-inclined-column-{}-{}.json",
            std::process::id(),
            section.is_circular()
        ));
        storage::save(&e.project, &path).unwrap();
        assert_eq!(storage::load(&path).unwrap(), moved);
        std::fs::remove_file(path).unwrap();
    }
}
#[test]
fn legacy_vertical_files_do_not_gain_inclination_or_change_geometry() {
    let mut e = Editor::new(Project::default());
    e.place_column(Point::new(2., 3.)).unwrap();
    let json = serde_json::to_string(&e.project).unwrap();
    assert!(!json.contains("inclination"));
    let loaded: Project = serde_json::from_str(&json).unwrap();
    loaded.validate().unwrap();
    assert_eq!(loaded, e.project);
    let vertical = column::solid(&loaded, &loaded.elements[0]).unwrap();
    let generalized = column::inclined_solid(&loaded, &loaded.elements[0]).unwrap();
    assert_eq!(
        vertical.mesh().unwrap().triangles,
        generalized.mesh().unwrap().triangles
    );
    assert_eq!(vertical.bounds().unwrap(), generalized.bounds().unwrap());
}
#[test]
fn invalid_inclination_preserves_project_revision_and_redo() {
    let mut e = Editor::new(Project::default());
    e.place_column(Point::default()).unwrap();
    e.undo();
    let before = e.project.clone();
    let revision = e.revision;
    for t in [
        ColumnInclination {
            slant_degrees: 0.,
            ..tilt()
        },
        ColumnInclination {
            slant_degrees: 91.,
            ..tilt()
        },
        ColumnInclination {
            slant_degrees: f64::NAN,
            ..tilt()
        },
        ColumnInclination {
            direction_degrees: f64::INFINITY,
            ..tilt()
        },
        ColumnInclination {
            slant_degrees: 1e-200,
            ..tilt()
        },
    ] {
        assert!(
            e.place_column_with_inclination(Point::default(), e.column_section, 3., Some(t))
                .is_err()
        );
        assert_eq!(e.project, before);
        assert_eq!(e.revision, revision);
        assert_eq!(e.redo.len(), 1);
    }
    e.redo();
    let before = e.project.clone();
    let mut changed = before.elements[0].clone();
    if let Shape::Column { inclination, .. } = &mut changed.shape {
        *inclination = Some(ColumnInclination {
            slant_degrees: -30.,
            ..tilt()
        });
    }
    assert!(e.replace_element(changed).is_err());
    assert_eq!(e.project, before);
}
#[test]
fn replacing_orientation_is_one_undoable_change_and_malformed_files_fail() {
    let mut e = Editor::new(Project::default());
    e.place_column(Point::default()).unwrap();
    let before = e.project.clone();
    let mut changed = before.elements[0].clone();
    if let Shape::Column { inclination, .. } = &mut changed.shape {
        *inclination = Some(tilt());
    }
    e.replace_element(changed).unwrap();
    let after = e.project.clone();
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project, after);
    let text = serde_json::to_string(&after).unwrap();
    assert!(text.contains("slant_degrees"));
    let missing = text.replace(
        "\"direction_degrees\":-90.0",
        "\"misspelled_direction\":-90.0",
    );
    assert!(serde_json::from_str::<Project>(&missing).is_err());
    let mut invalid: Project =
        serde_json::from_str(&text.replace("\"slant_degrees\":60.0", "\"slant_degrees\":0.0"))
            .unwrap();
    assert!(invalid.validate().is_err());
    if let Shape::Column { inclination, .. } = &mut invalid.elements[0].shape {
        *inclination = None;
    }
    invalid.validate().unwrap();
}

#[test]
fn inclined_column_selection_move_and_delete_preserve_cross_story_geometry_and_history() {
    for section in [
        ColumnSection::Rectangular {
            width: 0.4,
            depth: 0.6,
        },
        ColumnSection::Circular { diameter: 0.6 },
    ] {
        let mut e = Editor::new(Project::default());
        e.column_axis_rotation = 25.;
        let id = e
            .place_column_with_inclination(Point::new(2., 3.), section, 6., Some(tilt()))
            .unwrap();
        e.switch_story(1);
        e.set_selection(vec![id]);
        let before = e.project.clone();
        let original = atelier_bim::column_plan::for_element(&before, &before.elements[0], 1)
            .unwrap()
            .unwrap();
        let references =
            atelier_bim::column_plan::reference_points(&before, &before.elements[0], 1);
        assert!(!original.cut.is_empty() && !references.is_empty());
        let history = e.undo.len();
        let offset = Point::new(4., -2.);
        e.move_selection(offset);
        let moved = e.project.clone();
        assert_eq!(e.undo.len(), history + 1);
        assert_eq!(e.selection_ids(), vec![id]);
        let shifted = atelier_bim::column_plan::for_element(&moved, &moved.elements[0], 1)
            .unwrap()
            .unwrap();
        let shifted_refs =
            atelier_bim::column_plan::reference_points(&moved, &moved.elements[0], 1);
        for (a, b) in [
            (&original.cut, &shifted.cut),
            (&original.uncut, &shifted.uncut),
            (&original.overhead, &shifted.overhead),
            (&references, &shifted_refs),
        ] {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                assert!((b.x - a.x - offset.x).abs() < 0.00001);
                assert!((b.y - a.y - offset.y).abs() < 0.00001);
            }
        }
        let mut expected = before.elements[0].clone();
        if let Shape::Column { center, .. } = &mut expected.shape {
            center.x += offset.x;
            center.y += offset.y;
        }
        assert_eq!(moved.elements[0], expected);
        e.delete();
        let deleted = e.project.clone();
        assert!(deleted.elements.is_empty());
        assert!(e.selection_ids().is_empty());
        assert_eq!(e.undo.len(), history + 2);
        e.undo();
        assert_eq!(e.project, moved);
        e.undo();
        assert_eq!(e.project, before);
        e.redo();
        assert_eq!(e.project, moved);
        e.redo();
        assert_eq!(e.project, deleted);
    }
}

#[test]
fn inclined_column_batch_failures_keep_selection_revision_and_real_redo() {
    let mut e = Editor::new(Project::default());
    let a = e
        .place_column_with_inclination(
            Point::default(),
            ColumnSection::Circular { diameter: 0.6 },
            6.,
            Some(tilt()),
        )
        .unwrap();
    let b = e
        .place_column_with_inclination(
            Point::new(4., 0.),
            ColumnSection::Rectangular {
                width: 0.4,
                depth: 0.6,
            },
            6.,
            Some(tilt()),
        )
        .unwrap();
    e.set_selection(vec![a, b]);
    e.move_selection(Point::new(1., 2.));
    let redo_target = e.project.clone();
    e.undo();
    e.set_selection(vec![a, b]);
    let baseline = e.project.clone();
    let revision = e.revision;
    let history = (e.undo.len(), e.redo.len());
    assert_eq!(history.1, 1);
    e.move_selection(Point::new(200_000., 0.));
    assert_eq!(e.project, baseline);
    assert_eq!(e.revision, revision);
    assert_eq!((e.undo.len(), e.redo.len()), history);
    assert_eq!(e.selection_ids(), vec![a, b]);
    e.project.edit_state.locked.insert(b);
    let locked = e.project.clone();
    e.move_selection(Point::new(1., 0.));
    assert_eq!(e.project, locked);
    e.delete();
    assert_eq!(e.project, locked);
    assert_eq!(e.revision, revision);
    assert_eq!((e.undo.len(), e.redo.len()), history);
    assert_eq!(e.selection_ids(), vec![a, b]);
    e.project.edit_state.locked.remove(&b);
    e.redo();
    assert_eq!(e.project, redo_target);
}
