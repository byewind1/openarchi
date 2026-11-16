// Public shared-command fixture; these checks do not claim actual Agent or OS execution.
mod object_nearby_product_path {
    use super::*;
    use crate::{
        design_process::parametric::{self, Definition, Parameters},
        evidence::{
            EvidenceEdit, Target,
            topic_tasks::{Event as TaskEvent, Mode},
            topics,
        },
    };
    pub(super) fn fixture() -> (Editor, String, u64) {
        fixture_with_question("Study the teaching courtyard, preserve existing work")
    }
    fn fixture_with_question(question: &str) -> (Editor, String, u64) {
        let mut editor = Editor::new(crate::storage::startup_example());
        let source = editor
            .project
            .elements
            .iter()
            .find(|e| matches!(e.shape, Shape::Zone { .. }))
            .unwrap()
            .id;
        let topic = storage::new_document_identity();
        let request = storage::new_document_identity();
        let study = storage::new_document_identity();
        let exchange = topics::capture(
            &editor.project,
            &[Target::Element { id: source }],
            question.into(),
        )
        .unwrap();
        let sources = exchange.objects.iter().map(|o| o.link.clone()).collect();
        let site_sha256 = crate::automation_journal::canonical_hash(&editor.project.site).unwrap();
        editor
            .apply(crate::command::ModelCommand::Evidence {
                edit: EvidenceEdit::SubmitTopicTask {
                    id: topic.clone(),
                    exchange,
                    expected_previous: None,
                    request_id: request.clone(),
                    mode: Mode::CreateTwoWing {
                        task_id: study.clone(),
                        story: 0,
                        layer: 0,
                        site_sha256: site_sha256.clone(),
                    },
                },
            })
            .unwrap();
        let expected = crate::automation_journal::canonical_hash(
            editor.project.evidence.topics.last().unwrap(),
        )
        .unwrap();
        editor
            .apply(crate::command::ModelCommand::Evidence {
                edit: EvidenceEdit::TopicTaskEvent {
                    id: topic.clone(),
                    exchange_index: 0,
                    expected_topic_sha256: expected,
                    request_id: request,
                    event: TaskEvent::Claim {
                        agent_label: "Public fixture client; not AI".into(),
                    },
                },
            })
            .unwrap();
        let a = Parameters {
            origin_mm: [30000, 30000],
            court_width_mm: 20000,
            court_depth_mm: 14000,
            wing_depth_mm: 6000,
            storeys: 2,
            floor_height_mm: 3300,
        };
        let mut b = a.clone();
        b.wing_depth_mm = 7000;
        let change = parametric::prepare(
            &editor,
            &study,
            "Generated teaching study".into(),
            question.into(),
            Definition {
                rule_version: 1,
                candidates: [a, b],
                site_sha256,
                sources,
                assumptions:
                    "Public concept dimensions only; measured units, capacity and approval unknown"
                        .into(),
            },
            0,
            0,
        )
        .unwrap();
        editor
            .commit_change("fixture-created-study", &change)
            .unwrap();
        let expected = crate::automation_journal::canonical_hash(
            editor.project.evidence.topics.last().unwrap(),
        )
        .unwrap();
        editor.apply(crate::command::ModelCommand::Evidence { edit: EvidenceEdit::RecordTopicReply { id: topic.clone(), exchange_index:0, expected_topic_sha256:expected, reply: topics::AgentReply { id:storage::new_document_identity(),agent_label:"Public fixture client; not AI".into(),text:"Long historical explanation; declared assumptions are not survey or approval. ".repeat(130),claims:vec![crate::design_process::intent::Claim { kind:crate::design_process::intent::ClaimKind::Observation,text:"Fixture candidates exist; this is not AI execution".into() }] } } }).unwrap();
        let member =
            editor.project.design_process.task().unwrap().candidates[0].initial_mass_ids[0];
        editor.set_selection(vec![member]);
        (editor, topic, member)
    }
    fn offset(ctx: &Context) -> f32 {
        ctx.data(|d| d.get_temp::<f32>(Id::new("inspector_scroll_offset")))
            .unwrap()
    }
    fn frames(ctx: &Context, app: &mut AtelierApp) {
        for _ in 0..8 {
            layers_frame_output(ctx, app, vec![]);
        }
    }
    fn scroll_down(ctx: &Context, app: &mut AtelierApp) {
        frames(ctx, app);
        for _ in 0..5 {
            task_scroll(ctx, app, -400.);
        }
        frames(ctx, app);
        assert!(
            offset(ctx) > 50.,
            "fixture must exercise an actual previously scrolled inspector"
        );
    }
    fn fixed_parameter_button(ctx: &Context, app: &mut AtelierApp, label: &str) {
        frames(ctx, app);
        let out = layers_frame_output(ctx, app, vec![]);
        let footer = ctx
            .data(|d| d.get_temp::<Rect>(Id::new("parametric_actions_rect")))
            .unwrap();
        let panel = app.workbench.panel_rects["workspace.navigator"];
        assert!(
            panel.contains_rect(footer),
            "fixed actions outside inspector"
        );
        let at = text_center(&out.shapes, app.language.translate(label)).unwrap();
        assert!(
            footer.contains(at),
            "action must be painted in the fixed footer"
        );
        assert!(out.shapes.iter().any(|s| s.clip_rect.contains(at)
            && matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == app.language.translate(label))));
        layers_click(ctx, app, at);
    }
    #[test]
    fn assistant_reserves_editing_space_without_selection_and_preserves_draft_bilingual() {
        fn size_frames(ctx: &Context, app: &mut AtelierApp, size: Vec2) -> egui::FullOutput {
            let mut output = egui::FullOutput::default();
            for _ in 0..8 {
                output = ctx.run_ui(
                    RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |ui| app.workspace(ui),
                );
            }
            output
        }
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            let (mut editor, _, _) = fixture();
            editor.set_selection(vec![]);
            app.host.document.editor = editor;
            let before = app.host.document.editor.project.clone();
            let context = app.host.context();
            button(&ctx, &mut app, "Adjust generated wings");
            let parameters = app
                .research_ui
                .parameters
                .as_ref()
                .unwrap()
                .definition
                .clone();
            for (size, minimum) in [(vec2(1400., 900.), 240.), (vec2(1200., 760.), 150.)] {
                let out = size_frames(&ctx, &mut app, size);
                let body = ctx
                    .data(|d| d.get_temp::<Rect>(Id::new("inspector_scroll_rect")))
                    .unwrap();
                let footer = ctx
                    .data(|d| d.get_temp::<Rect>(Id::new("parametric_actions_rect")))
                    .unwrap();
                let panel = app.workbench.panel_rects["workspace.navigator"];
                eprintln!(
                    "ASSISTANT_SPACE language={language:?} viewport={size:?} body={body:?} footer={footer:?}"
                );
                assert!(
                    body.height() >= minimum,
                    "usable editing height at {size:?}: body={body:?}"
                );
                assert!(panel.contains_rect(body) && panel.contains_rect(footer));
                assert!(body.max.y <= footer.min.y);
                for label in [
                    "NAVIGATOR",
                    "Preview two-wing parameters",
                    "Cancel parameter draft",
                ] {
                    assert!(
                        out.shapes.iter().any(|s| {
                            let mut shapes = vec![];
                            collect_shapes(std::slice::from_ref(s), &mut shapes);
                            shapes.iter().any(|shape| match shape {
                                egui::Shape::Text(t) => {
                                    t.galley.text() == language.translate(label)
                                        && s.clip_rect.contains(t.pos + t.galley.size() / 2.)
                                }
                                _ => false,
                            })
                        }),
                        "visible original entry {label} at {size:?}"
                    );
                }
                assert_eq!(app.host.document.editor.project, before);
                assert_eq!(app.host.context(), context);
            }
            button(&ctx, &mut app, "NAVIGATOR");
            frames(&ctx, &mut app);
            let out = layers_frame_output(&ctx, &mut app, vec![]);
            let tree = ctx
                .data(|d| d.get_temp::<Rect>(Id::new("navigator_tree_rect")))
                .unwrap();
            let at = text_center(&out.shapes, language.translate("Stories")).unwrap();
            assert!(
                tree.contains(at),
                "original navigation tree remains available"
            );
            layers_frame_output(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(tree.center()),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: vec2(0., -200.),
                        modifiers: Modifiers::NONE,
                        phase: egui::TouchPhase::Move,
                    },
                ],
            );
            frames(&ctx, &mut app);
            assert!(
                ctx.data(|d| d.get_temp::<f32>(Id::new("navigator_tree_offset")))
                    .unwrap()
                    > 0.,
                "original tree scroll remains usable"
            );
            button(&ctx, &mut app, "NAVIGATOR");
            frames(&ctx, &mut app);
            assert!(
                ctx.data(|d| d.get_temp::<Rect>(Id::new("inspector_scroll_rect")))
                    .unwrap()
                    .height()
                    >= 240.
            );
            for tab in ["Library", "Properties", "AI Assistant"] {
                button(&ctx, &mut app, tab);
                frames(&ctx, &mut app);
                assert_eq!(
                    app.research_ui.parameters.as_ref().unwrap().definition,
                    parameters
                );
                assert_eq!(app.host.document.editor.project, before);
                assert_eq!(app.host.context(), context);
            }
            fixed_parameter_button(&ctx, &mut app, "Cancel parameter draft");
            assert!(app.research_ui.parameters.is_none());
            assert_eq!(app.host.document.editor.project, before);
        }
    }
    #[test]
    fn candidate_bar_parameters_fixed_actions_and_reopen_bilingual() {
        nearby_parameters(false);
    }
    #[test]
    #[ignore = "protected current project input and unused output required"]
    fn protected_candidate_bar_parameters_reopen_bilingual() {
        nearby_parameters(true);
    }
    fn nearby_parameters(supplied: bool) {
        let root = if supplied {
            std::path::PathBuf::from(std::env::var_os("OPENARCHI_NEAR_PARAMETER_OUTPUT").unwrap())
        } else {
            std::env::temp_dir().join(format!(
                "oa-near-parameters-{}",
                storage::new_document_identity()
            ))
        };
        std::fs::create_dir(&root).expect("new protected output; do not replay");
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            let journal = root.join(if language == UiLanguage::English {
                "journal-en"
            } else {
                "journal-zh"
            });
            app.host.configure_operation_journal(Some(journal)).unwrap();
            app.host.document.editor = if supplied {
                Editor::from_stored_document(std::path::Path::new(
                    &std::env::var_os("OPENARCHI_NEAR_PARAMETER_INPUT").unwrap(),
                ))
                .unwrap()
            } else {
                fixture_with_question(&"Preserve existing work, compare courtyard sharing and explain unknown capacity. ".repeat(20)).0
            };
            let original = app.host.document.editor.project.clone();
            let task = original.design_process.task().unwrap().clone();
            let option = original.plan_options.active_design_option.unwrap();
            let initial = parametric::current_parameters(&original, &task.id, option).unwrap();
            let selection = app.host.document.editor.selection_ids();
            button(&ctx, &mut app, "AI Assistant");
            scroll_down(&ctx, &mut app);
            button(&ctx, &mut app, "Properties");
            app.show_right = false; // Workspace precondition, never model/task mutation.
            frames(&ctx, &mut app);
            reveal_manual_control(&ctx, &mut app, "Adjust generated wings");
            let out = layers_frame_output(&ctx, &mut app, vec![]);
            let at =
                text_center(&out.shapes, language.translate("Adjust generated wings")).unwrap();
            assert!(out.shapes.iter().any(|s| s.clip_rect.contains(at)
                && matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == language.translate("Adjust generated wings"))));
            layers_click(&ctx, &mut app, at);
            frames(&ctx, &mut app);
            assert!(app.show_right && app.tab == Tab::Assistant);
            assert_eq!(
                offset(&ctx),
                0.,
                "explicit entry must reveal parameters once"
            );
            assert_eq!(app.host.document.editor.project, original);
            assert_eq!(app.host.document.editor.selection_ids(), selection);
            assert!(!app.host.operations.borrow().enabled());
            let d = app.research_ui.parameters.as_ref().unwrap();
            assert_eq!(d.definition.candidates[0], initial);
            assert_eq!(d.question, task.question);
            // Dismiss the task tooltip after leaving the task controls.
            layers_frame_output(&ctx, &mut app, vec![Event::PointerGone]);
            frames(&ctx, &mut app);
            // A long persisted question is available on demand, not a wall before inputs.
            let out = layers_frame_output(&ctx, &mut app, vec![]);
            assert!(
                !out.shapes.iter().any(|s| {
                    text_center(std::slice::from_ref(s), &task.question)
                        .is_some_and(|p| s.clip_rect.contains(p))
                }),
                "long question is not visible ahead of the parameter inputs"
            );
            fixed_parameter_button(&ctx, &mut app, "Cancel parameter draft");
            assert!(app.research_ui.parameters.is_none());
            assert_eq!(app.host.document.editor.project, original);
            button(&ctx, &mut app, "Adjust generated wings");
            // Edit an actually visible numeric input through the normal AX value action.
            for _ in 0..100 {
                frames(&ctx, &mut app);
                let id = ctx
                    .data(|d| d.get_temp::<Id>(Id::new(("parametric", 0usize, "Court depth (mm)"))))
                    .unwrap();
                let rect = ctx.read_response(id).unwrap().rect;
                let body = ctx
                    .data(|d| d.get_temp::<Rect>(Id::new("inspector_scroll_rect")))
                    .unwrap();
                assert!(
                    body.height() >= rect.height(),
                    "a complete numeric control must fit above the actions: {body:?} / {rect:?}"
                );
                if body.contains_rect(rect) {
                    number(
                        &ctx,
                        &mut app,
                        0,
                        "Court depth (mm)",
                        (initial.court_depth_mm + 500) as f64,
                    );
                    break;
                }
                task_scroll(&ctx, &mut app, -8.);
            }
            assert_eq!(
                app.research_ui
                    .parameters
                    .as_ref()
                    .unwrap()
                    .definition
                    .candidates[0]
                    .court_depth_mm,
                initial.court_depth_mm + 500
            );
            fixed_parameter_button(&ctx, &mut app, "Preview two-wing parameters");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            assert_eq!(app.host.document.editor.project, original);
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, original);
            // Cancelling a known preview retains the current editable draft.
            assert_eq!(
                app.research_ui
                    .parameters
                    .as_ref()
                    .unwrap()
                    .definition
                    .candidates[0]
                    .court_depth_mm,
                initial.court_depth_mm + 500
            );
            fixed_parameter_button(&ctx, &mut app, "Preview two-wing parameters");
            assert!(
                app.process_ui.preview.is_some(),
                "second preview: {}",
                app.process_ui.error
            );
            button(&ctx, &mut app, "Confirm design change");
            assert!(
                app.process_ui.preview.is_none(),
                "confirm: {}",
                app.process_ui.error
            );
            let edited = app.host.document.editor.project.clone();
            assert_eq!(
                parametric::current_parameters(&edited, &task.id, option)
                    .unwrap()
                    .court_depth_mm,
                initial.court_depth_mm + 500
            );
            let members = &task
                .candidates
                .iter()
                .find(|c| c.option == option)
                .unwrap()
                .initial_mass_ids;
            for e in &original.elements {
                if !members.contains(&e.id) {
                    assert_eq!(edited.elements.iter().find(|v| v.id == e.id), Some(e));
                }
            }
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(app.host.document.editor.project.elements, original.elements);
            assert_eq!(
                parametric::current_parameters(&app.host.document.editor.project, &task.id, option)
                    .unwrap(),
                initial
            );
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            assert_eq!(app.host.document.editor.project, edited);
            let path = root.join(if language == UiLanguage::English {
                "continued-en.openarchi.json"
            } else {
                "continued-zh.openarchi.json"
            });
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let old = app.file_dialog_path.clone().unwrap();
            field(&ctx, &mut app, &old, &path.display().to_string());
            button(&ctx, &mut app, "Save");
            assert!(
                path.exists(),
                "normal SaveAs must produce the editable project"
            );
            let (new_ctx, mut fresh) = setup();
            new_ctx.enable_accesskit();
            fresh.language = language;
            open(&new_ctx, &mut fresh, &path);
            button(&new_ctx, &mut fresh, "Adjust generated wings");
            frames(&new_ctx, &mut fresh);
            assert_eq!(
                fresh
                    .research_ui
                    .parameters
                    .as_ref()
                    .unwrap()
                    .definition
                    .candidates[0]
                    .court_depth_mm,
                initial.court_depth_mm + 500
            );
            assert!(!fresh.host.operations.borrow().enabled());
            fixed_parameter_button(&new_ctx, &mut fresh, "Cancel parameter draft");
            // An unfinished question cannot be replaced by the candidate-bar action.
            fresh.host.document.editor.set_selection(vec![members[0]]);
            button(&new_ctx, &mut fresh, "Discuss selected objects");
            let question_before = fresh.research_ui.draft.as_ref().unwrap().exchange.clone();
            reveal_manual_control(&new_ctx, &mut fresh, "Adjust generated wings");
            let out = layers_frame_output(&new_ctx, &mut fresh, vec![]);
            let (id, node) = out
                .platform_output
                .accesskit_update
                .unwrap()
                .nodes
                .into_iter()
                .find(|(_, n)| n.label() == Some(language.translate("Adjust generated wings")))
                .unwrap();
            assert!(node.is_disabled());
            action(&new_ctx, &mut fresh, id, Action::Click, None);
            assert!(fresh.research_ui.parameters.is_none());
            assert_eq!(
                fresh.research_ui.draft.as_ref().unwrap().exchange,
                question_before
            );
            assert!(!fresh.host.operations.borrow().enabled());
        }
    }
    #[test]
    fn numeric_draft_hides_invalid_or_stale_geometry_and_preserves_inserted_edit() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            let (editor, _, member) = fixture();
            app.host.document.editor = editor;
            button(&ctx, &mut app, "2D Plan");
            let before = app.host.document.editor.project.clone();
            button(&ctx, &mut app, "Adjust generated wings");
            let label = language.translate("Unconfirmed two-wing draft");
            let visible = |out: &egui::FullOutput| {
                out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text().starts_with(label)))
            };
            number(&ctx, &mut app, 0, "Origin X (mm)", 1_000_000_001.);
            assert!(!visible(&layers_frame_output(&ctx, &mut app, vec![])));
            assert_eq!(app.host.document.editor.project, before);
            number(&ctx, &mut app, 0, "Origin X (mm)", 31000.);
            assert!(visible(&layers_frame_output(&ctx, &mut app, vec![])));
            assert_eq!(app.host.document.editor.project, before);
            // Simulated independent manual writer through the actual shared edit
            // transaction, not direct project assignment or an AI-authority grant.
            let change = crate::design_process::service::prepare_edit(
                &app.host.document.editor,
                member,
                Point::new(0.1, 0.),
                None,
            )
            .unwrap();
            app.host
                .document
                .editor
                .commit_change(&storage::new_document_identity(), &change)
                .unwrap();
            let inserted = app.host.document.editor.project.clone();
            assert_ne!(inserted.elements, before.elements);
            assert!(!visible(&layers_frame_output(&ctx, &mut app, vec![])));
            fixed_parameter_button(&ctx, &mut app, "Preview two-wing parameters");
            assert!(app.process_ui.preview.is_none());
            assert!(app.process_ui.error.contains("Discussion inputs changed"));
            fixed_parameter_button(&ctx, &mut app, "Cancel parameter draft");
            assert_eq!(app.host.document.editor.project, inserted);
            assert!(!app.host.operations.borrow().enabled());
        }
    }

    #[test]
    fn numeric_two_wing_draft_is_visible_without_mutation_then_commits_and_reopens_bilingual() {
        numeric_preview_path(false);
    }
    #[test]
    #[ignore = "explicit protected parametric project and new output required"]
    fn protected_numeric_two_wing_preview_continues_saved_candidate_bilingual() {
        numeric_preview_path(true);
    }
    fn numeric_preview_path(supplied: bool) {
        let root = if supplied {
            std::path::PathBuf::from(std::env::var_os("OPENARCHI_LIVE_PREVIEW_OUTPUT").unwrap())
        } else {
            std::env::temp_dir().join(format!(
                "oa-live-parameters-{}",
                storage::new_document_identity()
            ))
        };
        std::fs::create_dir(&root).expect("new output, inspect original result instead of replay");
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let dir = root.join(if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            });
            std::fs::create_dir(&dir).unwrap();
            let editor = if supplied {
                Editor::from_stored_document(std::path::Path::new(
                    &std::env::var_os("OPENARCHI_LIVE_PREVIEW_INPUT").unwrap(),
                ))
                .unwrap()
            } else {
                fixture().0
            };
            let input = dir.join("protected.openarchi.json");
            storage::save_document(&editor.project, &input, editor.project_identity()).unwrap();
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            app.host
                .configure_operation_journal(Some(dir.join("journal")))
                .unwrap();
            open(&ctx, &mut app, &input);
            button(&ctx, &mut app, "2D Plan");
            let original = app.host.document.editor.project.clone();
            let task = original.design_process.task().unwrap().clone();
            let option = original.plan_options.active_design_option.unwrap();
            let initial = parametric::current_parameters(&original, &task.id, option).unwrap();
            let members = task
                .candidates
                .iter()
                .find(|c| c.option == option)
                .unwrap()
                .initial_mass_ids
                .clone();
            let mut proposed = initial.clone();
            proposed.origin_mm[0] += 750;
            proposed.origin_mm[1] -= 250;
            proposed.court_width_mm += 1000;
            proposed.court_depth_mm += 500;
            proposed.wing_depth_mm += 250;
            proposed.storeys += 1;
            proposed.floor_height_mm += 100;
            let values = [
                ("Origin X (mm)", proposed.origin_mm[0] as f64),
                ("Origin Y (mm)", proposed.origin_mm[1] as f64),
                ("Court width (mm)", proposed.court_width_mm as f64),
                ("Court depth (mm)", proposed.court_depth_mm as f64),
                ("Wing depth (mm)", proposed.wing_depth_mm as f64),
                ("Concept storeys", proposed.storeys as f64),
                ("Floor height (mm)", proposed.floor_height_mm as f64),
            ];
            button(&ctx, &mut app, "Adjust generated wings");
            for (label, value) in values {
                for attempt in 0..160 {
                    frames(&ctx, &mut app);
                    let id = ctx
                        .data(|d| d.get_temp::<Id>(Id::new(("parametric", 0usize, label))))
                        .unwrap();
                    let rect = ctx.read_response(id).unwrap().rect;
                    let body = ctx
                        .data(|d| d.get_temp::<Rect>(Id::new("inspector_scroll_rect")))
                        .unwrap();
                    if body.contains_rect(rect) {
                        number(&ctx, &mut app, 0, label, value);
                        break;
                    }
                    assert!(attempt < 159, "parameter {label} cannot be reached");
                    task_scroll(
                        &ctx,
                        &mut app,
                        if rect.top() < body.top() { 12. } else { -12. },
                    );
                }
            }
            let out = layers_frame_output(&ctx, &mut app, vec![]);
            let draft_label = language.translate("Unconfirmed two-wing draft");
            assert!(out.shapes.iter().any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text().starts_with(draft_label))), "draft must be visible on the actual canvas");
            assert_eq!(app.host.document.editor.project, original);
            assert_eq!(
                app.research_ui
                    .parameters
                    .as_ref()
                    .unwrap()
                    .definition
                    .candidates[0],
                proposed
            );
            // Independent rectangle equations, not the engine used to draw them.
            let canvas = out
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Rect(p)
                        if p.fill == Color32::from_rgba_unmultiplied(250, 250, 248, 255)
                            && p.rect.width() > 200.
                            && p.rect.height() > 200. =>
                    {
                        Some(p.rect)
                    }
                    _ => None,
                })
                .expect("normal plan canvas");
            let [x, y] = proposed.origin_mm.map(|v| v as f64 / 1000.);
            let w = proposed.court_width_mm as f64 / 1000.;
            let d = proposed.court_depth_mm as f64 / 1000.;
            let t = proposed.wing_depth_mm as f64 / 1000.;
            let expected = [
                vec![
                    (x, y + d),
                    (x + w + t, y + d),
                    (x + w + t, y + d + t),
                    (x, y + d + t),
                ],
                vec![
                    (x + w, y),
                    (x + w + t, y),
                    (x + w + t, y + d),
                    (x + w, y + d),
                ],
            ];
            for ring in &expected {
                let points = ring
                    .iter()
                    .map(|(x, y)| app.screen(canvas, Point::new(*x, *y)))
                    .collect::<Vec<_>>();
                assert!(
                    out.shapes.iter().any(
                        |s| matches!(&s.shape,egui::Shape::Path(p) if p.closed && p.points==points)
                    ),
                    "draft ring in same world scale, not a detached image"
                );
            }
            fixed_parameter_button(&ctx, &mut app, "Preview two-wing parameters");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            assert_eq!(app.host.document.editor.project, original);
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, original);
            fixed_parameter_button(&ctx, &mut app, "Preview two-wing parameters");
            button(&ctx, &mut app, "Confirm design change");
            assert!(app.process_ui.preview.is_none(), "{}", app.process_ui.error);
            let edited = app.host.document.editor.project.clone();
            assert_eq!(
                parametric::current_parameters(&edited, &task.id, option).unwrap(),
                proposed
            );
            let height = proposed.storeys as f64 * proposed.floor_height_mm as f64 / 1000.;
            for (id, ring) in members.iter().zip(&expected) {
                let e = edited.elements.iter().find(|e| e.id == *id).unwrap();
                let Shape::ConceptMass { spec } = &e.shape else {
                    panic!("mapped mass")
                };
                assert_eq!(
                    spec.outline,
                    ring.iter()
                        .map(|(x, y)| Point::new(*x, *y))
                        .collect::<Vec<_>>()
                );
                assert_eq!(spec.height, height);
            }
            let metrics = task
                .compare(&edited)
                .unwrap()
                .candidates
                .into_iter()
                .find(|c| c.option == option)
                .unwrap()
                .mass_metrics
                .unwrap();
            let area = (w + t) * t + t * d;
            assert!((metrics.footprint_sum_m2 - area).abs() < 1e-8);
            assert!((metrics.envelope_volume_sum_m3 - area * height).abs() < 1e-7);
            for e in &original.elements {
                if !members.contains(&e.id) {
                    assert_eq!(edited.elements.iter().find(|v| v.id == e.id), Some(e));
                }
            }
            assert_eq!(edited.evidence, original.evidence);
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(app.host.document.editor.project.elements, original.elements);
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            assert_eq!(app.host.document.editor.project, edited);
            let output = dir.join("continued.openarchi.json");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let old = app.file_dialog_path.clone().unwrap();
            field(&ctx, &mut app, &old, &output.display().to_string());
            button(&ctx, &mut app, "Save");
            let (fresh_ctx, mut fresh) = setup();
            fresh_ctx.enable_accesskit();
            fresh.language = language;
            open(&fresh_ctx, &mut fresh, &output);
            let verified_assets = |mut project: Project| {
                use sha2::{Digest, Sha256};
                for page in &mut project.drawings.pages {
                    let path = if page.path.is_absolute() {
                        page.path.clone()
                    } else {
                        dir.join(&page.path)
                    };
                    assert_eq!(
                        format!("{:x}", Sha256::digest(std::fs::read(path).unwrap())),
                        page.sha256
                    );
                    page.path = std::path::PathBuf::from(&page.sha256);
                }
                project
            };
            assert_eq!(
                verified_assets(fresh.host.document.editor.project.clone()),
                verified_assets(edited)
            );
            button(&fresh_ctx, &mut fresh, "Adjust generated wings");
            assert_eq!(
                fresh
                    .research_ui
                    .parameters
                    .as_ref()
                    .unwrap()
                    .definition
                    .candidates[0],
                proposed
            );
            fixed_parameter_button(&fresh_ctx, &mut fresh, "Cancel parameter draft");
            assert!(!fresh.host.operations.borrow().enabled());
            let empty = layers_frame_output(&fresh_ctx, &mut fresh, vec![]);
            assert!(!empty.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text().starts_with(draft_label))));
            std::fs::write(dir.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"output":output,"old_parameters":initial,"parameters":proposed,"members":members,"footprint_m2":area,"envelope_height_m":height,"envelope_volume_m3":area*height,"normal_controls_and_same_canvas_preview":true,"other_elements_and_topics_equal":true,"native_OS":"not_run","actual_AI":"not_run"})).unwrap()).unwrap();
        }
    }

    #[test]
    fn explicit_discussion_resets_old_scroll_once_and_keeps_frozen_question() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            let (editor, _, member) = fixture();
            app.host.document.editor = editor;
            button(&ctx, &mut app, "AI Assistant");
            button(
                &ctx,
                &mut app,
                "Study the teaching courtyard, preserve existing work",
            );
            scroll_down(&ctx, &mut app);
            let before = app.host.document.editor.project.clone();
            button(&ctx, &mut app, "Discuss selected objects");
            frames(&ctx, &mut app);
            assert_eq!(offset(&ctx), 0.);
            // Typing may then scroll the caret into view; the explicit entry itself resets.
            assert_discussion_layout_and_type(&ctx, &mut app);
            assert_eq!(
                app.research_ui.draft.as_ref().unwrap().exchange.objects[0]
                    .link
                    .target,
                Target::Element { id: member }
            );
            assert_eq!(app.host.document.editor.project, before);
            assert!(!app.host.operations.borrow().enabled());
            // Scroll an active draft and type again: subsequent frames must not jump back.
            scroll_down(&ctx, &mut app);
            let previous = offset(&ctx);
            frames(&ctx, &mut app);
            assert!(
                offset(&ctx) >= previous - 1.,
                "later frames must not reset user scrolling; wheel smoothing may continue"
            );
            let question = app
                .research_ui
                .draft
                .as_ref()
                .unwrap()
                .exchange
                .question
                .clone();
            button(&ctx, &mut app, "Properties");
            frames(&ctx, &mut app);
            assert_eq!(offset(&ctx), 0.);
            button(&ctx, &mut app, "AI Assistant");
            frames(&ctx, &mut app);
            assert!(offset(&ctx) > 50.);
            assert_eq!(
                app.research_ui.draft.as_ref().unwrap().exchange.question,
                question
            );
        }
    }
    #[test]
    fn saved_topic_returns_to_live_generated_candidate_before_long_reply_without_authorizing() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (editor, topic, _) = fixture();
            let dir = std::env::temp_dir()
                .join(format!("oa-nearby-{}", storage::new_document_identity()));
            std::fs::create_dir(&dir).unwrap();
            let file = dir.join("saved.openarchi.json");
            storage::save_document(&editor.project, &file, editor.project_identity()).unwrap();
            let original = std::fs::read(&file).unwrap();
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            open(&ctx, &mut app, &file);
            let before = app.host.document.editor.project.clone();
            let task = before.design_process.task().unwrap().clone();
            let b = task.candidates[1].clone();
            button(&ctx, &mut app, "AI Assistant");
            button(
                &ctx,
                &mut app,
                "Study the teaching courtyard, preserve existing work",
            );
            let label = format!(
                "{} · {}",
                language.translate("Edit generated candidate"),
                b.name
            );
            task_visible_button(&ctx, &mut app, &label);
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .plan_options
                    .active_design_option,
                Some(b.option)
            );
            assert_eq!(
                app.host.document.editor.selection_ids(),
                vec![b.initial_mass_ids[0]]
            );
            assert_eq!(app.view, View::Plan);
            assert_eq!(app.host.document.editor.project.elements, before.elements);
            assert_eq!(app.host.document.editor.project.evidence, before.evidence);
            assert!(!app.host.operations.borrow().enabled());
            // Native regression: Undo used to leave hidden B selected while A was active.
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(app.host.document.editor.project, before);
            assert_eq!(
                app.host.document.editor.selection_ids(),
                vec![task.candidates[0].initial_mass_ids[0]]
            );
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
                    .plan_options
                    .active_design_option,
                Some(b.option)
            );
            assert_eq!(
                app.host.document.editor.selection_ids(),
                vec![b.initial_mass_ids[0]]
            );
            assert_eq!(app.host.document.editor.project.elements, before.elements);
            assert_eq!(app.host.document.editor.project.evidence, before.evidence);
            assert!(!app.host.operations.borrow().enabled());
            // A parameter draft, even unchanged, cannot be retargeted by a result link.
            button(&ctx, &mut app, "AI Assistant");
            button(&ctx, &mut app, "Adjust generated wings");
            frames(&ctx, &mut app);
            let out = layers_frame_output(&ctx, &mut app, vec![]);
            let label_a = format!(
                "{} · {}",
                language.translate("Edit generated candidate"),
                task.candidates[0].name
            );
            let tree = out.platform_output.accesskit_update.unwrap();
            let (node_id, node) = tree
                .nodes
                .iter()
                .find(|(_, n)| n.label() == Some(label_a.as_str()))
                .expect(
                    "original generated candidate remains discoverable during the parameter draft",
                );
            assert!(
                node.is_disabled(),
                "AX nodes retain their action but must be marked disabled"
            );
            action(&ctx, &mut app, *node_id, Action::Click, None);
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .plan_options
                    .active_design_option,
                Some(b.option)
            );
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .evidence
                    .topics
                    .iter()
                    .find(|t| t.id == topic)
                    .unwrap(),
                before
                    .evidence
                    .topics
                    .iter()
                    .find(|t| t.id == topic)
                    .unwrap()
            );
            assert_eq!(std::fs::read(&file).unwrap(), original);
        }
    }
    #[test]
    fn shared_host_history_clears_unmapped_members_but_preserves_common_objects() {
        use crate::{application_host::HostCommand, command::ModelCommand};
        let (mut editor, _, _) = fixture();
        let task = editor.project.design_process.task().unwrap().clone();
        let b = task.candidates[1].clone();
        let mut extra = editor
            .project
            .elements
            .iter()
            .find(|e| e.id == task.candidates[0].initial_mass_ids[0])
            .unwrap()
            .clone();
        extra.id = editor.project.next_id;
        extra.name = "Manually added, no declared counterpart".into();
        let extra_id = extra.id;
        editor
            .apply(ModelCommand::AddElement { element: extra })
            .unwrap();
        let common = editor
            .project
            .elements
            .iter()
            .find(|e| e.design_option.is_none())
            .unwrap()
            .id;
        let change =
            crate::design_process::service::prepare_activate(&editor, Some(b.option)).unwrap();
        editor
            .commit_change("switch-with-unmapped-member", &change)
            .unwrap();
        let (_, mut app) = setup();
        app.host.document.editor = editor;
        app.host
            .document
            .editor
            .set_selection(vec![b.initial_mass_ids[0]]);
        app.host
            .dispatch(app.host.context(), HostCommand::Undo)
            .unwrap();
        assert_eq!(
            app.host.document.editor.selection_ids(),
            vec![task.candidates[0].initial_mass_ids[0]]
        );
        app.host.document.editor.set_selection(vec![extra_id]);
        app.host
            .dispatch(app.host.context(), HostCommand::Redo)
            .unwrap();
        assert!(app.host.document.editor.selection_ids().is_empty());
        app.host.document.editor.set_selection(vec![common]);
        app.host
            .dispatch(app.host.context(), HostCommand::Undo)
            .unwrap();
        assert_eq!(app.host.document.editor.selection_ids(), vec![common]);
        assert!(!app.host.operations.borrow().enabled());
    }
    #[test]
    fn generated_result_in_another_task_uses_existing_preview_cancel_confirm_and_undo() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (mut editor, _, _) = fixture();
            let first = editor.project.design_process.task().unwrap().clone();
            let definition = first
                .intent
                .as_ref()
                .unwrap()
                .parametric
                .as_ref()
                .unwrap()
                .clone();
            let second_id = storage::new_document_identity();
            let change = parametric::prepare(
                &editor,
                &second_id,
                "Another task".into(),
                "Keep both task histories".into(),
                definition,
                0,
                0,
            )
            .unwrap();
            editor
                .commit_change("fixture-second-study", &change)
                .unwrap();
            let before = editor.project.clone();
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            app.host.document.editor = editor;
            let journal = std::env::temp_dir().join(format!(
                "oa-nearby-switch-{}",
                storage::new_document_identity()
            ));
            app.host.configure_operation_journal(Some(journal)).unwrap();
            button(&ctx, &mut app, "AI Assistant");
            button(
                &ctx,
                &mut app,
                "Study the teaching courtyard, preserve existing work",
            );
            let label = format!(
                "{} · {}",
                language.translate("Edit generated candidate"),
                first.candidates[1].name
            );
            task_visible_button(&ctx, &mut app, &label);
            assert!(
                app.process_ui.preview.is_some(),
                "{}",
                app.research_ui.error
            );
            assert_eq!(app.host.document.editor.project, before);
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, before);
            task_visible_button(&ctx, &mut app, &label);
            button(&ctx, &mut app, "Confirm design change");
            assert!(app.process_ui.error.is_empty(), "{}", app.process_ui.error);
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .design_process
                    .current_task
                    .as_ref(),
                Some(&first.id)
            );
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .plan_options
                    .active_design_option,
                Some(first.candidates[1].option)
            );
            assert_eq!(app.host.document.editor.project.elements, before.elements);
            assert_eq!(app.host.document.editor.project.evidence, before.evidence);
            assert!(!app.host.operations.borrow().enabled());
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(app.host.document.editor.project, before);
            assert!(app.host.document.editor.selection_ids().iter().all(|id| {
                before.elements.iter().any(|e| {
                    e.id == *id
                        && (e.design_option.is_none()
                            || e.design_option == before.plan_options.active_design_option)
                })
            }));
        }
    }
    #[test]
    fn editing_shortcuts_rejected_common_model_edit_returns_original_error_and_preserves_history() {
        let (mut editor, _, _) = fixture();
        let wall = editor
            .project
            .elements
            .iter()
            .find(|e| e.design_option.is_none() && matches!(e.shape, Shape::Wall { .. }))
            .unwrap()
            .id;
        editor.set_selection(vec![wall]);
        let before = editor.project.clone();
        let revision = editor.revision;
        let history = format!("{:?}/{:?}", editor.undo, editor.redo);
        let last = editor.last_edit.clone();
        let result = editor.edit(crate::editing::EditCommand::Transform {
            offset: Point::new(1., 0.),
            origin: Point::default(),
            angle: 0.,
            scale: 1.,
            mirror: false,
            copies: 0,
        });
        assert!(
            result.is_err(),
            "rolled-back common-model change must not report Edit applied"
        );
        let error = result.unwrap_err();
        assert!(error.contains("baseline"), "{error}");
        assert_eq!(editor.status, error);
        assert_eq!(editor.project, before);
        assert_eq!(editor.revision, revision);
        assert_eq!(editor.selection_ids(), vec![wall]);
        assert_eq!(format!("{:?}/{:?}", editor.undo, editor.redo), history);
        assert_eq!(editor.last_edit, last);
    }
}

#[test]
#[cfg(unix)]
#[ignore = "requires latest protected grain project and unused output directory; never writes original"]
fn protected_grain_nearby_candidates_and_new_document_scroll_bilingual() {
    use crate::evidence::topic_tasks::Mode;
    let input = PathBuf::from(std::env::var_os("OPENARCHI_NEARBY_INPUT").unwrap());
    let original = std::fs::read(&input).unwrap();
    let output = PathBuf::from(std::env::var_os("OPENARCHI_NEARBY_OUTPUT").unwrap());
    std::fs::create_dir(&output).expect("inspect existing saved results, never overwrite");
    let protected = output.join("protected-source.openarchi.json");
    let source = Editor::from_stored_document(&input).unwrap();
    storage::save_document(&source.project, &protected, source.project_identity()).unwrap();
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app) = setup();
        ctx.enable_accesskit();
        ctx.global_style_mut(|s| s.animation_time = 0.);
        app.language = language;
        open(&ctx, &mut app, &protected);
        let before = app.host.document.editor.project.clone();
        let task = before.design_process.task().unwrap().clone();
        let topic=before.evidence.topics.iter().find(|t|t.exchanges.iter().any(|e|matches!(&e.assignment,Some(a) if matches!(&a.mode,Mode::CreateTwoWing { task_id,.. } if task_id==&task.id)))).unwrap().clone();
        let question = topic.exchanges.last().unwrap().question.clone();
        button(&ctx, &mut app, "AI Assistant");
        task_visible_button(&ctx, &mut app, &question);
        let b = &task.candidates[1];
        let label = format!(
            "{} · {}",
            language.translate("Edit generated candidate"),
            b.name
        );
        task_visible_button(&ctx, &mut app, &label);
        assert_eq!(
            app.host.document.editor.selection_ids(),
            vec![b.initial_mass_ids[0]]
        );
        assert_eq!(app.host.document.editor.project.elements, before.elements);
        assert_eq!(app.host.document.editor.project.evidence, before.evidence);
        assert!(!app.host.operations.borrow().enabled());
        layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
        assert_eq!(
            app.host.document.editor.selection_ids(),
            vec![task.candidates[0].initial_mass_ids[0]]
        );
        layers_frame_output(
            &ctx,
            &mut app,
            vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
        );
        assert_eq!(
            app.host.document.editor.selection_ids(),
            vec![b.initial_mass_ids[0]]
        );
        assert_eq!(app.host.document.editor.project.elements, before.elements);
        button(&ctx, &mut app, "Discuss selected objects");
        assert_discussion_layout_and_type(&ctx, &mut app);
        button(&ctx, &mut app, "Cancel discussion draft");
        let file = output.join(if language == UiLanguage::English {
            "nearby-en.openarchi.json"
        } else {
            "nearby-zh.openarchi.json"
        });
        layers_frame_output(
            &ctx,
            &mut app,
            vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
        );
        let old = app.file_dialog_path.clone().unwrap();
        field(&ctx, &mut app, &old, &file.display().to_string());
        assert_eq!(
            app.file_dialog_path.as_deref(),
            file.to_str(),
            "do not Save until the exact new target is verified"
        );
        assert!(!file.exists());
        assert_ne!(file, input);
        assert_ne!(file, protected);
        assert_eq!(std::fs::read(&input).unwrap(), original);
        button(&ctx, &mut app, "Save");
        assert!(
            file.exists(),
            "{} / {}",
            app.file_error,
            app.host.document.editor.status
        );
        let (fresh_ctx, mut fresh) = setup();
        fresh_ctx.enable_accesskit();
        fresh_ctx.global_style_mut(|s| s.animation_time = 0.);
        fresh.language = language;
        open(&fresh_ctx, &mut fresh, &file);
        assert_eq!(fresh.host.document.editor.project.elements, before.elements);
        assert!(!fresh.host.operations.borrow().enabled());
        button(&fresh_ctx, &mut fresh, "AI Assistant");
        task_visible_button(&fresh_ctx, &mut fresh, &question);
        let a = &task.candidates[0];
        let label = format!(
            "{} · {}",
            language.translate("Edit generated candidate"),
            a.name
        );
        task_visible_button(&fresh_ctx, &mut fresh, &label);
        assert_eq!(
            fresh.host.document.editor.selection_ids(),
            vec![a.initial_mass_ids[0]]
        );
        assert_eq!(fresh.host.document.editor.project.elements, before.elements);
        assert_eq!(fresh.host.document.editor.project.evidence, before.evidence);
        // A normally opened new document must not inherit this long discussion's offset.
        button(&fresh_ctx, &mut fresh, "AI Assistant");
        for _ in 0..4 {
            task_scroll(&fresh_ctx, &mut fresh, -400.);
        }
        let offset = fresh_ctx
            .data(|d| d.get_temp::<f32>(Id::new("inspector_scroll_offset")))
            .unwrap();
        assert!(offset > 0.);
        open(&fresh_ctx, &mut fresh, &protected);
        button(&fresh_ctx, &mut fresh, "AI Assistant");
        for _ in 0..6 {
            layers_frame_output(&fresh_ctx, &mut fresh, vec![]);
        }
        assert_eq!(
            fresh_ctx
                .data(|d| d.get_temp::<f32>(Id::new("inspector_scroll_offset")))
                .unwrap(),
            0.
        );
        button(&fresh_ctx, &mut fresh, "Select a source mass");
        let name = fresh
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .find(|e| e.id == a.initial_mass_ids[0])
            .unwrap()
            .name
            .clone();
        button(
            &fresh_ctx,
            &mut fresh,
            &format!("#{} · {}", a.initial_mass_ids[0], name),
        );
        button(&fresh_ctx, &mut fresh, "Discuss selected objects");
        assert_discussion_layout_and_type(&fresh_ctx, &mut fresh);
        assert_eq!(fresh.host.document.editor.project.elements, before.elements);
        assert_eq!(std::fs::read(&input).unwrap(), original);
        std::fs::write(output.join(if language==UiLanguage::English {"en.json"}else{"zh.json"}),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"output":file,"model_unchanged":true,"topic":topic.id,"first_study":task.id,"normal_egui":"pass","new_host":"pass","native_os":"not_run","real_AI":"not_run","permission":"not_granted","old_seed_not_reapplied":true})).unwrap()).unwrap();
    }
}
