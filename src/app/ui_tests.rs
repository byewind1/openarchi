use super::*;
fn setup() -> (Context, AtelierApp) {
    let ctx = Context::default();
    AtelierApp::style(&ctx);
    let mut app = AtelierApp::from_renderer(None, "Headless test".into());
    app.recovery_path = None;
    // Headless tests run on macOS too, where the production app hides the
    // in-window menus in favor of the native menu bar; force the egui menu
    // path back on so workspace tests keep covering it.
    app.in_window_menus = true;
    app.editor = Editor::new(Project::default());
    app.plan_center = Point::default();
    app.plan_scale = 50.;
    (ctx, app)
}
// A non-repeat Key event in these helpers means a new user keystroke.
// egui 0.36 derives repeat from held-key state, ignoring the supplied flag;
// release a prior synthetic press before sending the next discrete stroke.
fn discrete_key_events(ctx: &Context, events: Vec<Event>) -> Vec<Event> {
    let mut result = Vec::new();
    for event in events {
        if let Event::Key {
            key,
            physical_key,
            pressed: true,
            repeat: false,
            modifiers,
        } = &event
        {
            if ctx.input(|input| input.key_down(*key)) {
                result.push(Event::Key {
                    key: *key,
                    physical_key: *physical_key,
                    pressed: false,
                    repeat: false,
                    modifiers: *modifiers,
                });
            }
        }
        result.push(event);
    }
    result
}
fn plan_frame(ctx: &Context, app: &mut AtelierApp, events: Vec<Event>) -> CursorIcon {
    let output = plan_frame_output(ctx, app, events);
    output.platform_output.cursor_icon
}
fn plan_frame_output(
    ctx: &Context,
    app: &mut AtelierApp,
    events: Vec<Event>,
) -> eframe::egui::FullOutput {
    let events = discrete_key_events(ctx, events);
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
            events,
            ..Default::default()
        },
        |ui| {
            app.shortcuts(ui.ctx());
            CentralPanel::default()
                .frame(Frame::NONE)
                .show(ui, |ui| app.plan(ui));
        },
    );
    output.textures_delta.clear();
    output
}
fn model_frame(ctx: &Context, app: &mut AtelierApp, events: Vec<Event>) {
    model_frame_output(ctx, app, events);
}
fn model_frame_output(
    ctx: &Context,
    app: &mut AtelierApp,
    events: Vec<Event>,
) -> eframe::egui::FullOutput {
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
            events,
            ..Default::default()
        },
        |ui| {
            CentralPanel::default()
                .frame(Frame::NONE)
                .show(ui, |ui| app.model_view(ui));
        },
    );
    output.textures_delta.clear();
    output
}
fn click(ctx: &Context, app: &mut AtelierApp, pos: Pos2) -> CursorIcon {
    plan_frame(ctx, app, vec![Event::PointerMoved(pos)]);
    plan_frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    plan_frame(
        ctx,
        app,
        vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    )
}
fn secondary_click(ctx: &Context, app: &mut AtelierApp, pos: Pos2) -> CursorIcon {
    plan_frame(ctx, app, vec![Event::PointerMoved(pos)]);
    plan_frame(
        ctx,
        app,
        vec![Event::PointerButton {
            pos,
            button: PointerButton::Secondary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    plan_frame(
        ctx,
        app,
        vec![Event::PointerButton {
            pos,
            button: PointerButton::Secondary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    )
}
fn drag(ctx: &Context, app: &mut AtelierApp, from: Pos2, to: Pos2) {
    plan_frame(ctx, app, vec![Event::PointerMoved(from)]);
    plan_frame(
        ctx,
        app,
        vec![Event::PointerButton {
            pos: from,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    plan_frame(ctx, app, vec![Event::PointerMoved(to)]);
    plan_frame(
        ctx,
        app,
        vec![Event::PointerButton {
            pos: to,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
}
#[test]
fn select_cursor_uses_arrow_on_body_check_at_nodes_and_triple_ray_on_edges() {
    let (ctx, mut app) = setup();
    app.editor.project.add(
        0,
        Material::Plaster,
        Shape::Polyline {
            points: vec![Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 2.)],
            arcs: vec![],
        },
    );
    app.editor.project.add(
        0,
        Material::Concrete,
        Shape::Slab {
            min: Point::new(4., 0.),
            max: Point::new(6., 2.),
            thickness: 0.2,
        },
    );

    assert_eq!(
        app.select_cursor_feedback(Point::new(5., 1.)),
        CursorFeedback::Arrow
    );
    assert_eq!(
        app.select_cursor_feedback(Point::new(0., 0.)),
        CursorFeedback::ArrowWithCheck
    );
    assert_eq!(
        app.select_cursor_feedback(Point::new(1., 0.)),
        CursorFeedback::ArrowWithTripleRay
    );
    assert_eq!(
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(500., 300.))]),
        CursorIcon::Default,
        "the node cursor must use the arrow platform icon"
    );
}
fn click_model(ctx: &Context, app: &mut AtelierApp, pos: Pos2, button: PointerButton) {
    model_frame(ctx, app, vec![Event::PointerMoved(pos)]);
    model_frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    model_frame(
        ctx,
        app,
        vec![Event::PointerButton {
            pos,
            button,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
}
fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}
fn collect_text(shapes: &[egui::epaint::ClippedShape], output: &mut Vec<String>) {
    fn walk(shape: &egui::Shape, output: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(text) => output.push(text.galley.text().to_owned()),
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    walk(shape, output);
                }
            }
            _ => {}
        }
    }
    for clipped in shapes {
        walk(&clipped.shape, output);
    }
}
fn text_center(shapes: &[egui::epaint::ClippedShape], needle: &str) -> Option<Pos2> {
    fn walk(shape: &egui::Shape, needle: &str) -> Option<Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == needle => {
                Some(text.pos + text.galley.size() / 2.)
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| walk(shape, needle)),
            _ => None,
        }
    }
    shapes
        .iter()
        .find_map(|clipped| walk(&clipped.shape, needle))
}
fn collect_shapes<'a>(shapes: &'a [egui::epaint::ClippedShape], output: &mut Vec<&'a egui::Shape>) {
    fn walk<'a>(shape: &'a egui::Shape, output: &mut Vec<&'a egui::Shape>) {
        output.push(shape);
        if let egui::Shape::Vec(shapes) = shape {
            for shape in shapes {
                walk(shape, output);
            }
        }
    }
    for clipped in shapes {
        walk(&clipped.shape, output);
    }
}
#[test]
fn selected_wall_draws_the_archicad_style_reference_line_and_handles() {
    let (ctx, mut app) = setup();
    let id = app.editor.project.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: Point::new(0., 0.),
            end: Point::new(4., 0.),
            thickness: 0.3,
            height: 3.,
        },
    );
    app.editor.project.elements[0].wall_settings = Some(WallSettings::default());
    app.editor.project.elements[0]
        .wall_settings
        .as_mut()
        .unwrap()
        .geometry
        .reference_offset = 0.25;

    let output = plan_frame_output(&ctx, &mut app, vec![]);
    let mut shapes = Vec::new();
    collect_shapes(&output.shapes, &mut shapes);
    assert!(
        !shapes.iter().any(|shape| matches!(
            shape,
            egui::Shape::LineSegment { stroke, .. } if stroke.color == WALL_REFERENCE_LINE
        )),
        "an unselected wall must not show its reference line"
    );

    app.editor.set_selection(vec![id]);
    let output = plan_frame_output(&ctx, &mut app, vec![]);
    let mut shapes = Vec::new();
    collect_shapes(&output.shapes, &mut shapes);
    assert!(shapes.iter().any(|shape| matches!(
        shape,
        egui::Shape::Mesh(mesh) if !mesh.indices.is_empty()
            && mesh.vertices.iter().all(|vertex| vertex.color == WALL_SELECTION_FILL)
    )));
    let (line, stroke) = shapes
        .iter()
        .find_map(|shape| match shape {
            egui::Shape::LineSegment { points, stroke } if stroke.color == WALL_REFERENCE_LINE => {
                Some((*points, *stroke))
            }
            _ => None,
        })
        .expect("selected wall reference line");
    assert_eq!(stroke.width, WALL_REFERENCE_LINE_WIDTH);
    let handles: Vec<Rect> = shapes
        .iter()
        .filter_map(|shape| match shape {
            egui::Shape::Rect(rect)
                if rect.fill == Color32::BLACK
                    && (rect.rect.width() - WALL_REFERENCE_HANDLE_SIZE).abs() < 1e-4
                    && (rect.rect.height() - WALL_REFERENCE_HANDLE_SIZE).abs() < 1e-4 =>
            {
                Some(rect.rect)
            }
            _ => None,
        })
        .collect();
    assert_eq!(handles.len(), 2);
    for endpoint in line {
        assert!(
            handles
                .iter()
                .any(|handle| handle.center().distance(endpoint) < 1e-4),
            "the blue line must end in the black square handles"
        );
    }
    let before = app.editor.project.clone();
    let undo_count = app.editor.undo.len();
    let length = app
        .editor
        .project
        .plan_options
        .dimension_style
        .format_length(4.);
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        app.language = language;
        let output = plan_frame_output(&ctx, &mut app, vec![]);
        let mut shapes = Vec::new();
        collect_shapes(&output.shapes, &mut shapes);
        let selection_label_at = pos2(500., 286.);
        assert!(!shapes.iter().any(|shape|matches!(shape,egui::Shape::Text(text) if text.galley.text()==length && (text.pos+text.galley.size()/2.).distance(selection_label_at)<1.)),"selected baseline must not carry its own wall-length label");
        assert!(
            text_center(&output.shapes, &length).is_some(),
            "the separate overall dimension must remain"
        );
        assert_eq!(app.editor.project, before);
        assert_eq!(app.editor.undo.len(), undo_count);
    }
}

#[test]
fn same_material_wall_join_has_no_internal_plan_outline_across_renovation_groups() {
    let (ctx, mut app) = setup();
    app.editor.project.add_with_status(
        0,
        Material::Plaster,
        Shape::Wall {
            start: Point::new(0., 0.),
            end: Point::new(4., 0.),
            thickness: 0.2,
            height: 3.,
        },
        RenovationStatus::Existing,
    );
    app.editor.project.add_with_status(
        0,
        Material::Plaster,
        Shape::Wall {
            start: Point::new(2., 0.),
            end: Point::new(2., 2.),
            thickness: 0.2,
            height: 3.,
        },
        RenovationStatus::New,
    );

    let output = plan_frame_output(&ctx, &mut app, vec![]);
    let mut shapes = Vec::new();
    collect_shapes(&output.shapes, &mut shapes);
    let style = app
        .plan_style(&app.editor.project.elements[0])
        .expect("wall plan style");
    let outline_color = AtelierApp::plan_color(style.line);
    let mut outline_segments = Vec::new();
    for shape in shapes {
        match shape {
            egui::Shape::Path(path)
                if path.stroke.color == egui::epaint::ColorMode::Solid(outline_color) =>
            {
                outline_segments.extend(path.points.windows(2).map(|pair| (pair[0], pair[1])));
            }
            egui::Shape::LineSegment { points, stroke } if stroke.color == outline_color => {
                outline_segments.push((points[0], points[1]));
            }
            _ => {}
        }
    }
    assert!(
        !outline_segments.is_empty(),
        "the plan must paint wall outlines"
    );

    let rect = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
    let face = app.screen(rect, Point::new(0., 0.1));
    let near_face_left = app.screen(rect, Point::new(1.9, 0.1));
    let near_face_right = app.screen(rect, Point::new(2.1, 0.1));
    assert!(
        !outline_segments.iter().any(|(a, b)| {
            (a.y - face.y).abs() < 0.75
                && (b.y - face.y).abs() < 0.75
                && a.x.min(b.x) < near_face_left.x + 0.75
                && a.x.max(b.x) > near_face_right.x - 0.75
        }),
        "the branch's buried end face must not be stroked between the two wall groups"
    );
}
fn add_wall_for_editing(app: &mut AtelierApp) -> u64 {
    app.editor.project.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: Point::new(0., 0.),
            end: Point::new(4., 0.),
            thickness: 0.3,
            height: 3.,
        },
    )
}
fn wall_reference_line(app: &AtelierApp, id: u64) -> (Point, Point) {
    let wall = app
        .editor
        .project
        .elements
        .iter()
        .find(|element| element.id == id)
        .expect("wall exists");
    match wall.shape {
        Shape::Wall { start, end, .. } => (start, end),
        _ => panic!("expected wall"),
    }
}
#[test]
fn wall_endpoint_handle_drag_stretches_only_that_endpoint_once() {
    let (ctx, mut app) = setup();
    let id = add_wall_for_editing(&mut app);
    let settings = app.editor.project.elements[0]
        .wall_settings
        .clone()
        .expect("wall settings");
    app.editor.set_selection(vec![id]);

    drag(&ctx, &mut app, pos2(400., 300.), pos2(450., 300.));

    assert_eq!(
        wall_reference_line(&app, id),
        (Point::new(1., 0.), Point::new(4., 0.))
    );
    assert_eq!(app.editor.project.elements[0].wall_settings, Some(settings));
    assert_eq!(app.editor.undo.len(), 1, "one drag is one transaction");
    assert_eq!(app.editor.selection_ids(), vec![id]);
    assert!(app.wall_edit.is_none(), "a committed drag closes the edit");

    plan_frame(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
    assert_eq!(
        wall_reference_line(&app, id),
        (Point::new(0., 0.), Point::new(4., 0.))
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
    );
    assert_eq!(
        wall_reference_line(&app, id),
        (Point::new(1., 0.), Point::new(4., 0.))
    );
}
#[test]
fn wall_baseline_drag_inserts_node_on_release() {
    let (ctx, mut app) = setup();
    let id = add_wall_for_editing(&mut app);
    app.editor.set_selection(vec![id]);
    drag(&ctx, &mut app, pos2(500., 300.), pos2(500., 200.));
    assert!(app.pointer_drag.is_none());
    assert!(app.wall_insert.is_none());
    assert_eq!(app.editor.project.elements.len(), 2);
    assert_eq!(app.editor.undo.len(), 1);
    assert_eq!(
        wall_reference_line(&app, id),
        (Point::new(0., 0.), Point::new(2., 2.))
    );
    app.editor.undo();
    assert_eq!(app.editor.project.elements.len(), 1);
    assert_eq!(
        wall_reference_line(&app, id),
        (Point::new(0., 0.), Point::new(4., 0.))
    );
}

#[test]
fn wall_endpoint_drag_snaps_to_another_reference_line_and_joins() {
    let (ctx, mut app) = setup();
    let id = add_wall_for_editing(&mut app);
    let target = app.editor.project.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: Point::new(2., 2.),
            end: Point::new(2., -1.),
            thickness: 0.2,
            height: 3.,
        },
    );
    app.editor.set_selection(vec![id]);

    drag(&ctx, &mut app, pos2(400., 300.), pos2(497., 301.));

    assert_eq!(
        wall_reference_line(&app, id),
        (Point::new(2., 0.), Point::new(4., 0.)),
        "the released endpoint must land exactly on the target reference line"
    );
    assert!(
        app.editor
            .status
            .starts_with("Wall endpoint joined at reference line"),
        "status was {}",
        app.editor.status
    );
    assert!(app.wall_edit.is_none(), "a committed drag closes the edit");
    assert_eq!(app.editor.undo.len(), 1, "one drag is one transaction");

    let walls: Vec<&Element> = app
        .editor
        .project
        .elements
        .iter()
        .filter(|element| element.story == 0 && matches!(element.shape, Shape::Wall { .. }))
        .collect();
    let groups = vec![1u8; walls.len()];
    let footprints = geometry::plan_wall_footprints(&walls, &groups);
    let edited = walls
        .iter()
        .position(|wall| wall.id == id)
        .expect("edited wall");
    let min_x = footprints[edited]
        .iter()
        .map(|corner| corner.x)
        .fold(f32::MAX, f32::min);
    assert!(
        (min_x - 2.1).abs() < 1e-4,
        "the T join must trim the edited wall to the near face of the target, got {min_x}"
    );
    assert!(
        geometry::plan_wall_attached_ends(&walls, &footprints, &groups).contains(&(id, true)),
        "3D must hide the end cap buried in the target wall"
    );
    assert!(
        wall_reference_line(&app, target) == (Point::new(2., 2.), Point::new(2., -1.)),
        "the target wall's stored reference line must not be rewritten by the join"
    );
}

#[test]
fn wall_endpoint_drag_onto_a_foreign_material_wall_body_snaps_and_joins() {
    let (ctx, mut app) = setup();
    app.editor.project.grid_settings.grid_snap_enabled = true;
    app.editor.project.grid_settings.snap_mode = crate::grids::GridSnapMode::SnapGrid;
    let id = add_wall_for_editing(&mut app);
    // The pointer lands 17.5 px from the target reference line but inside its
    // 1 m body. A material or renovation boundary must not block the join.
    let target = app.editor.project.add_with_status(
        0,
        Material::Plaster,
        Shape::Wall {
            start: Point::new(2., 2.),
            end: Point::new(2., -2.),
            thickness: 1.,
            height: 3.,
        },
        crate::view_settings::RenovationStatus::Existing,
    );
    click(&ctx, &mut app, pos2(430., 300.));
    assert_eq!(
        app.editor.selection_ids(),
        vec![id],
        "the drag must start from a normally selected wall"
    );

    drag(&ctx, &mut app, pos2(400., 300.), pos2(517., 294.));

    let (start, end) = wall_reference_line(&app, id);
    assert!(
        (start.x - 2.).abs() < 1e-3 && (end.x - 4.).abs() < 1e-3,
        "the released endpoint must snap onto the foreign wall baseline: {start:?} -> {end:?}"
    );
    assert!(
        (start.y - 0.1).abs() < 1e-3,
        "the snap must keep the pointer's along-wall position: {start:?}"
    );
    assert!(
        app.editor
            .status
            .starts_with("Wall endpoint joined at reference line"),
        "status was {}",
        app.editor.status
    );
    assert_eq!(
        wall_reference_line(&app, target),
        (Point::new(2., 2.), Point::new(2., -2.)),
        "the target wall's stored reference line must not be rewritten"
    );

    let walls: Vec<&Element> = app
        .editor
        .project
        .elements
        .iter()
        .filter(|element| element.story == 0 && matches!(element.shape, Shape::Wall { .. }))
        .collect();
    let groups = vec![1u8; walls.len()];
    let footprints = geometry::plan_wall_footprints(&walls, &groups);
    let edited = walls
        .iter()
        .position(|wall| wall.id == id)
        .expect("edited wall");
    let min_x = footprints[edited]
        .iter()
        .map(|corner| corner.x)
        .fold(f32::MAX, f32::min);
    assert!(
        (min_x - 2.5).abs() < 1e-4,
        "the branch must be cut at the far face of the thick host, got {min_x}"
    );
    assert!(
        geometry::plan_wall_attached_ends(&walls, &footprints, &groups).contains(&(id, true)),
        "3D must hide the end cap buried in the foreign-material host"
    );

    plan_frame(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
    assert_eq!(
        wall_reference_line(&app, id),
        (Point::new(0., 0.), Point::new(4., 0.)),
        "one undo must restore the whole join gesture"
    );
}

#[test]
fn wall_endpoint_reference_snap_wins_over_a_coarse_grid_snap() {
    let (ctx, mut app) = setup();
    let id = add_wall_for_editing(&mut app);
    let target = app.editor.project.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: Point::new(2.4, 2.),
            end: Point::new(2.4, -1.),
            thickness: 0.2,
            height: 3.,
        },
    );
    app.editor.project.grid_settings.grid_snap_enabled = true;
    app.editor.project.grid_settings.snap_mode = crate::grids::GridSnapMode::SnapGrid;
    app.editor.project.grid_settings.snap_spacing_x = 1.;
    app.editor.project.grid_settings.snap_spacing_y = 1.;
    click(&ctx, &mut app, pos2(430., 300.));
    assert_eq!(app.editor.selection_ids(), vec![id]);

    // The pointer is exactly on the target baseline, but ordinary grid snap
    // would first move it to x = 2.0 (0.4 m away). The endpoint must still
    // capture the nearby reference line and join.
    drag(&ctx, &mut app, pos2(400., 300.), pos2(520., 300.));

    let (start, end) = wall_reference_line(&app, id);
    assert!(
        (start.x - 2.4).abs() < 1e-3 && (end.x - 4.).abs() < 1e-3,
        "the endpoint must snap to the target baseline instead of the coarse grid: {start:?} -> {end:?}"
    );
    assert!((start.y - 0.).abs() < 1e-3);
    assert!(
        app.editor
            .status
            .starts_with("Wall endpoint joined at reference line"),
        "status was {}",
        app.editor.status
    );
    assert_eq!(
        wall_reference_line(&app, target),
        (Point::new(2.4, 2.), Point::new(2.4, -1.)),
        "the target wall's reference line must stay unchanged"
    );
}

#[test]
fn wall_endpoint_release_inside_snap_radius_joins_without_pointer_move() {
    let (ctx, mut app) = setup();
    app.editor.project.grid_settings.grid_snap_enabled = true;
    app.editor.project.grid_settings.snap_mode = crate::grids::GridSnapMode::SnapGrid;
    let id = add_wall_for_editing(&mut app);
    app.editor.project.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: Point::new(2., 2.),
            end: Point::new(2., -1.),
            thickness: 0.2,
            height: 3.,
        },
    );
    app.editor.set_selection(vec![id]);
    let from = pos2(400., 300.);
    let to = pos2(497., 301.);
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(from)]);
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: from,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    let outside = pos2(470., 300.);
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(outside)]);
    // Release inside the snap radius without a PointerMoved frame at `to`:
    // the release handler itself must resolve the reference line.
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: to,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );

    assert_eq!(
        wall_reference_line(&app, id),
        (Point::new(2., 0.), Point::new(4., 0.))
    );
    assert_eq!(app.editor.undo.len(), 1);
    assert!(app.wall_edit.is_none());
    assert!(app.editor.status.starts_with("Wall endpoint joined"));
}
#[test]
fn wall_endpoint_snap_preview_matches_the_active_language() {
    let (ctx, mut app) = setup();
    let id = add_wall_for_editing(&mut app);
    let target = app.editor.project.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: Point::new(2., 2.),
            end: Point::new(2., -1.),
            thickness: 0.2,
            height: 3.,
        },
    );
    app.editor.set_selection(vec![id]);
    let from = pos2(400., 300.);
    let to = pos2(497., 301.);
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(from)]);
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: from,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    let output = plan_frame_output(&ctx, &mut app, vec![Event::PointerMoved(to)]);

    let edit = app.wall_edit.as_ref().expect("endpoint edit");
    let snap = edit.snap.expect("reference-line snap");
    assert_eq!(snap.element_id, target);
    assert!(snap.point.distance(Point::new(2., 0.)) < 1e-4);
    assert_eq!(
        wall_reference_line(&app, id),
        (Point::new(2., 0.), Point::new(4., 0.)),
        "the live preview must use the projected reference-line point"
    );
    let mut english = Vec::new();
    collect_text(&output.shapes, &mut english);
    assert!(
        english
            .iter()
            .any(|text| text == "Snap: wall reference line")
    );

    app.language = UiLanguage::SimplifiedChinese;
    let output = plan_frame_output(&ctx, &mut app, vec![]);
    let mut chinese = Vec::new();
    collect_text(&output.shapes, &mut chinese);
    assert!(chinese.iter().any(|text| text == "捕捉：墙参考线"));

    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: to,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert!(app.wall_edit.is_none());
    assert_eq!(app.status_text(), "墙端点已连接到参考线");
}

#[test]
fn wall_baseline_click_opens_endpoint_or_line_edit_and_escape_closes_it() {
    let (ctx, mut app) = setup();
    let id = add_wall_for_editing(&mut app);
    app.editor.set_selection(vec![id]);

    click(&ctx, &mut app, pos2(400., 300.));
    let edit = app.wall_edit.as_ref().expect("endpoint edit");
    assert_eq!((edit.element_id, edit.node), (id, Some(0)));
    assert_eq!(edit.command, WallEditCommand::MoveNode);
    assert!(
        app.editor
            .status
            .starts_with("Wall reference-line endpoint edit")
    );

    click(&ctx, &mut app, pos2(500., 300.));
    assert!(app.wall_edit.is_none());
    assert_eq!(
        app.wall_insert.as_ref().unwrap().command,
        super::wall_node_ui::WallPetCommand::InsertNode
    );

    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.wall_edit.is_none());
    assert_eq!(app.editor.selection_ids(), vec![id]);
}
#[test]
fn wall_reference_line_palette_renders_in_both_languages() {
    use super::wall_node_ui::WallPetCommand as C;
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        for (node, at) in [(0, pos2(400., 300.)), (1, pos2(600., 300.))] {
            let (ctx, mut app) = setup();
            app.language = language;
            let id = add_wall_for_editing(&mut app);
            app.editor.set_selection(vec![id]);
            let before = app.editor.project.clone();
            click(&ctx, &mut app, at);
            plan_frame(&ctx, &mut app, vec![]);
            let output = plan_frame_output(&ctx, &mut app, vec![]);
            assert_eq!(app.wall_edit.as_ref().unwrap().node, Some(node));
            let palette = app.wall_edit_palette_rect.unwrap();
            assert!(
                palette.width() <= 150. && palette.height() <= 66.,
                "{palette:?}"
            );
            for command in C::ALL {
                let rect = ctx
                    .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", command as u8))))
                    .unwrap();
                assert_eq!(rect.size(), vec2(26., 26.));
                assert!(palette.contains(rect.center()));
            }
            let mut labels = Vec::new();
            collect_text(&output.shapes, &mut labels);
            for absent in [
                "Wall reference line",
                "Start endpoint",
                "End endpoint",
                "Done",
                "Drag onto another reference line to join",
            ] {
                assert!(!labels.iter().any(|s| s == language.translate(absent)));
            }
            assert!(
                !labels
                    .iter()
                    .any(|s| s.starts_with("Length:") || s.starts_with("长度:"))
            );
            assert_eq!(app.editor.project, before);
            assert!(app.editor.undo.is_empty());
            plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
            click(&ctx, &mut app, pos2(500., 300.));
            plan_frame(&ctx, &mut app, vec![]);
            plan_frame(&ctx, &mut app, vec![]);
            assert_eq!(
                app.wall_edit_palette_rect.unwrap().size(),
                palette.size(),
                "endpoint and insert node must share palette layout"
            );
        }
    }
}
#[test]
fn wall_palette_keeps_the_edit_when_the_canvas_is_under_the_popup() {
    let (ctx, mut app) = setup();
    let id = add_wall_for_editing(&mut app);
    app.editor.set_selection(vec![id]);
    click(&ctx, &mut app, pos2(400., 300.));
    assert!(app.wall_edit_palette_rect.is_some());
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(700., 500.))]);
    let palette = app
        .wall_edit_palette_rect
        .expect("palette bounds settle on the following frame");

    let palette_click = palette.left_top() + vec2(4., 4.);
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(palette_click)]);
    assert!(
        app.wall_edit.is_some(),
        "hovering the palette keeps the edit"
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: palette_click,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    assert!(
        app.wall_edit.is_some(),
        "pressing the palette keeps the edit"
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: palette_click,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );

    assert!(app.wall_edit.is_some());
    assert_eq!(
        wall_reference_line(&app, id),
        (Point::new(0., 0.), Point::new(4., 0.))
    );
    assert!(app.editor.undo.is_empty());

    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(
        app.wall_edit.is_none(),
        "Escape dismisses the unchanged icon palette"
    );
    assert!(app.editor.undo.is_empty());
    assert_eq!(app.editor.selection_ids(), vec![id]);
}
#[test]
fn escape_cancels_a_live_wall_baseline_drag_without_undo() {
    let (ctx, mut app) = setup();
    let id = add_wall_for_editing(&mut app);
    app.editor.set_selection(vec![id]);

    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(400., 300.))]);
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: pos2(400., 300.),
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(450., 300.))]);
    assert_eq!(
        wall_reference_line(&app, id),
        (Point::new(1., 0.), Point::new(4., 0.))
    );
    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: pos2(450., 300.),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );

    assert_eq!(
        wall_reference_line(&app, id),
        (Point::new(0., 0.), Point::new(4., 0.))
    );
    assert!(app.editor.undo.is_empty());
    assert!(app.wall_edit.is_none());
}
#[test]
fn wall_baseline_edit_refuses_to_strand_a_hosted_opening() {
    let (ctx, mut app) = setup();
    let id = add_wall_for_editing(&mut app);
    app.editor.project.add(
        0,
        Material::Timber,
        Shape::Opening {
            host_wall: id,
            kind: OpeningKind::Door,
            width: 1.,
            height: 2.1,
            sill: 0.,
            offset: 3.5,
        },
    );
    app.editor.set_selection(vec![id]);

    drag(&ctx, &mut app, pos2(600., 300.), pos2(500., 300.));

    assert_eq!(
        wall_reference_line(&app, id),
        (Point::new(0., 0.), Point::new(4., 0.))
    );
    assert!(app.editor.undo.is_empty());
    let edit = app.wall_edit.as_ref().expect("edit remains for feedback");
    assert!(edit.error.contains("outside its host wall"));
}
#[test]
fn pointer_wall_creation_selection_drag_undo_redo() {
    let (ctx, mut app) = setup();
    plan_frame(&ctx, &mut app, vec![key(Key::W, Modifiers::NONE)]);
    click(&ctx, &mut app, pos2(400., 300.));
    assert_eq!(app.editor.anchor, Some(Point::new(0., 0.)));
    click(&ctx, &mut app, pos2(600., 300.));
    assert_eq!(app.editor.project.elements.len(), 1);
    assert_eq!(app.editor.project.elements[0].area(), 12.);
    plan_frame(&ctx, &mut app, vec![key(Key::V, Modifiers::NONE)]);
    click(&ctx, &mut app, pos2(500., 300.));
    assert_eq!(app.editor.selected, Some(1));
    assert!(
        app.wall_insert.is_some(),
        "the wall was already selected after drawing"
    );
    plan_frame(&ctx, &mut app, vec![]);
    let drag_button = ctx
        .data(|d| {
            d.get_temp::<Rect>(Id::new((
                "wall_pet",
                super::wall_node_ui::WallPetCommand::Drag as u8,
            )))
        })
        .unwrap();
    click(&ctx, &mut app, drag_button.center());
    plan_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(pos2(500., 300.)),
            Event::PointerButton {
                pos: pos2(500., 300.),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(550., 250.))]);
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: pos2(550., 250.),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert!(app.wall_insert.is_none());
    let wall = &app.editor.project.elements[0];
    assert!(matches!(wall.shape,Shape::Wall{start,..} if start==Point::new(1.,1.)));
    assert_eq!(app.editor.undo.len(), 2, "one drag is one transaction");
    plan_frame(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
    assert!(
        matches!(app.editor.project.elements[0].shape,Shape::Wall{start,..} if start==Point::default())
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
    );
    assert!(
        matches!(app.editor.project.elements[0].shape,Shape::Wall{start,..} if start==Point::new(1.,1.))
    );
}
#[test]
fn pointer_slab_creation_and_cancel() {
    let (ctx, mut app) = setup();
    plan_frame(&ctx, &mut app, vec![key(Key::L, Modifiers::NONE)]);
    click(&ctx, &mut app, pos2(400., 300.));
    click(&ctx, &mut app, pos2(600., 150.));
    assert_eq!(app.editor.project.elements[0].area(), 12.);
    click(&ctx, &mut app, pos2(350., 350.));
    assert!(app.editor.anchor.is_some());
    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.editor.anchor.is_none());
    assert_eq!(app.editor.project.elements.len(), 1);
}
#[test]
fn pointer_polyline_draw_commit_json_undo_and_redo() {
    let (ctx, mut app) = setup();
    plan_frame(&ctx, &mut app, vec![key(Key::P, Modifiers::NONE)]);
    assert_eq!(app.editor.tool, Tool::Polyline);
    click(&ctx, &mut app, pos2(360., 280.));
    click(&ctx, &mut app, pos2(470., 250.));
    click(&ctx, &mut app, pos2(580., 320.));
    assert_eq!(
        app.editor.project.elements.len(),
        0,
        "nodes stay a preview until commit"
    );
    assert_eq!(app.editor.polyline_points.len(), 3);
    plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
    assert_eq!(app.editor.project.elements.len(), 1);
    assert_eq!(app.editor.selected, Some(1));
    let Shape::Polyline { points, .. } = &app.editor.project.elements[0].shape else {
        panic!("Polyline tool created a different shape");
    };
    assert_eq!(points.len(), 3);
    assert!(
        points
            .windows(2)
            .all(|pair| pair[0].distance(pair[1]) >= 0.01)
    );
    assert!(app.editor.project.validate().is_ok());
    let json = serde_json::to_string(&app.editor.project).unwrap();
    assert_eq!(
        serde_json::from_str::<Project>(&json).unwrap(),
        app.editor.project
    );
    app.editor.undo();
    assert!(app.editor.project.elements.is_empty());
    app.editor.redo();
    assert!(
        matches!(app.editor.project.elements[0].shape, Shape::Polyline { ref points , .. } if points.len() == 3)
    );
}
#[test]
fn pointer_polyline_escape_discards_draft_without_model_change() {
    let (ctx, mut app) = setup();
    let before = app.editor.project.clone();
    plan_frame(&ctx, &mut app, vec![key(Key::P, Modifiers::NONE)]);
    click(&ctx, &mut app, pos2(360., 280.));
    assert_eq!(app.editor.polyline_points.len(), 1);
    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert_eq!(app.editor.tool, Tool::Select);
    assert!(app.editor.polyline_points.is_empty());
    assert_eq!(app.editor.project, before);
}
#[test]
fn escape_cancels_current_command_returns_to_previous_then_select() {
    let (ctx, mut app) = setup();
    plan_frame(&ctx, &mut app, vec![key(Key::W, Modifiers::NONE)]);
    plan_frame(&ctx, &mut app, vec![key(Key::P, Modifiers::NONE)]);
    app.editor.polyline_points = vec![Point::new(1., 1.), Point::new(2., 1.)];

    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert_eq!(
        app.editor.tool,
        Tool::Wall,
        "first Escape restores the previous command"
    );
    assert!(
        app.editor.polyline_points.is_empty(),
        "first Escape cancels the active drawing"
    );

    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert_eq!(
        app.editor.tool,
        Tool::Select,
        "second Escape returns to Select"
    );
}
#[test]
fn pointer_stair_creation_persists_parametric_settings_and_undoes() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Stair);
    app.editor.stair_spec.width = 1.25;
    app.editor.stair_spec.total_rise = 3.4;
    app.editor.stair_spec.risers = 20;
    click(&ctx, &mut app, pos2(400., 300.));
    click(&ctx, &mut app, pos2(640., 300.));
    assert_eq!(app.editor.project.elements.len(), 1);
    let element = &app.editor.project.elements[0];
    let Shape::Stair { spec } = &element.shape else {
        panic!("stair tool created another kind")
    };
    assert_eq!(spec.width, 1.25);
    assert_eq!(spec.risers, 20);
    let geometry =
        crate::stairs::generate(spec, app.editor.project.elevation(element.story)).unwrap();
    assert_eq!(geometry.tread_lines.len(), 20);
    assert!(app.editor.project.validate().is_ok());
    let json = serde_json::to_string(&app.editor.project).unwrap();
    let restored: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, app.editor.project);
    app.editor.undo();
    assert!(app.editor.project.elements.is_empty());
    app.editor.redo();
    assert_eq!(app.editor.project.elements.len(), 1);
    let id = app.editor.project.elements[0].id;
    app.editor.set_selection(vec![id]);
    app.open_element_settings();
    if let Some(Element {
        shape: Shape::Stair { spec },
        ..
    }) = app.element_settings.as_mut()
    {
        spec.width = 1.4;
    }
    app.apply_element_settings().unwrap();
    assert!(
        matches!(app.editor.project.elements[0].shape, Shape::Stair { ref spec } if spec.width == 1.4)
    );
    app.editor.undo();
    assert!(
        matches!(app.editor.project.elements[0].shape, Shape::Stair { ref spec } if spec.width == 1.25)
    );
}
#[test]
fn command_t_opens_selected_wall_settings_or_wall_defaults() {
    let (_ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.editor.click(Point::new(0., 0.), 0.);
    app.editor.click(Point::new(3., 0.), 0.);
    let id = app.editor.project.elements[0].id;
    app.editor.set_selection(vec![id]);
    app.open_element_settings();
    assert!(app.element_settings.is_none());
    let selected = app.wall_settings_dialog.as_ref().unwrap();
    assert_eq!(
        selected.mode,
        crate::wall_ui::WallSettingsMode::Selected(id)
    );
    app.wall_settings_dialog = None;
    app.editor.set_selection(Vec::new());
    app.editor.set_tool(Tool::Wall);
    app.open_element_settings();
    assert_eq!(
        app.wall_settings_dialog.as_ref().unwrap().mode,
        crate::wall_ui::WallSettingsMode::Default
    );
}
#[test]
fn wall_settings_dialog_renders_without_mutating_the_project() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.editor.click(Point::new(0., 0.), 0.);
    app.editor.click(Point::new(3., 0.), 0.);
    let id = app.editor.project.elements[0].id;
    app.editor.set_selection(vec![id]);
    app.open_element_settings();
    let before = app.editor.project.clone();
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(900., 700.))),
            ..Default::default()
        },
        |ui| app.wall_settings_dialog(ui.ctx()),
    );
    output.textures_delta.clear();
    assert!(app.wall_settings_dialog.is_some());
    assert_eq!(app.editor.project, before);
}
#[test]
fn stair_settings_dialog_renders_without_mutating_the_project() {
    let (ctx, mut app) = setup();
    app.stair_settings_open = true;
    let before = app.editor.project.clone();
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
            ..Default::default()
        },
        |ui| app.stair_settings_dialog(ui.ctx()),
    );
    output.textures_delta.clear();
    assert!(app.stair_settings_open);
    assert_eq!(app.editor.project, before);
}
#[test]
fn stair_shortcut_opens_settings_and_activates_the_real_tool() {
    let (ctx, mut app) = setup();
    let before = app.editor.project.clone();
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
            events: vec![key(Key::S, Modifiers::NONE)],
            ..Default::default()
        },
        |ui| app.shortcuts(ui.ctx()),
    );
    output.textures_delta.clear();
    assert_eq!(app.editor.tool, Tool::Stair);
    assert!(app.stair_settings_open);
    assert_eq!(app.editor.project, before);
}
#[test]
fn marquee_rectangle_is_created_by_plan_canvas_drag() {
    let (ctx, mut app) = setup();
    plan_frame(&ctx, &mut app, vec![key(Key::M, Modifiers::NONE)]);
    let a = pos2(340., 260.);
    let b = pos2(460., 340.);
    plan_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(a),
            Event::PointerButton {
                pos: a,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(b)]);
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: b,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    let area = app.editor.marquee.as_ref().expect("drag places a marquee");
    assert_eq!(area.vertices.len(), 4);
    assert!(area.z_range.is_none());
}
#[test]
fn polygon_marquee_uses_right_click_to_close_and_enter_does_not_close() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Marquee);
    app.marquee_geometry = MarqueeGeometry::Polygon;
    click(&ctx, &mut app, pos2(330., 270.));
    click(&ctx, &mut app, pos2(470., 270.));
    click(&ctx, &mut app, pos2(440., 370.));
    assert_eq!(
        app.marquee_points.len(),
        3,
        "points before Enter: {:?}",
        app.marquee_points
    );
    plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
    assert!(
        app.editor.marquee.is_none(),
        "Enter is reserved for numeric tracker input"
    );
    let pos = pos2(440., 370.);
    plan_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Secondary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos,
            button: PointerButton::Secondary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert_eq!(
        app.editor.marquee.as_ref().map(|area| area.vertices.len()),
        Some(3),
        "right-click close failed: points={:?}, status={:?}",
        app.marquee_points,
        app.editor.status
    );
}
#[test]
fn three_dimensional_polygon_marquee_previews_and_closes_on_right_click() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Marquee);
    app.marquee_geometry = MarqueeGeometry::Polygon;
    let points = [pos2(250., 450.), pos2(430., 450.), pos2(410., 500.)];
    click_model(&ctx, &mut app, points[0], PointerButton::Primary);
    assert_eq!(
        app.marquee_3d_points.len(),
        1,
        "first screen point not registered: {:?}",
        app.editor.status
    );
    click_model(&ctx, &mut app, points[1], PointerButton::Primary);
    model_frame(&ctx, &mut app, vec![Event::PointerMoved(points[2])]);
    assert_eq!(
        app.marquee_3d_points.len(),
        2,
        "ground points={:?}, status={:?}",
        app.marquee_points,
        app.editor.status
    );
    assert!(
        app.marquee_3d_cursor.is_some(),
        "the open polygon has a live cursor segment"
    );
    click_model(&ctx, &mut app, points[2], PointerButton::Primary);
    assert!(
        app.editor.marquee.is_none(),
        "three vertices only define the base until right-click"
    );
    click_model(&ctx, &mut app, points[2], PointerButton::Secondary);
    let area = app
        .editor
        .marquee
        .as_ref()
        .expect("right click closes the 3D polygon");
    assert_eq!(area.vertices.len(), 3);
    assert_eq!(area.z_range, Some((0., app.marquee_height)));
    assert!(app.show_marquee_3d);
    assert!(app.marquee_3d_points.is_empty());
}
#[test]
fn three_dimensional_rotated_rectangle_uses_three_ground_plane_points() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Marquee);
    app.marquee_geometry = MarqueeGeometry::RotatedRectangle;
    for pos in [pos2(250., 440.), pos2(430., 430.), pos2(420., 500.)] {
        click_model(&ctx, &mut app, pos, PointerButton::Primary);
    }
    let area = app.editor.marquee.as_ref().unwrap_or_else(|| {
        panic!(
            "third point failed: points={:?}, status={:?}",
            app.marquee_points, app.editor.status
        )
    });
    assert_eq!(area.vertices.len(), 4);
    assert_eq!(area.z_range, Some((0., app.marquee_height)));
    assert!(app.show_marquee_3d);
}
#[test]
fn default_is_single_plan_window_and_f_keys_switch_3d_workflow() {
    let (ctx, mut app) = setup();
    assert_eq!(app.view, View::Plan);
    let selected_id = app.editor.project.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: Point::new(0., 0.),
            end: Point::new(3., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    app.editor.set_selection(vec![selected_id]);
    app.editor.marquee = MarqueeArea::new(
        vec![
            Point::new(0., 0.),
            Point::new(2., 0.),
            Point::new(2., 1.),
            Point::new(0., 1.),
        ],
        MarqueeScope::CurrentStory,
        0,
    );
    plan_frame(&ctx, &mut app, vec![key(Key::F5, Modifiers::NONE)]);
    assert_eq!(app.view, View::Model);
    assert!(app.show_marquee_3d);
    assert!(app.editor.status.contains("clipped Marquee"));
    assert_eq!(app.editor.selection_ids(), vec![selected_id]);
    plan_frame(&ctx, &mut app, vec![key(Key::F2, Modifiers::NONE)]);
    assert_eq!(app.view, View::Plan);
    app.editor.marquee = None;
    plan_frame(&ctx, &mut app, vec![key(Key::F5, Modifiers::NONE)]);
    assert!(app.editor.status.contains("selected elements"));
    plan_frame(&ctx, &mut app, vec![key(Key::F3, Modifiers::NONE)]);
    assert_eq!(app.view, View::Model);
}
#[test]
fn command_t_opens_independent_element_settings_and_applies_edits() {
    let (ctx, mut app) = setup();
    let id = app.editor.project.add(
        0,
        Material::Concrete,
        Shape::Slab {
            min: Point::new(0., 0.),
            max: Point::new(4., 3.),
            thickness: 0.2,
        },
    );
    app.editor.set_selection(vec![id]);
    app.tab = Tab::Library;
    app.show_right = false;
    plan_frame(&ctx, &mut app, vec![key(Key::T, Modifiers::COMMAND)]);
    assert!(matches!(app.tab, Tab::Library));
    assert!(
        !app.show_right,
        "the settings dialog is independent of the inspector"
    );
    assert_eq!(
        app.element_settings.as_ref().map(|element| element.id),
        Some(id)
    );

    let mut output = ctx.run_ui(RawInput::default(), |ui| {
        app.element_settings_dialog(ui.ctx());
    });
    output.textures_delta.clear();
    if let Some(Shape::Slab { thickness, .. }) = app
        .element_settings
        .as_mut()
        .map(|element| &mut element.shape)
    {
        *thickness = 0.35;
    }
    app.apply_element_settings().unwrap();
    assert!(
        matches!(app.editor.project.elements[0].shape, Shape::Slab { thickness, .. } if thickness == 0.35)
    );
    app.editor.undo();
    assert!(
        matches!(app.editor.project.elements[0].shape, Shape::Slab { thickness, .. } if thickness == 0.2)
    );
}
#[test]
fn shift_middle_drag_orbits_3d_while_plain_middle_drag_pans() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Marquee);
    let yaw = app.camera.yaw;
    let pitch = app.camera.pitch;
    let target = app.camera.target;
    let start = pos2(400., 300.);
    let end = pos2(450., 325.);
    let shift = Modifiers {
        shift: true,
        ..Modifiers::NONE
    };
    model_frame(&ctx, &mut app, vec![Event::PointerMoved(start)]);
    model_frame(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(shift),
            Event::PointerButton {
                pos: start,
                button: PointerButton::Middle,
                pressed: true,
                modifiers: shift,
            },
        ],
    );
    model_frame(
        &ctx,
        &mut app,
        vec![Event::ModifiersChanged(shift), Event::PointerMoved(end)],
    );
    assert!(ctx.input(|i| i.pointer.button_down(PointerButton::Middle)));
    assert_ne!(ctx.input(|i| i.pointer.delta()), Vec2::ZERO);
    model_frame(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(shift),
            Event::PointerButton {
                pos: end,
                button: PointerButton::Middle,
                pressed: false,
                modifiers: shift,
            },
            Event::ModifiersChanged(Modifiers::NONE),
        ],
    );
    assert_ne!(app.camera.yaw, yaw);
    assert_ne!(app.camera.pitch, pitch);
    assert_eq!(app.camera.target, target);

    let yaw = app.camera.yaw;
    let target = app.camera.target;
    let pan_start = pos2(300., 200.);
    let pan_end = pos2(360., 230.);
    model_frame(&ctx, &mut app, vec![Event::PointerMoved(pan_start)]);
    model_frame(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(Modifiers::NONE),
            Event::PointerButton {
                pos: pan_start,
                button: PointerButton::Middle,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    model_frame(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(Modifiers::NONE),
            Event::PointerMoved(pan_end),
        ],
    );
    model_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: pan_end,
            button: PointerButton::Middle,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert_eq!(app.camera.yaw, yaw);
    assert_ne!(app.camera.target, target);
}
#[test]
fn tracker_enter_creates_exact_wall() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.editor.anchor = Some(Point::new(1., 2.));
    app.tracker_a = 5.25;
    app.tracker_b = 90.;
    app.tracker_locked = true;
    let mut output = ctx.run_ui(
        RawInput {
            events: vec![key(Key::Enter, Modifiers::NONE)],
            ..Default::default()
        },
        |ui| app.tracker(ui),
    );
    output.textures_delta.clear();
    assert_eq!(app.editor.project.elements.len(), 1);
    if let Shape::Wall { start, end, .. } = app.editor.project.elements[0].shape {
        assert!((start.distance(end) - 5.25).abs() < 1e-5);
        assert!((end.y - 7.25).abs() < 1e-5);
    } else {
        panic!("Expected wall")
    }
}
#[test]
fn tracker_labels_follow_language() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.editor.anchor = Some(Point::new(1., 2.));
    for (language, expected) in [
        (
            UiLanguage::English,
            ["Length", "Angle", "Create [Enter]"].as_slice(),
        ),
        (
            UiLanguage::SimplifiedChinese,
            ["长度", "角度", "创建 [回车]"].as_slice(),
        ),
    ] {
        app.language = language;
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
                ..Default::default()
            },
            |ui| app.tracker(ui),
        );
        output.textures_delta.clear();
        let mut texts = Vec::new();
        collect_text(&output.shapes, &mut texts);
        for label in expected {
            assert!(
                texts.iter().any(|text| text == label),
                "{language:?} tracker is missing {label:?}: {texts:?}"
            );
        }
    }
}
#[test]
fn workspace_all_views_and_tabs_render_at_minimum_window() {
    let (ctx, mut app) = setup();
    for view in [View::Plan, View::Split, View::Model] {
        for tab in [Tab::Properties, Tab::Library, Tab::Assistant] {
            for navigator_view in [
                NavigatorView::ProjectMap,
                NavigatorView::ViewMap,
                NavigatorView::LayoutBook,
                NavigatorView::Publisher,
            ] {
                app.view = view;
                app.tab = tab;
                app.navigator_view = navigator_view;
                let mut out = ctx.run_ui(
                    RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1050., 700.))),
                        ..Default::default()
                    },
                    |ui| app.workspace(ui),
                );
                out.textures_delta.clear();
                assert!(!out.shapes.is_empty());
            }
        }
    }
}
#[test]
fn split_modules_render_through_workspace_shell() {
    let (ctx, mut app) = setup();
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
            ..Default::default()
        },
        |ui| app.workspace(ui),
    );
    let mut text = Vec::new();
    collect_text(&output.shapes, &mut text);
    output.textures_delta.clear();
    assert!(!output.shapes.is_empty());
    for expected in [
        "OpenArchi",
        "File",
        "INFO BOX",
        "03 Plans - Prelimi",
        "1:100",
    ] {
        assert!(
            text.iter().any(|value| value.contains(expected)),
            "workspace shell, inspector or Quick Options did not render {expected}: {text:?}"
        );
    }
}

#[test]
fn workspace_does_not_repeat_the_native_window_title_or_language_picker() {
    let (ctx, mut app) = setup();
    app.language = UiLanguage::English;
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
            ..Default::default()
        },
        |ui| app.workspace(ui),
    );
    let mut text = Vec::new();
    collect_text(&output.shapes, &mut text);
    output.textures_delta.clear();

    assert!(text.iter().any(|value| value == "File"));
    assert!(!text.iter().any(|value| value == "Untitled"));
    assert!(!text.iter().any(|value| value == "English"));
    assert!(
        !text
            .iter()
            .any(|value| value.contains("ARCHITECTURAL DESIGN"))
    );
}

#[test]
fn simplified_chinese_workspace_renders_without_changing_model_state() {
    let (ctx, mut app) = setup();
    app.language = UiLanguage::SimplifiedChinese;
    let before = app.editor.project.elements.clone();
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1050., 700.))),
            ..Default::default()
        },
        |ui| app.workspace(ui),
    );
    assert!(!output.shapes.is_empty());
    output.textures_delta.clear();
    let chinese_glyphs_present = ctx.fonts_mut(|fonts| {
        let font = FontId::proportional(14.);
        fonts.has_glyphs(&font, "墙体楼层项目")
    });
    assert!(
        chinese_glyphs_present,
        "the UI font fallback must contain Simplified Chinese glyphs"
    );
    assert_eq!(app.editor.project.elements, before);
}
#[test]
fn quick_options_bar_and_palette_render_in_the_selected_language() {
    let (ctx, mut app) = setup();
    app.quick_options_palette_visible = true;
    let before = app.editor.project.clone();

    app.language = UiLanguage::English;
    let mut first = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
            ..Default::default()
        },
        |ui| app.workspace(ui),
    );
    first.textures_delta.clear();
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
            ..Default::default()
        },
        |ui| app.workspace(ui),
    );
    let mut english = Vec::new();
    collect_text(&output.shapes, &mut english);
    output.textures_delta.clear();
    assert!(
        english
            .iter()
            .any(|text| text.contains("Layer Combination")),
        "English palette labels missing: {english:?}"
    );
    assert!(
        english
            .iter()
            .any(|text| text.contains("03 Plans - Preliminary"))
    );

    app.language = UiLanguage::SimplifiedChinese;
    let mut first = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
            ..Default::default()
        },
        |ui| app.workspace(ui),
    );
    first.textures_delta.clear();
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
            ..Default::default()
        },
        |ui| app.workspace(ui),
    );
    let mut chinese = Vec::new();
    collect_text(&output.shapes, &mut chinese);
    output.textures_delta.clear();
    assert!(
        chinese.iter().any(|text| text.contains("图层组合")),
        "Simplified Chinese palette labels missing: {chinese:?}"
    );
    assert!(chinese.iter().any(|text| text.contains("03 平面 - 初步")));
    assert!(
        !chinese
            .iter()
            .any(|text| text.contains("Layer Combination")),
        "English Quick Options labels remain after switching to Chinese: {chinese:?}"
    );
    assert_eq!(app.editor.project, before);
}
#[test]
fn main_window_status_tabs_and_counts_follow_the_selected_language() {
    let (ctx, mut app) = setup();
    app.editor.status = "Selected 2 element(s) in Marquee".into();
    let before = app.editor.project.clone();

    app.language = UiLanguage::SimplifiedChinese;
    for pass in 0..2 {
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
                ..Default::default()
            },
            |ui| app.workspace(ui),
        );
        output.textures_delta.clear();
        if pass == 1 {
            let mut chinese = Vec::new();
            collect_text(&output.shapes, &mut chinese);
            for expected in [
                "已选 0 个",
                "0 个构件",
                "分屏",
                "南立面",
                "在选区框中选择 2 个构件",
            ] {
                assert!(
                    chinese.iter().any(|text| text.contains(expected)),
                    "missing Chinese main-window label {expected:?}: {chinese:?}"
                );
            }
            for english in [
                "0 selected",
                "Split",
                "South Elevation",
                "Metres · Z up",
                "Selected 2 element(s) in Marquee",
            ] {
                assert!(
                    !chinese.iter().any(|text| text.contains(english)),
                    "English main-window text {english:?} remains in Chinese mode: {chinese:?}"
                );
            }
        }
    }

    app.language = UiLanguage::English;
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
            ..Default::default()
        },
        |ui| app.workspace(ui),
    );
    let mut english = Vec::new();
    collect_text(&output.shapes, &mut english);
    output.textures_delta.clear();
    for expected in [
        "0 selected",
        "0 elements",
        "Split",
        "South Elevation",
        "Selected 2 element(s) in Marquee",
    ] {
        assert!(
            english.iter().any(|text| text.contains(expected)),
            "missing English main-window label {expected:?}: {english:?}"
        );
    }
    assert_eq!(
        app.editor.status, "Selected 2 element(s) in Marquee",
        "switching language must not rewrite model/editor state"
    );
    assert_eq!(app.editor.project, before);
}
#[test]
fn status_text_localizes_dynamic_messages_without_rewriting_editor_state() {
    let (_ctx, mut app) = setup();
    app.language = UiLanguage::SimplifiedChinese;
    for (status, expected) in [
        (
            "Selected 2 element(s) in Marquee",
            "在选区框中选择 2 个构件",
        ),
        ("Copied 3 element(s)", "已复制 3 个构件"),
        ("Cut 1 element(s)", "已剪切 1 个构件"),
        ("Pasted 4 element(s)", "已粘贴 4 个构件"),
        ("Created element 013", "已创建构件 013"),
        ("Element Settings · Wall 007", "构件设置 · 墙 007"),
        ("Project saved", "项目已保存"),
        ("Navigator item · Ground Floor", "导航器项目 · 首层"),
        ("View Map · South Elevation", "视图地图 · 南立面"),
        (
            "Wall tool selected · interface preview",
            "墙 · 工具已选择 · 界面预览",
        ),
    ] {
        app.editor.status = status.into();
        assert_eq!(app.status_text(), expected, "status {status:?}");
        assert_eq!(
            app.editor.status, status,
            "editor state must stay canonical"
        );
    }
}
#[test]
fn command_preview_dialog_renders_without_mutating_the_project() {
    let (ctx, mut app) = setup();
    let initial = app.editor.project.clone();
    app.command_dialog = Some("Wall Settings".into());
    app.command_dialog_name = "Wall Settings".into();
    for tab in 0..4 {
        app.command_dialog_tab = tab;
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1100., 760.))),
                ..Default::default()
            },
            |ui| app.workspace(ui),
        );
        output.textures_delta.clear();
        assert!(!output.shapes.is_empty());
    }
    assert_eq!(app.editor.project, initial);
}
#[test]
fn trace_toggle_opens_plan_and_resolves_an_available_story() {
    let (_ctx, mut app) = setup();
    app.view = View::Model;
    app.trace.enabled = true;
    app.trace_toggled();
    assert_eq!(app.view, View::Plan);
    assert_eq!(app.trace.reference, crate::trace::TraceReference::Story(1));
    assert_eq!(
        app.trace.selection(
            &app.editor.project,
            app.editor.story,
            app.editor.previous_story
        ),
        Some(crate::trace::TraceSelection::Story(1))
    );
}
#[test]
fn trace_switch_reference_preserves_model_and_undo_history() {
    let (_ctx, mut app) = setup();
    app.editor.switch_story(1);
    app.trace.enabled = true;
    app.trace.reference = crate::trace::TraceReference::AboveCurrentStory;
    let initial = app.editor.project.clone();
    let undo_len = app.editor.undo.len();
    app.switch_trace_reference();
    assert_eq!(app.editor.story, 2);
    assert_eq!(app.trace.reference, crate::trace::TraceReference::Story(1));
    assert_eq!(app.editor.project, initial);
    assert_eq!(app.editor.undo.len(), undo_len);
    assert!(!app.editor.dirty());
}
#[test]
fn trace_drag_moves_only_the_reference() {
    let (ctx, mut app) = setup();
    app.editor.switch_story(1);
    app.trace.enabled = true;
    app.trace.reference = crate::trace::TraceReference::BelowCurrentStory;
    let before = app.editor.project.clone();
    let offset = app.trace.offset;
    app.trace.drag_mode = true;
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(400., 300.))]);
    plan_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(pos2(400., 300.)),
            Event::PointerButton {
                pos: pos2(400., 300.),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(450., 320.))]);
    assert_ne!(app.trace.offset, offset);
    assert!((app.trace.offset.x - 1.).abs() < 1e-5);
    assert_eq!(app.editor.project, before);
    assert!(app.editor.undo.is_empty());
    assert!(!app.editor.dirty());
}
#[test]
fn escape_cancels_trace_pointer_mode_and_restores_temporary_displacement() {
    let (ctx, mut app) = setup();
    app.trace.enabled = true;
    app.trace.displace_mode = true;
    app.trace.displace_origin = Some(Point::new(1., 2.));
    app.trace.offset = Point::new(5., 6.);
    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(!app.trace.pointer_mode_active());
    assert_eq!(app.trace.offset, Point::new(1., 2.));
}
#[test]
fn trace_overlay_palette_and_splitter_render_without_mutating_the_model() {
    let (ctx, mut app) = setup();
    app.editor.switch_story(1);
    app.trace.enabled = true;
    app.trace.palette_open = true;
    app.trace.browse_open = true;
    app.trace.splitter = Some(0.5);
    app.trace.on_top = true;
    let initial = app.editor.project.clone();
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1100., 760.))),
            ..Default::default()
        },
        |ui| app.workspace(ui),
    );
    output.textures_delta.clear();
    assert!(!output.shapes.is_empty());
    assert_eq!(app.editor.project, initial);
}
fn quick_option_element(layer: crate::layers::ElementLayer, design_option: Option<u32>) -> Element {
    Element {
        id: 1,
        name: "Wall 001".into(),
        story: 0,
        material: Material::Concrete,
        layer,
        renovation: RenovationStatus::Existing,
        design_option,
        wall_contour: None,
        slab_contour: None,
        wall_settings: None,
        shape: Shape::Wall {
            start: Point::new(0., 0.),
            end: Point::new(4., 0.),
            thickness: 0.2,
            height: 3.,
        },
    }
}

#[test]
fn quick_options_edit_plan_options_through_undoable_commands() {
    let (_ctx, mut app) = setup();
    assert_eq!(
        app.editor.project.plan_options.pen_set,
        PenSet::Architectural100
    );
    assert_eq!(
        app.editor.project.plan_options.partial_structure,
        PartialStructureDisplay::EntireModel
    );

    // Applying a layer combination is one reversible layer-document change.
    let detailed = app
        .editor
        .project
        .layers
        .combinations
        .iter()
        .find(|c| c.name == "05 Plans - Detailed")
        .unwrap()
        .id;
    app.apply_layer_combination(detailed);
    app.set_pen_set(PenSet::ColorConceptual);
    app.set_partial_structure(PartialStructureDisplay::WithoutFinishes);
    app.set_model_view(ModelViewOptions::CeilingPlans);
    app.set_graphic_overrides(GraphicOverrides::FirePlan);
    app.set_renovation_filter(RenovationFilter::NewConstruction);
    app.set_dimension_style(DimensionStyle::PlainMillimeter);
    assert_eq!(
        app.editor.project.plan_options.pen_set,
        PenSet::ColorConceptual
    );
    assert_eq!(
        app.editor.project.plan_options.partial_structure,
        PartialStructureDisplay::WithoutFinishes
    );
    assert_eq!(app.editor.undo.len(), 7);

    for _ in 0..7 {
        app.editor.undo();
    }
    assert_eq!(app.editor.project.plan_options, PlanOptions::default());
    assert_eq!(
        app.editor.project.layers,
        crate::layers::LayerDocument::template(),
        "undo restores the layer document"
    );
}

#[test]
fn quick_option_visibility_follows_layers_and_design_options() {
    let (_ctx, mut app) = setup();
    let electrical = crate::layers::LayerDocument::legacy_layer_id("Electrical");
    let main = quick_option_element(crate::layers::ElementLayer(electrical), None);
    let mut option_element = main.clone();
    option_element.id = 2;
    let option_id = app.editor.project.design_options.create("Option A".into());
    option_element.design_option = Some(option_id);
    app.editor.project.elements = vec![main.clone(), option_element.clone()];
    app.editor.project.next_id = 3;
    app.editor.project.validate().unwrap();

    let layers = &app.editor.project.layers;
    let options = &app.editor.project.plan_options;
    assert!(options.element_visible(&main, layers));

    // 01 Site hides MEP layers; Show All restores them.
    app.apply_layer_combination(1);
    let layers = &app.editor.project.layers;
    let options = &app.editor.project.plan_options;
    assert!(!layers.is_visible(electrical));
    assert!(!options.element_visible(&main, layers));
    assert!(!options.element_visible(&option_element, layers));
    app.apply_show_all_layers(true);
    let layers = &app.editor.project.layers;
    let options = &app.editor.project.plan_options;
    assert!(options.element_visible(&main, layers));

    app.set_active_design_option(None);
    let layers = &app.editor.project.layers;
    let options = &app.editor.project.plan_options;
    assert!(options.element_visible(&main, layers));
    assert!(!options.element_visible(&option_element, layers));
    app.set_active_design_option(Some(option_id));
    let layers = &app.editor.project.layers;
    let options = &app.editor.project.plan_options;
    assert!(options.element_visible(&option_element, layers));
}

#[test]
fn quick_options_bar_and_palette_visibility_are_independent() {
    let (_ctx, mut app) = setup();
    assert!(app.quick_options_visible);
    assert!(!app.quick_options_palette_visible);
    let before = app.editor.project.clone();
    let undo_len = app.editor.undo.len();

    app.toggle_quick_options_palette();
    assert!(app.quick_options_visible);
    assert!(app.quick_options_palette_visible);
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo.len(), undo_len);

    app.toggle_quick_options_bar();
    assert!(!app.quick_options_visible);
    assert!(app.quick_options_palette_visible);
    assert_eq!(app.editor.project, before);

    app.toggle_quick_options_palette();
    assert!(!app.quick_options_palette_visible);
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.undo.len(), undo_len);
}

#[test]
fn plan_orientation_round_trips_between_world_and_screen() {
    let (_ctx, mut app) = setup();
    app.set_orientation_degrees(37.5);
    assert!((app.editor.project.plan_options.normalized_orientation() - 37.5).abs() < 1e-5);
    let rect = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
    for point in [
        Point::new(0., 0.),
        Point::new(3., -2.),
        Point::new(-1.5, 4.),
    ] {
        let screen = app.screen(rect, point);
        let back = app.world(rect, screen);
        assert!((back.x - point.x).abs() < 1e-4, "{back:?} != {point:?}");
        assert!((back.y - point.y).abs() < 1e-4, "{back:?} != {point:?}");
    }
}
#[test]
fn quick_options_zoom_and_bar_visibility_round_trip() {
    let (_ctx, mut app) = setup();
    assert!(app.quick_options_visible);
    app.set_plan_zoom_percent(150.);
    assert!((app.plan_scale - 60.).abs() < 1e-5);
    assert_eq!(app.editor.status, "Zoom · 150.0%");

    app.toggle_quick_options_bar();
    assert!(!app.quick_options_visible);
    assert_eq!(app.editor.status, "Quick Options bar hidden");
    app.toggle_quick_options_bar();
    assert!(app.quick_options_visible);
    assert_eq!(app.editor.status, "Quick Options bar shown");
}

fn add_info_box_wall(app: &mut AtelierApp, id_offset: f32) -> u64 {
    app.editor.project.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: Point::new(id_offset, 0.),
            end: Point::new(id_offset + 4., 0.),
            thickness: 0.2,
            height: 3.,
        },
    )
}

fn info_box_wall(app: &AtelierApp, id: u64) -> &Element {
    app.editor
        .project
        .elements
        .iter()
        .find(|element| element.id == id)
        .expect("wall exists")
}

#[test]
fn info_box_tracks_single_wall_selection() {
    let (_ctx, mut app) = setup();
    let id = add_info_box_wall(&mut app, 0.);
    app.editor.set_selection(vec![id]);
    app.sync_info_box_wall();
    let state = app.info_box_wall.as_ref().expect("wall state");
    assert_eq!(state.element_id, id);
    assert!(!state.dirty);
    assert_eq!(state.draft.thickness, 0.2);
    assert_eq!(state.draft.height, 3.);

    app.editor.set_selection(Vec::new());
    app.sync_info_box_wall();
    assert!(app.info_box_wall.is_none());
}

#[test]
fn info_box_renders_wall_fields_and_follows_palette_visibility() {
    let (ctx, mut app) = setup();
    let id = add_info_box_wall(&mut app, 0.);
    app.editor.set_selection(vec![id]);
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
            ..Default::default()
        },
        |ui| app.workspace(ui),
    );
    let mut texts = Vec::new();
    collect_text(&output.shapes, &mut texts);
    output.textures_delta.clear();
    for expected in [
        "INFO BOX",
        "All Selected: 1",
        "Structural:",
        "Reference Line:",
        "Thickness",
        "Height",
        "Floor Plan and Section…",
        "Apply",
    ] {
        assert!(
            texts.iter().any(|text| text == expected),
            "missing Info Box text {expected:?}: {texts:?}"
        );
    }

    app.info_box_visible = false;
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
            ..Default::default()
        },
        |ui| app.workspace(ui),
    );
    let mut texts = Vec::new();
    collect_text(&output.shapes, &mut texts);
    output.textures_delta.clear();
    assert!(!texts.iter().any(|text| text == "All Selected: 1"));
    assert!(!texts.iter().any(|text| text == "INFO BOX"));
}

#[test]
fn info_box_wall_fields_follow_simplified_chinese() {
    let (ctx, mut app) = setup();
    app.language = UiLanguage::SimplifiedChinese;
    let id = add_info_box_wall(&mut app, 0.);
    app.editor.set_selection(vec![id]);
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
            ..Default::default()
        },
        |ui| app.workspace(ui),
    );
    let mut texts = Vec::new();
    collect_text(&output.shapes, &mut texts);
    output.textures_delta.clear();
    for expected in ["信息框", "结构功能：", "厚度", "平面图与剖面…"] {
        assert!(
            texts.iter().any(|text| text == expected),
            "missing localized Info Box text {expected:?}: {texts:?}"
        );
    }
    assert!(
        texts.iter().any(|text| text.starts_with("全部已选: 1")),
        "missing localized selection count: {texts:?}"
    );
}

#[test]
fn info_box_commit_updates_plan_model_and_single_undo_restores() {
    let (_ctx, mut app) = setup();
    let id = add_info_box_wall(&mut app, 0.);
    app.editor.set_selection(vec![id]);
    app.sync_info_box_wall();
    {
        let state = app.info_box_wall.as_mut().expect("wall state");
        state.draft.thickness = 0.35;
        state.draft.height = 4.25;
        state.draft.settings.geometry.reference_line = crate::wall::WallReferenceLine::Outside;
        state.draft.settings.geometry.reference_offset = 0.12;
        state.dirty = true;
    }
    assert!(app.commit_info_box_wall());
    let wall = info_box_wall(&app, id);
    assert!(matches!(
        wall.shape,
        Shape::Wall {
            thickness: 0.35,
            height: 4.25,
            ..
        }
    ));
    let settings = wall.wall_settings.as_ref().expect("wall settings");
    assert_eq!(
        settings.geometry.reference_line,
        crate::wall::WallReferenceLine::Outside
    );
    assert_eq!(settings.geometry.reference_offset, 0.12);
    assert_eq!(app.editor.undo.len(), 1);

    app.editor.undo();
    let wall = info_box_wall(&app, id);
    assert!(matches!(
        wall.shape,
        Shape::Wall {
            thickness: 0.2,
            height: 3.,
            ..
        }
    ));
    assert_eq!(
        wall.wall_settings
            .as_ref()
            .expect("wall settings")
            .geometry
            .reference_line,
        crate::wall::WallReferenceLine::Center
    );
    app.editor.set_selection(vec![id]);
    app.sync_info_box_wall();
    assert_eq!(
        app.info_box_wall.as_ref().expect("wall state").draft.height,
        3.
    );
}

#[test]
fn info_box_invalid_thickness_preserves_model_and_reports_error() {
    let (_ctx, mut app) = setup();
    app.language = UiLanguage::SimplifiedChinese;
    let id = add_info_box_wall(&mut app, 0.);
    app.editor.set_selection(vec![id]);
    app.sync_info_box_wall();
    {
        let state = app.info_box_wall.as_mut().expect("wall state");
        state.draft.thickness = 0.001;
        state.dirty = true;
    }
    assert!(!app.commit_info_box_wall());
    assert!(matches!(
        info_box_wall(&app, id).shape,
        Shape::Wall { thickness: 0.2, .. }
    ));
    let state = app.info_box_wall.as_ref().expect("wall state");
    assert!(state.dirty);
    assert!(state.error.contains("墙厚"));
    assert!(app.editor.undo.is_empty());
}

#[test]
fn info_box_selection_change_commits_pending_wall() {
    let (_ctx, mut app) = setup();
    let first = add_info_box_wall(&mut app, 0.);
    let second = add_info_box_wall(&mut app, 6.);
    app.editor.set_selection(vec![first]);
    app.sync_info_box_wall();
    {
        let state = app.info_box_wall.as_mut().expect("wall state");
        state.draft.thickness = 0.31;
        state.dirty = true;
    }
    app.editor.set_selection(vec![second]);
    app.sync_info_box_wall();
    assert!(matches!(
        info_box_wall(&app, first).shape,
        Shape::Wall {
            thickness: 0.31,
            ..
        }
    ));
    let state = app.info_box_wall.as_ref().expect("wall state");
    assert_eq!(state.element_id, second);
    assert!(!state.dirty);
    assert_eq!(state.draft.thickness, 0.2);
}

#[test]
fn info_box_hide_commits_pending_value() {
    let (_ctx, mut app) = setup();
    let id = add_info_box_wall(&mut app, 0.);
    app.editor.set_selection(vec![id]);
    app.sync_info_box_wall();
    {
        let state = app.info_box_wall.as_mut().expect("wall state");
        state.draft.height = 3.6;
        state.dirty = true;
    }
    app.toggle_info_box_visibility();
    assert!(!app.info_box_visible);
    assert!(matches!(
        info_box_wall(&app, id).shape,
        Shape::Wall { height: 3.6, .. }
    ));
}

#[test]
fn command_t_commits_pending_info_box_edit_before_wall_settings() {
    let (ctx, mut app) = setup();
    let id = add_info_box_wall(&mut app, 0.);
    app.editor.set_selection(vec![id]);
    app.sync_info_box_wall();
    {
        let state = app.info_box_wall.as_mut().expect("wall state");
        state.draft.thickness = 0.42;
        state.dirty = true;
    }
    plan_frame(&ctx, &mut app, vec![key(Key::T, Modifiers::COMMAND)]);
    assert!(app.wall_settings_dialog.is_some());
    assert!(app.info_box_wall.is_none());
    assert!(matches!(
        info_box_wall(&app, id).shape,
        Shape::Wall {
            thickness: 0.42,
            ..
        }
    ));
}

#[test]
fn info_box_floor_plan_and_settings_buttons_open_wall_settings() {
    let (_ctx, mut app) = setup();
    let id = add_info_box_wall(&mut app, 0.);
    app.editor.set_selection(vec![id]);
    app.sync_info_box_wall();

    app.open_info_box_wall_settings(true);
    let dialog = app.wall_settings_dialog.as_ref().expect("wall settings");
    assert!(dialog.sections.floor_plan);
    assert_eq!(dialog.draft.name, "Wall 001");

    app.wall_settings_dialog = None;
    app.open_info_box_wall_settings(false);
    let dialog = app.wall_settings_dialog.as_ref().expect("wall settings");
    assert!(!dialog.sections.floor_plan);
    assert!(dialog.sections.geometry);
}

#[test]
fn wall_tool_info_box_shows_creation_defaults_in_chinese() {
    let (ctx, mut app) = setup();
    app.language = UiLanguage::SimplifiedChinese;
    app.editor.set_tool(Tool::Wall);
    app.sync_info_box_wall_tool();
    let state = app.info_box_wall_tool.as_ref().expect("wall tool defaults");
    assert_eq!(state.draft.thickness, app.editor.thickness);
    assert_eq!(state.draft.layer, app.editor.wall_layer);
    assert!(!state.dirty);
    assert!(app.info_box_wall.is_none());

    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1500., 900.))),
            ..Default::default()
        },
        |ui| app.workspace(ui),
    );
    let mut texts = Vec::new();
    collect_text(&output.shapes, &mut texts);
    output.textures_delta.clear();
    for expected in [
        "默认设置",
        "图层：",
        "几何方法：",
        "参考线：",
        "参考线偏移",
        "结构：",
        "厚度",
        "高度",
        "顶部链接：",
        "顶部偏移",
        "底部偏移",
        "平面图与剖面…",
        "设置…",
    ] {
        assert!(
            texts.iter().any(|text| text == expected),
            "missing wall-tool Info Box text {expected:?}: {texts:?}"
        );
    }
    assert!(!texts.iter().any(|text| text == "全部已选: 1"));
}

#[test]
fn wall_tool_info_box_defaults_apply_to_the_next_wall() {
    let (_ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.sync_info_box_wall_tool();
    {
        let state = app.info_box_wall_tool.as_mut().expect("wall tool defaults");
        state.draft.thickness = 0.36;
        state.draft.height = 4.2;
        state.draft.layer = crate::layers::ElementLayer(
            crate::layers::LayerDocument::legacy_layer_id("Structural"),
        );
        state.draft.settings.geometry.geometry_method = crate::wall::WallGeometryMethod::Single;
        state.draft.settings.geometry.reference_line = crate::wall::WallReferenceLine::Outside;
        state.draft.settings.geometry.reference_offset = 0.15;
        state.draft.settings.geometry.top_link_offset = -1;
        state.draft.settings.geometry.top_offset = 0.25;
        state.draft.settings.geometry.bottom_offset = -0.1;
        state.draft.settings.profile.name = "Brick 240".into();
        state.dirty = true;
    }
    assert!(app.commit_info_box_wall_tool());
    assert!(app.editor.undo.is_empty());
    assert_eq!(app.editor.thickness, 0.36);
    assert_eq!(app.editor.height, 4.2);
    assert_eq!(
        app.editor.wall_layer.0,
        crate::layers::LayerDocument::legacy_layer_id("Structural")
    );

    app.editor.click(Point::new(0., 0.), 0.);
    app.editor.click(Point::new(4., 0.), 0.);
    let wall = app.editor.project.elements.last().expect("new wall");
    assert!(matches!(
        wall.shape,
        Shape::Wall {
            thickness: 0.36,
            height: 4.2,
            ..
        }
    ));
    assert_eq!(
        wall.layer.0,
        crate::layers::LayerDocument::legacy_layer_id("Structural")
    );
    let settings = wall.wall_settings.as_ref().expect("wall settings");
    assert_eq!(
        settings.geometry.geometry_method,
        crate::wall::WallGeometryMethod::Single
    );
    assert_eq!(
        settings.geometry.reference_line,
        crate::wall::WallReferenceLine::Outside
    );
    assert_eq!(settings.geometry.reference_offset, 0.15);
    assert_eq!(settings.geometry.top_link_offset, -1);
    assert_eq!(settings.geometry.top_offset, 0.25);
    assert_eq!(settings.geometry.bottom_offset, -0.1);
    assert_eq!(settings.profile.name, "Brick 240");
    assert_eq!(app.editor.undo.len(), 1);
}

#[test]
fn wall_tool_info_box_geometry_method_drives_real_drawing_behavior() {
    let (_ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.sync_info_box_wall_tool();
    {
        let state = app.info_box_wall_tool.as_mut().expect("wall tool defaults");
        state.draft.settings.geometry.geometry_method = WallGeometryMethod::Continuous;
        state.dirty = true;
    }
    assert!(app.commit_info_box_wall_tool());
    for point in [Point::new(0., 0.), Point::new(4., 0.), Point::new(4., 3.)] {
        app.editor.click(point, 0.);
    }
    assert_eq!(app.editor.wall_points.len(), 3);
    assert!(app.editor.project.elements.is_empty());
    app.editor.finish_wall(None).unwrap();
    assert_eq!(app.editor.project.elements.len(), 2);
    assert_eq!(app.editor.selection_ids().len(), 1);
    app.sync_info_box_wall();
    assert!(
        app.info_box_wall.is_some(),
        "the last created wall stays selected so the Info Box remains usable"
    );
}

#[test]
fn continuous_wall_double_click_finishes_the_chain() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.editor.wall_settings.geometry.geometry_method = WallGeometryMethod::Continuous;
    click(&ctx, &mut app, pos2(400., 300.));
    click(&ctx, &mut app, pos2(500., 300.));
    click(&ctx, &mut app, pos2(600., 300.));
    click(&ctx, &mut app, pos2(600., 300.));
    assert_eq!(app.editor.project.elements.len(), 2);
    assert!(app.editor.wall_points.is_empty());
    assert_eq!(app.editor.undo.len(), 1);
}

#[test]
fn continuous_wall_clicking_the_start_point_closes_the_chain() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.editor.wall_settings.geometry.geometry_method = WallGeometryMethod::Continuous;
    click(&ctx, &mut app, pos2(300., 300.));
    click(&ctx, &mut app, pos2(500., 300.));
    click(&ctx, &mut app, pos2(500., 200.));
    click(&ctx, &mut app, pos2(300., 300.));
    assert_eq!(app.editor.project.elements.len(), 3);
    assert!(app.editor.wall_points.is_empty());
    assert!(app.editor.anchor.is_none());
    assert_eq!(app.editor.undo.len(), 1);
}

#[test]
fn wall_start_click_is_magnetic_even_when_grid_snap_moves_the_pointer() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.editor.wall_settings.geometry.geometry_method = WallGeometryMethod::Continuous;
    app.editor.project.grid_settings.grid_snap_enabled = false;
    click(&ctx, &mut app, pos2(300., 300.));
    click(&ctx, &mut app, pos2(500., 300.));
    click(&ctx, &mut app, pos2(500., 200.));
    app.editor.project.grid_settings.grid_snap_enabled = true;
    app.editor.project.grid_settings.snap_spacing_x = 0.5;
    app.editor.project.grid_settings.snap_spacing_y = 0.5;
    // 10.4 px away from the first node; grid snap would otherwise move this
    // click to a different world point and append a spurious node.
    click(&ctx, &mut app, pos2(310., 303.));
    assert_eq!(app.editor.project.elements.len(), 3);
    assert!(app.editor.wall_points.is_empty());
    assert!(app.editor.anchor.is_none());
    assert_eq!(app.editor.undo.len(), 1);
}

#[test]
fn continuous_wall_start_click_with_two_nodes_finishes_the_single_wall() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.editor.wall_settings.geometry.geometry_method = WallGeometryMethod::Continuous;
    click(&ctx, &mut app, pos2(300., 300.));
    click(&ctx, &mut app, pos2(500., 300.));
    click(&ctx, &mut app, pos2(300., 300.));
    assert_eq!(app.editor.project.elements.len(), 1);
    assert!(app.editor.wall_points.is_empty());
    assert!(app.editor.anchor.is_none());
    assert_eq!(app.editor.undo.len(), 1);
}

#[test]
fn polygonal_wall_start_click_with_two_nodes_does_not_append_a_backtrack() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.editor.wall_settings.geometry.geometry_method = WallGeometryMethod::Polygonal;
    click(&ctx, &mut app, pos2(300., 300.));
    click(&ctx, &mut app, pos2(500., 300.));
    click(&ctx, &mut app, pos2(300., 300.));
    assert_eq!(app.editor.wall_points.len(), 2);
    assert!(app.editor.project.elements.is_empty());
    assert!(app.editor.undo.is_empty());
}

#[test]
fn right_click_finishes_a_continuous_wall_draft() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.editor.wall_settings.geometry.geometry_method = WallGeometryMethod::Continuous;
    for point in [pos2(300., 300.), pos2(400., 300.), pos2(400., 200.)] {
        click(&ctx, &mut app, point);
    }
    assert_eq!(app.editor.wall_points.len(), 3);
    secondary_click(&ctx, &mut app, pos2(500., 300.));
    assert_eq!(app.editor.project.elements.len(), 2);
    assert!(app.editor.wall_points.is_empty());
    assert!(app.editor.anchor.is_none());
    assert_eq!(app.editor.undo.len(), 1);
}

#[test]
fn right_click_cancels_single_and_rectangular_drafts_without_an_undo_entry() {
    for method in [WallGeometryMethod::Single, WallGeometryMethod::Rectangular] {
        let (ctx, mut app) = setup();
        app.editor.set_tool(Tool::Wall);
        app.editor.wall_settings.geometry.geometry_method = method;
        click(&ctx, &mut app, pos2(300., 300.));
        assert!(app.editor.anchor.is_some());
        secondary_click(&ctx, &mut app, pos2(400., 300.));
        assert!(app.editor.project.elements.is_empty());
        assert!(app.editor.anchor.is_none());
        assert!(app.editor.wall_points.is_empty());
        assert!(app.editor.undo.is_empty());
        assert_eq!(app.editor.status, app.language.translate("Wall cancelled"));
    }
}

#[test]
fn right_drag_still_pans_without_finishing_the_wall_draft() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.editor.wall_settings.geometry.geometry_method = WallGeometryMethod::Continuous;
    for point in [pos2(300., 300.), pos2(400., 300.), pos2(400., 200.)] {
        click(&ctx, &mut app, point);
    }
    let start = pos2(500., 300.);
    plan_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(start),
            Event::PointerButton {
                pos: start,
                button: PointerButton::Secondary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(560., 340.))]);
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: pos2(560., 340.),
            button: PointerButton::Secondary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert_eq!(app.editor.wall_points.len(), 3);
    assert!(app.editor.project.elements.is_empty());
}

#[test]
fn polygonal_wall_double_click_on_start_closes_once() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.editor.wall_settings.geometry.geometry_method = WallGeometryMethod::Polygonal;
    click(&ctx, &mut app, pos2(300., 300.));
    click(&ctx, &mut app, pos2(500., 300.));
    click(&ctx, &mut app, pos2(500., 200.));
    click(&ctx, &mut app, pos2(300., 300.));
    click(&ctx, &mut app, pos2(300., 300.));
    assert_eq!(app.editor.project.elements.len(), 3);
    assert!(app.editor.wall_points.is_empty());
    assert!(app.editor.anchor.is_none());
    assert_eq!(app.editor.undo.len(), 1);
    assert!(
        !app.editor.status.contains("needs"),
        "{}",
        app.editor.status
    );
}

#[test]
fn wall_tool_info_box_rejects_invalid_thickness_without_changing_defaults() {
    let (_ctx, mut app) = setup();
    app.language = UiLanguage::SimplifiedChinese;
    app.editor.set_tool(Tool::Wall);
    app.sync_info_box_wall_tool();
    {
        let state = app.info_box_wall_tool.as_mut().expect("wall tool defaults");
        state.draft.thickness = 0.001;
        state.dirty = true;
    }
    assert!(!app.commit_info_box_wall_tool());
    assert_eq!(app.editor.thickness, 0.22);
    let state = app.info_box_wall_tool.as_ref().expect("wall tool defaults");
    assert!(state.dirty);
    assert!(state.error.contains("墙厚"));
    assert!(app.editor.undo.is_empty());
}

#[test]
fn command_t_commits_pending_wall_tool_defaults_before_settings() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.sync_info_box_wall_tool();
    {
        let state = app.info_box_wall_tool.as_mut().expect("wall tool defaults");
        state.draft.height = 3.75;
        state.dirty = true;
    }
    plan_frame(&ctx, &mut app, vec![key(Key::T, Modifiers::COMMAND)]);
    let dialog = app.wall_settings_dialog.as_ref().expect("wall settings");
    assert_eq!(dialog.mode, crate::wall_ui::WallSettingsMode::Default);
    assert_eq!(dialog.draft.height, 3.75);
    assert_eq!(app.editor.height, 3.75);
    assert!(app.editor.undo.is_empty());
}

#[test]
fn wall_tool_info_box_settings_buttons_open_default_dialog() {
    let (_ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.sync_info_box_wall_tool();

    app.open_info_box_wall_settings(true);
    let dialog = app.wall_settings_dialog.as_ref().expect("wall settings");
    assert_eq!(dialog.mode, crate::wall_ui::WallSettingsMode::Default);
    assert!(dialog.sections.floor_plan);
    assert_eq!(dialog.draft.thickness, app.editor.thickness);

    app.wall_settings_dialog = None;
    app.open_info_box_wall_settings(false);
    let dialog = app.wall_settings_dialog.as_ref().expect("wall settings");
    assert_eq!(dialog.mode, crate::wall_ui::WallSettingsMode::Default);
    assert!(!dialog.sections.floor_plan);
    assert!(dialog.sections.geometry);
}

fn close_frame(app: &mut AtelierApp, ctx: &Context) -> eframe::egui::FullOutput {
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
            viewports: [(
                ViewportId::ROOT,
                ViewportInfo {
                    events: vec![ViewportEvent::Close],
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        },
        |ui| app.handle_close_request(ui.ctx()),
    );
    output.textures_delta.clear();
    output
}

#[test]
fn dirty_close_request_is_cancelled_and_opens_prompt() {
    let (ctx, mut app) = setup();
    app.editor.project.add(
        0,
        Material::Plaster,
        Shape::Wall {
            start: Point::new(0., 0.),
            end: Point::new(2., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    let before = app.editor.project.clone();
    app.editor.set_selection(vec![1]);
    let selection = app.editor.selected_ids.clone();
    let output = close_frame(&mut app, &ctx);
    assert!(app.close_prompt);
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.selected_ids, selection);
    assert!(
        output.viewport_output[&ViewportId::ROOT]
            .commands
            .contains(&ViewportCommand::CancelClose)
    );
}

#[test]
fn clean_close_request_is_not_cancelled() {
    let (ctx, mut app) = setup();
    let output = close_frame(&mut app, &ctx);
    assert!(app.allow_close);
    assert!(!app.close_prompt);
    assert!(
        !output.viewport_output[&ViewportId::ROOT]
            .commands
            .contains(&ViewportCommand::CancelClose)
    );
}

#[test]
fn save_project_clears_dirty_and_recovery_file() {
    let (_ctx, mut app) = setup();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let project_path = std::env::temp_dir().join(format!(
        "openarchi-save-{}-{stamp}.openarchi.json",
        std::process::id()
    ));
    let recovery_path = std::env::temp_dir().join(format!(
        "openarchi-save-recovery-{}-{stamp}.openarchi.json",
        std::process::id()
    ));
    app.path = project_path.display().to_string();
    app.recovery_path = Some(recovery_path.clone());
    app.editor.project.add(
        0,
        Material::Plaster,
        Shape::Wall {
            start: Point::new(0., 0.),
            end: Point::new(2., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    assert!(app.editor.dirty());
    crate::recovery::write(&recovery_path, &app.editor.project).unwrap();
    app.save_project().unwrap();
    assert!(!app.editor.dirty());
    assert!(!recovery_path.exists());
    let loaded = storage::load(&project_path).unwrap();
    assert_eq!(loaded, app.editor.project);
    let _ = std::fs::remove_file(project_path);
}

#[test]
fn closing_after_discard_does_not_rewrite_the_recovery_file() {
    let (ctx, mut app) = setup();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let recovery_path = std::env::temp_dir().join(format!(
        "openarchi-discard-recovery-{}-{stamp}.openarchi.json",
        std::process::id()
    ));
    app.recovery_path = Some(recovery_path.clone());
    app.editor.project.add(
        0,
        Material::Plaster,
        Shape::Wall {
            start: Point::new(0., 0.),
            end: Point::new(2., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    app.autosave_recovery(&ctx);
    assert!(recovery_path.exists());

    app.discard_recovery_file();
    app.allow_close = true;
    app.autosave_recovery(&ctx);
    assert!(!recovery_path.exists());
    crate::recovery::clear(Some(&recovery_path));
}

#[test]
fn commit_refreshes_recovery_inside_the_throttle_window() {
    let (ctx, mut app) = setup();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let recovery_path = std::env::temp_dir().join(format!(
        "openarchi-commit-recovery-{}-{stamp}.openarchi.json",
        std::process::id()
    ));
    app.recovery_path = Some(recovery_path.clone());

    let before = app.editor.project.clone();
    app.editor.project.add(
        0,
        Material::Plaster,
        Shape::Wall {
            start: Point::new(0., 0.),
            end: Point::new(2., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    app.editor.commit(before);
    app.autosave_recovery(&ctx);
    let first = crate::recovery::read(&recovery_path).unwrap().unwrap();

    let before = app.editor.project.clone();
    app.editor.project.add(
        0,
        Material::Plaster,
        Shape::Wall {
            start: Point::new(0., 1.),
            end: Point::new(2., 1.),
            thickness: 0.2,
            height: 3.,
        },
    );
    app.editor.commit(before);
    app.autosave_recovery(&ctx);
    let second = crate::recovery::read(&recovery_path).unwrap().unwrap();

    assert_eq!(second.elements.len(), first.elements.len() + 1);
    assert_eq!(second, app.editor.project);
    crate::recovery::clear(Some(&recovery_path));
}

#[test]
fn recovered_project_remains_dirty_until_saved() {
    let mut editor = Editor::new(Project::default());
    let mut recovered = Project::default();
    recovered.add(
        0,
        Material::Plaster,
        Shape::Wall {
            start: Point::new(0., 0.),
            end: Point::new(2., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    editor.restore_unsaved(recovered.clone());
    assert_eq!(editor.project, recovered);
    assert!(editor.dirty());
    editor.mark_saved();
    assert!(!editor.dirty());
}

#[test]
fn door_info_box_shows_and_edits_hosted_opening_fields() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.editor.click(Point::new(0., 0.), 0.);
    app.editor.click(Point::new(6., 0.), 0.);
    let wall_id = app.editor.project.elements[0].id;
    app.editor.set_tool(Tool::Door);
    app.editor.click(Point::new(3., 0.), 0.05);
    let opening_id = app.editor.selection_ids()[0];

    app.sync_info_box_opening();
    let state = app.info_box_opening.as_ref().expect("opening state");
    assert_eq!(state.element_id, opening_id);
    assert_eq!(state.host_wall, wall_id);
    assert!((state.width - 0.9).abs() < 1e-4);
    assert!((state.height - 2.1).abs() < 1e-4);

    {
        let state = app.info_box_opening.as_mut().unwrap();
        state.width = 1.1;
        state.height = 2.0;
        state.sill = 0.05;
        state.offset = 2.5;
        state.dirty = true;
    }
    assert!(app.commit_info_box_opening());
    let updated = app
        .editor
        .project
        .elements
        .iter()
        .find(|element| element.id == opening_id)
        .unwrap();
    let Shape::Opening {
        width,
        height,
        sill,
        offset,
        ..
    } = updated.shape
    else {
        panic!("expected opening");
    };
    assert!((width - 1.1).abs() < 1e-4);
    assert!((height - 2.0).abs() < 1e-4);
    assert!((sill - 0.05).abs() < 1e-4);
    assert!((offset - 2.5).abs() < 1e-4);

    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
            ..Default::default()
        },
        |ui| app.workspace(ui),
    );
    let mut texts = Vec::new();
    collect_text(&output.shapes, &mut texts);
    output.textures_delta.clear();
    for expected in [
        "Door 002",
        "Host Wall: 001",
        "Width",
        "Height",
        "Sill Height",
        "Offset",
    ] {
        assert!(
            texts.iter().any(|text| text.contains(expected)),
            "missing {expected:?} in {texts:?}"
        );
    }
}

#[test]
fn opening_tool_defaults_are_editable_and_localized() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Door);
    app.editor.set_selection(Vec::new());
    app.sync_info_box_opening_tool();
    let state = app.info_box_opening_tool.as_ref().expect("tool state");
    assert_eq!(state.kind, OpeningKind::Door);
    assert!((state.defaults.width - 0.9).abs() < 1e-4);

    {
        let state = app.info_box_opening_tool.as_mut().unwrap();
        state.defaults.width = 1.0;
        state.defaults.height = 2.2;
        state.dirty = true;
    }
    assert!(app.commit_info_box_opening_tool());
    assert!((app.editor.door_defaults.width - 1.0).abs() < 1e-4);

    app.language = UiLanguage::SimplifiedChinese;
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
            ..Default::default()
        },
        |ui| app.workspace(ui),
    );
    let mut texts = Vec::new();
    collect_text(&output.shapes, &mut texts);
    output.textures_delta.clear();
    for expected in ["门", "洞口", "宽度", "高度", "窗台高度"] {
        assert!(
            texts.iter().any(|text| text.contains(expected)),
            "missing {expected:?} in {texts:?}"
        );
    }
}

fn transfer_test_wall(
    project: &mut Project,
    start: (f32, f32),
    end: (f32, f32),
    thickness: f32,
    height: f32,
    material: crate::model::Material,
) -> u64 {
    project.add(
        0,
        material,
        crate::model::Shape::Wall {
            start: Point::new(start.0, start.1),
            end: Point::new(end.0, end.1),
            thickness,
            height,
        },
    )
}

fn modifier_click(ctx: &Context, app: &mut AtelierApp, pos: Pos2, modifiers: Modifiers) {
    plan_frame(
        ctx,
        app,
        vec![Event::ModifiersChanged(modifiers), Event::PointerMoved(pos)],
    );
    plan_frame(
        ctx,
        app,
        vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers,
        }],
    );
    // egui reports `clicked` on the release frame, so the modifiers must
    // still be held here.
    plan_frame(
        ctx,
        app,
        vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers,
        }],
    );
}

#[test]
fn option_c_picks_up_from_selection_and_sets_tool_defaults() {
    let (ctx, mut app) = setup();
    let source = transfer_test_wall(
        &mut app.editor.project,
        (0., 0.),
        (4., 0.),
        0.4,
        2.6,
        crate::model::Material::Timber,
    );
    let target = transfer_test_wall(
        &mut app.editor.project,
        (10., 0.),
        (14., 0.),
        0.2,
        3.,
        crate::model::Material::Concrete,
    );
    app.editor.mark_saved();
    app.editor.set_selection(vec![source]);
    plan_frame(&ctx, &mut app, vec![key(Key::C, Modifiers::ALT)]);
    assert!(
        app.editor.status.contains("Picked up"),
        "{}",
        app.editor.status
    );
    assert_eq!(
        app.editor
            .pick_up_buffer
            .get(crate::transfer::TransferType::Wall)
            .unwrap()
            .thickness(),
        0.4
    );
    assert_eq!(app.editor.thickness, 0.4);
    assert_eq!(app.editor.height, 2.6);
    assert_eq!(app.editor.material, crate::model::Material::Timber);
    // AC29 semantics: the pick-up arms the matching tool, ready to draw.
    assert_eq!(app.editor.tool, Tool::Wall);
    // Picking up only changes tool defaults: no undo record and no dirty flag.
    assert!(app.editor.undo.is_empty());
    assert!(!app.editor.dirty());

    app.editor.set_selection(vec![target]);
    plan_frame(
        &ctx,
        &mut app,
        vec![key(Key::C, Modifiers::ALT | Modifiers::COMMAND)],
    );
    assert!(
        app.editor.status.contains("Injected"),
        "{}",
        app.editor.status
    );
    assert_eq!(
        app.editor
            .project
            .elements
            .iter()
            .find(|element| element.id == target)
            .unwrap()
            .thickness(),
        0.4
    );
    // The whole injection is one reversible change.
    assert_eq!(app.editor.undo.len(), 1);
    app.editor.undo();
    assert_eq!(
        app.editor
            .project
            .elements
            .iter()
            .find(|element| element.id == target)
            .unwrap()
            .thickness(),
        0.2
    );
}

#[test]
fn cmd_option_c_without_selection_arms_one_shot_inject() {
    let (ctx, mut app) = setup();
    let source = transfer_test_wall(
        &mut app.editor.project,
        (0., 0.),
        (4., 0.),
        0.4,
        3.,
        crate::model::Material::Timber,
    );
    let target = transfer_test_wall(
        &mut app.editor.project,
        (5., 0.),
        (6., 0.),
        0.2,
        3.,
        crate::model::Material::Concrete,
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![key(Key::C, Modifiers::ALT | Modifiers::COMMAND)],
    );
    assert!(
        !app.inject_armed,
        "inject must refuse to arm before anything was picked up"
    );
    app.editor.set_selection(vec![source]);
    plan_frame(&ctx, &mut app, vec![key(Key::C, Modifiers::ALT)]);
    app.editor.set_selection(Vec::new());
    plan_frame(
        &ctx,
        &mut app,
        vec![key(Key::C, Modifiers::ALT | Modifiers::COMMAND)],
    );
    assert!(app.inject_armed, "{}", app.editor.status);
    // The next plan click injects into the element under the cursor.
    let pos = app.screen(
        Rect::from_min_size(Pos2::ZERO, vec2(800., 600.)),
        Point::new(5.5, 0.),
    );
    modifier_click(&ctx, &mut app, pos, Modifiers::NONE);
    assert!(!app.inject_armed);
    assert_eq!(
        app.editor
            .project
            .elements
            .iter()
            .find(|element| element.id == target)
            .unwrap()
            .thickness(),
        0.4
    );
    // Escape cancels an armed mode before it is used.
    plan_frame(
        &ctx,
        &mut app,
        vec![key(Key::C, Modifiers::ALT | Modifiers::COMMAND)],
    );
    assert!(app.inject_armed);
    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(!app.inject_armed && !app.pick_armed);
}

#[test]
fn option_click_gesture_picks_up_in_plan() {
    let (ctx, mut app) = setup();
    let source = transfer_test_wall(
        &mut app.editor.project,
        (0., 0.),
        (4., 0.),
        0.4,
        3.,
        crate::model::Material::Timber,
    );
    app.editor.set_tool(Tool::Wall);
    // Option+click on the wall never starts a wall draft; it picks up.
    let pos = app.screen(
        Rect::from_min_size(Pos2::ZERO, vec2(800., 600.)),
        Point::new(2., 0.),
    );
    modifier_click(&ctx, &mut app, pos, Modifiers::ALT);
    assert!(
        app.editor.status.contains("Picked up"),
        "{}",
        app.editor.status
    );
    assert!(app.editor.wall_points.is_empty() && app.editor.anchor.is_none());
    assert!(
        app.editor
            .pick_up_buffer
            .get(crate::transfer::TransferType::Wall)
            .is_some()
    );
    assert_eq!(app.editor.thickness, 0.4);
    assert_eq!(app.editor.selected_ids.len(), 0);

    // Option+Cmd+click on empty space reports the miss and changes nothing.
    let empty = app.screen(
        Rect::from_min_size(Pos2::ZERO, vec2(800., 600.)),
        Point::new(-4., -4.),
    );
    let before = app.editor.project.clone();
    modifier_click(&ctx, &mut app, empty, Modifiers::ALT | Modifiers::COMMAND);
    assert_eq!(app.editor.project, before);
    assert!(app.editor.undo.is_empty());
    let _ = source;
}

#[test]
fn cmd_option_click_gesture_injects_into_target_element() {
    let (ctx, mut app) = setup();
    let source = transfer_test_wall(
        &mut app.editor.project,
        (0., 0.),
        (4., 0.),
        0.4,
        3.,
        crate::model::Material::Timber,
    );
    let target = transfer_test_wall(
        &mut app.editor.project,
        (5., 0.),
        (7., 0.),
        0.2,
        2.5,
        crate::model::Material::Concrete,
    );
    app.editor.mark_saved();

    // Option+click the source to pick up, then Option+Cmd+click the target
    // element to inject into it — the AC29 gesture pair.
    let source_hit = app.screen(
        Rect::from_min_size(Pos2::ZERO, vec2(800., 600.)),
        Point::new(2., 0.),
    );
    modifier_click(&ctx, &mut app, source_hit, Modifiers::ALT);
    let target_hit = app.screen(
        Rect::from_min_size(Pos2::ZERO, vec2(800., 600.)),
        Point::new(6., 0.),
    );
    modifier_click(
        &ctx,
        &mut app,
        target_hit,
        Modifiers::ALT | Modifiers::COMMAND,
    );
    assert!(
        app.editor.status.contains("Injected parameters into 1"),
        "{}",
        app.editor.status
    );
    let injected = app
        .editor
        .project
        .elements
        .iter()
        .find(|element| element.id == target)
        .unwrap();
    assert_eq!(injected.thickness(), 0.4);
    assert_eq!(injected.material, crate::model::Material::Timber);
    // The injected wall keeps its own reference line: only parameters move.
    match &injected.shape {
        crate::model::Shape::Wall { start, end, .. } => {
            assert_eq!((start.x, start.y), (5., 0.));
            assert_eq!((end.x, end.y), (7., 0.));
        }
        _ => unreachable!(),
    }
    assert_eq!(app.editor.undo.len(), 1);
    let _ = source;
}

#[test]
fn pick_up_activates_the_matching_tool() {
    let (ctx, mut app) = setup();
    let slab_id = app.editor.project.add(
        0,
        crate::model::Material::Concrete,
        crate::model::Shape::Slab {
            min: Point::new(0., 0.),
            max: Point::new(4., 3.),
            thickness: 0.25,
        },
    );
    app.editor.mark_saved();
    app.editor.set_tool(Tool::Select);

    // Picking up slab parameters arms the Slab tool with them.
    app.editor.set_selection(vec![slab_id]);
    plan_frame(&ctx, &mut app, vec![key(Key::C, Modifiers::ALT)]);
    assert_eq!(app.editor.tool, Tool::Slab);
    assert!(app.editor.status.contains("Picked up"));
    assert_eq!(app.editor.thickness, 0.25);

    // Picking up wall parameters re-arms the Wall tool.
    let wall = transfer_test_wall(
        &mut app.editor.project,
        (8., 0.),
        (12., 0.),
        0.3,
        2.7,
        crate::model::Material::Timber,
    );
    app.editor.mark_saved();
    app.editor.set_selection(vec![wall]);
    plan_frame(&ctx, &mut app, vec![key(Key::C, Modifiers::ALT)]);
    assert_eq!(app.editor.tool, Tool::Wall);
    assert_eq!(app.editor.thickness, 0.3);
    assert_eq!(app.editor.height, 2.7);
    // Tool activation is view state, not model state.
    assert!(app.editor.undo.is_empty());
    assert!(!app.editor.dirty());
}

#[test]
fn alt_hover_shows_eyedropper_cursor_and_highlights_element() {
    let (ctx, mut app) = setup();
    let wall = transfer_test_wall(
        &mut app.editor.project,
        (0., 0.),
        (4., 0.),
        0.4,
        3.,
        crate::model::Material::Timber,
    );
    let over_wall = app.screen(
        Rect::from_min_size(Pos2::ZERO, vec2(800., 600.)),
        Point::new(2., 0.),
    );
    let empty = app.screen(
        Rect::from_min_size(Pos2::ZERO, vec2(800., 600.)),
        Point::new(-4., -4.),
    );
    // Holding Option over the wall primes the eyedropper and highlights it.
    // egui hit-tests against the previous pass's widgets, so the first frame
    // settles the pointer before the asserted frame. The painted glyph is
    // the pointer: the OS cursor is hidden while the mode is active.
    plan_frame_output(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(Modifiers::ALT),
            Event::PointerMoved(over_wall),
        ],
    );
    let output = plan_frame_output(&ctx, &mut app, vec![Event::PointerMoved(over_wall)]);
    assert_eq!(output.platform_output.cursor_icon, CursorIcon::None);
    assert!(output.platform_output.cursor_image.is_none());
    assert_eq!(
        app.transfer_cursor.map(|(kind, primed, _)| (kind, primed)),
        Some((crate::transfer_cursor::TransferCursor::PickUp, true))
    );
    assert_eq!(app.transfer_hover, Some(wall));

    // Over empty space the eyedropper stays but is empty and nothing is
    // highlighted.
    let output = plan_frame_output(&ctx, &mut app, vec![Event::PointerMoved(empty)]);
    assert_eq!(output.platform_output.cursor_icon, CursorIcon::None);
    let (kind, primed, _) = app.transfer_cursor.expect("eyedropper still drawn");
    assert_eq!(kind, crate::transfer_cursor::TransferCursor::PickUp);
    assert!(!primed);
    assert_eq!(app.transfer_hover, None);

    // Releasing Option restores the standard cursor and stops the glyph.
    let output = plan_frame_output(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(Modifiers::NONE),
            Event::PointerMoved(over_wall),
        ],
    );
    assert!(output.platform_output.cursor_image.is_none());
    assert!(app.transfer_cursor.is_none());
    assert_eq!(app.transfer_hover, None);
    let _ = wall;
}

#[test]
fn alt_hover_suppresses_snap_feedback_until_released() {
    let (ctx, mut app) = setup();
    let _wall = transfer_test_wall(
        &mut app.editor.project,
        (0., 0.),
        (4., 0.),
        0.4,
        3.,
        crate::model::Material::Timber,
    );
    // The wall start node: without Option the hover feedback shows its snap
    // marker style (default cursor icon); with Option only the eyedropper
    // may show, so the feedback block must not run at all.
    let endpoint = app.screen(
        Rect::from_min_size(Pos2::ZERO, vec2(800., 600.)),
        Point::new(0., 0.),
    );
    plan_frame_output(&ctx, &mut app, vec![Event::PointerMoved(endpoint)]);
    let output = plan_frame_output(&ctx, &mut app, vec![Event::PointerMoved(endpoint)]);
    assert_eq!(output.platform_output.cursor_icon, CursorIcon::Default);
    assert!(output.platform_output.cursor_image.is_none());

    plan_frame_output(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(Modifiers::ALT),
            Event::PointerMoved(endpoint),
        ],
    );
    let output = plan_frame_output(&ctx, &mut app, vec![Event::PointerMoved(endpoint)]);
    assert_eq!(
        output.platform_output.cursor_icon,
        CursorIcon::None,
        "snap/hover feedback must be suppressed while Option is held"
    );
    assert!(output.platform_output.cursor_image.is_none());
    assert!(app.transfer_cursor.is_some());

    let output = plan_frame_output(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(Modifiers::NONE),
            Event::PointerMoved(endpoint),
        ],
    );
    assert_eq!(
        output.platform_output.cursor_icon,
        CursorIcon::Default,
        "snap/hover feedback returns when Option is released"
    );
    assert!(output.platform_output.cursor_image.is_none());
    assert!(app.transfer_cursor.is_none());
}

#[test]
fn cmd_alt_hover_primes_syringe_only_when_buffer_holds_type() {
    let (ctx, mut app) = setup();
    let source = transfer_test_wall(
        &mut app.editor.project,
        (0., 0.),
        (4., 0.),
        0.4,
        3.,
        crate::model::Material::Timber,
    );
    let target = transfer_test_wall(
        &mut app.editor.project,
        (5., 0.),
        (7., 0.),
        0.2,
        2.5,
        crate::model::Material::Concrete,
    );
    let over_target = app.screen(
        Rect::from_min_size(Pos2::ZERO, vec2(800., 600.)),
        Point::new(6., 0.),
    );
    // Cmd+Option over a wall with an empty buffer: the syringe cursor shows
    // but stays empty, because a click would do nothing. First frame settles
    // the pointer (egui hit-tests against the previous pass's widgets).
    plan_frame_output(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(Modifiers::ALT | Modifiers::COMMAND),
            Event::PointerMoved(over_target),
        ],
    );
    let output = plan_frame_output(&ctx, &mut app, vec![Event::PointerMoved(over_target)]);
    assert_eq!(
        output.platform_output.cursor_icon,
        CursorIcon::None,
        "syringe cursor while Cmd+Option is held"
    );
    assert!(app.transfer_cursor.is_some_and(|(kind, primed, _)| kind
        == crate::transfer_cursor::TransferCursor::Inject
        && !primed));
    assert_eq!(app.transfer_hover, None);

    // After picking up wall parameters the syringe primes over the target.
    app.editor.set_selection(vec![source]);
    plan_frame(&ctx, &mut app, vec![key(Key::C, Modifiers::ALT)]);
    plan_frame_output(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(Modifiers::ALT | Modifiers::COMMAND),
            Event::PointerMoved(over_target),
        ],
    );
    let output = plan_frame_output(&ctx, &mut app, vec![Event::PointerMoved(over_target)]);
    assert_eq!(output.platform_output.cursor_icon, CursorIcon::None);
    assert!(app.transfer_cursor.is_some_and(|(kind, primed, _)| kind
        == crate::transfer_cursor::TransferCursor::Inject
        && primed));
    assert_eq!(app.transfer_hover, Some(target));
}

#[test]
fn alt_hover_in_3d_shows_transfer_cursor_and_clears_without_modifier() {
    let (ctx, mut app) = setup();
    let _wall = transfer_test_wall(
        &mut app.editor.project,
        (0., 0.),
        (4., 0.),
        0.4,
        3.,
        crate::model::Material::Timber,
    );
    // Whether the default camera puts the wall under the pointer is framing
    // dependent, so only the bitmap and its clearing are asserted here. The
    // second frame settles the pointer for the hit test.
    model_frame_output(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(Modifiers::ALT),
            Event::PointerMoved(pos2(400., 300.)),
        ],
    );
    let output = model_frame_output(&ctx, &mut app, vec![Event::PointerMoved(pos2(400., 300.))]);
    assert_eq!(
        output.platform_output.cursor_icon,
        CursorIcon::None,
        "the painted transfer glyph is the pointer in 3D while Option is held"
    );
    assert!(
        app.transfer_cursor
            .is_some_and(|(kind, _, _)| kind == crate::transfer_cursor::TransferCursor::PickUp)
    );
    let output = model_frame_output(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(Modifiers::NONE),
            Event::PointerMoved(pos2(400., 300.)),
        ],
    );
    assert_eq!(output.platform_output.cursor_icon, CursorIcon::Default);
    assert!(app.transfer_cursor.is_none());
    assert_eq!(app.transfer_hover, None);
}

#[test]
fn drawing_tool_shows_simple_crosshair_cursor() {
    let (ctx, mut app) = setup();
    let _wall = transfer_test_wall(
        &mut app.editor.project,
        (0., 0.),
        (4., 0.),
        0.4,
        3.,
        crate::model::Material::Timber,
    );
    let over_canvas = app.screen(
        Rect::from_min_size(Pos2::ZERO, vec2(800., 600.)),
        Point::new(2., 0.),
    );
    // A drawing tool replaces the arrow with the AC29 square crosshair
    // (painted at the snap point); the OS cursor is hidden on the canvas.
    app.editor.set_tool(Tool::Wall);
    plan_frame_output(&ctx, &mut app, vec![Event::PointerMoved(over_canvas)]);
    let output = plan_frame_output(&ctx, &mut app, vec![Event::PointerMoved(over_canvas)]);
    assert_eq!(output.platform_output.cursor_icon, CursorIcon::None);
    assert!(app.transfer_cursor.is_none());

    // Select keeps the arrow-and-marker hover feedback.
    app.editor.set_tool(Tool::Select);
    let output = plan_frame_output(&ctx, &mut app, vec![Event::PointerMoved(over_canvas)]);
    assert_eq!(output.platform_output.cursor_icon, CursorIcon::Default);
}

#[test]
fn injection_rejects_cross_type_and_invalid_geometry() {
    let (ctx, mut app) = setup();
    let wall_id = transfer_test_wall(
        &mut app.editor.project,
        (0., 0.),
        (4., 0.),
        0.4,
        3.,
        crate::model::Material::Timber,
    );
    let slab_id = app.editor.project.add(
        0,
        crate::model::Material::Concrete,
        crate::model::Shape::Slab {
            min: Point::new(10., 0.),
            max: Point::new(14., 2.),
            thickness: 0.25,
        },
    );
    app.editor.set_selection(vec![wall_id]);
    plan_frame(&ctx, &mut app, vec![key(Key::C, Modifiers::ALT)]);
    app.editor.set_selection(vec![slab_id]);
    app.editor.set_tool(Tool::Select);
    plan_frame(
        &ctx,
        &mut app,
        vec![key(Key::C, Modifiers::ALT | Modifiers::COMMAND)],
    );
    // Injection never activates a tool — only pick-up does.
    assert_eq!(app.editor.tool, Tool::Select);
    // The type-keyed buffer has no slab entry, so the injection is refused
    // before any field changes (apply_transfer's same-type check is covered
    // by the unit tests in transfer.rs).
    assert!(
        app.editor.status.contains("No picked-up parameters")
            || app.editor.status.contains("所选构件类型没有已吸取的参数"),
        "{}",
        app.editor.status
    );
    assert_eq!(
        app.editor
            .project
            .elements
            .iter()
            .find(|element| element.id == slab_id)
            .unwrap()
            .thickness(),
        0.25
    );
    assert!(app.editor.undo.is_empty());

    // An injection that fails project validation must leave everything
    // untouched, including the undo history.
    let host = transfer_test_wall(
        &mut app.editor.project,
        (20., 0.),
        (26., 0.),
        0.2,
        2.,
        crate::model::Material::Plaster,
    );
    let door_id = app.editor.project.add(
        0,
        crate::model::Material::Timber,
        crate::model::Shape::Opening {
            host_wall: host,
            kind: crate::model::OpeningKind::Door,
            width: 0.9,
            height: 1.8,
            sill: 0.,
            offset: 3.,
        },
    );
    app.editor.set_selection(vec![door_id]);
    plan_frame(&ctx, &mut app, vec![key(Key::C, Modifiers::ALT)]);
    let tall_host = transfer_test_wall(
        &mut app.editor.project,
        (30., 0.),
        (36., 0.),
        0.2,
        2.,
        crate::model::Material::Plaster,
    );
    let tall_door = app.editor.project.add(
        0,
        crate::model::Material::Timber,
        crate::model::Shape::Opening {
            host_wall: tall_host,
            kind: crate::model::OpeningKind::Door,
            width: 1.2,
            height: 2.4,
            sill: 0.,
            offset: 3.,
        },
    );
    // Pick up the taller door, then try to inject it into the door on the
    // shorter wall: sill 0 + height 2.4 exceeds the 2 m host, so the whole
    // injection must fail validation and stay completely reversible.
    app.editor.set_selection(vec![tall_door]);
    plan_frame(&ctx, &mut app, vec![key(Key::C, Modifiers::ALT)]);
    app.editor.set_selection(vec![door_id]);
    plan_frame(
        &ctx,
        &mut app,
        vec![key(Key::C, Modifiers::ALT | Modifiers::COMMAND)],
    );
    assert!(app.editor.undo.is_empty(), "{}", app.editor.status);
    assert_eq!(
        app.editor
            .project
            .elements
            .iter()
            .find(|element| element.id == door_id)
            .unwrap()
            .area(),
        0.9 * 1.8
    );
}

#[test]
fn transfer_settings_dialog_change_is_one_undo_record() {
    let (ctx, mut app) = setup();
    let wall_id = transfer_test_wall(
        &mut app.editor.project,
        (0., 0.),
        (4., 0.),
        0.4,
        3.,
        crate::model::Material::Timber,
    );
    let target = transfer_test_wall(
        &mut app.editor.project,
        (10., 0.),
        (14., 0.),
        0.2,
        3.,
        crate::model::Material::Concrete,
    );
    app.editor.set_selection(vec![wall_id]);
    plan_frame(&ctx, &mut app, vec![key(Key::C, Modifiers::ALT)]);
    let mut settings = app.editor.project.transfer_settings.clone();
    settings.sets[0]
        .groups
        .remove(crate::transfer::GROUP_GEOMETRY);
    app.editor.set_selection(vec![target]);
    let set_before = app.editor.project.transfer_settings.clone();
    app.editor.set_transfer_settings(settings);
    assert_eq!(app.editor.undo.len(), 1, "dialog OK is one undo record");
    plan_frame(
        &ctx,
        &mut app,
        vec![key(Key::C, Modifiers::ALT | Modifiers::COMMAND)],
    );
    let target_element = app
        .editor
        .project
        .elements
        .iter()
        .find(|element| element.id == target)
        .unwrap();
    assert_eq!(target_element.thickness(), 0.2, "geometry group was off");
    assert_eq!(
        target_element.material,
        crate::model::Material::Timber,
        "material group stayed on"
    );
    // Two records: the settings change and the injection itself.
    assert_eq!(app.editor.undo.len(), 2);
    app.editor.undo();
    assert_eq!(
        app.editor
            .project
            .elements
            .iter()
            .find(|element| element.id == target)
            .unwrap()
            .material,
        crate::model::Material::Concrete,
        "undo reverts the injection"
    );
    app.editor.undo();
    assert_eq!(
        app.editor.project.transfer_settings, set_before,
        "undo restores the previous transfer settings"
    );
    let _ = ctx;
}

fn layers_frame_output(
    ctx: &Context,
    app: &mut AtelierApp,
    events: Vec<Event>,
) -> eframe::egui::FullOutput {
    let events = discrete_key_events(ctx, events);
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
            events,
            ..Default::default()
        },
        |ui| {
            app.workspace(ui);
        },
    );
    output.textures_delta.clear();
    output
}
fn layers_click(ctx: &Context, app: &mut AtelierApp, pos: Pos2) {
    layers_frame_output(ctx, app, vec![Event::PointerMoved(pos)]);
    layers_frame_output(
        ctx,
        app,
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    layers_frame_output(
        ctx,
        app,
        vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
}

#[test]
fn cmd_l_opens_layer_settings_and_selection_previews_before_ok() {
    let (ctx, mut app) = setup();
    let before = app.editor.project.layers.clone();

    layers_frame_output(&ctx, &mut app, vec![key(Key::L, Modifiers::COMMAND)]);
    assert!(app.layer_settings.is_some(), "Cmd+L opens the dialog");

    let output = layers_frame_output(&ctx, &mut app, vec![]);
    let mut dbg_texts = Vec::new();
    collect_text(&output.shapes, &mut dbg_texts);
    println!("DIALOG TEXTS: {dbg_texts:?}");
    for expected in ["Layer Combinations", "01 Site", "Search Layers", "- Hidden"] {
        assert!(
            text_center(&output.shapes, expected).is_some(),
            "dialog is missing {expected}"
        );
    }

    // Selecting 01 Site applies its snapshot to the working copy only.
    let site = text_center(&output.shapes, "01 Site").expect("01 Site row");
    layers_click(&ctx, &mut app, site);
    let electrical = crate::layers::LayerDocument::legacy_layer_id("Electrical");
    let state = app.layer_settings.as_ref().unwrap();
    assert!(!state.draft.is_visible(electrical));
    assert_eq!(
        app.editor.project.layers, before,
        "project unchanged before OK"
    );

    // Cancel discards the preview.
    let output = layers_frame_output(&ctx, &mut app, vec![]);
    let cancel = text_center(&output.shapes, "Cancel").expect("Cancel button");
    layers_click(&ctx, &mut app, cancel);
    assert!(app.layer_settings.is_none());
    assert_eq!(app.editor.project.layers, before);
}

#[test]
fn layer_settings_ok_applies_combination_and_undo_restores() {
    let (ctx, mut app) = setup();
    let before = app.editor.project.layers.clone();
    app.editor.project.elements = vec![quick_option_element(
        crate::layers::ElementLayer(crate::layers::LayerDocument::legacy_layer_id("Electrical")),
        None,
    )];
    app.editor.project.next_id = 2;
    app.editor.project.validate().unwrap();
    let undo_len = app.editor.undo.len();

    layers_frame_output(&ctx, &mut app, vec![key(Key::L, Modifiers::COMMAND)]);
    let output = layers_frame_output(&ctx, &mut app, vec![]);
    let site = text_center(&output.shapes, "01 Site").expect("01 Site row");
    layers_click(&ctx, &mut app, site);
    let output = layers_frame_output(&ctx, &mut app, vec![]);
    let ok = text_center(&output.shapes, "OK").expect("OK button");
    layers_click(&ctx, &mut app, ok);

    let electrical = crate::layers::LayerDocument::legacy_layer_id("Electrical");
    assert!(app.layer_settings.is_none(), "OK closes the dialog");
    assert!(!app.editor.project.layers.is_visible(electrical));
    assert_eq!(app.editor.project.layers.active_combination, Some(1));
    assert!(
        !app.editor
            .project
            .plan_options
            .element_visible(&app.editor.project.elements[0], &app.editor.project.layers),
        "hidden layer elements disappear from the plan"
    );
    assert_eq!(
        app.editor.undo.len(),
        undo_len + 1,
        "one reversible command"
    );

    app.editor.undo();
    assert_eq!(app.editor.project.layers, before);
    assert!(app.editor.project.layers.is_visible(electrical));
}

#[test]
fn layer_settings_new_layer_creates_and_esc_cancels() {
    let (ctx, mut app) = setup();
    let before = app.editor.project.layers.clone();

    layers_frame_output(&ctx, &mut app, vec![key(Key::L, Modifiers::COMMAND)]);
    let output = layers_frame_output(&ctx, &mut app, vec![]);
    let new = text_center(&output.shapes, "New").expect("New button");
    layers_click(&ctx, &mut app, new);
    let state = app.layer_settings.as_ref().unwrap();
    assert!(
        state
            .draft
            .layers
            .iter()
            .any(|layer| layer.name.starts_with("New Layer")),
        "New adds a layer to the working copy"
    );

    // Escape cancels like AC29.
    layers_frame_output(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.layer_settings.is_none());
    assert_eq!(app.editor.project.layers, before);
}

#[test]
fn locked_layer_blocks_delete_edit_and_placement() {
    let (_ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.editor.click(Point::new(0., 0.), 0.);
    app.editor.click(Point::new(4., 0.), 0.);
    let id = app.editor.project.elements[0].id;
    assert_eq!(app.editor.project.elements.len(), 1);

    // Lock the wall's layer.
    let mut layers = app.editor.project.layers.clone();
    layers.layer_mut(ARCHICAD_LAYER).unwrap().status.locked = true;
    app.editor.set_layer_document(layers, &[]);

    app.editor.set_selection(vec![id]);
    app.editor.delete();
    assert_eq!(
        app.editor.project.elements.len(),
        1,
        "elements on locked layers cannot be deleted"
    );
    assert!(app.editor.status.contains("locked"));

    // New walls cannot be placed on the locked layer either.
    app.editor.click(Point::new(10., 0.), 0.);
    app.editor.click(Point::new(14., 0.), 0.);
    assert_eq!(
        app.editor.project.elements.len(),
        1,
        "placement on a locked layer is rejected"
    );

    // Undo restores the unlocked layer document.
    app.editor.undo();
    assert!(!app.editor.project.layers.is_locked(ARCHICAD_LAYER));
}
#[test]
fn document_title_uses_file_name_and_untitled_before_first_save() {
    let (_ctx, mut app) = setup();
    app.language = UiLanguage::English;
    assert_eq!(app.document_title(), "Untitled");
    app.language = UiLanguage::SimplifiedChinese;
    assert_eq!(app.document_title(), "未命名");
    app.language = UiLanguage::English;
    app.path = "/tmp/院子.openarchi.json".into();
    assert_eq!(
        app.document_title(),
        "Untitled",
        "a suggested path alone must not claim a document name"
    );
    app.path_confirmed = true;
    assert_eq!(app.document_title(), "院子");
    app.path = "/tmp/plain.json".into();
    assert_eq!(app.document_title(), "plain");
    app.path = "/tmp/no-extension".into();
    assert_eq!(app.document_title(), "no-extension");
    app.path = "/tmp/.openarchi.json".into();
    assert_eq!(app.document_title(), "Untitled");
}

#[test]
fn file_display_name_strips_openarchi_json_suffixes() {
    assert_eq!(
        file_display_name("/a/b/院子.openarchi.json"),
        Some("院子".into())
    );
    assert_eq!(file_display_name("plain.json"), Some("plain".into()));
    assert_eq!(
        file_display_name("marker.openarchi.json"),
        Some("marker".into())
    );
    assert_eq!(file_display_name("none"), Some("none".into()));
    // `Path::file_name` normalizes a trailing separator to the directory name.
    assert_eq!(file_display_name("/dir/"), Some("dir".into()));
    assert_eq!(file_display_name(""), None);
}

#[test]
fn edit_menu_numeric_apply_cancel_and_bilingual_labels() {
    let (ctx, mut app) = setup();
    let id = app.editor.project.add(
        0,
        Material::Plaster,
        Shape::Wall {
            start: Point::new(0., 0.),
            end: Point::new(4., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    app.editor.set_selection(vec![id]);
    let before = app.editor.project.clone();
    app.dispatch_edit("Multiply…");
    layers_frame_output(&ctx, &mut app, vec![]);
    let output = layers_frame_output(&ctx, &mut app, vec![]);
    assert!(text_center(&output.shapes, "Displacement X").is_some());
    let cancel = text_center(&output.shapes, "Cancel").unwrap();
    layers_click(&ctx, &mut app, cancel);
    assert!(app.edit_dialog.is_none());
    assert_eq!(app.editor.project, before);
    app.language = UiLanguage::SimplifiedChinese;
    app.dispatch_edit("Multiply…");
    layers_frame_output(&ctx, &mut app, vec![]);
    let output = layers_frame_output(&ctx, &mut app, vec![]);
    assert!(text_center(&output.shapes, "X 位移").is_some());
    let ok = text_center(&output.shapes, "确定").unwrap();
    layers_click(&ctx, &mut app, ok);
    assert!(app.edit_dialog.is_none());
    assert_eq!(app.editor.undo.len(), 1);
    let Shape::Wall { start, .. } = app
        .editor
        .project
        .elements
        .iter()
        .find(|e| e.id == id)
        .unwrap()
        .shape
    else {
        panic!()
    };
    assert_eq!(start.x, 0.);
    app.editor.undo();
    assert_eq!(app.editor.project, before);
}
#[test]
fn edit_menu_catalog_has_translations_and_never_opens_unsupported_preview() {
    let (_ctx, mut app) = setup();
    for &(group, items) in edit_menu::EDIT_GROUPS {
        if !group.is_empty() {
            assert_ne!(
                UiLanguage::SimplifiedChinese.translate(group),
                group,
                "{group}"
            );
        }
        for &name in items {
            assert_ne!(
                UiLanguage::SimplifiedChinese.translate(name),
                name,
                "{name}"
            );
            if !edit_menu::implemented(name) {
                app.open_command_preview(name.into());
                assert!(app.command_dialog.is_none(), "{name}");
                assert!(app.edit_dialog.is_none());
            }
        }
    }
}
#[test]
fn edit_menu_escape_and_language_switch_preserve_model_and_history() {
    let (ctx, mut app) = setup();
    let id = app.editor.project.add(
        0,
        Material::Plaster,
        Shape::Wall {
            start: Point::default(),
            end: Point::new(4., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    app.editor.set_selection(vec![id]);
    app.dispatch_edit("Rotate");
    let before = app.editor.project.clone();
    let ids = app.editor.selection_ids();
    let undo = app.editor.undo.len();
    for language in [
        UiLanguage::English,
        UiLanguage::SimplifiedChinese,
        UiLanguage::English,
    ] {
        app.language = language;
        layers_frame_output(&ctx, &mut app, vec![]);
        assert_eq!(app.editor.project, before);
        assert_eq!(app.editor.selection_ids(), ids);
        assert_eq!(app.editor.undo.len(), undo);
    }
    layers_frame_output(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.edit_dialog.is_none());
    assert_eq!(app.editor.project, before);
}

fn edit_shortcut_frame(ctx: &Context, app: &mut AtelierApp, k: Key, m: Modifiers) {
    layers_frame_output(
        ctx,
        app,
        vec![
            key(k, m),
            Event::Key {
                key: k,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: m,
            },
        ],
    );
}
fn shortcut_test_wall(app: &mut AtelierApp) -> u64 {
    let id = app.editor.project.add(
        0,
        Material::Plaster,
        Shape::Wall {
            start: Point::new(0., 0.),
            end: Point::new(4., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    app.editor.set_selection(vec![id]);
    id
}
#[test]
fn edit_shortcuts_match_installed_ac29_standard_configuration() {
    let evidence: serde_json::Value = serde_json::from_str(include_str!(
        "../../docs/features/edit-menu/standard-shortcuts-29.json"
    ))
    .unwrap();
    for row in evidence["bindings"].as_array().unwrap() {
        assert!(
            edit_shortcuts::EDIT_SHORTCUTS
                .iter()
                .any(|b| b.command == row["command"].as_str().unwrap()
                    && format!("{:?}", b.key) == row["key"].as_str().unwrap()
                    && u64::from(b.modifiers) == row["modifiers"].as_u64().unwrap()),
            "{row}"
        );
    }
    for (i, a) in edit_shortcuts::EDIT_SHORTCUTS.iter().enumerate() {
        for b in &edit_shortcuts::EDIT_SHORTCUTS[i + 1..] {
            assert!(
                a.key != b.key || a.modifiers != b.modifiers,
                "duplicate chord: {}",
                a.command
            );
        }
    }
}
#[test]
fn edit_shortcuts_all_bindings_dispatch_like_the_menu() {
    for binding in edit_shortcuts::EDIT_SHORTCUTS {
        let (ctx, mut keyboard) = setup();
        let (_, mut menu) = setup();
        for app in [&mut keyboard, &mut menu] {
            shortcut_test_wall(app);
            app.editor.copy_selection();
            app.editor.edit(crate::editing::EditCommand::Lock).unwrap();
            app.editor
                .edit(crate::editing::EditCommand::Unlock)
                .unwrap();
        }
        let mods = Modifiers {
            command: binding.modifiers & 1 != 0,
            alt: binding.modifiers & 2 != 0,
            shift: binding.modifiers & 4 != 0,
            ..Modifiers::NONE
        };
        edit_shortcut_frame(&ctx, &mut keyboard, binding.key, mods);
        menu.dispatch_edit(binding.command);
        assert_eq!(
            keyboard.editor.project, menu.editor.project,
            "{}",
            binding.command
        );
        assert_eq!(
            keyboard.editor.selection_ids(),
            menu.editor.selection_ids(),
            "{}",
            binding.command
        );
        assert_eq!(
            keyboard.editor.undo.len(),
            menu.editor.undo.len(),
            "{}",
            binding.command
        );
        assert_eq!(
            keyboard.editor.redo.len(),
            menu.editor.redo.len(),
            "{}",
            binding.command
        );
        assert_eq!(
            keyboard.edit_dialog, menu.edit_dialog,
            "{}",
            binding.command
        );
        assert_eq!(keyboard.pick_armed, menu.pick_armed, "{}", binding.command);
        assert_eq!(
            keyboard.inject_armed, menu.inject_armed,
            "{}",
            binding.command
        );
    }
}
#[test]
fn edit_shortcuts_drag_is_pointer_driven_without_dialog_and_cancel_is_clean() {
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app) = setup();
        app.language = language;
        shortcut_test_wall(&mut app);
        let before = app.editor.project.clone();
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(400., 300.))]);
        edit_shortcut_frame(&ctx, &mut app, Key::D, Modifiers::COMMAND);
        assert!(app.edit_dialog.is_none());
        assert!(app.pointer_drag.is_some());
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(500., 300.))]);
        assert_eq!(app.editor.project, before);
        assert!(app.editor.undo.is_empty());
        edit_shortcut_frame(&ctx, &mut app, Key::Escape, Modifiers::NONE);
        assert!(app.pointer_drag.is_none());
        assert_eq!(app.editor.project, before);
    }
}
#[test]
fn edit_shortcuts_pointer_drag_commits_at_click_and_undo_restores() {
    let (ctx, mut app) = setup();
    shortcut_test_wall(&mut app);
    let before = app.editor.project.clone();
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(400., 300.))]);
    app.dispatch_edit("Drag");
    click(&ctx, &mut app, pos2(400., 300.));
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(500., 300.))]);
    click(&ctx, &mut app, pos2(500., 300.));
    assert!(app.pointer_drag.is_none());
    assert!(app.edit_dialog.is_none());
    assert_eq!(app.editor.undo.len(), 1);
    assert_eq!(app.editor.project.elements.len(), 1);
    let Shape::Wall { start, .. } = app.editor.project.elements[0].shape else {
        panic!()
    };
    assert!((start.x - 2.).abs() < 0.001);
    app.editor.undo();
    assert_eq!(app.editor.project, before);
}

#[test]
fn edit_shortcuts_exact_modifiers_unavailable_commands_and_repeat_are_safe() {
    let (ctx, mut app) = setup();
    shortcut_test_wall(&mut app);
    let before = app.editor.project.clone();
    edit_shortcut_frame(&ctx, &mut app, Key::T, Modifiers::COMMAND | Modifiers::ALT);
    assert!(app.wall_settings_dialog.is_none());
    assert!(app.element_settings.is_none());
    edit_shortcut_frame(
        &ctx,
        &mut app,
        Key::D,
        Modifiers::COMMAND | Modifiers::SHIFT,
    );
    assert!(app.edit_dialog.is_none());
    assert_eq!(app.editor.project, before);
    layers_frame_output(&ctx, &mut app, vec![key(Key::R, Modifiers::COMMAND)]);
    let rotated = app.editor.project.clone();
    let repeated = Event::Key {
        key: Key::R,
        physical_key: None,
        pressed: true,
        repeat: true,
        modifiers: Modifiers::COMMAND,
    };
    layers_frame_output(&ctx, &mut app, vec![repeated]);
    assert_eq!(app.editor.project, rotated);
    assert_eq!(app.editor.undo.len(), 1);
    app.editor.set_selection(vec![]);
    edit_shortcut_frame(&ctx, &mut app, Key::D, Modifiers::COMMAND);
    assert!(app.edit_dialog.is_none());
}
#[test]
fn edit_shortcuts_select_all_respects_visibility_story_and_marquee() {
    let (ctx, mut app) = setup();
    let id = shortcut_test_wall(&mut app);
    let other = app.editor.project.add(
        1,
        Material::Plaster,
        Shape::Wall {
            start: Point::default(),
            end: Point::new(3., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    app.editor.set_selection(vec![]);
    edit_shortcut_frame(&ctx, &mut app, Key::A, Modifiers::COMMAND);
    assert_eq!(app.editor.selection_ids(), vec![id]);
    assert!(!app.editor.selection_ids().contains(&other));
    assert!(app.editor.undo.is_empty());
    app.editor.set_tool(Tool::Marquee);
    app.editor.marquee = MarqueeArea::new(
        vec![
            Point::new(10., 10.),
            Point::new(12., 10.),
            Point::new(12., 12.),
            Point::new(10., 12.),
        ],
        MarqueeScope::CurrentStory,
        0,
    );
    edit_shortcut_frame(&ctx, &mut app, Key::A, Modifiers::COMMAND);
    assert!(app.editor.selection_ids().is_empty());
}
#[test]
fn edit_shortcuts_do_not_edit_behind_modals_or_focused_text() {
    let (ctx, mut app) = setup();
    shortcut_test_wall(&mut app);
    let before = app.editor.project.clone();
    app.command_dialog = Some("Test".into());
    edit_shortcut_frame(&ctx, &mut app, Key::R, Modifiers::COMMAND);
    assert_eq!(app.editor.project, before);
    app.command_dialog = None;
    let mut text = String::from("Editable");
    let field = Id::new("shortcut_focus_test");
    let _ = ctx.run_ui(RawInput::default(), |ui| {
        ui.add(TextEdit::singleline(&mut text).id(field))
            .request_focus();
    });
    let _ = ctx.run_ui(
        RawInput {
            events: vec![key(Key::D, Modifiers::COMMAND)],
            ..Default::default()
        },
        |ui| {
            app.shortcuts(ui.ctx());
            ui.add(TextEdit::singleline(&mut text).id(field));
        },
    );
    assert!(app.edit_dialog.is_none());
    assert_eq!(app.editor.project, before);
    assert!(app.editor.undo.is_empty());
}
#[test]
fn edit_shortcuts_bilingual_menu_hints_use_the_same_binding() {
    let (_, mut app) = setup();
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        app.language = language;
        assert!(app.edit_menu_label("Drag").contains("⌘D"));
        assert!(app.edit_menu_label("Redo").contains("⌥⌘Z"));
        assert!(app.edit_menu_label("Rotate a Copy").contains("⌥⌘R"));
        assert!(!app.edit_menu_label("Autogroup").contains('⌘'));
        assert!(
            app.edit_menu_label("Drag")
                .starts_with(language.translate("Drag"))
        );
    }
}

#[test]
fn edit_shortcuts_handle_winit_clipboard_events_and_inject_modifiers() {
    let (ctx, mut app) = setup();
    let id = shortcut_test_wall(&mut app);
    let output = layers_frame_output(
        &ctx,
        &mut app,
        vec![Event::ModifiersChanged(Modifiers::COMMAND), Event::Copy],
    );
    assert_eq!(app.editor.clipboard.len(), 1);
    assert!(
        output
            .platform_output
            .commands
            .iter()
            .any(|c| matches!(c,egui::OutputCommand::CopyText(text) if text=="OpenArchi elements"))
    );
    layers_frame_output(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(Modifiers::COMMAND),
            Event::Paste("OpenArchi elements".into()),
        ],
    );
    assert_eq!(app.editor.project.elements.len(), 2);
    assert_eq!(app.editor.undo.len(), 1);
    app.editor.set_selection(vec![id]);
    app.pick_up_command();
    let clipboard = app.editor.clipboard.clone();
    app.editor.set_selection(vec![]);
    layers_frame_output(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(Modifiers::COMMAND | Modifiers::ALT),
            Event::Copy,
        ],
    );
    assert!(app.inject_armed);
    assert_eq!(app.editor.clipboard, clipboard);
}

#[test]
fn pointer_drag_option_copy_latches_and_multiple_clicks_preserve_original() {
    let (ctx, mut app) = setup();
    shortcut_test_wall(&mut app);
    let original = app.editor.project.clone();
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(400., 300.))]);
    app.dispatch_edit("Drag");
    click(&ctx, &mut app, pos2(400., 300.));
    plan_frame(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(Modifiers::ALT),
            Event::PointerMoved(pos2(500., 300.)),
        ],
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::ModifiersChanged(Modifiers::NONE)],
    );
    assert_eq!(app.editor.project, original);
    click(&ctx, &mut app, pos2(500., 300.));
    assert!(app.pointer_drag.is_none());
    assert_eq!(app.editor.project.elements.len(), 2);
    assert_eq!(app.editor.project.elements[0], original.elements[0]);
    assert_eq!(app.editor.undo.len(), 1);
    app.editor.undo();
    app.editor.set_selection(vec![original.elements[0].id]);
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(400., 300.))]);
    app.dispatch_edit("Drag");
    click(&ctx, &mut app, pos2(400., 300.));
    plan_frame(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(Modifiers::ALT | Modifiers::COMMAND),
            Event::PointerMoved(pos2(500., 300.)),
        ],
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::ModifiersChanged(Modifiers::NONE)],
    );
    click(&ctx, &mut app, pos2(500., 300.));
    click(&ctx, &mut app, pos2(600., 300.));
    assert!(app.pointer_drag.is_some());
    assert_eq!(app.editor.project.elements.len(), 3);
    assert_eq!(app.editor.undo.len(), 2);
    let saved = serde_json::to_string(&app.editor.project).unwrap();
    assert_eq!(
        serde_json::from_str::<Project>(&saved).unwrap(),
        app.editor.project
    );
    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.pointer_drag.is_none());
    assert_eq!(app.editor.project.elements.len(), 3);
    assert_eq!(app.editor.selection_ids().len(), 3);
    assert_eq!(
        UiLanguage::SimplifiedChinese.translate(&app.editor.status),
        "就绪"
    );
    app.editor.undo();
    assert_eq!(app.editor.selection_ids().len(), 2);
    app.editor.undo();
    assert_eq!(app.editor.selection_ids().len(), 1);
    assert_eq!(app.editor.project, original);
}

#[test]
fn pointer_drag_locked_selection_does_not_commit() {
    let (ctx, mut app) = setup();
    shortcut_test_wall(&mut app);
    app.editor.edit(crate::editing::EditCommand::Lock).unwrap();
    let original = app.editor.project.clone();
    let history = app.editor.undo.len();
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(400., 300.))]);
    app.dispatch_edit("Drag");
    click(&ctx, &mut app, pos2(400., 300.));
    click(&ctx, &mut app, pos2(500., 300.));
    assert_eq!(app.editor.project, original);
    assert_eq!(app.editor.undo.len(), history);
}

#[test]
fn pointer_drag_3d_uses_current_story_plane_without_dialog() {
    let (ctx, mut app) = setup();
    shortcut_test_wall(&mut app);
    let original = app.editor.project.clone();
    model_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(400., 300.))]);
    app.dispatch_edit("Drag");
    click_model(&ctx, &mut app, pos2(400., 300.), PointerButton::Primary);
    model_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(500., 300.))]);
    assert_eq!(app.editor.project, original);
    assert!(app.edit_dialog.is_none());
    click_model(&ctx, &mut app, pos2(500., 300.), PointerButton::Primary);
    assert!(app.pointer_drag.is_none());
    assert_eq!(app.editor.undo.len(), 1);
    assert_ne!(app.editor.project, original);
    app.editor.undo();
    assert_eq!(app.editor.project, original);
}

#[test]
fn pointer_drag_reference_point_is_first_click_not_previous_mouse_position() {
    let (ctx, mut app) = setup();
    shortcut_test_wall(&mut app);
    let before = app.editor.project.clone();
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(200., 200.))]);
    app.dispatch_edit("Drag");
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(450., 350.))]);
    assert_eq!(app.editor.status, "Enter Drag Reference Point");
    click(&ctx, &mut app, pos2(450., 350.));
    assert_eq!(app.editor.project, before);
    assert!(app.editor.undo.is_empty());
    assert_eq!(app.editor.status, "Enter Drag-to Point");
    click(&ctx, &mut app, pos2(500., 350.));
    let Shape::Wall { start, .. } = app.editor.project.elements[0].shape else {
        panic!()
    };
    assert!((start.x - 1.).abs() < 0.001);
    app.editor.undo();
    assert_eq!(app.editor.project, before);
}

#[test]
fn pointer_drag_copy_chords_toggle_out_and_control_alone_does_not_copy() {
    for exit_chord in [Modifiers::ALT, Modifiers::ALT | Modifiers::COMMAND] {
        let (ctx, mut app) = setup();
        shortcut_test_wall(&mut app);
        app.dispatch_edit("Drag");
        click(&ctx, &mut app, pos2(400., 300.));
        for modifiers in [
            Modifiers::ALT | Modifiers::COMMAND,
            Modifiers::NONE,
            exit_chord,
            Modifiers::NONE,
            Modifiers::CTRL,
            Modifiers::NONE,
        ] {
            plan_frame(&ctx, &mut app, vec![Event::ModifiersChanged(modifiers)]);
        }
        assert_eq!(app.editor.status, "Enter Drag-to Point");
        click(&ctx, &mut app, pos2(500., 300.));
        assert_eq!(app.editor.project.elements.len(), 1);
        assert_eq!(app.editor.undo.len(), 1);
    }
}

#[test]
fn pointer_drag_reference_and_copy_statuses_are_bilingual() {
    for (key, chinese) in [
        ("Enter Drag Reference Point", "指定移动基点"),
        ("Enter Drag-to Point", "指定移动终点"),
        (
            "Enter Drag-to Point · Drag a Copy",
            "指定移动终点 · 移动副本",
        ),
        (
            "Enter Drag-to Point · Drag Multiple Copies",
            "指定移动终点 · 多重复制",
        ),
    ] {
        assert_eq!(UiLanguage::English.translate(key), key);
        assert_eq!(UiLanguage::SimplifiedChinese.translate(key), chinese);
    }
}

#[test]
fn pointer_drag_quick_option_tap_in_one_frame_is_not_lost() {
    let (ctx, mut app) = setup();
    shortcut_test_wall(&mut app);
    app.dispatch_edit("Drag");
    click(&ctx, &mut app, pos2(400., 300.));
    plan_frame(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(Modifiers::ALT),
            Event::ModifiersChanged(Modifiers::NONE),
        ],
    );
    assert_eq!(app.editor.status, "Enter Drag-to Point · Drag a Copy");
    click(&ctx, &mut app, pos2(500., 300.));
    assert_eq!(app.editor.project.elements.len(), 2);
}

fn begin_direct_drag(ctx: &Context, app: &mut AtelierApp, start: Pos2, end: Pos2) {
    plan_frame(ctx, app, vec![Event::PointerMoved(start)]);
    plan_frame(
        ctx,
        app,
        vec![Event::PointerButton {
            pos: start,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    plan_frame(ctx, app, vec![Event::PointerMoved(end)]);
}
fn release_direct_drag(ctx: &Context, app: &mut AtelierApp, pos: Pos2) {
    plan_frame(
        ctx,
        app,
        vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
}
#[test]
fn direct_drag_held_option_then_command_uses_copy_session_without_eyedropper() {
    for selected in [false, true] {
        let (ctx, mut app) = setup();
        let id = shortcut_test_wall(&mut app);
        if !selected {
            app.editor.set_selection(vec![]);
        }
        if let Shape::Wall { thickness, .. } = &mut app.editor.project.elements[0].shape {
            *thickness = 1.;
        }
        let original = app.editor.project.clone();
        begin_direct_drag(&ctx, &mut app, pos2(500., 320.), pos2(550., 370.));
        assert!(app.pointer_drag.is_some());
        assert!(app.edit_dialog.is_none());
        assert_eq!(app.editor.selection_ids(), vec![id]);
        assert_eq!(app.editor.status, "Enter Drag-to Point");
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::ModifiersChanged(Modifiers::ALT)],
        );
        assert!(app.editor.status.ends_with("Drag a Copy"));
        assert!(app.transfer_cursor.is_none());
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::ModifiersChanged(Modifiers::ALT | Modifiers::COMMAND)],
        );
        assert!(app.editor.status.ends_with("Drag Multiple Copies"));
        assert!(app.transfer_cursor.is_none());
        assert_eq!(app.editor.project, original);
        release_direct_drag(&ctx, &mut app, pos2(550., 370.));
        assert!(app.pointer_drag.is_some());
        assert_eq!(app.editor.project, original);
        assert!(app.editor.undo.is_empty());
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::ModifiersChanged(Modifiers::NONE)],
        );
        click(&ctx, &mut app, pos2(600., 320.));
        click(&ctx, &mut app, pos2(700., 320.));
        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        assert!(app.pointer_drag.is_none());
        assert_eq!(app.editor.project.elements.len(), 3);
        assert_eq!(app.editor.selection_ids().len(), 3);
        assert_eq!(app.editor.project.elements[0], original.elements[0]);
        assert_eq!(app.editor.undo.len(), 2);
        for (element, x) in app.editor.project.elements[1..].iter().zip([2., 4.]) {
            match element.shape {
                Shape::Wall {
                    start: a, end: b, ..
                } => {
                    assert_eq!(a, Point::new(x, 0.));
                    assert_eq!(b, Point::new(x + 4., 0.));
                }
                _ => panic!("wall"),
            }
        }
        let restored: Project =
            serde_json::from_str(&serde_json::to_string(&app.editor.project).unwrap()).unwrap();
        assert_eq!(restored, app.editor.project);
        app.editor.undo();
        assert_eq!(app.editor.project.elements.len(), 2);
        app.editor.undo();
        assert_eq!(app.editor.project, original);
    }
}
#[test]
fn direct_drag_move_copy_cancel_and_locked_commit() {
    for copy in [false, true] {
        let (ctx, mut app) = setup();
        shortcut_test_wall(&mut app);
        if let Shape::Wall { thickness, .. } = &mut app.editor.project.elements[0].shape {
            *thickness = 1.;
        }
        let original = app.editor.project.clone();
        begin_direct_drag(&ctx, &mut app, pos2(500., 320.), pos2(550., 370.));
        if copy {
            plan_frame(
                &ctx,
                &mut app,
                vec![Event::ModifiersChanged(Modifiers::ALT)],
            );
        }
        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        release_direct_drag(&ctx, &mut app, pos2(550., 370.));
        assert!(app.pointer_drag.is_none());
        assert_eq!(app.editor.project, original);
        assert!(app.editor.undo.is_empty());
        begin_direct_drag(&ctx, &mut app, pos2(500., 320.), pos2(550., 370.));
        if copy {
            plan_frame(
                &ctx,
                &mut app,
                vec![Event::ModifiersChanged(Modifiers::ALT)],
            );
        }
        release_direct_drag(&ctx, &mut app, pos2(550., 370.));
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::ModifiersChanged(Modifiers::NONE)],
        );
        click(&ctx, &mut app, pos2(600., 320.));
        assert!(app.pointer_drag.is_none());
        assert_eq!(app.editor.project.elements.len(), if copy { 2 } else { 1 });
        assert_eq!(app.editor.undo.len(), 1);
        app.editor.undo();
        assert_eq!(app.editor.project, original);
    }
    let (ctx, mut app) = setup();
    shortcut_test_wall(&mut app);
    app.editor.edit(crate::editing::EditCommand::Lock).unwrap();
    if let Shape::Wall { thickness, .. } = &mut app.editor.project.elements[0].shape {
        *thickness = 1.;
    }
    let original = app.editor.project.clone();
    let history = app.editor.undo.len();
    begin_direct_drag(&ctx, &mut app, pos2(500., 320.), pos2(550., 370.));
    release_direct_drag(&ctx, &mut app, pos2(550., 370.));
    click(&ctx, &mut app, pos2(600., 320.));
    assert_eq!(app.editor.project, original);
    assert_eq!(app.editor.undo.len(), history);
}

#[test]
fn cmd_drag_object_reference_press_then_motion_is_not_lost() {
    for mode in 0..3 {
        let (ctx, mut app) = setup();
        let id = shortcut_test_wall(&mut app);
        let before = app.editor.project.clone();
        // Exercise the real workspace, including the selection Info Box and palettes.
        layers_frame_output(&ctx, &mut app, vec![]);
        layers_frame_output(&ctx, &mut app, vec![key(Key::D, Modifiers::COMMAND)]);
        let start = pos2(735., 538.); // wall midpoint in the 1400x900 workspace
        layers_frame_output(&ctx, &mut app, vec![Event::PointerMoved(start)]);
        layers_frame_output(
            &ctx,
            &mut app,
            vec![Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            }],
        );
        layers_frame_output(&ctx, &mut app, vec![Event::PointerMoved(pos2(785., 538.))]);
        assert_eq!(
            app.editor.status, "Enter Drag-to Point",
            "reference press must survive motion"
        );
        assert!(app.wall_edit.is_none() && app.wall_insert.is_none());
        if mode > 0 {
            layers_frame_output(
                &ctx,
                &mut app,
                vec![Event::ModifiersChanged(Modifiers::ALT)],
            );
        }
        if mode == 2 {
            layers_frame_output(
                &ctx,
                &mut app,
                vec![Event::ModifiersChanged(Modifiers::ALT | Modifiers::COMMAND)],
            );
        }
        layers_frame_output(
            &ctx,
            &mut app,
            vec![
                Event::PointerButton {
                    pos: pos2(785., 538.),
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
                Event::ModifiersChanged(Modifiers::NONE),
            ],
        );
        assert_eq!(
            app.editor.project, before,
            "reference release cannot commit"
        );
        assert!(app.editor.undo.is_empty());
        assert!(app.transfer_cursor.is_none());
        // Place on the original wall's endpoint, not on empty canvas.
        layers_click(&ctx, &mut app, pos2(835., 538.));
        assert_eq!(app.editor.undo.len(), 1);
        if mode == 2 {
            layers_click(&ctx, &mut app, pos2(635., 538.));
            layers_frame_output(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
            assert_eq!(app.editor.project.elements.len(), 3);
            assert_eq!(app.editor.selection_ids().len(), 3);
            app.editor.undo();
        } else {
            assert!(app.pointer_drag.is_none());
            assert_eq!(
                app.editor.project.elements.len(),
                if mode == 0 { 1 } else { 2 }
            );
        }
        let moved = &app.editor.project.elements[if mode == 0 { 0 } else { 1 }];
        assert!(
            matches!(moved.shape, Shape::Wall {start,end,..} if start==Point::new(2.,0.) && end==Point::new(6.,0.))
        );
        if mode > 0 {
            assert_eq!(app.editor.project.elements[0], before.elements[0]);
        }
        app.editor.undo();
        assert_eq!(app.editor.project, before);
        assert_eq!(
            app.editor.selection_ids(),
            if mode == 1 { vec![] } else { vec![id] }
        );
    }
}

#[test]
fn cmd_drag_reference_accepts_any_canvas_point_and_consumes_jitter_release() {
    for start in [
        pos2(400., 300.),
        pos2(500., 300.),
        pos2(500., 400.),
        pos2(300., 200.),
    ] {
        let (ctx, mut app) = setup();
        let id = shortcut_test_wall(&mut app);
        let other = app.editor.project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(0., -2.),
                end: Point::new(4., -2.),
                thickness: 0.2,
                height: 3.,
            },
        );
        let before = app.editor.project.clone();
        plan_frame(&ctx, &mut app, vec![key(Key::D, Modifiers::COMMAND)]);
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(start)]);
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_eq!(app.editor.status, "Enter Drag-to Point");
        let jitter = start + vec2(1., 0.);
        plan_frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(jitter),
                Event::PointerButton {
                    pos: jitter,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        assert_eq!(
            app.editor.project, before,
            "a one-pixel release must not commit"
        );
        assert_eq!(app.editor.selection_ids(), vec![id]);
        assert!(app.editor.undo.is_empty());
        assert!(app.wall_edit.is_none() && app.wall_insert.is_none());
        click(&ctx, &mut app, start + vec2(50., 0.));
        assert!(app.pointer_drag.is_none());
        assert_eq!(
            wall_reference_line(&app, id),
            (Point::new(1., 0.), Point::new(5., 0.))
        );
        assert_eq!(
            wall_reference_line(&app, other),
            (Point::new(0., -2.), Point::new(4., -2.))
        );
        app.editor.undo();
        assert_eq!(app.editor.project, before);
    }
}

#[test]
fn orbit_crosses_horizon_with_rotation_cursor_and_preserves_model() {
    for button in [PointerButton::Primary, PointerButton::Middle] {
        let (ctx, mut app) = setup();
        shortcut_test_wall(&mut app);
        let project = app.editor.project.clone();
        let selection = app.editor.selection_ids();
        app.camera.pitch = 0.05;
        let modifiers = if button == PointerButton::Middle {
            Modifiers::SHIFT
        } else {
            Modifiers::NONE
        };
        model_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(400., 400.))]);
        model_frame(
            &ctx,
            &mut app,
            vec![
                Event::ModifiersChanged(modifiers),
                Event::PointerButton {
                    pos: pos2(400., 400.),
                    button,
                    pressed: true,
                    modifiers,
                },
            ],
        );
        let output = model_frame_output(
            &ctx,
            &mut app,
            vec![
                Event::ModifiersChanged(modifiers),
                Event::PointerMoved(pos2(450., 300.)),
            ],
        );
        assert!(app.camera.pitch < 0., "must rotate below the horizon");
        assert_eq!(output.platform_output.cursor_icon, CursorIcon::None);
        assert!(app.transfer_cursor.is_none());
        let held = model_frame_output(&ctx, &mut app, vec![Event::ModifiersChanged(modifiers)]);
        assert_eq!(
            held.platform_output.cursor_icon,
            CursorIcon::None,
            "retain orbit glyph while paused"
        );
        assert!(app.camera.matrix(4. / 3.).is_finite());
        for y in [-1000., 2000.] {
            model_frame(
                &ctx,
                &mut app,
                vec![
                    Event::ModifiersChanged(modifiers),
                    Event::PointerMoved(pos2(450., y)),
                ],
            );
            assert!(app.camera.matrix(4. / 3.).is_finite());
            assert!((-1.53..=1.53).contains(&app.camera.pitch));
        }
        let released = model_frame_output(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos2(450., 300.)),
                Event::PointerButton {
                    pos: pos2(450., 300.),
                    button,
                    pressed: false,
                    modifiers,
                },
                Event::ModifiersChanged(Modifiers::NONE),
            ],
        );
        assert_ne!(released.platform_output.cursor_icon, CursorIcon::None);
        assert_eq!(app.editor.project, project);
        assert_eq!(app.editor.selection_ids(), selection);
        assert!(app.editor.undo.is_empty());
    }
}

fn drag_tracker_type(ctx: &Context, app: &mut AtelierApp, field: Key, value: &str) {
    plan_frame(
        ctx,
        app,
        vec![key(field, Modifiers::NONE), Event::Text(value.into())],
    );
}
#[test]
fn drag_tracker_distance_angle_enters_all_copy_modes_and_undo() {
    for (mode, lang) in [
        (0, UiLanguage::English),
        (1, UiLanguage::SimplifiedChinese),
        (2, UiLanguage::English),
    ] {
        let (ctx, mut app) = setup();
        app.language = lang;
        let id = shortcut_test_wall(&mut app);
        let before = app.editor.project.clone();
        app.dispatch_edit("Drag");
        click(&ctx, &mut app, pos2(400., 300.));
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(500., 300.))]);
        drag_tracker_type(&ctx, &mut app, Key::D, "5");
        drag_tracker_type(&ctx, &mut app, Key::A, "90");
        if mode > 0 {
            plan_frame(
                &ctx,
                &mut app,
                vec![Event::ModifiersChanged(Modifiers::ALT)],
            );
        }
        if mode == 2 {
            plan_frame(
                &ctx,
                &mut app,
                vec![Event::ModifiersChanged(Modifiers::ALT | Modifiers::COMMAND)],
            );
        }
        assert_eq!(app.editor.project, before);
        assert!(app.editor.undo.is_empty());
        assert!(app.transfer_cursor.is_none());
        let output = plan_frame_output(
            &ctx,
            &mut app,
            vec![Event::ModifiersChanged(Modifiers::NONE)],
        );
        let mut labels = vec![];
        collect_text(&output.shapes, &mut labels);
        assert!(
            labels
                .iter()
                .any(|t| t.contains(if lang == UiLanguage::English {
                    "Distance [D]"
                } else {
                    "距离 [D]"
                }))
        );
        assert!(
            labels
                .iter()
                .any(|t| t.contains(if lang == UiLanguage::English {
                    "Angle [A]"
                } else {
                    "角度 [A]"
                }))
        );
        plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
        let element = &app.editor.project.elements[if mode == 0 { 0 } else { 1 }];
        assert!(
            matches!(element.shape,Shape::Wall {start,..} if start.distance(Point::new(0.,5.))<1e-5)
        );
        if mode > 0 {
            assert_eq!(app.editor.project.elements[0], before.elements[0]);
        }
        assert_eq!(app.editor.undo.len(), 1);
        if mode == 2 {
            drag_tracker_type(&ctx, &mut app, Key::D, "3");
            drag_tracker_type(&ctx, &mut app, Key::A, "-90");
            plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
            assert_eq!(app.editor.project.elements.len(), 3);
            assert!(
                matches!(app.editor.project.elements[2].shape,Shape::Wall {start,..} if start.distance(Point::new(0.,-3.))<1e-5)
            );
            plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
            assert_eq!(app.editor.selection_ids().len(), 3);
            app.editor.undo();
        }
        assert!(app.pointer_drag.is_none());
        app.editor.undo();
        assert_eq!(app.editor.project, before);
        assert!(app.editor.project.elements.iter().any(|e| e.id == id));
    }
}
#[test]
fn drag_tracker_units_tab_invalid_and_escape() {
    let (ctx, mut app) = setup();
    shortcut_test_wall(&mut app);
    app.editor.project.plan_options.dimension_style = DimensionStyle::PlainMillimeter;
    let before = app.editor.project.clone();
    app.dispatch_edit("Drag");
    click(&ctx, &mut app, pos2(400., 300.));
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(500., 300.))]);
    drag_tracker_type(&ctx, &mut app, Key::D, "2500");
    plan_frame(
        &ctx,
        &mut app,
        vec![key(Key::Tab, Modifiers::NONE), Event::Text("0".into())],
    );
    plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
    assert_eq!(wall_reference_line(&app, 1).0, Point::new(2.5, 0.));
    let roundtrip: Project =
        serde_json::from_str(&serde_json::to_string(&app.editor.project).unwrap()).unwrap();
    assert_eq!(roundtrip, app.editor.project);
    app.editor.undo();
    assert_eq!(app.editor.project, before);
    app.dispatch_edit("Drag");
    click(&ctx, &mut app, pos2(400., 300.));
    for value in ["-1", "NaN", "1e99", "bad"] {
        drag_tracker_type(&ctx, &mut app, Key::D, "1");
        plan_frame(
            &ctx,
            &mut app,
            vec![key(Key::A, Modifiers::COMMAND), Event::Paste(value.into())],
        );
        plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
        assert_eq!(app.editor.project, before);
        assert!(app.editor.undo.is_empty());
        assert!(app.pointer_drag.is_some());
        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        assert!(
            app.pointer_drag.is_some(),
            "first Esc cancels only numeric input"
        );
    }
    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.pointer_drag.is_none());
}

#[test]
fn drag_tracker_error_labels_and_3d_numeric_placement() {
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app) = setup();
        app.language = language;
        shortcut_test_wall(&mut app);
        app.dispatch_edit("Drag");
        click_model(&ctx, &mut app, pos2(400., 300.), PointerButton::Primary);
        model_frame(
            &ctx,
            &mut app,
            vec![key(Key::D, Modifiers::NONE), Event::Text("-2".into())],
        );
        let output = model_frame_output(&ctx, &mut app, vec![]);
        let mut labels = vec![];
        collect_text(&output.shapes, &mut labels);
        assert!(
            labels
                .iter()
                .any(|t| t == language.translate("Enter a valid number"))
        );
        model_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
        assert!(app.editor.undo.is_empty());
        model_frame(
            &ctx,
            &mut app,
            vec![key(Key::A, Modifiers::COMMAND), Event::Text("2".into())],
        );
        model_frame(
            &ctx,
            &mut app,
            vec![key(Key::A, Modifiers::NONE), Event::Text("0".into())],
        );
        model_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
        assert!(app.pointer_drag.is_none());
        assert_eq!(wall_reference_line(&app, 1).0, Point::new(2., 0.));
        assert_eq!(app.editor.undo.len(), 1);
    }
}

#[test]
fn active_wall_method_switch_retains_nodes_through_info_box_clicks() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.editor.click(Point::new(0., 0.), 0.);
    for (label, method) in [
        ("Continuous", WallGeometryMethod::Continuous),
        ("Single", WallGeometryMethod::Single),
    ] {
        layers_frame_output(&ctx, &mut app, vec![]);
        let output = layers_frame_output(&ctx, &mut app, vec![]);
        let button = text_center(&output.shapes, label).expect("method button");
        layers_click(&ctx, &mut app, button);
        assert_eq!(app.editor.wall_settings.geometry.geometry_method, method);
        assert!(app.editor.wall_draft_active());
        assert!(app.editor.project.elements.is_empty());
        assert!(app.editor.undo.is_empty());
        if method == WallGeometryMethod::Continuous {
            app.editor.click(Point::new(4., 0.), 0.);
        }
    }
    assert_eq!(
        app.editor.wall_points,
        vec![Point::new(0., 0.), Point::new(4., 0.)]
    );
    let expected = vec![
        (Point::new(0., 0.), Point::new(4., 0.)),
        (Point::new(4., 0.), Point::new(4., 3.)),
    ];
    assert_eq!(app.wall_preview_segments(Point::new(4., 3.), 0.), expected);
    app.editor.click(Point::new(4., 3.), 0.);
    let actual: Vec<_> = app
        .editor
        .project
        .elements
        .iter()
        .map(|e| match e.shape {
            Shape::Wall { start, end, .. } => (start, end),
            _ => panic!("expected wall"),
        })
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(app.editor.project.elements.len(), 2);
    assert_eq!(app.editor.undo.len(), 1);
    app.editor.undo();
    assert!(app.editor.project.elements.is_empty());
    app.editor.redo();
    assert_eq!(app.editor.project.elements.len(), 2);
}

#[test]
fn active_wall_method_switch_rectangle_and_cancel_preserve_transaction_boundary() {
    let (_, mut app) = setup();
    app.editor.set_tool(Tool::Wall);
    app.editor
        .set_wall_geometry_method(WallGeometryMethod::Continuous);
    app.editor.click(Point::new(0., 0.), 0.);
    app.editor.click(Point::new(4., 0.), 0.);
    app.editor
        .set_wall_geometry_method(WallGeometryMethod::Rectangular);
    app.editor.finish_wall(Some(Point::new(7., 3.))).unwrap();
    assert_eq!(app.editor.project.elements.len(), 5);
    assert_eq!(app.editor.undo.len(), 1);
    app.editor.undo();
    app.editor.set_tool(Tool::Wall);
    app.editor.click(Point::new(1., 1.), 0.);
    app.editor
        .set_wall_geometry_method(WallGeometryMethod::Continuous);
    app.editor.click(Point::new(5., 1.), 0.);
    app.editor
        .set_wall_geometry_method(WallGeometryMethod::Single);
    app.editor.cancel_wall_draft();
    assert!(!app.editor.wall_draft_active());
    assert!(app.editor.project.elements.is_empty());
    assert!(app.editor.undo.is_empty());
}

fn reshape_test_line(app: &mut AtelierApp, a: (f32, f32), b: (f32, f32)) -> u64 {
    app.editor.project.add(
        0,
        Material::Concrete,
        Shape::Polyline {
            points: vec![Point::new(a.0, a.1), Point::new(b.0, b.1)],
            arcs: vec![],
        },
    )
}
#[test]
fn reshape_toolbar_click_trim_preview_cancel_commit_both_languages() {
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app) = setup();
        app.language = language;
        app.view = View::Plan;
        reshape_test_line(&mut app, (-4., 0.), (4., 0.));
        reshape_test_line(&mut app, (-1., -2.), (-1., 2.));
        reshape_test_line(&mut app, (1., -2.), (1., 2.));
        let before = app.editor.project.clone();
        layers_frame_output(&ctx, &mut app, vec![]);
        layers_frame_output(&ctx, &mut app, vec![]);
        let rect = ctx
            .data(|d| d.get_temp::<Rect>(Id::new(("reshape_button", "Trim"))))
            .expect("toolbar button rect");
        layers_click(&ctx, &mut app, rect.center());
        assert_eq!(app.reshape_session.as_ref().unwrap().name, "Trim");
        assert_eq!(app.editor.project, before);
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(400., 300.))]);
        assert_eq!(app.editor.project, before);
        assert!(app.editor.undo.is_empty());
        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        assert!(app.reshape_session.is_none());
        assert_eq!(app.editor.project, before);
        app.dispatch_edit("Trim");
        click(&ctx, &mut app, pos2(400., 300.));
        assert_eq!(app.reshape_session.as_ref().unwrap().name, "Trim");
        assert_eq!(app.editor.project.elements.len(), 4);
        assert_eq!(app.editor.undo.len(), 1);
        app.editor.undo();
        assert_eq!(app.editor.project, before);
        app.editor.redo();
        assert_eq!(app.editor.project.elements.len(), 4);
        assert_ne!(
            app.language
                .translate("Click the segment to trim · Escape cancels"),
            ""
        );
    }
}
#[test]
fn reshape_adjust_real_pointer_boundary_and_no_premature_commit() {
    let (ctx, mut app) = setup();
    app.view = View::Plan;
    let id = reshape_test_line(&mut app, (-3., 0.), (-1., 0.));
    app.editor.set_selection(vec![id]);
    let before = app.editor.project.clone();
    app.dispatch_edit("Adjust");
    click(&ctx, &mut app, pos2(500., 350.));
    assert_eq!(app.editor.project, before);
    assert!(app.editor.undo.is_empty());
    click(&ctx, &mut app, pos2(500., 250.));
    assert!(app.reshape_session.is_none());
    assert_eq!(app.editor.undo.len(), 1);
    let path = crate::reshape::straight_path(&app.editor.project.elements[0]).unwrap();
    assert_eq!(path[1], Point::new(2., 0.));
}
#[test]
fn reshape_resize_dialog_graphical_three_clicks_and_cancel() {
    let (ctx, mut app) = setup();
    app.view = View::Plan;
    let id = reshape_test_line(&mut app, (0., 0.), (2., 0.));
    app.editor.set_selection(vec![id]);
    let before = app.editor.project.clone();
    app.dispatch_edit("Resize…");
    layers_frame_output(&ctx, &mut app, vec![]);
    let output = layers_frame_output(&ctx, &mut app, vec![]);
    let ok = text_center(&output.shapes, "OK").unwrap();
    layers_click(&ctx, &mut app, ok);
    assert!(!app.reshape_session.as_ref().unwrap().dialog);
    assert_eq!(app.editor.project, before);
    click(&ctx, &mut app, pos2(400., 300.));
    click(&ctx, &mut app, pos2(500., 300.));
    assert_eq!(app.editor.project, before);
    assert!(app.editor.undo.is_empty());
    click(&ctx, &mut app, pos2(600., 300.));
    assert!(app.reshape_session.is_none());
    assert_eq!(
        crate::reshape::straight_path(&app.editor.project.elements[0]).unwrap()[1],
        Point::new(4., 0.)
    );
    assert_eq!(app.editor.undo.len(), 1);
}
#[test]
fn reshape_magic_wand_space_click_creates_without_selecting_source() {
    let (ctx, mut app) = setup();
    app.view = View::Plan;
    let id = app.editor.project.add(
        0,
        Material::Concrete,
        Shape::Slab {
            min: Point::new(-2., -2.),
            max: Point::new(2., 2.),
            thickness: 0.2,
        },
    );
    app.editor.set_tool(Tool::Wall);
    let before = app.editor.project.clone();
    plan_frame(
        &ctx,
        &mut app,
        vec![
            key(Key::Space, Modifiers::NONE),
            Event::PointerMoved(pos2(400., 300.)),
        ],
    );
    assert_eq!(app.editor.project, before);
    click(&ctx, &mut app, pos2(400., 300.));
    assert_eq!(app.editor.project.elements.len(), 5);
    assert_eq!(app.editor.project.elements[0].id, id);
    assert_eq!(app.editor.undo.len(), 1);
}
#[test]
fn reshape_eligibility_and_localization_do_not_mutate_project() {
    let (ctx, mut app) = setup();
    app.view = View::Plan;
    assert!(app.edit_command_enabled("Trim"));
    assert!(!app.edit_command_enabled("Resize…"));
    assert!(!app.edit_command_enabled("Magic Wand"));
    assert!(!app.edit_command_enabled("Trim Elements to Roof/Shell"));
    let id = reshape_test_line(&mut app, (0., 0.), (2., 0.));
    app.editor.set_selection(vec![id]);
    assert!(app.edit_command_enabled("Adjust"));
    assert!(!app.edit_command_enabled("Intersect"));
    app.editor.project.edit_state.locked.insert(id);
    assert!(!app.edit_command_enabled("Adjust"));
    app.editor.project.edit_state.locked.clear();
    let before = app.editor.project.clone();
    let selection = app.editor.selection_ids();
    for language in [UiLanguage::SimplifiedChinese, UiLanguage::English] {
        app.language = language;
        layers_frame_output(&ctx, &mut app, vec![]);
        assert_eq!(app.editor.project, before);
        assert_eq!(app.editor.selection_ids(), selection);
        assert!(app.editor.undo.is_empty());
    }
}

fn trim_cmd_frame(
    ctx: &Context,
    app: &mut AtelierApp,
    pos: Pos2,
    modifiers: Modifiers,
    pressed: Option<bool>,
) -> CursorIcon {
    let mut events = vec![Event::ModifiersChanged(modifiers), Event::PointerMoved(pos)];
    if let Some(pressed) = pressed {
        events.push(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers,
        });
    }
    plan_frame(ctx, app, events)
}
#[test]
fn trim_cmd_hold_release_preserves_tool_draft_selection_and_history() {
    for lang in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app) = setup();
        app.language = lang;
        app.view = View::Plan;
        let id = reshape_test_line(&mut app, (-4., 0.), (4., 0.));
        reshape_test_line(&mut app, (-1., -2.), (-1., 2.));
        reshape_test_line(&mut app, (1., -2.), (1., 2.));
        app.editor.set_selection(vec![id]);
        app.editor.set_tool(Tool::Wall);
        app.editor.click(Point::new(5., 5.), 0.);
        let before = app.editor.project.clone();
        let selection = app.editor.selection_ids();
        let status = app.editor.status.clone();
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(400., 300.))]);
        let cursor = trim_cmd_frame(
            &ctx,
            &mut app,
            pos2(400., 300.),
            (Modifiers::MAC_CMD | Modifiers::COMMAND),
            None,
        );
        assert_eq!(cursor, CursorIcon::None);
        assert!(app.trim_modifier_active(&ctx));
        assert!(app.reshape_session.is_none());
        assert_eq!(app.editor.project, before);
        assert_eq!(app.editor.selection_ids(), selection);
        assert!(app.editor.undo.is_empty());
        assert_eq!(app.editor.anchor, Some(Point::new(5., 5.)));
        assert_eq!(app.editor.tool, Tool::Wall);
        trim_cmd_frame(&ctx, &mut app, pos2(400., 300.), Modifiers::NONE, None);
        assert!(!app.trim_modifier_active(&ctx));
        assert!(app.reshape_session.is_none());
        assert_eq!(app.editor.project, before);
        assert_eq!(app.editor.status, status);
        assert_eq!(app.editor.anchor, Some(Point::new(5., 5.)));
        let tip = app.reshape_tip("Trim");
        assert!(tip.contains("Cmd"));
        if lang == UiLanguage::SimplifiedChinese {
            assert!(tip.contains("松开"));
        } else {
            assert!(tip.contains("release"));
        }
    }
}
#[test]
fn trim_cmd_click_commits_once_and_can_repeat_while_held() {
    let (ctx, mut app) = setup();
    app.view = View::Plan;
    reshape_test_line(&mut app, (-4., 0.), (4., 0.));
    reshape_test_line(&mut app, (-1., -2.), (-1., 2.));
    reshape_test_line(&mut app, (1., -2.), (1., 2.));
    let before = app.editor.project.clone();
    let pos = pos2(400., 300.);
    trim_cmd_frame(
        &ctx,
        &mut app,
        pos,
        (Modifiers::MAC_CMD | Modifiers::COMMAND),
        None,
    );
    trim_cmd_frame(
        &ctx,
        &mut app,
        pos,
        (Modifiers::MAC_CMD | Modifiers::COMMAND),
        Some(true),
    );
    assert_eq!(app.editor.project, before);
    assert!(app.editor.undo.is_empty());
    trim_cmd_frame(
        &ctx,
        &mut app,
        pos,
        (Modifiers::MAC_CMD | Modifiers::COMMAND),
        Some(false),
    );
    assert_eq!(app.editor.undo.len(), 1);
    assert_eq!(app.editor.project.elements.len(), 4);
    assert!(!app.trim_shortcut_gesture);
    assert!(app.reshape_session.is_none());
    assert!(app.pointer_drag.is_none());
    let after = app.editor.project.clone();
    app.editor.undo();
    assert_eq!(app.editor.project, before);
    app.editor.redo();
    assert_eq!(app.editor.project, after);
    // Same physical hold, second independent click; no latched menu command needed.
    let pos = pos2(350., 250.);
    trim_cmd_frame(
        &ctx,
        &mut app,
        pos,
        (Modifiers::MAC_CMD | Modifiers::COMMAND),
        Some(true),
    );
    trim_cmd_frame(
        &ctx,
        &mut app,
        pos,
        (Modifiers::MAC_CMD | Modifiers::COMMAND),
        Some(false),
    );
    assert_eq!(app.editor.undo.len(), 2);
}
#[test]
fn trim_cmd_released_before_mouse_up_does_not_draw_or_select() {
    let (ctx, mut app) = setup();
    app.view = View::Plan;
    reshape_test_line(&mut app, (-4., 0.), (4., 0.));
    reshape_test_line(&mut app, (-1., -2.), (-1., 2.));
    app.editor.set_tool(Tool::Wall);
    app.editor.click(Point::new(5., 5.), 0.);
    let before = app.editor.project.clone();
    let pos = pos2(400., 300.);
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos)]);
    trim_cmd_frame(
        &ctx,
        &mut app,
        pos,
        (Modifiers::MAC_CMD | Modifiers::COMMAND),
        Some(true),
    );
    assert!(app.trim_shortcut_gesture);
    trim_cmd_frame(&ctx, &mut app, pos, Modifiers::NONE, None);
    trim_cmd_frame(&ctx, &mut app, pos, Modifiers::NONE, Some(false));
    assert!(!app.trim_shortcut_gesture);
    assert!(app.editor.undo.is_empty());
    assert_eq!(app.editor.project, before);
    assert_eq!(app.editor.anchor, Some(Point::new(5., 5.)));
    trim_cmd_frame(&ctx, &mut app, pos, Modifiers::NONE, None);
    assert!(!app.trim_modifier_active(&ctx));
}
#[test]
fn trim_cmd_does_not_latch_outside_canvas_or_steal_modifier_chords() {
    let (ctx, mut app) = setup();
    app.view = View::Plan;
    trim_cmd_frame(
        &ctx,
        &mut app,
        pos2(900., 700.),
        (Modifiers::MAC_CMD | Modifiers::COMMAND),
        None,
    );
    assert!(app.reshape_session.is_none());
    trim_cmd_frame(&ctx, &mut app, pos2(400., 300.), Modifiers::NONE, None);
    assert!(!app.trim_modifier_active(&ctx));
    for modifiers in [
        Modifiers::CTRL,
        (Modifiers::MAC_CMD | Modifiers::COMMAND) | Modifiers::ALT,
        (Modifiers::MAC_CMD | Modifiers::COMMAND) | Modifiers::SHIFT,
        Modifiers {
            ctrl: true,
            mac_cmd: true,
            command: true,
            ..Modifiers::NONE
        },
    ] {
        trim_cmd_frame(&ctx, &mut app, pos2(400., 300.), modifiers, None);
        assert!(!app.trim_modifier_active(&ctx));
        assert!(app.editor.undo.is_empty());
    }
    trim_cmd_frame(&ctx, &mut app, pos2(400., 300.), Modifiers::NONE, None);
    app.dispatch_edit("Trim");
    trim_cmd_frame(
        &ctx,
        &mut app,
        pos2(400., 300.),
        (Modifiers::MAC_CMD | Modifiers::COMMAND),
        None,
    );
    trim_cmd_frame(&ctx, &mut app, pos2(400., 300.), Modifiers::NONE, None);
    assert_eq!(app.reshape_session.as_ref().unwrap().name, "Trim");
}
#[test]
fn trim_cmd_locked_target_is_atomic() {
    let (ctx, mut app) = setup();
    app.view = View::Plan;
    let id = reshape_test_line(&mut app, (-4., 0.), (4., 0.));
    reshape_test_line(&mut app, (-1., -2.), (-1., 2.));
    app.editor.project.edit_state.locked.insert(id);
    let before = app.editor.project.clone();
    let pos = pos2(400., 300.);
    trim_cmd_frame(
        &ctx,
        &mut app,
        pos,
        (Modifiers::MAC_CMD | Modifiers::COMMAND),
        Some(true),
    );
    trim_cmd_frame(
        &ctx,
        &mut app,
        pos,
        (Modifiers::MAC_CMD | Modifiers::COMMAND),
        Some(false),
    );
    assert_eq!(app.editor.project, before);
    assert!(app.editor.undo.is_empty());
    assert!(app.reshape_session.is_none());
}

#[test]
fn trim_cmd_and_explicit_mode_repeat_nearest_intervals_until_escape() {
    for explicit in [false, true] {
        let (ctx, mut app) = setup();
        app.view = View::Plan;
        let id = reshape_test_line(&mut app, (-4., 0.), (4., 0.));
        for x in [-2., 0., 2.] {
            reshape_test_line(&mut app, (x, -2.), (x, 2.));
        }
        let before = app.editor.project.clone();
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(250., 300.))]);
        if explicit {
            app.dispatch_edit("Trim");
        }
        let mods = if explicit {
            Modifiers::NONE
        } else {
            (Modifiers::MAC_CMD | Modifiers::COMMAND)
        };
        for (step, x) in [250., 350., 450., 550.].into_iter().enumerate() {
            let pos = pos2(x, 300.);
            trim_cmd_frame(&ctx, &mut app, pos, mods, Some(true));
            trim_cmd_frame(&ctx, &mut app, pos, mods, Some(false));
            assert_eq!(app.editor.undo.len(), step + 1);
            assert_eq!(app.reshape_session.is_some(), explicit);
            if step < 3 {
                let wall = app
                    .editor
                    .project
                    .elements
                    .iter()
                    .find(|e| e.id == id)
                    .unwrap();
                assert_eq!(
                    crate::reshape::straight_path(wall).unwrap(),
                    vec![Point::new(-2. + step as f32 * 2., 0.), Point::new(4., 0.)]
                );
            } else {
                assert!(!app.editor.project.elements.iter().any(|e| e.id == id));
            }
        }
        trim_cmd_frame(&ctx, &mut app, pos2(550., 300.), Modifiers::NONE, None);
        if explicit {
            plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        }
        assert!(app.reshape_session.is_none());
        let after = app.editor.project.clone();
        for _ in 0..4 {
            app.editor.undo();
        }
        assert_eq!(app.editor.project, before);
        for _ in 0..4 {
            app.editor.redo();
        }
        assert_eq!(app.editor.project, after);
    }
}

#[test]
fn trim_cmd_hold_does_not_steal_undo_shortcut() {
    let (ctx, mut app) = setup();
    app.view = View::Plan;
    reshape_test_line(&mut app, (-4., 0.), (4., 0.));
    reshape_test_line(&mut app, (-1., -2.), (-1., 2.));
    let before = app.editor.project.clone();
    let pos = pos2(400., 300.);
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos)]);
    trim_cmd_frame(
        &ctx,
        &mut app,
        pos,
        (Modifiers::MAC_CMD | Modifiers::COMMAND),
        Some(true),
    );
    trim_cmd_frame(
        &ctx,
        &mut app,
        pos,
        (Modifiers::MAC_CMD | Modifiers::COMMAND),
        Some(false),
    );
    assert_eq!(app.editor.undo.len(), 1);
    plan_frame(
        &ctx,
        &mut app,
        vec![key(Key::Z, (Modifiers::MAC_CMD | Modifiers::COMMAND))],
    );
    assert_eq!(app.editor.project, before);
    assert!(app.trim_modifier_active(&ctx));
}

#[test]
fn wall_node_rotated_plan_palette_preview_commit_cancel_both_languages() {
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app) = setup();
        app.language = language;
        app.view = View::Plan;
        let id = app.editor.project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(-3., -2.),
                end: Point::new(3., 2.),
                thickness: 0.2,
                height: 3.,
            },
        );
        app.editor.project.plan_options.orientation_degrees = 27.;
        app.editor.set_selection(vec![id]);
        let viewport = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
        let before = app.editor.project.clone();
        let midpoint = app.screen(viewport, Point::default());
        click(&ctx, &mut app, midpoint);
        assert!(app.wall_insert.is_some());
        plan_frame(&ctx, &mut app, vec![]);
        let button = ctx
            .data(|d| d.get_temp::<Rect>(Id::new("wall_insert_button")))
            .unwrap();
        click(&ctx, &mut app, button.center());
        assert!(app.wall_insert.is_some());
        let node = Point::new(-2., 2.);
        let target = app.screen(viewport, node);
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(target)]);
        assert_eq!(app.editor.project, before);
        assert!(app.editor.undo.is_empty());
        let out = plan_frame_output(&ctx, &mut app, vec![]);
        let mut labels = Vec::new();
        collect_text(&out.shapes, &mut labels);
        assert!(labels.iter().any(|s| s == language.translate("Distance")));
        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        assert!(app.wall_insert.is_none());
        assert_eq!(app.editor.project, before);
        click(&ctx, &mut app, midpoint);
        plan_frame(&ctx, &mut app, vec![]);
        let button = ctx
            .data(|d| d.get_temp::<Rect>(Id::new("wall_insert_button")))
            .unwrap();
        click(&ctx, &mut app, button.center());
        click(&ctx, &mut app, target);
        assert!(app.wall_insert.is_none());
        assert_eq!(app.editor.undo.len(), 1);
        let paths: Vec<_> = app
            .editor
            .project
            .elements
            .iter()
            .map(|e| crate::reshape::straight_path(e).unwrap())
            .collect();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0][0], Point::new(-3., -2.));
        assert_eq!(paths[1][1], Point::new(3., 2.));
        assert!(paths[0][1].distance(node) < 0.001);
        assert_eq!(paths[0][1], paths[1][0]);
        let after = app.editor.project.clone();
        app.editor.undo();
        assert_eq!(app.editor.project, before);
        app.editor.redo();
        assert_eq!(app.editor.project, after);
    }
}
fn wall_model_click(ctx: &Context, app: &mut AtelierApp, pos: Pos2) {
    model_frame(ctx, app, vec![Event::PointerMoved(pos)]);
    model_frame(
        ctx,
        app,
        vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    model_frame(
        ctx,
        app,
        vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
}
#[test]
fn wall_node_model_orbit_then_middle_click_and_place_keeps_camera_fixed() {
    let (ctx, mut app) = setup();
    app.view = View::Model;
    let id = app.editor.project.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: Point::new(-3., -1.),
            end: Point::new(3., 1.),
            thickness: 0.2,
            height: 3.,
        },
    );
    app.editor.set_selection(vec![id]);
    app.camera.target = glam::Vec3::new(0., 0., 1.5);
    app.camera.span = 12.;
    let viewport = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
    // A real orbit gesture precedes edge hit-testing.
    model_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(200., 200.))]);
    model_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: pos2(200., 200.),
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    model_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(230., 215.))]);
    model_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: pos2(230., 215.),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    let camera = (app.camera.yaw, app.camera.pitch);
    let midpoint = app
        .wall_project_point(viewport, Point::default(), 3.)
        .unwrap();
    wall_model_click(&ctx, &mut app, midpoint);
    assert!(app.wall_insert.is_some());
    assert_eq!(
        app.editor.project.elements.len(),
        1,
        "initiating click must not commit"
    );
    model_frame(&ctx, &mut app, vec![]);
    let button = ctx
        .data(|d| d.get_temp::<Rect>(Id::new("wall_insert_button")))
        .unwrap();
    wall_model_click(&ctx, &mut app, button.center());
    assert!(app.wall_insert.is_some());
    let before = app.editor.project.clone();
    let node = Point::new(-1., -2.);
    let target = app.wall_project_point(viewport, node, 3.).unwrap();
    model_frame(&ctx, &mut app, vec![Event::PointerMoved(target)]);
    assert_eq!(app.editor.project, before);
    wall_model_click(&ctx, &mut app, target);
    assert_eq!(app.editor.undo.len(), 1);
    assert_eq!((app.camera.yaw, app.camera.pitch), camera);
    let paths: Vec<_> = app
        .editor
        .project
        .elements
        .iter()
        .map(|e| crate::reshape::straight_path(e).unwrap())
        .collect();
    assert!(
        paths[0][1].distance(node) < 0.002,
        "actual {:?}, expected {:?}, tracker {:?}, target {:?}",
        paths[0][1],
        node,
        ctx.data(|d| d.get_temp::<Rect>(Id::new("wall_pet_tracker_rect"))),
        target
    );
    assert_eq!(paths[0][1], paths[1][0]);
}

#[test]
fn wall_pet_ten_icons_switch_without_commit_in_both_languages() {
    use super::wall_node_ui::WallPetCommand;
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app) = setup();
        app.language = language;
        app.view = View::Plan;
        let id = app.editor.project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(-3., 0.),
                end: Point::new(3., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        app.editor.set_selection(vec![id]);
        let before = app.editor.project.clone();
        let viewport = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
        click(&ctx, &mut app, app_screen_point(viewport));
        plan_frame(&ctx, &mut app, vec![]);
        for command in WallPetCommand::ALL {
            let rect = ctx
                .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", command as u8))))
                .unwrap();
            click(&ctx, &mut app, rect.center());
            assert_eq!(app.wall_insert.as_ref().unwrap().command, command);
            assert_eq!(app.editor.project, before);
            assert!(app.editor.undo.is_empty());
            plan_frame(&ctx, &mut app, vec![]);
        }
        plan_frame(&ctx, &mut app, vec![key(Key::P, Modifiers::NONE)]);
        assert_eq!(
            app.wall_insert.as_ref().unwrap().command,
            WallPetCommand::InsertNode
        );
        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        assert!(app.wall_insert.is_none());
        assert_eq!(app.editor.project, before);
    }
    fn app_screen_point(viewport: Rect) -> Pos2 {
        viewport.center()
    }
}

#[test]
fn wall_pet_graphical_steps_commit_once_and_undo_exactly() {
    use super::wall_node_ui::WallPetCommand as C;
    for command in [
        C::InsertNode,
        C::Curve,
        C::Tangent,
        C::OuterSide,
        C::TrapezoidNode,
        C::Drag,
        C::Rotate,
        C::Mirror,
        C::Multiply,
    ] {
        let (ctx, mut app) = setup();
        app.view = View::Plan;
        let id = app.editor.project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(-3., 0.),
                end: Point::new(3., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        app.editor.set_selection(vec![id]);
        let before = app.editor.project.clone();
        let viewport = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
        click(&ctx, &mut app, viewport.center());
        plan_frame(&ctx, &mut app, vec![]);
        let rect = ctx
            .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", command as u8))))
            .unwrap();
        click(&ctx, &mut app, rect.center());
        let points = match command {
            C::OuterSide => vec![Point::new(-3., 1.), Point::new(3., 1.5)],
            C::Rotate => vec![
                Point::new(-3., 0.),
                Point::new(-3., 2.),
                Point::new(-5., 0.),
            ],
            C::Mirror => vec![Point::new(-4., -1.), Point::new(-4., 1.)],
            _ => vec![Point::new(-1., 2.)],
        };
        for (i, point) in points.iter().enumerate() {
            let target = app.screen(viewport, *point);
            for _ in 0..3 {
                plan_frame(&ctx, &mut app, vec![]);
            }
            click(&ctx, &mut app, target);
            if i + 1 < points.len() {
                assert_eq!(app.editor.project, before, "{command:?} staged");
                assert!(app.editor.undo.is_empty());
            }
        }
        assert!(
            app.wall_insert.is_none(),
            "{command:?}: {}",
            app.editor.status
        );
        assert_eq!(app.editor.undo.len(), 1, "{command:?}");
        assert_ne!(app.editor.project, before);
        app.editor.project.validate().unwrap();
        app.editor.undo();
        assert_eq!(app.editor.project, before, "{command:?} undo");
    }
}

#[test]
fn wall_pet_default_pencil_hover_is_visual_and_click_locks_command() {
    use super::wall_node_ui::WallPetCommand as C;
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app) = setup();
        app.language = language;
        let id = add_wall_for_editing(&mut app);
        app.editor.set_selection(vec![id]);
        app.editor.project.plan_options.orientation_degrees = 31.;
        let viewport = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
        let before = app.editor.project.clone();
        let baseline = app.screen(viewport, Point::new(2., 0.));
        click(&ctx, &mut app, baseline);
        assert_eq!(app.wall_insert.as_ref().unwrap().command, C::InsertNode);
        plan_frame(&ctx, &mut app, vec![]);
        let cursor = || {
            ctx.data(|d| d.get_temp::<Option<CursorFeedback>>(Id::new("wall_pet_cursor")))
                .flatten()
        };
        assert_eq!(
            cursor(),
            Some(CursorFeedback::SolidPencil),
            "palette {:?}, tracker {:?}, pointer {:?}",
            app.wall_edit_palette_rect,
            ctx.data(|d| d.get_temp::<Rect>(Id::new("wall_pet_tracker_rect"))),
            ctx.input(|i| i.pointer.hover_pos())
        );
        // Still inside the thick wall, but farther than 4 screen points from its baseline.
        let outside = app.screen(viewport, Point::new(1., 0.12));
        let output = plan_frame_output(&ctx, &mut app, vec![Event::PointerMoved(outside)]);
        assert_eq!(output.platform_output.cursor_icon, CursorIcon::None);
        assert_eq!(cursor(), Some(CursorFeedback::HollowPencil));
        assert_eq!(app.editor.project, before);
        assert!(app.editor.undo.is_empty());
        let on_line = app.screen(viewport, Point::new(1., 0.));
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(on_line)]);
        assert_eq!(
            cursor(),
            Some(CursorFeedback::SolidPencil),
            "palette {:?}, tracker {:?}, pointer {:?}",
            app.wall_edit_palette_rect,
            ctx.data(|d| d.get_temp::<Rect>(Id::new("wall_pet_tracker_rect"))),
            ctx.input(|i| i.pointer.hover_pos())
        );
        for command in C::ALL {
            let rect = ctx
                .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", command as u8))))
                .unwrap();
            let active = app.wall_insert.as_ref().unwrap().command;
            plan_frame(&ctx, &mut app, vec![Event::PointerMoved(rect.center())]);
            let output = plan_frame_output(&ctx, &mut app, vec![]);
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Rect(painted) if painted.rect==rect && (painted.fill.to_srgba_unmultiplied()[0] as i16 - if active==command {24}else{42}).abs()<=2 && painted.fill.a()>0)), "candidate icon must have its appropriate visual state: {command:?}");
            assert_eq!(
                app.wall_insert.as_ref().unwrap().command,
                active,
                "browsing must preserve active command"
            );
            assert_eq!(cursor(), None, "palette should use the normal pointer");
            plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(250., 450.))]);
            assert_eq!(
                app.wall_insert.as_ref().unwrap().command,
                active,
                "returning to canvas must retain the clicked command"
            );
            click(&ctx, &mut app, rect.center());
            assert_eq!(app.wall_insert.as_ref().unwrap().command, command);
            assert_eq!(app.editor.project, before);
            assert!(app.editor.undo.is_empty());
        }
        let rect = ctx
            .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", C::Curve as u8))))
            .unwrap();
        click(&ctx, &mut app, rect.center());
        let target = app.screen(viewport, Point::new(1., 2.));
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(target)]);
        assert_eq!(app.wall_insert.as_ref().unwrap().command, C::Curve);
        assert_eq!(cursor(), Some(CursorFeedback::HollowPencil));
        click(&ctx, &mut app, target);
        assert!(matches!(
            app.editor.project.elements[0].wall_contour,
            Some(crate::model::WallContour::Arc { .. })
        ));
        assert_eq!(app.editor.undo.len(), 1);
        app.editor.undo();
        assert_eq!(app.editor.project, before);
    }
}

#[test]
fn wall_pet_hovering_active_rotation_preserves_accepted_points() {
    use super::wall_node_ui::WallPetCommand as C;
    let (ctx, mut app) = setup();
    let id = add_wall_for_editing(&mut app);
    app.editor.set_selection(vec![id]);
    let before = app.editor.project.clone();
    click(&ctx, &mut app, pos2(500., 300.));
    plan_frame(&ctx, &mut app, vec![]);
    let rotate = ctx
        .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", C::Rotate as u8))))
        .unwrap();
    click(&ctx, &mut app, rotate.center());
    for point in [pos2(400., 300.), pos2(400., 200.), pos2(300., 300.)] {
        let other = ctx
            .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", C::Curve as u8))))
            .unwrap();
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(other.center())]);
        let rect = ctx
            .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", C::Rotate as u8))))
            .unwrap();
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(rect.center())]);
        for _ in 0..3 {
            plan_frame(&ctx, &mut app, vec![]);
        }
        assert_eq!(app.editor.project, before);
        click(&ctx, &mut app, point);
    }
    assert!(app.wall_insert.is_none());
    assert_eq!(app.editor.undo.len(), 1);
    let (a, b) = wall_reference_line(&app, id);
    assert!(a.distance(Point::default()) < 1e-4);
    assert!(b.distance(Point::new(0., 4.)) < 1e-4);
}

#[test]
fn wall_pet_curved_original_path_and_projected_model_pencils() {
    use crate::model::WallContour;
    let (ctx, mut app) = setup();
    let id = add_wall_for_editing(&mut app);
    app.editor.set_selection(vec![id]);
    app.editor.project.elements[0].wall_contour = Some(WallContour::Arc {
        arc: geometry::arc_segment_through(
            Point::new(0., 0.),
            Point::new(4., 0.),
            Point::new(2., 2.),
        )
        .unwrap(),
    });
    let viewport = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
    let curve_point = app.screen(viewport, Point::new(2., 2.));
    click(&ctx, &mut app, curve_point);
    plan_frame(&ctx, &mut app, vec![]);
    let cursor = || {
        ctx.data(|d| d.get_temp::<Option<CursorFeedback>>(Id::new("wall_pet_cursor")))
            .flatten()
    };
    assert_eq!(
        cursor(),
        Some(CursorFeedback::SolidPencil),
        "palette {:?}, tracker {:?}, pointer {:?}",
        app.wall_edit_palette_rect,
        ctx.data(|d| d.get_temp::<Rect>(Id::new("wall_pet_tracker_rect"))),
        ctx.input(|i| i.pointer.hover_pos())
    );
    let chord = app.screen(viewport, Point::new(1., 0.));
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(chord)]);
    assert_eq!(cursor(), Some(CursorFeedback::HollowPencil));
    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    app.view = View::Model;
    app.camera.target = glam::Vec3::new(2., 1., 1.5);
    app.camera.span = 12.;
    let edge = app
        .wall_project_point(viewport, Point::new(2., 2.), 3.)
        .unwrap();
    wall_model_click(&ctx, &mut app, edge);
    model_frame(&ctx, &mut app, vec![]);
    assert!(app.wall_insert.is_some());
    assert!(app.editor.undo.is_empty());
    assert_eq!(
        cursor(),
        Some(CursorFeedback::SolidPencil),
        "palette {:?}, tracker {:?}, pointer {:?}",
        app.wall_edit_palette_rect,
        ctx.data(|d| d.get_temp::<Rect>(Id::new("wall_pet_tracker_rect"))),
        ctx.input(|i| i.pointer.hover_pos())
    );
    model_frame(
        &ctx,
        &mut app,
        vec![Event::PointerMoved(edge - vec2(50., 80.))],
    );
    assert_eq!(cursor(), Some(CursorFeedback::HollowPencil));
    assert!(app.editor.undo.is_empty());
}

#[test]
fn wall_pet_compact_tracker_numeric_enter_and_drag_release() {
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app) = setup();
        app.language = language;
        let id = add_wall_for_editing(&mut app);
        app.editor.set_selection(vec![id]);
        let original = app.editor.project.clone();
        click(&ctx, &mut app, pos2(500., 300.));
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(450., 180.))]);
        let out = plan_frame_output(&ctx, &mut app, vec![]);
        let palette = app.wall_edit_palette_rect.unwrap();
        assert!(
            palette.width() <= 150. && palette.height() <= 66.,
            "{palette:?}"
        );
        let tracker = ctx
            .data(|d| d.get_temp::<Rect>(Id::new("wall_pet_tracker_rect")))
            .unwrap();
        assert!(
            tracker.width() <= 180. && tracker.height() <= 60.,
            "{tracker:?}"
        );
        assert!(!tracker.intersects(palette));
        let mut labels = Vec::new();
        collect_text(&out.shapes, &mut labels);
        for absent in [
            "Apply",
            "Cancel",
            "Insert node",
            "Move pointer to preview · Click or Enter to apply · Esc cancel",
        ] {
            assert!(
                !labels.iter().any(|s| s == language.translate(absent)),
                "{labels:?}"
            );
        }
        let distance = ctx
            .data(|d| d.get_temp::<Rect>(Id::new("wall_pet_distance")))
            .unwrap();
        click(&ctx, &mut app, distance.center());
        plan_frame(&ctx, &mut app, vec![]);
        assert!(ctx.egui_wants_keyboard_input());
        assert_eq!(
            app.editor.project, original,
            "field release must not commit"
        );
        plan_frame(
            &ctx,
            &mut app,
            vec![key(Key::A, Modifiers::COMMAND), Event::Text("2".into())],
        );
        // Browse another candidate, then return through the active command: numeric locks survive.
        for command in [
            super::wall_node_ui::WallPetCommand::Curve,
            super::wall_node_ui::WallPetCommand::InsertNode,
        ] {
            let icon = ctx
                .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", command as u8))))
                .unwrap();
            plan_frame(&ctx, &mut app, vec![Event::PointerMoved(icon.center())]);
        }
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(350., 150.))]);
        let angle = ctx
            .data(|d| d.get_temp::<Rect>(Id::new("wall_pet_angle")))
            .unwrap();
        click(&ctx, &mut app, angle.center());
        plan_frame(&ctx, &mut app, vec![]);
        plan_frame(
            &ctx,
            &mut app,
            vec![
                key(Key::A, Modifiers::COMMAND),
                Event::Text("90".into()),
                key(Key::Enter, Modifiers::NONE),
            ],
        );
        assert!(app.wall_insert.is_none(), "{}", app.editor.status);
        assert_eq!(app.editor.undo.len(), 1);
        let end = wall_reference_line(&app, id).1;
        assert!(end.distance(Point::new(2., 2.)) < 0.001, "{end:?}");
        app.editor.undo();
        assert_eq!(app.editor.project, original);
        app.editor.set_selection(vec![id]);
        click(&ctx, &mut app, pos2(500., 300.));
        begin_direct_drag(&ctx, &mut app, pos2(500., 300.), pos2(450., 180.));
        assert_eq!(app.editor.project, original, "held preview is private");
        release_direct_drag(&ctx, &mut app, pos2(450., 180.));
        assert!(app.wall_insert.is_none());
        assert_eq!(app.editor.undo.len(), 1);
        assert!(
            wall_reference_line(&app, id)
                .1
                .distance(Point::new(1., 2.4))
                < 0.001
        );
    }
}

#[test]
fn wall_pet_model_drag_release_and_palette_release_do_not_orbit_or_miscommit() {
    use super::wall_node_ui::WallPetCommand as C;
    let (ctx, mut app) = setup();
    let id = app.editor.project.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: Point::new(-3., -1.),
            end: Point::new(3., 1.),
            thickness: 0.3,
            height: 3.,
        },
    );
    app.editor.set_selection(vec![id]);
    app.camera.target = glam::Vec3::new(0., 0., 1.5);
    app.camera.span = 12.;
    let viewport = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
    let original = app.editor.project.clone();
    let camera = (app.camera.yaw, app.camera.pitch);
    let start = app
        .wall_project_point(viewport, Point::default(), 3.)
        .unwrap();
    let node = Point::new(-2., 2.);
    let end = app.wall_project_point(viewport, node, 3.).unwrap();
    model_frame(&ctx, &mut app, vec![Event::PointerMoved(start)]);
    model_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: start,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    model_frame(&ctx, &mut app, vec![Event::PointerMoved(end)]);
    assert_eq!(app.editor.project, original);
    model_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: end,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert!(app.wall_insert.is_none());
    assert_eq!(app.editor.undo.len(), 1);
    assert!(wall_reference_line(&app, id).1.distance(node) < 0.002);
    assert_eq!((app.camera.yaw, app.camera.pitch), camera);
    app.editor.undo();
    app.editor.set_selection(vec![id]);
    wall_model_click(&ctx, &mut app, start);
    model_frame(&ctx, &mut app, vec![]);
    model_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: start,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    let icon = ctx
        .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", C::Curve as u8))))
        .unwrap()
        .center();
    model_frame(&ctx, &mut app, vec![Event::PointerMoved(icon)]);
    assert_eq!(app.wall_insert.as_ref().unwrap().command, C::InsertNode);
    model_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: icon,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert!(app.wall_insert.is_some());
    assert_eq!(app.editor.project, original);
    assert!(app.editor.undo.is_empty());
    model_frame(&ctx, &mut app, vec![Event::PointerMoved(end)]);
    assert_eq!(app.wall_insert.as_ref().unwrap().command, C::InsertNode);
    wall_model_click(&ctx, &mut app, icon);
    assert_eq!(app.wall_insert.as_ref().unwrap().command, C::Curve);
    model_frame(&ctx, &mut app, vec![Event::PointerMoved(end)]);
    assert_eq!(app.wall_insert.as_ref().unwrap().command, C::Curve);
    assert_eq!(app.editor.project, original);
}

fn junction_fixture(app: &mut AtelierApp, count: usize) -> Vec<u64> {
    [
        Point::new(-4., 0.),
        Point::new(4., 0.),
        Point::new(0., 4.),
        Point::new(0., -4.),
    ]
    .into_iter()
    .take(count)
    .map(|end| {
        app.editor.project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::default(),
                end,
                thickness: 0.2,
                height: 3.,
            },
        )
    })
    .collect()
}
#[test]
fn wall_junction_plan_drag_all_selected_ends_and_cancel_in_both_languages() {
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        for count in [2, 3, 4] {
            let (ctx, mut app) = setup();
            app.language = language;
            let ids = junction_fixture(&mut app, count);
            app.editor.set_selection(ids.clone());
            app.editor.project.plan_options.orientation_degrees = 29.;
            let before = app.editor.project.clone();
            let r = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
            let start = app.screen(r, Point::default());
            let point = Point::new(-1., 2.);
            let end = app.screen(r, point);
            begin_direct_drag(&ctx, &mut app, start, end);
            assert_eq!(app.editor.project, before);
            assert!(app.wall_insert.is_some());
            assert!(app.editor.undo.is_empty());
            release_direct_drag(&ctx, &mut app, end);
            assert!(app.wall_insert.is_none(), "{}", app.editor.status);
            assert_eq!(app.editor.undo.len(), 1);
            assert_eq!(app.editor.project.elements.len(), count);
            for (i, id) in ids.iter().enumerate() {
                let (a, b) = wall_reference_line(&app, *id);
                assert!(a.distance(point) < 1e-5);
                assert_eq!(
                    b,
                    crate::geometry::wall_reference_line(&before.elements[i])
                        .unwrap()
                        .1
                );
            }
            app.editor.undo();
            assert_eq!(app.editor.project, before);
            app.editor.set_selection(ids.clone());
            click(&ctx, &mut app, start);
            assert!(app.wall_insert.is_some());
            assert_eq!(app.editor.status, "Move node");
            plan_frame(&ctx, &mut app, vec![Event::PointerMoved(end)]);
            plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
            assert_eq!(app.editor.project, before);
            assert!(app.editor.undo.is_empty());
        }
    }
}
#[test]
fn wall_junction_t_crossing_click_enter_preserves_unselected_and_unrelated_selection() {
    let (ctx, mut app) = setup();
    let ids = junction_fixture(&mut app, 3);
    // Join the two horizontal halves into one through-wall for a real T intersection.
    if let Shape::Wall { start, .. } = &mut app.editor.project.elements[1].shape {
        *start = Point::new(-4., 0.);
    }
    let unselected = app.editor.project.elements[0].clone();
    let extra = app.editor.project.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: Point::new(5., 5.),
            end: Point::new(7., 5.),
            thickness: 0.2,
            height: 3.,
        },
    );
    app.editor.set_selection(vec![ids[1], ids[2], extra]);
    let before = app.editor.project.clone();
    click(&ctx, &mut app, pos2(400., 300.));
    assert!(app.wall_insert.is_some());
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(350., 200.))]);
    assert_eq!(app.editor.project, before);
    plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
    assert_eq!(app.editor.project.elements.len(), 5);
    assert_eq!(app.editor.undo.len(), 1);
    assert_eq!(app.editor.project.elements[0], unselected);
    assert!(app.editor.selection_ids().contains(&extra));
    assert_eq!(
        wall_reference_line(&app, ids[1]),
        (Point::new(-4., 0.), Point::new(-1., 2.))
    );
    assert_eq!(
        wall_reference_line(&app, ids[2]),
        (Point::new(-1., 2.), Point::new(0., 4.))
    );
    let after = app.editor.project.clone();
    app.editor.undo();
    assert_eq!(app.editor.project, before);
    app.editor.redo();
    assert_eq!(app.editor.project, after);
}
#[test]
fn wall_junction_model_drag_moves_every_shared_endpoint_without_orbit() {
    let (ctx, mut app) = setup();
    let ids = junction_fixture(&mut app, 3);
    app.editor.set_selection(ids.clone());
    app.camera.target = glam::Vec3::new(0., 0., 1.5);
    app.camera.span = 12.;
    let r = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
    let point = Point::new(-1., 2.);
    let start = app.wall_project_point(r, Point::default(), 3.).unwrap();
    let end = app.wall_project_point(r, point, 3.).unwrap();
    let before = app.editor.project.clone();
    let camera = (app.camera.yaw, app.camera.pitch);
    model_frame(&ctx, &mut app, vec![Event::PointerMoved(start)]);
    model_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: start,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    model_frame(&ctx, &mut app, vec![Event::PointerMoved(end)]);
    assert_eq!(app.editor.project, before);
    model_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: end,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert!(app.wall_insert.is_none());
    assert_eq!(app.editor.undo.len(), 1);
    assert_eq!((app.camera.yaw, app.camera.pitch), camera);
    for id in ids {
        assert!(wall_reference_line(&app, id).0.distance(point) < 0.002);
    }
}

#[test]
fn wall_junction_model_requires_shared_elevation_and_uses_all_edge_levels() {
    let (_, mut app) = setup();
    let ids = junction_fixture(&mut app, 3);
    app.editor.set_selection(ids.clone());
    for e in &mut app.editor.project.elements[1..] {
        if let Shape::Wall { height, .. } = &mut e.shape {
            *height = 5.;
        }
    }
    app.camera.target = glam::Vec3::new(0., 0., 2.);
    app.camera.span = 12.;
    let r = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
    let top3 = app.wall_project_point(r, Point::default(), 3.).unwrap();
    assert!(app.selected_wall_junction_model(r, top3).is_none());
    let top5 = app.wall_project_point(r, Point::default(), 5.).unwrap();
    let (_, sharing, z) = app.selected_wall_junction_model(r, top5).unwrap();
    assert_eq!(sharing, ids[1..]);
    assert_eq!(z, 5.);
}

#[test]
fn wall_pet_three_visual_states_and_stationary_keyboard_cycle() {
    use super::wall_node_ui::WallPetCommand as C;
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app) = setup();
        app.language = language;
        let id = add_wall_for_editing(&mut app);
        app.editor.set_selection(vec![id]);
        click(&ctx, &mut app, pos2(500., 300.));
        plan_frame(&ctx, &mut app, vec![]);
        let rect = |command: C| {
            ctx.data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", command as u8))))
                .unwrap()
        };
        let active = rect(C::InsertNode);
        let hovered = rect(C::Rotate);
        let normal = rect(C::Mirror);
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(hovered.center())]);
        let output = plan_frame_output(&ctx, &mut app, vec![]);
        let fill = |rect: Rect| {
            output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Rect(painted) if painted.rect == rect && painted.fill.a() > 0 => {
                    Some(painted.fill.to_srgba_unmultiplied()[0])
                }
                _ => None,
            })
        };
        assert!(fill(active).is_some_and(|v| (i16::from(v) - 24).abs() <= 2));
        assert!(fill(hovered).is_some_and(|v| (i16::from(v) - 42).abs() <= 2));
        assert_eq!(fill(normal), None);
        assert_eq!(app.wall_insert.as_ref().unwrap().command, C::InsertNode);
        // Gaps and canvas return cannot promote a hovered icon.
        let gap = pos2(hovered.right() + 1., hovered.center().y);
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(gap)]);
        assert_eq!(app.wall_insert.as_ref().unwrap().command, C::InsertNode);
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(300., 150.))]);
        assert_eq!(app.wall_insert.as_ref().unwrap().command, C::InsertNode);
        // Explicit click selects a command, with no geometry transaction.
        click(&ctx, &mut app, rect(C::InsertNode).center());
        assert_eq!(app.wall_insert.as_ref().unwrap().command, C::InsertNode);
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::PointerMoved(rect(C::Rotate).center())],
        );
        plan_frame(&ctx, &mut app, vec![key(Key::P, Modifiers::NONE)]);
        assert_eq!(app.wall_insert.as_ref().unwrap().command, C::Curve);
        for _ in 0..3 {
            plan_frame(&ctx, &mut app, vec![]);
        }
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(300., 150.))]);
        assert_eq!(app.wall_insert.as_ref().unwrap().command, C::Curve);
        assert!(app.editor.undo.is_empty());
    }
}

#[test]
fn wall_pet_held_drag_hover_cannot_replace_locked_insert_command() {
    use super::wall_node_ui::WallPetCommand as C;
    let (ctx, mut app) = setup();
    let id = add_wall_for_editing(&mut app);
    app.editor.set_selection(vec![id]);
    let before = app.editor.project.clone();
    click(&ctx, &mut app, pos2(500., 300.));
    plan_frame(&ctx, &mut app, vec![]);
    begin_direct_drag(&ctx, &mut app, pos2(500., 300.), pos2(400., 200.));
    let rect = ctx
        .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", C::Curve as u8))))
        .unwrap();
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(rect.center())]);
    assert_eq!(app.wall_insert.as_ref().unwrap().command, C::InsertNode);
    assert_eq!(app.editor.project, before);
    release_direct_drag(&ctx, &mut app, pos2(500., 200.));
    assert!(app.wall_insert.is_none(), "{}", app.editor.status);
    assert_eq!(app.editor.project.elements.len(), 2);
    assert!(
        app.editor
            .project
            .elements
            .iter()
            .all(|wall| wall.wall_contour.is_none())
    );
    assert_eq!(app.editor.undo.len(), 1);
    app.editor.undo();
    assert_eq!(app.editor.project, before);
}

#[test]
fn wall_polygon_painter_preserves_concave_cap_without_filling_its_notch() {
    let ctx = egui::Context::default();
    let polygon = vec![
        Point::new(0., 0.),
        Point::new(4., 0.),
        Point::new(4., 4.),
        Point::new(2., 2.),
        Point::new(0., 4.),
    ];
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let painter = ui.ctx().layer_painter(egui::LayerId::background());
        AtelierApp::paint_wall_polygon(
            &painter,
            &polygon,
            |p| pos2(p.x, p.y),
            Color32::WHITE,
            Stroke::new(1., Color32::BLACK),
        );
    });
    output.textures_delta.clear();
    let mut shapes = Vec::new();
    collect_shapes(&output.shapes, &mut shapes);
    let meshes: Vec<_> = shapes
        .iter()
        .filter_map(|shape| match shape {
            egui::Shape::Mesh(mesh) => Some(mesh),
            _ => None,
        })
        .collect();
    assert_eq!(meshes.len(), 1);
    let mesh = meshes[0];
    let area: f32 = mesh
        .indices
        .chunks_exact(3)
        .map(|indices| {
            let a = mesh.vertices[indices[0] as usize].pos;
            let b = mesh.vertices[indices[1] as usize].pos;
            let c = mesh.vertices[indices[2] as usize].pos;
            ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs() * 0.5
        })
        .sum();
    assert!(
        (area - 12.).abs() < 1e-5,
        "a convex fill would incorrectly paint 16 square units"
    );
    let paths: Vec<_> = shapes
        .iter()
        .filter_map(|shape| match shape {
            egui::Shape::Path(path) => Some(path),
            _ => None,
        })
        .collect();
    assert_eq!(paths.len(), 1, "only the actual perimeter is stroked");
    assert_eq!(paths[0].points.len(), 5);
}

fn polyline_pet_fixture() -> (Context, AtelierApp, u64) {
    let (ctx, mut app) = setup();
    let id = app.editor.project.add(
        0,
        Material::Plaster,
        Shape::Polyline {
            points: vec![Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 2.)],
            arcs: vec![],
        },
    );
    app.editor.set_selection(vec![id]);
    (ctx, app, id)
}
fn polyline_pet_command(
    ctx: &Context,
    app: &mut AtelierApp,
    command: super::wall_node_ui::WallPetCommand,
) {
    plan_frame(ctx, app, vec![]);
    plan_frame(ctx, app, vec![]);
    let button = ctx
        .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", command as u8))))
        .unwrap();
    click(ctx, app, button.center());
    assert_eq!(
        app.wall_insert
            .as_ref()
            .expect(&format!(
                "session disappeared while choosing {command:?}, button {button:?}, status {}",
                app.editor.status
            ))
            .command,
        command
    );
}
#[test]
fn polyline_pet_node_click_and_drag_uses_wall_session_and_one_transaction() {
    use super::wall_node_ui::WallPetCommand as C;
    let (ctx, mut app, _) = polyline_pet_fixture();
    let before = app.editor.project.clone();
    click(&ctx, &mut app, pos2(500., 300.));
    assert_eq!(app.wall_insert.as_ref().unwrap().command, C::MoveNode);
    assert_eq!(app.editor.project, before);
    drag(&ctx, &mut app, pos2(500., 300.), pos2(550., 250.));
    assert!(app.wall_insert.is_none(), "{}", app.editor.status);
    assert!(
        matches!(&app.editor.project.elements[0].shape,Shape::Polyline {points,..} if points==&vec![Point::new(0.,0.),Point::new(3.,1.),Point::new(2.,2.)])
    );
    assert_eq!(app.editor.undo.len(), 1);
    let after = app.editor.project.clone();
    app.editor.undo();
    assert_eq!(app.editor.project, before);
    app.editor.redo();
    assert_eq!(app.editor.project, after);
}
#[test]
fn polyline_pet_direct_node_drag_edits_node_instead_of_whole_selection() {
    let (ctx, mut app, _) = polyline_pet_fixture();
    drag(&ctx, &mut app, pos2(500., 300.), pos2(550., 250.));
    assert!(app.pointer_drag.is_none());
    assert_eq!(app.editor.undo.len(), 1);
    assert!(
        matches!(&app.editor.project.elements[0].shape,Shape::Polyline{points,..} if points[0]==Point::default() && points[1]==Point::new(3.,1.) && points[2]==Point::new(2.,2.))
    );
}
#[test]
fn polyline_pet_edge_click_is_non_mutating_and_off_edge_insert_follows_wall() {
    use super::wall_node_ui::WallPetCommand as C;
    let (ctx, mut app, _) = polyline_pet_fixture();
    let before = app.editor.project.clone();
    click(&ctx, &mut app, pos2(450., 300.));
    assert_eq!(app.editor.project, before);
    assert!(app.editor.undo.is_empty());
    assert_eq!(app.wall_insert.as_ref().unwrap().command, C::InsertNode);
    click(&ctx, &mut app, pos2(450., 250.));
    assert!(app.wall_insert.is_none(), "{}", app.editor.status);
    assert!(
        matches!(&app.editor.project.elements[0].shape,Shape::Polyline{points,..} if points==&vec![Point::default(),Point::new(1.,1.),Point::new(2.,0.),Point::new(2.,2.)])
    );
    assert_eq!(app.editor.undo.len(), 1);
    app.editor.undo();
    assert_eq!(app.editor.project, before);
}
#[test]
fn polyline_pet_all_icons_localized_cycle_and_cancel_without_model_change() {
    use super::wall_node_ui::WallPetCommand as C;
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app, _) = polyline_pet_fixture();
        app.language = language;
        let before = app.editor.project.clone();
        click(&ctx, &mut app, pos2(500., 300.));
        for command in [
            C::MoveNode,
            C::Chamfer,
            C::Fillet,
            C::Curve,
            C::Tangent,
            C::OffsetPolyline,
            C::Drag,
            C::Rotate,
            C::Mirror,
            C::Multiply,
        ] {
            polyline_pet_command(&ctx, &mut app, command);
            assert_eq!(app.editor.project, before);
            assert!(app.editor.undo.is_empty());
            let label = command.label();
            assert!(!language.translate(label).is_empty());
        }
        plan_frame(&ctx, &mut app, vec![key(Key::P, Modifiers::NONE)]);
        assert_eq!(app.wall_insert.as_ref().unwrap().command, C::MoveNode);
        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        assert!(app.wall_insert.is_none());
        assert_eq!(app.editor.project, before);
    }
}
#[test]
fn polyline_pet_staged_rotate_and_mirror_use_same_transform_as_walls() {
    use super::wall_node_ui::WallPetCommand as C;
    for command in [C::Rotate, C::Mirror] {
        let (ctx, mut app, _) = polyline_pet_fixture();
        let before = app.editor.project.clone();
        click(&ctx, &mut app, pos2(450., 300.));
        polyline_pet_command(&ctx, &mut app, command);
        click(&ctx, &mut app, pos2(400., 300.));
        assert!(app.editor.undo.is_empty());
        if command == C::Rotate {
            click(&ctx, &mut app, pos2(500., 300.));
            assert!(app.editor.undo.is_empty());
        }
        click(
            &ctx,
            &mut app,
            if command == C::Rotate {
                pos2(400., 200.)
            } else {
                pos2(500., 200.)
            },
        );
        assert!(
            app.wall_insert.is_none(),
            "{command:?}: {}",
            app.editor.status
        );
        assert_eq!(app.editor.undo.len(), 1);
        assert!(
            matches!(&app.editor.project.elements[0].shape,Shape::Polyline{points,..} if points[1].distance(Point::new(0.,2.))<1e-4)
        );
        app.editor.undo();
        assert_eq!(app.editor.project, before);
    }
}
#[test]
fn polyline_pet_drag_and_multiply_transform_selected_elements_once() {
    use super::wall_node_ui::WallPetCommand as C;
    for command in [C::Drag, C::Multiply] {
        let (ctx, mut app, _) = polyline_pet_fixture();
        let before = app.editor.project.clone();
        click(&ctx, &mut app, pos2(450., 300.));
        polyline_pet_command(&ctx, &mut app, command);
        click(&ctx, &mut app, pos2(450., 200.));
        assert!(app.wall_insert.is_none(), "{}", app.editor.status);
        assert_eq!(
            app.editor.project.elements.len(),
            if command == C::Multiply { 4 } else { 1 }
        );
        assert_eq!(app.editor.undo.len(), 1);
        app.editor.undo();
        assert_eq!(app.editor.project, before);
    }
}
#[test]
fn polyline_pet_curve_repeated_offset_and_json_roundtrip_use_current_geometry() {
    use super::wall_node_ui::WallPetCommand as C;
    let (ctx, mut app, id) = polyline_pet_fixture();
    app.start_polyline_pet(
        id,
        None,
        Some(0),
        pos2(600., 400.),
        Point::new(1., 0.),
        false,
    );
    polyline_pet_command(&ctx, &mut app, C::Curve);
    click(&ctx, &mut app, pos2(450., 250.));
    let curved = app.editor.project.clone();
    for step in 0..2 {
        let anchor = Point::new(1., 1. + step as f32 * 0.2);
        app.start_polyline_pet(id, None, Some(0), pos2(600., 400.), anchor, false);
        polyline_pet_command(&ctx, &mut app, C::OffsetPolyline);
        click(&ctx, &mut app, pos2(450., 240. - step as f32 * 10.));
        assert!(app.wall_insert.is_none(), "{}", app.editor.status);
        assert!(
            matches!(&app.editor.project.elements[0].shape,Shape::Polyline{points,arcs} if points.len()==3 && arcs[0].is_some())
        );
    }
    let loaded: Project =
        serde_json::from_slice(&serde_json::to_vec(&app.editor.project).unwrap()).unwrap();
    assert_eq!(loaded, app.editor.project);
    app.editor.undo();
    app.editor.undo();
    assert_eq!(app.editor.project, curved);
}

#[test]
fn polyline_pet_numeric_tracker_locks_pointer_and_enter_commits() {
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app, _) = polyline_pet_fixture();
        app.language = language;
        let before = app.editor.project.clone();
        click(&ctx, &mut app, pos2(500., 300.));
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(600., 300.))]);
        plan_frame(&ctx, &mut app, vec![]);
        let distance = ctx
            .data(|d| d.get_temp::<Rect>(Id::new("wall_pet_distance")))
            .unwrap();
        click(&ctx, &mut app, distance.center());
        plan_frame(&ctx, &mut app, vec![]);
        plan_frame(
            &ctx,
            &mut app,
            vec![key(Key::A, Modifiers::COMMAND), Event::Text("3".into())],
        );
        let angle = ctx
            .data(|d| d.get_temp::<Rect>(Id::new("wall_pet_angle")))
            .unwrap();
        click(&ctx, &mut app, angle.center());
        plan_frame(&ctx, &mut app, vec![]);
        plan_frame(
            &ctx,
            &mut app,
            vec![key(Key::A, Modifiers::COMMAND), Event::Text("0".into())],
        );
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(350., 150.))]);
        assert_eq!(app.editor.project, before);
        plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
        assert!(app.wall_insert.is_none(), "{}", app.editor.status);
        assert_eq!(app.editor.undo.len(), 1);
        assert!(
            matches!(&app.editor.project.elements[0].shape,Shape::Polyline{points,..} if points[1].distance(Point::new(5.,0.))<1e-4)
        );
    }
}
#[test]
fn polyline_pet_hover_cannot_activate_and_invalid_cancel_keeps_history() {
    use super::wall_node_ui::WallPetCommand as C;
    let (ctx, mut app, _) = polyline_pet_fixture();
    let before = app.editor.project.clone();
    click(&ctx, &mut app, pos2(500., 300.));
    plan_frame(&ctx, &mut app, vec![]);
    let icon = ctx
        .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", C::Curve as u8))))
        .unwrap();
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(icon.center())]);
    assert_eq!(app.wall_insert.as_ref().unwrap().command, C::MoveNode);
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(350., 150.))]);
    assert_eq!(app.wall_insert.as_ref().unwrap().command, C::MoveNode);
    click(&ctx, &mut app, icon.center());
    assert_eq!(app.wall_insert.as_ref().unwrap().command, C::Curve);
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(350., 150.))]);
    assert_eq!(app.wall_insert.as_ref().unwrap().command, C::Curve);
    assert_eq!(app.editor.project, before);
    assert!(app.editor.undo.is_empty());
    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert_eq!(app.editor.project, before);
    click(&ctx, &mut app, pos2(400., 300.));
    polyline_pet_command(&ctx, &mut app, C::Fillet);
    plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
    assert!(app.wall_insert.is_some());
    assert_eq!(app.editor.project, before);
    assert!(app.editor.undo.is_empty());
    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.wall_insert.is_none());
    assert_eq!(app.editor.project, before);
}

#[test]
fn polyline_pet_overlapping_node_wins_over_other_edges_and_hidden_stories() {
    let (_ctx, mut app, id) = polyline_pet_fixture();
    let edge = app.editor.project.add(
        0,
        Material::Plaster,
        Shape::Polyline {
            points: vec![Point::new(1., 0.), Point::new(3., 0.)],
            arcs: vec![],
        },
    );
    let remote = app.editor.project.add(
        1,
        Material::Plaster,
        Shape::Polyline {
            points: vec![Point::new(2., 0.01), Point::new(4., 0.)],
            arcs: vec![],
        },
    );
    app.editor.set_selection(vec![id, edge, remote]);
    assert_eq!(
        app.selected_polyline_hit(Point::new(2., 0.01), 0.2),
        Some((id, Some(1), None))
    );
    assert_eq!(
        app.selected_polyline_hit(Point::new(2.7, 0.), 0.2),
        Some((edge, None, Some(0)))
    );
}

#[test]
fn wall_and_polyline_pet_require_left_click_and_remain_locked_after_crossing_icons() {
    use super::wall_node_ui::WallPetCommand as C;
    for polyline in [false, true] {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            app.language = language;
            let (id, entry) = if polyline {
                let id = app.editor.project.add(
                    0,
                    Material::Plaster,
                    Shape::Polyline {
                        points: vec![Point::new(0., 0.), Point::new(4., 0.)],
                        arcs: vec![],
                    },
                );
                (id, pos2(500., 300.))
            } else {
                (add_wall_for_editing(&mut app), pos2(500., 300.))
            };
            app.editor.set_selection(vec![id]);
            let before = app.editor.project.clone();
            click(&ctx, &mut app, entry);
            plan_frame(&ctx, &mut app, vec![]);
            assert_eq!(app.wall_insert.as_ref().unwrap().command, C::InsertNode);
            let icon = |command: C| {
                ctx.data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", command as u8))))
                    .unwrap()
                    .center()
            };
            for command in [C::Curve, C::Drag, C::Rotate, C::Mirror] {
                plan_frame(&ctx, &mut app, vec![Event::PointerMoved(icon(command))]);
                plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(300., 150.))]);
                assert_eq!(app.wall_insert.as_ref().unwrap().command, C::InsertNode);
            }
            let curve = icon(C::Curve);
            plan_frame(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(curve),
                    Event::PointerButton {
                        pos: curve,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
            assert_eq!(
                app.wall_insert.as_ref().unwrap().command,
                C::InsertNode,
                "press alone must not activate"
            );
            plan_frame(
                &ctx,
                &mut app,
                vec![Event::PointerButton {
                    pos: curve,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                }],
            );
            assert_eq!(app.wall_insert.as_ref().unwrap().command, C::Curve);
            for command in [C::InsertNode, C::Drag, C::Rotate, C::Mirror, C::Multiply] {
                plan_frame(&ctx, &mut app, vec![Event::PointerMoved(icon(command))]);
                plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(300., 150.))]);
                assert_eq!(
                    app.wall_insert.as_ref().unwrap().command,
                    C::Curve,
                    "polyline={polyline}, hovered={command:?}"
                );
            }
            assert_eq!(app.editor.project, before);
            assert!(app.editor.undo.is_empty());
            // Release outside an icon after pressing it is not an icon click.
            let rotate = icon(C::Rotate);
            plan_frame(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(rotate),
                    Event::PointerButton {
                        pos: rotate,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
            plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(300., 150.))]);
            release_direct_drag(&ctx, &mut app, pos2(300., 150.));
            assert_eq!(app.wall_insert.as_ref().unwrap().command, C::Curve);
            assert_eq!(app.editor.project, before);
            plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
            assert!(app.wall_insert.is_none());
            assert_eq!(app.editor.project, before);
        }
    }
}

#[test]
fn wall_endpoint_palette_switch_roundtrip_keeps_endpoint_drag_and_undo() {
    use super::wall_node_ui::WallPetCommand as C;
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        for (node, at, target, expected) in [
            (
                0,
                pos2(400., 300.),
                pos2(450., 250.),
                (Point::new(1., 1.), Point::new(4., 0.)),
            ),
            (
                1,
                pos2(600., 300.),
                pos2(650., 250.),
                (Point::default(), Point::new(5., 1.)),
            ),
        ] {
            let (ctx, mut app) = setup();
            app.language = language;
            let id = add_wall_for_editing(&mut app);
            app.editor.set_selection(vec![id]);
            let before = app.editor.project.clone();
            click(&ctx, &mut app, at);
            plan_frame(&ctx, &mut app, vec![]);
            let icon = |command: C| {
                ctx.data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", command as u8))))
                    .unwrap()
                    .center()
            };
            click(&ctx, &mut app, icon(C::Curve));
            assert!(app.wall_edit.is_none());
            assert_eq!(app.wall_insert.as_ref().unwrap().command, C::Curve);
            plan_frame(&ctx, &mut app, vec![Event::PointerMoved(icon(C::Rotate))]);
            plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(300., 150.))]);
            assert_eq!(app.wall_insert.as_ref().unwrap().command, C::Curve);
            assert_eq!(app.editor.project, before);
            assert!(app.editor.undo.is_empty());
            click(&ctx, &mut app, icon(C::InsertNode));
            assert!(app.wall_insert.is_none());
            assert_eq!(app.wall_edit.as_ref().unwrap().node, Some(node));
            assert_eq!(app.editor.project, before);
            assert!(app.editor.undo.is_empty());
            drag(&ctx, &mut app, at, target);
            assert_eq!(wall_reference_line(&app, id), expected);
            assert_eq!(app.editor.undo.len(), 1);
            let after = app.editor.project.clone();
            app.editor.undo();
            assert_eq!(app.editor.project, before);
            app.editor.redo();
            assert_eq!(app.editor.project, after);
        }
    }
}

#[test]
fn slab_edge_pet_insert_hover_cancel_commit_and_undo_both_languages() {
    use super::wall_node_ui::WallPetCommand as C;
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app) = setup();
        app.language = language;
        let id = app.editor.project.add(
            0,
            Material::Concrete,
            Shape::Slab {
                min: Point::new(0., 0.),
                max: Point::new(4., 3.),
                thickness: 0.3,
            },
        );
        app.editor.set_selection(vec![id]);
        let before = app.editor.project.clone();
        // Actual plan edge entry: bottom world edge is y=300 in this viewport.
        click(&ctx, &mut app, pos2(500., 300.));
        assert_eq!(app.wall_insert.as_ref().unwrap().command, C::InsertNode);
        assert_eq!(app.editor.project, before);
        assert!(app.editor.undo.is_empty());
        plan_frame(&ctx, &mut app, vec![]);
        plan_frame(&ctx, &mut app, vec![]);
        let drag = ctx
            .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", C::Drag as u8))))
            .unwrap();
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(drag.center())]);
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(500., 250.))]);
        assert_eq!(app.wall_insert.as_ref().unwrap().command, C::InsertNode);
        assert_eq!(app.editor.project, before);
        click(&ctx, &mut app, pos2(500., 250.));
        assert!(app.wall_insert.is_none(), "{}", app.editor.status);
        assert_eq!(app.editor.project.elements[0].area(), 10.);
        assert_eq!(app.editor.undo.len(), 1);
        let after = app.editor.project.clone();
        app.editor.undo();
        assert_eq!(app.editor.project, before);
        app.editor.redo();
        assert_eq!(app.editor.project, after);
        // Open again and cancel; no second history entry or model change.
        plan_frame(&ctx, &mut app, vec![]);
        plan_frame(&ctx, &mut app, vec![]);
        app.editor.set_selection(vec![id]);
        click(&ctx, &mut app, pos2(400., 225.));
        assert!(
            app.wall_insert.is_some(),
            "{} {:?}",
            app.editor.status,
            app.editor.selection_ids()
        );
        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        assert!(app.wall_insert.is_none());
        assert_eq!(app.editor.project, after);
        assert_eq!(app.editor.undo.len(), 1);
    }
}

fn slab_full_pet_fixture(language: UiLanguage) -> (Context, AtelierApp, u64) {
    let (ctx, mut app) = setup();
    app.language = language;
    let id = app.editor.project.add(
        0,
        Material::Concrete,
        Shape::Slab {
            min: Point::new(0., 0.),
            max: Point::new(4., 3.),
            thickness: 0.3,
        },
    );
    app.editor.set_selection(vec![id]);
    (ctx, app, id)
}

#[test]
fn slab_full_pet_actual_curve_entry_preview_commit_undo_and_json_both_languages() {
    use super::wall_node_ui::WallPetCommand as C;
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app, id) = slab_full_pet_fixture(language);
        let before = app.editor.project.clone();
        click(&ctx, &mut app, pos2(500., 300.));
        assert_eq!(app.wall_insert.as_ref().unwrap().command, C::InsertNode);
        polyline_pet_command(&ctx, &mut app, C::Curve);
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(500., 350.))]);
        assert_eq!(
            app.editor.project, before,
            "a candidate must not alter the model"
        );
        assert!(app.editor.undo.is_empty());
        let rotate = ctx
            .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", C::Rotate as u8))))
            .unwrap();
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(rotate.center())]);
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(500., 350.))]);
        assert_eq!(app.wall_insert.as_ref().unwrap().command, C::Curve);
        plan_frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(rotate.center()),
                Event::PointerButton {
                    pos: rotate.center(),
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        assert_eq!(
            app.wall_insert.as_ref().unwrap().command,
            C::Curve,
            "press alone is not activation"
        );
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(300., 150.))]);
        release_direct_drag(&ctx, &mut app, pos2(300., 150.));
        assert_eq!(app.wall_insert.as_ref().unwrap().command, C::Curve);
        assert_eq!(app.editor.project, before);
        assert!(app.editor.undo.is_empty());
        click(&ctx, &mut app, pos2(500., 350.));
        assert!(app.wall_insert.is_none(), "{}", app.editor.status);
        assert_eq!(app.editor.undo.len(), 1);
        let element = &app.editor.project.elements[0];
        assert_eq!(element.id, id);
        assert_eq!(element.thickness(), 0.3);
        let contour = element
            .slab_contour
            .as_ref()
            .expect("a curved slab must retain exact contour data");
        assert_eq!(
            contour.points.len(),
            4,
            "display tessellation is not editable nodes"
        );
        assert_eq!(contour.points[0], Point::new(0., 0.));
        assert_eq!(contour.points[1], Point::new(4., 0.));
        let arc = contour.arcs[0].expect("the clicked edge must retain an exact arc");
        assert!((arc.radius - 2.5).abs() < 1e-4);
        assert!(
            element.area() > 14.7 && element.area() < 14.9,
            "{}",
            element.area()
        );
        let triangle_area: f32 = crate::slab_geometry::triangles(element)
            .iter()
            .map(|t| crate::slab_geometry::area(t))
            .sum();
        assert!(
            (triangle_area - element.area()).abs() < 0.02,
            "triangulated cap {triangle_area}, exact area {}",
            element.area()
        );
        let camera = geometry::Camera {
            yaw: 0.,
            pitch: std::f32::consts::FRAC_PI_2 - 0.001,
            span: 10.,
            target: glam::Vec3::new(2., 1.5, 0.),
        };
        for (point, expected) in [
            (Point::new(2., -0.7), Some(id)),
            (Point::new(2., -1.3), None),
        ] {
            let ndc = camera
                .matrix(1.)
                .project_point3(glam::Vec3::new(point.x, point.y, 0.));
            assert_eq!(
                camera.pick(&app.editor.project, [ndc.x, ndc.y], 1., Some(0)),
                expected
            );
        }
        let edited = app.editor.project.clone();
        let loaded: Project =
            serde_json::from_str(&serde_json::to_string(&edited).unwrap()).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded, edited);
        app.editor.undo();
        assert_eq!(app.editor.project, before);
        app.editor.redo();
        assert_eq!(app.editor.project, edited);
    }
}

#[test]
fn slab_full_pet_all_thirteen_commands_have_real_click_activation_and_cancel() {
    use super::wall_node_ui::WallPetCommand as C;
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app, _) = slab_full_pet_fixture(language);
        let original = app.editor.project.clone();
        click(&ctx, &mut app, pos2(500., 300.));
        for command in [
            C::Curve,
            C::Tangent,
            C::OffsetEdge,
            C::OffsetAll,
            C::AddPolygon,
            C::SubtractPolygon,
            C::Drag,
            C::Rotate,
            C::Mirror,
            C::Elevate,
            C::Multiply,
            C::InsertNode,
        ] {
            polyline_pet_command(&ctx, &mut app, command);
            assert_eq!(
                app.editor.project, original,
                "switching {command:?} must not commit"
            );
            assert!(app.editor.undo.is_empty());
            let label = command.label();
            assert!(!language.translate(label).is_empty());
            if language == UiLanguage::SimplifiedChinese {
                assert_ne!(language.translate(label), label);
            }
            let icon = ctx
                .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", C::CustomEdge as u8))))
                .unwrap();
            plan_frame(&ctx, &mut app, vec![Event::PointerMoved(icon.center())]);
            plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(300., 150.))]);
            assert_eq!(app.wall_insert.as_ref().unwrap().command, command);
        }
        polyline_pet_command(&ctx, &mut app, C::CustomEdge);
        assert_eq!(app.editor.project, original);
        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        assert_eq!(app.editor.project, original);
        assert!(app.editor.undo.is_empty());
    }
}

#[test]
fn slab_full_pet_polygon_subtract_waits_for_enter_and_hole_node_remains_editable() {
    use super::wall_node_ui::WallPetCommand as C;
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app, id) = slab_full_pet_fixture(language);
        let original = app.editor.project.clone();
        click(&ctx, &mut app, pos2(500., 300.));
        polyline_pet_command(&ctx, &mut app, C::SubtractPolygon);
        for point in [
            pos2(450., 260.),
            pos2(550., 260.),
            pos2(550., 190.),
            pos2(450., 190.),
        ] {
            click(&ctx, &mut app, point);
            assert_eq!(app.editor.project, original);
            assert!(
                app.editor.undo.is_empty(),
                "polygon vertices are draft input"
            );
        }
        plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
        assert!(app.wall_insert.is_none(), "{}", app.editor.status);
        assert_eq!(app.editor.undo.len(), 1);
        let slab = &app.editor.project.elements[0];
        let contour = slab.slab_contour.as_ref().unwrap();
        assert_eq!(contour.holes.len(), 1);
        assert!((slab.area() - 9.2).abs() < 1e-4, "{}", slab.area());
        assert_eq!(
            geometry::hit_plan(&app.editor.project, 0, Point::new(2., 1.5), 0.01),
            None
        );
        let hole_node = contour.holes[0].points[0];
        let flat_node = contour.points.len();
        assert_eq!(
            app.selected_polyline_hit(hole_node, 0.05),
            Some((id, Some(flat_node), None))
        );
        let edited = app.editor.project.clone();
        plan_frame(&ctx, &mut app, vec![]);
        plan_frame(&ctx, &mut app, vec![]);
        click(
            &ctx,
            &mut app,
            pos2(400. + 50. * hole_node.x, 300. - 50. * hole_node.y),
        );
        assert_eq!(app.wall_insert.as_ref().unwrap().command, C::MoveNode);
        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        assert_eq!(app.editor.project, edited);
        assert_eq!(app.editor.undo.len(), 1);
        app.editor.undo();
        assert_eq!(app.editor.project, original);
        app.editor.redo();
        assert_eq!(app.editor.project, edited);
        let loaded: Project =
            serde_json::from_str(&serde_json::to_string(&edited).unwrap()).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded, edited);
    }
}

#[test]
fn slab_full_pet_polygon_cancel_discards_every_accepted_vertex() {
    use super::wall_node_ui::WallPetCommand as C;
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app, _) = slab_full_pet_fixture(language);
        let original = app.editor.project.clone();
        click(&ctx, &mut app, pos2(500., 300.));
        polyline_pet_command(&ctx, &mut app, C::AddPolygon);
        for point in [pos2(550., 250.), pos2(650., 250.), pos2(650., 200.)] {
            click(&ctx, &mut app, point);
            assert_eq!(app.editor.project, original);
            assert!(app.editor.undo.is_empty());
        }
        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        assert!(app.wall_insert.is_none());
        assert_eq!(app.editor.project, original);
        assert!(app.editor.undo.is_empty());
    }
}

#[test]
fn slab_full_pet_single_edge_and_all_edges_offset_keep_polygon_connections() {
    use super::wall_node_ui::WallPetCommand as C;
    for (command, expected_area) in [(C::OffsetEdge, 14.), (C::OffsetAll, 20.)] {
        let (ctx, mut app, _) = slab_full_pet_fixture(UiLanguage::English);
        let original = app.editor.project.clone();
        click(&ctx, &mut app, pos2(500., 300.));
        polyline_pet_command(&ctx, &mut app, command);
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(500., 325.))]);
        assert_eq!(app.editor.project, original);
        assert!(app.editor.undo.is_empty());
        click(&ctx, &mut app, pos2(500., 325.));
        assert!(app.wall_insert.is_none(), "{}", app.editor.status);
        let contour = app.editor.project.elements[0]
            .slab_contour
            .as_ref()
            .unwrap();
        assert_eq!(contour.points.len(), 4);
        assert!((app.editor.project.elements[0].area() - expected_area).abs() < 1e-4);
        if command == C::OffsetEdge {
            assert_eq!(contour.points[0], Point::new(0., -0.5));
            assert_eq!(contour.points[1], Point::new(4., -0.5));
            assert_eq!(contour.points[2], Point::new(4., 3.));
            assert_eq!(contour.points[3], Point::new(0., 3.));
        }
        app.editor.project.validate().unwrap();
        assert_eq!(app.editor.undo.len(), 1);
        app.editor.undo();
        assert_eq!(app.editor.project, original);
    }
}

#[test]
fn slab_full_pet_node_entry_moves_only_hit_node_and_preserves_one_undo() {
    use super::wall_node_ui::WallPetCommand as C;
    let (ctx, mut app, _) = slab_full_pet_fixture(UiLanguage::English);
    let original = app.editor.project.clone();
    click(&ctx, &mut app, pos2(400., 300.));
    assert_eq!(app.wall_insert.as_ref().unwrap().command, C::MoveNode);
    click(&ctx, &mut app, pos2(350., 325.));
    assert!(app.wall_insert.is_none(), "{}", app.editor.status);
    let contour = app.editor.project.elements[0]
        .slab_contour
        .as_ref()
        .unwrap();
    assert_eq!(
        contour.points,
        vec![
            Point::new(-1., -0.5),
            Point::new(4., 0.),
            Point::new(4., 3.),
            Point::new(0., 3.)
        ]
    );
    assert!((app.editor.project.elements[0].area() - 14.5).abs() < 1e-4);
    assert_eq!(app.editor.undo.len(), 1);
    app.editor.undo();
    assert_eq!(app.editor.project, original);
}

fn slab_pet_assert_transaction(app: &mut AtelierApp, before: &Project) {
    assert_eq!(app.editor.undo.len(), 1);
    let after = app.editor.project.clone();
    assert_ne!(&after, before);
    after.validate().unwrap();
    let reopened: Project = serde_json::from_str(&serde_json::to_string(&after).unwrap()).unwrap();
    assert_eq!(reopened, after);
    app.editor.undo();
    assert_eq!(&app.editor.project, before);
    app.editor.redo();
    assert_eq!(app.editor.project, after);
}

#[test]
fn slab_full_pet_transforms_commit_actual_contours_once_and_roundtrip() {
    use super::wall_node_ui::WallPetCommand as C;
    for command in [C::Drag, C::Rotate, C::Mirror, C::Multiply] {
        let (ctx, mut app, _) = slab_full_pet_fixture(UiLanguage::English);
        let before = app.editor.project.clone();
        click(&ctx, &mut app, pos2(500., 300.));
        polyline_pet_command(&ctx, &mut app, command);
        if matches!(command, C::Rotate | C::Mirror) {
            click(&ctx, &mut app, pos2(400., 300.));
            assert_eq!(app.editor.project, before);
            if command == C::Rotate {
                click(&ctx, &mut app, pos2(500., 300.));
                assert_eq!(app.editor.project, before);
            }
        }
        click(
            &ctx,
            &mut app,
            if command == C::Mirror {
                pos2(500., 200.)
            } else {
                pos2(400., 200.)
            },
        );
        assert!(
            app.wall_insert.is_none(),
            "{command:?}: {}",
            app.editor.status
        );
        assert_eq!(
            app.editor.project.elements.len(),
            if command == C::Multiply { 4 } else { 1 }
        );
        let expected = match command {
            C::Rotate => Point::new(0., 4.),
            C::Mirror => Point::new(0., 4.),
            _ => Point::new(2., 2.),
        };
        let moved = &app.editor.project.elements[if command == C::Multiply { 1 } else { 0 }];
        let points = crate::slab_geometry::contour(moved).points;
        assert!(
            points.iter().any(|p| p.distance(expected) < 1e-4),
            "{command:?}: {:?}",
            points
        );
        assert!((moved.area() - 12.).abs() < 1e-4);
        slab_pet_assert_transaction(&mut app, &before);
    }
}

#[test]
fn slab_full_pet_tangent_and_corner_commands_commit_exact_geometry() {
    use super::wall_node_ui::WallPetCommand as C;
    for command in [C::Tangent, C::Fillet, C::Chamfer] {
        let (ctx, mut app, _) = slab_full_pet_fixture(UiLanguage::English);
        let before = app.editor.project.clone();
        click(
            &ctx,
            &mut app,
            if command == C::Tangent {
                pos2(500., 300.)
            } else {
                pos2(400., 300.)
            },
        );
        polyline_pet_command(&ctx, &mut app, command);
        click(&ctx, &mut app, pos2(350., 350.));
        assert!(
            app.wall_insert.is_none(),
            "{command:?}: {}",
            app.editor.status
        );
        let slab = &app.editor.project.elements[0];
        let contour = slab.slab_contour.as_ref().unwrap();
        if command == C::Tangent {
            assert_eq!(contour.points.len(), 4);
            assert!(contour.arcs[0].is_some());
            assert_eq!(contour.points[0], Point::new(0., 0.));
            assert_eq!(contour.points[1], Point::new(4., 0.));
        } else {
            assert_eq!(contour.points.len(), 5);
            assert!(slab.area() < 12. && slab.area() > 11.9);
            assert_eq!(
                contour.arcs.iter().filter(|a| a.is_some()).count(),
                usize::from(command == C::Fillet)
            );
        }
        slab_pet_assert_transaction(&mut app, &before);
    }
}

#[test]
fn slab_full_pet_elevate_numeric_field_changes_shared_top_elevation() {
    use super::wall_node_ui::WallPetCommand as C;
    let (ctx, mut app, _) = slab_full_pet_fixture(UiLanguage::English);
    let before = app.editor.project.clone();
    click(&ctx, &mut app, pos2(500., 300.));
    polyline_pet_command(&ctx, &mut app, C::Elevate);
    plan_frame(&ctx, &mut app, vec![]);
    plan_frame(&ctx, &mut app, vec![]);
    let field = ctx
        .data(|d| d.get_temp::<Rect>(Id::new("wall_pet_distance")))
        .unwrap();
    click(&ctx, &mut app, field.center());
    plan_frame(&ctx, &mut app, vec![]);
    assert!(
        ctx.egui_wants_keyboard_input(),
        "elevation field must own keyboard focus"
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![key(Key::A, Modifiers::COMMAND), Event::Text("2".into())],
    );
    assert_eq!(app.editor.project, before);
    plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
    assert!(app.wall_insert.is_none(), "{}", app.editor.status);
    let slab = &app.editor.project.elements[0];
    assert_eq!(slab.slab_contour.as_ref().unwrap().elevation, 2.);
    assert_eq!(slab.area(), 12.);
    slab_pet_assert_transaction(&mut app, &before);
}

#[test]
fn slab_full_pet_custom_surface_and_scope_apply_real_controls_both_languages() {
    use super::wall_node_ui::WallPetCommand as C;
    use crate::slab_geometry::SlabEdgeScope as S;
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        for scope in [S::Clicked, S::Ring, S::All] {
            let (ctx, mut app, _) = slab_full_pet_fixture(language);
            let before = app.editor.project.clone();
            click(&ctx, &mut app, pos2(500., 300.));
            polyline_pet_command(&ctx, &mut app, C::CustomEdge);
            let surface = ctx
                .data(|d| d.get_temp::<Rect>(Id::new("slab_edge_surface")))
                .unwrap();
            click(&ctx, &mut app, surface.center());
            plan_frame(&ctx, &mut app, vec![]);
            plan_frame(&ctx, &mut app, vec![]);
            let material = ctx
                .data(|d| {
                    d.get_temp::<Rect>(Id::new(("slab_edge_material", Material::Timber as u8)))
                })
                .unwrap();
            click(&ctx, &mut app, material.center());
            let scope_rect = ctx
                .data(|d| d.get_temp::<Rect>(Id::new(("slab_edge_scope", scope as u8))))
                .unwrap();
            click(&ctx, &mut app, scope_rect.center());
            assert_eq!(app.editor.project, before);
            let apply = ctx
                .data(|d| d.get_temp::<Rect>(Id::new("slab_edge_apply")))
                .unwrap();
            click(&ctx, &mut app, apply.center());
            assert!(app.wall_insert.is_none(), "{}", app.editor.status);
            let c = crate::slab_geometry::contour(&app.editor.project.elements[0]);
            assert_eq!(
                c.edge_settings
                    .iter()
                    .filter(|s| s.surface == Some(Material::Timber))
                    .count(),
                if scope == S::Clicked { 1 } else { 4 }
            );
            assert!(
                c.edge_settings
                    .iter()
                    .all(|s| (s.angle_degrees - 90.).abs() < 1e-5)
            );
            slab_pet_assert_transaction(&mut app, &before);
        }
    }
}

#[test]
fn slab_full_pet_boolean_split_candidate_keeps_identity_and_allocates_unique_ids() {
    use crate::slab_geometry::SlabChange;
    let (_, mut app, id) = slab_full_pet_fixture(UiLanguage::English);
    let before = app.editor.project.clone();
    let command = crate::reshape::ReshapeCommand::SlabEdit {
        id,
        index: 0,
        anchor: Point::new(2., 0.),
        point: Point::new(2., 0.),
        change: SlabChange::SubtractPolygon(vec![
            Point::new(1.5, -1.),
            Point::new(2.5, -1.),
            Point::new(2.5, 4.),
            Point::new(1.5, 4.),
        ]),
    };
    let (candidate, ids) = crate::reshape::candidate(&before, &command).unwrap();
    assert_eq!(app.editor.project, before);
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0], id);
    assert_ne!(ids[0], ids[1]);
    assert_eq!(candidate.elements.len(), 2);
    assert!((candidate.elements.iter().map(|e| e.area()).sum::<f32>() - 9.).abs() < 1e-4);
    candidate.validate().unwrap();
    app.editor.reshape(command).unwrap();
    assert_eq!(app.editor.project, candidate);
    slab_pet_assert_transaction(&mut app, &before);
}

#[test]
fn slab_full_pet_custom_angle_numeric_apply_and_undo_both_languages() {
    use super::wall_node_ui::WallPetCommand as C;
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        for angle in [45., 135.] {
            let (ctx, mut app, _) = slab_full_pet_fixture(language);
            let before = app.editor.project.clone();
            click(&ctx, &mut app, pos2(500., 300.));
            polyline_pet_command(&ctx, &mut app, C::CustomEdge);
            let field = ctx
                .data(|d| d.get_temp::<Rect>(Id::new("slab_edge_angle")))
                .unwrap();
            click(&ctx, &mut app, field.center());
            click(&ctx, &mut app, field.center());
            plan_frame(
                &ctx,
                &mut app,
                vec![
                    key(Key::A, Modifiers::COMMAND),
                    Event::Text(format!("{angle}")),
                ],
            );
            assert_eq!(app.editor.project, before);
            let apply = ctx
                .data(|d| d.get_temp::<Rect>(Id::new("slab_edge_apply")))
                .unwrap();
            click(&ctx, &mut app, apply.center());
            assert!(app.wall_insert.is_none(), "{}", app.editor.status);
            let c = crate::slab_geometry::contour(&app.editor.project.elements[0]);
            assert!((c.edge_settings[0].angle_degrees - angle).abs() < 1e-4);
            assert!(c.edge_settings[1..].iter().all(|s| s.angle_degrees == 90.));
            assert_eq!(app.editor.project.elements[0].area(), 12.);
            slab_pet_assert_transaction(&mut app, &before);
        }
    }
}

#[test]
fn slab_full_pet_insert_arc_node_activates_only_on_release_and_preserves_arc() {
    use super::wall_node_ui::WallPetCommand as C;
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app, _) = slab_full_pet_fixture(language);
        let contours = crate::slab_geometry::changed(
            &app.editor.project.elements[0],
            0,
            &crate::slab_geometry::SlabChange::Curve,
            Point::new(2., 0.),
            Point::new(2., -1.),
        )
        .unwrap();
        crate::slab_geometry::sync(&mut app.editor.project.elements[0], contours[0].clone())
            .unwrap();
        let before = app.editor.project.clone();
        let area = before.elements[0].area();
        click(&ctx, &mut app, pos2(500., 350.));
        plan_frame(&ctx, &mut app, vec![]);
        let icon = ctx
            .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", C::InsertArcNode as u8))))
            .unwrap();
        plan_frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(icon.center()),
                Event::PointerButton {
                    pos: icon.center(),
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        assert_eq!(app.editor.project, before);
        assert!(app.editor.undo.is_empty());
        release_direct_drag(&ctx, &mut app, icon.center());
        assert!(app.wall_insert.is_none(), "{}", app.editor.status);
        let c = crate::slab_geometry::contour(&app.editor.project.elements[0]);
        assert_eq!(c.points.len(), 5);
        assert_eq!(c.arcs.iter().filter(|a| a.is_some()).count(), 2);
        assert!((app.editor.project.elements[0].area() - area).abs() < 1e-4);
        assert!(
            c.points
                .iter()
                .any(|p| p.distance(Point::new(2., -1.)) < 1e-4)
        );
        slab_pet_assert_transaction(&mut app, &before);
    }
}
