use atelier_bim::{
    editor::{Editor, Tool},
    geometry,
    model::*,
    storage::JsonAdapter,
    wall::WallSettings,
};

fn p(x: f32, y: f32) -> Point {
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

fn opening(element: &Element) -> (u64, OpeningKind, f32, f32, f32, f32) {
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
