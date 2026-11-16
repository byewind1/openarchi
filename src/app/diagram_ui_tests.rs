mod linked_figure_product_path {
    use super::*;
    use crate::command::ModelCommand;
    use crate::diagrams::Status;
    pub(super) fn button(ctx: &Context, app: &mut AtelierApp, key: &str) {
        reveal_manual_control(ctx, app, key);
        for _ in 0..if key == "Apply changes" { 40 } else { 5 } {
            let frame = layers_frame_output(ctx, app, vec![]);
            if let Some(p) = frame.shapes.iter().find_map(|s| {
                text_center(std::slice::from_ref(s), app.language.translate(key))
                    .filter(|p| s.clip_rect.contains(*p))
            }) {
                layers_click(ctx, app, p);
                return;
            }
            if key == "Apply changes" {
                layers_frame_output(
                    ctx,
                    app,
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
        }
        panic!("visible control {key}: {}", app.process_ui.error);
    }
    fn text(ctx: &Context, app: &mut AtelierApp, old: &str, value: &str) {
        let frame = layers_frame_output(ctx, app, vec![]);
        let at =
            text_center(&frame.shapes, old).unwrap_or_else(|| panic!("visible text field {old}"));
        layers_click(ctx, app, at);
        layers_frame_output(
            ctx,
            app,
            vec![key(Key::A, Modifiers::COMMAND), Event::Text(value.into())],
        );
    }
    pub(super) fn height(ctx: &Context, app: &mut AtelierApp, value: f64) {
        // Selection is delivered at frame end; inspect the newly rendered draft,
        // rather than guessing its object-specific label from the prior frame.
        layers_frame_output(ctx, app, vec![]);
        let height_label = if app
            .draft
            .as_ref()
            .is_some_and(|e| matches!(e.shape, Shape::Zone { .. }))
        {
            "Clear height"
        } else {
            "Height"
        };
        let mut numeric = None;
        for _ in 0..25 {
            let f = layers_frame_output(ctx, app, vec![]);
            if let Some(label) = f.shapes.iter().find_map(|s| {
                text_center(
                    std::slice::from_ref(s),
                    app.language.translate(height_label),
                )
                .filter(|p| s.clip_rect.contains(*p) && p.x > 1000.)
            }) {
                numeric = f.shapes.iter().find_map(|s| {
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
                ctx,
                app,
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
        layers_click(ctx, app, numeric.expect("visible selected object's height"));
        layers_frame_output(
            ctx,
            app,
            vec![
                key(Key::A, Modifiers::COMMAND),
                Event::Text(value.to_string()),
            ],
        );
        layers_frame_output(ctx, app, vec![key(Key::Tab, Modifiers::NONE)]);
        button(ctx, app, "Apply changes");
    }
    #[test]
    fn normal_task_button_preview_edit_stale_refresh_undo_export_save_reopen() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let input = std::env::var_os("OPENARCHI_FIGURE_INPUT").map(PathBuf::from);
            if input.is_some() && language == UiLanguage::SimplifiedChinese {
                continue;
            }
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            let root = std::env::var_os("OPENARCHI_FIGURE_OUTPUT")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::temp_dir()
                        .join(format!("oa-figure-ui-{}", storage::new_document_identity()))
                });
            std::fs::create_dir(&root)
                .expect("existing result is not permission to replay writes; inspect original run");
            app.host.document.editor = if let Some(input) = &input {
                Editor::from_stored_document(input).unwrap()
            } else {
                Editor::new(storage::synthetic_building_fixture())
            };
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            if input.is_none() {
                let sources = app
                    .host
                    .document
                    .editor
                    .project
                    .elements
                    .iter()
                    .filter(|e| e.story == 0 && matches!(e.shape, Shape::Zone { .. }))
                    .map(|e| e.id)
                    .collect();
                let prepared = crate::design_process::service::prepare_create(
                    &app.host.document.editor,
                    &storage::new_document_identity(),
                    crate::design_process::service::Draft {
                        name: "Figure task".into(),
                        question: "Compare saved authored spaces without a complete survey".into(),
                        statements: vec![],
                        candidate_names: ["A".into(), "B".into()],
                        source_ids: sources,
                        entry: crate::design_process::Entry::ManualInterface,
                    },
                )
                .unwrap();
                app.host
                    .document
                    .editor
                    .commit_change("fixture-task-start", &prepared)
                    .unwrap();
            }
            let original = app.host.document.editor.project.clone();
            let task = original.design_process.task().unwrap();
            let selected = task.candidates[0].element_ids()[0];
            let element = original.elements.iter().find(|e| e.id == selected).unwrap();
            let (old_height, at) = match &element.shape {
                Shape::Zone { spec } => (spec.height, spec.label_point()),
                Shape::ConceptMass { spec } => (
                    spec.height,
                    Point::new(
                        spec.outline.iter().map(|p| p.x).sum::<f64>() / spec.outline.len() as f64,
                        spec.outline.iter().map(|p| p.y).sum::<f64>() / spec.outline.len() as f64,
                    ),
                ),
                _ => unreachable!(),
            };
            std::fs::write(root.join("operation-plan.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"original_model_sha256":crate::automation_journal::canonical_hash(&original).unwrap(),"output":root.join("continued.openarchi.json"),"manual_test_only":true,"source_edit_id":selected,"old_height":old_height,"test_height":old_height+0.25,"entry":"normal task strip/figure canvas/inspector/file text controls; fixture start only is loaded","steps":["select source","preview/cancel/create","read-only hidden selection","back to model/local height","stale/export refusal","preview/cancel/refresh","undo/redo","SVG create-new path","save/reopen"],"native_desktop":"not_run"})).unwrap()).unwrap();
            app.fit();
            app.plan_center = at;
            app.plan_scale = 35.;
            layers_frame_output(&ctx, &mut app, vec![]);
            let rect = app.workbench.panel_rects["view.canvas"];
            let point = app.screen(rect, at);
            assert!(rect.contains(point));
            layers_click(&ctx, &mut app, point);
            assert_eq!(app.host.document.editor.selection_ids(), vec![selected]);
            let selection = app.host.document.editor.selection_ids();
            button(&ctx, &mut app, "Preview linked figure");
            assert!(app.process_ui.preview.is_some());
            assert!(
                app.host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .is_empty()
            );
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, original);
            button(&ctx, &mut app, "Preview linked figure");
            button(&ctx, &mut app, "Confirm design change");
            assert!(app.process_ui.preview.is_none(), "{}", app.process_ui.error);
            let figure = app.host.document.editor.project.drawings.diagrams[0].clone();
            assert!(app.view == View::Figure(figure.id));
            assert_eq!(app.host.document.editor.project.elements, original.elements);
            assert_eq!(app.host.document.editor.selection_ids(), selection);

            let frame = layers_frame_output(&ctx, &mut app, vec![]);
            let footer = frame
                .shapes
                .iter()
                .find_map(|s| {
                    let egui::Shape::Text(t) = &s.shape else {
                        return None;
                    };
                    t.galley
                        .text()
                        .contains(&figure.source_sha256[..16])
                        .then_some((s.clip_rect, t))
                })
                .expect("actual figure source/north footer");
            assert!(
                footer.0.contains(footer.1.pos)
                    && footer.0.contains(footer.1.pos + footer.1.galley.size()),
                "entire source/north line must remain visible at fit, not below scroll clipping"
            );
            let after = app.host.document.editor.project.clone();
            assert!(!app.edit_command_enabled("Delete"));
            app.dispatch_edit("Delete");
            assert_eq!(app.host.document.editor.project, after);

            layers_frame_output(&ctx, &mut app, vec![key(Key::Delete, Modifiers::NONE)]);
            assert_eq!(
                app.host.document.editor.project, after,
                "hidden selected model must not be deleted from the read-only figure"
            );
            button(&ctx, &mut app, "Edit source model");
            assert!(app.view == View::Plan);
            assert_eq!(app.host.document.editor.selection_ids(), selection);
            height(&ctx, &mut app, old_height + 0.25);
            let changed = app.host.document.editor.project.clone();
            assert_ne!(changed.elements, original.elements);
            assert_eq!(changed.elements.len(), original.elements.len());
            assert!(
                original
                    .elements
                    .iter()
                    .filter(|e| e.id != selected)
                    .all(|e| changed.elements.contains(e))
            );
            assert_eq!(changed.design_process, original.design_process);
            button(&ctx, &mut app, "Show linked figure");
            assert_eq!(figure.status(&changed), Status::Stale);
            assert!(figure.svg(&changed).is_err());
            button(&ctx, &mut app, "Preview figure update");
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, changed);
            button(&ctx, &mut app, "Preview figure update");
            button(&ctx, &mut app, "Confirm design change");
            let current = app.host.document.editor.project.clone();
            let updated = current.drawings.diagrams[0].clone();
            assert_eq!(updated.generation, 2);
            assert_eq!(updated.id, figure.id);
            assert_eq!(updated.status(&current), Status::Current);
            assert_eq!(current.elements, changed.elements);
            button(&ctx, &mut app, "Undo");
            assert_eq!(
                app.host.document.editor.project.drawings.diagrams[0],
                figure
            );
            button(&ctx, &mut app, "Redo");
            assert_eq!(app.host.document.editor.project, current);
            layers_frame_output(&ctx, &mut app, vec![]);
            let old_path = app.drawings_ui.figure_export.clone();
            let svg = root.join("figure.svg");
            text(&ctx, &mut app, &old_path, &svg.to_string_lossy());
            button(&ctx, &mut app, "Export current SVG");
            assert!(svg.is_file(), "{}", app.drawings_ui.error);
            let exported = std::fs::read(&svg).unwrap();
            button(&ctx, &mut app, "Export current SVG");
            assert_eq!(
                std::fs::read(&svg).unwrap(),
                exported,
                "existing destination cannot be overwritten"
            );
            let save = root.join("continued.openarchi.json");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let old_path = app.path.clone();
            text(&ctx, &mut app, &old_path, &save.to_string_lossy());
            button(&ctx, &mut app, "Save");
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let reopened = Editor::from_stored_document(&save).unwrap();
            // Saving relocates verified original images into the new file's sidecar.
            // Assert all other fields exactly, and verify BOTH old and rebound bytes.
            let mut rebound = current.clone();
            assert_eq!(
                reopened.project.drawings.pages.len(),
                rebound.drawings.pages.len()
            );
            for (expected, actual) in rebound
                .drawings
                .pages
                .iter_mut()
                .zip(&reopened.project.drawings.pages)
            {
                assert_eq!(
                    crate::drawings::verified_bytes(expected).unwrap(),
                    crate::drawings::verified_bytes(actual).unwrap()
                );
                expected.path = actual.path.clone();
            }
            assert_eq!(reopened.project, rebound);
            assert_eq!(
                reopened.project.drawings.diagrams[0].status(&reopened.project),
                Status::Current
            );
            for page in &reopened.project.drawings.pages {
                crate::drawings::verified_bytes(page).unwrap();
            }
            std::fs::write(root.join("walkthrough.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"output":save,"svg":svg,"language":format!("{language:?}"),"figure_id":updated.id,"generation":updated.generation,"original_source_sha256":figure.source_sha256,"updated_source_sha256":updated.source_sha256,"expected_record_sha256":crate::diagrams::expected(&updated),"input_elements":original.elements.len(),"output_elements":current.elements.len(),"manual_test_changed_only":selected,"task_choices_unchanged":true,"selection_retained":true,"preview_cancel_create_stale_refresh_undo_redo_export_save_reopen":"pass","native_desktop":"not_run","package":"older than this source","actual_AI":"not_run; manual product-path regression"})).unwrap()).unwrap();
            println!(
                "FIGURE_PRODUCT_WALK={}",
                root.join("walkthrough.json").display()
            );
            drop(app.automation.take());
            if input.is_none() {
                std::fs::remove_dir_all(root).unwrap();
            }
        }
    }

    fn edit_purpose(ctx: &Context, app: &mut AtelierApp, old: &str, new: &str) {
        let mut at = None;
        for _ in 0..25 {
            let frame = layers_frame_output(ctx, app, vec![]);
            at = frame.shapes.iter().find_map(|s| {
                let egui::Shape::Text(t) = &s.shape else {
                    return None;
                };
                let rect = Rect::from_min_size(t.pos, t.galley.size());
                (t.galley.text() == old
                    && rect.center().x > 1000.
                    && s.clip_rect.contains_rect(rect))
                .then_some(rect.center())
            });
            if at.is_some() {
                break;
            }
            layers_frame_output(
                ctx,
                app,
                vec![
                    Event::PointerMoved(pos2(1230., 760.)),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        delta: vec2(0., -32.),
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        layers_click(ctx, app, at.expect("visible source purpose field"));
        layers_frame_output(
            ctx,
            app,
            vec![key(Key::A, Modifiers::COMMAND), Event::Text(new.into())],
        );
        layers_frame_output(ctx, app, vec![key(Key::Tab, Modifiers::NONE)]);
        button(ctx, app, "Apply changes");
    }
    fn choose_candidate(ctx: &Context, app: &mut AtelierApp, option: u32) {
        let current = app
            .host
            .document
            .editor
            .project
            .plan_options
            .active_design_option;
        if current == Some(option) {
            return;
        }
        let current_name = current
            .and_then(|id| app.host.document.editor.project.design_options.name(id))
            .unwrap_or("Main model")
            .to_owned();
        let target = app
            .host
            .document
            .editor
            .project
            .design_options
            .name(option)
            .unwrap()
            .to_owned();
        button(ctx, app, &current_name);
        button(ctx, app, &target);
        assert_eq!(
            app.host
                .document
                .editor
                .project
                .plan_options
                .active_design_option,
            Some(option)
        );
    }
    #[test]
    fn function_diagram_real_task_canvas_unknown_purpose_change_switch_and_restore() {
        use crate::diagrams::Presentation;
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let input = std::env::var_os("OPENARCHI_FUNCTION_INPUT").map(PathBuf::from);
            if input.is_some() && language == UiLanguage::SimplifiedChinese {
                continue;
            }
            let (ctx, mut app) = setup();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            let root = std::env::var_os("OPENARCHI_FUNCTION_OUTPUT")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::temp_dir().join(format!(
                        "oa-function-ui-{}",
                        storage::new_document_identity()
                    ))
                });
            std::fs::create_dir(&root)
                .expect("original outcomes must be inspected; never replay an unknown write");
            app.host.document.editor = if let Some(input) = &input {
                Editor::from_stored_document(input).unwrap()
            } else {
                Editor::new(storage::synthetic_building_fixture())
            };
            if input.is_none() {
                let sources = app
                    .host
                    .document
                    .editor
                    .project
                    .elements
                    .iter()
                    .filter(|e| e.story == 0 && matches!(e.shape, Shape::Zone { .. }))
                    .map(|e| e.id)
                    .collect();
                let c=crate::design_process::service::prepare_create(&app.host.document.editor,&storage::new_document_identity(),crate::design_process::service::Draft{name:"Functional design task".into(),question:"Express current declared purposes without requiring an approved use classification".into(),statements:vec![],candidate_names:["A spaces".into(),"B spaces".into()],source_ids:sources,entry:crate::design_process::Entry::ManualInterface}).unwrap();
                app.host
                    .document
                    .editor
                    .commit_change("function-fixture", &c)
                    .unwrap();
            }
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            let initial = app.host.document.editor.project.clone();
            let zone_task = initial
                .design_process
                .tasks
                .iter()
                .find(|t| !t.is_mass())
                .unwrap()
                .clone();
            std::fs::write(root.join("operation-plan.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"input_model_sha256":crate::automation_journal::canonical_hash(&initial).unwrap(),"output":root.join("continued.openarchi.json"),"task":zone_task.id,"entry":"normal workspace task/candidate controls, source picking/property text, shared proposal/SVG/Save","steps":["resume existing Zone task","function preview cancel/create","unknown annotation cancel/configure","source purpose change/stale/refresh","UndoRedo","candidate switch and local edit/Undo","new SVG/Save/reopen"],"manual_fixture_not_AI":true})).unwrap()).unwrap();
            if initial.design_process.current_task.as_deref() != Some(zone_task.id.as_str()) {
                let name = initial.design_process.task().unwrap().name.clone();
                button(&ctx, &mut app, &name);
                button(&ctx, &mut app, &zone_task.name);
                button(&ctx, &mut app, "Confirm design change");
            }
            choose_candidate(&ctx, &mut app, zone_task.candidates[0].option);
            let original = app.host.document.editor.project.clone();
            assert_eq!(original.elements, initial.elements);
            assert_eq!(original.drawings.diagrams, initial.drawings.diagrams);
            let selected = zone_task.candidates[0].element_ids()[0];
            let source = original.elements.iter().find(|e| e.id == selected).unwrap();
            let Shape::Zone { spec } = &source.shape else {
                unreachable!()
            };
            let old_purpose = spec.purpose.clone();
            let target = spec.label_point();
            app.fit();
            app.plan_center = target;
            app.plan_scale = 35.;
            layers_frame_output(&ctx, &mut app, vec![]);
            let point = app.screen(app.workbench.panel_rects["view.canvas"], target);
            layers_click(&ctx, &mut app, point);
            assert_eq!(app.host.document.editor.selection_ids(), vec![selected]);
            button(&ctx, &mut app, "Preview function diagram");
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, original);
            button(&ctx, &mut app, "Preview function diagram");
            button(&ctx, &mut app, "Confirm design change");
            let created = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .last()
                .unwrap()
                .clone();
            assert!(matches!(
                created.definition.presentation,
                Presentation::ZoneFunctions { .. }
            ));
            assert_eq!(created.snapshot.elements.len(), 9);
            assert_eq!(
                created.status(&app.host.document.editor.project),
                Status::Current
            );
            let frame = layers_frame_output(&ctx, &mut app, vec![]);
            let mut texts = vec![];
            collect_text(&frame.shapes, &mut texts);
            assert!(texts.iter().any(|s| s
                == app.language.translate(
                    "Linked figure · metric model scale bar · paper scale unspecified"
                )));
            assert!(
                !texts.iter().any(|s| s == "1:100"),
                "plan paper scale is not this figure's export scale"
            );
            let unknown_purpose = created.function_groups()[0].purpose.clone();
            button(&ctx, &mut app, "Declared purpose groups");
            button(&ctx, &mut app, "Unknown purpose");
            button(&ctx, &mut app, "Preview purpose annotations");
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .last()
                    .unwrap(),
                &created
            );
            button(&ctx, &mut app, "Preview purpose annotations");
            button(&ctx, &mut app, "Confirm design change");
            let annotated = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .last()
                .unwrap()
                .clone();
            assert_eq!(annotated.generation, 2);
            assert!(
                annotated
                    .function_groups()
                    .iter()
                    .any(|g| g.purpose == unknown_purpose && g.unknown)
            );
            assert_eq!(app.host.document.editor.project.elements, original.elements);
            button(&ctx, &mut app, "Edit source model");
            assert_eq!(app.host.document.editor.selection_ids(), vec![selected]);
            edit_purpose(&ctx, &mut app, &old_purpose, "flexible study (concept)");
            let changed = app.host.document.editor.project.clone();
            assert!(
                original
                    .elements
                    .iter()
                    .filter(|e| e.id != selected)
                    .all(|e| changed.elements.contains(e))
            );
            assert_ne!(changed.elements, original.elements);
            button(&ctx, &mut app, "Show function diagram");
            assert_eq!(annotated.status(&changed), Status::Stale);
            assert!(annotated.svg(&changed).is_err());
            button(&ctx, &mut app, "Preview figure update");
            button(&ctx, &mut app, "Cancel design proposal");
            button(&ctx, &mut app, "Preview figure update");
            button(&ctx, &mut app, "Confirm design change");
            let updated = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .last()
                .unwrap()
                .clone();
            assert_eq!(updated.id, created.id);
            assert_eq!(updated.generation, 3);
            assert_eq!(updated.total(), created.total());
            assert!(
                updated
                    .function_groups()
                    .iter()
                    .any(|g| g.purpose == "flexible study (concept)"
                        && g.element_ids.contains(&selected))
            );
            button(&ctx, &mut app, "Undo");
            button(&ctx, &mut app, "Undo");
            assert_eq!(app.host.document.editor.project.elements, original.elements);
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .last()
                    .unwrap()
                    .status(&app.host.document.editor.project),
                Status::Current
            );
            // After Undo changed the generation, a clean settings draft must bind
            // to the actual restored record before the user starts a fresh annotation.
            let frame = layers_frame_output(&ctx, &mut app, vec![]);
            let visible = frame.shapes.iter().any(|s| {
                text_center(
                    std::slice::from_ref(s),
                    app.language.translate("Unknown purpose"),
                )
                .is_some_and(|p| s.clip_rect.contains(p))
            });
            if !visible {
                button(&ctx, &mut app, "Declared purpose groups");
            }
            button(&ctx, &mut app, "Unknown purpose");
            button(&ctx, &mut app, "Preview purpose annotations");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            button(&ctx, &mut app, "Cancel design proposal");
            button(&ctx, &mut app, "Redo");
            button(&ctx, &mut app, "Redo");
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .last()
                    .unwrap(),
                &updated
            );
            // The cancelled local annotation is still a draft. Redo changed the
            // live record, so it must not silently adopt a fresh write precondition.
            layers_frame_output(&ctx, &mut app, vec![]);
            let draft = app.drawings_ui.figure_unknown_draft.as_ref().unwrap();
            assert_ne!(draft.2, draft.3);
            assert_ne!(draft.1, crate::diagrams::expected(&updated));
            let before_reload = app.host.document.editor.project.clone();
            button(&ctx, &mut app, "Reload purpose settings");
            let draft = app.drawings_ui.figure_unknown_draft.as_ref().unwrap();
            assert_eq!(draft.2, draft.3);
            assert_eq!(draft.1, crate::diagrams::expected(&updated));
            assert_eq!(
                app.host.document.editor.project, before_reload,
                "reloading an optional UI draft cannot write the model or figure"
            );
            choose_candidate(&ctx, &mut app, zone_task.candidates[1].option);
            button(&ctx, &mut app, "Preview function diagram");
            button(&ctx, &mut app, "Confirm design change");
            let other = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .last()
                .unwrap()
                .clone();
            assert_ne!(other.id, updated.id);
            let a_groups = updated.function_groups();
            let b_groups = other.function_groups();
            let common = a_groups
                .iter()
                .find_map(|g| {
                    b_groups
                        .iter()
                        .find(|h| h.purpose == g.purpose && !h.unknown && !g.unknown)
                        .map(|h| (g, h))
                })
                .unwrap();
            assert_eq!(common.0.key, common.1.key);
            assert_eq!(common.0.color, common.1.color);
            button(&ctx, &mut app, "Edit source model");
            app.fit();
            let selected_b = zone_task.candidates[1].element_ids()[0];
            let e = app
                .host
                .document
                .editor
                .project
                .elements
                .iter()
                .find(|e| e.id == selected_b)
                .unwrap()
                .clone();
            let Shape::Zone { spec } = &e.shape else {
                unreachable!()
            };
            layers_frame_output(&ctx, &mut app, vec![]);
            let point = app.screen(app.workbench.panel_rects["view.canvas"], spec.label_point());
            layers_click(&ctx, &mut app, point);
            edit_purpose(&ctx, &mut app, &spec.purpose, "B local trial");
            button(&ctx, &mut app, "Undo");
            assert!(app.host.document.editor.project.elements.contains(&e));
            assert!(
                app.host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .contains(&updated)
            );
            let expected = app.host.document.editor.project.clone();
            assert!(
                initial
                    .drawings
                    .diagrams
                    .iter()
                    .all(|f| expected.drawings.diagrams.contains(f))
            );
            assert_eq!(expected.design_process.tasks, initial.design_process.tasks);
            assert_eq!(
                expected.design_process.decisions,
                initial.design_process.decisions
            );
            assert_eq!(
                expected.design_process.updates,
                initial.design_process.updates
            );
            button(&ctx, &mut app, "Show function diagram");
            layers_frame_output(&ctx, &mut app, vec![]);
            let svg = root.join("function.svg");
            let old = app.drawings_ui.figure_export.clone();
            text(&ctx, &mut app, &old, &svg.to_string_lossy());
            button(&ctx, &mut app, "Export current SVG");
            assert!(svg.exists(), "{}", app.drawings_ui.error);
            let saved = root.join("continued.openarchi.json");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let old = app.path.clone();
            text(&ctx, &mut app, &old, &saved.to_string_lossy());
            button(&ctx, &mut app, "Save");
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let reopened = Editor::from_stored_document(&saved).unwrap();
            let mut rebound = expected.clone();
            for (p, q) in rebound
                .drawings
                .pages
                .iter_mut()
                .zip(&reopened.project.drawings.pages)
            {
                assert_eq!(
                    crate::drawings::verified_bytes(p).unwrap(),
                    crate::drawings::verified_bytes(q).unwrap()
                );
                p.path = q.path.clone();
            }
            assert_eq!(reopened.project, rebound);
            std::fs::write(root.join("walkthrough.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"output":saved,"svg":svg,"language":format!("{language:?}"),"entry":"actual workspace pointer/text controls; not native OS/GPU","created_A_id":created.id,"A_generation":updated.generation,"B_id":other.id,"unknown_annotation":unknown_purpose,"original_elements":initial.elements.len(),"output_elements":expected.elements.len(),"changed_source_purpose_only":selected,"old_mass_diagrams_preserved":true,"old_tasks_choices_updates_preserved":true,"same_label_colours_across_candidates":true,"fixture_manual_not_AI":true,"native_desktop":"not_run","current_package":"previous840E; does not include this source"})).unwrap()).unwrap();
            println!(
                "FUNCTION_PRODUCT_WALK={}",
                root.join("walkthrough.json").display()
            );
            drop(app.automation.take());
            if input.is_none() {
                std::fs::remove_dir_all(root).unwrap();
            }
        }
    }

    #[test]
    fn accesskit_task_and_candidate_choices_close_popup_without_extra_task_write() {
        use egui::accesskit::{Action, ActionRequest, Node, NodeId, Role, TreeId};
        fn merge(out: egui::FullOutput, cache: &mut std::collections::BTreeMap<NodeId, Node>) {
            if let Some(update) = out.platform_output.accesskit_update {
                for (id, node) in update.nodes {
                    cache.insert(id, node);
                }
            }
        }
        fn click(
            ctx: &Context,
            app: &mut AtelierApp,
            cache: &mut std::collections::BTreeMap<NodeId, Node>,
            name: &str,
            combo: bool,
        ) {
            reveal_manual_control(ctx, app, name);
            merge(layers_frame_output(ctx, app, vec![]), cache);
            let id = *cache
                .iter()
                .find(|(_, n)| {
                    n.supports_action(Action::Click)
                        && (n.label() == Some(name) || n.value() == Some(name))
                        && (n.role() == Role::ComboBox) == combo
                })
                .map(|(id, _)| id)
                .unwrap_or_else(|| {
                    panic!(
                        "actual accessibility click node {name}; {:?}",
                        cache
                            .iter()
                            .map(|(_, n)| (n.role(), n.label(), n.value()))
                            .collect::<Vec<_>>()
                    )
                });
            merge(
                layers_frame_output(
                    ctx,
                    app,
                    vec![Event::AccessKitActionRequest(ActionRequest {
                        action: Action::Click,
                        target_tree: TreeId::ROOT,
                        target_node: id,
                        data: None,
                    })],
                ),
                cache,
            );
            merge(layers_frame_output(ctx, app, vec![]), cache);
        }
        let (ctx, mut app) = setup();
        ctx.enable_accesskit();
        ctx.global_style_mut(|s| s.animation_time = 0.);
        app.host.document.editor = Editor::new(storage::synthetic_building_fixture());
        let ids = app
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .filter(|e| e.story == 0 && matches!(e.shape, Shape::Zone { .. }))
            .map(|e| e.id)
            .collect();
        let prepared = crate::design_process::service::prepare_create(
            &app.host.document.editor,
            &storage::new_document_identity(),
            crate::design_process::service::Draft {
                name: "AX task".into(),
                question: "Return from task selection to the canvas".into(),
                statements: vec![],
                candidate_names: ["AX A".into(), "AX B".into()],
                source_ids: ids,
                entry: crate::design_process::Entry::ManualInterface,
            },
        )
        .unwrap();
        app.host
            .document
            .editor
            .commit_change("ax-fixture", &prepared)
            .unwrap();
        let task = app
            .host
            .document
            .editor
            .project
            .design_process
            .task()
            .unwrap()
            .clone();
        let before = app.host.document.editor.project.clone();
        let revision = app.host.document.editor.revision;
        let mut cache = std::collections::BTreeMap::new();
        click(&ctx, &mut app, &mut cache, &task.name, true);
        assert!(egui::Popup::is_any_open(&ctx));
        click(&ctx, &mut app, &mut cache, &task.name, false);
        assert!(
            !egui::Popup::is_any_open(&ctx),
            "assistive activation must close the selected task popup"
        );
        assert_eq!(app.host.document.editor.project, before);
        assert_eq!(app.host.document.editor.revision, revision);
        let first = app
            .host
            .document
            .editor
            .project
            .design_options
            .name(task.candidates[0].option)
            .unwrap()
            .to_owned();
        let second = app
            .host
            .document
            .editor
            .project
            .design_options
            .name(task.candidates[1].option)
            .unwrap()
            .to_owned();
        click(&ctx, &mut app, &mut cache, &first, true);
        assert!(egui::Popup::is_any_open(&ctx));
        click(&ctx, &mut app, &mut cache, &second, false);
        assert!(!egui::Popup::is_any_open(&ctx));
        assert_eq!(
            app.host
                .document
                .editor
                .project
                .plan_options
                .active_design_option,
            Some(task.candidates[1].option)
        );
        assert_eq!(app.host.document.editor.project.elements, before.elements);
    }
    #[test]
    fn exploded_axon_normal_task_preview_cancel_edit_history_export_and_reopen() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let input = std::env::var_os("OPENARCHI_AXON_INPUT").map(PathBuf::from);
            if input.is_some() && language == UiLanguage::SimplifiedChinese {
                continue;
            }
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            let root = std::env::var_os("OPENARCHI_AXON_OUTPUT")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::temp_dir()
                        .join(format!("oa-axon-ui-{}", storage::new_document_identity()))
                });
            std::fs::create_dir(&root)
                .expect("inspect original results, do not replay unknown writes");
            app.host.document.editor = if let Some(input) = &input {
                Editor::from_stored_document(input).unwrap()
            } else {
                Editor::new(storage::synthetic_building_fixture())
            };
            if input.is_none() {
                let sources = app
                    .host
                    .document
                    .editor
                    .project
                    .elements
                    .iter()
                    .filter(|e| e.story == 0 && matches!(e.shape, Shape::Zone { .. }))
                    .map(|e| e.id)
                    .collect();
                let prepared = crate::design_process::service::prepare_create(
                    &app.host.document.editor,
                    &storage::new_document_identity(),
                    crate::design_process::service::Draft {
                        name: "Axon design task".into(),
                        question: "Inspect spatial solids without moving the source".into(),
                        statements: vec![],
                        candidate_names: ["A spaces".into(), "B spaces".into()],
                        source_ids: sources,
                        entry: crate::design_process::Entry::ManualInterface,
                    },
                )
                .unwrap();
                app.host
                    .document
                    .editor
                    .commit_change("axon-fixture", &prepared)
                    .unwrap();
            }
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            let initial = app.host.document.editor.project.clone();
            let task = initial
                .design_process
                .tasks
                .iter()
                .find(|t| !t.is_mass())
                .unwrap()
                .clone();
            std::fs::write(root.join("operation-plan.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"input_model_sha256":crate::automation_journal::canonical_hash(&initial).unwrap(),"output":root.join("continued.openarchi.json"),"entry":"normal workspace task/candidate/proposal/source inspector/SVG/Save; not OS/GPU","task":task.id,"AI":"not_run"})).unwrap()).unwrap();
            if initial.design_process.current_task.as_deref() != Some(task.id.as_str()) {
                let name = initial.design_process.task().unwrap().name.clone();
                button(&ctx, &mut app, &name);
                button(&ctx, &mut app, &task.name);
                button(&ctx, &mut app, "Confirm design change");
            }
            choose_candidate(&ctx, &mut app, task.candidates[0].option);
            let original = app.host.document.editor.project.clone();
            let selected = task.candidates[0].element_ids()[0];
            let source = original.elements.iter().find(|e| e.id == selected).unwrap();
            let Shape::Zone { spec } = &source.shape else {
                unreachable!()
            };
            let old_height = spec.height;
            let target = spec.label_point();
            app.fit();
            app.plan_center = target;
            app.plan_scale = 35.;
            layers_frame_output(&ctx, &mut app, vec![]);
            let at = app.screen(app.workbench.panel_rects["view.canvas"], target);
            layers_click(&ctx, &mut app, at);
            assert_eq!(app.host.document.editor.selection_ids(), vec![selected]);
            button(&ctx, &mut app, "Preview exploded axon");
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, original);
            button(&ctx, &mut app, "Preview exploded axon");
            button(&ctx, &mut app, "Confirm design change");
            let created = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .last()
                .unwrap()
                .clone();
            assert!(created.definition.presentation.is_axon());
            assert_eq!(created.snapshot.elements.len(), 9);
            assert_eq!(app.host.document.editor.project.elements, original.elements);
            assert!(matches!(app.view,View::Figure(id)if id==created.id));
            let frame = layers_frame_output(&ctx, &mut app, vec![]);
            for label in [
                "Orthographic axon · display offsets only · original model quantities",
                "Orthographic axon · XYZ directions · paper scale unspecified",
                "Display separation is not a storey elevation or measured distance",
            ] {
                assert!(
                    frame.shapes.iter().any(|s| text_center(
                        std::slice::from_ref(s),
                        language.translate(label)
                    )
                    .is_some_and(|p| s.clip_rect.contains(p))),
                    "visible axon explanation {label}"
                );
            }
            // Actual numerical widget input, never a direct DrawingsUi parameter assignment.
            button(&ctx, &mut app, "Axon display separation");
            text(&ctx, &mut app, "1000", "1500");
            layers_frame_output(&ctx, &mut app, vec![key(Key::Tab, Modifiers::NONE)]);
            button(&ctx, &mut app, "Preview display separation");
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .last()
                    .unwrap(),
                &created
            );
            button(&ctx, &mut app, "Preview display separation");
            button(&ctx, &mut app, "Confirm design change");
            let configured = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .last()
                .unwrap()
                .clone();
            assert_eq!(configured.generation, 2);
            assert_eq!(configured.snapshot, created.snapshot);
            assert_eq!(app.host.document.editor.project.elements, original.elements);
            button(&ctx, &mut app, "Edit source model");
            assert_eq!(app.host.document.editor.selection_ids(), vec![selected]);
            height(&ctx, &mut app, old_height + 0.25);
            button(&ctx, &mut app, "Show exploded axon");
            let stale = app.host.document.editor.project.clone();
            assert_eq!(configured.status(&stale), Status::Stale);
            assert!(configured.svg(&stale).is_err());
            button(&ctx, &mut app, "Preview figure update");
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, stale);
            button(&ctx, &mut app, "Preview figure update");
            button(&ctx, &mut app, "Confirm design change");
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .last()
                    .unwrap()
                    .generation,
                3
            );
            button(&ctx, &mut app, "Undo");
            button(&ctx, &mut app, "Undo");
            assert_eq!(app.host.document.editor.project.elements, original.elements);
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .last()
                    .unwrap(),
                &configured
            );
            button(&ctx, &mut app, "Redo");
            button(&ctx, &mut app, "Redo");
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .last()
                    .unwrap()
                    .status(&app.host.document.editor.project),
                Status::Current
            );
            button(&ctx, &mut app, "Undo");
            button(&ctx, &mut app, "Undo");
            button(&ctx, &mut app, "Undo");
            assert_eq!(app.host.document.editor.project.elements, original.elements);
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .last()
                    .unwrap(),
                &created
            );
            choose_candidate(&ctx, &mut app, task.candidates[1].option);
            button(&ctx, &mut app, "Preview exploded axon");
            button(&ctx, &mut app, "Confirm design change");
            let other = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .last()
                .unwrap()
                .clone();
            assert_eq!(other.definition.option, Some(task.candidates[1].option));
            assert_ne!(created.id, other.id);
            assert_eq!(app.host.document.editor.project.elements, initial.elements);
            button(&ctx, &mut app, "Edit source model");
            let b_source = app
                .host
                .document
                .editor
                .project
                .elements
                .iter()
                .find(|e| e.id == task.candidates[1].element_ids()[0])
                .unwrap()
                .clone();
            let Shape::Zone { spec: b_spec } = &b_source.shape else {
                unreachable!()
            };
            app.fit();
            layers_frame_output(&ctx, &mut app, vec![]);
            let at = app.screen(
                app.workbench.panel_rects["view.canvas"],
                b_spec.label_point(),
            );
            layers_click(&ctx, &mut app, at);
            assert_eq!(app.host.document.editor.selection_ids(), vec![b_source.id]);
            height(&ctx, &mut app, b_spec.height + 0.1);
            button(&ctx, &mut app, "Show exploded axon");
            assert_eq!(
                other.status(&app.host.document.editor.project),
                Status::Stale
            );
            button(&ctx, &mut app, "Undo");
            assert_eq!(
                other.status(&app.host.document.editor.project),
                Status::Current
            );
            assert_eq!(app.host.document.editor.project.elements, initial.elements);
            let expected = app.host.document.editor.project.clone();
            for f in &initial.drawings.diagrams {
                assert!(expected.drawings.diagrams.contains(f));
            }
            for original in [&initial.design_process.tasks] {
                assert_eq!(&expected.design_process.tasks, original);
            }
            assert_eq!(
                expected.design_process.decisions,
                initial.design_process.decisions
            );
            assert_eq!(
                expected.design_process.updates,
                initial.design_process.updates
            );
            let svg = root.join("axon.svg");
            let old = app.drawings_ui.figure_export.clone();
            text(&ctx, &mut app, &old, &svg.to_string_lossy());
            button(&ctx, &mut app, "Export current SVG");
            assert!(svg.exists(), "{}", app.drawings_ui.error);
            let saved = root.join("continued.openarchi.json");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let old = app.path.clone();
            text(&ctx, &mut app, &old, &saved.to_string_lossy());
            button(&ctx, &mut app, "Save");
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let reopened = Editor::from_stored_document(&saved).unwrap();
            let mut rebound = expected.clone();
            for (p, q) in rebound
                .drawings
                .pages
                .iter_mut()
                .zip(&reopened.project.drawings.pages)
            {
                assert_eq!(
                    crate::drawings::verified_bytes(p).unwrap(),
                    crate::drawings::verified_bytes(q).unwrap()
                );
                p.path = q.path.clone();
            }
            assert_eq!(reopened.project, rebound);
            std::fs::write(root.join("walkthrough.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"output":saved,"svg":svg,"language":format!("{language:?}"),"normal_product_entry":true,"figure_A":created.id,"figure_B":other.id,"all_original_elements_and_history_preserved":true,"native_desktop":"not_run","real_AI":"not_run","source_height_probe":"undone; not promoted","configuration_probe":"undone"})).unwrap()).unwrap();
            println!(
                "AXON_PRODUCT_WALK={}",
                root.join("walkthrough.json").display()
            );
        }
    }
    fn site_node_number(ctx: &Context, app: &mut AtelierApp, index: usize, axis: &str, value: f64) {
        layers_frame_output(ctx, app, vec![]);
        let label = format!("{} {} {}", app.language.translate("Node"), index, axis);
        let mut at = None;
        for _ in 0..35 {
            let frame = layers_frame_output(ctx, app, vec![]);
            if let Some(q) = frame.shapes.iter().find_map(|s| {
                text_center(std::slice::from_ref(s), &label)
                    .filter(|p| s.clip_rect.contains(*p) && p.x > 1000.)
            }) {
                at = frame.shapes.iter().find_map(|s| {
                    let egui::Shape::Text(t) = &s.shape else {
                        return None;
                    };
                    let p = t.pos + t.galley.size() / 2.;
                    (s.clip_rect.contains(p)
                        && p.x > q.x
                        && (p.y - q.y).abs() < 10.
                        && t.galley.text().ends_with(" m"))
                    .then_some(p)
                });
            }
            if at.is_some() {
                break;
            }
            layers_frame_output(
                ctx,
                app,
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
        layers_click(
            ctx,
            app,
            at.expect("actual visible source boundary coordinate"),
        );
        layers_frame_output(
            ctx,
            app,
            vec![
                key(Key::A, Modifiers::COMMAND),
                Event::Text(value.to_string()),
            ],
        );
        layers_frame_output(ctx, app, vec![key(Key::Tab, Modifiers::NONE)]);
        button(ctx, app, "Apply changes");
    }
    #[test]
    fn site_figures_normal_no_brief_gate_boundary_edit_config_history_export_and_reopen() {
        use crate::diagrams::{Presentation, site};
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            for boundary_fixture in [false, true] {
                let input = std::env::var_os("OPENARCHI_SITE_FIGURE_INPUT").map(PathBuf::from);
                if input.is_some()
                    && (language == UiLanguage::SimplifiedChinese || boundary_fixture)
                {
                    continue;
                }
                let (ctx, mut app) = setup();
                app.language = language;
                ctx.global_style_mut(|s| s.animation_time = 0.);
                let root = std::env::var_os("OPENARCHI_SITE_FIGURE_OUTPUT")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        std::env::temp_dir().join(format!(
                            "oa-site-figure-ui-{}",
                            storage::new_document_identity()
                        ))
                    });
                std::fs::create_dir(&root)
                    .expect("existing output requires recovery, not write replay");
                app.host.document.editor = if let Some(input) = &input {
                    Editor::from_stored_document(input).unwrap()
                } else if boundary_fixture {
                    let mut e = Editor::new(Project::default());
                    e.apply(ModelCommand::ApplySiteBoundary {
                        proposal: crate::site::boundary::BoundaryProposal {
                            name: "Synthetic plot, not a real site".into(),
                            story: 0,
                            layer: 0,
                            outline_m: vec![[30., 30.], [50., 30.], [50., 60.], [30., 60.]],
                            origin: crate::site::boundary::BoundaryOrigin {
                                source_sha256: "a".repeat(64),
                                entity_handle: "A1".into(),
                                record_sha256: "b".repeat(64),
                                unit: crate::site::LengthUnit::Metre,
                                placement: crate::site::boundary::Placement::ModelAxes,
                                centerline_width_native: None,
                            },
                            expected_previous: None,
                        },
                    })
                    .unwrap();
                    e
                } else {
                    Editor::new(storage::synthetic_building_fixture())
                };
                app.host
                    .configure_operation_journal(Some(root.join("journal")))
                    .unwrap();
                let before = app.host.document.editor.project.clone();
                let has_boundary = before.site.boundary.is_some();
                std::fs::write(root.join("operation-plan.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"model_sha256":crate::automation_journal::canonical_hash(&before).unwrap(),"output":root.join("continued.openarchi.json"),"entry":"normal workspace same canvas, no forced design task/brief; existing source inspector; SVG and Save","source_boundary":has_boundary,"actual_AI":"not_run"})).unwrap()).unwrap();
                button(&ctx, &mut app, "Preview location diagram");
                button(&ctx, &mut app, "Cancel design proposal");
                assert_eq!(app.host.document.editor.project, before);
                button(&ctx, &mut app, "Preview location diagram");
                button(&ctx, &mut app, "Confirm design change");
                let location = app
                    .host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .last()
                    .unwrap()
                    .clone();
                assert!(matches!(
                    location.definition.presentation,
                    Presentation::SiteLocation
                ));
                assert_eq!(app.host.document.editor.project.elements, before.elements);
                assert_eq!(
                    app.host.document.editor.project.design_process,
                    before.design_process
                );
                let frame = layers_frame_output(&ctx, &mut app, vec![]);
                let unknown = if has_boundary {
                    "Boundary recorded; survey approval unknown"
                } else {
                    "Boundary unknown; no boundary invented"
                };
                assert!(
                    frame.shapes.iter().any(|s| text_center(
                        std::slice::from_ref(s),
                        language.translate(unknown)
                    )
                    .is_some_and(|p| s.clip_rect.contains(p))),
                    "unknown source state visible"
                );
                let mut active = location.clone();
                if has_boundary {
                    button(&ctx, &mut app, "Preview setback study");
                    button(&ctx, &mut app, "Cancel design proposal");
                    assert_eq!(
                        app.host
                            .document
                            .editor
                            .project
                            .drawings
                            .diagrams
                            .last()
                            .unwrap(),
                        &location
                    );
                    button(&ctx, &mut app, "Preview setback study");
                    button(&ctx, &mut app, "Confirm design change");
                    active = app
                        .host
                        .document
                        .editor
                        .project
                        .drawings
                        .diagrams
                        .last()
                        .unwrap()
                        .clone();
                    assert!(matches!(
                        active.definition.presentation,
                        Presentation::SetbackStudy {
                            distance_mm: 3000,
                            ..
                        }
                    ));
                    button(&ctx, &mut app, "Setback study settings");
                    text(&ctx, &mut app, "3000", "5000");
                    layers_frame_output(&ctx, &mut app, vec![key(Key::Tab, Modifiers::NONE)]);
                    button(&ctx, &mut app, "Preview study settings");
                    button(&ctx, &mut app, "Cancel design proposal");
                    assert_eq!(
                        app.host
                            .document
                            .editor
                            .project
                            .drawings
                            .diagrams
                            .last()
                            .unwrap(),
                        &active
                    );
                    button(&ctx, &mut app, "Preview study settings");
                    button(&ctx, &mut app, "Confirm design change");
                    let configured = app
                        .host
                        .document
                        .editor
                        .project
                        .drawings
                        .diagrams
                        .last()
                        .unwrap()
                        .clone();
                    assert_eq!(configured.generation, 2);
                    assert_eq!(configured.snapshot, active.snapshot);
                    assert_eq!(app.host.document.editor.project.elements, before.elements);
                    button(&ctx, &mut app, "Undo");
                    button(&ctx, &mut app, "Redo");
                    assert_eq!(
                        app.host
                            .document
                            .editor
                            .project
                            .drawings
                            .diagrams
                            .last()
                            .unwrap(),
                        &configured
                    );
                    button(&ctx, &mut app, "Undo");
                    assert_eq!(
                        app.host
                            .document
                            .editor
                            .project
                            .drawings
                            .diagrams
                            .last()
                            .unwrap(),
                        &active
                    );
                    button(&ctx, &mut app, "Setback study settings");
                } else {
                    button(&ctx, &mut app, "Preview setback study");
                    assert!(app.process_ui.preview.is_none());
                    assert_eq!(
                        app.host
                            .document
                            .editor
                            .project
                            .drawings
                            .diagrams
                            .last()
                            .unwrap(),
                        &location
                    );
                }
                // Return to the existing source inspector, then perform an actual numeric edit.
                button(&ctx, &mut app, "Edit source model");
                app.fit();
                layers_frame_output(&ctx, &mut app, vec![]);
                if let Some(b) = &before.site.boundary {
                    let src = before
                        .elements
                        .iter()
                        .find(|e| e.id == b.element_id)
                        .unwrap();
                    let Shape::Polyline { points, .. } = &src.shape else {
                        unreachable!()
                    };
                    let mid = Point::new(
                        (points[0].x + points[1].x) / 2.,
                        (points[0].y + points[1].y) / 2.,
                    );
                    let at = app.screen(app.workbench.panel_rects["view.canvas"], mid);
                    layers_click(&ctx, &mut app, at);
                    assert_eq!(app.host.document.editor.selection_ids(), vec![b.element_id]);
                    site_node_number(&ctx, &mut app, 2, "X", points[1].x + 0.5);
                } else {
                    let e = active.snapshot.elements.first().unwrap();
                    let (p, h) = match &e.shape {
                        Shape::Zone { spec } => (spec.label_point(), spec.height),
                        Shape::ConceptMass { spec } => {
                            let (a, b) = crate::slab_geometry::bounds(&spec.outline);
                            (Point::new((a.x + b.x) / 2., (a.y + b.y) / 2.), spec.height)
                        }
                        _ => unreachable!(),
                    };
                    let at = app.screen(app.workbench.panel_rects["view.canvas"], p);
                    layers_click(&ctx, &mut app, at);
                    assert_eq!(app.host.document.editor.selection_ids(), vec![e.id]);
                    height(&ctx, &mut app, h + 0.1);
                }
                button(
                    &ctx,
                    &mut app,
                    if has_boundary {
                        "Show setback study"
                    } else {
                        "Show location diagram"
                    },
                );
                let edited = app.host.document.editor.project.clone();
                assert_eq!(active.status(&edited), Status::Stale);
                assert!(active.svg(&edited).is_err());
                button(&ctx, &mut app, "Preview figure update");
                button(&ctx, &mut app, "Cancel design proposal");
                assert_eq!(app.host.document.editor.project, edited);
                button(&ctx, &mut app, "Preview figure update");
                button(&ctx, &mut app, "Confirm design change");
                let updated = app
                    .host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .last()
                    .unwrap()
                    .clone();
                assert_eq!(updated.id, active.id);
                assert_eq!(updated.generation, 2);
                assert_eq!(
                    updated.status(&app.host.document.editor.project),
                    Status::Current
                );
                if has_boundary {
                    assert!(updated.snapshot.site.as_ref().unwrap().user_modified());
                }
                button(&ctx, &mut app, "Undo");
                button(&ctx, &mut app, "Undo");
                assert_eq!(app.host.document.editor.project.elements, before.elements);
                assert_eq!(
                    active.status(&app.host.document.editor.project),
                    Status::Current
                );
                button(&ctx, &mut app, "Redo");
                button(&ctx, &mut app, "Redo");
                assert_eq!(
                    app.host
                        .document
                        .editor
                        .project
                        .drawings
                        .diagrams
                        .last()
                        .unwrap(),
                    &updated
                );
                button(&ctx, &mut app, "Undo");
                button(&ctx, &mut app, "Undo");
                let expected = app.host.document.editor.project.clone();
                assert_eq!(expected.elements, before.elements);
                assert_eq!(expected.site, before.site);
                assert_eq!(expected.design_process, before.design_process);
                for f in &before.drawings.diagrams {
                    assert!(expected.drawings.diagrams.contains(f));
                }
                let svg = root.join("site.svg");
                let old = app.drawings_ui.figure_export.clone();
                text(&ctx, &mut app, &old, &svg.to_string_lossy());
                button(&ctx, &mut app, "Export current SVG");
                assert!(svg.exists(), "{}", app.drawings_ui.error);
                let saved = root.join("continued.openarchi.json");
                layers_frame_output(
                    &ctx,
                    &mut app,
                    vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
                );
                let old = app.path.clone();
                text(&ctx, &mut app, &old, &saved.to_string_lossy());
                button(&ctx, &mut app, "Save");
                assert!(app.file_action.is_none(), "{}", app.file_error);
                let reopened = Editor::from_stored_document(&saved).unwrap();
                let mut rebound = expected.clone();
                for (p, q) in rebound
                    .drawings
                    .pages
                    .iter_mut()
                    .zip(&reopened.project.drawings.pages)
                {
                    assert_eq!(
                        crate::drawings::verified_bytes(p).unwrap(),
                        crate::drawings::verified_bytes(q).unwrap()
                    );
                    p.path = q.path.clone();
                }
                assert_eq!(reopened.project, rebound);
                let summary =
                    site::figure_summary(reopened.project.drawings.diagrams.last().unwrap());
                assert_eq!(summary["boundary_approval"], "unknown");
                assert_eq!(summary["compliance"], "not_evaluated");
                std::fs::write(root.join("walkthrough.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"output":saved,"svg":svg,"language":format!("{language:?}"),"entry":"normal workspace pointer/text; source inspector/preview/history/SVG/Save","has_boundary":has_boundary,"location_id":location.id,"study_id":has_boundary.then_some(active.id),"original_model_site_task_history_and_diagrams_preserved":true,"source_edit_probe":"undone","native_OS_GPU":"not_run","real_AI":"not_run"})).unwrap()).unwrap();
                println!(
                    "SITE_PRODUCT_WALK={}",
                    root.join("walkthrough.json").display()
                );
            }
        }
    }
    #[test]
    fn site_native_tessellated_fill_and_strokes_do_not_escape_source_figure() {
        let editor = if let Some(p) = std::env::var_os("OPENARCHI_SITE_RENDER_INPUT") {
            Editor::from_stored_document(&PathBuf::from(p)).unwrap()
        } else {
            let mut e = Editor::new(Project::default());
            e.apply(ModelCommand::ApplySiteBoundary {
                proposal: crate::site::boundary::BoundaryProposal {
                    name: "Synthetic concave plot".into(),
                    story: 0,
                    layer: 0,
                    outline_m: vec![
                        [54560., 50470.],
                        [54640., 50469.5],
                        [54639.8, 50438.],
                        [54640., 50354.],
                        [54520., 50355.],
                        [54514., 50351.],
                        [54513., 50334.],
                        [54482., 50334.],
                    ],
                    origin: crate::site::boundary::BoundaryOrigin {
                        source_sha256: "a".repeat(64),
                        entity_handle: "A1".into(),
                        record_sha256: "b".repeat(64),
                        unit: crate::site::LengthUnit::Metre,
                        placement: crate::site::boundary::Placement::ModelAxes,
                        centerline_width_native: None,
                    },
                    expected_previous: None,
                },
            })
            .unwrap();
            let c = e
                .prepare_change(
                    e.revision,
                    vec![ModelCommand::Diagram {
                        edit: crate::diagrams::Edit::Create {
                            name: "Study".into(),
                            definition: crate::diagrams::Definition {
                                story: 0,
                                option: None,
                                domain: crate::diagrams::Domain::Site,
                                presentation: crate::diagrams::Presentation::SetbackStudy {
                                    distance_mm: 3000,
                                    assumption: "Synthetic design assumption".into(),
                                },
                            },
                        },
                    }],
                )
                .unwrap();
            e.commit_change("render-fixture", &c).unwrap();
            e
        };
        let figure = editor
            .project
            .drawings
            .diagrams
            .iter()
            .find(|f| {
                matches!(
                    f.definition.presentation,
                    crate::diagrams::Presentation::SetbackStudy { .. }
                )
            })
            .unwrap();
        let page = crate::diagrams::site::Page::new(figure).unwrap();
        for width in [500., 600., 750., 1000., 1200.] {
            for ppp in [1., 1.3125, 2.] {
                let ctx = Context::default();
                ctx.set_pixels_per_point(ppp);
                let mut top = Pos2::ZERO;
                for _ in 0..2 {
                    let mut warm = ctx.run_ui(
                        RawInput {
                            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 1100.))),
                            ..Default::default()
                        },
                        |ui| {
                            AtelierApp::paint_figure(ui, figure, UiLanguage::English, width);
                        },
                    );
                    warm.textures_delta.clear();
                }
                let mut out = ctx.run_ui(
                    RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 1100.))),
                        ..Default::default()
                    },
                    |ui| {
                        top = ui.cursor().min;
                        AtelierApp::paint_figure(ui, figure, UiLanguage::English, width);
                    },
                );
                let points: Vec<_> = page
                    .paths
                    .iter()
                    .filter(|p| p.kind == crate::diagrams::site::Kind::Inset)
                    .flat_map(|p| p.rings.iter().flatten())
                    .map(|p| {
                        let q = page.layout.point(*p);
                        top + vec2(q[0] as f32, q[1] as f32) * (width / 1200.)
                    })
                    .collect();
                let bounds = Rect::from_points(&points).expand(3.);
                out.textures_delta.clear();
                let actual_top = out
                    .shapes
                    .iter()
                    .find_map(|s| match &s.shape {
                        egui::Shape::Rect(r) if r.fill == Color32::WHITE => Some(r.rect.min),
                        _ => None,
                    })
                    .expect("painted figure page");
                let offset = actual_top - top;
                let bounds = bounds.translate(offset);
                let primitives = ctx.tessellate(out.shapes, ppp);
                let legend_index = crate::diagrams::site::legend(&page)
                    .iter()
                    .position(|(kind, _, _)| *kind == crate::diagrams::site::Kind::Inset)
                    .unwrap() as f32;
                let legend = Rect::from_min_max(
                    actual_top + vec2(835., 650. + legend_index * 19.) * (width / 1200.),
                    actual_top + vec2(845., 660. + legend_index * 19.) * (width / 1200.),
                )
                .expand(3.);
                let color = Color32::from_rgb(71, 116, 182);
                let mut checked = 0;
                for p in primitives {
                    if let egui::epaint::Primitive::Mesh(m) = p.primitive {
                        for v in m.vertices {
                            if v.color == color
                                || v.color == Color32::from_rgba_unmultiplied(71, 116, 182, 31)
                            {
                                checked += 1;
                                assert!(
                                    bounds.contains(v.pos) || legend.contains(v.pos),
                                    "inset fill/stroke escaped source bounds: width={width} ppp={ppp} point={:?} bounds={bounds:?}",
                                    v.pos
                                );
                            }
                        }
                    }
                }
                assert!(checked > 0);
            }
        }
    }

    #[test]
    fn normal_constraint_figure_from_checks_model_edit_report_update_save_bilingual() {
        use crate::diagrams::{self, Presentation};
        use crate::model::ModelAdapter;
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            let parent = std::env::var_os("OPENARCHI_CONSTRAINT_FIGURE_OUTPUT")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::temp_dir().join(format!(
                        "oa-constraint-ui-{}",
                        storage::new_document_identity()
                    ))
                });
            std::fs::create_dir_all(&parent).unwrap();
            let root = parent.join(if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            });
            std::fs::create_dir(&root).expect("do not replay or overwrite an unknown result");
            let supplied = std::env::var_os("OPENARCHI_CONSTRAINT_FIGURE_INPUT").map(PathBuf::from);
            let file = root.join("controlled.openarchi.json");
            let original_bytes = supplied.as_ref().map(|p| std::fs::read(p).unwrap());
            if let Some(source) = &supplied {
                std::fs::copy(source, &file).unwrap();
                let data: serde_json::Value =
                    serde_json::from_slice(original_bytes.as_ref().unwrap()).unwrap();
                for page in data["drawings"]["pages"].as_array().into_iter().flatten() {
                    let rel = PathBuf::from(page["path"].as_str().unwrap());
                    assert!(
                        !rel.is_absolute()
                            && !rel
                                .components()
                                .any(|c| matches!(c, std::path::Component::ParentDir))
                    );
                    let to = root.join(&rel);
                    std::fs::create_dir_all(to.parent().unwrap()).unwrap();
                    std::fs::copy(source.parent().unwrap().join(&rel), to).unwrap();
                }
            } else {
                // Fixed native19 input keeps this the create path; current
                // bundled startup's existing graph is separately verified.
                let mut p=crate::samples::concept_study(); p.drawings.diagrams.clear();
                storage::save(&p, &file).unwrap();
            }
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            open_test_project(&ctx, &mut app, &file);
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let baseline = app.host.document.editor.project.clone();
            let context = app.host.context();
            let history = app.host.document.editor.undo.len();
            button(&ctx, &mut app, "Check current design");
            button(&ctx, &mut app, "Preview constraint figure");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            assert_eq!(app.host.document.editor.project, baseline);
            button(&ctx, &mut app, "Cancel design proposal");
            assert!(app.checks_ui.open);
            assert_eq!(app.host.context(), context);
            assert_eq!(app.host.document.editor.undo.len(), history);
            button(&ctx, &mut app, "Preview constraint figure");
            button(&ctx, &mut app, "Confirm design change");
            assert!(app.process_ui.preview.is_none(), "{}", app.process_ui.error);
            assert!(
                !app.checks_ui.open,
                "the confirmed figure must be visible on the same canvas"
            );
            let figure = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .last()
                .unwrap()
                .clone();
            assert!(matches!(
                figure.definition.presentation,
                Presentation::ConstraintOverlay
            ));
            assert!(app.view == View::Figure(figure.id));
            assert_eq!(
                figure.status(&app.host.document.editor.project),
                Status::Current
            );
            assert_eq!(app.host.document.editor.project.elements, baseline.elements);
            assert_eq!(app.host.document.editor.project.rules, baseline.rules);
            assert_eq!(
                app.host.document.editor.project.design_process,
                baseline.design_process
            );
            let source = diagrams::checks::Source::read(&figure.snapshot).unwrap();
            assert_eq!(source.entries.len(), 6);
            if std::env::var_os("OPENARCHI_CONSTRAINT_FIGURE_PROFILE").is_some() {
                let first = std::time::Instant::now();
                layers_frame_output(&ctx, &mut app, vec![]);
                let first_ms = first.elapsed().as_secs_f64() * 1000.;
                let start = std::time::Instant::now();
                for _ in 0..30 {
                    layers_frame_output(&ctx, &mut app, vec![]);
                }
                let warm_ms = start.elapsed().as_secs_f64() * 1000. / 30.;
                std::fs::write(root.join("frame-timing.json"),serde_json::to_vec_pretty(&serde_json::json!({"boundary":"CPU egui event frame including current linked figure status/capture/painting; no GPU or AI","first_measured_frame_after_confirm_ms":first_ms,"warm_mean_of30_frames_ms":warm_ms,"model_elements":baseline.elements.len(),"checks":source.entries.len(),"snapshot_bytes":serde_json::to_vec(&figure.snapshot).unwrap().len(),"cold_IO_cache":"uncontrolled; no cold-cache claim","source_sha256":figure.source_sha256})).unwrap()).unwrap();
            }

            let chosen = source.entries[0].active_ids[0];
            let check_id = source.entries[0].check.definition.id.clone();
            let element = baseline.elements.iter().find(|e| e.id == chosen).unwrap();
            let old_height = match &element.shape {
                Shape::ConceptMass { spec } => spec.height,
                Shape::Zone { spec } => spec.height,
                _ => unreachable!(),
            };
            let output = layers_frame_output(&ctx, &mut app, vec![]);
            assert!(
                output.shapes.iter().any(|s| text_center(
                    std::slice::from_ref(s),
                    language.translate("Authored constraint checks · not project approval")
                )
                .is_some_and(|p| s.clip_rect.contains(p))),
                "actual overlay heading must be visible"
            );
            let svg = figure.svg(&app.host.document.editor.project).unwrap();
            std::fs::write(root.join("initial-overlay.svg"), svg).unwrap();
            button(&ctx, &mut app, "Full check list");
            button(&ctx, &mut app, &format!("#{chosen}"));
            assert!(app.view == View::Plan);
            assert_eq!(app.host.document.editor.selection_ids(), vec![chosen]);
            height(&ctx, &mut app, old_height + 0.25);
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .elements
                    .iter()
                    .filter(|e| baseline.elements.iter().find(|v| v.id == e.id) != Some(*e))
                    .count(),
                1
            );
            button(&ctx, &mut app, "Check current design");
            button(&ctx, &mut app, "Show constraint figure");
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .last()
                    .unwrap()
                    .status(&app.host.document.editor.project),
                Status::Stale
            );
            button(&ctx, &mut app, "Preview figure update");
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .last()
                    .unwrap()
                    .generation,
                1
            );
            button(&ctx, &mut app, "Preview figure update");
            button(&ctx, &mut app, "Confirm design change");
            let figure = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .last()
                .unwrap()
                .clone();
            assert_eq!(figure.id, baseline.drawings.next_id().unwrap());
            assert_eq!(figure.generation, 2);
            let source = diagrams::checks::Source::read(&figure.snapshot).unwrap();
            assert_eq!(
                source
                    .entries
                    .iter()
                    .find(|e| e.check.definition.id == check_id)
                    .unwrap()
                    .state,
                crate::constraints::State::Unknown,
                "figure refresh cannot secretly rerun a changed check"
            );
            let frame = layers_frame_output(&ctx, &mut app, vec![]);
            let visible = frame.shapes.iter().any(|s| {
                text_center(std::slice::from_ref(s), language.translate("Open check"))
                    .is_some_and(|p| s.clip_rect.contains(p))
            });
            if !visible {
                button(&ctx, &mut app, "Full check list");
            }
            button(&ctx, &mut app, "Open check");
            assert!(app.checks_ui.open);
            button(&ctx, &mut app, "Preview check update");
            button(&ctx, &mut app, "Confirm design change");
            let report = app
                .host
                .document
                .editor
                .project
                .rules
                .checks
                .iter()
                .find(|c| c.definition.id == check_id)
                .unwrap()
                .report
                .as_ref()
                .unwrap();
            assert_eq!(report.generation, 2);
            button(&ctx, &mut app, "Show constraint figure");
            button(&ctx, &mut app, "Preview figure update");
            button(&ctx, &mut app, "Confirm design change");
            let f = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .last()
                .unwrap();
            assert_eq!(f.generation, 3);
            assert_eq!(f.status(&app.host.document.editor.project), Status::Current);
            let expected = app
                .host
                .document
                .editor
                .project
                .rules
                .checks
                .iter()
                .find(|c| c.definition.id == check_id)
                .unwrap()
                .state(&app.host.document.editor.project);
            assert_eq!(
                diagrams::checks::Source::read(&f.snapshot)
                    .unwrap()
                    .entries
                    .iter()
                    .find(|e| e.check.definition.id == check_id)
                    .unwrap()
                    .state,
                expected
            );
            let after = app.host.document.editor.project.clone();
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .last()
                    .unwrap()
                    .generation,
                2
            );
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::ALT)],
            );
            assert_eq!(app.host.document.editor.project, after);
            button(&ctx, &mut app, "Save project");
            assert!(app.file_action.is_none(), "{}", app.file_error);
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let reopened = Editor::from_stored_document(&file).unwrap();
            // The protected input retains its original sidecar basename; normal
            // save binds the copies to this file's new basename. Only the asset
            // path may change: independently compare both byte streams and every
            // other page field, then compare the complete project.
            let mut rebound = after.clone();
            assert_eq!(
                rebound.drawings.pages.len(),
                reopened.project.drawings.pages.len()
            );
            for (expected, actual) in rebound
                .drawings
                .pages
                .iter_mut()
                .zip(&reopened.project.drawings.pages)
            {
                let old_bytes = std::fs::read(&expected.path).unwrap();
                let new_bytes = std::fs::read(&actual.path).unwrap();
                assert_eq!(old_bytes, new_bytes);
                assert_eq!(
                    crate::drawings::verified_bytes(expected).unwrap(),
                    new_bytes
                );
                assert_eq!(crate::drawings::verified_bytes(actual).unwrap(), new_bytes);
                expected.path = actual.path.clone();
                assert_eq!(&*expected, actual);
            }
            assert_eq!(reopened.project, rebound);
            let raw: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
            assert_eq!(raw["schema_version"], 20);
            assert_eq!(after.design_process, baseline.design_process);
            assert_eq!(
                after.drawings.diagrams[..baseline.drawings.diagrams.len()],
                baseline.drawings.diagrams
            );
            for page in &after.drawings.pages {
                assert!(std::fs::read(&page.path).is_ok());
            }
            if let Some(p) = supplied {
                assert_eq!(std::fs::read(p).unwrap(), original_bytes.unwrap());
            }
            std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"entry":"normal File Open / Check current design / preview cancel confirm / linked canvas list object / Inspector / stale / explicit check and figure update / history / Save","input":file,"figure_id":figure.id,"generation":3,"changed_object":chosen,"old_height":old_height,"new_height":old_height+0.25,"original_other_objects_tasks_options_figures_preserved":true,"native_format":20,"native_package":"not_run","real_AI":"not_run"})).unwrap()).unwrap();
        }
    }

    // Existing-context continuation is the task under test. Fixture authoring
    // below only supplies the already-saved task; all task actions use UI events.
    #[test]
    fn preliminary_loop_existing_candidates_checks_choice_local_edit_refresh_and_reopen_bilingual()
    {
        preliminary_loop_path(false);
    }
    #[test]
    fn preliminary_loop_deferred_reason_without_mandatory_form_and_reopen_bilingual() {
        preliminary_loop_path(true);
    }
    fn preliminary_loop_path(deferred_reason: bool) {
        use crate::{
            constraints as qc,
            design_process::{self as dp, Entry, service},
        };
        // Use the normal accessibility actions with actual per-frame nodes.
        // Painted labels behind an overlapping window or moving under smooth
        // wheel input are not reliable pointer targets on this long real task.
        fn activate(
            ctx: &Context,
            app: &mut AtelierApp,
            id: egui::accesskit::NodeId,
            action: egui::accesskit::Action,
        ) {
            layers_frame_output(
                ctx,
                app,
                vec![Event::AccessKitActionRequest(
                    egui::accesskit::ActionRequest {
                        action,
                        target_tree: egui::accesskit::TreeId::ROOT,
                        target_node: id,
                        data: None,
                    },
                )],
            );
            for _ in 0..3 {
                layers_frame_output(ctx, app, vec![]);
            }
        }
        fn stable_button(ctx: &Context, app: &mut AtelierApp, key: &str) {
            reveal_manual_control(ctx, app, key);
            let out = layers_frame_output(ctx, app, vec![]);
            let nodes = out
                .platform_output
                .accesskit_update
                .expect("actual product accessibility tree")
                .nodes;
            let (id, _) = nodes
                .iter()
                .find(|(_, n)| {
                    n.supports_action(egui::accesskit::Action::Click)
                        && (n.label() == Some(app.language.translate(key))
                            || n.value() == Some(app.language.translate(key)))
                })
                .unwrap_or_else(|| {
                    panic!(
                        "current accessible action {key}; error={}",
                        app.process_ui.error
                    )
                });
            activate(ctx, app, *id, egui::accesskit::Action::Click);
        }
        fn assert_label(ctx: &Context, app: &mut AtelierApp, key: &str) {
            if key == "No check is bound to this candidate" {
                let id = ctx
                    .data(|d| d.get_temp::<Id>(Id::new("unbound_candidate_check_label")))
                    .unwrap();
                activate(
                    ctx,
                    app,
                    id.accesskit_id(),
                    egui::accesskit::Action::ScrollIntoView,
                );
            }
            let out = layers_frame_output(ctx, app, vec![]);
            let mut texts = vec![];
            collect_text(&out.shapes, &mut texts);
            assert!(
                texts.iter().any(|s| s == app.language.translate(key)),
                "actual rendered label {key}; {texts:?}"
            );
        }
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let parent = std::env::var_os("OPENARCHI_PRELIMINARY_OUTPUT")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::temp_dir().join(format!(
                        "oa-preliminary-{}",
                        storage::new_document_identity()
                    ))
                });
            std::fs::create_dir_all(&parent).unwrap();
            let root = parent.join(if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            });
            std::fs::create_dir(&root)
                .expect("inspect original outcome; do not replay into an existing run");
            let supplied = std::env::var_os("OPENARCHI_PRELIMINARY_INPUT").map(PathBuf::from);
            let before_bytes = supplied.as_ref().map(|p| std::fs::read(p).unwrap());
            let file = root.join("continued.openarchi.json");
            if let Some(source) = &supplied {
                let e = Editor::from_stored_document(source).unwrap();
                storage::save_document(&e.project, &file, e.project_identity()).unwrap();
            } else {
                let mut author =
                    crate::application_host::ApplicationHost::new(storage::synthetic_building_fixture());
                let editor = &author.document.editor;
                let sources = editor
                    .project
                    .elements
                    .iter()
                    .filter(|e| e.story == 0 && matches!(e.shape, Shape::Zone { .. }))
                    .take(2)
                    .map(|e| e.id)
                    .collect();
                let id = storage::new_document_identity();
                let change=service::prepare_create(editor,&id,service::Draft{name:"Saved preliminary task".into(),question:"Compare editable space arrangements; parameter height limit3.1m is an explicit design assumption".into(),statements:vec![dp::Statement{kind:dp::StatementKind::Assumption,text:"Height limit3.1m is authored for the test; no measured or statutory basis".into()}],candidate_names:["Scheme A".into(),"Scheme B".into()],source_ids:sources,entry:Entry::ManualInterface}).unwrap();
                author
                    .commit_prepared(author.context(), &id, &change)
                    .unwrap();
                let task = author
                    .document
                    .editor
                    .project
                    .design_process
                    .task()
                    .unwrap()
                    .clone();
                let element = task.candidates[0].initial_zone_ids[0];
                let change = service::prepare_edit(
                    &author.document.editor,
                    element,
                    Point::new(0., 0.),
                    Some(2.9),
                )
                .unwrap();
                author
                    .commit_prepared(author.context(), &storage::new_document_identity(), &change)
                    .unwrap();
                let d=qc::Definition{id:storage::new_document_identity(),title:"Authored height <=3.1m (assumption)".into(),domain:qc::Domain::Zones,scope:qc::Scope::Candidate{story:task.story,option:Some(task.candidates[0].option)},metric:qc::Metric::Height,relation:qc::Relation::AtMost,limit:Some(3.1),basis:qc::Basis::DesignAssumption{note:"Saved design intent for this controlled task, not a client/code requirement".into()}};
                let check = qc::Check {
                    definition: d.clone(),
                    report: None,
                };
                let change = author
                    .document
                    .editor
                    .prepare_change(
                        author.document.editor.revision,
                        vec![
                            ModelCommand::Constraint {
                                edit: qc::Edit::Put {
                                    expected_sha256: None,
                                    definition: d.clone(),
                                },
                            },
                            ModelCommand::Constraint {
                                edit: qc::Edit::Run {
                                    id: d.id,
                                    expected_sha256: qc::expected(&check),
                                },
                            },
                        ],
                    )
                    .unwrap();
                author
                    .commit_prepared(author.context(), &storage::new_document_identity(), &change)
                    .unwrap();
                storage::save_document(
                    &author.document.editor.project,
                    &file,
                    author.document.editor.project_identity(),
                )
                .unwrap();
            }
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| {
                s.animation_time = 0.;
                s.scroll_animation = egui::style::ScrollAnimation::none();
            });
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            open_test_project(&ctx, &mut app, &file);
            let baseline = app.host.document.editor.project.clone();
            let task = baseline.design_process.task().unwrap().clone();
            let candidate = &task.candidates[0];
            choose_candidate(&ctx, &mut app, candidate.option);
            let selected = if task.is_mass() {
                candidate.initial_mass_ids[0]
            } else {
                candidate.initial_zone_ids[0]
            };
            let definition=baseline.rules.checks.iter().find(|c|c.definition.metric==qc::Metric::Height&&matches!(c.definition.scope,qc::Scope::Candidate{option:Some(id),..}if id==candidate.option)).unwrap().definition.clone();
            stable_button(&ctx, &mut app, "Compare options…");
            assert!(app.process_ui.open);
            let comparison = task.compare(&app.host.document.editor.project).unwrap();
            assert!(comparison.different);
            assert!(
                !comparison.basis_current,
                "adding a saved check must retain the real changed basis"
            );
            let context = app.host.context();
            let selection = app.host.document.editor.selection_ids();
            stable_button(&ctx, &mut app, "Inspect candidate checks");
            assert!(!app.process_ui.open && app.checks_ui.open);
            assert_eq!(app.host.context(), context);
            assert_eq!(app.host.document.editor.selection_ids(), selection);
            // Select the checks window's real return button, not an occluded
            // identically labeled task-strip control behind this window.
            stable_button(&ctx, &mut app, "Compare options…");
            assert!(
                app.process_ui.open && !app.checks_ui.open,
                "compare={}, checks={}",
                app.process_ui.open,
                app.checks_ui.open
            );
            for _ in 0..4 {
                layers_frame_output(&ctx, &mut app, vec![]);
            }
            stable_button(&ctx, &mut app, "Checks bound to these candidates");
            assert_label(&ctx, &mut app, "No check is bound to this candidate");
            stable_button(&ctx, &mut app, "Checks bound to these candidates");
            // Scroll the real existing comparison surface, then choose its
            // option control and enter an explicit acceptance-test rationale.

            let chosen_id = ctx
                .data(|d| d.get_temp::<Id>(Id::new(("comparison_choice", candidate.option))))
                .unwrap();
            activate(
                &ctx,
                &mut app,
                chosen_id.accesskit_id(),
                egui::accesskit::Action::Click,
            );
            let reason_id = ctx
                .data(|d| d.get_temp::<Id>(Id::new("comparison_reason")))
                .unwrap();
            activate(
                &ctx,
                &mut app,
                reason_id.accesskit_id(),
                egui::accesskit::Action::Focus,
            );
            if !deferred_reason {
                layers_frame_output(
                    &ctx,
                    &mut app,
                    vec![
                        Event::Text(
                            "Controlled acceptance choice; preserve the other candidate".into(),
                        ),
                        key(Key::Tab, Modifiers::NONE),
                    ],
                );
            }
            stable_button(
                &ctx,
                &mut app,
                "I reviewed the changed basis for this choice",
            );
            layers_frame_output(&ctx, &mut app, vec![]);
            let reviewed = ctx
                .data(|d| d.get_temp::<(bool, bool, String)>(Id::new("comparison_review_state")))
                .unwrap();
            assert!(
                reviewed.0,
                "review checkbox did not toggle; busy={}, chosen={:?}, reason={}",
                reviewed.1, app.process_ui.chosen, app.process_ui.reason
            );
            stable_button(&ctx, &mut app, "Preview choice and saved versions");
            assert!(
                app.process_ui.preview.is_some(),
                "error={}, review={reviewed:?}, chosen={:?}, reason={}",
                app.process_ui.error,
                app.process_ui.chosen,
                app.process_ui.reason
            );
            stable_button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, baseline);
            stable_button(&ctx, &mut app, "Preview choice and saved versions");
            stable_button(&ctx, &mut app, "Confirm design change");
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .design_process
                    .decisions
                    .len(),
                baseline.design_process.decisions.len() + 1
            );
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .design_process
                    .decisions
                    .last()
                    .unwrap()
                    .reason
                    .is_empty(),
                deferred_reason
            );
            if app.process_ui.open {
                stable_button(&ctx, &mut app, "Continue in canvas");
            }
            if task.is_mass() {
                stable_button(&ctx, &mut app, "Select a source mass");
                let name = baseline
                    .elements
                    .iter()
                    .find(|e| e.id == selected)
                    .unwrap()
                    .name
                    .clone();
                stable_button(&ctx, &mut app, &format!("#{selected} · {name}"));
            } else {
                layers_frame_output(&ctx, &mut app, vec![]);
                let e = baseline.elements.iter().find(|e| e.id == selected).unwrap();
                let Shape::Zone { spec } = &e.shape else {
                    unreachable!()
                };
                let canvas = app.workbench.panel_rects["view.canvas"];
                let at = app.screen(canvas, spec.label_point());
                layers_click(&ctx, &mut app, at);
            }
            assert_eq!(app.host.document.editor.selection_ids(), vec![selected]);
            let old_height = match &baseline
                .elements
                .iter()
                .find(|e| e.id == selected)
                .unwrap()
                .shape
            {
                Shape::Zone { spec } => spec.height,
                Shape::ConceptMass { spec } => spec.height,
                _ => unreachable!(),
            };
            height(&ctx, &mut app, old_height + 0.2);
            let edited = app.host.document.editor.project.clone();
            assert_eq!(
                edited.plan_options.active_design_option,
                Some(candidate.option)
            );
            let changed = edited.elements.iter().find(|e| e.id == selected).unwrap();
            let actual_height = match &changed.shape {
                Shape::Zone { spec } => spec.height,
                Shape::ConceptMass { spec } => spec.height,
                _ => unreachable!(),
            };
            assert!((actual_height - old_height - 0.2).abs() < 1e-9);
            assert!(!app.process_ui.open);
            for old in &baseline.elements {
                let actual = edited.elements.iter().find(|e| e.id == old.id).unwrap();
                if old.id != selected {
                    assert_eq!(actual, old);
                }
            }
            let check = edited
                .rules
                .checks
                .iter()
                .find(|c| c.definition.id == definition.id)
                .unwrap();
            assert_eq!(check.freshness(&edited), "Check result is out of date");
            stable_button(&ctx, &mut app, "Undo");
            assert_eq!(app.host.document.editor.project.elements, baseline.elements);
            stable_button(&ctx, &mut app, "Redo");
            assert_eq!(app.host.document.editor.project.elements, edited.elements);
            stable_button(&ctx, &mut app, "Check current design");
            // Existing focus control is a visible check title; update only its
            // original definition/identity, never all checks as a side effect.
            assert_label(&ctx, &mut app, &definition.title);
            stable_button(&ctx, &mut app, "Preview check update");
            assert!(
                app.process_ui.preview.is_some(),
                "check preview missing; error={}; check_error={}",
                app.process_ui.error,
                app.checks_ui.error
            );
            assert_eq!(
                app.process_ui.preview.as_ref().unwrap().label,
                "Edit quantified checks"
            );
            stable_button(&ctx, &mut app, "Confirm design change");
            assert!(
                app.process_ui.preview.is_none(),
                "check confirmation unresolved: error={}, check_error={}",
                app.process_ui.error,
                app.checks_ui.error
            );
            let p = &app.host.document.editor.project;
            let refreshed = p
                .rules
                .checks
                .iter()
                .find(|c| c.definition.id == definition.id)
                .unwrap();
            assert_eq!(refreshed.freshness(p), "Check inputs unchanged");
            let figure_label = if app.current_constraint_figure_id().is_some() {
                "Show constraint figure"
            } else {
                "Preview constraint figure"
            };
            stable_button(&ctx, &mut app, figure_label);
            if app.process_ui.preview.is_some() {
                stable_button(&ctx, &mut app, "Confirm design change");
            }
            let figure_id = app.current_constraint_figure_id().unwrap();
            let needs_refresh = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .iter()
                .find(|f| f.id == figure_id)
                .unwrap()
                .status(&app.host.document.editor.project)
                == Status::Stale;
            if needs_refresh {
                stable_button(&ctx, &mut app, "Preview figure update");
                stable_button(&ctx, &mut app, "Confirm design change");
            }
            let figure = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .iter()
                .find(|f| Some(f.id) == app.current_constraint_figure_id())
                .unwrap();
            assert_eq!(
                figure.status(&app.host.document.editor.project),
                Status::Current
            );
            stable_button(&ctx, &mut app, "Save project");
            assert!(app.file_action.is_none(), "{}", app.file_error);
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let stored = Editor::from_stored_document(&file).unwrap();
            assert_eq!(
                stored.project.elements,
                app.host.document.editor.project.elements
            );
            assert_eq!(
                stored.project.design_process,
                app.host.document.editor.project.design_process
            );
            // Fresh host via the same normal product Open entry.
            let (fresh_ctx, mut fresh) = setup();
            fresh.language = language;
            fresh_ctx.enable_accesskit();
            open_test_project(&fresh_ctx, &mut fresh, &file);
            assert_eq!(fresh.host.document.editor.project, stored.project);
            if let Some(source) = supplied {
                assert_eq!(std::fs::read(source).unwrap(), before_bytes.unwrap());
            }
            std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":file,"task":task.id,"candidate":candidate.option,"changed_element":selected,"height_before":old_height,"height_after":old_height+0.2,"check":definition.id,"state":"related normal GUI event path pass; native not_run","other_original_elements_preserved":true,"real_AI":"not_run","native_package":"not_run"})).unwrap()).unwrap();
        }
    }
}
