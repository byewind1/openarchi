mod mass_polygon_closure {
    use super::*;

    fn tap(ctx: &Context, app: &mut AtelierApp, at: Pos2, button: PointerButton) {
        layers_frame_output(ctx, app, vec![Event::PointerMoved(at)]);
        for pressed in [true, false] {
            layers_frame_output(
                ctx,
                app,
                vec![
                    Event::PointerMoved(at),
                    Event::PointerButton {
                        pos: at,
                        button,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
    }

    #[test]
    fn mass_polygon_right_click_closes_only_accepted_nodes_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            for view in [View::Plan, View::Split] {
                let (ctx, mut app) = setup();
                app.language = language;
                app.view = view;
                app.host.document.editor = Editor::new(Project::default());
                app.host.document.editor.set_tool(Tool::ConceptMass);
                app.host.document.editor.zone_polygon = true;
                app.host.document.editor.snap = false;
                app.plan_center = Point::new(0., 0.);
                app.plan_scale = 30.;
                for _ in 0..4 {
                    layers_frame_output(&ctx, &mut app, vec![]);
                }
                let mut r = app.workbench.panel_rects["view.canvas"];
                if view == View::Split {
                    r.max.x = r.left() + r.width() * app.split as f32 - 3.;
                }
                let points = [
                    Point::new(-3., -2.),
                    Point::new(3., -2.),
                    Point::new(3., 2.),
                ];
                for point in points {
                    let at = app.screen(r, point);
                    tap(&ctx, &mut app, at, PointerButton::Primary);
                }
                assert_eq!(app.host.document.editor.polyline_points.len(), 3);
                let accepted = app.host.document.editor.polyline_points.clone();
                tap(&ctx, &mut app, r.center(), PointerButton::Secondary);
                assert_eq!(
                    app.host.document.editor.project.elements.len(),
                    1,
                    "{}",
                    app.host.document.editor.status
                );
                let Shape::ConceptMass { spec } =
                    &app.host.document.editor.project.elements[0].shape
                else {
                    panic!()
                };
                assert_eq!(spec.outline, accepted);
                assert!(app.host.document.editor.polyline_points.is_empty());
                assert_eq!(app.host.document.editor.undo.len(), 1);
                let created = app.host.document.editor.project.clone();
                app.host.document.editor.undo();
                assert!(app.host.document.editor.project.elements.is_empty());
                app.host.document.editor.redo();
                assert_eq!(app.host.document.editor.project, created);
            }
        }
    }

    #[test]
    fn mass_polygon_start_hit_wins_over_model_snap_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            app.language = language;
            app.host.document.editor = Editor::new(Project::default());
            // Another mass has a node at the pointer, closer than the draft start.
            app.host
                .document
                .editor
                .place_concept_mass(crate::concept_mass::ConceptMassSpec {
                    outline: vec![
                        Point::new(0.25, 0.),
                        Point::new(-2., 0.),
                        Point::new(-2., -2.),
                    ],
                    height: 3.,
                    ..Default::default()
                })
                .unwrap();
            app.host.document.editor.set_tool(Tool::ConceptMass);
            app.host.document.editor.zone_polygon = true;
            app.host.document.editor.snap = false;
            app.plan_center = Point::new(1., 1.);
            app.plan_scale = 30.;
            for _ in 0..4 {
                layers_frame_output(&ctx, &mut app, vec![]);
            }
            let r = app.workbench.panel_rects["view.canvas"];
            for point in [Point::new(0., 0.), Point::new(4., 0.), Point::new(4., 3.)] {
                let at = app.screen(r, point);
                tap(&ctx, &mut app, at, PointerButton::Primary);
            }
            app.host.document.editor.snap = true;
            let at = app.screen(r, Point::new(0.25, 0.));
            tap(&ctx, &mut app, at, PointerButton::Primary);
            assert_eq!(
                app.host.document.editor.project.elements.len(),
                2,
                "start marker must close despite a closer model node"
            );
            assert!(app.host.document.editor.polyline_points.is_empty());
        }
    }

    #[test]
    fn mass_polygon_invalid_finish_keeps_draft_and_escape_cancels_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            app.language = language;
            app.host.document.editor.set_tool(Tool::ConceptMass);
            app.host.document.editor.zone_polygon = true;
            app.host.document.editor.snap = false;
            app.plan_scale = 30.;
            for _ in 0..4 {
                layers_frame_output(&ctx, &mut app, vec![]);
            }
            let r = app.workbench.panel_rects["view.canvas"];
            for points in [
                vec![Point::new(-3., -2.), Point::new(3., -2.)],
                vec![
                    Point::new(-3., -2.),
                    Point::new(3., 2.),
                    Point::new(-3., 2.),
                    Point::new(3., -2.),
                ],
            ] {
                app.host.document.editor.set_tool(Tool::ConceptMass);
                for &point in &points {
                    let at = app.screen(r, point);
                    tap(&ctx, &mut app, at, PointerButton::Primary);
                }
                let draft = app.host.document.editor.polyline_points.clone();
                tap(&ctx, &mut app, r.center(), PointerButton::Secondary);
                assert_eq!(app.host.document.editor.polyline_points, draft);
                assert!(app.host.document.editor.project.elements.is_empty());
                assert!(app.host.document.editor.undo.is_empty());
                layers_frame_output(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
                assert!(app.host.document.editor.polyline_points.is_empty());
                assert!(app.host.document.editor.undo.is_empty());
            }
            // A rejected two-node finish can be continued normally.
            app.host.document.editor.set_tool(Tool::ConceptMass);
            for point in [Point::new(-3., -2.), Point::new(3., -2.)] {
                let at = app.screen(r, point);
                tap(&ctx, &mut app, at, PointerButton::Primary);
            }
            tap(&ctx, &mut app, r.center(), PointerButton::Secondary);
            let at = app.screen(r, Point::new(3., 2.));
            tap(&ctx, &mut app, at, PointerButton::Primary);
            tap(&ctx, &mut app, r.center(), PointerButton::Secondary);
            assert_eq!(app.host.document.editor.project.elements.len(), 1);
            let path = std::env::temp_dir().join(format!(
                "mass-close-{}.json",
                storage::new_document_identity()
            ));
            app.save_project_to(path.to_str().unwrap()).unwrap();
            let reopened = Editor::from_stored_document(&path).unwrap();
            assert_eq!(reopened.project, app.host.document.editor.project);
            std::fs::remove_file(path).unwrap();
        }
    }
}

mod mass_product_path {
    use super::*;
    use serde_json::json;
    fn button(ctx: &Context, app: &mut AtelierApp, key: &str) {
        reveal_manual_control(ctx, app, key);
        for _ in 0..8 {
            let frame = layers_frame_output(ctx, app, vec![]);
            if let Some(p) = frame.shapes.iter().find_map(|s| {
                text_center(std::slice::from_ref(s), app.language.translate(key))
                    .filter(|p| s.clip_rect.contains(*p))
            }) {
                layers_click(ctx, app, p);
                return;
            }
        }
        let frame = layers_frame_output(ctx, app, vec![]);
        let mut texts = vec![];
        collect_text(&frame.shapes, &mut texts);
        panic!("visible {key}: {texts:?}");
    }
    #[test]
    fn v4_mass_normal_canvas_create_cancel_numeric_edit_undo_save_reopen_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            let supplied =
                std::env::var_os("OPENARCHI_MASS_CASE_INPUT").map(std::path::PathBuf::from);
            let output =
                std::env::var_os("OPENARCHI_MASS_CASE_OUTPUT").map(std::path::PathBuf::from);
            let root = output
                .clone()
                .unwrap_or_else(|| {
                    std::env::temp_dir().join(format!(
                        "oa-mass-ui-{}",
                        &storage::new_document_identity()[..12]
                    ))
                })
                .join(if language == UiLanguage::English {
                    "en"
                } else {
                    "zh"
                });
            std::fs::create_dir_all(root.parent().unwrap()).unwrap();
            std::fs::create_dir(&root).expect(
                "keep original operation/output; inspect existing result, never unknown replay",
            );
            let mut trace = vec![];
            if let Some(input) = &supplied {
                app.host.document.editor = Editor::from_stored_document(input).unwrap();
            } else {
                app.host.document.editor = Editor::new(storage::startup_example());
            }
            let initial = app.host.document.editor.project.clone();
            std::fs::write(root.join("operation-plan.json"),serde_json::to_vec_pretty(&json!({"input":supplied,"output":root.join("continued.openarchi.json"),"task":"protected-copy manual concept mass; all dimensions/placement are design assumptions; no image interpretation","steps":["explicit main-model view if currently in candidate","native concept mass entry","preview then Escape cancellation","rectangle create","canvas selection and numeric height Apply","Undo/Redo","normal Save","independent reopen"],"original_elements":initial.elements,"operation_identity":root})).unwrap()).unwrap();
            // Use the existing inline candidate context, not direct mutation of task/option state.
            if let Some(option) = app
                .host
                .document
                .editor
                .project
                .plan_options
                .active_design_option
            {
                let name = app
                    .host
                    .document
                    .editor
                    .project
                    .design_options
                    .name(option)
                    .unwrap()
                    .to_owned();
                let mut at = None;
                for _ in 0..6 {
                    let frame = layers_frame_output(&ctx, &mut app, vec![]);
                    at = frame
                        .shapes
                        .iter()
                        .filter_map(|s| {
                            text_center(std::slice::from_ref(s), &name)
                                .filter(|p| s.clip_rect.contains(*p))
                        })
                        .min_by(|a, b| a.y.total_cmp(&b.y));
                    if at.is_some() {
                        break;
                    }
                }
                layers_click(
                    &ctx,
                    &mut app,
                    at.expect("existing inline candidate context"),
                );
                button(&ctx, &mut app, "Main Model Only");
                assert_eq!(
                    app.host
                        .document
                        .editor
                        .project
                        .plan_options
                        .active_design_option,
                    None
                );
                trace.push("explicit main-model context via existing inline candidate context");
            }
            app.plan_center = Point::new(32., 32.);
            app.plan_scale = 40.;
            app.host.document.editor.snap = false;
            button(&ctx, &mut app, "Concept mass");
            assert_eq!(app.host.document.editor.tool, Tool::ConceptMass);
            assert!(app.preview_tool.is_none());
            for _ in 0..3 {
                layers_frame_output(&ctx, &mut app, vec![]);
            }
            let view = app.workbench.panel_rects["view.canvas"];
            let a = app.screen(view, Point::new(30., 30.));
            let b = app.screen(view, Point::new(34., 33.));
            let before = app.host.document.editor.project.clone();
            let history = app.host.document.editor.undo.len();
            layers_click(&ctx, &mut app, a);
            assert!(app.host.document.editor.anchor.is_some());
            layers_frame_output(&ctx, &mut app, vec![Event::PointerMoved(b)]);
            assert_eq!(app.host.document.editor.project, before);
            layers_frame_output(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
            assert!(app.host.document.editor.anchor.is_none());
            assert_eq!(app.host.document.editor.project, before);
            assert_eq!(app.host.document.editor.undo.len(), history);
            trace.push("preview/Escape keeps original model/history");
            button(&ctx, &mut app, "Concept mass");
            for _ in 0..3 {
                layers_frame_output(&ctx, &mut app, vec![]);
            }
            let view = app.workbench.panel_rects["view.canvas"];
            let a = app.screen(view, Point::new(30., 30.));
            let b = app.screen(view, Point::new(34., 33.));
            layers_click(&ctx, &mut app, a);
            assert_eq!(
                app.workbench.panel_rects["view.canvas"], view,
                "first-point tracker must not move canvas"
            );
            layers_click(&ctx, &mut app, b);
            assert_eq!(
                app.host.document.editor.project.elements.len(),
                initial.elements.len() + 1,
                "{}",
                app.host.document.editor.status
            );
            let mass = app
                .host
                .document
                .editor
                .project
                .elements
                .last()
                .unwrap()
                .clone();
            assert!(matches!(mass.shape, Shape::ConceptMass { .. }));
            assert!(
                (mass.area() - 12.).abs() < 1e-7,
                "actual mass {:?}; canvas {:?}",
                mass,
                view
            );
            assert_eq!(mass.design_option, None);
            assert_eq!(
                app.host.document.editor.project.design_process,
                initial.design_process
            );
            for old in &initial.elements {
                assert_eq!(
                    app.host
                        .document
                        .editor
                        .project
                        .elements
                        .iter()
                        .find(|e| e.id == old.id),
                    Some(old)
                );
            }
            trace.push("two canvas clicks create one true editable prism without task brief or extra window");
            button(&ctx, &mut app, "Select");
            let view = app.workbench.panel_rects["view.canvas"];
            let point = app.screen(view, Point::new(32., 31.5));
            layers_click(&ctx, &mut app, point);
            assert_eq!(app.host.document.editor.selection_ids(), vec![mass.id]);
            let mut numeric = None;
            for _ in 0..25 {
                let frame = layers_frame_output(&ctx, &mut app, vec![]);
                let label = frame.shapes.iter().find_map(|s| {
                    text_center(std::slice::from_ref(s), language.translate("Height"))
                        .filter(|p| s.clip_rect.contains(*p) && p.x > 1000.)
                });
                if let Some(label) = label {
                    numeric = frame.shapes.iter().find_map(|s| {
                        let egui::Shape::Text(t) = &s.shape else {
                            return None;
                        };
                        let p = t.pos + t.galley.size() / 2.;
                        (s.clip_rect.contains(p)
                            && (p.y - label.y).abs() < 10.
                            && p.x > label.x
                            && t.galley.text().starts_with(|c: char| c.is_ascii_digit()))
                        .then_some(p)
                    });
                }
                if numeric.is_some() {
                    break;
                }
                layers_frame_output(
                    &ctx,
                    &mut app,
                    vec![
                        Event::PointerMoved(pos2(1230., 760.)),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            phase: egui::TouchPhase::Move,
                            delta: vec2(0., -24.),
                            modifiers: Modifiers::NONE,
                        },
                    ],
                );
            }
            let numeric = numeric.unwrap_or_else(|| {
                let f = layers_frame_output(&ctx, &mut app, vec![]);
                let mut t = vec![];
                collect_text(&f.shapes, &mut t);
                panic!("nearby inspector height: {t:?}")
            });
            layers_click(&ctx, &mut app, numeric);
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::A, Modifiers::COMMAND), Event::Text("4.5".into())],
            );
            layers_frame_output(&ctx, &mut app, vec![key(Key::Tab, Modifiers::NONE)]);
            assert!(
                matches!(&app.draft.as_ref().unwrap().shape,Shape::ConceptMass{spec} if (spec.height-4.5).abs()<1e-9)
            );
            button(&ctx, &mut app, "Apply changes");
            let edited = app.host.document.editor.project.clone();
            let volume = app
                .host
                .document
                .editor
                .geometry_source(mass.id)
                .unwrap()
                .source
                .concept_volume_m3()
                .unwrap();
            assert!((volume - 54.).abs() < 1e-7);
            trace.push("existing inspector actual numeric text and Apply update only mass height");
            button(&ctx, &mut app, "Undo");
            assert_eq!(
                app.host.document.editor.project.elements.last(),
                Some(&mass)
            );
            button(&ctx, &mut app, "Redo");
            assert_eq!(app.host.document.editor.project, edited);
            trace.push(
                "normal model Undo/Redo exact; transient selection clears by existing behavior",
            );
            let saved = root.join("continued.openarchi.json");
            app.path = saved.to_string_lossy().into_owned();
            app.path_confirmed = false;
            app.file_baseline = None;
            button(&ctx, &mut app, "Save project");
            assert!(app.file_action.is_some());
            button(&ctx, &mut app, "Save");
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let reopened = Editor::from_stored_document(&saved).unwrap();
            // Save As deliberately relocates verified sidecars; every other model/header/history
            // field still has to match exactly. Do not erase drawings from this assertion.
            let mut expected = edited.clone();
            for page in &mut expected.drawings.pages {
                let actual = reopened
                    .project
                    .drawings
                    .pages
                    .iter()
                    .find(|p| p.id == page.id)
                    .unwrap();
                assert_eq!(
                    crate::drawings::verified_bytes(page).unwrap(),
                    crate::drawings::verified_bytes(actual).unwrap()
                );
                let asset = std::path::absolute(&saved)
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join(format!(
                        "{}.assets",
                        saved.file_name().unwrap().to_string_lossy()
                    ))
                    .join(format!("{}.image", page.sha256));
                assert_eq!(actual.path, asset);
                page.path = asset;
            }
            if reopened.project != expected {
                std::fs::write(
                    root.join("roundtrip-before.json"),
                    serde_json::to_vec_pretty(&edited).unwrap(),
                )
                .unwrap();
                std::fs::write(
                    root.join("roundtrip-after.json"),
                    serde_json::to_vec_pretty(&reopened.project).unwrap(),
                )
                .unwrap();
            }
            assert!(
                reopened.project == expected,
                "Exact saved/reopened model with verified target asset bindings differs; preserved roundtrip-before/after.json under {}",
                root.display()
            );
            assert_eq!(
                reopened.project_identity(),
                app.host.document.editor.project_identity()
            );
            let report = json!({"language":format!("{language:?}"),"input":supplied,"output":saved,"initial_model":initial,"final_model":edited,"mass_id":mass.id,"volume_m3":volume,"trace":trace,"entry":"actual egui workspace pointer/text widgets; no native OS/GPU claim","menu_context_transfers":0,"inline_context_switches":if before.plan_options.active_design_option!=initial.plan_options.active_design_option{1}else{0},"repeated_input":0,"new_windows":0,"AI":"not_run; manual foundation","image_intent":"not_run; next W14 subwork","saved_reopen_equal":true});
            std::fs::write(
                root.join("walkthrough.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
            println!(
                "MASS_WALK_REPORT={}",
                root.join("walkthrough.json").display()
            );
            if output.is_none() {
                std::fs::remove_dir_all(root.parent().unwrap()).unwrap();
            }
        }
    }
}
