use atelier_bim::{
    editor::{Editor, Tool},
    geometry,
    model::*,
    storage::JsonAdapter,
    wall::WallSettings,
};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn editor_with_wall() -> (Editor, u64) {
    let mut editor = Editor::new(Project::default());
    editor.set_tool(Tool::Wall);
    editor.click(p(0., 0.), 0.);
    editor.click(p(6., 0.), 0.);
    let id = editor.project.elements[0].id;
    (editor, id)
}

fn opening(element: &Element) -> (u64, OpeningKind, f64, f64, f64, f64) {
    match element.shape {
        Shape::Opening {
            host_wall,
            kind,
            width,
            height,
            sill,
            offset,
        } => (host_wall, kind, width, height, sill, offset),
        _ => panic!("expected an opening"),
    }
}

#[test]
fn door_tool_places_a_hosted_opening_on_the_clicked_wall() {
    let (mut editor, wall) = editor_with_wall();
    editor.set_tool(Tool::Door);
    editor.click(p(3., 0.), 0.1);
    assert_eq!(editor.project.elements.len(), 2);
    let door = editor
        .project
        .elements
        .iter()
        .find(|e| e.kind() == "Door")
        .unwrap();
    let (host, kind, width, height, sill, offset) = opening(door);
    assert_eq!(host, wall);
    assert_eq!(kind, OpeningKind::Door);
    assert_eq!((width, height, sill), (0.9, 2.1, 0.0));
    assert!((offset - 3.0).abs() < 1e-4);
    assert_eq!(editor.selection_ids(), vec![door.id]);
    assert_eq!(editor.undo.len(), 2);
}

#[test]
fn window_tool_uses_sill_default_and_clamps_near_wall_end() {
    let (mut editor, _wall) = editor_with_wall();
    editor.set_tool(Tool::Window);
    editor.click(p(5.95, 0.05), 0.1);
    let window = editor
        .project
        .elements
        .iter()
        .find(|e| e.kind() == "Window")
        .unwrap();
    let (_, kind, width, height, sill, offset) = opening(window);
    assert_eq!(kind, OpeningKind::Window);
    assert_eq!((width, height, sill), (1.2, 1.4, 0.9));
    assert!((offset - (6.0 - 0.6)).abs() < 1e-4);
}

#[test]
fn validation_rejects_missing_host_out_of_range_overlap_and_wrong_story() {
    let (mut editor, wall) = editor_with_wall();
    editor.set_tool(Tool::Door);
    editor.click(p(2., 0.), 0.1);

    let mut missing = editor.project.clone();
    if let Shape::Opening { host_wall, .. } = &mut missing
        .elements
        .iter_mut()
        .find(|e| e.kind() == "Door")
        .unwrap()
        .shape
    {
        *host_wall = 999;
    }
    assert!(missing.validate().is_err());

    let mut outside = editor.project.clone();
    if let Shape::Opening { offset, .. } = &mut outside
        .elements
        .iter_mut()
        .find(|e| e.kind() == "Door")
        .unwrap()
        .shape
    {
        *offset = 6.0;
    }
    assert!(outside.validate().is_err());

    let mut wrong_story = editor.project.clone();
    if let Some(door) = wrong_story.elements.iter_mut().find(|e| e.kind() == "Door") {
        door.story = 1;
    }
    assert!(wrong_story.validate().is_err());

    editor.click(p(2.4, 0.), 0.1);
    assert_eq!(editor.project.elements.len(), 2);
    assert!(editor.project.validate().is_err() || editor.project.elements.len() == 2);
    // The second door overlaps the first, so the click must not change the model.
    assert_eq!(editor.project.elements.len(), 2);

    let mut negative = editor.project.clone();
    if let Shape::Opening { width, .. } = &mut negative
        .elements
        .iter_mut()
        .find(|e| e.kind() == "Door")
        .unwrap()
        .shape
    {
        *width = -1.0;
    }
    assert!(negative.validate().is_err());
    let _ = wall;
}

#[test]
fn deleting_the_host_wall_cascades_and_undo_restores_both() {
    let (mut editor, wall) = editor_with_wall();
    editor.set_tool(Tool::Door);
    editor.click(p(2., 0.), 0.1);
    let before = editor.project.clone();
    editor.set_selection(vec![wall]);
    editor.delete();
    assert!(editor.project.elements.is_empty());
    editor.undo();
    assert_eq!(editor.project, before);
    assert_eq!(editor.project.elements.len(), 2);
}

#[test]
fn moving_a_wall_moves_its_openings_and_an_opening_slides_along_the_wall() {
    let (mut editor, wall) = editor_with_wall();
    editor.set_tool(Tool::Door);
    editor.click(p(2., 0.), 0.1);
    let door_id = editor
        .project
        .elements
        .iter()
        .find(|e| e.kind() == "Door")
        .unwrap()
        .id;
    let Before {
        door_span,
        wall_start,
    } = snapshot(&editor);

    editor.set_selection(vec![wall]);
    editor.move_selection(p(1.5, 2.0));
    let after_wall = snapshot(&editor);
    assert!((after_wall.wall_start.x - (wall_start.x + 1.5)).abs() < 1e-4);
    assert!((after_wall.wall_start.y - (wall_start.y + 2.0)).abs() < 1e-4);
    assert!((after_wall.door_span.0.y - (door_span.0.y + 2.0)).abs() < 1e-4);
    assert!((after_wall.door_span.0.x - (door_span.0.x + 1.5)).abs() < 1e-4);

    editor.set_selection(vec![door_id]);
    editor.move_selection(p(0.4, 0.0));
    let offset = opening(
        editor
            .project
            .elements
            .iter()
            .find(|e| e.id == door_id)
            .unwrap(),
    )
    .5;
    assert!((offset - 2.4).abs() < 1e-4);

    // Perpendicular drag cannot leave the wall.
    editor.move_selection(p(0.0, 5.0));
    assert_eq!(editor.undo.len(), 4);
}

#[test]
fn dragging_an_opening_clamps_it_to_the_host_wall_ends() {
    let (mut editor, _wall) = editor_with_wall();
    editor.set_tool(Tool::Door);
    editor.click(p(3., 0.), 0.1);
    let door_id = editor
        .project
        .elements
        .iter()
        .find(|e| e.kind() == "Door")
        .unwrap()
        .id;
    editor.set_selection(vec![door_id]);

    editor.move_selection(p(100., 0.));
    let (_, _, width, _, _, offset) = opening(
        editor
            .project
            .elements
            .iter()
            .find(|e| e.id == door_id)
            .unwrap(),
    );
    assert!((offset - (6.0 - width * 0.5)).abs() < 1e-4);
    assert!(editor.project.validate().is_ok());

    editor.move_selection(p(-100., 0.));
    let (_, _, width, _, _, offset) = opening(
        editor
            .project
            .elements
            .iter()
            .find(|e| e.id == door_id)
            .unwrap(),
    );
    assert!((offset - width * 0.5).abs() < 1e-4);
    assert!(editor.project.validate().is_ok());
}

#[test]
fn raised_wall_bottom_offset_is_rejected_for_doors_and_windows() {
    let (mut editor, wall) = editor_with_wall();
    let mut settings = WallSettings::default();
    settings.geometry.bottom_offset = 1.0;
    editor
        .project
        .elements
        .iter_mut()
        .find(|e| e.id == wall)
        .unwrap()
        .wall_settings = Some(settings);

    editor.set_tool(Tool::Window);
    editor.click(p(3., 0.), 0.1);
    assert_eq!(editor.project.elements.len(), 1);

    editor.set_tool(Tool::Door);
    editor.click(p(3., 0.), 0.1);
    assert_eq!(editor.project.elements.len(), 1);
}

#[test]
fn copying_a_wall_with_its_opening_remaps_the_host() {
    let (mut editor, wall) = editor_with_wall();
    editor.set_tool(Tool::Door);
    editor.click(p(2., 0.), 0.1);
    let door_id = editor
        .project
        .elements
        .iter()
        .find(|e| e.kind() == "Door")
        .unwrap()
        .id;
    editor.set_selection(vec![wall, door_id]);
    editor.copy_selection();
    assert_eq!(editor.paste(p(0.5, 0.5)), 2);

    assert_eq!(editor.project.elements.len(), 4);
    let walls: Vec<u64> = editor
        .project
        .elements
        .iter()
        .filter(|e| matches!(e.shape, Shape::Wall { .. }))
        .map(|e| e.id)
        .collect();
    let doors: Vec<u64> = editor
        .project
        .elements
        .iter()
        .filter(|e| e.kind() == "Door")
        .map(|e| e.id)
        .collect();
    assert_eq!(walls.len(), 2);
    assert_eq!(doors.len(), 2);
    for door in &doors {
        let (host, ..) = opening(
            editor
                .project
                .elements
                .iter()
                .find(|e| e.id == *door)
                .unwrap(),
        );
        assert!(walls.contains(&host));
    }
    assert!(editor.project.validate().is_ok());

    editor.set_selection(vec![doors[0]]);
    editor.copy_selection();
    assert_eq!(editor.paste(p(0.5, 0.5)), 0);
    assert_eq!(editor.project.elements.len(), 4);
}

struct Before {
    door_span: (Point, Point),
    wall_start: Point,
}

fn snapshot(editor: &Editor) -> Before {
    let door = editor
        .project
        .elements
        .iter()
        .find(|e| e.kind() == "Door")
        .unwrap();
    let wall = editor
        .project
        .elements
        .iter()
        .find(|e| matches!(e.shape, Shape::Wall { .. }))
        .unwrap();
    let Shape::Wall { start, .. } = wall.shape else {
        unreachable!()
    };
    Before {
        door_span: geometry::opening_span(&editor.project, door).unwrap(),
        wall_start: start,
    }
}

#[test]
fn plan_hit_prefers_the_opening_and_its_rect_is_inside_the_wall() {
    let (mut editor, wall) = editor_with_wall();
    editor.set_tool(Tool::Door);
    editor.click(p(3., 0.), 0.1);
    let door_id = editor
        .project
        .elements
        .iter()
        .find(|e| e.kind() == "Door")
        .unwrap()
        .id;
    let door = editor
        .project
        .elements
        .iter()
        .find(|e| e.id == door_id)
        .unwrap();
    let rect = geometry::opening_footprint(&editor.project, door).unwrap();
    assert!(rect[0].distance(rect[1]) > 0.1);
    assert_eq!(
        geometry::hit_plan(&editor.project, 0, p(3., 0.), 0.02),
        Some(door_id)
    );
    assert_eq!(
        geometry::hit_plan(&editor.project, 0, p(1., 0.), 0.02),
        Some(wall)
    );
}

#[test]
fn openings_round_trip_through_json() {
    let (mut editor, _wall) = editor_with_wall();
    editor.set_tool(Tool::Door);
    editor.click(p(2., 0.), 0.1);
    editor.set_tool(Tool::Window);
    editor.click(p(4.5, 0.), 0.1);
    let bytes = JsonAdapter.export(&editor.project).unwrap();
    let loaded = JsonAdapter.import(&bytes).unwrap();
    assert_eq!(loaded, editor.project);
    assert_eq!(
        loaded
            .elements
            .iter()
            .filter(|e| matches!(e.shape, Shape::Opening { .. }))
            .count(),
        2
    );
}

#[test]
fn transform_drag_matches_nudge_for_standalone_and_mixed_openings() {
    use atelier_bim::editing::EditCommand;
    let (mut editor, wall) = editor_with_wall();
    editor.set_tool(Tool::Door);
    editor.click(p(2., 0.), 0.1);
    let door = editor.selection_ids()[0];
    let before = editor.project.clone();
    let command = EditCommand::Transform {
        offset: p(1., 7.),
        origin: p(0., 0.),
        angle: 0.,
        scale: 1.,
        mirror: false,
        copies: 0,
    };
    let mut nudge = Editor::new(editor.project.clone());
    nudge.set_selection(vec![door]);
    nudge.move_selection(p(1., 7.));
    editor.edit(command.clone()).unwrap();
    assert_eq!(editor.project, nudge.project);
    assert_eq!(opening(&editor.project.elements[1]).5, 3.);
    let moved = editor.project.clone();
    editor.undo();
    assert_eq!(editor.project, before);
    editor.redo();
    assert_eq!(editor.project, moved);
    editor.set_selection(vec![wall, door]);
    editor.edit(command).unwrap();
    assert_eq!(opening(&editor.project.elements[1]).5, 3.);
    assert!(matches!(editor.project.elements[0].shape,Shape::Wall {start,..} if start==p(1.,7.)));
    // A separate selected element follows the full vector while the opening
    // only uses the component along the host wall.
    let column = editor.project.add(
        0,
        Material::Plaster,
        Shape::Column {
            center: p(0., 0.),
            section: atelier_bim::column::ColumnSection::Rectangular {
                width: 0.4,
                depth: 0.4,
            },
            height: 3.,
            bottom_offset: 0.,
            axis_rotation: 0.,
            inclination: None,
            plan: Default::default(),
        },
    );
    editor.set_selection(vec![door, column]);
    editor
        .edit(EditCommand::Transform {
            offset: p(0.5, 2.),
            origin: p(0., 0.),
            angle: 0.,
            scale: 1.,
            mirror: false,
            copies: 0,
        })
        .unwrap();
    assert_eq!(opening(&editor.project.elements[1]).5, 3.5);
    assert!(
        matches!(editor.project.elements.last().unwrap().shape, Shape::Column{center,..} if center==p(0.5,2.))
    );
}

#[test]
fn opening_transform_rejection_keeps_history_and_clamps_valid_translation() {
    use atelier_bim::editing::EditCommand;
    let (mut editor, _) = editor_with_wall();
    editor.set_tool(Tool::Door);
    editor.click(p(1., 0.), 0.1);
    let door = editor.selection_ids()[0];
    editor.set_tool(Tool::Window);
    editor.click(p(3., 0.), 0.1);
    editor.set_selection(vec![door]);
    let drag = |offset| EditCommand::Transform {
        offset,
        origin: p(0., 0.),
        angle: 0.,
        scale: 1.,
        mirror: false,
        copies: 0,
    };
    let before = editor.project.clone();
    let history = editor.undo.len();
    assert!(editor.edit(drag(p(2., 0.))).is_err());
    assert_eq!(editor.project, before);
    assert_eq!(editor.undo.len(), history);
    editor.project.edit_state.locked.insert(door);
    let locked = editor.project.clone();
    assert!(editor.edit(drag(p(-0.2, 0.))).is_err());
    assert_eq!(editor.project, locked);
    editor.project.edit_state.locked.remove(&door);
    editor.edit(drag(p(-100., 12.))).unwrap();
    assert!((opening(&editor.project.elements[1]).5 - 0.45).abs() < 1e-5);
    let copied = EditCommand::Transform {
        offset: p(0., 0.),
        origin: p(0., 0.),
        angle: 0.,
        scale: 1.,
        mirror: false,
        copies: 1,
    };
    assert!(editor.edit(copied).is_err());
    let encoded = JsonAdapter.export(&editor.project).unwrap();
    assert_eq!(JsonAdapter.import(&encoded).unwrap(), editor.project);
}

#[test]
fn typed_opening_matches_pointer_model_and_readonly_candidate_for_both_kinds() {
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        let (mut direct, _) = editor_with_wall();
        direct.wall_renovation = atelier_bim::view_settings::RenovationStatus::Existing;
        let before = direct.project.clone();
        let revision = direct.revision;
        let candidate = direct.opening_placement(kind, p(5.95, 0.05), 0.1).unwrap();
        assert_eq!(direct.project, before);
        assert_eq!(direct.revision, revision);
        let id = direct.place_opening(kind, p(5.95, 0.05), 0.1).unwrap();
        let actual = direct.project.elements.iter().find(|e| e.id == id).unwrap();
        assert_eq!(actual.shape, candidate.shape);
        assert_eq!(actual.material, candidate.material);
        assert_eq!(actual.renovation, candidate.renovation);
        let mut pointer = Editor::new(before.clone());
        pointer.wall_renovation = direct.wall_renovation;
        pointer.set_tool(if kind == OpeningKind::Door {
            Tool::Door
        } else {
            Tool::Window
        });
        pointer.click(p(5.95, 0.05), 0.1);
        assert_eq!(pointer.project, direct.project);
        assert_eq!(pointer.selected, Some(id));
        let created = direct.project.clone();
        direct.undo();
        assert_eq!(direct.project, before);
        direct.redo();
        assert_eq!(direct.project, created);
        let path = std::env::temp_dir().join(format!(
            "oa-opening-typed-{}-{}.json",
            std::process::id(),
            kind.name()
        ));
        atelier_bim::storage::save(&created, &path).unwrap();
        assert_eq!(atelier_bim::storage::load(&path).unwrap(), created);
        std::fs::remove_file(path).unwrap();
    }
}
#[test]
fn typed_opening_invalid_and_exhausted_identity_keep_project_selection_and_redo() {
    let (mut e, _) = editor_with_wall();
    e.place_opening(OpeningKind::Door, p(3., 0.), 0.1).unwrap();
    let created = e.project.clone();
    e.undo();
    let before = e.project.clone();
    let revision = e.revision;
    let selected = e.selection_ids();
    assert!(e.place_opening(OpeningKind::Door, p(3., 10.), 0.1).is_err());
    e.project.next_id = u64::MAX;
    let exhausted = e.project.clone();
    assert!(e.place_opening(OpeningKind::Door, p(3., 0.), 0.1).is_err());
    assert_eq!(e.project, exhausted);
    e.project.next_id = before.next_id;
    assert_eq!(e.project, before);
    assert_eq!(e.revision, revision);
    assert_eq!(e.selection_ids(), selected);
    e.redo();
    assert_eq!(e.project, created);
}

#[test]
fn opening_creation_candidate_is_read_only_and_matches_commit_with_real_redo() {
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        let (mut e, host) = editor_with_wall();
        e.place_column(p(9., 9.)).unwrap();
        let redo = e.project.clone();
        e.undo();
        let before = e.project.clone();
        let revision = e.revision;
        let history = e.undo.len();
        let selection = e.selection_ids();
        let status = e.status.clone();
        let candidate = e
            .opening_creation_candidate(kind, p(5.95, 0.05), 0.1)
            .unwrap();
        assert_eq!(candidate.id, before.next_id);
        assert_eq!(opening(&candidate).0, host);
        assert_eq!(
            candidate.name,
            format!("{} {:03}", kind.name(), candidate.id)
        );
        let preview = e
            .preview_add_elements(vec![candidate.clone()], false)
            .unwrap();
        assert!(
            e.opening_creation_candidate(kind, p(50., 50.), 0.1)
                .is_err()
        );
        assert_eq!(e.project, before);
        assert_eq!(e.revision, revision);
        assert_eq!(e.undo.len(), history);
        assert_eq!(e.selection_ids(), selection);
        assert_eq!(e.status, status);
        e.redo();
        assert_eq!(e.project, redo);
        e.undo();
        let id = e.place_opening(kind, p(5.95, 0.05), 0.1).unwrap();
        assert_eq!(id, candidate.id);
        assert_eq!(e.project.elements.last(), Some(&candidate));
        assert_eq!(e.project, preview);
        assert_eq!(e.undo.len(), history + 1);
        let serialized = serde_json::to_string(&e.project).unwrap();
        assert_eq!(
            serde_json::from_str::<Project>(&serialized).unwrap(),
            preview
        );
        e.undo();
        assert_eq!(e.project, before);
        e.redo();
        assert_eq!(e.project, preview);
    }
}

#[test]
fn near_full_wall_openings_respect_existing_tolerance_without_panicking() {
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        for width in [5.99995, 6., 6.00005] {
            let (mut e, _) = editor_with_wall();
            let mut defaults = kind.defaults();
            defaults.width = width;
            e.set_opening_defaults(kind, defaults);
            e.place_column(p(9., 9.)).unwrap();
            let redo = e.project.clone();
            e.undo();
            let before = e.project.clone();
            let revision = e.revision;
            let candidate = e
                .opening_creation_candidate(kind, p(5.95, 0.), 0.1)
                .unwrap();
            let (_, _, actual_width, _, _, offset) = opening(&candidate);
            assert_eq!(actual_width, width);
            assert!((offset - 3.).abs() < 0.0001);
            let expected = e.preview_add_elements(vec![candidate], false).unwrap();
            assert_eq!(e.project, before);
            assert_eq!(e.revision, revision);
            e.redo();
            assert_eq!(e.project, redo);
            e.undo();
            e.place_opening(kind, p(5.95, 0.), 0.1).unwrap();
            assert_eq!(e.project, expected);
            e.project.validate().unwrap();
            let json = serde_json::to_string(&e.project).unwrap();
            assert_eq!(serde_json::from_str::<Project>(&json).unwrap(), expected);
            e.undo();
            assert_eq!(e.project, before);
            defaults.width = 6.001;
            e.set_opening_defaults(kind, defaults);
            assert!(e.opening_creation_candidate(kind, p(3., 0.), 0.1).is_err());
            assert!(e.place_opening(kind, p(3., 0.), 0.1).is_err());
            assert_eq!(e.project, before);
            e.redo();
            assert_eq!(e.project, expected);
        }
    }
}

#[test]
fn explicit_host_creation_uses_exact_parameters_without_ui_state_changes() {
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        let (mut e, host) = editor_with_wall();
        e.project.elements[0].material = Material::Timber;
        e.story = 1;
        e.place_column(p(9., 9.)).unwrap();
        let redo = e.project.clone();
        e.undo();
        let before = e.project.clone();
        let revision = e.revision;
        let selection = e.selection_ids();
        let status = e.status.clone();
        let defaults = e.opening_defaults(kind);
        let dimensions = OpeningDefaults {
            width: 1.1,
            height: 1.8,
            sill: 0.2,
        };
        let candidate = e
            .opening_on_wall_candidate(kind, host, 2.25, dimensions)
            .unwrap();
        assert_eq!(candidate.story, 0);
        assert_eq!(candidate.material, Material::Timber);
        assert_eq!(opening(&candidate), (host, kind, 1.1, 1.8, 0.2, 2.25));
        let expected = e
            .preview_add_elements(vec![candidate.clone()], false)
            .unwrap();
        assert_eq!(e.project, before);
        assert_eq!(e.revision, revision);
        assert_eq!(e.selection_ids(), selection);
        e.redo();
        assert_eq!(e.project, redo);
        e.undo();
        let selection = e.selection_ids();
        let history = e.undo.len();
        assert_eq!(
            e.place_opening_on_wall(kind, host, 2.25, dimensions)
                .unwrap(),
            candidate.id
        );
        assert_eq!(e.project, expected);
        assert_eq!(e.undo.len(), history + 1);
        assert_eq!(e.story, 1);
        assert_eq!(e.selection_ids(), selection);
        assert_eq!(e.status, status);
        let unchanged = e.opening_defaults(kind);
        assert_eq!(
            (unchanged.width, unchanged.height, unchanged.sill),
            (defaults.width, defaults.height, defaults.sill)
        );
        let path = std::env::temp_dir().join(format!(
            "oa-explicit-host-{}-{}.json",
            std::process::id(),
            kind.name()
        ));
        atelier_bim::storage::save(&e.project, &path).unwrap();
        assert_eq!(atelier_bim::storage::load(&path).unwrap(), expected);
        std::fs::remove_file(path).unwrap();
        e.undo();
        assert_eq!(e.project, before);
        e.redo();
        assert_eq!(e.project, expected);
    }
}

#[test]
fn explicit_host_rejections_are_atomic_and_do_not_clamp_invalid_stations() {
    let (mut e, host) = editor_with_wall();
    let column = e.place_column(p(9., 9.)).unwrap();
    e.place_opening_on_wall(OpeningKind::Door, host, 2., OpeningKind::Door.defaults())
        .unwrap();
    let redo = e.project.clone();
    e.undo();
    let before = e.project.clone();
    let revision = e.revision;
    let status = e.status.clone();
    for (target, offset, width) in [
        (host, -1., 0.9),
        (host, 0., 0.9),
        (host, 6., 0.9),
        (host, f64::NAN, 0.9),
        (host, f64::INFINITY, 0.9),
        (host, 3., f64::NAN),
        (host, 3., 7.),
        (u64::MAX, 2., 0.9),
        (column, 2., 0.9),
    ] {
        let d = OpeningDefaults {
            width,
            ..OpeningKind::Door.defaults()
        };
        assert!(
            e.opening_on_wall_candidate(OpeningKind::Door, target, offset, d)
                .is_err()
        );
        assert!(
            e.place_opening_on_wall(OpeningKind::Door, target, offset, d)
                .is_err()
        );
        assert_eq!(e.project, before);
        assert_eq!(e.revision, revision);
        assert_eq!(e.status, status);
    }
    e.project.edit_state.locked.insert(host);
    let locked = e.project.clone();
    assert!(
        e.place_opening_on_wall(OpeningKind::Door, host, 2., OpeningKind::Door.defaults())
            .is_err()
    );
    assert_eq!(e.project, locked);
    e.project = before.clone();
    e.project.next_id = u64::MAX;
    let exhausted = e.project.clone();
    assert!(
        e.place_opening_on_wall(OpeningKind::Door, host, 2., OpeningKind::Door.defaults())
            .is_err()
    );
    assert_eq!(e.project, exhausted);
    e.project = before;
    e.redo();
    assert_eq!(e.project, redo);
    assert!(
        e.place_opening_on_wall(
            OpeningKind::Window,
            host,
            2.,
            OpeningKind::Window.defaults()
        )
        .is_err()
    );
    assert_eq!(e.project, redo);
}
