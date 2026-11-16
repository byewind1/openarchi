// Existing native controls and shared model preconditions. No alternative task guard.
#[cfg(unix)]
mod discussion_restore_product_path {
    use super::*;
    use crate::evidence::{EvidenceEdit, Target, topic_tasks::Event as TaskEvent, topics};

    fn fixture() -> Editor {
        let (mut editor, _, mass) = object_nearby_product_path::fixture();
        let topic = editor.project.evidence.topics.last().unwrap();
        let request = topic.exchanges[0].assignment.as_ref().unwrap().id.clone();
        editor
            .apply(crate::command::ModelCommand::Evidence {
                edit: EvidenceEdit::TopicTaskEvent {
                    id: topic.id.clone(),
                    exchange_index: 0,
                    expected_topic_sha256: crate::automation_journal::canonical_hash(topic)
                        .unwrap(),
                    request_id: request,
                    event: TaskEvent::Cancel,
                },
            })
            .unwrap();
        let id = storage::new_document_identity();
        let exchange = topics::capture(
            &editor.project,
            &[Target::Element { id: mass }],
            "Can this teaching wing support a shared concept space?".into(),
        )
        .unwrap();
        editor
            .apply(crate::command::ModelCommand::Evidence {
                edit: EvidenceEdit::RecordTopic {
                    id: id.clone(),
                    exchange,
                    expected_previous: None,
                },
            })
            .unwrap();
        let topic = editor.project.evidence.topics.last().unwrap();
        editor
            .apply(crate::command::ModelCommand::Evidence {
                edit: EvidenceEdit::RecordTopicReply {
                    id,
                    exchange_index: 0,
                    expected_topic_sha256: crate::automation_journal::canonical_hash(topic)
                        .unwrap(),
                    reply: topics::AgentReply {
                        id: storage::new_document_identity(),
                        agent_label: "Public fixture; not actual AI".into(),
                        text:
                            "One editable concept container; capacity and approval remain unknown."
                                .into(),
                        claims: vec![crate::design_process::intent::Claim {
                            kind: crate::design_process::intent::ClaimKind::DesignCompletion,
                            text: "Assumed single-level space, not measured.".into(),
                        }],
                    },
                },
            })
            .unwrap();
        let source = editor
            .project
            .elements
            .iter()
            .find(|e| e.id == mass)
            .unwrap();
        editor
            .apply(crate::command::ModelCommand::MassSpaces {
                edit: crate::mass_spaces::Edit::Create {
                    mass,
                    expected_source_sha256: crate::mass_spaces::element_stamp(
                        &editor.project,
                        source,
                    )
                    .unwrap(),
                    name: "Editable shared concept".into(),
                    purpose: "Concept only; approval unknown".into(),
                    height: 3.,
                    assumption: "Public synthetic fixture, not actual AI.".into(),
                },
            })
            .unwrap();
        editor
    }
    fn choose(ctx: &Context, app: &mut AtelierApp, id: u64) {
        for _ in 0..6 {
            layers_frame_output(ctx, app, vec![]);
        }
        let current = app
            .mass_space_canvas_selection()
            .map(|e| format!("#{} · {}", e.id, e.name))
            .unwrap_or_else(|| app.language.translate("Select a source mass").into());
        button(ctx, app, &current);
        let e = app
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .find(|e| e.id == id)
            .unwrap();
        button(ctx, app, &format!("#{id} · {}", e.name));
        assert_eq!(app.host.document.editor.selected, Some(id));
    }
    fn related_button(ctx: &Context, app: &mut AtelierApp) {
        let count = app.related_selection_count();
        assert!(count > 0);
        let label = format!(
            "{} ({count})",
            app.language.translate("Related discussions")
        );
        button(ctx, app, &label);
    }
    #[test]
    fn related_discussions_normal_source_result_frozen_review_and_reopen_bilingual() {
        path(false);
    }
    #[test]
    #[ignore = "latest protected input and unused output required"]
    fn related_discussions_protected_source_result_frozen_review_and_reopen_bilingual() {
        path(true);
    }
    fn path(supplied: bool) {
        let root = if supplied {
            PathBuf::from(std::env::var_os("OPENARCHI_OBJECT_RESTORE_OUTPUT").unwrap())
        } else {
            std::env::temp_dir().join(format!(
                "oa-object-restore-{}",
                storage::new_document_identity()
            ))
        };
        std::fs::create_dir(&root).expect("unused output; do not replay unknown operations");
        let input = if supplied {
            PathBuf::from(std::env::var_os("OPENARCHI_OBJECT_RESTORE_INPUT").unwrap())
        } else {
            let editor = fixture();
            let file = root.join("public-fixture.openarchi.json");
            storage::save_document(&editor.project, &file, editor.project_identity()).unwrap();
            file
        };
        let bytes = std::fs::read(&input).unwrap();
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let output = root.join(if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            });
            std::fs::create_dir(&output).unwrap();
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            app.host
                .configure_operation_journal(Some(output.join("journal")))
                .unwrap();
            open(&ctx, &mut app, &input);
            let before = app.host.document.editor.project.clone();
            let link = before.design_process.mass_spaces.last().unwrap();
            let mass = link.mass;
            let zone = link.zone;
            choose(&ctx, &mut app, mass);
            related_button(&ctx, &mut app);
            assert!(app.object_review_active());
            assert!(app.research_ui.draft.is_none());
            assert!(app.process_ui.preview.is_none());
            assert_eq!(
                app.research_ui.review.as_ref().unwrap().targets,
                vec![Target::Element { id: mass }]
            );
            let latest = before
                .evidence
                .topics
                .iter()
                .rev()
                .find(|t| t.related_to(&before, &[Target::Element { id: mass }]))
                .unwrap();
            let question = &latest.exchanges.last().unwrap().question;
            let out = layers_frame_output(&ctx, &mut app, vec![]);
            let mut texts = vec![];
            collect_text(&out.shapes, &mut texts);
            assert!(
                texts.contains(question),
                "latest related discussion must open without finding it in all history: expected={question:?}, painted={:?}",
                texts
                    .iter()
                    .map(|s| s.chars().take(120).collect::<String>())
                    .collect::<Vec<_>>()
            );
            let heading =
                text_center(&out.shapes, language.translate("Related discussions")).unwrap();
            assert!(
                out.shapes.iter().any(|s| s.clip_rect.contains(heading)),
                "review heading is clipped away"
            );
            if let Some(status) = latest.exchanges.last().unwrap().result_status(&before) {
                let mut visible = false;
                for _ in 0..10 {
                    let out = layers_frame_output(&ctx, &mut app, vec![]);
                    let mut texts = vec![];
                    collect_text(&out.shapes, &mut texts);
                    if texts.iter().any(|s| s == language.translate(status)) {
                        visible = true;
                        break;
                    }
                    task_scroll(&ctx, &mut app, -60.);
                }
                assert!(
                    visible,
                    "current generated-result status must be readable beside its historical answer: {status}"
                );
                related_button(&ctx, &mut app);
            }
            assert_eq!(app.host.document.editor.project, before);
            assert!(!app.host.operations.borrow().enabled());
            // A new selection is not a request to retarget the frozen reader or task.
            let other = before.design_process.task().unwrap().candidates[1].initial_mass_ids[0];
            app.host.document.editor.set_selection(vec![other]);
            layers_frame_output(&ctx, &mut app, vec![]);
            assert_eq!(
                app.research_ui.review.as_ref().unwrap().targets,
                vec![Target::Element { id: mass }]
            );
            button(&ctx, &mut app, "All discussions");
            assert!(app.research_ui.review.as_ref().unwrap().targets.is_empty());
            choose(&ctx, &mut app, mass);
            button(&ctx, &mut app, "Select linked space");
            assert_eq!(app.host.document.editor.selected, Some(zone));
            related_button(&ctx, &mut app);
            assert_eq!(
                app.research_ui.review.as_ref().unwrap().targets,
                vec![Target::Element { id: zone }]
            );
            button(&ctx, &mut app, "New contribution with these objects");
            assert!(app.research_ui.draft.is_some());
            assert!(!app.can_review_object_discussions());
            assert!(
                !app.research_ui.allow_task_parameters
                    && !app.research_ui.allow_task_creation
                    && !app.research_ui.allow_task_space
            );
            assert!(!app.host.operations.borrow().enabled());
            assert_discussion_layout_and_type(&ctx, &mut app);
            button(&ctx, &mut app, "Preview discussion record");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            assert!(!app.can_review_object_discussions());
            let label = format!(
                "{} ({})",
                language.translate("Related discussions"),
                app.related_selection_count()
            );
            let out = layers_frame_output(&ctx, &mut app, vec![]);
            let tree = out.platform_output.accesskit_update.unwrap();
            let node = tree.nodes.iter().find(|(_, n)| {
                n.label() == Some(label.as_str()) || n.value() == Some(label.as_str())
            });
            assert!(
                node.is_none_or(|(_, node)| node.is_disabled()),
                "collapsed menu must not expose an enabled review during a native preview"
            );
            assert!(!app.can_review_object_discussions());
            button(&ctx, &mut app, "Cancel design proposal");
            button(&ctx, &mut app, "Cancel discussion draft");
            assert_eq!(app.host.document.editor.project, before);
            // Normal SaveAs, then a new host, not a copied UI permission or in-memory session.
            let saved = output.join("restored.openarchi.json");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(
                    Key::S,
                    Modifiers {
                        command: true,
                        shift: true,
                        ..Modifiers::NONE
                    },
                )],
            );
            let old = app.file_dialog_path.clone().unwrap();
            field(&ctx, &mut app, &old, &saved.display().to_string());
            button(&ctx, &mut app, "Save");
            let (fresh_ctx, mut fresh) = setup();
            fresh_ctx.enable_accesskit();
            fresh_ctx.global_style_mut(|s| s.animation_time = 0.);
            fresh.language = language;
            open(&fresh_ctx, &mut fresh, &saved);
            assert!(!fresh.object_review_active());
            assert!(!fresh.host.operations.borrow().enabled());
            choose(&fresh_ctx, &mut fresh, mass);
            button(&fresh_ctx, &mut fresh, "Select linked space");
            related_button(&fresh_ctx, &mut fresh);
            assert!(fresh.object_review_active());
            assert!(fresh.research_ui.draft.is_none());
            assert_eq!(fresh.host.document.editor.project.evidence, before.evidence);
            assert_eq!(fresh.host.document.editor.project.elements, before.elements);
            // A document replacement cannot attach this frozen reader to coincident IDs.
            open(&fresh_ctx, &mut fresh, &input);
            assert!(!fresh.object_review_active());
            std::fs::write(output.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"saved":saved,"source":mass,"zone":zone,"native_task_started":false,"actual_AI":false,"frozen_selection":true,"new_host_read_only":true,"model_unchanged":true,"original_input_unchanged":true})).unwrap()).unwrap();
        }
        assert_eq!(std::fs::read(&input).unwrap(), bytes);
    }
}
