fn shortcut_slab(app: &mut AtelierApp) -> u64 {
    app.host.document.editor.project.add(
        0,
        Material::Concrete,
        Shape::Slab {
            min: Point::new(-2., -2.),
            max: Point::new(2., 2.),
            thickness: 0.2,
        },
    )
}
fn selection_shortcut_frame(ctx: &Context, app: &mut AtelierApp, model: bool, events: Vec<Event>) {
    if model {
        model_frame(ctx, app, events);
    } else {
        plan_frame(ctx, app, events);
    }
}
fn selection_shortcut_click(
    ctx: &Context,
    app: &mut AtelierApp,
    model: bool,
    pos: Pos2,
    down: Modifiers,
    up: Modifiers,
) {
    selection_shortcut_frame(
        ctx,
        app,
        model,
        vec![Event::ModifiersChanged(down), Event::PointerMoved(pos)],
    );
    selection_shortcut_frame(
        ctx,
        app,
        model,
        vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: down,
        }],
    );
    selection_shortcut_frame(
        ctx,
        app,
        model,
        vec![
            Event::ModifiersChanged(up),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: up,
            },
        ],
    );
}
fn selection_space_release() -> Event {
    Event::Key {
        key: Key::Space,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn selection_shortcuts_tab_previews_cycles_confirms_and_filters_permissions() {
    let (ctx, mut app) = setup();
    let a = shortcut_slab(&mut app);
    let b = shortcut_slab(&mut app);
    let locked = shortcut_slab(&mut app);
    app.host
        .document
        .editor
        .project
        .edit_state
        .locked
        .insert(locked);
    app.host.document.editor.set_tool(Tool::Select);
    let before = app.host.document.editor.project.clone();
    let revision = app.host.document.editor.revision;
    let pos = pos2(400., 300.);
    let owner = Id::new("plan_canvas_selection");
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos)]);
    plan_frame(&ctx, &mut app, vec![]);
    assert_eq!(app.canvas_hover_id(owner), Some(b));
    assert!(app.host.document.editor.selection_ids().is_empty());
    plan_frame(&ctx, &mut app, vec![key(Key::Tab, Modifiers::NONE)]);
    assert_eq!(app.canvas_hover_id(owner), Some(a));
    selection_shortcut_click(&ctx, &mut app, false, pos, Modifiers::NONE, Modifiers::NONE);
    assert_eq!(app.host.document.editor.selection_ids(), vec![a]);
    app.select_locked_elements = true;
    plan_frame(&ctx, &mut app, vec![]);
    assert_eq!(app.canvas_hover_id(owner), Some(locked));
    app.host
        .document
        .editor
        .project
        .elements
        .retain(|e| e.id != locked);
    plan_frame(&ctx, &mut app, vec![]);
    assert_ne!(app.canvas_hover_id(owner), Some(locked));
    app.host.document.editor.project = before.clone();
    assert_eq!(app.host.document.editor.project, before);
    assert_eq!(app.host.document.editor.revision, revision);
    assert!(app.host.document.editor.undo.is_empty());
}

#[test]
fn selection_shortcuts_shift_idle_tools_add_remove_and_release_before_click_up_never_create() {
    for tool in [
        Tool::Wall,
        Tool::Slab,
        Tool::Beam,
        Tool::Column,
        Tool::ConceptMass,
        Tool::Zone,
        Tool::Roof,
        Tool::Polyline,
    ] {
        let (ctx, mut app) = setup();
        let id = shortcut_slab(&mut app);
        app.host.document.editor.set_tool(tool);
        let before = app.host.document.editor.project.clone();
        let dirty = app.host.document.editor.dirty();
        selection_shortcut_click(
            &ctx,
            &mut app,
            false,
            pos2(400., 300.),
            Modifiers::SHIFT,
            Modifiers::NONE,
        );
        assert_eq!(
            app.host.document.editor.selection_ids(),
            vec![id],
            "{tool:?}"
        );
        assert_eq!(app.host.document.editor.tool, tool);
        selection_shortcut_click(
            &ctx,
            &mut app,
            false,
            pos2(400., 300.),
            Modifiers::SHIFT,
            Modifiers::SHIFT,
        );
        assert!(
            app.host.document.editor.selection_ids().is_empty(),
            "{tool:?}"
        );
        assert_eq!(app.host.document.editor.project, before);
        assert_eq!(app.host.document.editor.dirty(), dirty);
        assert!(app.host.document.editor.undo.is_empty());
        assert!(!app.host.document.editor.model_input_active());
    }
}

#[test]
fn selection_shortcuts_active_draft_keeps_shift_construction_and_space_does_not_select() {
    let (ctx, mut app) = setup();
    let id = shortcut_slab(&mut app);
    app.host.document.editor.set_tool(Tool::Wall);
    app.host
        .document
        .editor
        .wall_settings
        .geometry
        .geometry_method = WallGeometryMethod::Continuous;
    click(&ctx, &mut app, pos2(170., 480.));
    let accepted = app.host.document.editor.wall_points.clone();
    assert!(!accepted.is_empty());
    let before = app.host.document.editor.project.clone();
    selection_shortcut_click(
        &ctx,
        &mut app,
        false,
        pos2(400., 300.),
        Modifiers::SHIFT,
        Modifiers::SHIFT,
    );
    assert!(app.host.document.editor.wall_points.starts_with(&accepted));
    assert!(app.host.document.editor.wall_points.len() > accepted.len());
    assert!(!app.host.document.editor.selection_ids().contains(&id));
    assert_eq!(app.host.document.editor.project, before);
    assert!(app.host.document.editor.undo.is_empty());
}

#[test]
fn selection_shortcuts_surface_switch_and_space_are_temporary_and_preserve_magic_wand() {
    for enabled in [false, true] {
        for tool in [Tool::Select, Tool::Wall, Tool::Slab, Tool::ConceptMass] {
            let (ctx, mut app) = setup();
            let id = shortcut_slab(&mut app);
            app.canvas_pick.surface = enabled;
            app.host.document.editor.set_tool(tool);
            let modifiers = if tool == Tool::Select {
                Modifiers::NONE
            } else {
                Modifiers::SHIFT
            };
            let before = app.host.document.editor.project.clone();
            plan_frame(&ctx, &mut app, vec![key(Key::Space, modifiers)]);
            selection_shortcut_click(
                &ctx,
                &mut app,
                false,
                pos2(400., 300.),
                modifiers,
                modifiers,
            );
            plan_frame(&ctx, &mut app, vec![selection_space_release()]);
            assert_eq!(
                app.host.document.editor.selection_ids(),
                if enabled { vec![] } else { vec![id] },
                "{tool:?} surface {enabled}"
            );
            assert_eq!(app.canvas_pick.surface, enabled);
            assert_eq!(app.host.document.editor.project, before);
            assert!(app.host.document.editor.undo.is_empty());
            assert!(app.reshape_session.is_none());
        }
    }
}

#[test]
fn selection_shortcuts_model_overlap_shift_and_draft_protection() {
    let (ctx, mut app) = setup();
    let a = shortcut_slab(&mut app);
    let b = shortcut_slab(&mut app);
    app.host.document.editor.set_tool(Tool::Select);
    app.camera = geometry::Camera {
        yaw: 0.,
        pitch: 1.2,
        span: 8.,
        target: glam::DVec3::ZERO,
    };
    let before = app.host.document.editor.project.clone();
    let pos = pos2(400., 300.);
    model_frame(&ctx, &mut app, vec![Event::PointerMoved(pos)]);
    model_frame(&ctx, &mut app, vec![]);
    let owner = Id::new("model_canvas_selection");
    let first = app.canvas_hover_id(owner).unwrap();
    model_frame(&ctx, &mut app, vec![key(Key::Tab, Modifiers::NONE)]);
    let second = app.canvas_hover_id(owner).unwrap();
    assert_ne!(first, second);
    selection_shortcut_click(&ctx, &mut app, true, pos, Modifiers::NONE, Modifiers::NONE);
    assert_eq!(app.host.document.editor.selection_ids(), vec![second]);
    app.host.document.editor.set_tool(Tool::Column);
    selection_shortcut_click(&ctx, &mut app, true, pos, Modifiers::SHIFT, Modifiers::NONE);
    assert_eq!(app.host.document.editor.tool, Tool::Column);
    app.host.document.editor.anchor = Some(Point::new(8., 8.));
    app.draft_id = Some(a);
    selection_shortcut_click(
        &ctx,
        &mut app,
        true,
        pos,
        Modifiers::SHIFT,
        Modifiers::SHIFT,
    );
    assert_eq!(app.host.document.editor.anchor, Some(Point::new(8., 8.)));
    assert_eq!(app.draft_id, Some(a));
    assert_eq!(app.host.document.editor.project, before);
    assert!(app.host.document.editor.undo.is_empty());
    assert_ne!(a, b);
}

#[test]
fn selection_shortcuts_cancel_modal_and_focus_cannot_commit_or_steal_tab() {
    let (ctx, mut app) = setup();
    shortcut_slab(&mut app);
    shortcut_slab(&mut app);
    app.host.document.editor.set_tool(Tool::Wall);
    let before = app.host.document.editor.project.clone();
    let pos = pos2(400., 300.);
    plan_frame(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(Modifiers::SHIFT),
            Event::PointerMoved(pos),
            key(Key::Tab, Modifiers::SHIFT),
        ],
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::SHIFT,
        }],
    );
    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::SHIFT)]);
    plan_frame(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(Modifiers::NONE),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    assert!(app.host.document.editor.selection_ids().is_empty());
    assert_eq!(app.host.document.editor.project, before);
    assert_eq!(app.host.document.editor.tool, Tool::Wall);
    app.help = true;
    selection_shortcut_click(
        &ctx,
        &mut app,
        false,
        pos,
        Modifiers::SHIFT,
        Modifiers::NONE,
    );
    assert!(app.host.document.editor.selection_ids().is_empty());
    assert_eq!(app.host.document.editor.project, before);
    assert!(
        !app.host.document.editor.model_input_active(),
        "modal click must not start a draft"
    );
    app.help = false;
    plan_frame(&ctx, &mut app, vec![Event::WindowFocused(false)]);
    selection_shortcut_click(
        &ctx,
        &mut app,
        false,
        pos,
        Modifiers::SHIFT,
        Modifiers::NONE,
    );
    assert_eq!(app.host.document.editor.project, before);
}

#[test]
fn selection_shortcuts_cancel_then_click_same_position_recovers_candidate() {
    let (ctx, mut app) = setup();
    let a = shortcut_slab(&mut app);
    let b = shortcut_slab(&mut app);
    app.host.document.editor.set_tool(Tool::Select);
    let pos = pos2(400., 300.);
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos)]);
    plan_frame(&ctx, &mut app, vec![]);
    plan_frame(&ctx, &mut app, vec![key(Key::Tab, Modifiers::NONE)]);
    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    selection_shortcut_click(&ctx, &mut app, false, pos, Modifiers::NONE, Modifiers::NONE);
    assert_eq!(app.host.document.editor.selection_ids(), vec![b]);
    assert_ne!(a, b);
}

#[test]
fn selection_shortcuts_space_drag_pans_without_selecting_and_release_before_up_keeps_intent() {
    let (ctx, mut app) = setup();
    let id = shortcut_slab(&mut app);
    app.host.document.editor.set_tool(Tool::Select);
    app.canvas_pick.surface = false;
    let before = app.host.document.editor.project.clone();
    let origin = app.plan_center;
    let p = pos2(400., 300.);
    let q = pos2(445., 325.);
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerMoved(p), key(Key::Space, Modifiers::NONE)],
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(q)]);
    plan_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerButton {
                pos: q,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
            selection_space_release(),
        ],
    );
    assert_ne!(app.plan_center, origin);
    assert!(app.host.document.editor.selection_ids().is_empty());
    assert_eq!(app.host.document.editor.project, before);
    app.plan_center = origin;
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerMoved(p), key(Key::Space, Modifiers::NONE)],
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    plan_frame(&ctx, &mut app, vec![selection_space_release()]);
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert_eq!(app.host.document.editor.selection_ids(), vec![id]);
    assert_eq!(app.host.document.editor.project, before);
    assert!(app.host.document.editor.undo.is_empty());
}

#[test]
fn selection_shortcuts_group_toggle_stays_inside_visible_permissions() {
    let (ctx, mut app) = setup();
    let a = shortcut_slab(&mut app);
    let b = shortcut_slab(&mut app);
    let hidden = shortcut_slab(&mut app);
    app.host.document.editor.project.edit_state.groups = vec![vec![a, b, hidden]];
    app.host
        .document
        .editor
        .project
        .edit_state
        .locked
        .insert(hidden);
    app.host.document.editor.set_tool(Tool::Select);
    selection_shortcut_click(
        &ctx,
        &mut app,
        false,
        pos2(400., 300.),
        Modifiers::NONE,
        Modifiers::NONE,
    );
    assert_eq!(app.host.document.editor.selection_ids(), vec![a, b]);
    selection_shortcut_click(
        &ctx,
        &mut app,
        false,
        pos2(400., 300.),
        Modifiers::SHIFT,
        Modifiers::SHIFT,
    );
    assert!(app.host.document.editor.selection_ids().is_empty());
}

#[test]
fn selection_shortcuts_text_focus_keeps_tab_and_selection_out_of_canvas() {
    let (ctx, mut app) = setup();
    shortcut_slab(&mut app);
    shortcut_slab(&mut app);
    app.host.document.editor.set_tool(Tool::Select);
    let before = app.host.document.editor.project.clone();
    let mut value = String::from("input");
    let id = Id::new("selection_shortcuts_text");
    let mut frame = |app: &mut AtelierApp, events: Vec<Event>, request: bool| {
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
                events,
                ..Default::default()
            },
            |ui| {
                let response = ui.add(egui::TextEdit::singleline(&mut value).id(id));
                if request {
                    response.request_focus();
                }
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(Rect::from_min_max(pos2(0., 60.), pos2(800., 600.))),
                    |ui| app.plan(ui),
                );
            },
        );
        output.textures_delta.clear();
    };
    frame(&mut app, vec![], true);
    frame(&mut app, vec![Event::PointerMoved(pos2(400., 300.))], false);
    assert!(
        app.canvas_hover_id(Id::new("plan_canvas_selection"))
            .is_none()
    );
    frame(&mut app, vec![key(Key::Tab, Modifiers::NONE)], false);
    assert!(app.host.document.editor.selection_ids().is_empty());
    assert_eq!(app.host.document.editor.project, before);
}

#[test]
fn selection_shortcuts_active_guide_command_keeps_shift_input_and_escape() {
    let (ctx, mut app) = setup();
    shortcut_slab(&mut app);
    app.host.document.editor.set_tool(Tool::Select);
    app.guide_ui.action = Some(GuideAction::Erase);
    let before = app.host.document.editor.project.clone();
    selection_shortcut_click(
        &ctx,
        &mut app,
        false,
        pos2(400., 300.),
        Modifiers::SHIFT,
        Modifiers::NONE,
    );
    assert!(app.host.document.editor.selection_ids().is_empty());
    assert_eq!(app.host.document.editor.project, before);
    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.guide_ui.action.is_none());
}
