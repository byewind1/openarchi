mod direct_concept_product_path {
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
        let tree = out
            .platform_output
            .accesskit_update
            .expect("normal product accessibility tree");
        let id = tree
            .nodes
            .iter()
            .find(|(_, n)| {
                n.supports_action(Action::Click)
                    && (n.label() == Some(app.language.translate(label))
                        || n.value() == Some(app.language.translate(label)))
            })
            .map(|(id, _)| *id)
            .unwrap_or_else(|| panic!("action {label}; {}", app.process_ui.error));
        action(ctx, app, id, Action::Click, None);
    }
    fn field(ctx: &Context, app: &mut AtelierApp, old: &str, value: &str) {
        let out = layers_frame_output(ctx, app, vec![]);
        let at = text_center(&out.shapes, old).expect("visible file path");
        layers_click(ctx, app, at);
        layers_frame_output(
            ctx,
            app,
            vec![key(Key::A, Modifiers::COMMAND), Event::Text(value.into())],
        );
    }
    fn finish(ctx: &Context, app: &mut AtelierApp) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            layers_frame_output(ctx, app, vec![]);
            if !app.source_ui.pending_for_ui() {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(app.source_ui.error.is_empty(), "{}", app.source_ui.error);
    }
    fn shift_click(ctx: &Context, app: &mut AtelierApp, at: Pos2) {
        for pressed in [true, false] {
            layers_frame_output(
                ctx,
                app,
                vec![
                    Event::ModifiersChanged(Modifiers::SHIFT),
                    Event::PointerMoved(at),
                    Event::PointerButton {
                        pos: at,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::SHIFT,
                    },
                ],
            );
        }
        layers_frame_output(ctx, app, vec![Event::ModifiersChanged(Modifiers::NONE)]);
    }
    #[test]
    fn drawn_masses_enter_two_candidates_through_normal_controls_bilingual() {
        path(false);
    }
    #[test]
    #[ignore = "requires explicit protected input, output and authored rectangle coordinates"]
    fn supplied_site_masses_enter_two_candidates_through_normal_controls_bilingual() {
        path(true);
    }
    #[test]
    #[ignore = "requires explicitly saved two-candidate project copies and source repositories"]
    fn saved_site_candidates_check_choice_local_feedback_and_figure_update_bilingual() {
        use crate::{constraints as qc, diagrams::Status};
        let parent =
            PathBuf::from(std::env::var_os("OPENARCHI_SITE_CONTINUE_OUTPUT").expect("new output"));
        let input_root = PathBuf::from(
            std::env::var_os("OPENARCHI_SITE_CONTINUE_INPUT").expect("saved bilingual candidates"),
        );
        fn check_input(ctx: &Context, app: &mut AtelierApp, label: &str, value: &str) {
            let rect = ctx
                .data(|d| d.get_temp::<Rect>(Id::new(("check_field", label))))
                .unwrap();
            layers_click(ctx, app, rect.center());
            layers_frame_output(
                ctx,
                app,
                vec![
                    key(Key::A, Modifiers::COMMAND),
                    Event::Text(value.into()),
                    key(Key::Tab, Modifiers::NONE),
                ],
            );
        }
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let lang = if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            };
            let root = parent.join(lang);
            std::fs::create_dir(&root).expect("inspect original outcome, no replay");
            let input = input_root.join(lang).join("continued.internal.json");
            let old_bytes = std::fs::read(&input).unwrap();
            let start = Editor::from_stored_document(&input).unwrap();
            let task = start.project.design_process.task().unwrap().clone();
            assert!(task.is_mass());
            assert!(start.project.site.control_region.is_some());
            let repository = crate::project_repository::Repository::open(
                &input_root.join(lang).join("repository"),
            )
            .unwrap()
            .copy_to(&root.join("repository"))
            .unwrap();
            let file = root.join("continued.internal.json");
            storage::save_document(&start.project, &file, start.project_identity()).unwrap();
            std::fs::write(root.join("operation-plan.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"input_sha256":crate::drawings::digest(&old_bytes),"output":file,"task":task.id,"option":task.candidates[0].option,"edit_only":task.candidates[0].initial_mass_ids[0],"local_feedback":"translate first wing -2m in model Y; authored concept feedback, no demolition/capacity/approval claim","check":"complete current candidate footprint inside declared source control region; AtLeast0 is geometric containment, not an approved setback","entry":"normal checks/comparison/figure/preview/cancel/update/save/fresh-app-open events","on_unknown":"inspect original journal and target, never replay with another identity"})).unwrap()).unwrap();
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
            open_test_project(&ctx, &mut app, &file);
            finish(&ctx, &mut app);
            assert!(app.host.document.editor.selection_ids().is_empty());
            let initial = app.host.document.editor.project.clone();
            button(&ctx, &mut app, "Check current design");
            button(
                &ctx,
                &mut app,
                "Add check for current selection or candidate",
            );
            let metric = ctx
                .data(|d| d.get_temp::<Rect>(Id::new("check_metric_control")))
                .unwrap();
            layers_click(&ctx, &mut app, metric.center());
            button(&ctx, &mut app, "Clearance within declared control region");
            check_input(&ctx, &mut app, "Limit (blank means unknown)", "0");
            check_input(
                &ctx,
                &mut app,
                "Optional assumption note",
                "Only geometric containment within the declared source outline. Authority, current applicability, measured units, demolition and capacity remain unknown.",
            );
            button(&ctx, &mut app, "Preview quantified check");
            assert!(
                app.process_ui.preview.is_some(),
                "{} / {}",
                app.process_ui.error,
                app.checks_ui.error
            );
            assert_eq!(app.host.document.editor.project, initial);
            button(&ctx, &mut app, "Cancel design proposal");
            button(&ctx, &mut app, "Preview quantified check");
            button(&ctx, &mut app, "Confirm design change");
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
                matches!(check.definition.scope,qc::Scope::Candidate{option:Some(id),..} if id==task.candidates[0].option)
            );
            assert_eq!(
                check.definition.metric,
                qc::Metric::ReferenceControlRegionClearance
            );
            assert_eq!(
                check.state(&app.host.document.editor.project),
                qc::State::Pass
            );
            let previous_value = check
                .report
                .as_ref()
                .unwrap()
                .rows
                .iter()
                .filter_map(|r| r.value)
                .fold(f64::INFINITY, f64::min);
            button(&ctx, &mut app, "Compare options…");
            assert!(app.process_ui.open && !app.checks_ui.open);
            let current = task.compare(&app.host.document.editor.project).unwrap();
            assert!(
                !current.basis_current,
                "saved check is a real condition change, never silently rebase"
            );
            let chosen = ctx
                .data(|d| {
                    d.get_temp::<Id>(Id::new(("comparison_choice", task.candidates[0].option)))
                })
                .unwrap();
            action(&ctx, &mut app, chosen.accesskit_id(), Action::Click, None);
            button(
                &ctx,
                &mut app,
                "I reviewed the changed basis for this choice",
            );
            button(&ctx, &mut app, "Preview choice and saved versions");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            button(&ctx, &mut app, "Confirm design change");
            button(&ctx, &mut app, "Continue in canvas");
            button(&ctx, &mut app, "Check current design");
            button(&ctx, &mut app, "Preview constraint figure");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            button(&ctx, &mut app, "Cancel design proposal");
            button(&ctx, &mut app, "Preview constraint figure");
            button(&ctx, &mut app, "Confirm design change");
            let figure_id = app.current_constraint_figure_id().unwrap();
            let before_feedback = app.host.document.editor.project.clone();
            let figure = before_feedback
                .drawings
                .diagrams
                .iter()
                .find(|f| f.id == figure_id)
                .unwrap()
                .clone();
            assert_eq!(figure.status(&before_feedback), Status::Current);
            button(&ctx, &mut app, "Edit source model");
            button(&ctx, &mut app, "Compare options…");
            let target = task.candidates[0].initial_mass_ids[0];
            if app.process_ui.selected_space != Some(target) {
                button(&ctx, &mut app, "Select a mass on the canvas");
                let name = before_feedback
                    .elements
                    .iter()
                    .find(|e| e.id == target)
                    .unwrap()
                    .name
                    .clone();
                button(&ctx, &mut app, &name);
            }
            let dy = ctx
                .data(|d| d.get_temp::<Id>(Id::new("candidate_dy_field")))
                .unwrap();
            action(
                &ctx,
                &mut app,
                dy.accesskit_id(),
                Action::SetValue,
                Some(ActionData::NumericValue(-2.)),
            );
            assert_eq!(app.process_ui.dy, -2.);
            button(&ctx, &mut app, "Preview candidate edit");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            assert_eq!(app.host.document.editor.project, before_feedback);
            button(&ctx, &mut app, "Cancel design proposal");
            button(&ctx, &mut app, "Preview candidate edit");
            button(&ctx, &mut app, "Confirm design change");
            let changed = app.host.document.editor.project.clone();
            for e in &before_feedback.elements {
                if e.id != target {
                    assert_eq!(changed.elements.iter().find(|x| x.id == e.id), Some(e));
                }
            }
            let stale = changed
                .rules
                .checks
                .iter()
                .find(|c| c.definition.id == check.definition.id)
                .unwrap();
            assert_eq!(stale.state(&changed), qc::State::Unknown);
            assert_eq!(stale.report, check.report);
            assert_eq!(
                changed.drawings.diagrams.iter().find(|f| f.id == figure_id),
                Some(&figure)
            );
            assert_eq!(figure.status(&changed), Status::Stale);
            button(&ctx, &mut app, "Continue in canvas");
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(
                app.host.document.editor.project.elements,
                before_feedback.elements
            );
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::ALT)],
            );
            assert_eq!(app.host.document.editor.project.elements, changed.elements);
            button(&ctx, &mut app, "Check current design");
            button(&ctx, &mut app, "Preview check update");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            button(&ctx, &mut app, "Cancel design proposal");
            button(&ctx, &mut app, "Preview check update");
            button(&ctx, &mut app, "Confirm design change");
            let updated_check = app
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
                updated_check.report.as_ref().unwrap().generation,
                check.report.as_ref().unwrap().generation + 1
            );
            let updated_value = updated_check
                .report
                .as_ref()
                .unwrap()
                .rows
                .iter()
                .filter_map(|r| r.value)
                .fold(f64::INFINITY, f64::min);
            assert!(
                (updated_value - previous_value).abs() > 0.1,
                "local feedback must change the independently inspectable clearance"
            );
            button(&ctx, &mut app, "Show constraint figure");
            button(&ctx, &mut app, "Preview figure update");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            button(&ctx, &mut app, "Cancel design proposal");
            button(&ctx, &mut app, "Preview figure update");
            button(&ctx, &mut app, "Confirm design change");
            let final_project = app.host.document.editor.project.clone();
            let final_figure = final_project
                .drawings
                .diagrams
                .iter()
                .find(|f| f.id == figure_id)
                .unwrap();
            assert_eq!(final_figure.generation, figure.generation + 1);
            assert_eq!(final_figure.status(&final_project), Status::Current);
            assert_eq!(final_project.elements, changed.elements);
            for old in &initial.drawings.diagrams {
                assert_eq!(
                    final_project
                        .drawings
                        .diagrams
                        .iter()
                        .find(|f| f.id == old.id),
                    Some(old)
                );
            }
            button(&ctx, &mut app, "Edit source model");
            button(&ctx, &mut app, "Save project");
            button(&ctx, &mut app, "Save");
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let reopened = Editor::from_stored_document(&file).unwrap();
            assert_eq!(reopened.project, final_project);
            let (fresh_ctx, mut fresh) = setup();
            fresh_ctx.enable_accesskit();
            fresh.language = language;
            fresh_ctx.global_style_mut(|s| s.animation_time = 0.);
            fresh.initialize_source_connections(&root);
            open_test_project(&fresh_ctx, &mut fresh, &file);
            finish(&fresh_ctx, &mut fresh);
            assert_eq!(fresh.host.document.editor.project, final_project);
            assert!(fresh.source_ui.repository.is_some());
            button(&fresh_ctx, &mut fresh, "Compare options…");
            assert!(fresh.process_ui.open);
            assert_eq!(std::fs::read(&input).unwrap(), old_bytes);
            std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"output":file,"task":task.id,"choice":final_project.design_process.current_choice,"target_mass":target,"check_id":check.definition.id,"previous_clearance_m":previous_value,"updated_clearance_m":updated_value,"figure_id":figure_id,"figure_generation":final_figure.generation,"originals_and_other_candidate":"preserved","normal_product_events":"pass","fresh_app_source_restore":"pass","new_os_process":"not_run","native_package":"not_run","real_ai":"not_run","approval":"unknown"})).unwrap()).unwrap();
        }
    }
    fn path(supplied: bool) {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            let parent = if supplied {
                PathBuf::from(
                    std::env::var_os("OPENARCHI_SITE_CONCEPT_OUTPUT")
                        .expect("new protected output"),
                )
            } else {
                std::env::temp_dir().join(format!(
                    "oa-direct-mass-{}",
                    storage::new_document_identity()
                ))
            };
            std::fs::create_dir_all(&parent).unwrap();
            let root = parent.join(if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            });
            std::fs::create_dir(&root).expect("inspect existing test outcome, do not replay");
            let file = root.join("continued.internal.json");
            let input = if supplied {
                Some(PathBuf::from(
                    std::env::var_os("OPENARCHI_SITE_CONCEPT_INPUT")
                        .expect("saved source/control project"),
                ))
            } else {
                None
            };
            let input_bytes = input.as_ref().map(|p| std::fs::read(p).unwrap());
            let start = if let Some(p) = &input {
                Editor::from_stored_document(p).unwrap()
            } else {
                Editor::new(Project::default())
            };
            assert!(start.project.design_process.current_task.is_none());
            storage::save_document(&start.project, &file, start.project_identity()).unwrap();
            let rectangles: Vec<[[f64; 2]; 2]> = if supplied {
                serde_json::from_str(
                    &std::env::var("OPENARCHI_SITE_CONCEPT_RECTANGLES")
                        .expect("explicit authored assumptions"),
                )
                .unwrap()
            } else {
                vec![[[-4., 1.], [-1., 3.]], [[1., 1.], [4., 3.]]]
            };
            assert_eq!(rectangles.len(), 2);
            std::fs::write(root.join("operation-plan.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"input_sha256":input_bytes.as_ref().map(|b|crate::drawings::digest(b)),"output":file,"rectangles":rectangles,"geometry":"explicit authored concept assumptions, not measurements or approved design","entry":"normal file-open/canvas/tool/project-explore/preview/confirm/candidate-edit/save events","native_os":"not_run","real_ai":"not_run","on_interruption":"inspect original journal and output; never replay unknown operations"})).unwrap()).unwrap();
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            app.initialize_source_connections(&root);
            let source_manifest = if supplied {
                let original = PathBuf::from(
                    std::env::var_os("OPENARCHI_SITE_CONCEPT_REPOSITORY")
                        .expect("explicit existing source repository"),
                );
                let before = std::fs::read(original.join("project.json")).unwrap();
                let copied = crate::project_repository::Repository::open(&original)
                    .unwrap()
                    .copy_to(&root.join("repository"))
                    .unwrap();
                // Fixture start: an already-authorized local source connection.
                // Normal File/Open must actually restore it; no direct model/UI attachment.
                crate::project_repository::connection::remember(
                    app.source_ui.connections_directory.as_ref().unwrap(),
                    &file,
                    start.project_identity(),
                    &storage::FileVersion::read(&file).unwrap().token().unwrap(),
                    &copied,
                )
                .unwrap();
                Some((original, before))
            } else {
                None
            };
            open_test_project(&ctx, &mut app, &file);
            finish(&ctx, &mut app);
            if supplied {
                assert!(
                    app.source_ui.repository.is_some(),
                    "normal file-open restores existing source context"
                );
            }
            let baseline = app.host.document.editor.project.clone();
            button(&ctx, &mut app, "Concept mass");
            let view = app.workbench.panel_rects["view.canvas"];
            let at = app.screen(view, Point::new(rectangles[0][0][0], rectangles[0][0][1]));
            assert!(view.contains(at));
            layers_click(&ctx, &mut app, at);
            layers_frame_output(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
            assert_eq!(app.host.document.editor.project, baseline);
            for rect in &rectangles {
                button(&ctx, &mut app, "Concept mass");
                let view = app.workbench.panel_rects["view.canvas"];
                for [x, y] in rect {
                    let p = app.screen(view, Point::new(*x, *y));
                    assert!(
                        view.contains(p),
                        "authored point {x},{y} projected {p:?}, canvas {view:?}; center={:?}, scale={}",
                        app.plan_center,
                        app.plan_scale
                    );
                    layers_click(&ctx, &mut app, p);
                }
            }
            let drawn = app.host.document.editor.project.clone();
            assert_eq!(drawn.elements.len(), baseline.elements.len() + 2);
            assert_eq!(
                &drawn.elements[..baseline.elements.len()],
                &baseline.elements
            );
            let seeds = drawn.elements[baseline.elements.len()..].to_vec();
            assert!(
                seeds
                    .iter()
                    .all(|e| matches!(e.shape, Shape::ConceptMass { .. }))
            );
            button(&ctx, &mut app, "Select");
            for (i, e) in seeds.iter().enumerate() {
                let Shape::ConceptMass { spec } = &e.shape else {
                    unreachable!()
                };
                let center = spec.outline.iter().fold(Point::default(), |p, q| {
                    Point::new(p.x + q.x / 4., p.y + q.y / 4.)
                });
                let view = app.workbench.panel_rects["view.canvas"];
                let p = app.screen(view, center);
                if i == 0 {
                    layers_click(&ctx, &mut app, p);
                } else {
                    shift_click(&ctx, &mut app, p);
                }
            }
            assert_eq!(
                app.host.document.editor.selection_ids(),
                seeds.iter().map(|e| e.id).collect::<Vec<_>>(),
                "selection diagnostic: language={language:?}, tool={:?}, text={}, focus={:?}, popup={}, draft={}, hover={:?}, pointer={:?}, view={:?}, help={}, site={}, preview={}, settings={}",
                app.host.document.editor.tool,
                ctx.text_edit_focused(),
                ctx.memory(|m| m.focused()),
                egui::Popup::is_any_open(&ctx),
                app.host.document.editor.model_input_active(),
                app.canvas_hover_id(Id::new("plan_canvas_selection")),
                ctx.input(|i| i.pointer.hover_pos()),
                app.workbench.panel_rects.get("view.canvas"),
                app.help,
                app.site_dialog.is_some(),
                app.process_ui.preview.is_some(),
                app.workbench.configuration_open
            );
            button(&ctx, &mut app, "Project");
            button(&ctx, &mut app, "Explore space options…");
            assert!(
                app.process_ui.use_selection,
                "carry current canvas selection automatically"
            );
            let question = ctx
                .data(|d| d.get_temp::<Id>(Id::new("exploration_question_field")))
                .unwrap();
            action(&ctx, &mut app, question.accesskit_id(), Action::Focus, None);
            layers_frame_output(&ctx,&mut app,vec![Event::Text("Compare two editable mass-height directions within the declared control range; geometry is a design assumption, not a capacity or approval claim".into()),key(Key::Tab,Modifiers::NONE)]);
            button(&ctx, &mut app, "Preview two editable candidates");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            assert_eq!(app.host.document.editor.project, drawn);
            assert_eq!(
                app.process_ui.preview.as_ref().unwrap().label,
                "Create editable mass candidates"
            );
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, drawn);
            button(&ctx, &mut app, "Preview two editable candidates");
            button(&ctx, &mut app, "Confirm design change");
            let p = &app.host.document.editor.project;
            let task = p
                .design_process
                .task()
                .expect("mass task through normal controls")
                .clone();
            assert!(task.is_mass() && task.intent.is_none());
            assert_eq!(task.candidates[0].initial_mass_ids.len(), 2);
            assert_eq!(task.candidates[1].initial_mass_ids.len(), 2);
            assert_eq!(&p.elements[..baseline.elements.len()], &baseline.elements);
            let mut expected = 0.;
            for e in &seeds {
                let Shape::ConceptMass { spec } = &e.shape else {
                    unreachable!()
                };
                expected += spec.area();
            }
            let comparison = task.compare(p).unwrap();
            for c in &comparison.candidates {
                assert!(
                    (c.mass_metrics.as_ref().unwrap().footprint_sum_m2 - expected).abs() < 1e-7
                );
            }
            let before_edit = app.host.document.editor.project.clone();
            if app.process_ui.selected_space.is_none() {
                button(&ctx, &mut app, "Select a mass on the canvas");
                button(&ctx, &mut app, &seeds[0].name);
            }
            let edited_id = app
                .process_ui
                .selected_space
                .expect("editable current mass carried into comparison");
            let height_id = ctx
                .data(|d| d.get_temp::<Id>(Id::new("candidate_height_field")))
                .unwrap();
            action(
                &ctx,
                &mut app,
                height_id.accesskit_id(),
                Action::SetValue,
                Some(ActionData::NumericValue(6.)),
            );
            assert_eq!(app.process_ui.height, 6.);
            button(&ctx, &mut app, "Preview candidate edit");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            assert_eq!(app.host.document.editor.project, before_edit);
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, before_edit);
            button(&ctx, &mut app, "Preview candidate edit");
            button(&ctx, &mut app, "Confirm design change");
            let changed = app.host.document.editor.project.clone();
            assert!(
                matches!(&changed.elements.iter().find(|e|e.id==edited_id).unwrap().shape,Shape::ConceptMass{spec} if spec.height==6.)
            );
            for e in &before_edit.elements {
                if e.id != edited_id {
                    assert_eq!(changed.elements.iter().find(|x| x.id == e.id), Some(e));
                }
            }
            // Undo/Redo uses the normal history route while preserving the task.
            button(&ctx, &mut app, "Continue in canvas");
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(app.host.document.editor.project, before_edit);
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::ALT)],
            );
            assert_eq!(app.host.document.editor.project, changed);
            button(&ctx, &mut app, "Compare options…");
            let comparison = task.compare(&changed).unwrap();
            assert!(comparison.different);
            let chosen = ctx
                .data(|d| {
                    d.get_temp::<Id>(Id::new(("comparison_choice", task.candidates[0].option)))
                })
                .unwrap();
            action(&ctx, &mut app, chosen.accesskit_id(), Action::Click, None);
            button(&ctx, &mut app, "Preview choice and saved versions");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            button(&ctx, &mut app, "Confirm design change");
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .design_process
                    .decisions
                    .last()
                    .unwrap()
                    .option,
                task.candidates[0].option
            );
            button(&ctx, &mut app, "Continue in canvas");
            layers_frame_output(&ctx, &mut app, vec![key(Key::S, Modifiers::COMMAND)]);
            for _ in 0..3 {
                layers_frame_output(&ctx, &mut app, vec![]);
            }
            if app.file_action.is_some() {
                button(&ctx, &mut app, "Save");
                for _ in 0..3 {
                    layers_frame_output(&ctx, &mut app, vec![]);
                }
            }
            assert!(app.file_action.is_none());
            let saved = Editor::from_stored_document(&file).unwrap();
            assert_eq!(saved.project, app.host.document.editor.project);
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&std::fs::read(&file).unwrap())
                    .unwrap()["schema_version"],
                25
            );
            let (fresh_ctx, mut fresh) = setup();
            fresh_ctx.enable_accesskit();
            fresh.language = language;
            fresh_ctx.global_style_mut(|s| s.animation_time = 0.);
            fresh.initialize_source_connections(&root);
            fresh
                .host
                .configure_operation_journal(Some(root.join("fresh-journal")))
                .unwrap();
            open_test_project(&fresh_ctx, &mut fresh, &file);
            finish(&fresh_ctx, &mut fresh);
            assert_eq!(fresh.host.document.editor.project, saved.project);
            if supplied {
                assert!(
                    fresh.source_ui.repository.is_some(),
                    "source context restored after fresh app file-open"
                );
            }
            button(&fresh_ctx, &mut fresh, "Compare options…");
            assert!(fresh.process_ui.open);
            assert_eq!(
                fresh
                    .host
                    .document
                    .editor
                    .project
                    .design_process
                    .current_task,
                Some(task.id.clone())
            );
            drop(fresh);
            if let (Some(input), Some(bytes)) = (&input, &input_bytes) {
                assert_eq!(std::fs::read(input).unwrap(), *bytes);
            }
            if let Some((source, bytes)) = source_manifest {
                assert_eq!(std::fs::read(source.join("project.json")).unwrap(), bytes);
            }
            std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"entry":"normal product events","task":task.id,"source_ids":task.source_mass_ids,"candidates":task.candidates,"input_protected":true,"model":file,"native_format":25,"native_os":"not_run","real_ai":"not_run","later_combined_checks_choice_feedback":"not_yet_in_this_script"})).unwrap()).unwrap();
            drop(app);
            if !supplied {
                std::fs::remove_dir_all(parent).unwrap();
            }
        }
    }
}
