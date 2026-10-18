use super::*;
fn setup() -> (Context, AtelierApp) {
    let ctx = Context::default();
    AtelierApp::style(&ctx);
    let mut app = AtelierApp::from_renderer(None, "Headless test".into());
    app.recovery_path = None;
    app.editor = Editor::new(Project::default());
    app.plan_center = Point::default();
    app.plan_scale = 50.;
    (ctx, app)
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
            app.apply_polyline_edit_cursor(ui.ctx());
        },
    );
    output.textures_delta.clear();
    output
}
fn model_frame(ctx: &Context, app: &mut AtelierApp, events: Vec<Event>) {
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
fn selected_polyline_node_opens_palette_and_drag_edits_both_segments_once() {
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
    let node = pos2(500., 300.);
    click(&ctx, &mut app, node);
    assert!(
        app.polyline_edit
            .as_ref()
            .is_some_and(|edit| edit.element_id == id && edit.node == Some(1))
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(node),
            Event::PointerButton {
                pos: node,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    let moved = pos2(550., 250.);
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(moved)]);
    assert!(
        matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points[1]==Point::new(3.,1.)),
        "incident segments update during the drag preview"
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: moved,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    let Shape::Polyline { points, .. } = &app.editor.project.elements[0].shape else {
        panic!("shape changed");
    };
    assert_eq!(points[1], Point::new(3., 1.));
    assert_eq!(points[0], Point::new(0., 0.));
    assert_eq!(points[2], Point::new(2., 2.));
    assert_eq!(app.editor.undo.len(), 1, "node edit is one transaction");
    app.editor.undo();
    assert!(
        matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points[1]==Point::new(2.,0.))
    );
}
#[test]
fn selected_polyline_edge_click_requires_explicit_insert_command() {
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
    click(&ctx, &mut app, pos2(450., 300.));
    assert!(
        matches!(app.editor.project.elements[0].shape, Shape::Polyline { ref points, .. } if points.len()==3)
    );
    assert!(app.editor.undo.is_empty());
    assert_eq!(app.polyline_edit.as_ref().unwrap().edge, Some(0));
    app.insert_polyline_node_at(id, 0, Point::new(1., 0.), pos2(450., 300.));
    assert!(
        matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points.len()==4 && points[1]==Point::new(1.,0.))
    );
    assert!(
        app.polyline_edit
            .as_ref()
            .is_some_and(|edit| edit.element_id == id
                && edit.node == Some(1)
                && edit.edge == Some(0)
                && edit.command == PolylineEditCommand::MoveNode)
    );
    assert_eq!(
        app.editor.undo.len(),
        1,
        "node insertion is one undoable operation"
    );
    app.editor.undo();
    assert!(
        matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points.len()==3 && points[1]==Point::new(2.,0.))
    );
}
#[test]
fn explicit_arc_node_insertion_preserves_circle_and_undo() {
    let (_ctx, mut app) = setup();
    let a = Point::new(0., 0.);
    let b = Point::new(2., 0.);
    let through = Point::new(1., 1.);
    let arc = geometry::arc_segment_through(a, b, through).unwrap();
    let id = app.editor.project.add(
        0,
        Material::Plaster,
        Shape::Polyline {
            points: vec![a, b],
            arcs: vec![Some(arc)],
        },
    );
    let before = app.editor.project.clone();
    app.open_polyline_edit(id, None, Some(0), Pos2::ZERO);
    let mut edit = app.polyline_edit.clone().unwrap();
    edit.command = PolylineEditCommand::InsertNode;
    app.apply_polyline_drag_preview(&before, &edit, through, through);
    let Shape::Polyline { points, arcs } = &app.editor.project.elements[0].shape else {
        panic!()
    };
    assert_eq!(points.len(), 3);
    assert!(points[1].distance(through) < 1e-4);
    for piece in arcs.iter().flatten() {
        assert!((piece.radius - arc.radius).abs() < 1e-5);
        assert_eq!(piece.clockwise, arc.clockwise);
    }
    assert!(app.editor.project.validate().is_ok());
    let loaded =
        serde_json::from_slice::<Project>(&serde_json::to_vec(&app.editor.project).unwrap())
            .unwrap();
    assert_eq!(loaded, app.editor.project);
    app.editor.commit(before.clone());
    app.editor.undo();
    assert_eq!(app.editor.project, before);
    app.editor.redo();
    assert_eq!(app.editor.project, loaded);
}
#[test]
fn selecting_curved_polyline_edge_opens_edit_without_inserting_a_node() {
    let (_ctx, mut app) = setup();
    let a = Point::new(0., 0.);
    let b = Point::new(2., 0.);
    let arc = geometry::arc_segment_through(a, b, Point::new(1., 0.5)).unwrap();
    let id = app.editor.project.add(
        0,
        Material::Plaster,
        Shape::Polyline {
            points: vec![a, b],
            arcs: vec![Some(arc)],
        },
    );
    app.editor.set_selection(vec![id]);
    app.insert_polyline_node_at(id, 0, Point::new(1., 0.5), Pos2::ZERO);
    assert!(
        matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points,ref arcs} if points.len()==2 && arcs[0].is_some())
    );
    assert!(
        app.polyline_edit
            .as_ref()
            .is_some_and(|edit| edit.element_id == id
                && edit.edge == Some(0)
                && edit.node.is_none())
    );
    assert!(app.editor.undo.is_empty());
}
#[test]
fn dragging_polyline_node_in_select_moves_the_whole_object() {
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
    let start = pos2(500., 300.);
    let end = pos2(550., 250.);
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
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(end)]);
    assert!(
        app.drag.is_some(),
        "dragging a node target in Select must start a whole-element drag"
    );
    assert!(
        app.polyline_edit.is_none(),
        "a drag from a node target must not enter node-edit mode"
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: end,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert!(
        matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points[0]==Point::new(1.,1.) && points[1]==Point::new(3.,1.) && points[2]==Point::new(3.,3.))
    );
    assert!(app.polyline_edit.is_none());
    assert_eq!(
        app.editor.undo.len(),
        1,
        "whole-element movement is one undoable action"
    );
}
#[test]
fn confirmed_curve_then_pointer_offset_uses_current_shape_and_undo() {
    let (ctx, mut app) = setup();
    let id = app.editor.project.add(
        0,
        Material::Plaster,
        Shape::Polyline {
            points: vec![Point::new(0., 0.), Point::new(2., 0.)],
            arcs: vec![],
        },
    );
    app.editor.set_selection(vec![id]);
    app.open_polyline_edit(id, None, Some(0), pos2(450., 300.));
    let before = app.editor.project.clone();
    let mut edit = app.polyline_edit.clone().unwrap();
    edit.command = PolylineEditCommand::CurveEdge;
    app.apply_polyline_drag_preview(&before, &edit, Point::new(1., 0.), Point::new(1., 1.));
    app.editor.commit(before);
    let curved = app.editor.project.clone();
    for step in 0..2 {
        let y = 250. - step as f32 * 12.5;
        app.open_polyline_edit(id, None, Some(0), pos2(450., y));
        app.polyline_edit.as_mut().unwrap().command = PolylineEditCommand::OffsetPolyline;
        let start = pos2(450., y);
        let end = pos2(450., y - 12.5);
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(start)]);
        plan_frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(start),
                Event::PointerButton {
                    pos: start,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(end)]);
        let Shape::Polyline { points, arcs } = &app.editor.project.elements[0].shape else {
            panic!()
        };
        assert_eq!(points.len(), 2);
        assert!(
            (arcs[0].unwrap().radius - (1.25 + step as f32 * 0.25)).abs() < 1e-4,
            "step={step} arc={:?} edit={:?}",
            arcs[0],
            app.polyline_edit
        );
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::PointerButton {
                pos: end,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert!(app.polyline_edit.is_none());
    }
    let offset = app.editor.project.clone();
    let loaded: Project = serde_json::from_slice(&serde_json::to_vec(&offset).unwrap()).unwrap();
    assert_eq!(loaded, offset);
    app.editor.undo();
    app.editor.undo();
    assert_eq!(app.editor.project, curved);
    app.editor.redo();
    app.editor.redo();
    assert_eq!(app.editor.project, offset);
}

#[test]
fn polyline_offset_and_curve_commands_follow_pointer_preview_and_undo() {
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
    app.open_polyline_edit(id, Some(1), Some(0), pos2(450., 300.));
    app.polyline_edit.as_mut().unwrap().command = PolylineEditCommand::OffsetPolyline;
    let start = pos2(450., 300.);
    let offset = pos2(450., 287.5);
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(start)]);
    plan_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(start),
            Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(offset)]);
    assert!(
        matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if (points[0].y-0.25).abs()<1e-4 && (points[1].x-1.75).abs()<1e-4),
        "shape={:?} edit={:?} status={}",
        app.editor.project.elements[0].shape,
        app.polyline_edit
            .as_ref()
            .map(|edit| (edit.drag.is_some(), edit.error.clone())),
        app.editor.status
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: offset,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert_eq!(
        app.editor.undo.len(),
        1,
        "whole-path offset commits as one operation"
    );
    app.editor.undo();
    assert!(
        matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points[0]==Point::new(0.,0.) && points[1]==Point::new(2.,0.))
    );

    app.open_polyline_edit(id, Some(1), Some(0), start);
    app.polyline_edit.as_mut().unwrap().command = PolylineEditCommand::CurveEdge;
    let arc = pos2(450., 250.);
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(start)]);
    plan_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(start),
            Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(arc)]);
    assert!(
        matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, ref arcs} if points.len()==3 && arcs[0].is_some_and(|arc| (arc.radius-1.).abs()<1e-3))
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: arc,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert_eq!(
        app.editor.undo.len(),
        1,
        "curve conversion commits as one operation"
    );
    app.editor.undo();
    assert!(
        matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points.len()==3)
    );
}
#[test]
fn dragging_selected_polyline_body_moves_whole_object_with_arrow_without_palette() {
    let (ctx, mut app) = setup();
    let id = app.editor.project.add(
        0,
        Material::Plaster,
        Shape::Polyline {
            points: vec![Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 2.)],
            arcs: vec![],
        },
    );
    let start = pos2(450., 300.);
    let end = pos2(500., 250.);
    click(&ctx, &mut app, start);
    assert_eq!(app.editor.selection_ids(), vec![id]);
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
    let moving_cursor = plan_frame(&ctx, &mut app, vec![Event::PointerMoved(end)]);
    assert!(app.drag.is_some());
    assert!(app.polyline_edit.is_none());
    assert_eq!(
        moving_cursor,
        CursorIcon::Default,
        "moving a selected Polyline keeps the arrow cursor"
    );
    assert!(
        app.polyline_edit.is_none(),
        "a move gesture must not open the edit palette"
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: end,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert!(
        matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points[0]==Point::new(1.,1.) && points[1]==Point::new(3.,1.) && points[2]==Point::new(3.,3.))
    );
    assert!(app.polyline_edit.is_none());
    assert_eq!(app.editor.undo.len(), 1);
}
#[test]
fn moving_polyline_node_keeps_arrow_cursor_while_middle_pan_uses_hand() {
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
    let node = pos2(500., 300.);
    let click_cursor = click(&ctx, &mut app, node);
    assert_eq!(
        click_cursor,
        CursorIcon::Default,
        "opening the node palette should not let its controls replace the arrow cursor"
    );

    let away_from_geometry = pos2(650., 450.);
    assert_eq!(
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::PointerMoved(away_from_geometry)]
        ),
        CursorIcon::Default,
        "Move node mode should keep the arrow while the pointer moves off the node"
    );
    assert_eq!(
        plan_frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(node),
                Event::PointerButton {
                    pos: node,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        ),
        CursorIcon::Default,
        "node editing should keep the arrow while the primary button is held"
    );
    let moved_node = node + vec2(30., -30.);
    assert_eq!(
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(moved_node)]),
        CursorIcon::Default,
        "dragging the node should keep the arrow cursor"
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: moved_node,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: node,
            button: PointerButton::Middle,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    assert_eq!(
        plan_frame(
            &ctx,
            &mut app,
            vec![Event::PointerMoved(node + vec2(20., 20.))]
        ),
        CursorIcon::Grabbing,
        "middle-button view pan should keep the hand cursor"
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
#[test]
fn chamfer_and_fillet_edit_polyline_corners_and_undo() {
    let (_ctx, mut app) = setup();
    let id = app.editor.project.add(
        0,
        Material::Plaster,
        Shape::Polyline {
            points: vec![Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 2.)],
            arcs: vec![],
        },
    );
    app.open_polyline_edit(id, Some(1), None, pos2(500., 300.));
    app.polyline_edit.as_mut().unwrap().command = PolylineEditCommand::Chamfer;
    app.apply_polyline_corner_edit();
    assert!(
        matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points.len()==4)
    );
    assert_eq!(app.editor.undo.len(), 1);
    app.editor.undo();
    app.open_polyline_edit(id, Some(1), None, pos2(500., 300.));
    app.polyline_edit.as_mut().unwrap().command = PolylineEditCommand::Fillet;
    app.apply_polyline_corner_edit();
    assert!(
        matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, ref arcs} if points.len()==4 && arcs[1].is_some())
    );
    assert!(app.editor.project.validate().is_ok());
}
#[test]
fn polyline_edge_offset_move_rotate_mirror_and_multiply_preview_geometry() {
    let (_ctx, mut app) = setup();
    let id = app.editor.project.add(
        0,
        Material::Plaster,
        Shape::Polyline {
            points: vec![Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 2.)],
            arcs: vec![],
        },
    );
    let before = app.editor.project.clone();
    let mut edit = PolylineEdit {
        element_id: id,
        node: None,
        edge: Some(0),
        command: PolylineEditCommand::OffsetPolyline,
        palette_at: Pos2::ZERO,
        drag: None,
        chamfer_length: 0.25,
        fillet_radius: 0.25,
        multiply_count: 3,
        error: String::new(),
    };
    app.apply_polyline_drag_preview(&before, &edit, Point::new(1., 0.), Point::new(1., 0.25));
    assert!(
        matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points[0]==Point::new(0.,0.25) && points[1]==Point::new(1.75,0.25) && points[2]==Point::new(1.75,2.))
    );
    edit.command = PolylineEditCommand::CurveEdge;
    app.apply_polyline_drag_preview(&before, &edit, Point::new(1., 0.), Point::new(1., 1.));
    let Shape::Polyline { points, arcs } = &app.editor.project.elements[0].shape else {
        panic!("curve edit changed element type");
    };
    assert_eq!(
        points,
        &[Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 2.)],
        "curve stays a single segment with endpoint nodes only"
    );
    assert!(arcs[0].is_some());
    assert!(
        geometry::polyline_path(points, arcs).len() > 18,
        "display tessellation is generated without changing model vertices"
    );
    let first_radius = arcs[0].unwrap().radius;
    app.apply_polyline_drag_preview(&before, &edit, Point::new(1., 0.), Point::new(1., 0.5));
    let Shape::Polyline { points, arcs } = &app.editor.project.elements[0].shape else {
        panic!("curve preview changed element type");
    };
    assert_eq!(points.len(), 3, "drag preview never creates arc nodes");
    assert!(
        arcs[0].unwrap().radius > first_radius,
        "moving the pointer updates the circle radius live"
    );
    let closed_id = app.editor.project.add(
        0,
        Material::Plaster,
        Shape::Polyline {
            points: vec![
                Point::new(0., 0.),
                Point::new(2., 0.),
                Point::new(2., 2.),
                Point::new(0., 2.),
                Point::new(0., 0.),
            ],
            arcs: vec![],
        },
    );
    let closed_before = app.editor.project.clone();
    edit.command = PolylineEditCommand::OffsetPolyline;
    edit.element_id = closed_id;
    app.apply_polyline_drag_preview(
        &closed_before,
        &edit,
        Point::new(1., 0.),
        Point::new(1., 0.25),
    );
    assert!(
        matches!(app.editor.project.elements.iter().find(|element|element.id==closed_id).unwrap().shape,Shape::Polyline{ref points, .. } if points[0]==Point::new(0.25,0.25) && points[1]==Point::new(1.75,0.25) && points[4]==points[0])
    );
    edit.element_id = id;
    edit.command = PolylineEditCommand::Move;
    app.apply_polyline_drag_preview(&before, &edit, Point::default(), Point::new(1., 1.));
    assert!(
        matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points[0]==Point::new(1.,1.))
    );
    edit.command = PolylineEditCommand::Rotate;
    edit.node = Some(0);
    app.apply_polyline_drag_preview(&before, &edit, Point::new(1., 0.), Point::new(0., 1.));
    assert!(
        matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if (points[1].x).abs()<1e-5 && (points[1].y-2.).abs()<1e-5)
    );
    edit.command = PolylineEditCommand::Mirror;
    app.apply_polyline_drag_preview(&before, &edit, Point::default(), Point::new(0., 2.));
    assert!(
        matches!(app.editor.project.elements[0].shape,Shape::Polyline{ref points, .. } if points[1]==Point::new(-2.,0.))
    );
    edit.command = PolylineEditCommand::Multiply;
    edit.node = None;
    app.apply_polyline_drag_preview(&before, &edit, Point::default(), Point::new(1., 1.));
    assert_eq!(app.editor.project.elements.len(), 4);
    assert!(
        app.editor
            .project
            .elements
            .windows(2)
            .all(|pair| pair[0].id != pair[1].id)
    );
    assert!(
        matches!(app.editor.project.elements[3].shape,Shape::Polyline{ref points, .. } if points[0]==Point::new(3.,3.))
    );
    assert!(app.editor.project.validate().is_ok());
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
        egui::Shape::Path(path) if path.fill == WALL_SELECTION_FILL
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
fn wall_baseline_drag_moves_the_whole_reference_line() {
    let (ctx, mut app) = setup();
    let id = add_wall_for_editing(&mut app);
    app.editor.set_selection(vec![id]);

    drag(&ctx, &mut app, pos2(500., 300.), pos2(550., 300.));

    assert_eq!(
        wall_reference_line(&app, id),
        (Point::new(1., 0.), Point::new(5., 0.))
    );
    assert_eq!(app.editor.undo.len(), 1);
    assert_eq!(app.editor.selection_ids(), vec![id]);
    assert!(app.wall_edit.is_none());
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
    let footprints = geometry::plan_wall_footprints(&walls);
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
        geometry::plan_wall_attached_ends(&walls, &footprints).contains(&(id, true)),
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
    let footprints = geometry::plan_wall_footprints(&walls);
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
        geometry::plan_wall_attached_ends(&walls, &footprints).contains(&(id, true)),
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
    let edit = app.wall_edit.as_ref().expect("line edit");
    assert_eq!((edit.element_id, edit.node), (id, None));
    assert_eq!(edit.command, WallEditCommand::Move);
    assert!(app.editor.status.starts_with("Wall reference-line edit"));

    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.wall_edit.is_none());
    assert_eq!(app.editor.selection_ids(), vec![id]);
}
#[test]
fn wall_reference_line_palette_renders_in_both_languages() {
    let (ctx, mut app) = setup();
    let id = add_wall_for_editing(&mut app);
    app.editor.set_selection(vec![id]);
    click(&ctx, &mut app, pos2(400., 300.));
    assert!(app.wall_edit.is_some());

    let output = plan_frame_output(&ctx, &mut app, vec![]);
    let mut english = Vec::new();
    collect_text(&output.shapes, &mut english);
    assert!(english.iter().any(|text| text == "Wall reference line"));
    assert!(english.iter().any(|text| text == "Start endpoint"));
    assert!(english.iter().any(|text| text.contains("Length:")));
    assert!(
        english
            .iter()
            .any(|text| text == "Drag onto another reference line to join")
    );

    app.language = UiLanguage::SimplifiedChinese;
    let output = plan_frame_output(&ctx, &mut app, vec![]);
    let mut chinese = Vec::new();
    collect_text(&output.shapes, &mut chinese);
    assert!(chinese.iter().any(|text| text == "墙参考线"));
    assert!(chinese.iter().any(|text| text == "起点"));
    assert!(chinese.iter().any(|text| text.contains("长度:")));
    assert!(
        chinese
            .iter()
            .any(|text| text == "拖到另一条参考线上以自动连接")
    );
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

    let output = plan_frame_output(&ctx, &mut app, vec![]);
    let done = text_center(&output.shapes, "Done").expect("Done button label");
    click(&ctx, &mut app, done);
    assert!(app.wall_edit.is_none(), "Done dismisses the unchanged edit");
    assert!(app.editor.undo.is_empty());
    assert_eq!(app.editor.status, "Select an element or start drawing");
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
    for expected in ["OpenArchi", "File", "INFO BOX", "02 Drafting", "1:100"] {
        assert!(
            text.iter().any(|value| value.contains(expected)),
            "workspace shell, inspector or Quick Options did not render {expected}: {text:?}"
        );
    }
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
    assert!(english.iter().any(|text| text.contains("02 Drafting")));

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
    assert!(chinese.iter().any(|text| text.contains("02 绘图")));
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
fn quick_option_element(layer: Layer, design_option: Option<u32>) -> Element {
    Element {
        id: 1,
        name: "Wall 001".into(),
        story: 0,
        material: Material::Concrete,
        layer,
        renovation: RenovationStatus::Existing,
        design_option,
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
        app.editor.project.plan_options.layer_combination,
        LayerCombination::Drafting
    );
    assert_eq!(
        app.editor.project.plan_options.pen_set,
        PenSet::Architectural100
    );
    assert_eq!(
        app.editor.project.plan_options.partial_structure,
        PartialStructureDisplay::EntireModel
    );

    app.set_layer_combination(LayerCombination::PlansDetailed);
    app.set_pen_set(PenSet::ColorConceptual);
    app.set_partial_structure(PartialStructureDisplay::WithoutFinishes);
    app.set_model_view(ModelViewOptions::CeilingPlans);
    app.set_graphic_overrides(GraphicOverrides::FirePlan);
    app.set_renovation_filter(RenovationFilter::NewConstruction);
    app.set_dimension_style(DimensionStyle::PlainMillimeter);
    assert_eq!(
        app.editor.project.plan_options.layer_combination,
        LayerCombination::PlansDetailed
    );
    assert_eq!(
        app.editor.project.plan_options.pen_set,
        PenSet::ColorConceptual
    );
    assert_eq!(
        app.editor.project.plan_options.partial_structure,
        PartialStructureDisplay::WithoutFinishes
    );
    assert_eq!(
        app.editor.project.plan_options.model_view,
        ModelViewOptions::CeilingPlans
    );
    assert_eq!(
        app.editor.project.plan_options.graphic_overrides,
        GraphicOverrides::FirePlan
    );
    assert_eq!(
        app.editor.project.plan_options.renovation_filter,
        RenovationFilter::NewConstruction
    );
    assert_eq!(
        app.editor.project.plan_options.dimension_style,
        DimensionStyle::PlainMillimeter
    );
    assert_eq!(app.editor.undo.len(), 7);

    for _ in 0..7 {
        app.editor.undo();
    }
    assert_eq!(app.editor.project.plan_options, PlanOptions::default());
}

#[test]
fn quick_option_visibility_follows_layers_and_design_options() {
    let (_ctx, mut app) = setup();
    let main = quick_option_element(Layer::Structural, None);
    let mut option_element = main.clone();
    option_element.id = 2;
    let option_id = app.editor.project.design_options.create("Option A".into());
    option_element.design_option = Some(option_id);
    app.editor.project.elements = vec![main.clone(), option_element.clone()];

    app.set_layer_combination(LayerCombination::Layouting);
    assert!(!app.editor.project.plan_options.element_visible(&main));
    assert!(
        !app.editor
            .project
            .plan_options
            .element_visible(&option_element)
    );
    app.set_show_all_layers(true);
    assert!(app.editor.project.plan_options.element_visible(&main));

    app.set_active_design_option(None);
    assert!(app.editor.project.plan_options.element_visible(&main));
    assert!(
        !app.editor
            .project
            .plan_options
            .element_visible(&option_element)
    );
    app.set_active_design_option(Some(option_id));
    assert!(
        app.editor
            .project
            .plan_options
            .element_visible(&option_element)
    );
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
        state.draft.layer = Layer::Structural;
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
    assert_eq!(app.editor.wall_layer, Layer::Structural);

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
    assert_eq!(wall.layer, Layer::Structural);
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
