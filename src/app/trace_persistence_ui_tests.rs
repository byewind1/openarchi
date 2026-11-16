mod trace_persistence {
    use super::*;
    use crate::model::ModelAdapter;
    use crate::{
        trace::{TraceReference, TraceState},
        trace_document::{TraceSettings, TraceView},
    };

    fn root() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "oa-trace-persistence-{}",
            storage::new_document_identity()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
    fn button(ctx: &Context, app: &mut AtelierApp, label: &str) {
        for _ in 0..3 {
            layers_frame_output(ctx, app, vec![]);
        }
        let out = layers_frame_output(ctx, app, vec![]);
        let at = text_center(&out.shapes, app.language.translate(label)).expect(label);
        layers_click(ctx, app, at);
    }
    fn reopen(project: Project) -> AtelierApp {
        let mut app = AtelierApp::from_renderer(None, "Trace reopen".into());
        app.host.document.editor = Editor::new(project);
        app.reset_worksheet_session();
        app
    }

    #[test]
    fn trace_persistence_menu_palette_dirty_save_reopen_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            app.language = language;
            app.in_window_menus = true;
            let root = root();
            let geometry = app.host.document.editor.project.elements.clone();
            let history = app.host.document.editor.undo.len();
            button(&ctx, &mut app, "View");
            button(&ctx, &mut app, "Trace Options");
            button(&ctx, &mut app, "Choose Reference");
            button(&ctx, &mut app, "Above Current Story");
            assert_eq!(app.trace.reference, TraceReference::AboveCurrentStory);
            assert!(app.host.document.editor.dirty());
            app.trace.palette_open = true;
            button(&ctx, &mut app, "Reference on Top");
            button(&ctx, &mut app, "Transparent Fills and Zones");
            app.trace.offset = Point::new(17.25, -9.125);
            app.trace.rotation_deg = 37.;
            app.trace.reference_opacity = 71;
            app.trace.active_opacity = 203;
            button(&ctx, &mut app, "Active original colors");
            app.trace.reference_color = Some([17, 93, 181]);
            assert!(app.trace.active_color.is_some());
            app.trace.visibility.walls = false;
            let expected = TraceSettings::capture(&app.trace);
            app.save_project_to(root.join("saved.json").to_str().unwrap())
                .unwrap();
            assert!(!app.host.document.editor.dirty());
            let loaded = storage::load(&root.join("saved.json")).unwrap();
            assert_eq!(loaded.trace.settings(TraceView::FloorPlan), Some(&expected));
            let fresh = reopen(loaded);
            assert_eq!(TraceSettings::capture(&fresh.trace), expected);
            assert_eq!(fresh.host.document.editor.project.elements, geometry);
            assert_eq!(app.host.document.editor.undo.len(), history);
            assert!(!fresh.trace.palette_open && !fresh.trace.pointer_mode_active());
            assert!(!fresh.host.document.editor.dirty());
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn trace_persistence_view_context_and_project_isolation() {
        let (_ctx, mut app) = setup();
        let root = root();
        app.host
            .document
            .editor
            .project
            .drawings
            .worksheets
            .push(crate::drawings::Worksheet {
                id: 1,
                name: "Sheet".into(),
            });
        app.choose_trace_reference(TraceReference::AboveCurrentStory);
        app.trace.offset = Point::new(7., 8.);
        app.trace.rotation_deg = 19.;
        app.persist_trace_state(false);
        let plan = TraceSettings::capture(&app.trace);
        app.enter_worksheet(1);
        app.choose_trace_reference(TraceReference::Story(2));
        app.trace.reference_opacity = 28;
        app.trace.offset = Point::new(-1., 5.);
        app.persist_trace_state(false);
        let sheet = TraceSettings::capture(&app.trace);
        app.save_project_to(root.join("a.json").to_str().unwrap())
            .unwrap();
        let a = storage::load(&root.join("a.json")).unwrap();
        assert_eq!(a.trace.settings(TraceView::FloorPlan), Some(&plan));
        assert_eq!(a.trace.settings(TraceView::Worksheet(1)), Some(&sheet));
        let mut fresh = reopen(a.clone());
        assert_eq!(fresh.trace_view, TraceView::Worksheet(1));
        assert_eq!(TraceSettings::capture(&fresh.trace), sheet);
        fresh.host.document.editor.switch_story(1);
        fresh.sync_trace_context();
        assert_eq!(TraceSettings::capture(&fresh.trace), plan);
        fresh.host.document.editor = Editor::new(Project::default());
        fresh.reset_worksheet_session();
        assert_eq!(fresh.trace, TraceState::default());
        assert!(!fresh.host.document.editor.dirty());
        fresh.host.document.editor = Editor::new(a);
        fresh.reset_worksheet_session();
        assert_eq!(TraceSettings::capture(&fresh.trace), sheet);
        fresh.turn_off_trace_everywhere();
        assert!(
            fresh
                .host
                .document
                .editor
                .project
                .trace
                .windows
                .iter()
                .all(|w| !w.settings.enabled)
        );
        fresh.host.document.editor.switch_story(0);
        fresh.sync_trace_context();
        assert!(!fresh.trace.enabled);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn trace_persistence_transients_invalid_data_and_missing_source() {
        let (_ctx, mut app) = setup();
        let root = root();
        app.choose_trace_reference(TraceReference::Story(1));
        app.trace.offset = Point::new(3., 4.);
        app.trace.rotation_deg = 13.;
        app.persist_trace_state(false);
        let committed = TraceSettings::capture(&app.trace);
        app.toggle_trace_displace();
        app.trace.offset = Point::new(111., 222.);
        assert_eq!(
            TraceSettings::capture(&app.trace),
            committed,
            "temporary placement is protected before the first point too"
        );
        app.save_project_to(root.join("before-first-point.json").to_str().unwrap())
            .unwrap();
        assert_eq!(
            storage::load(&root.join("before-first-point.json"))
                .unwrap()
                .trace
                .settings(TraceView::FloorPlan),
            Some(&committed)
        );
        app.trace.cancel_pointer_modes();
        app.toggle_trace_rotate();
        app.trace.rotation_deg = 89.;
        assert_eq!(TraceSettings::capture(&app.trace), committed);
        app.trace.cancel_pointer_modes();

        app.trace.gesture_origin = Some((app.trace.offset, app.trace.rotation_deg));
        app.trace.offset = Point::new(999., 888.);
        app.trace.rotation_deg = 73.;
        app.trace.drag_mode = true;
        app.trace.browse_open = true;
        app.trace.browse_choice = TraceReference::Story(2);
        app.save_project_to(root.join("gesture.json").to_str().unwrap())
            .unwrap();
        let fresh = reopen(storage::load(&root.join("gesture.json")).unwrap());
        assert_eq!(TraceSettings::capture(&fresh.trace), committed);
        app.trace.cancel_pointer_modes();
        assert_eq!(TraceSettings::capture(&app.trace), committed);
        app.trace.displace_mode = true;
        app.trace.displace_origin = Some(app.trace.offset);
        app.trace.offset = Point::new(999., 888.);
        app.save_project_to(root.join("temporary.json").to_str().unwrap())
            .unwrap();
        assert_eq!(
            TraceSettings::capture(
                &reopen(storage::load(&root.join("temporary.json")).unwrap()).trace
            ),
            committed
        );
        app.trace.cancel_pointer_modes();
        let good = app.host.document.editor.project.clone();
        let mut bad = good.clone();
        bad.trace.windows[0].settings.rotation_deg = f64::NAN;
        assert!(storage::save(&bad, &root.join("invalid.json")).is_err());
        let mut dangling = good;
        dangling.trace.windows[0].settings.reference = TraceReference::Worksheet(999);
        let fresh = reopen(dangling);
        assert_eq!(fresh.trace.reference, TraceReference::Worksheet(999));
        assert!(!fresh.trace.enabled);
        assert!(!fresh.host.document.editor.dirty());
        let old = Project::default();
        let bytes = storage::JsonAdapter.export(&old).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("\"trace\""));
        assert_eq!(
            reopen(storage::JsonAdapter.import(&bytes).unwrap()).trace,
            TraceState::default()
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn trace_persistence_recovery_save_as_and_model_history_are_independent() {
        let (_ctx, mut app) = setup();
        let root = root();
        app.choose_trace_reference(TraceReference::Story(1));
        app.trace.offset = Point::new(2., 7.);
        app.trace.rotation_deg = -22.;
        app.persist_trace_state(false);
        let settings = TraceSettings::capture(&app.trace);
        let before = app.host.document.editor.project.clone();
        app.host.document.editor.project.name = "Geometry history".into();
        app.host.document.editor.commit(before);
        app.host.document.editor.undo();
        assert_eq!(
            app.host
                .document
                .editor
                .project
                .trace
                .settings(TraceView::FloorPlan),
            Some(&settings)
        );
        app.host.document.editor.redo();
        assert_eq!(
            app.host
                .document
                .editor
                .project
                .trace
                .settings(TraceView::FloorPlan),
            Some(&settings)
        );
        let slot = crate::recovery::RecoverySlot::create(&root.join("recoveries")).unwrap();
        slot.write_document(
            &app.host.project_snapshot_for_save().collect().unwrap(),
            app.host.document.editor.project_identity(),
            None,
        )
        .unwrap();
        drop(slot);
        let mut recovered = AtelierApp::from_renderer(None, "Recovery".into());
        recovered.initialize_recovery(&root);
        recovered.recovery_choice = Some(0);
        recovered.resolve_recovery(true).unwrap();
        assert_eq!(TraceSettings::capture(&recovered.trace), settings);
        assert!(recovered.host.document.editor.dirty());
        recovered
            .save_project_to(root.join("one.json").to_str().unwrap())
            .unwrap();
        recovered
            .save_project_to(root.join("two.json").to_str().unwrap())
            .unwrap();
        assert_eq!(
            storage::load(&root.join("one.json")).unwrap().trace,
            storage::load(&root.join("two.json")).unwrap().trace
        );
        assert!(!recovered.host.document.editor.dirty());
        drop(recovered);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn trace_click_move_drag_rotate_displace_commit_cancel_and_save() {
        let (ctx, mut app) = setup();
        app.choose_trace_reference(TraceReference::Story(1));
        app.trace.offset = Point::new(2., 3.);
        app.trace.rotation_deg = 17.;
        app.persist_trace_state(false);
        app.host.document.editor.mark_saved();
        let history = app.host.document.editor.undo.len();
        app.reset_trace_position();
        assert!(
            app.trace.pivot.is_some(),
            "reset keeps a stable source basis for subsequent rotation"
        );
        app.trace.offset = Point::new(2., 3.);
        app.trace.rotation_deg = 17.;
        app.persist_trace_state(false);
        app.host.document.editor.mark_saved();
        let initial = TraceSettings::capture(&app.trace);
        let root = root();
        app.toggle_trace_drag();
        click(&ctx, &mut app, pos2(400., 300.));
        assert!(app.trace.pointer_anchor.is_some());
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(450., 320.))]);
        assert!(app.trace.offset.distance(initial.offset) > 0.1);
        assert_eq!(
            TraceSettings::capture(&app.trace),
            initial,
            "moving preview is not committed"
        );
        app.save_project_to(root.join("during-drag.json").to_str().unwrap())
            .unwrap();
        assert_eq!(
            storage::load(&root.join("during-drag.json"))
                .unwrap()
                .trace
                .settings(TraceView::FloorPlan),
            Some(&initial)
        );
        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        assert_eq!(TraceSettings::capture(&app.trace), initial);
        assert!(!app.host.document.editor.dirty());
        app.toggle_trace_drag();
        click(&ctx, &mut app, pos2(400., 300.));
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(450., 320.))]);
        click(&ctx, &mut app, pos2(450., 320.));
        assert!(!app.trace.pointer_mode_active());
        assert!(app.host.document.editor.dirty());
        let moved = TraceSettings::capture(&app.trace);
        app.host.document.editor.mark_saved();
        app.toggle_trace_displace();
        click(&ctx, &mut app, pos2(400., 300.));
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(500., 350.))]);
        assert!(app.trace.displace_mode);
        assert!(app.trace.offset.distance(moved.offset) > 0.1);
        assert_eq!(TraceSettings::capture(&app.trace), moved);
        click(&ctx, &mut app, pos2(500., 350.));
        assert_eq!(TraceSettings::capture(&app.trace), moved);
        assert!(!app.trace.pointer_mode_active());
        assert!(!app.host.document.editor.dirty());
        let before_rotation = app.trace.clone();
        app.toggle_trace_rotate();
        click(&ctx, &mut app, pos2(400., 300.));
        let center = app.trace.pointer_anchor.unwrap();
        let fixed_source = before_rotation.inverse_transform(center, app.trace_current_pivot());
        click(&ctx, &mut app, pos2(500., 300.));
        assert!(app.trace.rotation_arm.is_some());
        plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(400., 200.))]);
        assert!((app.trace.rotation_deg - before_rotation.rotation_deg - 90.).abs() < 1e-7);
        assert!(
            app.trace
                .transform(fixed_source, app.trace_current_pivot())
                .distance(center)
                < 1e-7,
            "chosen rotation center stays fixed"
        );
        click(&ctx, &mut app, pos2(400., 200.));
        assert!(!app.trace.pointer_mode_active());
        assert_eq!(app.host.document.editor.undo.len(), history);
        app.plan_center = Point::new(19.25, -37.5);
        app.plan_scale = 237.5;
        app.save_project_to(root.join("confirmed.json").to_str().unwrap())
            .unwrap();
        let mut fresh = reopen(storage::load(&root.join("confirmed.json")).unwrap());
        fresh.fit_opened_project();
        assert_eq!(fresh.plan_center, app.plan_center);
        assert_eq!(fresh.plan_scale, app.plan_scale);
        assert_eq!(
            TraceSettings::capture(&fresh.trace),
            TraceSettings::capture(&app.trace)
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn trace_splitter_four_edges_are_temporary_and_do_not_dirty_project() {
        for edge in 0..4 {
            let ctx = Context::default();
            let mut app = AtelierApp::from_renderer(None, "Splitter".into());
            app.choose_trace_reference(TraceReference::Story(1));
            app.trace.splitter = Some(0.5);
            app.host.document.editor.mark_saved();
            let before = app.host.document.editor.project.clone();
            let rect = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
            let (handle, horizontal, from_end) = AtelierApp::trace_splitter_handles(rect)[edge];
            let start = handle.center();
            let end = pos2(360., 240.);
            let frame = |ctx: &Context, app: &mut AtelierApp, events| {
                let mut out = ctx.run_ui(
                    RawInput {
                        screen_rect: Some(rect),
                        events,
                        ..Default::default()
                    },
                    |ui| app.trace_splitter_input(ui, rect),
                );
                out.textures_delta.clear();
            };
            frame(&ctx, &mut app, vec![Event::PointerMoved(start)]);
            frame(
                &ctx,
                &mut app,
                vec![Event::PointerButton {
                    pos: start,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                }],
            );
            frame(&ctx, &mut app, vec![Event::PointerMoved(end)]);
            assert_eq!(app.trace.split_axis, Some(horizontal));
            assert_eq!(app.trace.split_from_end, from_end);
            let (reference, active) = app.trace_split_regions(rect).unwrap();
            assert!((reference.area() + active.area() - rect.area()).abs() < 1e-6);
            frame(
                &ctx,
                &mut app,
                vec![Event::PointerButton {
                    pos: end,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                }],
            );
            assert!(app.trace.split_axis.is_none());
            assert!(app.trace_split_regions(rect).is_none());
            assert_eq!(app.host.document.editor.project, before);
            assert!(!app.host.document.editor.dirty());
        }
    }

    #[test]
    fn trace_model_snap_uses_display_transform_and_does_not_select_reference() {
        let (_ctx, mut app) = setup();
        shortcut_test_wall(&mut app);
        app.host.document.editor.project.elements[0].story = 1;
        app.choose_trace_reference(TraceReference::Story(1));
        app.trace.offset = Point::new(17., -11.);
        app.trace.rotation_deg = 37.;
        let e = &app.host.document.editor.project.elements[0];
        let source = geometry::element_snap_points(&app.host.document.editor.project, e, 1)[0].0;
        let expected = app.trace.transform(source, app.trace_current_pivot());
        let near = Point::new(expected.x + 0.01, expected.y - 0.02);
        let (point, kind) = app.base_target(near, false);
        assert!(matches!(
            kind,
            SnapKind::Endpoint | SnapKind::SnapPoint | SnapKind::Midpoint
        ));
        assert!(point.distance(expected) < 1e-8);
        assert!(
            geometry::hit_plan(&app.host.document.editor.project, 0, expected, 0.1).is_none(),
            "reference remains read-only for normal selection"
        );
        app.trace.visibility.walls = false;
        assert!(app.trace_model_snap(near, 0.1).is_none());
        app.trace.visibility.walls = true;
        app.trace.reference_opacity = 0;
        assert!(app.trace_model_snap(near, 0.1).is_none());
    }

    #[test]
    fn trace_active_color_only_tints_active_shapes_and_preserves_alpha() {
        let (ctx, mut app) = setup();
        app.trace.enabled = true;
        app.trace.active_color = Some([19, 79, 173]);
        let project = app.host.document.editor.project.clone();
        let mut out = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
                ..Default::default()
            },
            |ui| {
                let p = ui.painter();
                p.line_segment(
                    [pos2(0., 0.), pos2(10., 10.)],
                    Stroke::new(1., Color32::RED),
                );
                let start = p.add(egui::Shape::Noop);
                p.line_segment(
                    [pos2(20., 20.), pos2(30., 30.)],
                    Stroke::new(1., Color32::from_rgba_unmultiplied(200, 30, 40, 137)),
                );
                app.tint_trace_active(p, start);
                p.line_segment(
                    [pos2(40., 40.), pos2(50., 50.)],
                    Stroke::new(1., Color32::GREEN),
                );
            },
        );
        out.textures_delta.clear();
        let lines = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::LineSegment { stroke, .. } => Some(stroke.color),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            lines,
            vec![
                Color32::RED,
                Color32::from_rgba_unmultiplied(19, 79, 173, 137),
                Color32::GREEN
            ]
        );
        assert_eq!(app.host.document.editor.project, project);
    }
    #[test]
    fn trace_numeric_text_draft_saves_only_after_confirmation() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let ctx = Context::default();
            let mut app = AtelierApp::from_renderer(None, "Numeric draft".into());
            app.language = language;
            app.choose_trace_reference(TraceReference::Story(1));
            app.host.document.editor.mark_saved();
            let frame = |ctx: &Context, app: &mut AtelierApp, events| {
                let events = discrete_key_events(ctx, events);
                let time = ctx.input(|i| i.time) + 1. / 60.;
                let mut out = ctx.run_ui(
                    RawInput {
                        time: Some(time),
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 900.))),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        app.shortcuts(ui.ctx());
                        app.trace_palette_contents(ui);
                    },
                );
                out.textures_delta.clear();
                out
            };
            for _ in 0..3 {
                frame(&ctx, &mut app, vec![]);
            }
            let out = frame(&ctx, &mut app, vec![]);
            let pos = text_center(&out.shapes, "0.0").expect("rotation field");
            let edit = |app: &mut AtelierApp| {
                frame(&ctx, app, vec![Event::PointerMoved(pos)]);
                for _ in 0..2 {
                    for pressed in [true, false] {
                        frame(
                            &ctx,
                            app,
                            vec![Event::PointerButton {
                                pos,
                                button: PointerButton::Primary,
                                pressed,
                                modifiers: Modifiers::NONE,
                            }],
                        );
                    }
                }
                frame(&ctx, app, vec![key(Key::A, Modifiers::COMMAND)]);
                frame(&ctx, app, vec![Event::Text("49".into())]);
            };
            edit(&mut app);
            let root = root();
            app.save_project_to(root.join("draft.json").to_str().unwrap())
                .unwrap();
            assert_eq!(
                storage::load(&root.join("draft.json"))
                    .unwrap()
                    .trace
                    .settings(TraceView::FloorPlan)
                    .unwrap()
                    .rotation_deg,
                0.
            );
            assert_eq!(app.trace.rotation_deg, 0.);
            assert!(!app.host.document.editor.dirty());
            frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
            // DragValue has a deferred lost-focus path; settling catches a draft
            // incorrectly committed one frame after Escape on the real desktop.
            for _ in 0..5 {
                frame(&ctx, &mut app, vec![]);
            }
            assert_eq!(app.trace.rotation_deg, 0.);
            assert!(!app.host.document.editor.dirty());
            edit(&mut app);
            frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
            assert_eq!(app.trace.rotation_deg, 49.);
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .trace
                    .settings(TraceView::FloorPlan)
                    .unwrap()
                    .rotation_deg,
                49.
            );
            assert!(app.host.document.editor.dirty());
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn trace_transparent_active_mass_and_zone_keep_edges_without_polygon_fills() {
        let (ctx, mut app) = setup();
        app.host.document.editor = Editor::new(Project::default());
        app.host
            .document
            .editor
            .place_concept_mass(crate::concept_mass::ConceptMassSpec {
                outline: vec![
                    Point::new(-5., -2.),
                    Point::new(-1., -2.),
                    Point::new(-1., 2.),
                    Point::new(-5., 2.),
                ],
                height: 3.,
                ..Default::default()
            })
            .unwrap();
        app.host
            .document
            .editor
            .place_zone(crate::zones::ZoneSpec {
                surface: Default::default(),
                purpose: "Study".into(),
                outline: vec![
                    Point::new(1., -2.),
                    Point::new(5., -2.),
                    Point::new(5., 2.),
                    Point::new(1., 2.),
                ],
                holes: vec![],
                height: 3.,
                bottom_offset: 0.,
                label_position: None,
            })
            .unwrap();
        app.host.document.editor.set_selection(vec![]);
        app.trace.enabled = true;
        app.trace.reference = TraceReference::Story(1);
        app.trace.active_color = Some([47, 103, 180]);
        app.plan_center = Point::default();
        app.plan_scale = 40.;
        let project = app.host.document.editor.project.clone();
        let history = app.host.document.editor.undo.len();
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
        let samples = [Point::new(-3., 1.), Point::new(2., 1.)].map(|p| app.screen(rect, p));
        fn covers(shape: &egui::Shape, p: Pos2) -> bool {
            let inside = |points: Vec<Pos2>| {
                geometry::contains(
                    Point::new(p.x.into(), p.y.into()),
                    &points
                        .iter()
                        .map(|p| Point::new(p.x.into(), p.y.into()))
                        .collect::<Vec<_>>(),
                )
            };
            match shape {
                egui::Shape::Vec(v) => v.iter().any(|s| covers(s, p)),
                egui::Shape::Mesh(m) => m.indices.chunks_exact(3).any(|t| {
                    t.iter().all(|i| m.vertices[*i as usize].color.a() > 0)
                        && inside(t.iter().map(|i| m.vertices[*i as usize].pos).collect())
                }),
                egui::Shape::Path(path) => {
                    path.closed && path.fill.a() > 0 && inside(path.points.clone())
                }
                _ => false,
            }
        }
        for transparent in [false, true] {
            app.trace.transparent_fills = transparent;
            let out = plan_frame_output(&ctx, &mut app, vec![]);
            for sample in samples {
                assert_eq!(
                    out.shapes.iter().any(|s| covers(&s.shape, sample)),
                    !transparent
                );
            }
            assert!(
                out.shapes
                    .iter()
                    .any(|s| matches!(s.shape, egui::Shape::Path(ref p) if p.stroke.width > 0.))
            );
        }
        assert_eq!(app.host.document.editor.project, project);
        assert_eq!(app.host.document.editor.undo.len(), history);
    }

    #[test]
    fn trace_status_messages_translate_without_changing_source_names() {
        let mut app = AtelierApp::from_renderer(None, "Trace labels".into());
        for (raw, chinese) in [
            ("Trace Reference · Sheet A", "描图参考 · Sheet A"),
            ("Trace & Reference palette", "描图与参考面板"),
            (
                "Activated Trace Reference · story 01",
                "已激活描图参考 · 楼层 01",
            ),
            (
                "Trace Reference reset to default position",
                "描图参考已恢复默认位置",
            ),
            (
                "Trace source unavailable · original.dxf",
                "描图来源不可用 · original.dxf",
            ),
        ] {
            app.host.document.editor.status = raw.into();
            app.language = UiLanguage::SimplifiedChinese;
            assert_eq!(app.status_text(), chinese);
            app.language = UiLanguage::English;
            assert_eq!(app.status_text(), raw);
        }
    }
    #[test]
    fn trace_reused_image_id_does_not_rebind_until_explicitly_chosen() {
        let (_ctx, mut app) = setup();
        let root = root();
        let a = root.join("a.png");
        let b = root.join("b.png");
        image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]))
            .save(&a)
            .unwrap();
        image::RgbaImage::from_pixel(4, 4, image::Rgba([8, 9, 10, 255]))
            .save(&b)
            .unwrap();
        let old = crate::drawings::import(&a, &root.join("cache"), 1).unwrap();
        let new = crate::drawings::import(&b, &root.join("cache"), 1).unwrap();
        app.host.document.editor.project.drawings.pages.push(old);
        app.choose_trace_reference(TraceReference::Worksheet(1));
        let accepted = app.trace.source_identity.clone();
        app.host.document.editor.project.drawings.pages.clear();
        assert!(
            app.trace
                .selection(&app.host.document.editor.project, 0, 0)
                .is_none()
        );
        app.host.document.editor.project.drawings.pages.push(new);
        assert!(
            app.trace
                .selection(&app.host.document.editor.project, 0, 0)
                .is_none()
        );
        app.save_project_to(root.join("reused.json").to_str().unwrap())
            .unwrap();
        let mut fresh = reopen(storage::load(&root.join("reused.json")).unwrap());
        assert!(!fresh.trace.enabled);
        assert_eq!(fresh.trace.source_identity, accepted);
        fresh.choose_trace_reference(TraceReference::Worksheet(1));
        assert!(fresh.trace.enabled);
        assert_ne!(fresh.trace.source_identity, accepted);
        assert!(
            fresh
                .trace
                .selection(&fresh.host.document.editor.project, 0, 0)
                .is_some()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn trace_open_palette_preserves_saved_off_switch_and_disabled_controls() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            app.language = language;
            app.view = View::Model;
            let before = app.host.document.editor.project.clone();
            app.open_trace_palette();
            assert!(!app.trace.enabled && app.view == View::Model);
            assert_eq!(app.host.document.editor.project, before);
            let color = app.trace.reference_color;
            button(&ctx, &mut app, "Original colors");
            assert_eq!(
                app.trace.reference_color, color,
                "disabled palette control does not change document"
            );
            button(&ctx, &mut app, "Show Reference");
            assert!(app.trace.enabled && app.view == View::Plan);
            button(&ctx, &mut app, "Show Reference");
            assert!(!app.trace.enabled);
            let root = root();
            app.save_project_to(root.join("off.json").to_str().unwrap())
                .unwrap();
            let mut fresh = reopen(storage::load(&root.join("off.json")).unwrap());
            fresh.open_trace_palette();
            assert!(!fresh.trace.enabled && !fresh.host.document.editor.dirty());
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
