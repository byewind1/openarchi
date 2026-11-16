// Existing native discussion controls, not a runner-specific permission guard.
#[test]
#[cfg(unix)]
fn object_task_linked_space_normal_consent_bilingual() {
    linked_space_consent_path(false);
}
#[test]
#[cfg(unix)]
#[ignore = "latest protected input and unused output required"]
fn object_task_linked_space_protected_consent_bilingual() {
    linked_space_consent_path(true);
}
#[cfg(unix)]
fn linked_space_consent_path(supplied: bool) {
    use crate::evidence::{
        EvidenceEdit,
        topic_tasks::{self, Event, Mode},
    };
    let root = if supplied {
        PathBuf::from(std::env::var_os("OPENARCHI_LINKED_SPACE_OUTPUT").unwrap())
    } else {
        std::env::temp_dir().join(format!(
            "oa-linked-consent-{}",
            storage::new_document_identity()
        ))
    };
    std::fs::create_dir(&root).expect("unused output required; do not replay unknown operations");
    let input = if supplied {
        PathBuf::from(std::env::var_os("OPENARCHI_LINKED_SPACE_INPUT").unwrap())
    } else {
        let (mut editor, _, _) = object_nearby_product_path::fixture();
        let topic = editor.project.evidence.topics.last().unwrap();
        let previous = topic.exchanges[0].assignment.as_ref().unwrap().id.clone();
        editor
            .apply(crate::command::ModelCommand::Evidence {
                edit: EvidenceEdit::TopicTaskEvent {
                    id: topic.id.clone(),
                    exchange_index: 0,
                    expected_topic_sha256: crate::automation_journal::canonical_hash(topic)
                        .unwrap(),
                    request_id: previous,
                    event: Event::Cancel,
                },
            })
            .unwrap();
        let file = root.join("public-fixture.openarchi.json");
        storage::save_document(&editor.project, &file, editor.project_identity()).unwrap();
        file
    };
    let original_bytes = std::fs::read(&input).unwrap();
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app) = setup();
        ctx.enable_accesskit();
        ctx.global_style_mut(|s| s.animation_time = 0.);
        app.language = language;
        let root = root.join(if language == UiLanguage::English {
            "en"
        } else {
            "zh"
        });
        std::fs::create_dir(&root).unwrap();
        app.host
            .configure_operation_journal(Some(root.join("journal")))
            .unwrap();
        open(&ctx, &mut app, &input);
        let mass = app
            .host
            .document
            .editor
            .project
            .design_process
            .task()
            .unwrap()
            .candidates[0]
            .initial_mass_ids[0];
        app.host.document.editor.set_selection(vec![mass]);
        let before = app.host.document.editor.project.clone();
        button(&ctx, &mut app, "Discuss selected objects");
        assert!(!app.research_ui.allow_task_space);
        assert!(!app.research_ui.allow_task_parameters);
        assert!(!app.research_ui.allow_task_creation);
        assert_discussion_layout_and_type(&ctx, &mut app);
        button(
            &ctx,
            &mut app,
            "Allow parameter updates to the current two wings",
        );
        assert!(app.research_ui.allow_task_parameters);
        button(&ctx, &mut app, "Allow one linked concept space");
        assert!(app.research_ui.allow_task_space);
        assert!(!app.research_ui.allow_task_parameters);
        button(&ctx, &mut app, "Allow one new two-wing study");
        assert!(app.research_ui.allow_task_creation);
        assert!(!app.research_ui.allow_task_space);
        button(&ctx, &mut app, "Allow one linked concept space");
        assert!(!app.research_ui.allow_task_creation);
        assert!(app.research_ui.allow_task_space);
        let frozen = app.research_ui.draft.as_ref().unwrap().exchange.clone();
        let other = before.design_process.task().unwrap().candidates[0].initial_mass_ids[1];
        app.host.document.editor.set_selection(vec![other]);
        button(&ctx, &mut app, "Preview task for connected Agent");
        assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
        let out = layers_frame_output(&ctx, &mut app, vec![]);
        let mut texts = vec![];
        collect_text(&out.shapes, &mut texts);
        assert!(texts.iter().any(|s| {
            s == language
                .translate("Permission: one linked concept Zone; no changes to existing work")
        }));
        assert!(texts.contains(&format!("#{mass} → Zone #{}", before.next_id)));
        assert_eq!(app.host.document.editor.project, before);
        assert!(!app.host.operations.borrow().enabled());
        button(&ctx, &mut app, "Cancel design proposal");
        assert_eq!(app.research_ui.draft.as_ref().unwrap().exchange, frozen);
        assert!(!app.host.operations.borrow().enabled());
        button(&ctx, &mut app, "Preview task for connected Agent");
        button(&ctx, &mut app, "Confirm design change");
        assert!(
            app.research_ui.error.is_empty(),
            "{}",
            app.research_ui.error
        );
        let p = &app.host.document.editor.project;
        let a = p
            .evidence
            .topics
            .last()
            .unwrap()
            .exchanges
            .last()
            .unwrap()
            .assignment
            .as_ref()
            .unwrap();
        let (_, _, exchange, assignment) = topic_tasks::find(p, &a.id).unwrap();
        assert_eq!(exchange.objects, frozen.objects);
        assert_eq!(exchange.question, frozen.question);
        assert!(
            matches!(&assignment.mode, Mode::CreateLinkedSpace {mass:m,zone,source_sha256,operation_id,..}
            if *m==mass && *zone==before.next_id && storage::valid_document_identity(source_sha256)
            && storage::valid_document_identity(operation_id) && *operation_id!=assignment.id)
        );
        assert_eq!(app.host.object_task_authorization()["status"], "authorized");
        assert_eq!(p.elements, before.elements);
        assert_eq!(p.design_process, before.design_process);
        assert_eq!(p.site, before.site);
        let saved = root.join("consent.openarchi.json");
        storage::save_document(p, &saved, app.host.document.editor.project_identity()).unwrap();
        let raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&saved).unwrap()).unwrap();
        assert_eq!(raw["schema_version"], 33);
        let mut fresh = crate::application_host::ApplicationHost::new(Project::default());
        fresh.document.editor = Editor::from_stored_document(&saved).unwrap();
        let mut expected = p.clone();
        assert_eq!(
            expected.drawings.pages.len(),
            fresh.document.editor.project.drawings.pages.len()
        );
        for (old, new) in expected
            .drawings
            .pages
            .iter_mut()
            .zip(&fresh.document.editor.project.drawings.pages)
        {
            assert_eq!(
                crate::drawings::verified_bytes(old).unwrap(),
                crate::drawings::verified_bytes(new).unwrap()
            );
            old.path = new.path.clone();
        }
        assert!(
            fresh.document.editor.project == expected,
            "reopen changed data beyond verified sidecar path bindings"
        );
        assert!(!fresh.operations.borrow().enabled());
        std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({
            "input":input,"saved":saved,"entry":"normal Open/object discussion/optional consent/preview cancel/confirm/native33 save/new host",
            "mass":mass,"zone":before.next_id,"objects":before.elements.len(),"actual_AI":false,
            "model_unchanged":true,"frozen_selection":true,"reopen_default_read_only":true,
            "sidecar_bytes_exact":true,"original_input_unchanged":true
        })).unwrap()).unwrap();
    }
    assert_eq!(
        std::fs::read(&input).unwrap(),
        original_bytes,
        "protected input must remain exact"
    );
}
