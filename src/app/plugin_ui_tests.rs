mod plugin_product_path {
    use super::*;
    #[test]
    fn reopened_project_prefers_original_unresolved_job_without_overriding_manual_choice_bilingual()
    {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let root = std::env::temp_dir().join(format!(
                "oa-original-selection-ui-{}",
                storage::new_document_identity()
            ));
            std::fs::create_dir(&root).unwrap();
            println!("ORIGINAL_SELECTION_UI_ROOT={}", root.display());
            let profile = root.join("profile");
            let mut original =
                crate::application_host::ApplicationHost::new(storage::startup_example());
            original.configure_plugins(&profile).unwrap();
            let completed = "f".repeat(64);
            original
                .start_plugin_job("org.openarchi.quantities.report", &completed)
                .unwrap();
            let until = Instant::now() + Duration::from_secs(5);
            let mut interrupted = loop {
                original.jobs.as_mut().unwrap().poll();
                let b = original
                    .jobs
                    .as_ref()
                    .unwrap()
                    .query(original.document.editor.project_identity(), &completed)
                    .unwrap();
                if b.result.is_some() {
                    break b;
                }
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(1));
            };
            // Restored original-interruption evidence: no execution is replayed.
            let pending = "a".repeat(64);
            interrupted.intent.identity.request_id = pending.clone();
            interrupted.result = None;
            let evidence = root.join("interrupted.job.json");
            interrupted.export(&evidence).unwrap();
            original
                .import_plugin_job(evidence)
                .unwrap()
                .join()
                .unwrap()
                .unwrap();
            let project = root.join("current-project.json");
            storage::save_document(
                &original.document.editor.project,
                &project,
                original.document.editor.project_identity(),
            )
            .unwrap();
            let expected = original.document.editor.project.clone();
            drop(original);
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.host.configure_plugins(&profile).unwrap();
            open_test_project(&ctx, &mut app, &project);
            button(&ctx, &mut app, "Plugins and jobs…");
            layers_frame_output(&ctx, &mut app, vec![]);
            assert_eq!(
                app.plugins_ui
                    .record
                    .as_ref()
                    .unwrap()
                    .intent
                    .identity
                    .request_id,
                pending,
                "current unfinished task must not require copying its hash from another window"
            );
            assert!(app.plugins_ui.record.as_ref().unwrap().result.is_none());
            let original_ids = app
                .host
                .jobs
                .as_ref()
                .unwrap()
                .ids(app.host.document.editor.project_identity())
                .unwrap();
            button(&ctx, &mut app, "Study sunlight for current candidate…");
            assert_eq!(app.plugins_ui.original_request, pending);
            assert_eq!(
                app.host
                    .jobs
                    .as_ref()
                    .unwrap()
                    .ids(app.host.document.editor.project_identity())
                    .unwrap(),
                original_ids
            );
            button(&ctx, &mut app, "Inspect original job outcome");
            assert_eq!(
                app.plugins_ui
                    .record
                    .as_ref()
                    .unwrap()
                    .intent
                    .identity
                    .request_id,
                pending
            );
            button(&ctx, &mut app, "ffffffffffff");
            assert_eq!(
                app.plugins_ui
                    .record
                    .as_ref()
                    .unwrap()
                    .intent
                    .identity
                    .request_id,
                completed
            );
            button(&ctx, &mut app, "Close");
            button(&ctx, &mut app, "Plugins and jobs…");
            layers_frame_output(&ctx, &mut app, vec![]);
            assert_eq!(
                app.plugins_ui
                    .record
                    .as_ref()
                    .unwrap()
                    .intent
                    .identity
                    .request_id,
                completed,
                "do not override the user's deliberate record selection"
            );
            assert_eq!(app.host.document.editor.project, expected);
            assert_eq!(app.host.document.editor.revision, 0);
            assert!(app.host.document.editor.undo.is_empty());
        }
    }
    #[test]
    #[ignore = "Requires separately built original provider; explicit OPENARCHI_TEST_PROVIDER"]
    fn external_provider_normal_declaration_review_authorize_run_and_persist_bilingual() {
        let binary =
            std::env::var("OPENARCHI_TEST_PROVIDER").expect("original provider path required");
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let input = std::env::var_os("OPENARCHI_PROVIDER_UI_INPUT").map(PathBuf::from);
            if input.is_some() && language == UiLanguage::SimplifiedChinese {
                continue;
            }
            let root = std::env::var_os("OPENARCHI_PROVIDER_UI_OUTPUT")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::temp_dir()
                        .join(format!("oa-ext-ui-{}", storage::new_document_identity()))
                });
            std::fs::create_dir(&root)
                .expect("existing original-operation output must be recovered, never replayed");
            std::fs::write(root.join("intent.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"executable":binary,"source":"original offline-built example","entry":"normal workbench -> declaration -> explicit executable review/authorize -> run -> export/save/reopen","effects":"new isolated config/jobs/project copy only"})).unwrap()).unwrap();
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.host.document.editor = if let Some(p) = &input {
                Editor::from_stored_document(p).unwrap()
            } else {
                Editor::new(storage::startup_example())
            };
            app.host.configure_plugins(&root).unwrap();
            let before = app.host.document.editor.project.clone();
            let base = app.host.context();
            button(&ctx, &mut app, "Plugins and jobs…");
            button(&ctx, &mut app, "Load a local plugin declaration");
            let declaration = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("assets/plugins/example-zone-program.json");
            text(
                &ctx,
                &mut app,
                "Choose a local JSON declaration",
                &declaration.display().to_string(),
            );
            button(&ctx, &mut app, "Read declaration");
            assert_eq!(app.host.plugins.entries().len(), 2);
            text(&ctx, &mut app, "Choose a local executable", &binary);
            button(&ctx, &mut app, "Read executable identity");
            assert_eq!(app.host.plugins.entries().len(), 2);
            assert!(
                app.host
                    .jobs
                    .as_ref()
                    .unwrap()
                    .ids(app.host.document.editor.project_identity())
                    .unwrap()
                    .is_empty()
            );
            button(&ctx, &mut app, "Authorize and enable reviewed executable");
            assert_eq!(app.host.plugins.entries().len(), 3);
            assert!(
                app.host
                    .jobs
                    .as_ref()
                    .unwrap()
                    .ids(app.host.document.editor.project_identity())
                    .unwrap()
                    .is_empty()
            );
            button(
                &ctx,
                &mut app,
                "Review current Zones against declared requirements",
            );
            let id = app.plugins_ui.original_request.clone();
            let until = Instant::now() + Duration::from_secs(6);
            loop {
                layers_frame_output(&ctx, &mut app, vec![]);
                if app
                    .plugins_ui
                    .record
                    .as_ref()
                    .is_some_and(|b| b.result.is_some())
                {
                    break;
                }
                assert!(
                    Instant::now() < until,
                    "original external job pending; no replay: {}",
                    app.plugins_ui.error
                );
                std::thread::sleep(Duration::from_millis(1));
            }
            let record = app.plugins_ui.record.clone().unwrap();
            assert!(record.current(&app.host.document.editor));
            let report = match &record.result.as_ref().unwrap().terminal {
                crate::plugins::jobs::Terminal::Completed { report } => report,
                other => panic!("{other:?}"),
            };
            assert!(report.program_review.is_some());
            let f = layers_frame_output(&ctx, &mut app, vec![]);
            assert!(
                f.shapes.iter().any(|shape| text_center(
                    std::slice::from_ref(shape),
                    language.translate("No declared requirements; review remains unknown.")
                )
                .is_some_and(|at| shape.clip_rect.contains(at))),
                "external result must be visible in the current window"
            );
            assert_eq!(app.host.document.editor.project, before);
            assert_eq!(app.host.context(), base);
            assert!(app.host.document.editor.undo.is_empty());
            let export = root.join("original.job.json");
            let old = app.plugins_ui.export_path.clone();
            text(&ctx, &mut app, &old, &export.display().to_string());
            button(&ctx, &mut app, "Export job record");
            assert_eq!(
                crate::plugins::jobs::Bundle::read_export(&export).unwrap(),
                record
            );
            // Moving unchanged approved bytes still requires a separate explicit authorization.
            let reviewed_copy = root.join("explicitly-reviewed-provider-copy");
            std::fs::copy(&binary, &reviewed_copy).unwrap();
            button(&ctx, &mut app, "Load a local plugin declaration");
            button(&ctx, &mut app, "Read declaration");
            text(
                &ctx,
                &mut app,
                &binary,
                &reviewed_copy.display().to_string(),
            );
            button(&ctx, &mut app, "Read executable identity");
            button(&ctx, &mut app, "Authorize and enable reviewed executable");
            assert_eq!(app.host.plugins.entries().len(), 3);
            assert_eq!(
                app.host
                    .plugins
                    .executable("example.zone_program")
                    .unwrap()
                    .path,
                reviewed_copy.canonicalize().unwrap()
            );
            assert_eq!(
                app.host
                    .jobs
                    .as_ref()
                    .unwrap()
                    .query(app.host.document.editor.project_identity(), &id)
                    .unwrap(),
                record
            );
            assert_eq!(app.host.context(), base);
            button(&ctx, &mut app, "Close");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let old = app.path.clone();
            let saved = root.join("continued.openarchi.json");
            text(&ctx, &mut app, &old, &saved.display().to_string());
            button(&ctx, &mut app, "Save");
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let mut reopened = crate::application_host::ApplicationHost::new(Project::default());
            reopened.document.editor = Editor::from_stored_document(&saved).unwrap();
            reopened.configure_plugins(&root).unwrap();
            reopened.plugins = crate::plugins::Registry::default();
            let restored = reopened
                .jobs
                .as_ref()
                .unwrap()
                .query(reopened.document.editor.project_identity(), &id)
                .unwrap();
            assert_eq!(restored, record);
            assert!(restored.current(&reopened.document.editor));
            assert_eq!(reopened.document.editor.project.elements, before.elements);
            assert_eq!(
                reopened.document.editor.project.design_process,
                before.design_process
            );
            assert_eq!(
                reopened.document.editor.project.drawings.diagrams,
                before.drawings.diagrams
            );
            std::fs::write(root.join("walkthrough.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"saved":saved,"export":export,"request_id":id,"current":true,"language":format!("{language:?}"),"model":"unchanged","provider":"actual external process","native_package":"not_run","realAI":"not_run"})).unwrap()).unwrap();
            println!("EXTERNAL_PROVIDER_UI={}", root.display());
        }
    }
    #[test]
    fn exported_record_restore_normal_workspace_missing_provider_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let input = std::env::var_os("OPENARCHI_IMPORT_UI_INPUT").map(PathBuf::from);
            if input.is_some() && language == UiLanguage::SimplifiedChinese {
                continue;
            }
            let root = std::env::var_os("OPENARCHI_IMPORT_UI_OUTPUT")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::temp_dir().join(format!(
                        "oa-job-restore-{}",
                        storage::new_document_identity()
                    ))
                });
            std::fs::create_dir(&root).expect("preserve existing original recovery output");
            let (source, model) = if let Some(input) = input {
                (
                    PathBuf::from(
                        std::env::var_os("OPENARCHI_IMPORT_UI_RECORD")
                            .expect("original exported record required"),
                    ),
                    input,
                )
            } else {
                let mut host =
                    crate::application_host::ApplicationHost::new(storage::startup_example());
                host.configure_plugins(&root.join("source-profile"))
                    .unwrap();
                host.plugins
                    .enable(
                        crate::plugins::Manifest::parse(include_bytes!(
                            "../../assets/plugins/example-quantities.json"
                        ))
                        .unwrap(),
                    )
                    .unwrap();
                let id = storage::new_document_identity();
                host.start_plugin_job("example.quantities.review", &id)
                    .unwrap();
                let until = Instant::now() + Duration::from_secs(5);
                let bundle = loop {
                    host.jobs.as_mut().unwrap().poll();
                    let b = host
                        .jobs
                        .as_ref()
                        .unwrap()
                        .query(host.document.editor.project_identity(), &id)
                        .unwrap();
                    if b.result.is_some() {
                        break b;
                    }
                    assert!(Instant::now() < until);
                    std::thread::sleep(Duration::from_millis(1));
                };
                let source = root.join("original.job.json");
                bundle.export(&source).unwrap();
                let model = root.join("input.openarchi.json");
                storage::save_document(
                    &host.document.editor.project,
                    &model,
                    host.document.editor.project_identity(),
                )
                .unwrap();
                (source, model)
            };
            std::fs::write(root.join("operation-plan.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":model,"source":source,"entry":"normal workspace -> restore exported job -> original evidence","effects":"new isolated job profile only; never provider dispatch or authority renewal"})).unwrap()).unwrap();
            let original = crate::plugins::jobs::Bundle::read_export(&source).unwrap();
            let source_bytes = std::fs::read(&source).unwrap();
            let input_bytes = std::fs::read(&model).unwrap();
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.host.document.editor = Editor::from_stored_document(&model).unwrap();
            app.path = model.display().to_string();
            app.host
                .configure_plugins(&root.join("restored-profile"))
                .unwrap();
            let before = app.host.document.editor.project.clone();
            let context = app.host.context();
            button(&ctx, &mut app, "Plugins and jobs…");
            button(&ctx, &mut app, "Restore an exported job record");
            let invalid = root.join("invalid.job.json");
            std::fs::write(&invalid, b"{invalid").unwrap();
            text(
                &ctx,
                &mut app,
                "Choose an exported job record",
                &invalid.display().to_string(),
            );
            button(&ctx, &mut app, "Import original job record");
            let until = Instant::now() + Duration::from_secs(5);
            while app.plugins_ui.import_worker.is_some() {
                layers_frame_output(&ctx, &mut app, vec![]);
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(!app.plugins_ui.error.is_empty());
            assert!(
                app.host
                    .jobs
                    .as_ref()
                    .unwrap()
                    .ids(app.host.document.editor.project_identity())
                    .unwrap()
                    .is_empty()
            );
            text(
                &ctx,
                &mut app,
                &invalid.display().to_string(),
                &source.display().to_string(),
            );
            button(&ctx, &mut app, "Import original job record");
            let until = Instant::now() + Duration::from_secs(5);
            while app.plugins_ui.import_worker.is_some() {
                layers_frame_output(&ctx, &mut app, vec![]);
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(app.plugins_ui.error.is_empty(), "{}", app.plugins_ui.error);
            assert_eq!(app.plugins_ui.record.as_ref().unwrap(), &original);
            assert!(
                app.plugins_ui
                    .record
                    .as_ref()
                    .unwrap()
                    .current(&app.host.document.editor)
            );
            assert_eq!(app.host.plugins.entries().len(), 2);
            assert!(
                app.host
                    .plugins
                    .resolve(&original.intent.identity.command_id)
                    .is_err()
            );
            let output = layers_frame_output(&ctx, &mut app, vec![]);
            assert!(output.shapes.iter().any(|s| {
                text_center(
                    std::slice::from_ref(s),
                    language
                        .translate("Original job record restored; no provider was enabled or run."),
                )
                .is_some_and(|at| s.clip_rect.contains(at))
            }));
            button(&ctx, &mut app, "Restore an exported job record");
            button(&ctx, &mut app, "Import original job record");
            let until = Instant::now() + Duration::from_secs(5);
            while app.plugins_ui.import_worker.is_some() {
                layers_frame_output(&ctx, &mut app, vec![]);
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(1));
            }
            let id = &original.intent.identity;
            assert_eq!(
                app.host.jobs.as_ref().unwrap().ids(&id.project_id).unwrap(),
                vec![id.request_id.clone()]
            );
            assert!(
                app.host
                    .jobs
                    .as_ref()
                    .unwrap()
                    .progress(&id.project_id, &id.request_id)
                    .is_none()
            );
            assert_eq!(app.host.context(), context);
            assert_eq!(app.host.document.editor.project, before);
            assert!(app.host.document.editor.undo.is_empty());
            assert_eq!(std::fs::read(&source).unwrap(), source_bytes);
            assert_eq!(std::fs::read(&model).unwrap(), input_bytes);
            let fresh = crate::plugins::jobs::Jobs::new(root.join("restored-profile/plugin-jobs"));
            assert_eq!(
                fresh.query(&id.project_id, &id.request_id).unwrap(),
                original
            );
            assert!(fresh.process_receipt(id).unwrap().is_none());
            std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"pass","original":id,"model_unchanged":true,"no_provider_or_dispatch":true,"process_logs":"unknown; export did not contain them","UI":"normal events, not native package"})).unwrap()).unwrap();
            println!("RESTORED_JOB_UI={}", root.display());
        }
    }
    #[test]
    fn actual_import_completion_after_normal_project_open_keeps_original_project_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let root = std::env::temp_dir().join(format!(
                "oa-import-switch-{}",
                storage::new_document_identity()
            ));
            std::fs::create_dir(&root).unwrap();
            let mut original_host =
                crate::application_host::ApplicationHost::new(storage::startup_example());
            original_host
                .configure_plugins(&root.join("origin"))
                .unwrap();
            let id = storage::new_document_identity();
            original_host
                .start_plugin_job("org.openarchi.quantities.report", &id)
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            let bundle = loop {
                original_host.jobs.as_mut().unwrap().poll();
                let b = original_host
                    .jobs
                    .as_ref()
                    .unwrap()
                    .query(original_host.document.editor.project_identity(), &id)
                    .unwrap();
                if b.result.is_some() {
                    break b;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            };
            let source = root.join("original.job.json");
            bundle.export(&source).unwrap();
            let model = root.join("original.openarchi.json");
            storage::save_document(
                &original_host.document.editor.project,
                &model,
                &bundle.intent.identity.project_id,
            )
            .unwrap();
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.host.document.editor = Editor::from_stored_document(&model).unwrap();
            app.path = model.display().to_string();
            app.host.configure_plugins(&root.join("restored")).unwrap();
            button(&ctx, &mut app, "Plugins and jobs…");
            button(&ctx, &mut app, "Restore an exported job record");
            text(
                &ctx,
                &mut app,
                "Choose an exported job record",
                &source.display().to_string(),
            );
            button(&ctx, &mut app, "Import original job record");
            // The real product worker still performs the real import. Only hold
            // delivery of its completion, making the UI interleaving deterministic.
            // No fake identity, injected result, alternate permission or task guard.
            let actual = app.plugins_ui.import_worker.take().unwrap();
            let (release, wait) = std::sync::mpsc::channel();
            app.plugins_ui.import_worker = Some(std::thread::spawn(move || {
                wait.recv().unwrap();
                actual.join().unwrap()
            }));
            button(&ctx, &mut app, "Close");
            let other = root.join("other.openarchi.json");
            storage::save_document(
                &Project::default(),
                &other,
                &storage::new_document_identity(),
            )
            .unwrap();
            open_test_project(&ctx, &mut app, &other);
            let new_project = app.host.document.editor.project_identity().to_owned();
            assert_ne!(new_project, bundle.intent.identity.project_id);
            let context = app.host.context();
            let untouched = app.host.document.editor.project.clone();
            release.send(()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while app.plugins_ui.import_worker.is_some() {
                layers_frame_output(&ctx, &mut app, vec![]);
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
            button(&ctx, &mut app, "Plugins and jobs…");
            assert!(app.plugins_ui.record.is_none());
            assert_eq!(app.host.context(), context);
            assert_eq!(app.host.document.editor.project, untouched);
            assert!(
                app.host
                    .jobs
                    .as_ref()
                    .unwrap()
                    .ids(&new_project)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(
                app.host
                    .jobs
                    .as_ref()
                    .unwrap()
                    .query(&bundle.intent.identity.project_id, &id)
                    .unwrap(),
                bundle
            );
            button(&ctx, &mut app, "Close");
            open_test_project(&ctx, &mut app, &model);
            button(&ctx, &mut app, "Plugins and jobs…");
            let frame = layers_frame_output(&ctx, &mut app, vec![]);
            let mut painted = vec![];
            collect_text(&frame.shapes, &mut painted);
            std::fs::write(root.join("return-diagnostic.json"),serde_json::to_vec_pretty(&serde_json::json!({"path":app.path,"project":app.host.document.editor.project_identity(),"expected":bundle.intent.identity.project_id,"file_error":app.file_error,"file_action":app.file_action.is_some(),"plugin_open":app.plugins_ui.open,"plugin_error":app.plugins_ui.error,"ids":app.host.jobs.as_ref().unwrap().ids(app.host.document.editor.project_identity()),"texts":painted})).unwrap()).unwrap();
            println!("IMPORT_SWITCH_ROOT={}", root.display());
            assert_eq!(
                app.host.document.editor.project_identity(),
                bundle.intent.identity.project_id,
                "actual file Open must restore original document"
            );
            assert_eq!(app.plugins_ui.record.as_ref().unwrap(), &bundle);
            assert!(
                app.plugins_ui
                    .record
                    .as_ref()
                    .unwrap()
                    .current(&app.host.document.editor)
            );
            assert!(
                app.host
                    .jobs
                    .as_ref()
                    .unwrap()
                    .progress(&bundle.intent.identity.project_id, &id)
                    .is_none()
            );
            println!("IMPORT_SWITCH_REAL_WORKER={}", root.display());
        }
    }
    #[test]
    fn native_project_embedded_original_jobs_open_without_local_store_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let input = std::env::var_os("OPENARCHI_ARCHIVE_UI_INPUT").map(PathBuf::from);
            if input.is_some() && language == UiLanguage::SimplifiedChinese {
                continue;
            }
            let root = std::env::var_os("OPENARCHI_ARCHIVE_UI_OUTPUT")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::temp_dir().join(format!(
                        "oa-open-archive-{}",
                        storage::new_document_identity()
                    ))
                });
            std::fs::create_dir(&root).expect("preserve prior read-only recovery evidence");
            let input = if let Some(input) = input {
                input
            } else {
                let mut h =
                    crate::application_host::ApplicationHost::new(storage::startup_example());
                h.configure_plugins(&root.join("origin")).unwrap();
                h.plugins
                    .enable(
                        crate::plugins::Manifest::parse(include_bytes!(
                            "../../assets/plugins/example-quantities.json"
                        ))
                        .unwrap(),
                    )
                    .unwrap();
                let id = storage::new_document_identity();
                h.start_plugin_job("example.quantities.review", &id)
                    .unwrap();
                let until = Instant::now() + Duration::from_secs(5);
                loop {
                    h.jobs.as_mut().unwrap().poll();
                    if h.jobs
                        .as_ref()
                        .unwrap()
                        .query(h.document.editor.project_identity(), &id)
                        .unwrap()
                        .result
                        .is_some()
                    {
                        break;
                    }
                    assert!(Instant::now() < until);
                    std::thread::sleep(Duration::from_millis(1));
                }
                let project = h.project_snapshot_for_save().collect().unwrap();
                let path = root.join("carried.openarchi.json");
                storage::save_document(&project, &path, h.document.editor.project_identity())
                    .unwrap();
                path
            };
            let original = std::fs::read(&input).unwrap();
            let expected = Editor::from_stored_document(&input).unwrap();
            let records = expected.project.job_archive.records().to_vec();
            assert!(!records.is_empty());
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.host
                .configure_plugins(&root.join("fresh-profile"))
                .unwrap();
            open_test_project(&ctx, &mut app, &input);
            button(&ctx, &mut app, "Plugins and jobs…");
            layers_frame_output(&ctx, &mut app, vec![]);
            assert_eq!(
                app.host.document.editor.project_identity(),
                expected.project_identity()
            );
            assert_eq!(app.host.document.editor.project, expected.project);
            assert!(
                app.host
                    .jobs
                    .as_ref()
                    .unwrap()
                    .ids(expected.project_identity())
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(app.host.plugins.entries().len(), 2);
            for record in &records {
                let id = &record.bundle.intent.identity.request_id;
                button(&ctx, &mut app, &id[..12]);
                assert_eq!(app.plugins_ui.record.as_ref().unwrap(), &record.bundle);
                assert!(
                    app.plugins_ui
                        .record
                        .as_ref()
                        .unwrap()
                        .current(&app.host.document.editor)
                );
                assert!(
                    app.host
                        .jobs
                        .as_ref()
                        .unwrap()
                        .progress(expected.project_identity(), id)
                        .is_none()
                );
            }
            assert_eq!(std::fs::read(&input).unwrap(), original);
            assert!(app.host.document.editor.undo.is_empty());
            button(&ctx, &mut app, "Review current candidate quantities");
            let new_request = app.plugins_ui.original_request.clone();
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                layers_frame_output(&ctx, &mut app, vec![]);
                if app
                    .plugins_ui
                    .record
                    .as_ref()
                    .is_some_and(|r| r.result.is_some())
                {
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(
                !records
                    .iter()
                    .any(|r| r.bundle.intent.identity.request_id == new_request)
            );
            button(&ctx, &mut app, "Close");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let saved = root.join("saved-with-new-job.openarchi.json");
            text(
                &ctx,
                &mut app,
                &input.display().to_string(),
                &saved.display().to_string(),
            );
            button(&ctx, &mut app, "Save");
            layers_frame_output(&ctx, &mut app, vec![]);
            let reopened = Editor::from_stored_document(&saved).unwrap();
            assert_eq!(
                reopened.project.job_archive.records().len(),
                records.len() + 1
            );
            for record in &records {
                assert_eq!(
                    reopened
                        .project
                        .job_archive
                        .get(
                            expected.project_identity(),
                            &record.bundle.intent.identity.request_id
                        )
                        .unwrap()
                        .unwrap(),
                    record
                );
            }
            assert!(
                reopened
                    .project
                    .job_archive
                    .get(expected.project_identity(), &new_request)
                    .unwrap()
                    .is_some()
            );
            assert_eq!(reopened.project.elements, expected.project.elements);
            assert_eq!(
                reopened.project.design_process,
                expected.project.design_process
            );
            assert_eq!(
                reopened.project.drawings.diagrams,
                expected.project.drawings.diagrams
            );
            assert!(app.host.document.editor.undo.is_empty());
            assert_eq!(std::fs::read(&input).unwrap(), original);
            std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"pass","input":input,"saved":saved,"original_ids":records.iter().map(|r|&r.bundle.intent.identity.request_id).collect::<Vec<_>>(),"new_explicit_readonly_request":new_request,"saved_job_count":records.len()+1,"local_job_store":"empty before explicit NEW first-party query; no old request replayed","provider":"external provider not registered or run","UI":"normal Open/Plugins/NEW query/Save events, not packaged native acceptance"})).unwrap()).unwrap();
            println!("PROJECT_ARCHIVE_UI={}", root.display());
        }
    }
    #[test]
    fn original_native_operation_after_readonly_job_save_can_query_and_safe_undo_bilingual() {
        use crate::design_process::{Entry, service};
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let root = std::env::temp_dir().join(format!(
                "oa-archive-op-ui-{}",
                storage::new_document_identity()
            ));
            std::fs::create_dir(&root).unwrap();
            println!("ARCHIVE_OPERATION_UI_ROOT={}", root.display());
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.host.document.editor = Editor::new(storage::synthetic_building_fixture());
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            app.host.configure_plugins(&root.join("profile")).unwrap();
            let before = app.host.document.editor.project.clone();
            let task = storage::new_document_identity();
            // Same trusted native confirmation used by the existing exploration UI;
            // no remote grant or evaluator-only TaskGuard is installed.
            let proposal = service::prepare_create(
                &app.host.document.editor,
                &task,
                service::Draft {
                    name: "Readonly check must not change design ownership".into(),
                    question: "Save a review and keep the original operation inspectable/undoable"
                        .into(),
                    statements: vec![],
                    candidate_names: ["A".into(), "B".into()],
                    source_ids: before
                        .elements
                        .iter()
                        .filter(|e| e.story == 0 && matches!(e.shape, Shape::Zone { .. }))
                        .map(|e| e.id)
                        .collect(),
                    entry: Entry::ManualInterface,
                },
            )
            .unwrap();
            let request = storage::new_document_identity();
            let target = app.host.operation_target();
            let result = app.host.confirm_task_prepared(
                app.host.context(),
                &request,
                "Archive integration fixture",
                std::rc::Rc::new(proposal),
            );
            assert_eq!(result["result"]["status"], "committed", "{result}");
            let initial = app.host.inspect_operation(target.clone(), &request);
            assert_eq!(initial["result"]["current_effect"], "AfterImage");
            button(&ctx, &mut app, "Plugins and jobs…");
            button(&ctx, &mut app, "Review current candidate quantities");
            let original_job = app.plugins_ui.original_request.clone();
            let until = Instant::now() + Duration::from_secs(5);
            loop {
                layers_frame_output(&ctx, &mut app, vec![]);
                if app
                    .plugins_ui
                    .record
                    .as_ref()
                    .is_some_and(|r| r.result.is_some())
                {
                    break;
                }
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(1));
            }
            let model_revision = app.host.document.editor.revision;
            button(&ctx, &mut app, "Close");
            let previous = app.path.clone();
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let file = root.join("saved-with-review.openarchi.json");
            text(&ctx, &mut app, &previous, &file.display().to_string());
            button(&ctx, &mut app, "Save");
            layers_frame_output(&ctx, &mut app, vec![]);
            assert_eq!(app.host.document.editor.revision, model_revision);
            assert_eq!(
                app.host.document.editor.project.job_archive.records().len(),
                1
            );
            let saved = std::fs::read(&file).unwrap();
            let after = app.host.inspect_operation(target.clone(), &request);
            app.automation_window = true;
            button(&ctx, &mut app, "Safe Undo");
            layers_frame_output(&ctx, &mut app, vec![]);
            let undo = app.host.inspect_operation(target, &request);
            std::fs::write(root.join("boundary-result.json"),serde_json::to_vec_pretty(&serde_json::json!({"initial":initial,"after_save":after,"after_undo":undo,"undo_feedback":app.automation_feedback,"job":original_job,"saved_file":file,"elements_before":before.elements.len(),"elements_after_undo":app.host.document.editor.project.elements.len(),"archive_count":app.host.document.editor.project.job_archive.records().len()})).unwrap()).unwrap();
            assert_eq!(
                after["result"]["current_effect"], "AfterImage",
                "saving read-only history must not change the original model effect: {after}"
            );
            assert_eq!(
                app.automation_feedback, "undone",
                "{}",
                app.automation_feedback
            );
            assert_eq!(undo["result"]["current_effect"], "BeforeImage");
            assert_eq!(app.host.document.editor.project.elements, before.elements);
            assert_eq!(
                app.host.document.editor.project.job_archive.records().len(),
                1
            );
            assert_eq!(std::fs::read(file).unwrap(), saved);
        }
    }
    #[test]
    fn embedded_jobs_survive_normal_repository_checkpoint_and_recovery_events_bilingual() {
        use crate::command::ModelCommand;
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let root = std::env::temp_dir().join(format!(
                "oa-archive-warehouse-ui-{}",
                storage::new_document_identity()
            ));
            std::fs::create_dir(&root).unwrap();
            println!("ARCHIVE_WAREHOUSE_UI_ROOT={}", root.display());
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.host.document.editor = Editor::new(storage::startup_example());
            app.host.configure_plugins(&root.join("profile")).unwrap();
            app.initialize_recovery(&root.join("recovery-profile"));
            button(&ctx, &mut app, "Plugins and jobs…");
            button(&ctx, &mut app, "Review current candidate quantities");
            let until = Instant::now() + Duration::from_secs(5);
            while app
                .plugins_ui
                .record
                .as_ref()
                .is_none_or(|r| r.result.is_none())
            {
                layers_frame_output(&ctx, &mut app, vec![]);
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(1));
            }
            let original = app.plugins_ui.record.clone().unwrap();
            button(&ctx, &mut app, "Close");
            // Use the regular workbench command and real TextEdit/button events.
            app.execute_workbench(crate::workbench::WorkbenchCommand::Sources)
                .unwrap();
            // The retained command now focuses the single conditions window.
            // Expand its source group through normal events; archive assertions stay intact.
            button(&ctx, &mut app, "Stored originals and source files");
            layers_frame_output(&ctx, &mut app, vec![]);
            layers_frame_output(&ctx, &mut app, vec![]);
            let f = layers_frame_output(&ctx, &mut app, vec![]);
            let label = text_center(&f.shapes, language.translate("Project folder")).unwrap();
            layers_click(&ctx, &mut app, pos2(label.x + 180., label.y));
            let repository = root.join("repository");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![
                    key(Key::A, Modifiers::COMMAND),
                    Event::Text(repository.display().to_string()),
                ],
            );
            assert_eq!(app.source_ui.root, repository.display().to_string());
            button(&ctx, &mut app, "Create from current model");
            let until = Instant::now() + Duration::from_secs(10);
            while app.source_ui.pending_for_test() {
                layers_frame_output(&ctx, &mut app, vec![]);
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(app.source_ui.error.is_empty(), "{}", app.source_ui.error);
            let first = app
                .source_ui
                .repository
                .as_ref()
                .unwrap()
                .load_model()
                .unwrap();
            assert_eq!(first.project.job_archive.records()[0].bundle, original);
            let id = original.intent.input.elements[0].id;
            app.host
                .document
                .editor
                .apply(ModelCommand::MoveElements {
                    ids: vec![id],
                    offset: Point::new(0.2, 0.),
                })
                .unwrap();
            let expected = app.host.document.editor.project.clone();
            button(&ctx, &mut app, "Save model checkpoint");
            let until = Instant::now() + Duration::from_secs(10);
            while app.source_ui.pending_for_test() {
                layers_frame_output(&ctx, &mut app, vec![]);
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(app.source_ui.error.is_empty(), "{}", app.source_ui.error);
            let stored = app
                .source_ui
                .repository
                .as_ref()
                .unwrap()
                .load_model()
                .unwrap();
            assert!(stored.project.same_model_state(&expected));
            assert_eq!(stored.project.job_archive.records()[0].bundle, original);
            assert!(!original.current(&stored));
            let snapshot = app.recovery_slot.as_ref().unwrap().snapshot().to_owned();
            let recovered = Editor::from_stored_document(&snapshot).unwrap();
            assert!(recovered.project.same_model_state(&expected));
            assert_eq!(recovered.project.job_archive.records()[0].bundle, original);
            drop(app);
            let (next_ctx, mut next) = setup();
            next.language = language;
            next.initialize_recovery(&root.join("recovery-profile"));
            assert_eq!(next.recovery_candidates.len(), 1);
            button(&next_ctx, &mut next, "Recover");
            assert!(
                next.host
                    .document
                    .editor
                    .project
                    .same_model_state(&expected)
            );
            assert_eq!(
                next.host.document.editor.project.job_archive.records()[0].bundle,
                original
            );
            assert!(next.host.document.editor.dirty());
        }
    }
    #[test]
    fn normal_solar_job_settings_cancel_run_reuse_and_saved_reopen_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let root = std::env::temp_dir()
                .join(format!("oa-solar-ui-{}", storage::new_document_identity()));
            std::fs::create_dir(&root).unwrap();
            println!("SOLAR_UI_RUNTIME_ROOT={}", root.display());
            let source = std::env::var_os("OPENARCHI_SOLAR_UI_INPUT").map(PathBuf::from);
            let input = if let Some(source) = source {
                let target = root.join(source.file_name().unwrap());
                std::fs::copy(&source, &target).unwrap();
                let v: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(&source).unwrap()).unwrap();
                for page in v["drawings"]["pages"].as_array().into_iter().flatten() {
                    let rel = PathBuf::from(page["path"].as_str().unwrap());
                    assert!(
                        !rel.is_absolute()
                            && !rel
                                .components()
                                .any(|p| matches!(p, std::path::Component::ParentDir))
                    );
                    let dest = root.join(&rel);
                    std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
                    std::fs::copy(source.parent().unwrap().join(rel), dest).unwrap();
                }
                target
            } else {
                let target = root.join("input.openarchi.json");
                storage::save(&storage::startup_example(), &target).unwrap();
                target
            };
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.host.configure_plugins(&root.join("profile")).unwrap();
            open_test_project(&ctx, &mut app, &input);
            let before = app.host.document.editor.project.clone();
            let original_jobs = before.job_archive.records().len();
            let context = app.host.context();
            let option = before.plan_options.active_design_option;
            button(&ctx, &mut app, "Plugins and jobs…");
            button(&ctx, &mut app, "Study sunlight for current candidate…");
            button(&ctx, &mut app, "Run declared sunlight study");
            assert!(app.plugins_ui.original_request.is_empty());
            assert_eq!(app.host.context(), context);
            button(&ctx, &mut app, "Cancel study settings");
            assert_eq!(app.host.document.editor.project, before);
            button(&ctx, &mut app, "Study sunlight for current candidate…");
            for (label, value) in [
                ("Latitude (degrees, -90 to 90)", "39.742476"),
                ("Longitude (east-positive degrees)", "-105.1786"),
                ("Assumed north clockwise from model Y (degrees)", "0"),
                (
                    "Design assumption note",
                    "Explicit synthetic software scenario; not project survey or approval",
                ),
                ("Start UTC (YYYY-MM-DDTHH:MM:SSZ)", "2003-10-17T18:00:00Z"),
                ("End UTC (YYYY-MM-DDTHH:MM:SSZ)", "2003-10-17T19:00:00Z"),
                ("Horizontal receiver point (x, y, z metres)", "0, 0, 100"),
                ("Sampling step (seconds)", "600"),
            ] {
                for _ in 0..8 {
                    layers_frame_output(&ctx, &mut app, vec![]);
                }
                let rect = ctx
                    .data(|d| d.get_temp::<Rect>(Id::new(("solar_field", label))))
                    .expect(label);
                assert!(rect.is_positive(), "field clipped: {label}");
                layers_click(&ctx, &mut app, rect.center());
                layers_frame_output(
                    &ctx,
                    &mut app,
                    vec![key(Key::A, Modifiers::COMMAND), Event::Text(value.into())],
                );
                layers_frame_output(&ctx, &mut app, vec![key(Key::Tab, Modifiers::NONE)]);
            }
            button(&ctx, &mut app, "Run declared sunlight study");
            let id = app.plugins_ui.original_request.clone();
            assert_eq!(id.len(), 64, "run failed: {}", app.plugins_ui.error);
            let deadline = Instant::now() + Duration::from_secs(8);
            loop {
                layers_frame_output(&ctx, &mut app, vec![]);
                if app
                    .plugins_ui
                    .record
                    .as_ref()
                    .is_some_and(|b| b.result.is_some())
                {
                    break;
                }
                assert!(Instant::now() < deadline, "{}", app.plugins_ui.error);
                std::thread::sleep(Duration::from_millis(2));
            }
            let original = app.plugins_ui.record.clone().unwrap();
            let crate::plugins::jobs::Terminal::Completed { report } =
                &original.result.as_ref().unwrap().terminal
            else {
                panic!("job failed")
            };
            let solar = report.solar.as_ref().unwrap();
            assert_eq!(solar.points[0].sampled_sunlit_seconds, 3600);
            assert_eq!(solar.points[0].uncertain_seconds, 0);
            assert_eq!(solar.points[0].cells.len(), 6);
            assert!(original.current(&app.host.document.editor));
            assert_eq!(app.host.document.editor.project, before);
            assert_eq!(app.host.context(), context);
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .plan_options
                    .active_design_option,
                option
            );
            button(&ctx, &mut app, "Reuse this study's inputs");
            for _ in 0..8 {
                layers_frame_output(&ctx, &mut app, vec![]);
            }
            let f = layers_frame_output(&ctx, &mut app, vec![]);
            let mut words = vec![];
            collect_text(&f.shapes, &mut words);
            assert!(words.iter().any(|t| t == "39.742476"));
            button(&ctx, &mut app, "Cancel study settings");
            button(&ctx, &mut app, "Close");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let old = app.path.clone();
            let saved = root.join("saved-solar.openarchi.json");
            text(&ctx, &mut app, &old, &saved.display().to_string());
            button(&ctx, &mut app, "Save");
            let deadline = Instant::now() + Duration::from_secs(10);
            while !saved.exists() {
                layers_frame_output(&ctx, &mut app, vec![]);
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
            let stored = Editor::from_stored_document(&saved).unwrap();
            assert_eq!(
                stored.project.job_archive.records().len(),
                original_jobs + 1
            );
            let record = stored
                .project
                .job_archive
                .get(stored.project_identity(), &id)
                .unwrap()
                .unwrap();
            assert_eq!(record.bundle, original);
            let mut normalized = stored.project.clone();
            normalized.job_archive = before.job_archive.clone();
            for (new, old) in normalized
                .drawings
                .pages
                .iter_mut()
                .zip(&before.drawings.pages)
            {
                assert_eq!(
                    crate::drawings::verified_bytes(new).unwrap(),
                    crate::drawings::verified_bytes(old).unwrap()
                );
                new.path = old.path.clone();
            }
            assert_eq!(normalized, before);
            let raw: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&saved).unwrap()).unwrap();
            assert_eq!(raw["schema_version"], 21);
            let (next_ctx, mut next) = setup();
            next.language = language;
            next_ctx.global_style_mut(|s| s.animation_time = 0.);
            next.host
                .configure_plugins(&root.join("fresh-profile"))
                .unwrap();
            open_test_project(&next_ctx, &mut next, &saved);
            button(&next_ctx, &mut next, "Plugins and jobs…");
            button(&next_ctx, &mut next, &id[..12]);
            assert_eq!(next.plugins_ui.record.as_ref().unwrap(), &original);
            assert!(
                next.plugins_ui
                    .record
                    .as_ref()
                    .unwrap()
                    .current(&next.host.document.editor)
            );
            println!("SOLAR_NORMAL_UI={}", root.display());
        }
    }
    #[test]
    fn same_project_new_document_session_reloads_original_job_instead_of_cached_terminal_bilingual()
    {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let root = std::env::temp_dir().join(format!(
                "oa-solar-session-{}",
                storage::new_document_identity()
            ));
            std::fs::create_dir(&root).unwrap();
            let mut source =
                crate::application_host::ApplicationHost::new(storage::startup_example());
            source.configure_plugins(&root.join("producer")).unwrap();
            let v: serde_json::Value = serde_json::from_str(include_str!(
                "../../tests/fixtures/solar/concept-scenario.json"
            ))
            .unwrap();
            let params = serde_json::from_value(
                serde_json::json!({"request":v["request"],"assumed_north":v["assumed_north"]}),
            )
            .unwrap();
            let id = storage::new_document_identity();
            source.start_solar_job(&params, &id).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                source.jobs.as_mut().unwrap().poll();
                if source
                    .jobs
                    .as_ref()
                    .unwrap()
                    .query(source.document.editor.project_identity(), &id)
                    .unwrap()
                    .result
                    .is_some()
                {
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
            let complete = root.join("complete.openarchi.json");
            storage::save_document(
                &source.project_snapshot_for_save().collect().unwrap(),
                &complete,
                source.document.editor.project_identity(),
            )
            .unwrap();
            let original_bytes = std::fs::read(&complete).unwrap();
            let mut missing: serde_json::Value = serde_json::from_slice(&original_bytes).unwrap();
            missing["job_archive"]["records"][0]["bundle"]["result"] = serde_json::Value::Null;
            let loss = root.join("protected-lost-result.openarchi.json");
            std::fs::write(&loss, serde_json::to_vec_pretty(&missing).unwrap()).unwrap();
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.host
                .configure_plugins(&root.join("consumer-empty-jobs"))
                .unwrap();
            for (path, terminal) in [(&complete, true), (&loss, false), (&complete, true)] {
                let prior = app.host.document.editor.document_session();
                open_test_project(&ctx, &mut app, &path);
                assert_ne!(app.host.document.editor.document_session(), prior);
                button(&ctx, &mut app, "Plugins and jobs…");
                for _ in 0..6 {
                    layers_frame_output(&ctx, &mut app, vec![]);
                }
                let b = app.plugins_ui.record.as_ref().unwrap();
                assert_eq!(b.intent.identity.request_id, id);
                assert_eq!(
                    b.result.is_some(),
                    terminal,
                    "cached terminal crossed document session"
                );
                assert!(
                    app.host
                        .jobs
                        .as_ref()
                        .unwrap()
                        .ids(app.host.document.editor.project_identity())
                        .unwrap()
                        .is_empty(),
                    "read/restore may not replay the worker"
                );
                if !terminal {
                    button(&ctx, &mut app, "Study sunlight for current candidate…");
                    assert_eq!(app.plugins_ui.original_request, id);
                    assert!(app.plugins_ui.record.as_ref().unwrap().result.is_none());
                }
                button(&ctx, &mut app, "Close");
            }
            assert_eq!(std::fs::read(complete).unwrap(), original_bytes);
        }
    }
    fn button(ctx: &Context, app: &mut AtelierApp, key: &str) {
        reveal_manual_control(ctx, app, key);
        for attempt in 0..24 {
            let f = layers_frame_output(ctx, app, vec![]);
            if let Some(p) = f.shapes.iter().find_map(|s| {
                if key.len() == 12 && key.bytes().all(|c| c.is_ascii_hexdigit()) {
                    if let egui::Shape::Text(t) = &s.shape {
                        if t.galley.text().ends_with(key) {
                            let r =
                                Rect::from_min_size(t.pos, t.galley.size()).intersect(s.clip_rect);
                            if r.is_positive() {
                                return Some(r.center());
                            }
                        }
                    }
                }
                text_center(std::slice::from_ref(s), app.language.translate(key))
                    .filter(|p| s.clip_rect.contains(*p))
            }) {
                layers_click(ctx, app, p);
                return;
            }
            if attempt == 4 && key.len() == 12 && key.bytes().all(|c| c.is_ascii_hexdigit()) {
                button(ctx, app, "Job history");
            } else if attempt > 4 {
                let pos = ctx
                    .memory(|m| m.area_rect(Id::new("plugin_jobs")))
                    .map(|r| r.center())
                    .unwrap_or(pos2(550., 600.));
                layers_frame_output(
                    ctx,
                    app,
                    vec![
                        Event::PointerMoved(pos),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            phase: egui::TouchPhase::Move,
                            delta: vec2(0., -160.),
                            modifiers: Modifiers::NONE,
                        },
                    ],
                );
            }
        }
        panic!("visible plugin control {key}; {}", app.plugins_ui.error)
    }
    fn text(ctx: &Context, app: &mut AtelierApp, old: &str, value: &str) {
        for _ in 0..12 {
            let f = layers_frame_output(ctx, app, vec![]);
            if let Some(at) = f.shapes.iter().find_map(|s| {
                if let egui::Shape::Text(t) = &s.shape {
                    if t.galley.text() == app.language.translate(old) {
                        let r = Rect::from_min_size(t.pos, t.galley.size()).intersect(s.clip_rect);
                        if r.is_positive() {
                            return Some(r.center());
                        }
                    }
                }
                None
            }) {
                layers_click(ctx, app, at);
                layers_frame_output(
                    ctx,
                    app,
                    vec![key(Key::A, Modifiers::COMMAND), Event::Text(value.into())],
                );
                return;
            }
            layers_frame_output(
                ctx,
                app,
                vec![
                    Event::PointerMoved(pos2(550., 600.)),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        delta: vec2(0., -120.),
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        panic!("visible plugin input {old}");
    }
    #[test]
    fn plugin_normal_workspace_external_declaration_quantity_disable_export_save_reopen_bilingual()
    {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let input = std::env::var_os("OPENARCHI_PLUGIN_INPUT").map(PathBuf::from);
            if input.is_some() && language == UiLanguage::SimplifiedChinese {
                continue;
            }
            let root = std::env::var_os("OPENARCHI_PLUGIN_OUTPUT")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::temp_dir()
                        .join(format!("oa-plugin-ui-{}", storage::new_document_identity()))
                });
            std::fs::create_dir(&root)
                .expect("existing output requires original-operation recovery, not write replay");
            println!("PLUGIN_PRODUCT_ROOT={}", root.display());
            std::fs::write(root.join("operation-plan.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"profile":root,"entry":"normal workspace plugin lifecycle and query","effects":"configuration/job artifacts/new project copy only; source read-only"})).unwrap()).unwrap();
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.host.document.editor = if let Some(p) = &input {
                Editor::from_stored_document(p).unwrap()
            } else {
                Editor::new(storage::startup_example())
            };
            if let Some(p) = &input {
                app.path = p.display().to_string();
            }
            app.host.configure_plugins(&root).unwrap();
            let before = app.host.document.editor.project.clone();
            let original_id = app.host.document.editor.project_identity().to_owned();
            let context = app.host.context();
            button(&ctx, &mut app, "Plugins and jobs…");
            button(&ctx, &mut app, "Load a local plugin declaration");
            let declaration = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("assets/plugins/example-quantities.json");
            text(
                &ctx,
                &mut app,
                "Choose a local JSON declaration",
                &declaration.display().to_string(),
            );
            button(&ctx, &mut app, "Read declaration");
            button(&ctx, &mut app, "Enable reviewed plugin");
            assert_eq!(app.host.plugins.entries().len(), 3);
            assert_eq!(app.host.document.editor.project, before);
            button(&ctx, &mut app, "Review candidate with example plugin");
            let request = app.plugins_ui.original_request.clone();
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                layers_frame_output(&ctx, &mut app, vec![]);
                if app
                    .plugins_ui
                    .record
                    .as_ref()
                    .is_some_and(|b| b.result.is_some())
                {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "original job pending: {}",
                    app.plugins_ui.error
                );
                std::thread::sleep(Duration::from_millis(1));
            }
            let b = app.plugins_ui.record.clone().unwrap();
            assert_eq!(b.intent.identity.request_id, request);
            assert!(b.current(&app.host.document.editor));
            assert_eq!(app.host.context(), context);
            assert_eq!(app.host.document.editor.project, before);
            assert!(app.host.document.editor.undo.is_empty());
            let f = layers_frame_output(&ctx, &mut app, vec![]);
            let mut painted = vec![];
            collect_text(&f.shapes, &mut painted);
            std::fs::write(root.join("first-result-visible-text.json"),serde_json::to_vec_pretty(&serde_json::json!({"open":app.plugins_ui.open,"texts":painted,"shapes":f.shapes.iter().filter_map(|s|if let egui::Shape::Text(t)=&s.shape {Some(serde_json::json!({"text":t.galley.text(),"pos":[t.pos.x,t.pos.y],"clip":[s.clip_rect.min.x,s.clip_rect.min.y,s.clip_rect.max.x,s.clip_rect.max.y]}))}else{None}).collect::<Vec<_>>()})).unwrap()).unwrap();
            assert!(
                text_center(
                    &f.shapes,
                    language.translate("Job input matches current candidate")
                )
                .is_some()
            );
            button(&ctx, &mut app, "Example quantity extension");
            assert!(
                app.host
                    .plugins
                    .resolve("example.quantities.review")
                    .is_err()
            );
            let f = layers_frame_output(&ctx, &mut app, vec![]);
            assert!(
                text_center(
                    &f.shapes,
                    language.translate("Provider unavailable; saved input and result retained.")
                )
                .is_some()
            );
            button(&ctx, &mut app, "Example quantity extension");
            assert_eq!(app.host.plugins.entries().len(), 3);
            button(&ctx, &mut app, "Example quantity extension");
            let exported = root.join("candidate.job.json");
            let old = app.plugins_ui.export_path.clone();
            text(&ctx, &mut app, &old, &exported.display().to_string());
            button(&ctx, &mut app, "Export job record");
            assert_eq!(
                crate::plugins::jobs::Bundle::read_export(&exported).unwrap(),
                b
            );
            button(&ctx, &mut app, "Close");
            assert!(!app.plugins_ui.open);
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let path = app.path.clone();
            let saved = root.join("continued.openarchi.json");
            text(&ctx, &mut app, &path, &saved.display().to_string());
            button(&ctx, &mut app, "Save");
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let (_, mut restored) = setup();
            restored.language = language;
            restored.host.document.editor = Editor::from_stored_document(&saved).unwrap();
            assert_eq!(
                restored.host.document.editor.project_identity(),
                original_id
            );
            restored.host.configure_plugins(&root).unwrap();
            button(&ctx, &mut restored, "Plugins and jobs…");
            layers_frame_output(&ctx, &mut restored, vec![]);
            assert_eq!(restored.plugins_ui.record.as_ref().unwrap(), &b);
            assert_eq!(
                restored.plugins_ui.export_path,
                format!("{}.{}.job.json", restored.path, &request[..12])
            );
            assert!(
                restored
                    .plugins_ui
                    .record
                    .as_ref()
                    .unwrap()
                    .current(&restored.host.document.editor)
            );
            let after = &restored.host.document.editor.project;
            assert_eq!(after.elements, before.elements);
            assert_eq!(after.design_process, before.design_process);
            assert_eq!(after.drawings.diagrams, before.drawings.diagrams);
            assert_eq!(after.site, before.site);
            for page in &after.drawings.pages {
                crate::drawings::verified_bytes(page).unwrap();
            }
            std::fs::write(root.join("walkthrough.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"output":saved,"job_export":exported,"project_id":original_id,"request_id":request,"entry":"normal workspace plugin declaration/read/explicit enable/run/disable/re-enable/export/Save/reopen","language":format!("{language:?}"),"model_task_and_figures":"unchanged","real_AI":"not_run","native_package":"not_run"})).unwrap()).unwrap();
            println!(
                "PLUGIN_PRODUCT_WALK={}",
                root.join("walkthrough.json").display()
            );
        }
    }
    #[test]
    fn quantity_report_returns_to_exact_object_edits_stales_and_reopens_bilingual() {
        report_object_path(false);
    }
    #[test]
    #[ignore = "explicit protected saved project and unused output required"]
    fn protected_quantity_report_object_edit_and_reopen_bilingual() {
        report_object_path(true);
    }
    fn report_object_path(supplied: bool) {
        fn wait_result(ctx: &Context, app: &mut AtelierApp) -> crate::plugins::jobs::Bundle {
            let until = Instant::now() + Duration::from_secs(10);
            loop {
                layers_frame_output(ctx, app, vec![]);
                if let Some(record) = app
                    .plugins_ui
                    .record
                    .as_ref()
                    .filter(|b| b.result.is_some())
                {
                    return record.clone();
                }
                assert!(
                    Instant::now() < until,
                    "original result unknown; inspect request, never replay"
                );
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        fn open_file(ctx: &Context, app: &mut AtelierApp, path: &std::path::Path) {
            open_test_project(ctx, app, &path);
            let until = Instant::now() + Duration::from_secs(20);
            while app.source_ui.pending_for_ui() {
                assert!(Instant::now() < until);
                layers_frame_output(ctx, app, vec![]);
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(app.source_ui.error.is_empty(), "{}", app.source_ui.error);
        }
        fn visible_result_action(ctx: &Context, app: &mut AtelierApp, label: &str) {
            // Let the expanded native egui window settle, then target its actual
            // accessible button. Scrolling must reveal it before invoking it.
            for _ in 0..8 {
                layers_frame_output(ctx, app, vec![]);
            }
            for _ in 0..40 {
                let output = layers_frame_output(ctx, app, vec![]);
                if output.shapes.iter().any(|s| {
                    text_center(std::slice::from_ref(s), label)
                        .is_some_and(|p| s.clip_rect.contains(p))
                }) {
                    let (id, _) = output
                        .platform_output
                        .accesskit_update
                        .unwrap()
                        .nodes
                        .into_iter()
                        .find(|(_, n)| {
                            n.label() == Some(label)
                                && n.supports_action(egui::accesskit::Action::Click)
                        })
                        .expect("actual visible result button");
                    layers_frame_output(
                        ctx,
                        app,
                        vec![Event::AccessKitActionRequest(
                            egui::accesskit::ActionRequest {
                                action: egui::accesskit::Action::Click,
                                target_tree: egui::accesskit::TreeId::ROOT,
                                target_node: id,
                                data: None,
                            },
                        )],
                    );
                    for _ in 0..4 {
                        layers_frame_output(ctx, app, vec![]);
                    }
                    return;
                }
                let pos = if app.plugins_ui.open {
                    ctx.memory(|m| m.area_rect(Id::new("plugin_jobs")))
                        .unwrap()
                        .center()
                } else {
                    ctx.data(|d| d.get_temp::<Rect>(Id::new("inspector_scroll_rect")))
                        .unwrap()
                        .center()
                };
                layers_frame_output(
                    ctx,
                    app,
                    vec![
                        Event::PointerMoved(pos),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            phase: egui::TouchPhase::Move,
                            delta: vec2(0., -120.),
                            modifiers: Modifiers::NONE,
                        },
                    ],
                );
            }
            panic!("visible original result action {label}");
        }
        fn top_button(ctx: &Context, app: &mut AtelierApp, key: &str) {
            for _ in 0..4 {
                layers_frame_output(ctx, app, vec![]);
            }
            assert!(app.plugins_ui.open, "normal plugin window opened");
            // A reopened result retains its scroll position. Return to its real
            // command area rather than searching only downward from old output.
            for _ in 0..5 {
                let pos = ctx
                    .memory(|m| m.area_rect(Id::new("plugin_jobs")))
                    .unwrap()
                    .center();
                layers_frame_output(
                    ctx,
                    app,
                    vec![
                        Event::PointerMoved(pos),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            phase: egui::TouchPhase::Move,
                            delta: vec2(0., 1000.),
                            modifiers: Modifiers::NONE,
                        },
                    ],
                );
            }
            for _ in 0..8 {
                layers_frame_output(ctx, app, vec![]);
            }
            button(ctx, app, key);
        }
        fn normalize(project: &mut Project, path: &std::path::Path) {
            for page in &mut project.drawings.pages {
                let bytes = std::fs::read(path.parent().unwrap().join(&page.path)).unwrap();
                assert_eq!(crate::drawings::digest(&bytes), page.sha256);
                page.path = format!("sha256:{}", page.sha256).into();
            }
        }
        let parent = if supplied {
            PathBuf::from(std::env::var_os("OPENARCHI_JOB_OBJECT_OUTPUT").unwrap())
        } else {
            std::env::temp_dir().join(format!(
                "oa-job-object-{}",
                storage::new_document_identity()
            ))
        };
        std::fs::create_dir(&parent)
            .expect("new evidence output; inspect old operation, no replay");
        eprintln!("JOB_OBJECT_ROOT={}", parent.display());
        for (language, tag) in [
            (UiLanguage::English, "en"),
            (UiLanguage::SimplifiedChinese, "zh"),
        ] {
            let root = parent.join(tag);
            std::fs::create_dir(&root).unwrap();
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.host.configure_plugins(&root.join("profile")).unwrap();
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            let input = if supplied {
                Some(PathBuf::from(
                    std::env::var_os("OPENARCHI_JOB_OBJECT_INPUT").unwrap(),
                ))
            } else {
                None
            };
            if let Some(path) = &input {
                open_file(&ctx, &mut app, path);
            } else {
                app.host.document.editor = Editor::new(storage::startup_example());
            }
            let original = app.host.document.editor.project.clone();
            let context = app.host.context();
            button(&ctx, &mut app, "Plugins and jobs…");
            top_button(&ctx, &mut app, "Review current candidate quantities");
            let first = wait_result(&ctx, &mut app);
            assert!(first.current(&app.host.document.editor));
            let crate::plugins::jobs::Terminal::Completed { report } =
                &first.result.as_ref().unwrap().terminal
            else {
                panic!("completed actual job");
            };
            let id = *report
                .zones
                .element_ids
                .last()
                .expect("existing project Zone");
            let element = original.elements.iter().find(|e| e.id == id).unwrap();
            let old_height = match &element.shape {
                Shape::Zone { spec } => spec.height,
                _ => unreachable!(),
            };
            let label = format!(
                "{} · {} #{}",
                language.translate("Inspect reviewed object in 3D"),
                language.translate("Zone"),
                id
            );
            button(&ctx, &mut app, "Reviewed objects");
            visible_result_action(&ctx, &mut app, &label);
            assert!(
                !app.plugins_ui.open,
                "result action returns directly to canvas: {}",
                app.plugins_ui.error
            );
            assert_eq!(app.view, View::Model);
            assert_eq!(app.host.document.editor.selection_ids(), vec![id]);
            assert_eq!(app.host.document.editor.project, original);
            assert_eq!(app.host.context(), context);
            assert_eq!(
                app.plugins_ui.original_request,
                first.intent.identity.request_id
            );
            assert_eq!(app.plugins_ui.record.as_ref(), Some(&first));
            assert!(!app.host.operations.borrow().enabled());
            if original
                .design_process
                .task()
                .is_some_and(|t| t.intent.as_ref().is_some_and(|i| i.parametric.is_some()))
            {
                button(&ctx, &mut app, "Adjust generated wings");
                let parameters = app
                    .research_ui
                    .parameters
                    .as_ref()
                    .unwrap()
                    .definition
                    .clone();
                button(&ctx, &mut app, "Plugins and jobs…");
                visible_result_action(&ctx, &mut app, &label);
                assert!(app.plugins_ui.open, "parameter draft prevents navigation");
                assert_eq!(
                    app.research_ui.parameters.as_ref().unwrap().definition,
                    parameters
                );
                assert_eq!(app.host.document.editor.project, original);
                button(&ctx, &mut app, "Close");
                visible_result_action(&ctx, &mut app, language.translate("Cancel parameter draft"));
                assert!(app.research_ui.parameters.is_none());
            }
            button(&ctx, &mut app, "Discuss selected objects");
            let exchange = app.research_ui.draft.as_ref().unwrap().exchange.clone();
            button(&ctx, &mut app, "Plugins and jobs…");
            visible_result_action(&ctx, &mut app, &label);
            assert!(
                app.plugins_ui.open,
                "frozen object question prevents navigation"
            );
            assert_eq!(app.research_ui.draft.as_ref().unwrap().exchange, exchange);
            assert_eq!(app.host.document.editor.project, original);
            assert_eq!(app.host.document.editor.selection_ids(), vec![id]);
            button(&ctx, &mut app, "Close");
            visible_result_action(
                &ctx,
                &mut app,
                language.translate("Cancel discussion draft"),
            );
            assert!(app.research_ui.draft.is_none());
            button(&ctx, &mut app, "Properties");
            // The existing property editor writes through its original transaction.
            super::linked_figure_product_path::height(&ctx, &mut app, old_height + 0.2);
            eprintln!("JOB_OBJECT located and property edit submitted {tag} id={id}");
            let edited = app.host.document.editor.project.clone();
            assert_ne!(edited, original);
            for old in &original.elements {
                let new = edited.elements.iter().find(|e| e.id == old.id).unwrap();
                if old.id == id {
                    let Shape::Zone { spec } = &new.shape else {
                        unreachable!()
                    };
                    assert_eq!(spec.height, old_height + 0.2);
                    let mut restored = new.clone();
                    restored.shape = old.shape.clone();
                    assert_eq!(&restored, old);
                } else {
                    assert_eq!(new, old);
                }
            }
            button(&ctx, &mut app, "Plugins and jobs…");
            let old_record = wait_result(&ctx, &mut app);
            assert_eq!(old_record, first);
            assert!(!first.current(&app.host.document.editor));
            visible_result_action(&ctx, &mut app, &label);
            assert!(
                app.plugins_ui.open,
                "stale result cannot navigate or retarget"
            );
            assert_eq!(app.host.document.editor.project, edited);
            assert_eq!(app.plugins_ui.record.as_ref(), Some(&first));
            button(&ctx, &mut app, "Close");
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(app.host.document.editor.project, original);
            assert!(first.current(&app.host.document.editor));
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            assert_eq!(app.host.document.editor.project, edited);
            assert!(!first.current(&app.host.document.editor));
            button(&ctx, &mut app, "Plugins and jobs…");
            top_button(&ctx, &mut app, "Review current candidate quantities");
            let second = wait_result(&ctx, &mut app);
            assert_ne!(
                first.intent.identity.request_id,
                second.intent.identity.request_id
            );
            assert!(second.current(&app.host.document.editor));
            assert_eq!(
                app.host
                    .jobs
                    .as_ref()
                    .unwrap()
                    .query(
                        app.host.document.editor.project_identity(),
                        &first.intent.identity.request_id
                    )
                    .unwrap(),
                first
            );
            button(&ctx, &mut app, "Close");
            let path = root.join("report-object.openarchi.json");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let old = app.file_dialog_path.clone().unwrap();
            text(&ctx, &mut app, &old, &path.display().to_string());
            button(&ctx, &mut app, "Save");
            let disk = storage::load_document(&path).unwrap();
            let (fresh_ctx, mut fresh) = setup();
            fresh_ctx.enable_accesskit();
            fresh.language = language;
            fresh_ctx.global_style_mut(|s| s.animation_time = 0.);
            fresh
                .host
                .configure_plugins(&root.join("empty-profile"))
                .unwrap();
            open_file(&fresh_ctx, &mut fresh, &path);
            let mut actual = fresh.host.document.editor.project.clone();
            let mut expected = edited.clone();
            expected.job_archive = disk.0.job_archive.clone();
            normalize(&mut expected, input.as_deref().unwrap_or(&path));
            normalize(&mut actual, &path);
            assert_eq!(actual, expected);
            let project = fresh.host.document.editor.project_identity().to_owned();
            for record in [&first, &second] {
                assert_eq!(
                    fresh
                        .host
                        .jobs
                        .as_ref()
                        .unwrap()
                        .bundle_with_archive(
                            &project,
                            &record.intent.identity.request_id,
                            &fresh.host.document.editor.project.job_archive
                        )
                        .unwrap(),
                    *record
                );
            }
            button(&fresh_ctx, &mut fresh, "Plugins and jobs…");
            // Open the carried request, never run it again in the empty local store.
            button(
                &fresh_ctx,
                &mut fresh,
                &second.intent.identity.request_id[..12],
            );
            button(&fresh_ctx, &mut fresh, "Reviewed objects");
            visible_result_action(&fresh_ctx, &mut fresh, &label);
            assert_eq!(fresh.host.document.editor.selection_ids(), vec![id]);
            assert!(!fresh.plugins_ui.open);
            let report = serde_json::json!({"input":input,"output":path,"language":tag,"id":id,"old_height":old_height,"new_height":old_height+0.2,"original_request":first.intent.identity.request_id,"new_explicit_request":second.intent.identity.request_id,"entry":"normal workspace/plugin result/3D/property editor/UndoRedo/SaveAs/newAppOpen","new_AI":"not_run","native_OS":"not_run"});
            std::fs::write(
                root.join("result.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
        }
    }
    #[test]
    fn plugin_current_result_stales_after_canvas_inspector_edit_and_history_preserves_job() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            let mut p = Project::default();
            p.add(
                0,
                Material::Timber,
                Shape::ConceptMass {
                    spec: crate::concept_mass::ConceptMassSpec {
                        arcs: vec![],
                        hole_arcs: vec![],
                        outline: vec![
                            Point::new(30., 30.),
                            Point::new(38., 30.),
                            Point::new(38., 36.),
                            Point::new(30., 36.),
                        ],
                        holes: vec![],
                        height: 6.,
                        bottom_offset: 0.,
                    },
                },
            );
            p.elements[0].layer = crate::layers::ElementLayer(0);
            app.host.document.editor = Editor::new(p);
            let before = app.host.document.editor.project.clone();
            let root = std::env::temp_dir().join(format!(
                "oa-plugin-stale-{}",
                storage::new_document_identity()
            ));
            std::fs::create_dir(&root).unwrap();
            app.host.configure_plugins(&root).unwrap();
            button(&ctx, &mut app, "Plugins and jobs…");
            button(&ctx, &mut app, "Review current candidate quantities");
            let until = Instant::now() + Duration::from_secs(3);
            loop {
                layers_frame_output(&ctx, &mut app, vec![]);
                if app
                    .plugins_ui
                    .record
                    .as_ref()
                    .is_some_and(|b| b.result.is_some())
                {
                    break;
                }
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(1));
            }
            let original = app.plugins_ui.record.clone().unwrap();
            let request = app.plugins_ui.original_request.clone();
            button(&ctx, &mut app, "Close");
            button(&ctx, &mut app, "2D Plan");
            app.fit();
            layers_frame_output(&ctx, &mut app, vec![]);
            let at = app.screen(
                app.workbench.panel_rects["view.canvas"],
                Point::new(34., 30.),
            );
            layers_click(&ctx, &mut app, at);
            assert_eq!(
                app.host.document.editor.selection_ids(),
                vec![before.elements[0].id]
            );
            super::linked_figure_product_path::height(&ctx, &mut app, 6.5);
            button(&ctx, &mut app, "Plugins and jobs…");
            let f = layers_frame_output(&ctx, &mut app, vec![]);
            assert!(
                text_center(
                    &f.shapes,
                    language.translate("Job result is stale for the current candidate")
                )
                .is_some()
            );
            assert_eq!(app.plugins_ui.record.as_ref().unwrap(), &original);
            assert_eq!(app.plugins_ui.original_request, request);
            button(&ctx, &mut app, "Close");
            super::linked_figure_product_path::button(&ctx, &mut app, "Undo");
            button(&ctx, &mut app, "Plugins and jobs…");
            let f = layers_frame_output(&ctx, &mut app, vec![]);
            assert!(
                text_center(
                    &f.shapes,
                    language.translate("Job input matches current candidate")
                )
                .is_some()
            );
            assert_eq!(app.host.document.editor.project, before);
            assert_eq!(app.plugins_ui.record.as_ref().unwrap(), &original);
            assert_eq!(
                app.host
                    .jobs
                    .as_ref()
                    .unwrap()
                    .ids(app.host.document.editor.project_identity())
                    .unwrap(),
                vec![request]
            );
        }
    }
    #[test]
    fn plugin_search_after_custom_export_rebinds_selected_original_job_and_preserves_same_job_path()
    {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.host.document.editor = Editor::new(storage::startup_example());
            let before = app.host.document.editor.project.clone();
            let root = std::env::temp_dir().join(format!(
                "oa-plugin-search-{}",
                storage::new_document_identity()
            ));
            std::fs::create_dir(&root).unwrap();
            app.host.configure_plugins(&root).unwrap();
            app.host
                .plugins
                .enable(
                    crate::plugins::Manifest::parse(include_bytes!(
                        "../../assets/plugins/example-quantities.json"
                    ))
                    .unwrap(),
                )
                .unwrap();
            button(&ctx, &mut app, "Plugins and jobs…");
            button(&ctx, &mut app, "Review candidate with example plugin");
            let first = app.plugins_ui.original_request.clone();
            let until = Instant::now() + Duration::from_secs(3);
            loop {
                layers_frame_output(&ctx, &mut app, vec![]);
                if app
                    .plugins_ui
                    .record
                    .as_ref()
                    .is_some_and(|b| b.result.is_some())
                {
                    break;
                }
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(1));
            }
            let old = app.plugins_ui.export_path.clone();
            text(&ctx, &mut app, &old, "custom-first.job.json");
            assert_eq!(
                app.plugins_ui.export_path, "custom-first.job.json",
                "custom target entered after original result completed"
            );
            button(&ctx, &mut app, &first[..12]);
            assert_eq!(
                app.plugins_ui.export_path, "custom-first.job.json",
                "same selection retains explicit path"
            );
            button(&ctx, &mut app, "Close");
            button(&ctx, &mut app, "Find command…");
            text(
                &ctx,
                &mut app,
                "Command name or ID",
                "example.quantities.review",
            );
            button(&ctx, &mut app, "Review candidate with example plugin");
            let second = app.plugins_ui.original_request.clone();
            assert_ne!(first, second);
            assert_eq!(
                app.plugins_ui.export_path,
                format!("{}.{}.job.json", app.path, &second[..12])
            );
            let until = Instant::now() + Duration::from_secs(3);
            loop {
                layers_frame_output(&ctx, &mut app, vec![]);
                if app
                    .plugins_ui
                    .record
                    .as_ref()
                    .is_some_and(|b| b.result.is_some())
                {
                    break;
                }
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(
                app.plugins_ui
                    .record
                    .as_ref()
                    .unwrap()
                    .intent
                    .identity
                    .request_id,
                second
            );
            assert_eq!(app.host.document.editor.project, before);
        }
    }
}
