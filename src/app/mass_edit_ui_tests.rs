mod mass_edit_completion {
    use super::super::wall_node_ui::WallPetCommand as C;
    use super::*;
    fn frame(ctx: &Context, app: &mut AtelierApp, model: bool, events: Vec<Event>) -> FullOutput {
        let events = discrete_key_events(ctx, events);
        let mut out = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
                events,
                ..Default::default()
            },
            |ui| {
                app.shortcuts(ui.ctx());
                CentralPanel::default().frame(Frame::NONE).show(ui, |ui| {
                    if model {
                        app.model_view(ui)
                    } else {
                        app.plan(ui)
                    }
                });
            },
        );
        out.textures_delta.clear();
        out
    }
    fn tap(ctx: &Context, app: &mut AtelierApp, model: bool, at: Pos2) {
        frame(ctx, app, model, vec![Event::PointerMoved(at)]);
        for pressed in [true, false] {
            frame(
                ctx,
                app,
                model,
                vec![
                    Event::PointerMoved(at),
                    Event::PointerButton {
                        pos: at,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
    }
    fn fixture(language: UiLanguage) -> (Context, AtelierApp, u64) {
        let (ctx, mut app) = setup();
        app.language = language;
        let mut editor = Editor::new(Project::default());
        let id = editor
            .place_concept_mass(crate::concept_mass::ConceptMassSpec {
                outline: vec![
                    Point::new(0., 0.),
                    Point::new(8., 0.),
                    Point::new(8., 6.),
                    Point::new(0., 6.),
                ],
                height: 6.,
                bottom_offset: 1.25,
                ..Default::default()
            })
            .unwrap();
        editor.set_tool(Tool::Select);
        editor.undo.clear();
        app.host.document.editor = editor;
        app.plan_center = Point::new(4., 3.);
        app.plan_scale = 40.;
        app.camera.target = glam::DVec3::new(4., 3., 3.);
        app.camera.span = 25.;
        (ctx, app, id)
    }
    fn pet(ctx: &Context, app: &mut AtelierApp, model: bool, c: C) {
        for _ in 0..3 {
            frame(ctx, app, model, vec![]);
        }
        let r = ctx
            .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", c as u8))))
            .unwrap_or_else(|| {
                panic!(
                    "missing {c:?} model={model} current={:?} status={}",
                    app.wall_insert.as_ref().map(|e| e.command),
                    app.host.document.editor.status
                )
            });
        tap(ctx, app, model, r.center());
        if c == C::InsertArcNode {
            assert!(
                app.wall_insert.is_none(),
                "{}",
                app.host.document.editor.status
            );
        } else {
            assert_eq!(app.wall_insert.as_ref().unwrap().command, c);
        }
    }
    #[test]
    fn mass_edit_2d_3d_curve_palette_preview_cancel_history_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            for model in [false, true] {
                let (ctx, mut app, _) = fixture(language);
                for _ in 0..3 {
                    frame(&ctx, &mut app, model, vec![]);
                }
                let r = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
                let screen = |app: &AtelierApp, p: Point| {
                    if model {
                        app.wall_project_point(r, p, 7.25).unwrap()
                    } else {
                        app.screen(r, p)
                    }
                };
                let edge = screen(&app, Point::new(4., 0.));
                let before = app.host.document.editor.project.clone();
                tap(&ctx, &mut app, model, edge);
                assert!(
                    app.wall_insert.is_some(),
                    "{model} {edge:?}: {}",
                    app.host.document.editor.status
                );
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
                    pet(&ctx, &mut app, model, command);
                    assert_eq!(app.host.document.editor.project, before);
                }
                pet(&ctx, &mut app, model, C::Curve);
                let at = screen(&app, Point::new(4., -2.));
                frame(&ctx, &mut app, model, vec![Event::PointerMoved(at)]);
                assert_eq!(app.host.document.editor.project, before);
                frame(
                    &ctx,
                    &mut app,
                    model,
                    vec![key(Key::Escape, Modifiers::NONE)],
                );
                assert!(app.wall_insert.is_none());
                assert_eq!(app.host.document.editor.project, before);
                tap(&ctx, &mut app, model, edge);
                pet(&ctx, &mut app, model, C::Curve);
                frame(&ctx, &mut app, model, vec![Event::PointerMoved(at)]);
                frame(
                    &ctx,
                    &mut app,
                    model,
                    vec![key(Key::Enter, Modifiers::NONE)],
                );
                assert!(
                    app.wall_insert.is_none(),
                    "{}",
                    app.host.document.editor.status
                );
                let after = app.host.document.editor.project.clone();
                assert_ne!(after, before);
                assert_eq!(app.host.document.editor.undo.len(), 1);
                let Shape::ConceptMass { spec } = &after.elements[0].shape else {
                    panic!()
                };
                assert!(spec.arcs[0].is_some());
                assert!((spec.arcs[0].unwrap().radius - 5.).abs() < 1e-4);
                app.host.document.editor.undo();
                assert_eq!(app.host.document.editor.project, before);
                app.host.document.editor.redo();
                assert_eq!(app.host.document.editor.project, after);
                // The curved segment itself is pickable and exposes exact-arc insertion.
                for _ in 0..3 {
                    frame(&ctx, &mut app, model, vec![]);
                }
                tap(&ctx, &mut app, model, at);
                if app.wall_insert.is_none() {
                    tap(&ctx, &mut app, model, at);
                }
                pet(&ctx, &mut app, model, C::InsertArcNode);
                assert_ne!(app.host.document.editor.project, after);
                app.host.document.editor.undo();
                frame(
                    &ctx,
                    &mut app,
                    model,
                    vec![key(Key::Escape, Modifiers::NONE)],
                );
                assert_eq!(app.host.document.editor.project, after);
                let bytes =
                    crate::model::ModelAdapter::export(&storage::JsonAdapter, &after).unwrap();
                assert_eq!(
                    crate::model::ModelAdapter::import(&storage::JsonAdapter, &bytes).unwrap(),
                    after
                );
            }
        }
    }
    fn hover_mass(ctx: &Context, app: &mut AtelierApp) {
        let r = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
        let e = &app.host.document.editor.project.elements[0];
        let Shape::ConceptMass { spec } = &e.shape else {
            panic!()
        };
        let p = app
            .wall_project_point(r, Point::new(4., 3.), spec.height + spec.bottom_offset)
            .unwrap();
        frame(ctx, app, true, vec![Event::PointerMoved(p)]);
        for _ in 0..3 {
            frame(ctx, app, true, vec![]);
        }
    }
    fn value(ctx: &Context, app: &mut AtelierApp, label: &str, text: &str) {
        if app.mass_parameters.is_none() {
            hover_mass(ctx, app);
        }
        for _ in 0..3 {
            frame(ctx, app, true, vec![]);
        }
        let r = ctx
            .data(|d| d.get_temp::<Rect>(Id::new(("mass_model_value", label))))
            .unwrap();
        tap(ctx, app, true, r.center());
        frame(
            ctx,
            app,
            true,
            vec![key(Key::A, Modifiers::COMMAND), Event::Text(text.into())],
        );
        frame(ctx, app, true, vec![]);
    }
    fn action(ctx: &Context, app: &mut AtelierApp, name: &str) {
        let r = ctx.data(|d| d.get_temp::<Rect>(Id::new(name))).unwrap();
        tap(ctx, app, true, r.center());
    }
    #[test]
    fn mass_edit_3d_height_offset_click_apply_cancel_invalid_locked_and_undo_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app, id) = fixture(language);
            for _ in 0..3 {
                frame(&ctx, &mut app, true, vec![]);
            }
            let before = app.host.document.editor.project.clone();
            let camera = app.camera.matrix(4. / 3.);
            value(&ctx, &mut app, "Height", "9.25");
            assert_eq!(app.host.document.editor.project, before);
            action(&ctx, &mut app, "mass_model_cancel");
            assert_eq!(app.host.document.editor.project, before);
            assert_eq!(app.camera.matrix(4. / 3.), camera);
            assert_eq!(app.host.document.editor.selection_ids(), vec![id]);
            value(&ctx, &mut app, "Height", "-1");
            action(&ctx, &mut app, "mass_model_apply");
            assert_eq!(app.host.document.editor.project, before);
            assert!(app.host.document.editor.undo.is_empty());
            assert!(app.mass_parameters_preview().is_none());

            value(&ctx, &mut app, "Height", "9.25");
            value(&ctx, &mut app, "Bottom offset", "2.5");
            action(&ctx, &mut app, "mass_model_apply");
            let after = app.host.document.editor.project.clone();
            let Shape::ConceptMass { spec } = &after.elements[0].shape else {
                panic!()
            };
            assert_eq!(spec.height, 9.25);
            assert_eq!(spec.bottom_offset, 2.5);
            assert_eq!(app.host.document.editor.undo.len(), 1);
            assert_eq!(app.host.document.editor.selection_ids(), vec![id]);
            app.host.document.editor.undo();
            assert_eq!(app.host.document.editor.project, before);
            app.host.document.editor.redo();
            assert_eq!(app.host.document.editor.project, after);
            app.host.document.editor.set_selection(vec![id]);
            app.host
                .document
                .editor
                .project
                .edit_state
                .locked
                .insert(id);
            let locked = app.host.document.editor.project.clone();
            hover_mass(&ctx, &mut app);
            assert!(app.mass_parameters.is_none());
            assert_eq!(app.host.document.editor.project, locked);
        }
    }
    #[test]
    fn mass_edit_option_canvas_pickup_then_create_and_model_node_drag_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app, id) = fixture(language);
            let r = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
            frame(&ctx, &mut app, false, vec![]);
            let source = app.screen(r, Point::new(4., 3.));
            let before = app.host.document.editor.project.clone();
            modifier_click(&ctx, &mut app, source, Modifiers::ALT);
            assert_eq!(app.host.document.editor.tool, Tool::ConceptMass);
            assert_eq!(app.host.document.editor.concept_mass_height, 6.);
            assert_eq!(app.host.document.editor.concept_mass_bottom_offset, 1.25);
            assert_eq!(app.host.document.editor.project, before);
            frame(
                &ctx,
                &mut app,
                false,
                vec![Event::ModifiersChanged(Modifiers::NONE)],
            );
            for q in [Point::new(10., 0.), Point::new(12., 3.)] {
                let at = app.screen(r, q);
                tap(&ctx, &mut app, false, at);
            }
            assert_eq!(app.host.document.editor.project.elements.len(), 2);
            let Shape::ConceptMass { spec } = &app.host.document.editor.project.elements[1].shape
            else {
                panic!()
            };
            assert_eq!(spec.height, 6.);
            assert_eq!(spec.bottom_offset, 1.25);
            app.host.document.editor.set_tool(Tool::Select);
            app.host.document.editor.set_selection(vec![id]);
            let before = app.host.document.editor.project.clone();
            let h = app.host.document.editor.undo.len();
            for _ in 0..3 {
                frame(&ctx, &mut app, true, vec![]);
            }
            let a = app.wall_project_point(r, Point::new(0., 0.), 7.25).unwrap();
            let b = app
                .wall_project_point(r, Point::new(-1., -1.), 7.25)
                .unwrap();
            frame(
                &ctx,
                &mut app,
                true,
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
            frame(&ctx, &mut app, true, vec![Event::PointerMoved(b)]);
            assert!(app.wall_insert.is_some());
            assert_eq!(app.host.document.editor.project, before);
            frame(
                &ctx,
                &mut app,
                true,
                vec![Event::PointerButton {
                    pos: b,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                }],
            );
            let after = app.host.document.editor.project.clone();
            assert_eq!(app.host.document.editor.undo.len(), h + 1);
            let Shape::ConceptMass { spec } = &after.elements[0].shape else {
                panic!()
            };
            assert!(spec.outline[0].distance(Point::new(-1., -1.)) < 1e-5);
            app.host.document.editor.undo();
            assert_eq!(app.host.document.editor.project, before);
            app.host.document.editor.redo();
            assert_eq!(app.host.document.editor.project, after);
        }
    }
    fn split_panes(app: &AtelierApp) -> [Rect; 2] {
        let r = app.workbench.panel_rects["view.canvas"];
        if app.view != View::Split {
            return [r, r];
        }
        let x = r.left() + r.width() * app.split as f32;
        [
            Rect::from_min_max(r.min, pos2(x - 3., r.bottom())),
            Rect::from_min_max(pos2(x + 3., r.top()), r.max),
        ]
    }
    #[test]
    fn mass_pet_viewport_owner_mouse_keyboard_numeric_cancel_history_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            for model in [false, true] {
                for split in [false, true] {
                    for keyboard in [false, true] {
                        let (ctx, mut app, id) = fixture(language);
                        app.view = if split {
                            View::Split
                        } else if model {
                            View::Model
                        } else {
                            View::Plan
                        };
                        // Exercise both the manual and collaborative workspaces.
                        app.ai_workspace.enabled = keyboard;
                        for _ in 0..4 {
                            layers_frame_output(&ctx, &mut app, vec![]);
                        }
                        let panes = split_panes(&app);
                        let r = panes[usize::from(model)];
                        let project = |a: &AtelierApp, p: Point| {
                            if model {
                                a.wall_project_point(r, p, 7.25).unwrap()
                            } else {
                                a.screen(r, p)
                            }
                        };
                        let edge = project(&app, Point::new(4., 0.));
                        let before = app.host.document.editor.project.clone();
                        layers_click(&ctx, &mut app, edge);
                        assert!(
                            app.wall_insert.is_some(),
                            "split {model} edge={edge:?}: {}",
                            app.host.document.editor.status
                        );
                        for _ in 0..3 {
                            layers_frame_output(&ctx, &mut app, vec![]);
                        }
                        let button = ctx
                            .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", C::Curve as u8))))
                            .unwrap();
                        layers_click(&ctx, &mut app, button.center());
                        assert_eq!(
                            app.wall_insert.as_ref().unwrap().command,
                            C::Curve,
                            "split {model}: shared icon must activate"
                        );
                        let target = project(&app, Point::new(4., -2.));
                        layers_frame_output(&ctx, &mut app, vec![Event::PointerMoved(target)]);
                        assert_eq!(app.host.document.editor.project, before);
                        // Neither drawing nor hovering the other pane may interpret this session.
                        if split {
                            let other = panes[usize::from(!model)].center();
                            layers_click(&ctx, &mut app, other);
                            assert!(app.wall_insert.is_some());
                            assert_eq!(app.host.document.editor.project, before);
                        }
                        layers_frame_output(&ctx, &mut app, vec![Event::PointerMoved(target)]);
                        if keyboard {
                            layers_frame_output(
                                &ctx,
                                &mut app,
                                vec![key(Key::Enter, Modifiers::NONE)],
                            );
                        } else {
                            layers_click(&ctx, &mut app, target);
                        }
                        assert!(
                            app.wall_insert.is_none(),
                            "split {model}: {}",
                            app.host.document.editor.status
                        );
                        assert_eq!(app.host.document.editor.undo.len(), 1);
                        let after = app.host.document.editor.project.clone();
                        let Shape::ConceptMass { spec } = &after.elements[0].shape else {
                            panic!()
                        };
                        assert!(spec.arcs[0].is_some());
                        assert!(
                            (spec.arcs[0].unwrap().radius - 5.).abs() < 1e-3,
                            "split {model}: {:?}",
                            spec.arcs
                        );
                        app.host.document.editor.undo();
                        assert_eq!(app.host.document.editor.project, before);
                        app.host.document.editor.redo();
                        assert_eq!(app.host.document.editor.project, after);
                        app.host.document.editor.set_selection(vec![id]);
                        // The same numeric tracker belongs to the initiating pane only.
                        for _ in 0..3 {
                            layers_frame_output(&ctx, &mut app, vec![]);
                        }
                        let edge = project(&app, Point::new(8., 3.));
                        layers_click(&ctx, &mut app, edge);
                        for _ in 0..3 {
                            layers_frame_output(&ctx, &mut app, vec![]);
                        }
                        let button = ctx
                            .data(|d| {
                                d.get_temp::<Rect>(Id::new(("wall_pet", C::OffsetEdge as u8)))
                            })
                            .unwrap();
                        layers_click(&ctx, &mut app, button.center());
                        let field = ctx
                            .data(|d| d.get_temp::<Rect>(Id::new("wall_pet_distance")))
                            .unwrap();
                        layers_click(&ctx, &mut app, field.center());
                        layers_frame_output(
                            &ctx,
                            &mut app,
                            vec![key(Key::A, Modifiers::COMMAND), Event::Text("1.25".into())],
                        );
                        layers_frame_output(
                            &ctx,
                            &mut app,
                            vec![key(
                                if keyboard { Key::Enter } else { Key::Escape },
                                Modifiers::NONE,
                            )],
                        );
                        if keyboard {
                            // 1.25 m extends this edge beyond the adjoining arc's
                            // valid intersection. Rejection must preserve the session
                            // and document, then allow correction without restarting.
                            assert!(app.wall_insert.is_some());
                            assert_eq!(app.host.document.editor.project, after);
                            let field = ctx
                                .data(|d| d.get_temp::<Rect>(Id::new("wall_pet_distance")))
                                .unwrap();
                            layers_click(&ctx, &mut app, field.center());
                            layers_frame_output(
                                &ctx,
                                &mut app,
                                vec![key(Key::A, Modifiers::COMMAND), Event::Text("0.25".into())],
                            );
                            layers_frame_output(
                                &ctx,
                                &mut app,
                                vec![key(Key::Enter, Modifiers::NONE)],
                            );
                        }
                        assert!(
                            app.wall_insert.is_none(),
                            "model={model} split={split} keyboard={keyboard}: {}",
                            app.host.document.editor.status
                        );
                        if keyboard {
                            let numeric = app.host.document.editor.project.clone();
                            assert_ne!(numeric, after);
                            assert_eq!(app.host.document.editor.undo.len(), 2);
                            app.host.document.editor.undo();
                            assert_eq!(app.host.document.editor.project, after);
                            app.host.document.editor.redo();
                            assert_eq!(app.host.document.editor.project, numeric);
                        } else {
                            assert_eq!(app.host.document.editor.project, after);
                        }
                    }
                }
            }
        }
    }

    fn push_setup(language: UiLanguage) -> (Context, AtelierApp, u64, Rect, Pos2) {
        let (ctx, mut app, id) = fixture(language);
        app.view = View::Model;
        app.host
            .document
            .editor
            .project
            .plan_options
            .dimension_style = crate::view_settings::DimensionStyle::PlainMeter;
        for _ in 0..3 {
            frame(&ctx, &mut app, true, vec![]);
        }
        let r = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
        let top = app.wall_project_point(r, Point::new(4., 3.), 7.25).unwrap();
        frame(&ctx, &mut app, true, vec![Event::PointerMoved(top)]);
        assert!(!app.mass_push_pull.enabled);
        (ctx, app, id, r, top)
    }
    #[test]
    fn mass_push_pull_ui_click_move_numeric_cancel_invalid_cursor_and_history_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app, _, r, top) = push_setup(language);
            let before = app.host.document.editor.project.clone();
            let out = frame(&ctx, &mut app, true, vec![Event::PointerMoved(top)]);
            assert_eq!(out.platform_output.cursor_icon, CursorIcon::None);
            tap(&ctx, &mut app, true, top);
            assert!(app.mass_push_pull_active());
            let delta = app.wall_project_point(r, Point::new(4., 3.), 8.25).unwrap() - top;
            frame(&ctx, &mut app, true, vec![Event::PointerMoved(top + delta)]);
            assert_eq!(app.host.document.editor.project, before);
            assert!(app.mass_push_pull_preview().is_some());
            frame(&ctx, &mut app, true, vec![Event::Text("-100".into())]);
            frame(&ctx, &mut app, true, vec![push_key(Key::Enter)]);
            assert!(app.mass_push_pull_active());
            assert_eq!(app.host.document.editor.project, before);
            frame(&ctx, &mut app, true, vec![push_key(Key::Escape)]);
            assert!(!app.mass_push_pull_active());
            assert_eq!(app.host.document.editor.project, before);
            assert!(app.host.document.editor.undo.is_empty());
            app.mass_push_pull.enabled = true;
            tap(&ctx, &mut app, true, top);
            frame(
                &ctx,
                &mut app,
                true,
                vec![Event::Text("1.25".into()), push_key(Key::Enter)],
            );
            assert!(!app.mass_push_pull_active());
            assert_eq!(app.host.document.editor.undo.len(), 1);
            let Shape::ConceptMass { spec } = &app.host.document.editor.project.elements[0].shape
            else {
                panic!()
            };
            assert_eq!(spec.height, 7.25);
            app.host.document.editor.undo();
            assert_eq!(app.host.document.editor.project, before);
            app.host.document.editor.redo();
            let out = frame(
                &ctx,
                &mut app,
                true,
                vec![Event::PointerMoved(pos2(20., 550.))],
            );
            assert_ne!(out.platform_output.cursor_icon, CursorIcon::None);
        }
    }
    #[test]
    fn mass_push_pull_ui_drag_side_release_and_conflict_protection() {
        let (ctx, mut app, _, r, _) = push_setup(UiLanguage::English);
        let at = app.wall_project_point(r, Point::new(4., 0.), 4.).unwrap();
        let end = app.wall_project_point(r, Point::new(4., -1.), 4.).unwrap();
        let before = app.host.document.editor.project.clone();
        frame(
            &ctx,
            &mut app,
            true,
            vec![
                Event::PointerMoved(at),
                Event::PointerButton {
                    pos: at,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        frame(&ctx, &mut app, true, vec![Event::PointerMoved(end)]);
        assert!(app.mass_push_pull_active());
        assert_eq!(app.host.document.editor.project, before);
        frame(
            &ctx,
            &mut app,
            true,
            vec![Event::PointerButton {
                pos: end,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert!(!app.mass_push_pull_active());
        assert_eq!(app.host.document.editor.undo.len(), 1);
        let Shape::ConceptMass { spec } = &app.host.document.editor.project.elements[0].shape
        else {
            panic!()
        };
        assert!((spec.area() - 56.).abs() < 0.2, "area {}", spec.area());
        app.host.document.editor.undo();
        tap(&ctx, &mut app, true, at);
        assert!(app.mass_push_pull_active());
        app.path = "different-project".into();
        frame(&ctx, &mut app, true, vec![]);
        assert!(!app.mass_push_pull_active());
        assert_eq!(app.host.document.editor.project, before);
    }
    fn push_key(key: Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn mass_push_pull_split_full_workspace_mouse_confirm_and_save_guard() {
        for ai in [false, true] {
            let (ctx, mut app, _) = fixture(UiLanguage::SimplifiedChinese);
            app.view = View::Split;
            app.ai_workspace.enabled = ai;
            for _ in 0..4 {
                layers_frame_output(&ctx, &mut app, vec![]);
            }
            let r = split_panes(&app)[1];
            let top = app.wall_project_point(r, Point::new(4., 3.), 7.25).unwrap();
            let end = app.wall_project_point(r, Point::new(4., 3.), 8.25).unwrap();
            let before = app.host.document.editor.project.clone();
            layers_click(&ctx, &mut app, top);
            assert!(app.mass_push_pull_active());
            assert!(
                app.save_project_to("/tmp/oa-must-not-save-draft.json")
                    .is_err()
            );
            layers_frame_output(&ctx, &mut app, vec![Event::PointerMoved(end)]);
            assert_eq!(app.host.document.editor.project, before);
            layers_click(&ctx, &mut app, end);
            assert!(!app.mass_push_pull_active());
            assert_eq!(app.host.document.editor.undo.len(), 1);
            let Shape::ConceptMass { spec } = &app.host.document.editor.project.elements[0].shape
            else {
                panic!()
            };
            assert!((spec.height - 7.).abs() < 1e-4);
            app.host.document.editor.undo();
            assert_eq!(app.host.document.editor.project, before);
        }
    }

    #[test]
    fn mass_push_pull_tracker_hover_preserves_mouse_preview_then_apply() {
        let (ctx, mut app, _, r, top) = push_setup(UiLanguage::English);
        tap(&ctx, &mut app, true, top);
        // An Area's first sizing pass is invisible and non-interactive.
        for _ in 0..3 {
            frame(&ctx, &mut app, true, vec![]);
        }
        let end = app.wall_project_point(r, Point::new(4., 3.), 8.25).unwrap();
        frame(&ctx, &mut app, true, vec![Event::PointerMoved(end)]);
        let preview = app.mass_push_pull_preview().unwrap();
        let apply = ctx
            .data(|d| d.get_temp::<Rect>(Id::new("mass_push_pull_apply")))
            .unwrap();
        frame(
            &ctx,
            &mut app,
            true,
            vec![Event::PointerMoved(apply.center())],
        );
        assert_eq!(app.mass_push_pull_preview().unwrap(), preview);
        tap(&ctx, &mut app, true, apply.center());
        assert!(!app.mass_push_pull_active());
        assert_eq!(app.host.document.editor.project, preview);
        assert_eq!(app.host.document.editor.undo.len(), 1);
    }

    #[test]
    fn mass_push_pull_large_coordinates_stationary_side_is_valid_noop() {
        let (ctx, mut app, _, r, _) = push_setup(UiLanguage::English);
        let delta = Point::new(54600., 50400.);
        if let Shape::ConceptMass { spec } = &mut app.host.document.editor.project.elements[0].shape
        {
            spec.transform(|p| Point::new(p.x + delta.x, p.y + delta.y));
        }
        app.camera.target += glam::DVec3::new(delta.x, delta.y, 0.);
        let before = app.host.document.editor.project.clone();
        let side = app
            .wall_project_point(r, Point::new(delta.x + 4., delta.y), 4.)
            .unwrap();
        tap(&ctx, &mut app, true, side);
        frame(&ctx, &mut app, true, vec![]);
        assert_eq!(app.mass_push_pull_preview().unwrap(), before);
        tap(&ctx, &mut app, true, side);
        assert!(!app.mass_push_pull_active());
        assert_eq!(app.host.document.editor.project, before);
        assert!(app.host.document.editor.undo.is_empty());
    }

    #[test]
    fn mass_hover_popup_reaches_height_input_previews_and_leaves_without_activation() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app, _) = fixture(language);
            app.view = View::Model;
            let before = app.host.document.editor.project.clone();
            frame(&ctx, &mut app, true, vec![]);
            assert!(app.mass_parameters.is_none());
            hover_mass(&ctx, &mut app);
            let panel = ctx
                .data(|d| d.get_temp::<Rect>(Id::new("mass_model_popup")))
                .unwrap();
            let r = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
            let top = app.wall_project_point(r, Point::new(4., 3.), 7.25).unwrap();
            assert!(panel.min.distance(top) < 100.);
            assert!(!app.mass_push_pull_active());
            assert!(app.host.document.editor.undo.is_empty());
            frame(
                &ctx,
                &mut app,
                true,
                vec![Event::PointerMoved(panel.center())],
            );
            assert!(app.mass_parameters.is_some());
            value(&ctx, &mut app, "Height", "9.25");
            let preview = app.mass_parameters_preview().unwrap();
            let Shape::ConceptMass { spec } = &preview.elements[0].shape else {
                panic!()
            };
            assert_eq!(spec.height, 9.25);
            assert_eq!(app.host.document.editor.project, before);
            action(&ctx, &mut app, "mass_model_cancel");
            assert!(app.mass_parameters_preview().is_none());
            frame(
                &ctx,
                &mut app,
                true,
                vec![Event::PointerMoved(pos2(15., 550.))],
            );
            frame(&ctx, &mut app, true, vec![]);
            assert!(app.mass_parameters.is_none());
            assert_eq!(app.host.document.editor.project, before);
            assert!(app.host.document.editor.undo.is_empty());
        }
    }
    #[test]
    fn mass_hover_direct_face_distance_text_focus_and_local_confirmation_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app, _, _, top) = push_setup(language);
            tap(&ctx, &mut app, true, top);
            for _ in 0..3 {
                frame(&ctx, &mut app, true, vec![]);
            }
            assert!(app.mass_push_pull_active());
            let field = ctx
                .data(|d| d.get_temp::<Rect>(Id::new("mass_push_pull_distance")))
                .unwrap();
            assert!(field.center().distance(top) < 250.);
            tap(&ctx, &mut app, true, field.center());
            frame(
                &ctx,
                &mut app,
                true,
                vec![key(Key::A, Modifiers::COMMAND), Event::Text("2.5".into())],
            );
            frame(&ctx, &mut app, true, vec![push_key(Key::Enter)]);
            assert!(!app.mass_push_pull_active());
            let Shape::ConceptMass { spec } = &app.host.document.editor.project.elements[0].shape
            else {
                panic!()
            };
            assert_eq!(spec.height, 8.5);
            assert_eq!(app.host.document.editor.undo.len(), 1);
        }
    }
}
