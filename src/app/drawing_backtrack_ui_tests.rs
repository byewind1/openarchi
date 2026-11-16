// Full egui input routes: accepted pointer input -> keyboard -> continued drawing.
fn backtrack_configure(app: &mut AtelierApp, tool: Tool) {
    app.host.document.editor.set_tool(tool);
    app.host.document.editor.snap = false;
    app.host
        .document
        .editor
        .wall_settings
        .geometry
        .geometry_method = WallGeometryMethod::Continuous;
    app.host.document.editor.slab_method = crate::slab_creation::SlabGeometryMethod::Polygonal;
}
fn backtrack_nodes(app: &AtelierApp) -> Vec<Point> {
    let e = &app.host.document.editor;
    match e.tool {
        Tool::Wall => e.wall_points.clone(),
        Tool::Slab => e
            .slab_polygon_draft()
            .map(|d| d.points().to_vec())
            .unwrap_or_default(),
        _ => e.polyline_points.clone(),
    }
}
fn backtrack_key(ctx: &Context, app: &mut AtelierApp) {
    plan_frame(ctx, app, vec![key(Key::Backspace, Modifiers::NONE)]);
    assert!(
        !ctx.input(|i| i.key_pressed(Key::Backspace)),
        "consumed before later dispatch"
    );
}
#[test]
fn drawing_backtrack_event_sequence_abc_to_abd_completion_history_and_storage() {
    for tool in [Tool::Slab, Tool::Wall, Tool::Polyline] {
        let (ctx, mut app) = setup();
        shortcut_test_wall(&mut app);
        let baseline = app.host.document.editor.project.clone();
        let selection = app.host.document.editor.selection_ids();
        let history = app.host.document.editor.undo.len();
        backtrack_configure(&mut app, tool);
        for p in [pos2(300., 300.), pos2(500., 300.), pos2(500., 150.)] {
            click(&ctx, &mut app, p);
        }
        let abc = backtrack_nodes(&app);
        assert_eq!(abc.len(), 3, "{tool:?}");
        app.tracker_locked = true;
        app.tracker_a = 987.;
        backtrack_key(&ctx, &mut app);
        assert_eq!(backtrack_nodes(&app), abc[..2], "{tool:?}");
        assert!(!app.tracker_locked);
        assert_eq!(app.host.document.editor.project, baseline);
        assert_eq!(app.host.document.editor.selection_ids(), selection);
        assert_eq!(app.host.document.editor.undo.len(), history);
        click(&ctx, &mut app, pos2(300., 150.));
        let abd = backtrack_nodes(&app);
        assert_eq!(abd.len(), 3);
        assert_eq!(&abd[..2], &abc[..2]);
        assert_ne!(abd[2], abc[2]);
        if tool == Tool::Slab {
            click(&ctx, &mut app, pos2(300., 300.));
        } else {
            plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
        }
        assert!(backtrack_nodes(&app).is_empty());
        let result = app.host.document.editor.project.clone();
        assert_eq!(app.host.document.editor.undo.len(), history + 1);
        assert!(
            result
                .elements
                .iter()
                .any(|e| !baseline.elements.contains(e))
        );
        for e in result
            .elements
            .iter()
            .filter(|e| !baseline.elements.iter().any(|b| b.id == e.id))
        {
            let points = match &e.shape {
                Shape::Polyline { points, .. } => points.clone(),
                Shape::Wall { start, end, .. } => vec![*start, *end],
                _ => e.slab_contour.as_ref().unwrap().points.clone(),
            };
            assert!(!points.contains(&abc[2]), "removed C must not be committed");
        }
        app.host.document.editor.undo();
        assert_eq!(app.host.document.editor.project, baseline);
        app.host.document.editor.redo();
        assert_eq!(app.host.document.editor.project, result);
        let path =
            std::env::temp_dir().join(format!("oa-backtrack-{}-{tool:?}.json", std::process::id()));
        storage::save(&result, &path).unwrap();
        assert_eq!(storage::load(&path).unwrap(), result);
        std::fs::remove_file(path).unwrap();
    }
}
#[test]
fn drawing_backtrack_empty_repeated_start_and_cancel_protect_model_and_redo() {
    for tool in [
        Tool::Slab,
        Tool::Wall,
        Tool::Polyline,
        Tool::Zone,
        Tool::ConceptMass,
    ] {
        let (ctx, mut app) = setup();
        shortcut_test_wall(&mut app);
        app.host.document.editor.move_selection(Point::new(1., 0.));
        app.host.document.editor.undo();
        backtrack_configure(&mut app, tool);
        app.host.document.editor.zone_polygon = true;
        let before = app.host.document.editor.project.clone();
        let revision = app.host.document.editor.revision;
        let redo = app.host.document.editor.redo.len();
        backtrack_key(&ctx, &mut app); // Empty tool owns the key, selected wall survives.
        click(&ctx, &mut app, pos2(300., 300.));
        backtrack_key(&ctx, &mut app);
        assert!(!app.host.document.editor.model_input_active(), "{tool:?}");
        for _ in 0..3 {
            backtrack_key(&ctx, &mut app);
        }
        assert_eq!(app.host.document.editor.tool, tool);
        click(&ctx, &mut app, pos2(300., 300.));
        click(&ctx, &mut app, pos2(500., 300.));
        backtrack_key(&ctx, &mut app);
        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        assert!(!app.host.document.editor.model_input_active());
        assert_eq!(app.host.document.editor.project, before);
        assert_eq!(app.host.document.editor.revision, revision);
        assert_eq!(app.host.document.editor.redo.len(), redo);
    }
}
#[test]
fn drawing_backtrack_atomic_rectangle_arc_and_other_existing_two_point_tools() {
    for mode in 0..10 {
        let (ctx, mut app) = setup();
        let tool = [
            Tool::Slab,
            Tool::Slab,
            Tool::Wall,
            Tool::Beam,
            Tool::Stair,
            Tool::Column,
            Tool::Zone,
            Tool::ConceptMass,
            Tool::Wall,
            Tool::Beam,
        ][mode];
        app.host.document.editor.set_tool(tool);
        app.host.document.editor.snap = false;
        app.host.document.editor.column_two_click = true;
        if mode == 1 {
            app.host.document.editor.slab_method =
                crate::slab_creation::SlabGeometryMethod::RotatedRectangle;
        }
        if mode == 3 {
            app.host
                .document
                .editor
                .set_beam_method(crate::beam_creation::BeamGeometryMethod::CenterRadius);
        }
        if mode == 8 {
            app.host
                .document
                .editor
                .wall_settings
                .geometry
                .geometry_method = WallGeometryMethod::Rectangular;
        }
        let before = app.host.document.editor.project.clone();
        click(&ctx, &mut app, pos2(300., 300.));
        if mode == 1 || mode == 3 {
            click(&ctx, &mut app, pos2(500., 300.));
        }
        assert!(app.host.document.editor.model_input_active(), "mode {mode}");
        backtrack_key(&ctx, &mut app);
        assert!(
            !app.host.document.editor.model_input_active(),
            "mode {mode}"
        );
        assert_eq!(app.host.document.editor.project, before);
        assert!(app.host.document.editor.undo.is_empty());
        assert_eq!(app.host.document.editor.tool, tool);
        click(&ctx, &mut app, pos2(350., 300.));
        assert!(app.host.document.editor.model_input_active());
    }
}
#[test]
fn drawing_backtrack_preserves_precise_nodes_and_reanchors_numeric_input() {
    let (ctx, mut app) = setup();
    backtrack_configure(&mut app, Tool::Slab);
    let e = &mut app.host.document.editor;
    e.begin_slab_polygon().unwrap();
    for p in [
        Point::new(0.123456789, 0.234567891),
        Point::new(4.123456789, 0.234567891),
        Point::new(4., 4.),
    ] {
        e.accept_slab_polygon_precise_vertex(p.into()).unwrap();
    }
    let exact = e.slab_polygon_draft().unwrap().precise_points().to_vec();
    backtrack_key(&ctx, &mut app);
    let d = app.host.document.editor.slab_polygon_draft().unwrap();
    assert_eq!(d.precise_points(), &exact[..2]);
    let q = d.precise_numeric_target(2., 90.).unwrap();
    assert!((q.x - exact[1].x).abs() < 1e-14);
    assert!((q.y - exact[1].y - 2.).abs() < 1e-14);
}

#[test]
fn drawing_backtrack_text_and_numeric_fields_own_backspace() {
    for numeric in [false, true] {
        let (ctx, mut app) = setup();
        backtrack_configure(&mut app, Tool::Polyline);
        click(&ctx, &mut app, pos2(300., 300.));
        click(&ctx, &mut app, pos2(500., 300.));
        let nodes = backtrack_nodes(&app);
        let field = Id::new("backtrack.foreign.input");
        let mut text = "123".to_string();
        let mut value = 123.;
        // DragValue switches to its actual numeric TextEdit on keyboard focus.
        let mut output = ctx.run_ui(RawInput::default(), |ui| {
            if numeric {
                ui.add(DragValue::new(&mut value)).request_focus();
            } else {
                ui.add(TextEdit::singleline(&mut text).id(field))
                    .request_focus();
            }
        });
        output.textures_delta.clear();
        let mut output = ctx.run_ui(RawInput::default(), |ui| {
            if numeric {
                ui.add(DragValue::new(&mut value));
            } else {
                ui.add(TextEdit::singleline(&mut text).id(field));
            }
        });
        output.textures_delta.clear();
        assert!(ctx.text_edit_focused(), "numeric {numeric}");
        let mut visible = false;
        let mut output = ctx.run_ui(
            RawInput {
                events: vec![
                    key(Key::End, Modifiers::NONE),
                    key(Key::Backspace, Modifiers::NONE),
                ],
                ..Default::default()
            },
            |ui| {
                // Same order as the production app: shortcuts before input widgets.
                app.shortcuts(ui.ctx());
                visible = ui.input(|i| i.key_pressed(Key::Backspace));
                if numeric {
                    ui.add(DragValue::new(&mut value));
                } else {
                    ui.add(TextEdit::singleline(&mut text).id(field));
                }
            },
        );
        output.textures_delta.clear();
        assert!(visible, "the text widget still receives its editing event");
        assert_eq!(backtrack_nodes(&app), nodes);
        if !numeric {
            assert_eq!(text, "12");
        } else {
            assert_eq!(value, 12.);
        }
    }
}

#[test]
fn drawing_backtrack_ime_modal_popup_and_background_do_not_retract() {
    for blocker in 0..8 {
        let (ctx, mut app) = setup();
        backtrack_configure(&mut app, Tool::Wall);
        click(&ctx, &mut app, pos2(300., 300.));
        click(&ctx, &mut app, pos2(500., 300.));
        let nodes = backtrack_nodes(&app);
        match blocker {
            0 => app.help = true,
            1 => app.command_dialog = Some("test".into()),
            2 => app.workbench.configuration_open = true,
            3 => app.drawings_ui.open = true,
            4 => {
                egui::Popup::open_id(&ctx, Id::new("backtrack.popup"));
            }
            5 => {
                plan_frame(
                    &ctx,
                    &mut app,
                    vec![Event::Ime(egui::ImeEvent::Preedit {
                        text: "zhong".into(),
                        active_range_chars: None,
                    })],
                );
            }
            7 => {
                let mut output = ctx.run_ui(RawInput::default(), |ui| {
                    Modal::new(Id::new("backtrack.generic.modal")).show(ui.ctx(), |ui| {
                        ui.label("Modal owns input");
                    });
                });
                output.textures_delta.clear();
            }
            _ => {}
        }
        let mut output = ctx.run_ui(
            RawInput {
                focused: blocker != 6,
                events: vec![key(Key::Backspace, Modifiers::NONE)],
                ..Default::default()
            },
            |ui| app.shortcuts(ui.ctx()),
        );
        output.textures_delta.clear();
        assert_eq!(backtrack_nodes(&app), nodes, "blocker {blocker}");
        if blocker == 5 {
            plan_frame(
                &ctx,
                &mut app,
                vec![
                    Event::Ime(egui::ImeEvent::Commit("中".into())),
                    key(Key::Backspace, Modifiers::NONE),
                ],
            );
            assert_eq!(backtrack_nodes(&app), nodes);
            backtrack_key(&ctx, &mut app);
            assert_eq!(backtrack_nodes(&app).len(), 1);
        }
    }
}

#[test]
fn drawing_backtrack_repeat_modifiers_and_forward_delete_remain_distinct() {
    let (ctx, mut app) = setup();
    backtrack_configure(&mut app, Tool::Wall);
    for p in [pos2(300., 300.), pos2(500., 300.), pos2(500., 150.)] {
        click(&ctx, &mut app, p);
    }
    let nodes = backtrack_nodes(&app);
    for modifiers in [
        Modifiers::CTRL,
        Modifiers::ALT,
        Modifiers::COMMAND,
        Modifiers::SHIFT,
    ] {
        plan_frame(&ctx, &mut app, vec![key(Key::Backspace, modifiers)]);
        assert_eq!(backtrack_nodes(&app), nodes);
    }
    plan_frame(&ctx, &mut app, vec![key(Key::Backspace, Modifiers::NONE)]);
    plan_frame(
        &ctx,
        &mut app,
        vec![Event::Key {
            key: Key::Backspace,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers: Modifiers::NONE,
        }],
    );
    assert_eq!(backtrack_nodes(&app).len(), 1);
    plan_frame(&ctx, &mut app, vec![key(Key::Delete, Modifiers::NONE)]);
    assert!(
        backtrack_nodes(&app).is_empty(),
        "forward delete cancels whole wall draft"
    );
}

#[test]
fn drawing_backtrack_roof_polygon_and_atomic_constructions() {
    for method in [
        crate::roof::RoofGeometryMethod::Polygonal,
        crate::roof::RoofGeometryMethod::Rectangular,
        crate::roof::RoofGeometryMethod::RotatedRectangle,
    ] {
        let (ctx, mut app) = setup();
        app.host.document.editor.set_tool(Tool::Roof);
        app.host.document.editor.roof_method = method;
        app.host.document.editor.snap = false;
        for p in [pos2(300., 300.), pos2(500., 300.), pos2(400., 200.)] {
            click(&ctx, &mut app, p);
        }
        if method == crate::roof::RoofGeometryMethod::Polygonal {
            for p in [pos2(300., 200.), pos2(500., 200.), pos2(500., 100.)] {
                click(&ctx, &mut app, p);
            }
            backtrack_key(&ctx, &mut app);
            assert_eq!(
                app.host
                    .document
                    .editor
                    .roof_polygon_draft()
                    .unwrap()
                    .points()
                    .len(),
                2
            );
            backtrack_key(&ctx, &mut app);
            backtrack_key(&ctx, &mut app);
        } else {
            click(&ctx, &mut app, pos2(300., 200.));
            if method == crate::roof::RoofGeometryMethod::RotatedRectangle {
                click(&ctx, &mut app, pos2(500., 200.));
            }
            backtrack_key(&ctx, &mut app);
        }
        assert!(!app.host.document.editor.model_input_active());
        assert!(app.host.document.editor.project.elements.is_empty());
    }
}

#[test]
fn drawing_backtrack_marquee_plan_model_and_source_picks_share_event_route() {
    for model in [false, true] {
        let (ctx, mut app) = setup();
        app.host.document.editor.set_tool(Tool::Marquee);
        app.marquee_geometry = MarqueeGeometry::Polygon;
        if model {
            app.view = View::Model;
        }
        for p in [pos2(300., 350.), pos2(500., 350.), pos2(500., 450.)] {
            if model {
                click_model(&ctx, &mut app, p, PointerButton::Primary);
            } else {
                click(&ctx, &mut app, p);
            }
        }
        assert_eq!(app.marquee_points.len(), 3);
        let first = app.marquee_points[..2].to_vec();
        let mut output = ctx.run_ui(
            RawInput {
                events: vec![key(Key::Backspace, Modifiers::NONE)],
                ..Default::default()
            },
            |ui| {
                app.shortcuts(ui.ctx());
                if model {
                    app.model_view(ui);
                } else {
                    app.plan(ui);
                }
            },
        );
        output.textures_delta.clear();
        assert_eq!(app.marquee_points, first);
        if model {
            assert_eq!(app.marquee_3d_points.len(), 2);
        }
        assert!(app.host.document.editor.project.elements.is_empty());
    }
    for mode in [
        drawings_ui::Pick::Region,
        drawings_ui::Pick::Calibrate,
        drawings_ui::Pick::Place,
    ] {
        let (ctx, mut app) = setup();
        app.drawings_ui.mode = Some(mode);
        app.drawings_ui.points = vec![[1., 2.], [3., 4.]];
        app.drawings_ui.distance = 25.;
        let mut output = ctx.run_ui(
            RawInput {
                events: vec![key(Key::Backspace, Modifiers::NONE)],
                ..Default::default()
            },
            |ui| app.shortcuts(ui.ctx()),
        );
        output.textures_delta.clear();
        assert_eq!(app.drawings_ui.points, vec![[1., 2.]]);
        assert_eq!(app.drawings_ui.mode, Some(mode));
        assert_eq!(app.drawings_ui.distance, 25.);
    }
}

/// Explicit operator-selected local files only; never a public build dependency.
#[test]
fn drawing_backtrack_polygon_wall_and_rotated_marquee() {
    let (ctx, mut app) = setup();
    backtrack_configure(&mut app, Tool::Wall);
    app.host
        .document
        .editor
        .wall_settings
        .geometry
        .geometry_method = WallGeometryMethod::Polygonal;
    for p in [pos2(300., 300.), pos2(500., 300.), pos2(500., 150.)] {
        click(&ctx, &mut app, p);
    }
    backtrack_key(&ctx, &mut app);
    assert_eq!(app.host.document.editor.wall_points.len(), 2);
    click(&ctx, &mut app, pos2(300., 150.));
    plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
    assert_eq!(app.host.document.editor.project.elements.len(), 3);
    for model in [false, true] {
        app.host.document.editor.set_tool(Tool::Marquee);
        app.marquee_geometry = MarqueeGeometry::RotatedRectangle;
        app.marquee_points.clear();
        app.marquee_3d_points.clear();
        for p in [pos2(300., 350.), pos2(500., 350.)] {
            if model {
                click_model(&ctx, &mut app, p, PointerButton::Primary);
            } else {
                click(&ctx, &mut app, p);
            }
        }
        assert_eq!(app.marquee_points.len(), 2);
        let mut output = ctx.run_ui(
            RawInput {
                events: vec![key(Key::Backspace, Modifiers::NONE)],
                ..Default::default()
            },
            |ui| app.shortcuts(ui.ctx()),
        );
        output.textures_delta.clear();
        assert_eq!(app.marquee_points.len(), 1);
        if model {
            assert_eq!(app.marquee_3d_points.len(), 1);
        }
    }
}

/// Explicit operator-selected local files only; never a public build dependency.
#[test]
#[ignore = "requires OPENARCHI_BACKTRACK_CASES and OPENARCHI_BACKTRACK_OUTPUT"]
fn drawing_backtrack_local_project_roundtrips() {
    let paths: Vec<PathBuf> =
        serde_json::from_str(&std::env::var("OPENARCHI_BACKTRACK_CASES").unwrap()).unwrap();
    let output = PathBuf::from(std::env::var("OPENARCHI_BACKTRACK_OUTPUT").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    for (index, source) in paths.iter().enumerate() {
        let project = storage::load(source).unwrap();
        for tool in [Tool::Slab, Tool::Wall, Tool::Polyline] {
            let (ctx, mut app) = setup();
            app.project_open.choices.push_back(Some(source.clone()));
            layers_frame_output(&ctx, &mut app, vec![key(Key::O, Modifiers::COMMAND)]);
            layers_frame_output(&ctx, &mut app, vec![]);
            assert!(
                app.path_confirmed,
                "load with the normal file identity path"
            );
            app.plan_center = Point::new(1000., 1000.);
            // A saved active candidate intentionally forbids common-model
            // creation. Use the normal baseline service, preserving that guard.
            if let Some(task) = app.host.document.editor.project.design_process.task() {
                let baseline = task.baseline_option;
                app.activate_process_option(Some(baseline)).unwrap();
            }
            let baseline = app.host.document.editor.project.clone();
            backtrack_configure(&mut app, tool);
            for p in [pos2(300., 300.), pos2(500., 300.), pos2(500., 150.)] {
                click(&ctx, &mut app, p);
            }
            assert_eq!(backtrack_nodes(&app).len(), 3);
            backtrack_key(&ctx, &mut app);
            assert_eq!(backtrack_nodes(&app).len(), 2);
            click(&ctx, &mut app, pos2(300., 150.));
            if tool == Tool::Slab {
                click(&ctx, &mut app, pos2(300., 300.));
            } else {
                plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
            }
            assert!(
                backtrack_nodes(&app).is_empty(),
                "case {index} {tool:?}: {}",
                app.host.document.editor.status
            );
            let created = app.host.document.editor.project.clone();
            assert_eq!(
                created.elements.len(),
                project.elements.len() + if tool == Tool::Wall { 2 } else { 1 }
            );
            app.host.document.editor.undo();
            assert_eq!(app.host.document.editor.project, baseline);
            app.host.document.editor.redo();
            assert_eq!(app.host.document.editor.project, created);
            let path = output.join(format!("case-{index}-{tool:?}.openarchi.json"));
            storage::save(&created, &path).unwrap();
            let reopened = storage::load(&path).unwrap();
            let mut expected = serde_json::to_value(&created).unwrap();
            fn relocate(value: &mut serde_json::Value, old: &str, new: &str) {
                match value {
                    serde_json::Value::String(s) if s.starts_with(old) => {
                        *s = format!("{new}{}", &s[old.len()..]);
                    }
                    serde_json::Value::Array(a) => {
                        for v in a {
                            relocate(v, old, new);
                        }
                    }
                    serde_json::Value::Object(o) => {
                        for v in o.values_mut() {
                            relocate(v, old, new);
                        }
                    }
                    _ => {}
                }
            }
            // Save As relocates verified sidecars; compare every other field.
            relocate(
                &mut expected,
                &format!("{}.assets/", source.display()),
                &format!("{}.assets/", path.display()),
            );
            assert!(
                serde_json::to_value(&reopened).unwrap() == expected,
                "case {index} {tool:?}: reopened project differs beyond asset relocation"
            );
        }
    }
}
