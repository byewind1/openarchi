// User-requested input ownership: a prepared drag cannot consume another editor's keys.
fn precision_prepared_drag(ctx: &Context, app: &mut AtelierApp) {
    shortcut_test_wall(app);
    app.host.document.editor.move_selection(Point::new(1., 0.));
    app.host.document.editor.move_selection(Point::new(1., 0.));
    app.host.document.editor.undo();
    assert_eq!(app.host.document.editor.undo.len(), 1);
    assert_eq!(app.host.document.editor.redo.len(), 1);
    app.dispatch_edit("Drag");
    click(ctx, app, pos2(400., 300.));
    plan_frame(ctx, app, vec![Event::PointerMoved(pos2(500., 300.))]);
    drag_tracker_type(ctx, app, Key::D, "2");
    drag_tracker_type(ctx, app, Key::A, "0");
}

#[test]
fn precision_drag_modal_keys_preserve_prepared_input_project_and_history() {
    for modal in 0..4 {
        let (ctx, mut app) = setup();
        precision_prepared_drag(&ctx, &mut app);
        let before = app.host.document.editor.project.clone();
        let selection = app.host.document.editor.selection_ids();
        let revision = app.host.document.editor.revision;
        match modal {
            0 => app.help = true,
            1 => app.command_dialog = Some("input ownership guard".into()),
            2 => app.close_prompt = true,
            _ => app.workbench.configuration_open = true,
        }
        for events in [
            vec![key(Key::D, Modifiers::NONE), Event::Text("99".into())],
            vec![key(Key::Enter, Modifiers::NONE)],
            vec![
                key(Key::Backspace, Modifiers::NONE),
                key(Key::Delete, Modifiers::NONE),
            ],
            vec![key(Key::Escape, Modifiers::NONE)],
        ] {
            plan_frame(&ctx, &mut app, events);
            assert_eq!(app.host.document.editor.project, before, "modal {modal}");
            assert_eq!(app.host.document.editor.selection_ids(), selection);
            assert_eq!(app.host.document.editor.revision, revision);
            assert_eq!(app.host.document.editor.undo.len(), 1);
            assert_eq!(app.host.document.editor.redo.len(), 1);
            assert!(app.pointer_drag.is_some());
        }
        app.help = false;
        app.command_dialog = None;
        app.close_prompt = false;
        app.workbench.configuration_open = false;
        plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
        assert!(app.pointer_drag.is_none());
        assert_eq!(wall_reference_line(&app, 1).0, Point::new(3., 0.));
        assert_eq!(app.host.document.editor.undo.len(), 2);
        app.host.document.editor.undo();
        assert_eq!(app.host.document.editor.project, before);
        app.host.document.editor.redo();
        assert_eq!(wall_reference_line(&app, 1).0, Point::new(3., 0.));
    }
}

#[test]
fn precision_drag_text_focus_does_not_consume_confirm_cancel_or_delete() {
    let (ctx, mut app) = setup();
    precision_prepared_drag(&ctx, &mut app);
    let before = app.host.document.editor.project.clone();
    let revision = app.host.document.editor.revision;
    let field = Id::new("precision_foreign_text");
    let mut text = String::from("输入内容");
    for k in [Key::Enter, Key::Escape, Key::Backspace, Key::Delete] {
        let mut output = ctx.run_ui(RawInput::default(), |ui| {
            ui.add(TextEdit::singleline(&mut text).id(field))
                .request_focus();
        });
        output.textures_delta.clear();
        assert!(ctx.text_edit_focused());
        plan_frame(&ctx, &mut app, vec![key(k, Modifiers::NONE)]);
        assert!(app.pointer_drag.is_some(), "foreign text key {k:?}");
        assert_eq!(app.host.document.editor.project, before);
        assert_eq!(app.host.document.editor.revision, revision);
        assert_eq!(app.host.document.editor.undo.len(), 1);
        assert_eq!(app.host.document.editor.redo.len(), 1);
    }
    ctx.memory_mut(|m| m.surrender_focus(field));
    plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
    assert!(app.pointer_drag.is_none());
    assert_eq!(wall_reference_line(&app, 1).0, Point::new(3., 0.));
}

#[test]
fn precision_drag_background_events_do_not_commit_and_fresh_input_resumes() {
    let (ctx, mut app) = setup();
    precision_prepared_drag(&ctx, &mut app);
    let before = app.host.document.editor.project.clone();
    let revision = app.host.document.editor.revision;
    let mut output = ctx.run_ui(
        RawInput {
            focused: false,
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
            events: vec![
                key(Key::Enter, Modifiers::NONE),
                key(Key::Delete, Modifiers::NONE),
            ],
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
    assert!(app.pointer_drag.is_some());
    assert_eq!(app.host.document.editor.project, before);
    assert_eq!(app.host.document.editor.revision, revision);
    assert_eq!(app.host.document.editor.undo.len(), 1);
    assert_eq!(app.host.document.editor.redo.len(), 1);
    plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
    assert!(app.pointer_drag.is_none());
    assert_eq!(wall_reference_line(&app, 1).0, Point::new(3., 0.));
}

#[test]
fn precision_drag_ime_owns_keys_across_frames_and_commit_is_not_placement() {
    let (ctx, mut app) = setup();
    precision_prepared_drag(&ctx, &mut app);
    let before = app.host.document.editor.project.clone();
    let revision = app.host.document.editor.revision;
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Preedit {
            text: "zhong".into(),
            active_range_chars: None,
        })],
    );
    for k in [Key::Enter, Key::Escape, Key::Backspace, Key::Delete] {
        plan_frame(&ctx, &mut app, vec![key(k, Modifiers::NONE)]);
        assert!(app.pointer_drag.is_some());
        assert_eq!(app.host.document.editor.project, before);
        assert_eq!(app.host.document.editor.revision, revision);
        assert_eq!(app.host.document.editor.undo.len(), 1);
        assert_eq!(app.host.document.editor.redo.len(), 1);
    }
    plan_frame(
        &ctx,
        &mut app,
        vec![
            Event::Ime(egui::ImeEvent::Commit("中文".into())),
            key(Key::Enter, Modifiers::NONE),
        ],
    );
    assert!(app.pointer_drag.is_some());
    assert_eq!(app.host.document.editor.project, before);
    plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
    assert!(app.pointer_drag.is_none());
    assert_eq!(wall_reference_line(&app, 1).0, Point::new(3., 0.));
}

#[test]
fn precision_origin_node_gesture_keeps_geometry_grid_and_pickup_defaults_then_saves_reopens() {
    let (ctx, mut app) = setup();
    shortcut_test_wall(&mut app);
    let original = app.host.document.editor.project.clone();
    let selection = app.host.document.editor.selection_ids();
    let defaults = app.host.document.editor.wall_settings.clone();
    let mods = Modifiers::ALT | Modifiers::SHIFT;
    plan_frame(
        &ctx,
        &mut app,
        vec![
            Event::ModifiersChanged(mods),
            Event::PointerMoved(pos2(600., 300.)),
        ],
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: pos2(600., 300.),
            button: PointerButton::Primary,
            pressed: true,
            modifiers: mods,
        }],
    );
    plan_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerButton {
                pos: pos2(600., 300.),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: mods,
            },
            Event::ModifiersChanged(Modifiers::NONE),
        ],
    );
    assert_eq!(
        app.host.document.editor.project.plan_options.user_origin,
        Some(Point::new(4., 0.))
    );
    assert_eq!(app.host.document.editor.project.elements, original.elements);
    assert_eq!(
        app.host.document.editor.project.grid_settings,
        original.grid_settings
    );
    assert_eq!(app.host.document.editor.wall_settings, defaults);
    assert_eq!(app.host.document.editor.selection_ids(), selection);
    assert!(app.pointer_drag.is_none());
    assert_eq!(app.host.document.editor.undo.len(), 1);
    let path = std::env::temp_dir().join(format!(
        "oa-precision-origin-{}.json",
        crate::storage::new_document_identity()
    ));
    crate::storage::save(&app.host.document.editor.project, &path).unwrap();
    let reopened = crate::storage::load(&path).unwrap();
    assert_eq!(reopened.plan_options.user_origin, Some(Point::new(4., 0.)));
    assert_eq!(reopened.elements, original.elements);
    std::fs::remove_file(path).unwrap();
    app.host.document.editor.undo();
    assert_eq!(app.host.document.editor.project, original);
    app.host.document.editor.redo();
    app.change_input_origin(None);
    assert_eq!(app.host.document.editor.project.elements, original.elements);
    assert_eq!(
        app.host.document.editor.project.plan_options.user_origin,
        None
    );
}

#[test]
fn precision_drag_xy_explicit_units_undo_redo_and_saved_world_result() {
    let (ctx, mut app) = setup();
    shortcut_test_wall(&mut app);
    app.change_input_origin(Some(Point::new(10., 20.)));
    let before = app.host.document.editor.project.clone();
    let history = app.host.document.editor.undo.len();
    app.dispatch_edit("Drag");
    click(&ctx, &mut app, pos2(400., 300.));
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(500., 300.))]);
    drag_tracker_type(&ctx, &mut app, Key::X, "-300cm");
    drag_tracker_type(&ctx, &mut app, Key::Y, "2m");
    assert_eq!(app.host.document.editor.project, before);
    plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
    assert!(wall_reference_line(&app, 1).0.distance(Point::new(-3., 2.)) < 1e-10);
    assert_eq!(app.host.document.editor.undo.len(), history + 1);
    let path = std::env::temp_dir().join(format!(
        "oa-precision-drag-{}.json",
        crate::storage::new_document_identity()
    ));
    crate::storage::save(&app.host.document.editor.project, &path).unwrap();
    assert_eq!(
        crate::storage::load(&path).unwrap(),
        app.host.document.editor.project
    );
    std::fs::remove_file(path).unwrap();
    app.host.document.editor.undo();
    assert_eq!(app.host.document.editor.project, before);
    app.host.document.editor.redo();
    assert!(wall_reference_line(&app, 1).0.distance(Point::new(-3., 2.)) < 1e-10);
}

#[test]
fn precision_drag_scope_change_cancels_without_history_or_deleting_selection() {
    for change in 0..4 {
        let (ctx, mut app) = setup();
        precision_prepared_drag(&ctx, &mut app);
        match change {
            0 => app.view = View::Model,
            1 => app.host.document.editor.story = 1,
            2 => {
                app.host
                    .document
                    .editor
                    .project
                    .plan_options
                    .active_worksheet = Some(42)
            }
            _ => {
                app.host
                    .document
                    .editor
                    .project
                    .plan_options
                    .dimension_style = DimensionStyle::PlainMillimeter
            }
        }
        let before = app.host.document.editor.project.clone();
        let selection = app.host.document.editor.selection_ids();
        app.cancel_pointer_drag_outside_scope();
        assert!(app.pointer_drag.is_none());
        assert_eq!(app.host.document.editor.project, before);
        assert_eq!(app.host.document.editor.selection_ids(), selection);
        assert_eq!(app.host.document.editor.undo.len(), 1);
        assert_eq!(app.host.document.editor.redo.len(), 1);
    }
}

#[test]
fn precision_drag_zero_division_blocks_pointer_and_enter_then_cancels_before_selection() {
    let (ctx, mut app) = setup();
    precision_prepared_drag(&ctx, &mut app);
    let before = app.host.document.editor.project.clone();
    let selection = app.host.document.editor.selection_ids();
    drag_tracker_type(&ctx, &mut app, Key::D, "0/");
    click(&ctx, &mut app, pos2(600., 300.));
    plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
    assert!(app.pointer_drag.is_some());
    assert_eq!(app.host.document.editor.project, before);
    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.pointer_drag.is_some());
    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.pointer_drag.is_none());
    assert_eq!(app.host.document.editor.selection_ids(), selection);
    assert_eq!(app.host.document.editor.project, before);
    assert_eq!(app.host.document.editor.undo.len(), 1);
    assert_eq!(app.host.document.editor.redo.len(), 1);
}

#[test]
fn precision_drag_escape_releases_shift_between_numeric_and_session_and_backspace_never_deletes() {
    let (ctx, mut app) = setup();
    precision_prepared_drag(&ctx, &mut app);
    let before = app.host.document.editor.project.clone();
    let selection = app.host.document.editor.selection_ids();
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::ModifiersChanged(Modifiers::SHIFT)],
    );
    for _ in 0..2 {
        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::SHIFT)]);
        assert!(app.pointer_drag.is_some());
        assert_eq!(app.host.document.editor.project, before);
    }
    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::SHIFT)]);
    assert!(app.pointer_drag.is_none());
    assert_eq!(app.host.document.editor.selection_ids(), selection);
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::ModifiersChanged(Modifiers::NONE)],
    );
    app.dispatch_edit("Drag");
    click(&ctx, &mut app, pos2(400., 300.));
    plan_frame(&ctx, &mut app, vec![key(Key::Backspace, Modifiers::NONE)]);
    assert!(app.pointer_drag.is_none());
    assert_eq!(app.host.document.editor.project, before);
    assert_eq!(app.host.document.editor.selection_ids(), selection);
    assert_eq!(app.host.document.editor.undo.len(), 1);
    assert_eq!(app.host.document.editor.redo.len(), 1);
}

#[test]
fn precision_drag_click_tracker_field_edits_without_placing_then_enter_uses_preview() {
    fn text_position(shape: &egui::Shape) -> Option<Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.text().contains("Distance [D]") => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            egui::Shape::Vec(items) => items.iter().find_map(text_position),
            _ => None,
        }
    }
    let (ctx, mut app) = setup();
    shortcut_test_wall(&mut app);
    let before = app.host.document.editor.project.clone();
    app.dispatch_edit("Drag");
    click(&ctx, &mut app, pos2(400., 300.));
    let output = plan_frame_output(&ctx, &mut app, vec![Event::PointerMoved(pos2(500., 300.))]);
    let field = output
        .shapes
        .iter()
        .find_map(|s| text_position(&s.shape))
        .expect("visible distance field");
    click(&ctx, &mut app, field);
    assert!(app.pointer_drag.is_some());
    assert_eq!(app.host.document.editor.project, before);
    plan_frame(&ctx, &mut app, vec![Event::Text("2m".into())]);
    assert_eq!(app.host.document.editor.project, before);
    plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
    assert!(app.pointer_drag.is_none());
    assert!(wall_reference_line(&app, 1).0.distance(Point::new(2., 0.)) < 1e-10);
    assert_eq!(app.host.document.editor.undo.len(), 1);
}

#[test]
fn precision_drag_escape_retains_then_clears_selection_before_existing_marquee() {
    let (ctx, mut app) = setup();
    precision_prepared_drag(&ctx, &mut app);
    app.host.document.editor.marquee = crate::editor::MarqueeArea::new(
        geometry::rectangle_polygon(Point::new(-1., -1.), Point::new(7., 2.)),
        crate::editor::MarqueeScope::CurrentStory,
        0,
    );
    let before = app.host.document.editor.project.clone();
    let selection = app.host.document.editor.selection_ids();
    for _ in 0..2 {
        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    }
    assert!(app.pointer_drag.is_none());
    assert_eq!(app.host.document.editor.selection_ids(), selection);
    assert!(app.host.document.editor.marquee.is_some());
    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.host.document.editor.selection_ids().is_empty());
    assert!(app.host.document.editor.marquee.is_some());
    plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.host.document.editor.marquee.is_none());
    assert_eq!(app.host.document.editor.project, before);
    assert_eq!(app.host.document.editor.undo.len(), 1);
    assert_eq!(app.host.document.editor.redo.len(), 1);
}

#[test]
fn precision_drag_user_origin_coordinates_numeric_locks_override_shift_at_same_world_point() {
    fn basis_position(shape: &egui::Shape) -> Option<Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == "Edit-origin coordinates" => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            egui::Shape::Vec(items) => items.iter().find_map(basis_position),
            _ => None,
        }
    }
    let (ctx, mut app) = setup();
    shortcut_test_wall(&mut app);
    app.change_input_origin(Some(Point::new(4., 0.)));
    let before = app.host.document.editor.project.clone();
    app.dispatch_edit("Drag");
    click(&ctx, &mut app, pos2(400., 300.));
    plan_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(pos2(499.279, 301.843)),
            key(Key::X, Modifiers::NONE),
        ],
    );
    let output = plan_frame_output(&ctx, &mut app, vec![]);
    let basis = output
        .shapes
        .iter()
        .find_map(|s| basis_position(&s.shape))
        .expect("coordinate basis control");
    click(&ctx, &mut app, basis);
    assert!(
        app.pointer_drag.is_some(),
        "basis control cannot place the drag"
    );
    assert_eq!(app.host.document.editor.project, before);
    drag_tracker_type(&ctx, &mut app, Key::X, "1m");
    drag_tracker_type(&ctx, &mut app, Key::Y, "-2m");
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::ModifiersChanged(Modifiers::SHIFT)],
    );
    assert_eq!(app.host.document.editor.project, before);
    plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::SHIFT)]);
    assert!(app.pointer_drag.is_none());
    assert!(wall_reference_line(&app, 1).0.distance(Point::new(5., -2.)) < 1e-10);
    app.host.document.editor.undo();
    assert_eq!(app.host.document.editor.project, before);
}

#[test]
fn precision_drag_reference_near_real_node_uses_exact_node_for_numeric_coordinates() {
    let (ctx, mut app) = setup();
    shortcut_test_wall(&mut app);
    app.host.document.editor.snap = true;
    app.dispatch_edit("Drag");
    // The visible endpoint's hit band contains this press; it is not the exact pixel center.
    click(&ctx, &mut app, pos2(406., 297.5));
    assert_eq!(app.pointer_drag_origin(), Some(Point::default()));
    plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(500., 300.))]);
    drag_tracker_type(&ctx, &mut app, Key::X, "1m");
    drag_tracker_type(&ctx, &mut app, Key::Y, "2m");
    plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
    assert!(wall_reference_line(&app, 1).0.distance(Point::new(1., 2.)) < 1e-10);
    assert_eq!(app.host.document.editor.undo.len(), 1);
}

#[test]
fn precision_drag_tab_does_not_focus_toolbar_and_confirm_allows_immediate_undo_redo() {
    fn with_toolbar(ctx: &Context, app: &mut AtelierApp, events: Vec<Event>) {
        let events = discrete_key_events(ctx, events);
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
                events,
                ..Default::default()
            },
            |ui| {
                app.shortcuts(ui.ctx());
                egui::Panel::top("precision_focus_toolbar").show(ui, |ui| {
                    let _ = ui.button("Home / fit project");
                });
                CentralPanel::default()
                    .frame(Frame::NONE)
                    .show(ui, |ui| app.plan(ui));
            },
        );
        output.textures_delta.clear();
    }
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app) = setup();
        app.language = language;
        precision_prepared_drag(&ctx, &mut app);
        let before = app.host.document.editor.project.clone();
        with_toolbar(&ctx, &mut app, vec![]);
        with_toolbar(&ctx, &mut app, vec![key(Key::Tab, Modifiers::NONE)]);
        assert!(ctx.memory(|memory| memory.focused()).is_none());
        with_toolbar(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
        assert!(app.pointer_drag.is_none());
        assert_eq!(wall_reference_line(&app, 1).0, Point::new(3., 0.));
        with_toolbar(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
        assert_eq!(app.host.document.editor.project, before);
        with_toolbar(
            &ctx,
            &mut app,
            vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
        );
        assert_eq!(wall_reference_line(&app, 1).0, Point::new(3., 0.));
    }
}
