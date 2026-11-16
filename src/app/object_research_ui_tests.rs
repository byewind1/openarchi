mod object_research_product_path {
    use super::*;
    use egui::accesskit::{Action, ActionData, ActionRequest, NodeId, TreeId};
    fn action(
        ctx: &Context,
        app: &mut AtelierApp,
        id: NodeId,
        action: Action,
        data: Option<ActionData>,
    ) {
        layers_frame_output(
            ctx,
            app,
            vec![Event::AccessKitActionRequest(ActionRequest {
                action,
                target_tree: TreeId::ROOT,
                target_node: id,
                data,
            })],
        );
        for _ in 0..3 {
            layers_frame_output(ctx, app, vec![]);
        }
    }
    fn button(ctx: &Context, app: &mut AtelierApp, label: &str) {
        reveal_manual_control(ctx, app, label);
        let out = layers_frame_output(ctx, app, vec![]);
        let tree = out.platform_output.accesskit_update.unwrap();
        let id = tree
            .nodes
            .iter()
            .find(|(_, n)| {
                n.supports_action(Action::Click)
                    && (n.label() == Some(app.language.translate(label))
                        || n.value() == Some(app.language.translate(label)))
            })
            .map(|(id, _)| *id)
            .unwrap_or_else(|| {
                panic!(
                    "missing {label}; {} / {}",
                    app.research_ui.error, app.process_ui.error
                )
            });
        action(ctx, app, id, Action::Click, None);
    }
    fn field(ctx: &Context, app: &mut AtelierApp, old: &str, value: &str) {
        let out = layers_frame_output(ctx, app, vec![]);
        let tree = out.platform_output.accesskit_update.unwrap();
        let id = tree
            .nodes
            .iter()
            .find(|(_, node)| node.value() == Some(old) && node.supports_action(Action::Focus))
            .map(|(id, _)| *id)
            .expect("path input by accessible value; long painted text may extend beyond its clip");
        action(ctx, app, id, Action::Focus, None);
        layers_frame_output(
            ctx,
            app,
            vec![key(Key::A, Modifiers::COMMAND), Event::Text(value.into())],
        );
    }
    fn open(ctx: &Context, app: &mut AtelierApp, file: &std::path::Path) {
        open_test_project(ctx, app, &file);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while app.source_ui.pending_for_ui() {
            assert!(std::time::Instant::now() < deadline);
            layers_frame_output(ctx, app, vec![]);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(app.source_ui.error.is_empty(), "{}", app.source_ui.error);
    }
    #[test]
    fn object_discussion_normal_entry_freezes_target_and_saves_bilingual() {
        path(false, false);
    }
    #[test]
    #[ignore = "explicit protected grain input/output required; never writes original"]
    fn grain_object_discussion_normal_entry_and_reopen_bilingual() {
        path(true, false);
    }
    #[test]
    #[ignore = "explicit protected grain input/output required; never writes original"]
    fn grain_parametric_normal_entry_and_reopen_bilingual() {
        path(true, true);
    }
    fn path(supplied: bool, parametric: bool) {
        let parent = if supplied {
            PathBuf::from(std::env::var_os("OPENARCHI_OBJECT_OUTPUT").expect("new output"))
        } else {
            std::env::temp_dir().join(format!("oa-object-ui-{}", storage::new_document_identity()))
        };
        std::fs::create_dir(&parent).expect("inspect existing outcome; do not replay");
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let root = parent.join(if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            });
            std::fs::create_dir(&root).unwrap();
            let file = root.join("discussion.openarchi.json");
            let start = if supplied {
                Editor::from_stored_document(std::path::Path::new(
                    &std::env::var_os("OPENARCHI_OBJECT_INPUT").expect("input"),
                ))
                .unwrap()
            } else {
                let mut p = Project::default();
                p.add(
                    0,
                    Material::Plaster,
                    Shape::ConceptMass {
                        spec: crate::concept_mass::ConceptMassSpec {
                            arcs: vec![],
                            hole_arcs: vec![],
                            outline: vec![
                                Point::new(-4., 1.),
                                Point::new(-1., 1.),
                                Point::new(-1., 3.),
                                Point::new(-4., 3.),
                            ],
                            holes: vec![],
                            height: 3.,
                            bottom_offset: 0.,
                        },
                    },
                );
                Editor::new(p)
            };
            storage::save_document(&start.project, &file, start.project_identity()).unwrap();
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            open(&ctx, &mut app, &file);
            let initial = app.host.document.editor.project.clone();
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::F2, Modifiers::NONE), key(Key::F, Modifiers::NONE)],
            );
            let active = app
                .host
                .document
                .editor
                .project
                .plan_options
                .active_design_option;
            let mass = app
                .host
                .document
                .editor
                .project
                .elements
                .iter()
                .find(|e| e.design_option == active && matches!(e.shape, Shape::ConceptMass { .. }))
                .unwrap()
                .clone();
            if initial.design_process.task().is_some() {
                button(&ctx, &mut app, "Select a source mass");
                button(&ctx, &mut app, &format!("#{} · {}", mass.id, mass.name));
            } else {
                let Shape::ConceptMass { spec } = &mass.shape else {
                    unreachable!()
                };
                let at = Point::new(
                    (spec.outline[0].x + spec.outline[2].x) / 2.,
                    (spec.outline[0].y + spec.outline[2].y) / 2.,
                );
                let canvas = app.workbench.panel_rects["view.canvas"];
                let at = app.screen(canvas, at);
                layers_click(&ctx, &mut app, at);
            }
            assert_eq!(app.host.document.editor.selection_ids(), vec![mass.id]);
            button(&ctx, &mut app, "Discuss selected objects");
            assert!(app.tab == Tab::Assistant);
            let frozen = app
                .research_ui
                .draft
                .as_ref()
                .unwrap()
                .exchange
                .objects
                .clone();
            let input = ctx
                .data(|d| d.get_temp::<Id>(Id::new("object_research_question")))
                .unwrap();
            action(&ctx, &mut app, input.accesskit_id(), Action::Focus, None);
            layers_frame_output(&ctx,&mut app,vec![Event::Text("How does this wing relate to shared teaching? Keep unconfirmed capacity unknown.".into())]);
            if supplied {
                button(&ctx, &mut app, "Add an existing requirement row");
                let sheet = app
                    .host
                    .document
                    .editor
                    .project
                    .program
                    .sheets
                    .iter()
                    .find(|s| s.id == "original-taskbook-functions")
                    .unwrap();
                let label = format!("{} · {}", sheet.name, sheet.rows[0].values.label);
                button(&ctx, &mut app, &label);
                assert_eq!(
                    app.research_ui
                        .draft
                        .as_ref()
                        .unwrap()
                        .exchange
                        .objects
                        .len(),
                    2
                );
                assert!(
                    !app.research_ui
                        .draft
                        .as_ref()
                        .unwrap()
                        .exchange
                        .sources
                        .is_empty()
                );
            }
            assert_eq!(
                app.research_ui.draft.as_ref().unwrap().exchange.objects[0],
                frozen[0]
            );
            if parametric {
                parameter_path(&ctx, &mut app, &root, &file, &initial);
                continue;
            }
            button(&ctx, &mut app, "Preview discussion record");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            assert_eq!(app.host.document.editor.project, initial);
            button(&ctx, &mut app, "Cancel design proposal");
            assert!(app.host.document.editor.project.evidence.topics.is_empty());
            button(&ctx, &mut app, "Preview discussion record");
            button(&ctx, &mut app, "Confirm design change");
            assert_eq!(app.host.document.editor.project.elements, initial.elements);
            assert_eq!(
                app.host.document.editor.project.evidence.topics.len(),
                1,
                "{}",
                app.process_ui.error
            );
            layers_frame_output(&ctx, &mut app, vec![key(Key::S, Modifiers::COMMAND)]);
            if app.file_action.is_some() {
                button(&ctx, &mut app, "Save");
            }
            assert_eq!(
                app.host.document.editor.status,
                language.translate("Project saved")
            );
            let read = Editor::from_stored_document(&file).unwrap();
            assert_eq!(
                read.project.evidence.topics,
                app.host.document.editor.project.evidence.topics
            );
            assert_eq!(read.project.elements, initial.elements);
            let (fresh_ctx, mut fresh) = setup();
            fresh_ctx.enable_accesskit();
            fresh.language = language;
            fresh
                .host
                .configure_operation_journal(Some(root.join("reopen-journal")))
                .unwrap();
            open(&fresh_ctx, &mut fresh, &file);
            assert_eq!(
                fresh.host.document.editor.project.evidence.topics,
                read.project.evidence.topics
            );
            let report = serde_json::json!({"input":if supplied{std::env::var("OPENARCHI_OBJECT_INPUT").ok()}else{None},"output":file,"objects":frozen.iter().map(|o|&o.link).collect::<Vec<_>>(),"topic":read.project.evidence.topics[0].id,"models_unchanged":true,"normal_events":true,"external_agent":"not_connected","native_os":"not_run"});
            std::fs::write(
                root.join("result.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
        }
    }
    fn number(ctx: &Context, app: &mut AtelierApp, index: usize, label: &str, value: f64) {
        let id = ctx
            .data(|d| d.get_temp::<Id>(Id::new(("parametric", index, label))))
            .expect("visible numeric parameter");
        action(
            ctx,
            app,
            id.accesskit_id(),
            Action::SetValue,
            Some(ActionData::NumericValue(value)),
        );
    }
    #[test]
    fn bound_object_question_updates_existing_rule_atomically_bilingual() {
        bound_question_path(false);
    }
    #[test]
    #[ignore = "explicit protected saved project and unused output required"]
    fn continued_bound_object_question_normal_bilingual() {
        bound_question_path(true);
    }
    fn bound_question_path(supplied: bool) {
        use crate::design_process::parametric as pm;
        let parent = if supplied {
            PathBuf::from(std::env::var_os("OPENARCHI_BOUND_QUESTION_OUTPUT").unwrap())
        } else {
            std::env::temp_dir().join(format!(
                "oa-bound-question-{}",
                storage::new_document_identity()
            ))
        };
        std::fs::create_dir(&parent).expect("inspect previous result; never overwrite");
        let input = std::env::var_os("OPENARCHI_BOUND_QUESTION_INPUT").map(PathBuf::from);
        let original = input
            .as_ref()
            .filter(|_| supplied)
            .map(|p| std::fs::read(p).unwrap());
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let root = parent.join(if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            });
            std::fs::create_dir(&root).unwrap();
            let start = if supplied {
                Editor::from_stored_document(input.as_ref().unwrap()).unwrap()
            } else {
                let mut e = Editor::new(Project::default());
                let a = pm::Parameters {
                    origin_mm: [-4000, -4000],
                    court_width_mm: 10000,
                    court_depth_mm: 8000,
                    wing_depth_mm: 4000,
                    storeys: 2,
                    floor_height_mm: 3300,
                };
                let mut b = a.clone();
                b.court_width_mm = 9000;
                let definition = pm::Definition {
                    rule_version: 1,
                    candidates: [a, b],
                    site_sha256: crate::automation_journal::canonical_hash(&e.project.site)
                        .unwrap(),
                    sources: vec![],
                    assumptions: "Public authored concept fixture; no survey or AI claim".into(),
                };
                let p = pm::prepare(
                    &e,
                    &storage::new_document_identity(),
                    "Existing rule".into(),
                    "Explore courtyard depth".into(),
                    definition,
                    0,
                    0,
                )
                .unwrap();
                e.commit_change("public-fixture", &p).unwrap();
                e
            };
            let file = root.join("question-update.openarchi.json");
            storage::save_document(&start.project, &file, start.project_identity()).unwrap();
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            open(&ctx, &mut app, &file);
            let before = app.host.document.editor.project.clone();
            let task = before.design_process.task().unwrap().clone();
            let option = before.plan_options.active_design_option.unwrap();
            let old = pm::current_parameters(&before, &task.id, option).unwrap();
            let member = task
                .candidates
                .iter()
                .find(|c| c.option == option)
                .unwrap()
                .initial_mass_ids[0];
            let mass = before.elements.iter().find(|e| e.id == member).unwrap();
            button(&ctx, &mut app, "Select a source mass");
            button(&ctx, &mut app, &format!("#{} · {}", mass.id, mass.name));
            button(&ctx, &mut app, "Discuss selected objects");
            let question = "QA: deepen only these bound wings by 200 mm; preserve courtyard, height, B and retained work. Approval unknown.";
            let field = ctx
                .data(|d| d.get_temp::<Id>(Id::new("object_research_question")))
                .unwrap();
            action(&ctx, &mut app, field.accesskit_id(), Action::Focus, None);
            layers_frame_output(&ctx, &mut app, vec![Event::Text(question.into())]);
            let snapshot = app.research_ui.draft.as_ref().unwrap().exchange.clone();
            button(&ctx, &mut app, "Adjust bound wings from this question");
            assert_eq!(
                app.research_ui.parameters.as_ref().unwrap().task.as_ref(),
                Some(&task.id)
            );
            assert_eq!(
                app.research_ui
                    .parameters
                    .as_ref()
                    .unwrap()
                    .definition
                    .candidates[0],
                old
            );
            number(
                &ctx,
                &mut app,
                0,
                "Wing depth (mm)",
                (old.wing_depth_mm + 200) as f64,
            );
            // Re-entering must not silently replace an in-progress parameter draft.
            let draft_id = app.research_ui.parameters.as_ref().unwrap().id.clone();
            button(&ctx, &mut app, "Adjust bound wings from this question");
            assert!(app.research_ui.error.contains("Finish or cancel"));
            assert_eq!(app.research_ui.parameters.as_ref().unwrap().id, draft_id);
            assert_eq!(
                app.research_ui
                    .parameters
                    .as_ref()
                    .unwrap()
                    .definition
                    .candidates[0]
                    .wing_depth_mm,
                old.wing_depth_mm + 200
            );
            button(&ctx, &mut app, "Preview two-wing parameters");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            let out = layers_frame_output(&ctx, &mut app, vec![]);
            assert!(
                text_center(&out.shapes, question).is_some(),
                "current question must appear in confirmation"
            );
            assert_eq!(app.host.document.editor.project, before);
            button(&ctx, &mut app, "Return to design editing");
            assert_eq!(app.research_ui.draft.as_ref().unwrap().exchange, snapshot);
            button(&ctx, &mut app, "Preview two-wing parameters");
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, before);
            button(&ctx, &mut app, "Preview two-wing parameters");
            button(&ctx, &mut app, "Confirm design change");
            let after = app.host.document.editor.project.clone();
            assert_eq!(
                after.design_process.tasks.len(),
                before.design_process.tasks.len()
            );
            assert_eq!(
                after.evidence.topics.len(),
                before.evidence.topics.len() + 1
            );
            assert_eq!(
                after
                    .evidence
                    .topics
                    .last()
                    .unwrap()
                    .exchanges
                    .last()
                    .unwrap(),
                &snapshot
            );
            assert_eq!(
                after.design_process.updates.last().unwrap().instruction,
                question
            );
            assert_eq!(
                pm::current_parameters(&after, &task.id, option)
                    .unwrap()
                    .wing_depth_mm,
                old.wing_depth_mm + 200
            );
            assert!(app.research_ui.parameters.is_none() && app.research_ui.draft.is_none());
            for element in &before.elements {
                if element.design_option != Some(option) {
                    assert_eq!(
                        after.elements.iter().find(|e| e.id == element.id),
                        Some(element)
                    );
                }
            }
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            let mut undone = before.clone();
            // Undo restores live data while the existing immutable task audit is retained.
            undone.design_process = crate::design_process::Journal::restore_with_history(
                &before.design_process,
                &after.design_process,
            );
            assert_eq!(
                app.host.document.editor.project, undone,
                "question and geometry undo together; immutable task audit remains"
            );
            assert_eq!(
                pm::current_parameters(&app.host.document.editor.project, &task.id, option)
                    .unwrap(),
                old
            );
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(
                    Key::Z,
                    Modifiers {
                        command: true,
                        mac_cmd: true,
                        shift: true,
                        ..Modifiers::NONE
                    },
                )],
            );
            assert_eq!(app.host.document.editor.project, after);
            layers_frame_output(&ctx, &mut app, vec![key(Key::S, Modifiers::COMMAND)]);
            if app.file_action.is_some() {
                button(&ctx, &mut app, "Save");
            }
            assert_eq!(
                app.host.document.editor.status,
                language.translate("Project saved")
            );
            let saved = Editor::from_stored_document(&file).unwrap();
            assert_eq!(saved.project.evidence, after.evidence);
            assert_eq!(saved.project.design_process, after.design_process);
            assert_eq!(saved.project.elements, after.elements);
            let (fresh_ctx, mut fresh) = setup();
            fresh_ctx.enable_accesskit();
            fresh.language = language;
            open(&fresh_ctx, &mut fresh, &file);
            assert_eq!(
                fresh.host.document.editor.project.design_process,
                after.design_process
            );
            assert_eq!(fresh.host.document.editor.project.evidence, after.evidence);
            std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"entry":"normal product egui events","file":file,"task":task.id,"option":option,"depth_before_mm":old.wing_depth_mm,"depth_after_mm":old.wing_depth_mm+200,"new_tasks":0,"question_and_geometry_one_undo":true,"source_reading":"not_run","native_os":"not_run","real_agent":"not_run"})).unwrap()).unwrap();
        }
        if let Some(original) = original {
            assert_eq!(std::fs::read(input.unwrap()).unwrap(), original);
        }
    }
    fn parameter_path(
        ctx: &Context,
        app: &mut AtelierApp,
        root: &std::path::Path,
        file: &std::path::Path,
        initial: &Project,
    ) {
        button(ctx, app, "Explore two wings from this question");
        assert!(
            app.research_ui.parameters.is_some(),
            "{}",
            app.research_ui.error
        );
        for i in 0..2 {
            for (label, value) in [
                ("Origin X (mm)", 54576500.),
                ("Origin Y (mm)", 50415000.),
                ("Court width (mm)", if i == 0 { 41500. } else { 39500. }),
                ("Court depth (mm)", if i == 0 { 31000. } else { 29000. }),
            ] {
                number(ctx, app, i, label, value);
            }
        }
        button(ctx, app, "Preview two-wing parameters");
        assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
        assert_eq!(&app.host.document.editor.project, initial);
        button(ctx, app, "Cancel design proposal");
        assert_eq!(&app.host.document.editor.project, initial);
        button(ctx, app, "Preview two-wing parameters");
        button(ctx, app, "Confirm design change");
        let p = &app.host.document.editor.project;
        assert_eq!(
            &p.elements[..initial.elements.len()],
            initial.elements.as_slice()
        );
        let task = p.design_process.task().unwrap().clone();
        assert_ne!(
            task.id,
            initial.design_process.task().unwrap().id,
            "{}",
            app.process_ui.error
        );
        let c = task.compare(p).unwrap();
        assert!(
            (c.candidates[0]
                .mass_metrics
                .as_ref()
                .unwrap()
                .footprint_sum_m2
                - 471.)
                .abs()
                < 1e-7
        );
        button(ctx, app, "Adjust generated wings");
        number(ctx, app, 0, "Wing depth (mm)", 8000.);
        button(ctx, app, "Preview two-wing parameters");
        button(ctx, app, "Confirm design change");
        let updated = app.host.document.editor.project.clone();
        assert!(
            (task.compare(&updated).unwrap().candidates[0]
                .mass_metrics
                .as_ref()
                .unwrap()
                .footprint_sum_m2
                - 644.)
                .abs()
                < 1e-7,
            "{}",
            app.process_ui.error
        );
        assert_eq!(
            updated
                .design_process
                .updates
                .last()
                .unwrap()
                .parametric_parameters
                .as_ref()
                .unwrap()
                .wing_depth_mm,
            8000
        );
        layers_frame_output(ctx, app, vec![key(Key::Z, Modifiers::COMMAND)]);
        assert!(
            (task
                .compare(&app.host.document.editor.project)
                .unwrap()
                .candidates[0]
                .mass_metrics
                .as_ref()
                .unwrap()
                .footprint_sum_m2
                - 471.)
                .abs()
                < 1e-7
        );
        layers_frame_output(
            ctx,
            app,
            vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
        );
        assert_eq!(app.host.document.editor.project.elements, updated.elements);
        layers_frame_output(ctx, app, vec![key(Key::S, Modifiers::COMMAND)]);
        if app.file_action.is_some() {
            button(ctx, app, "Save");
        }
        assert_eq!(
            app.host.document.editor.status,
            app.language.translate("Project saved")
        );
        let read = Editor::from_stored_document(file).unwrap();
        assert_eq!(read.project, app.host.document.editor.project);
        let (fresh_ctx, mut fresh) = setup();
        fresh_ctx.enable_accesskit();
        fresh.language = app.language;
        fresh
            .host
            .configure_operation_journal(Some(root.join("reopen-journal")))
            .unwrap();
        open(&fresh_ctx, &mut fresh, file);
        button(&fresh_ctx, &mut fresh, "AI Assistant");
        button(&fresh_ctx, &mut fresh, "Adjust generated wings");
        number(&fresh_ctx, &mut fresh, 0, "Concept storeys", 3.);
        button(&fresh_ctx, &mut fresh, "Preview two-wing parameters");
        button(&fresh_ctx, &mut fresh, "Confirm design change");
        assert!(
            (task
                .compare(&fresh.host.document.editor.project)
                .unwrap()
                .candidates[0]
                .mass_metrics
                .as_ref()
                .unwrap()
                .envelope_volume_sum_m3
                - 6375.6)
                .abs()
                < 1e-6,
            "{}",
            fresh.process_ui.error
        );
        assert_eq!(
            &fresh.host.document.editor.project.elements[..initial.elements.len()],
            initial.elements.as_slice()
        );
        layers_frame_output(
            &fresh_ctx,
            &mut fresh,
            vec![key(Key::S, Modifiers::COMMAND)],
        );
        if fresh.file_action.is_some() {
            button(&fresh_ctx, &mut fresh, "Save");
        }
        let saved = Editor::from_stored_document(file).unwrap();
        assert_eq!(saved.project, fresh.host.document.editor.project);
        std::fs::write(root.join("parametric-result.json"),serde_json::to_vec_pretty(&serde_json::json!({"entry":"normal egui/accesskit events","file":file,"task":task.id,"original_elements_preserved":initial.elements.len(),"footprint_before_m2":471,"footprint_after_m2":644,"volume_after_reopen_change_m3":6375.6,"native_os":"not_run","ai_execution":"not_run","canvas_parameter_drag":"not_run"})).unwrap()).unwrap();
    }
    fn wait_sources(ctx: &Context, app: &mut AtelierApp) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            layers_frame_output(ctx, app, vec![]);
            if !app.source_ui.pending_for_ui() {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "source task pending");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(app.source_ui.error.is_empty(), "{}", app.source_ui.error);
    }
    fn check_field(ctx: &Context, app: &mut AtelierApp, label: &str, text: &str) {
        let rect = ctx
            .data(|d| d.get_temp::<Rect>(Id::new(("check_field", label))))
            .unwrap();
        layers_click(ctx, app, rect.center());
        layers_frame_output(
            ctx,
            app,
            vec![
                key(Key::A, Modifiers::COMMAND),
                Event::Text(text.into()),
                key(Key::Tab, Modifiers::NONE),
            ],
        );
    }
    #[test]
    #[ignore = "explicit latest parametric input/source repository and unused output required"]
    fn continued_parametric_check_figure_update_normal_bilingual() {
        use crate::{constraints as qc, diagrams::Status};
        let parent =
            PathBuf::from(std::env::var_os("OPENARCHI_PARAM_CHECK_OUTPUT").expect("new output"));
        std::fs::create_dir(&parent).expect("inspect previous outcome, never replay");
        let input = PathBuf::from(
            std::env::var_os("OPENARCHI_PARAM_CHECK_INPUT")
                .expect("latest saved parametric project"),
        );
        let source = PathBuf::from(
            std::env::var_os("OPENARCHI_PARAM_CHECK_REPOSITORY").expect("bound source repository"),
        );
        let original_bytes = std::fs::read(&input).unwrap();
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let root = parent.join(if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            });
            std::fs::create_dir(&root).unwrap();
            let start = Editor::from_stored_document(&input).unwrap();
            let repository = crate::project_repository::Repository::open(&source)
                .unwrap()
                .copy_to(&root.join("repository"))
                .unwrap();
            let file = root.join("continued.openarchi.json");
            storage::save_document(&start.project, &file, start.project_identity()).unwrap();
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            app.initialize_source_connections(&root);
            crate::project_repository::connection::remember(
                app.source_ui.connections_directory.as_ref().unwrap(),
                &file,
                start.project_identity(),
                &storage::FileVersion::read(&file).unwrap().token().unwrap(),
                &repository,
            )
            .unwrap();
            open(&ctx, &mut app, &file);
            wait_sources(&ctx, &mut app);
            let initial = app.host.document.editor.project.clone();
            let task = initial.design_process.task().unwrap().clone();
            let option = initial.plan_options.active_design_option.unwrap();
            assert!(task.intent.as_ref().unwrap().parametric.is_some());
            assert!(app.host.document.editor.selection_ids().is_empty());
            button(&ctx, &mut app, "Check current design");
            button(
                &ctx,
                &mut app,
                "Add check for current selection or candidate",
            );
            check_field(
                &ctx,
                &mut app,
                "Check name",
                "Current double-wing containment",
            );
            let metric = ctx
                .data(|d| d.get_temp::<Rect>(Id::new("check_metric_control")))
                .unwrap();
            layers_click(&ctx, &mut app, metric.center());
            button(&ctx, &mut app, "Clearance within declared control region");
            check_field(&ctx, &mut app, "Limit (blank means unknown)", "0");
            check_field(
                &ctx,
                &mut app,
                "Optional assumption note",
                "Geometric containment only. Survey, approval, use and statutory setbacks remain unknown.",
            );
            button(&ctx, &mut app, "Preview quantified check");
            wait_sources(&ctx, &mut app);
            assert!(
                app.process_ui.preview.is_some(),
                "{} / {}",
                app.process_ui.error,
                app.checks_ui.error
            );
            assert_eq!(app.host.document.editor.project, initial);
            button(&ctx, &mut app, "Cancel design proposal");
            button(&ctx, &mut app, "Preview quantified check");
            wait_sources(&ctx, &mut app);
            button(&ctx, &mut app, "Confirm design change");
            wait_sources(&ctx, &mut app);
            let check = app
                .host
                .document
                .editor
                .project
                .rules
                .checks
                .last()
                .unwrap()
                .clone();
            assert!(
                matches!(check.definition.scope,qc::Scope::Candidate{story:0,option:Some(id)} if id==option)
            );
            assert_eq!(
                check.state(&app.host.document.editor.project),
                qc::State::Pass,
                "{:?}",
                check.report
            );
            let before_value = check
                .report
                .as_ref()
                .unwrap()
                .rows
                .iter()
                .filter_map(|r| r.value)
                .fold(f64::INFINITY, f64::min);
            button(&ctx, &mut app, "Compare options…");
            let chosen = ctx
                .data(|d| d.get_temp::<Id>(Id::new(("comparison_choice", option))))
                .unwrap();
            action(&ctx, &mut app, chosen.accesskit_id(), Action::Click, None);
            button(
                &ctx,
                &mut app,
                "I reviewed the changed basis for this choice",
            );
            button(&ctx, &mut app, "Preview choice and saved versions");
            button(&ctx, &mut app, "Confirm design change");
            button(&ctx, &mut app, "Continue in canvas");
            button(&ctx, &mut app, "Check current design");
            button(&ctx, &mut app, "Preview constraint figure");
            button(&ctx, &mut app, "Confirm design change");
            let figure_id = app.current_constraint_figure_id().unwrap();
            let before_edit = app.host.document.editor.project.clone();
            let figure = before_edit
                .drawings
                .diagrams
                .iter()
                .find(|f| f.id == figure_id)
                .unwrap()
                .clone();
            assert_eq!(figure.status(&before_edit), Status::Current);
            button(&ctx, &mut app, "Edit source model");
            button(&ctx, &mut app, "AI Assistant");
            button(&ctx, &mut app, "Adjust generated wings");
            let previous_parameters = app
                .research_ui
                .parameters
                .as_ref()
                .unwrap()
                .definition
                .candidates[0]
                .clone();
            number(
                &ctx,
                &mut app,
                0,
                "Origin X (mm)",
                (previous_parameters.origin_mm[0] + 1000) as f64,
            );
            button(&ctx, &mut app, "Preview two-wing parameters");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(
                app.host.document.editor.project.elements,
                before_edit.elements
            );
            button(&ctx, &mut app, "Preview two-wing parameters");
            button(&ctx, &mut app, "Confirm design change");
            let changed = app.host.document.editor.project.clone();
            for old in &initial.elements {
                if !task.candidates[0].initial_mass_ids.contains(&old.id) {
                    assert_eq!(changed.elements.iter().find(|e| e.id == old.id), Some(old));
                }
            }
            assert_eq!(
                changed
                    .rules
                    .checks
                    .iter()
                    .find(|c| c.definition.id == check.definition.id)
                    .unwrap()
                    .state(&changed),
                qc::State::Unknown
            );
            assert_eq!(figure.status(&changed), Status::Stale);
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(
                app.host.document.editor.project.elements,
                before_edit.elements
            );
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            assert_eq!(app.host.document.editor.project.elements, changed.elements);
            button(&ctx, &mut app, "Check current design");
            button(&ctx, &mut app, "Preview check update");
            wait_sources(&ctx, &mut app);
            button(&ctx, &mut app, "Confirm design change");
            wait_sources(&ctx, &mut app);
            let updated = app
                .host
                .document
                .editor
                .project
                .rules
                .checks
                .iter()
                .find(|c| c.definition.id == check.definition.id)
                .unwrap()
                .clone();
            assert_eq!(
                updated.report.as_ref().unwrap().generation,
                check.report.as_ref().unwrap().generation + 1
            );
            let after_value = updated
                .report
                .as_ref()
                .unwrap()
                .rows
                .iter()
                .filter_map(|r| r.value)
                .fold(f64::INFINITY, f64::min);
            assert!((after_value - before_value).abs() > 0.1);
            button(&ctx, &mut app, "Show constraint figure");
            button(&ctx, &mut app, "Preview figure update");
            button(&ctx, &mut app, "Confirm design change");
            let final_project = app.host.document.editor.project.clone();
            let f = final_project
                .drawings
                .diagrams
                .iter()
                .find(|f| f.id == figure_id)
                .unwrap();
            assert_eq!(f.generation, figure.generation + 1);
            assert_eq!(f.status(&final_project), Status::Current);
            button(&ctx, &mut app, "Edit source model");
            button(&ctx, &mut app, "Save project");
            button(&ctx, &mut app, "Save");
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let saved = Editor::from_stored_document(&file).unwrap();
            assert_eq!(saved.project, final_project);
            let (fresh_ctx, mut fresh) = setup();
            fresh_ctx.enable_accesskit();
            fresh.language = language;
            fresh
                .host
                .configure_operation_journal(Some(root.join("reopen-journal")))
                .unwrap();
            fresh.initialize_source_connections(&root);
            open(&fresh_ctx, &mut fresh, &file);
            wait_sources(&fresh_ctx, &mut fresh);
            assert_eq!(fresh.host.document.editor.project, final_project);
            button(&fresh_ctx, &mut fresh, "AI Assistant");
            button(&fresh_ctx, &mut fresh, "Adjust generated wings");
            assert_eq!(
                fresh
                    .research_ui
                    .parameters
                    .as_ref()
                    .unwrap()
                    .definition
                    .candidates[0]
                    .origin_mm[0],
                previous_parameters.origin_mm[0] + 1000
            );
            assert_eq!(std::fs::read(&input).unwrap(), original_bytes);
            std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"output":file,"task":task.id,"option":option,"check":check.definition.id,"check_before_m":before_value,"check_after_m":after_value,"figure":figure_id,"model":"only mapped A members translated +1m X; B and original objects exact","normal_events":"bilingual save/reopen; not native OS","ai":"not_run"})).unwrap()).unwrap();
        }
    }
    fn pointer(ctx: &Context, app: &mut AtelierApp, at: Pos2, down: bool) -> FullOutput {
        layers_frame_output(
            ctx,
            app,
            vec![
                Event::PointerMoved(at),
                Event::PointerButton {
                    pos: at,
                    button: PointerButton::Primary,
                    pressed: down,
                    modifiers: Modifiers::NONE,
                },
            ],
        )
    }
    fn canvas_handle(ctx: &Context, app: &mut AtelierApp) -> Pos2 {
        button(ctx, app, "Move generated wings on canvas");
        assert!(
            app.research_ui.canvas_move.is_some(),
            "{}",
            app.research_ui.error
        );
        ctx.data(|d| d.get_temp::<Pos2>(Id::new("parametric_canvas_handle")))
            .expect("visible origin handle")
    }
    #[test]
    fn parametric_canvas_move_events_preview_cancel_focus_undo_save_bilingual() {
        canvas_feedback(false, false, false);
    }
    #[test]
    fn parametric_canvas_move_coalesced_press_first_move_bilingual() {
        canvas_feedback(false, true, false);
    }
    #[test]
    fn parametric_canvas_move_complete_gesture_one_frame_bilingual() {
        canvas_feedback(false, true, true);
    }
    #[test]
    #[ignore = "latest saved parametric/check/figure project + repository and unused output required"]
    fn continued_parametric_canvas_check_figure_normal_bilingual() {
        canvas_feedback(true, false, false);
    }
    #[test]
    #[ignore = "latest saved parametric/check/figure project + repository and unused output required"]
    fn continued_parametric_canvas_coalesced_press_bilingual() {
        canvas_feedback(true, true, false);
    }
    #[test]
    #[ignore = "latest saved parametric/check/figure project + repository and unused output required"]
    fn continued_parametric_canvas_complete_gesture_bilingual() {
        canvas_feedback(true, true, true);
    }
    fn canvas_feedback(supplied: bool, coalesced_press: bool, complete_gesture: bool) {
        use crate::{constraints as qc, design_process::parametric as pm, diagrams::Status};
        let parent = if supplied {
            PathBuf::from(std::env::var_os("OPENARCHI_PARAM_CANVAS_OUTPUT").expect("new output"))
        } else {
            std::env::temp_dir().join(format!(
                "oa-canvas-rule-{}",
                storage::new_document_identity()
            ))
        };
        std::fs::create_dir(&parent).expect("inspect original outcome before reuse");
        let input = std::env::var_os("OPENARCHI_PARAM_CANVAS_INPUT").map(PathBuf::from);
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let root = parent.join(if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            });
            std::fs::create_dir(&root).unwrap();
            let start = if supplied {
                Editor::from_stored_document(input.as_ref().unwrap()).unwrap()
            } else {
                let mut fixture = Project::default();
                fixture.plan_options.orientation_degrees = if language == UiLanguage::English {
                    37.
                } else {
                    -23.
                };
                let mut editor = Editor::new(fixture);
                let a = pm::Parameters {
                    origin_mm: [-4000, -4000],
                    court_width_mm: 10000,
                    court_depth_mm: 8000,
                    wing_depth_mm: 4000,
                    storeys: 2,
                    floor_height_mm: 3300,
                };
                let mut b = a.clone();
                b.court_width_mm = 9000;
                let rule = pm::Definition {
                    rule_version: 1,
                    candidates: [a, b],
                    site_sha256: crate::automation_journal::canonical_hash(&editor.project.site)
                        .unwrap(),
                    sources: vec![],
                    assumptions: "Authored public fixture, no real survey or Agent claim".into(),
                };
                let prepared = pm::prepare(
                    &editor,
                    &storage::new_document_identity(),
                    "Public rule fixture".into(),
                    "Test shared canvas translation without altering other members".into(),
                    rule,
                    0,
                    0,
                )
                .unwrap();
                editor.commit_change("fixture-create", &prepared).unwrap();
                editor
            };
            let file = root.join("canvas-continued.openarchi.json");
            storage::save_document(&start.project, &file, start.project_identity()).unwrap();
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            if supplied {
                let source =
                    PathBuf::from(std::env::var_os("OPENARCHI_PARAM_CANVAS_REPOSITORY").unwrap());
                let repo = crate::project_repository::Repository::open(&source)
                    .unwrap()
                    .copy_to(&root.join("repository"))
                    .unwrap();
                app.initialize_source_connections(&root);
                crate::project_repository::connection::remember(
                    app.source_ui.connections_directory.as_ref().unwrap(),
                    &file,
                    start.project_identity(),
                    &storage::FileVersion::read(&file).unwrap().token().unwrap(),
                    &repo,
                )
                .unwrap();
            }
            open(&ctx, &mut app, &file);
            wait_sources(&ctx, &mut app);
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::F2, Modifiers::NONE), key(Key::F, Modifiers::NONE)],
            );
            button(&ctx, &mut app, "AI Assistant");
            let before = app.host.document.editor.project.clone();
            let task = before.design_process.task().unwrap().clone();
            let option = before.plan_options.active_design_option.unwrap();
            let parameters = pm::current_parameters(&before, &task.id, option).unwrap();
            let history = app.host.document.editor.undo.len();
            let check=before.rules.checks.iter().find(|c|matches!(c.definition.scope,qc::Scope::Candidate{option:Some(id),..} if id==option)).cloned();
            let figure = before
                .drawings
                .diagrams
                .iter()
                .find(|f| {
                    f.definition.option == Some(option)
                        && matches!(
                            f.definition.presentation,
                            crate::diagrams::Presentation::ConstraintOverlay
                        )
                })
                .cloned();
            let at = canvas_handle(&ctx, &mut app);
            let hover = layers_frame_output(&ctx, &mut app, vec![Event::PointerMoved(at)]);
            assert_eq!(hover.platform_output.cursor_icon, CursorIcon::Move);
            pointer(&ctx, &mut app, at, true);
            let to = at + vec2(30., 0.);
            let dragging = layers_frame_output(&ctx, &mut app, vec![Event::PointerMoved(to)]);
            assert_eq!(dragging.platform_output.cursor_icon, CursorIcon::Grabbing);
            assert_eq!(app.host.document.editor.project, before);
            assert!(app.workbench_pointer_active());
            layers_frame_output(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
            pointer(&ctx, &mut app, to, false);
            assert!(app.research_ui.canvas_move.is_none());
            assert_eq!(app.host.document.editor.project, before);
            assert_eq!(app.host.document.editor.undo.len(), history);
            let at = canvas_handle(&ctx, &mut app);
            pointer(&ctx, &mut app, at, true);
            let mut lost_focus = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
                    focused: false,
                    events: vec![
                        Event::PointerMoved(at + vec2(30., 0.)),
                        Event::WindowFocused(false),
                    ],
                    ..Default::default()
                },
                |ui| app.workspace(ui),
            );
            lost_focus.textures_delta.clear();
            assert!(app.research_ui.canvas_move.is_none());
            assert_eq!(app.host.document.editor.project, before);
            layers_frame_output(&ctx, &mut app, vec![Event::WindowFocused(true)]);
            pointer(&ctx, &mut app, at, false);
            let at = canvas_handle(&ctx, &mut app);
            pointer(&ctx, &mut app, at, true);
            let outside = app.workbench.panel_rects["view.canvas"].min - vec2(10., 10.);
            layers_frame_output(&ctx, &mut app, vec![Event::PointerMoved(outside)]);
            pointer(&ctx, &mut app, outside, false);
            assert!(app.research_ui.canvas_move.is_none());
            assert_eq!(app.host.document.editor.project, before);
            let at = canvas_handle(&ctx, &mut app);
            pointer(&ctx, &mut app, at, true);
            pointer(&ctx, &mut app, at + vec2(0.1, 0.), false);
            assert!(app.process_ui.preview.is_none());
            assert!(app.research_ui.canvas_move.is_none());
            assert_eq!(app.host.document.editor.project, before);
            if coalesced_press {
                let at = canvas_handle(&ctx, &mut app);
                let mut events = vec![
                    Event::PointerMoved(at + vec2(30., 0.)),
                    Event::PointerButton {
                        pos: at + vec2(30., 0.),
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: Modifiers::NONE,
                    },
                    Event::PointerMoved(at),
                ];
                if complete_gesture {
                    events.push(Event::PointerButton {
                        pos: at,
                        button: PointerButton::Primary,
                        pressed: false,
                        modifiers: Modifiers::NONE,
                    });
                }
                layers_frame_output(&ctx, &mut app, events);
                assert!(
                    app.research_ui
                        .canvas_move
                        .as_ref()
                        .unwrap()
                        .start
                        .is_none()
                );
                if !complete_gesture {
                    pointer(&ctx, &mut app, at, false);
                }
                assert!(app.process_ui.preview.is_none());
                assert_eq!(app.host.document.editor.project, before);
                button(&ctx, &mut app, "Cancel canvas parameter move");
                let at = canvas_handle(&ctx, &mut app);
                let target = at + vec2(0.1, 0.);
                layers_frame_output(
                    &ctx,
                    &mut app,
                    vec![
                        Event::PointerMoved(at),
                        Event::PointerButton {
                            pos: at,
                            button: PointerButton::Primary,
                            pressed: true,
                            modifiers: Modifiers::NONE,
                        },
                        Event::PointerMoved(target),
                        Event::PointerButton {
                            pos: target,
                            button: PointerButton::Primary,
                            pressed: false,
                            modifiers: Modifiers::NONE,
                        },
                    ],
                );
                assert!(app.process_ui.preview.is_none());
                assert_eq!(app.host.document.editor.project, before);
                assert!(app.research_ui.canvas_move.is_none());
            }
            for cancel in [true, false] {
                let at = canvas_handle(&ctx, &mut app);
                let rect = app.workbench.panel_rects["view.canvas"];
                let target = app.screen(
                    rect,
                    Point::new(
                        parameters.origin_mm[0] as f64 / 1000. + 2.,
                        parameters.origin_mm[1] as f64 / 1000.,
                    ),
                );
                if coalesced_press {
                    let mut events = vec![
                        Event::PointerMoved(at),
                        Event::PointerButton {
                            pos: at,
                            button: PointerButton::Primary,
                            pressed: true,
                            modifiers: Modifiers::NONE,
                        },
                        Event::PointerMoved(target),
                    ];
                    if complete_gesture {
                        events.push(Event::PointerButton {
                            pos: target,
                            button: PointerButton::Primary,
                            pressed: false,
                            modifiers: Modifiers::NONE,
                        });
                    }
                    layers_frame_output(&ctx, &mut app, events);
                } else {
                    pointer(&ctx, &mut app, at, true);
                    layers_frame_output(&ctx, &mut app, vec![Event::PointerMoved(target)]);
                }
                if coalesced_press && !complete_gesture {
                    let state = app.research_ui.canvas_move.as_ref().unwrap();
                    let pointer_state = ctx.input(|i| {
                        (
                            i.pointer.press_origin(),
                            i.pointer.latest_pos(),
                            i.pointer.primary_pressed(),
                            i.pointer.is_decidedly_dragging(),
                        )
                    });
                    assert!(
                        state.start.is_some(),
                        "coalesced press missed: at={at:?}, target={target:?}, input={pointer_state:?}"
                    );
                    // egui intentionally defers drag classification in this
                    // first frame; the release must still produce the preview.
                }
                assert_eq!(app.host.document.editor.project, before);
                if !complete_gesture {
                    pointer(&ctx, &mut app, target, false);
                }
                assert!(
                    app.process_ui.preview.is_some(),
                    "{} / {}",
                    app.process_ui.error,
                    app.research_ui.error
                );
                assert_eq!(app.host.document.editor.project, before);
                assert_eq!(
                    app.research_ui
                        .parameters
                        .as_ref()
                        .unwrap()
                        .definition
                        .candidates[0]
                        .origin_mm,
                    [parameters.origin_mm[0] + 2000, parameters.origin_mm[1]]
                );
                if cancel {
                    button(&ctx, &mut app, "Cancel design proposal");
                    button(&ctx, &mut app, "Cancel parameter draft");
                    assert_eq!(app.host.document.editor.project, before);
                } else {
                    button(&ctx, &mut app, "Confirm design change");
                }
            }
            let changed = app.host.document.editor.project.clone();
            assert_eq!(
                pm::current_parameters(&changed, &task.id, option)
                    .unwrap()
                    .origin_mm,
                [parameters.origin_mm[0] + 2000, parameters.origin_mm[1]]
            );
            for old in &before.elements {
                if !task.candidates[0].initial_mass_ids.contains(&old.id) {
                    assert_eq!(changed.elements.iter().find(|e| e.id == old.id), Some(old));
                }
            }
            assert_eq!(changed.evidence, before.evidence);
            if let (Some(check), Some(figure)) = (&check, &figure) {
                assert_eq!(
                    changed
                        .rules
                        .checks
                        .iter()
                        .find(|c| c.definition.id == check.definition.id)
                        .unwrap()
                        .state(&changed),
                    qc::State::Unknown
                );
                assert_eq!(figure.status(&changed), Status::Stale);
            }
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(app.host.document.editor.project.elements, before.elements);
            assert_eq!(
                pm::current_parameters(&app.host.document.editor.project, &task.id, option)
                    .unwrap(),
                parameters
            );
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            assert_eq!(app.host.document.editor.project.elements, changed.elements);
            if let (Some(check), Some(figure)) = (&check, &figure) {
                button(&ctx, &mut app, "Compare options…");
                button(&ctx, &mut app, "Inspect candidate checks");
                let mut visible_run = None;
                let mut diagnostic = Vec::new();
                for _ in 0..32 {
                    let out = layers_frame_output(&ctx, &mut app, vec![]);
                    let run = ctx
                        .data(|d| d.get_temp::<Rect>(Id::new(("check_run", &check.definition.id))))
                        .expect("exact current-candidate check action");
                    let visible = out.shapes.iter().any(|clipped| {
                        let mut nested = Vec::new();
                        collect_shapes(std::slice::from_ref(clipped), &mut nested);
                        nested.iter().any(|shape| matches!(shape, egui::Shape::Text(t) if t.galley.text() == language.translate("Preview check update") && run.contains(t.pos + t.galley.size()*0.5) && clipped.clip_rect.contains(t.pos + t.galley.size()*0.5)))
                    });
                    diagnostic.push(format!("run={run:?}, visible={visible}"));
                    if visible {
                        visible_run = Some(run);
                        break;
                    }
                }
                visible_run.unwrap_or_else(|| {
                    panic!("correct check action must actually be visible: {diagnostic:?}")
                });
                let run_id = ctx
                    .data(|d| d.get_temp::<Id>(Id::new(("check_run_id", &check.definition.id))))
                    .unwrap();
                action(&ctx, &mut app, run_id.accesskit_id(), Action::Click, None);
                wait_sources(&ctx, &mut app);
                button(&ctx, &mut app, "Confirm design change");
                wait_sources(&ctx, &mut app);
                let project = &app.host.document.editor.project;
                let c = project
                    .rules
                    .checks
                    .iter()
                    .find(|c| c.definition.id == check.definition.id)
                    .unwrap();
                assert_eq!(
                    c.report.as_ref().unwrap().generation,
                    check.report.as_ref().unwrap().generation + 1
                );
                assert_eq!(c.state(project), qc::State::Pass);
                button(&ctx, &mut app, "Show constraint figure");
                button(&ctx, &mut app, "Preview figure update");
                button(&ctx, &mut app, "Confirm design change");
                let project = &app.host.document.editor.project;
                let f = project
                    .drawings
                    .diagrams
                    .iter()
                    .find(|f| f.id == figure.id)
                    .unwrap();
                assert_eq!(f.generation, figure.generation + 1);
                assert_eq!(f.status(project), Status::Current);
                button(&ctx, &mut app, "Edit source model");
            }
            layers_frame_output(&ctx, &mut app, vec![key(Key::S, Modifiers::COMMAND)]);
            if app.file_action.is_some() {
                button(&ctx, &mut app, "Save");
            }
            assert_eq!(
                app.host.document.editor.status,
                language.translate("Project saved")
            );
            let saved = Editor::from_stored_document(&file).unwrap();
            assert_eq!(saved.project, app.host.document.editor.project);
            let (fresh_ctx, mut fresh) = setup();
            fresh_ctx.enable_accesskit();
            fresh.language = language;
            fresh
                .host
                .configure_operation_journal(Some(root.join("reopen-journal")))
                .unwrap();
            if supplied {
                fresh.initialize_source_connections(&root);
            }
            open(&fresh_ctx, &mut fresh, &file);
            wait_sources(&fresh_ctx, &mut fresh);
            assert_eq!(fresh.host.document.editor.project, saved.project);
            button(&fresh_ctx, &mut fresh, "AI Assistant");
            button(&fresh_ctx, &mut fresh, "Adjust generated wings");
            assert_eq!(
                fresh
                    .research_ui
                    .parameters
                    .as_ref()
                    .unwrap()
                    .definition
                    .candidates[0]
                    .origin_mm[0],
                parameters.origin_mm[0] + 2000
            );
            if !supplied {
                button(&fresh_ctx, &mut fresh, "Cancel parameter draft");
                let at = canvas_handle(&fresh_ctx, &mut fresh);
                pointer(&fresh_ctx, &mut fresh, at, true);
                layers_frame_output(
                    &fresh_ctx,
                    &mut fresh,
                    vec![Event::PointerMoved(at + vec2(30., 0.))],
                );
                // Separate user actor enters through the existing shared editor command, not a raw project assignment.
                let member = task.candidates[0].initial_mass_ids[0];
                fresh
                    .host
                    .document
                    .editor
                    .apply(crate::command::ModelCommand::MoveElements {
                        ids: vec![member],
                        offset: Point::new(0., 1.),
                    })
                    .unwrap();
                let user_edit = fresh.host.document.editor.project.clone();
                layers_frame_output(&fresh_ctx, &mut fresh, vec![]);
                pointer(&fresh_ctx, &mut fresh, at + vec2(30., 0.), false);
                assert!(fresh.research_ui.canvas_move.is_none());
                assert!(fresh.process_ui.preview.is_none());
                assert_eq!(fresh.host.document.editor.project, user_edit);
                button(&fresh_ctx, &mut fresh, "Move generated wings on canvas");
                assert!(fresh.research_ui.canvas_move.is_none());
                assert!(
                    fresh.research_ui.error.contains("Manual changes"),
                    "{}",
                    fresh.research_ui.error
                );
                assert_eq!(fresh.host.document.editor.project, user_edit);
                layers_frame_output(
                    &fresh_ctx,
                    &mut fresh,
                    vec![key(Key::Z, Modifiers::COMMAND)],
                );
                assert_eq!(
                    fresh.host.document.editor.project.elements,
                    saved.project.elements
                );
                fresh.host.document.editor.set_selection(vec![member]);
                fresh
                    .host
                    .document
                    .editor
                    .edit(crate::editing::EditCommand::Lock)
                    .unwrap();
                let locked = fresh.host.document.editor.project.clone();
                button(&fresh_ctx, &mut fresh, "Move generated wings on canvas");
                assert!(fresh.research_ui.canvas_move.is_none());
                assert!(fresh.research_ui.error.contains("Manual changes"));
                assert_eq!(fresh.host.document.editor.project, locked);
                layers_frame_output(
                    &fresh_ctx,
                    &mut fresh,
                    vec![key(Key::Z, Modifiers::COMMAND)],
                );
                assert_eq!(
                    fresh.host.document.editor.project.elements,
                    saved.project.elements
                );
            }
            std::fs::write(root.join("canvas-result.json"),serde_json::to_vec_pretty(&serde_json::json!({"output":file,"task":task.id,"option":option,"origin_before_mm":parameters.origin_mm,"origin_after_mm":[parameters.origin_mm[0]+2000,parameters.origin_mm[1]],"entry":"normal pointer/AccessKit egui events","cancel_escape_focus_leave_tiny_click":true,"undo_redo_save_reopen":true,"native_os":"not_run","AI":"not_run"})).unwrap()).unwrap();
        }
    }
    #[test]
    #[ignore = "requires current protected project and unused output"]
    fn current_same_envelope_parameter_state_normal_bilingual() {
        use crate::design_process::parametric as pm;
        let parent = PathBuf::from(std::env::var_os("OPENARCHI_PARAM_STATE_OUTPUT").unwrap());
        std::fs::create_dir(&parent).expect("inspect existing outcomes, no replay");
        let input = PathBuf::from(std::env::var_os("OPENARCHI_PARAM_STATE_INPUT").unwrap());
        let original = std::fs::read(&input).unwrap();
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let root = parent.join(if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            });
            std::fs::create_dir(&root).unwrap();
            let start = Editor::from_stored_document(&input).unwrap();
            let file = root.join("parameter-version.openarchi.json");
            storage::save_document(&start.project, &file, start.project_identity()).unwrap();
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            open(&ctx, &mut app, &file);
            wait_sources(&ctx, &mut app);
            let task = app
                .host
                .document
                .editor
                .project
                .design_process
                .task()
                .unwrap()
                .clone();
            let option = app
                .host
                .document
                .editor
                .project
                .plan_options
                .active_design_option
                .unwrap();
            let old = pm::current_parameters(&app.host.document.editor.project, &task.id, option)
                .unwrap();
            assert_eq!((old.storeys, old.floor_height_mm), (3, 3300));
            let member = task.candidates[0].initial_mass_ids[0];
            let element = app
                .host
                .document
                .editor
                .project
                .elements
                .iter()
                .find(|e| e.id == member)
                .unwrap()
                .clone();
            button(&ctx, &mut app, "Select a source mass");
            button(&ctx, &mut app, &format!("#{member} · {}", element.name));
            button(&ctx, &mut app, "Discuss selected objects");
            let field = ctx
                .data(|d| d.get_temp::<Id>(Id::new("object_research_question")))
                .unwrap();
            action(&ctx, &mut app, field.accesskit_id(), Action::Focus, None);
            layers_frame_output(&ctx,&mut app,vec![Event::Text("Study concept storey organization while keeping this envelope; no code conclusion.".into())]);
            button(&ctx, &mut app, "Preview discussion record");
            button(&ctx, &mut app, "Confirm design change");
            let topic = app
                .host
                .document
                .editor
                .project
                .evidence
                .topics
                .last()
                .unwrap()
                .clone();
            assert!(topic.exchanges[0].scope.parameter_input_sha256.is_some());
            button(&ctx, &mut app, "Compare options…");
            let choice = ctx
                .data(|d| d.get_temp::<Id>(Id::new(("comparison_choice", option))))
                .unwrap();
            action(&ctx, &mut app, choice.accesskit_id(), Action::Click, None);
            button(
                &ctx,
                &mut app,
                "I reviewed the changed basis for this choice",
            );
            button(&ctx, &mut app, "Preview choice and saved versions");
            button(&ctx, &mut app, "Confirm design change");
            button(&ctx, &mut app, "Continue in canvas");
            button(&ctx, &mut app, "AI Assistant");
            let before = app.host.document.editor.project.clone();
            let comparison = task.compare(&before).unwrap();
            let decision = before.design_process.decisions.last().unwrap().clone();
            assert_eq!(decision.comparison.fingerprint, comparison.fingerprint);
            assert_eq!(
                decision.comparison.candidates[0]
                    .parametric_parameters
                    .as_ref(),
                Some(&old)
            );
            button(&ctx, &mut app, "Adjust generated wings");
            number(&ctx, &mut app, 0, "Concept storeys", 2.);
            number(&ctx, &mut app, 0, "Floor height (mm)", 4950.);
            let unchanged_file = std::fs::read(&file).unwrap();
            layers_frame_output(&ctx, &mut app, vec![key(Key::S, Modifiers::COMMAND)]);
            if app.file_action.is_some() {
                button(&ctx, &mut app, "Save");
            }
            assert!(
                app.file_error.contains("parameter draft"),
                "{}",
                app.file_error
            );
            assert_eq!(std::fs::read(&file).unwrap(), unchanged_file);
            assert_eq!(app.host.document.editor.project, before);
            if app.file_action.is_some() {
                button(&ctx, &mut app, "Cancel");
            }
            let selected = PathBuf::from(app.path.clone());
            open_test_project(&ctx, &mut app, &selected);
            assert!(
                app.file_error.contains("parameter draft"),
                "{}",
                app.file_error
            );
            assert_eq!(app.host.document.editor.project, before);
            button(&ctx, &mut app, "Cancel");
            app.perform_menu_action(crate::app::native_menu::MenuAction::Quit, &ctx);
            assert!(!app.allow_close && !app.close_prompt);
            assert_eq!(app.host.document.editor.project, before);
            assert_eq!(
                app.research_ui
                    .parameters
                    .as_ref()
                    .unwrap()
                    .definition
                    .candidates[0]
                    .floor_height_mm,
                4950
            );
            button(&ctx, &mut app, "Preview two-wing parameters");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            let out = layers_frame_output(&ctx, &mut app, vec![]);
            let mut texts = vec![];
            collect_text(&out.shapes, &mut texts);
            assert!(
                texts.iter().any(|t| t
                    == language
                        .translate("Envelope unchanged; parameter and comparison versions change")),
                "{texts:?}"
            );
            assert_eq!(app.host.document.editor.project, before);
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, before);
            button(&ctx, &mut app, "Preview two-wing parameters");
            button(&ctx, &mut app, "Confirm design change");
            let changed = app.host.document.editor.project.clone();
            let now = pm::current_parameters(&changed, &task.id, option).unwrap();
            assert_eq!((now.storeys, now.floor_height_mm), (2, 4950));
            assert_eq!(changed.elements, before.elements);
            assert_eq!(changed.site, before.site);
            assert_eq!(changed.program, before.program);
            assert_eq!(changed.evidence, before.evidence);
            assert_eq!(changed.rules, before.rules);
            assert_eq!(changed.drawings, before.drawings);
            assert_ne!(
                task.compare(&changed).unwrap().fingerprint,
                decision.comparison.fingerprint
            );
            assert_eq!(
                topic.exchanges[0].objects[0].link.status(&changed),
                crate::evidence::LinkStatus::Unchanged
            );
            assert_eq!(
                topic.exchanges[0].scope.input_status(&changed),
                Some("Discussion design inputs changed; old discussion retained")
            );
            for c in &before.rules.checks {
                assert_eq!(c.state(&changed), c.state(&before));
            }
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(app.host.document.editor.project.elements, before.elements);
            assert_eq!(
                pm::current_parameters(&app.host.document.editor.project, &task.id, option)
                    .unwrap(),
                old
            );
            assert_eq!(
                task.compare(&app.host.document.editor.project)
                    .unwrap()
                    .fingerprint,
                comparison.fingerprint
            );
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            assert_eq!(
                pm::current_parameters(&app.host.document.editor.project, &task.id, option)
                    .unwrap(),
                now
            );
            layers_frame_output(&ctx, &mut app, vec![key(Key::S, Modifiers::COMMAND)]);
            if app.file_action.is_some() {
                button(&ctx, &mut app, "Save");
            }
            let saved = Editor::from_stored_document(&file).unwrap();
            assert_eq!(saved.project, app.host.document.editor.project);
            assert_eq!(
                pm::current_parameters(&saved.project, &task.id, option).unwrap(),
                now
            );
            let raw: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
            assert_eq!(raw["schema_version"], 28);
            let (fresh_ctx, mut fresh) = setup();
            fresh_ctx.enable_accesskit();
            fresh.language = language;
            fresh
                .host
                .configure_operation_journal(Some(root.join("reopen-journal")))
                .unwrap();
            open(&fresh_ctx, &mut fresh, &file);
            wait_sources(&fresh_ctx, &mut fresh);
            button(&fresh_ctx, &mut fresh, "AI Assistant");
            button(&fresh_ctx, &mut fresh, "Adjust generated wings");
            assert_eq!(
                fresh
                    .research_ui
                    .parameters
                    .as_ref()
                    .unwrap()
                    .definition
                    .candidates[0],
                now
            );
            number(
                &fresh_ctx,
                &mut fresh,
                0,
                "Origin X (mm)",
                (now.origin_mm[0] + 1000) as f64,
            );
            button(&fresh_ctx, &mut fresh, "Preview two-wing parameters");
            button(&fresh_ctx, &mut fresh, "Confirm design change");
            assert_eq!(
                pm::current_parameters(&fresh.host.document.editor.project, &task.id, option)
                    .unwrap()
                    .storeys,
                2
            );
            assert_ne!(
                fresh.host.document.editor.project.elements,
                saved.project.elements
            );
            layers_frame_output(
                &fresh_ctx,
                &mut fresh,
                vec![key(Key::Z, Modifiers::COMMAND)],
            );
            assert_eq!(
                fresh.host.document.editor.project.elements,
                saved.project.elements
            );
            assert_eq!(
                pm::current_parameters(&fresh.host.document.editor.project, &task.id, option)
                    .unwrap(),
                now
            );
            layers_frame_output(
                &fresh_ctx,
                &mut fresh,
                vec![key(Key::S, Modifiers::COMMAND)],
            );
            if fresh.file_action.is_some() {
                button(&fresh_ctx, &mut fresh, "Save");
            }
            let final_read = Editor::from_stored_document(&file).unwrap();
            assert_eq!(final_read.project, fresh.host.document.editor.project);
            assert_eq!(
                pm::current_parameters(&final_read.project, &task.id, option).unwrap(),
                now
            );
            assert_eq!(std::fs::read(&input).unwrap(), original);
            std::fs::write(root.join("parameter-state-result.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"output":file,"task":task.id,"option":option,"before":old,"after":now,"geometry_unchanged":true,"rules_figures_not_recalculated":true,"old_decision_retained_stale":decision.id,"topic_inputs_changed_though_element_unchanged":topic.id,"native":28,"entry":"normal bilingual egui/AccessKit; not native OS","AI":"not_run","after_reopen_geometry_commit_and_undo":true})).unwrap()).unwrap();
        }
    }
    #[cfg(unix)]
    fn object_rpc(
        endpoint: &mut crate::automation_ipc::Endpoint,
        host: &mut crate::application_host::ApplicationHost,
        socket: &std::path::Path,
        root: &std::path::Path,
        request: serde_json::Value,
    ) -> serde_json::Value {
        use std::io::{BufRead, BufReader, Write};
        let stem = root.join(format!("rpc-{}", storage::new_document_identity()));
        std::fs::write(
            stem.with_extension("request.json"),
            serde_json::to_vec_pretty(&request).unwrap(),
        )
        .unwrap();
        let socket = socket.to_owned();
        let client = std::thread::spawn(move || {
            let mut stream = std::os::unix::net::UnixStream::connect(socket).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .unwrap();
            writeln!(stream, "{request}").unwrap();
            let mut line = String::new();
            BufReader::new(stream).read_line(&mut line).unwrap();
            serde_json::from_str::<serde_json::Value>(&line).unwrap()
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !client.is_finished() {
            endpoint.poll_with_host(host, None);
            assert!(
                std::time::Instant::now() < deadline,
                "unknown response: inspect recorded original request, never replay"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let response = client.join().unwrap();
        std::fs::write(
            stem.with_extension("response.json"),
            serde_json::to_vec_pretty(&response).unwrap(),
        )
        .unwrap();
        response
    }
    #[cfg(unix)]
    fn visible_agent_reply(ctx: &Context, app: &mut AtelierApp, question: &str) {
        let output = layers_frame_output(ctx, app, vec![]);
        let _ = output;
        let at = task_scroll_point(ctx);
        let wheel = |dy| {
            vec![
                Event::PointerMoved(at),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    phase: egui::TouchPhase::Move,
                    delta: vec2(0., dy),
                    modifiers: Modifiers::NONE,
                },
            ]
        };
        let expected = app
            .host
            .document
            .editor
            .project
            .evidence
            .topics
            .iter()
            .rev()
            .flat_map(|t| t.exchanges.iter().rev())
            .find(|e| e.question == question)
            .and_then(|e| e.reply.as_ref())
            .expect("saved answer for this question")
            .text
            .clone();
        let mut diagnostic = vec![];
        // A resumed task is already expanded. Do not blindly toggle it closed.
        for attempt in 0..2 {
            for _ in 0..4 {
                layers_frame_output(ctx, app, wheel(1000.));
            }
            if attempt == 1 {
                button(ctx, app, question);
            }
            for _ in 0..16 {
                let output = layers_frame_output(ctx, app, vec![]);
                let mut texts = vec![];
                collect_text(&output.shapes, &mut texts);
                diagnostic.push(serde_json::json!({"at":[at.x,at.y],"attempt":attempt,"text":texts.iter().map(|s|serde_json::json!({"text":s,"center":text_center(&output.shapes,s).map(|p|[p.x,p.y])})).collect::<Vec<_>>()}));
                if texts.iter().any(|s| s == &expected) {
                    return;
                }
                layers_frame_output(ctx, app, wheel(-100.));
            }
        }
        if let Some(root) = std::env::var_os("OPENARCHI_OBJECT_TASK_OUTPUT") {
            std::fs::write(
                PathBuf::from(root).join("reply-visibility-diagnostic.json"),
                serde_json::to_vec_pretty(&diagnostic).unwrap(),
            )
            .unwrap();
        }
        panic!(
            "This question's saved Agent answer must be visible through the normal topic and scroll path"
        );
    }
    #[test]
    #[cfg(unix)]
    #[ignore = "requires latest protected project + absolute unused output; preserves original operations"]
    fn current_object_agent_reply_normal_bilingual() {
        use serde_json::json;
        let parent = PathBuf::from(std::env::var_os("OPENARCHI_REPLY_OUTPUT").unwrap());
        assert!(parent.is_absolute());
        std::fs::create_dir(&parent).unwrap();
        let input = PathBuf::from(std::env::var_os("OPENARCHI_REPLY_INPUT").unwrap());
        let input_bytes = std::fs::read(&input).unwrap();
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let root = parent.join(if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            });
            std::fs::create_dir(&root).unwrap();
            let file = root.join("agent-discussion.openarchi.json");
            let input_editor = Editor::from_stored_document(&input).unwrap();
            storage::save_document(
                &input_editor.project,
                &file,
                input_editor.project_identity(),
            )
            .unwrap();
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            open(&ctx, &mut app, &file);
            wait_sources(&ctx, &mut app);
            let original = app.host.document.editor.project.elements.clone();
            let member = app
                .host
                .document
                .editor
                .project
                .design_process
                .task()
                .unwrap()
                .candidates[0]
                .initial_mass_ids[0];
            let name = app
                .host
                .document
                .editor
                .project
                .elements
                .iter()
                .find(|e| e.id == member)
                .unwrap()
                .name
                .clone();
            button(&ctx, &mut app, "Select a source mass");
            button(&ctx, &mut app, &format!("#{member} · {name}"));
            button(&ctx, &mut app, "Discuss selected objects");
            let question_text = std::env::var("OPENARCHI_REPLY_QUESTION").unwrap_or_else(|_| {
                "Can we keep this envelope and review the conceptual storey organization?".into()
            });
            let question = question_text.as_str();
            let field = ctx
                .data(|d| d.get_temp::<Id>(Id::new("object_research_question")))
                .unwrap();
            action(&ctx, &mut app, field.accesskit_id(), Action::Focus, None);
            layers_frame_output(&ctx, &mut app, vec![Event::Text(question.into())]);
            button(&ctx, &mut app, "Preview discussion record");
            button(&ctx, &mut app, "Confirm design change");
            let before = app.host.document.editor.project.clone();
            let topic = before.evidence.topics.last().unwrap().clone();
            if std::env::var_os("OPENARCHI_REPLY_QUESTION_ONLY").is_some() {
                layers_frame_output(&ctx, &mut app, vec![key(Key::S, Modifiers::COMMAND)]);
                if app.file_action.is_some() {
                    button(&ctx, &mut app, "Save");
                }
                let saved = Editor::from_stored_document(&file).unwrap();
                assert_eq!(saved.project.elements, original);
                assert_eq!(saved.project.evidence.topics.last().unwrap(), &topic);
                assert!(topic.exchanges[0].reply.is_none());
                std::fs::write(root.join("question-ready.json"),serde_json::to_vec_pretty(&json!({"input":input,"output":file,"topic":topic.id,"question":question,"preparation":"normal product GUI events + save; test driver input, no AI response","awaiting":"actual external Agent; no outbound submission claimed"})).unwrap()).unwrap();
                continue;
            }
            let socket_root = std::env::temp_dir().join(format!(
                "oa-reply-{}",
                &storage::new_document_identity()[..12]
            ));
            std::fs::create_dir(&socket_root).unwrap();
            let socket = socket_root.join("ipc/a.sock");
            let mut endpoint = crate::automation_ipc::Endpoint::bind_for_host(
                socket.clone(),
                true,
                Some(root.join("journal")),
                &mut app.host,
            )
            .unwrap();
            let target = object_rpc(
                &mut endpoint,
                &mut app.host,
                &socket,
                &root,
                json!({"method":"get_target"}),
            )["result"]["target"]
                .clone();
            let context = object_rpc(
                &mut endpoint,
                &mut app.host,
                &socket,
                &root,
                json!({"method":"get_context","params":target}),
            )["result"]
                .clone();
            let read = object_rpc(
                &mut endpoint,
                &mut app.host,
                &socket,
                &root,
                json!({"method":"query_source_evidence","params":{"target":target,"expected_revision":context["revision"],"topics":true,"offset":before.evidence.topics.len()-1,"limit":1}}),
            );
            assert_eq!(read["result"]["topics"][0]["topic"]["id"], topic.id);
            let reply=crate::evidence::topics::AgentReply{id:storage::new_document_identity(),agent_label:"Product-path regression client (not AI execution)".into(),text:"The envelope can stay unchanged while concept storeys and floor height change. Verify actual use and approval separately.".into(),claims:vec![crate::design_process::intent::Claim{kind:crate::design_process::intent::ClaimKind::Observation,text:"The saved rule has an editable storey/height pair.".into()},crate::design_process::intent::Claim{kind:crate::design_process::intent::ClaimKind::Inference,text:"This does not establish legal floor area or approved capacity.".into()}]};
            let edit = crate::evidence::EvidenceEdit::RecordTopicReply {
                id: topic.id.clone(),
                exchange_index: 0,
                expected_topic_sha256: read["result"]["topics"][0]["topic_sha256"]
                    .as_str()
                    .unwrap()
                    .into(),
                reply: reply.clone(),
            };
            let prepare = json!({"method":"prepare_native_change","params":{"target":target,"expected_revision":context["revision"],"expected_context_revision":context["context_revision"],"change":{"version":1,"edits":[{"operation":"evidence","edit":edit}]}}});
            let preview = object_rpc(
                &mut endpoint,
                &mut app.host,
                &socket,
                &root,
                prepare.clone(),
            );
            assert!(preview["result"]["preview_id"].is_string(), "{preview}");
            assert_eq!(app.host.document.editor.project, before);
            let cancelled = object_rpc(
                &mut endpoint,
                &mut app.host,
                &socket,
                &root,
                json!({"method":"cancel_operation","params":{"target":target,"preview_id":preview["result"]["preview_id"]}}),
            );
            assert!(cancelled.get("error").is_none());
            assert_eq!(app.host.document.editor.project, before);
            let preview = object_rpc(&mut endpoint, &mut app.host, &socket, &root, prepare);
            assert!(preview["result"]["preview_id"].is_string(), "{preview}");
            let commit = object_rpc(
                &mut endpoint,
                &mut app.host,
                &socket,
                &root,
                json!({"method":"commit_change","params":{"target":target,"preview_id":preview["result"]["preview_id"],"request_id":reply.id}}),
            );
            assert_eq!(commit["result"]["status"], "committed", "{commit}");
            let inspected = object_rpc(
                &mut endpoint,
                &mut app.host,
                &socket,
                &root,
                json!({"method":"get_operation","params":{"target":target,"original_target":target,"request_id":reply.id}}),
            );
            assert_eq!(inspected["result"]["status"], "committed");
            drop(endpoint);
            assert_eq!(app.host.document.editor.project.elements, original);
            visible_agent_reply(&ctx, &mut app, question);
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(app.host.document.editor.project.evidence, before.evidence);
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .evidence
                    .topics
                    .last()
                    .unwrap()
                    .exchanges[0]
                    .reply,
                Some(reply.clone())
            );
            layers_frame_output(&ctx, &mut app, vec![key(Key::S, Modifiers::COMMAND)]);
            if app.file_action.is_some() {
                button(&ctx, &mut app, "Save");
            }
            let saved = Editor::from_stored_document(&file).unwrap();
            assert_eq!(saved.project.elements, original);
            assert_eq!(
                saved.project.evidence,
                app.host.document.editor.project.evidence
            );
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&std::fs::read(&file).unwrap())
                    .unwrap()["schema_version"],
                29
            );
            let (fresh_ctx, mut fresh) = setup();
            fresh_ctx.enable_accesskit();
            fresh_ctx.global_style_mut(|s| s.animation_time = 0.);
            fresh.language = language;
            open(&fresh_ctx, &mut fresh, &file);
            wait_sources(&fresh_ctx, &mut fresh);
            button(&fresh_ctx, &mut fresh, "AI Assistant");
            visible_agent_reply(&fresh_ctx, &mut fresh, question);
            assert_eq!(
                fresh.host.document.editor.project.evidence,
                saved.project.evidence
            );
            std::fs::write(root.join("reply-result.json"),serde_json::to_vec_pretty(&json!({"input":input,"output":file,"topic":topic.id,"reply":reply.id,"original_geometry_preserved":true,"normal_question_and_save_reopen":true,"default_socket_reply_preview_cancel_commit_inspect":true,"identity":"declared, not authenticated","real_ai":"not_run","native_os":"not_run"})).unwrap()).unwrap();
        }
        assert_eq!(std::fs::read(&input).unwrap(), input_bytes);
    }
    #[test]
    #[ignore = "requires actual Agent saved result and unused output"]
    fn current_saved_agent_reply_continue_normal_bilingual() {
        use crate::design_process::parametric as pm;
        let parent = PathBuf::from(std::env::var_os("OPENARCHI_AGENT_RESULT_OUTPUT").unwrap());
        std::fs::create_dir(&parent).unwrap();
        let input = PathBuf::from(std::env::var_os("OPENARCHI_AGENT_RESULT_INPUT").unwrap());
        let original = std::fs::read(&input).unwrap();
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let root = parent.join(if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            });
            std::fs::create_dir(&root).unwrap();
            let file = root.join("continued-agent-result.openarchi.json");
            let initial = Editor::from_stored_document(&input).unwrap();
            storage::save_document(&initial.project, &file, initial.project_identity()).unwrap();
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            open(&ctx, &mut app, &file);
            wait_sources(&ctx, &mut app);
            let before = app.host.document.editor.project.clone();
            let topic = before.evidence.topics.last().unwrap();
            assert!(
                topic.exchanges[0]
                    .reply
                    .as_ref()
                    .unwrap()
                    .agent_label
                    .contains("当前开发主Agent")
            );
            let task = before.design_process.task().unwrap();
            let option = before.plan_options.active_design_option.unwrap();
            let old = pm::current_parameters(&before, &task.id, option).unwrap();
            assert_eq!(old.wing_depth_mm, 7500);
            button(&ctx, &mut app, "AI Assistant");
            visible_agent_reply(&ctx, &mut app, &topic.exchanges[0].question);
            // Move the existing scroll back to the inline parameter controls.
            let out = layers_frame_output(&ctx, &mut app, vec![]);
            let at = text_center(&out.shapes, app.language.translate("Object research"))
                .unwrap_or(pos2(1200., 550.))
                + vec2(0., 40.);
            for _ in 0..4 {
                layers_frame_output(
                    &ctx,
                    &mut app,
                    vec![
                        Event::PointerMoved(at),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            phase: egui::TouchPhase::Move,
                            delta: vec2(0., 1000.),
                            modifiers: Modifiers::NONE,
                        },
                    ],
                );
            }
            button(&ctx, &mut app, "Adjust generated wings");
            number(&ctx, &mut app, 0, "Wing depth (mm)", 7600.);
            button(&ctx, &mut app, "Preview two-wing parameters");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            assert_eq!(app.host.document.editor.project, before);
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, before);
            button(&ctx, &mut app, "Preview two-wing parameters");
            button(&ctx, &mut app, "Confirm design change");
            assert_eq!(
                pm::current_parameters(&app.host.document.editor.project, &task.id, option)
                    .unwrap()
                    .wing_depth_mm,
                7600
            );
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .design_process
                    .updates
                    .last()
                    .unwrap()
                    .entry,
                crate::design_process::Entry::ManualInterface
            );
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(app.host.document.editor.project.elements, before.elements);
            assert_eq!(
                pm::current_parameters(&app.host.document.editor.project, &task.id, option)
                    .unwrap(),
                old
            );
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            assert_eq!(
                pm::current_parameters(&app.host.document.editor.project, &task.id, option)
                    .unwrap()
                    .wing_depth_mm,
                7600
            );
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            layers_frame_output(&ctx, &mut app, vec![key(Key::S, Modifiers::COMMAND)]);
            if app.file_action.is_some() {
                button(&ctx, &mut app, "Save");
            }
            let saved = Editor::from_stored_document(&file).unwrap();
            assert_eq!(saved.project.elements, before.elements);
            assert_eq!(saved.project.evidence, before.evidence);
            assert_eq!(
                pm::current_parameters(&saved.project, &task.id, option).unwrap(),
                old
            );
            std::fs::write(root.join("continued-result.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"output":file,"actor":"normal egui manual followup (not AI)","actual_agent_reply_retained":true,"changed_depth_mm":7600,"undo_restored_depth_mm":7500,"all_original_geometry_exact_after_undo":true,"saved_history_retained":true,"native_os":"not_run"})).unwrap()).unwrap();
        }
        assert_eq!(std::fs::read(input).unwrap(), original);
    }
    include!("object_task_ui_tests.rs");
}
