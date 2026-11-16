mod quantified_check_product_path {
    use super::*;
    #[test]
    fn normal_cited_check_without_original_connection_never_reports_current_pass() {
        cited_product_path(false, UiLanguage::English);
    }
    #[test]
    fn normal_cited_check_rechecks_changed_original_before_confirmation_and_keeps_request_identity()
    {
        cited_product_path(true, UiLanguage::English);
    }
    #[test]
    fn normal_cited_source_snapshot_and_stale_definition_are_visible_in_chinese() {
        cited_product_path(true, UiLanguage::SimplifiedChinese);
    }
    fn cited_product_path(connected: bool, language: UiLanguage) {
        use crate::{
            application_host::{ApplicationHost, HostCommand},
            command::ModelCommand,
            constraints::{Basis, State},
            project_repository::Repository,
            rules::*,
        };
        let root = std::env::var_os("OPENARCHI_CITED_REPRO_OUTPUT")
            .map(|p| {
                PathBuf::from(p).join(if !connected {
                    "unavailable"
                } else if language == UiLanguage::English {
                    "connected"
                } else {
                    "connected-zh"
                })
            })
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!("oa-cited-{}", storage::new_document_identity()))
            });
        std::fs::create_dir_all(root.parent().unwrap()).unwrap();
        std::fs::create_dir(&root)
            .expect("preserve the original outcome; use a new protected fixture");
        let supplied = std::env::var_os("OPENARCHI_CITED_INPUT").map(PathBuf::from);
        let supplied_bytes = supplied.as_ref().map(|p| std::fs::read(p).unwrap());
        let mut author = ApplicationHost::new(storage::startup_example());
        if let Some(path) = &supplied {
            author.document.editor = Editor::from_stored_document(path).unwrap();
        }
        let original_geometry = author.document.editor.project.elements.clone();
        let mut repository =
            Repository::create_for_editor(&root.join("repository"), &author.document.editor)
                .unwrap();
        let original = root.join("developer-fixture.txt");
        std::fs::write(
            &original,
            "Controlled test parameter-height interpretation100m; not a regulation or approval.\n",
        )
        .unwrap();
        let sha = repository.add_source(&original).unwrap();
        // Explicit synthetic fixture preparation uses the same original-bound
        // Host admission as production; the actual task starts with normal Open.
        let mut context = author.document.editor.project.rules.context.clone();
        context.jurisdiction = "Developer test context only".into();
        context.uses = "Synthetic regression, no actual approved use".into();
        context.assessment_date = "2026-11-12".into();
        context.stage = "Controlled acceptance, not a legal assessment".into();
        author
            .dispatch(
                author.context(),
                HostCommand::Apply {
                    request_id: storage::new_document_identity(),
                    commands: vec![ModelCommand::Rules {
                        edit: RuleEdit::SetContext {
                            expected: author.document.editor.project.rules.context.clone(),
                            context,
                        },
                    }],
                },
            )
            .unwrap();
        let edit = RuleEdit::AddSource {
            source: RuleSource {
                id: "developer-cited-fixture".into(),
                title: "Controlled test original".into(),
                code: "DEVELOPER-TEST-NOT-CODE".into(),
                edition: "Synthetic1".into(),
                publisher: "Regression fixture".into(),
                jurisdiction: "Developer test context only".into(),
                published_on: "2020-01-01".into(),
                effective_on: "2020-01-01".into(),
                original_sha256: sha,
                origin_url: String::new(),
                origin: Origin::Unverified,
                verification_note: "Generated test text, not a regulation or approval".into(),
                replaces: vec![],
                same_edition_as: None,
            },
        };
        let guard = repository
            .guard_rule_edit(&author.document.editor.project.rules, &edit)
            .unwrap();
        author
            .apply_rule_edit(
                author.context(),
                &storage::new_document_identity(),
                edit,
                guard,
            )
            .unwrap();
        let edit = RuleEdit::PutRequirement { expected: None, requirement: Requirement {
            id: "developer-cited-height".into(), title: "Controlled citation — test only".into(),
            topic: Topic::AreaMassing,
            citation: Citation { source_id: "developer-cited-fixture".into(), locator: "text/line[1]".into(), text: "Controlled test parameter-height interpretation100m; not a regulation or approval.".into() },
            interpretation: "Synthetic numerical interpretation only".into(),
            decision: Decision { applicability: Applicability::Applicable, adoption: Adoption::Adopted,
                rationale: "Only this explicitly controlled software fixture, never actual legal approval".into(),
                missing: String::new(), basis_sha256: author.document.editor.project.rules.basis("developer-cited-fixture").unwrap(), },
            links: vec![],
        }};
        let guard = repository
            .guard_rule_edit(&author.document.editor.project.rules, &edit)
            .unwrap();
        author
            .apply_rule_edit(
                author.context(),
                &storage::new_document_identity(),
                edit,
                guard,
            )
            .unwrap();
        let file = root.join("controlled-start.openarchi.json");
        storage::save_document(
            &author.document.editor.project,
            &file,
            &author.document.editor.project_identity(),
        )
        .unwrap();
        let (ctx, mut app) = setup();
        app.language = language;
        ctx.global_style_mut(|s| s.animation_time = 0.);
        app.host
            .configure_operation_journal(Some(root.join("native-journal")))
            .unwrap();
        open_test_project(&ctx, &mut app, &file);
        assert!(
            app.source_ui.repository.is_none(),
            "no implicit original access grant"
        );
        if connected {
            press(&ctx, &mut app, "Project conditions");
            press(&ctx, &mut app, "Stored originals and source files");
            settle(&ctx, &mut app);
            let frame = layers_frame_output(&ctx, &mut app, vec![]);
            let at = text_center(&frame.shapes, app.language.translate("Project folder")).unwrap()
                + vec2(170., 0.);
            layers_click(&ctx, &mut app, at);
            layers_frame_output(
                &ctx,
                &mut app,
                vec![
                    key(Key::A, Modifiers::COMMAND),
                    Event::Text(repository.root().display().to_string()),
                ],
            );
            layers_frame_output(&ctx, &mut app, vec![key(Key::Tab, Modifiers::NONE)]);
            press(&ctx, &mut app, "Open project folder");
            finish_check_source(&ctx, &mut app);
            assert!(
                app.source_ui.repository.is_some(),
                "{}",
                app.source_ui.error
            );
            press(&ctx, &mut app, "Continue designing");
        }
        let before = app.host.document.editor.project.clone();
        press(&ctx, &mut app, "Check current design");
        press(
            &ctx,
            &mut app,
            "Add check for current selection or candidate",
        );
        input(
            &ctx,
            &mut app,
            "Check name",
            "Controlled cited numerical check",
        );
        input(&ctx, &mut app, "Limit (blank means unknown)", "100");
        press(&ctx, &mut app, "Design assumption");
        select_citation(&ctx, &mut app, "Controlled citation — test only");
        press(&ctx, &mut app, "Preview quantified check");
        if connected {
            finish_check_source(&ctx, &mut app);
            assert!(
                app.process_ui.preview.is_some(),
                "{} / {}",
                app.checks_ui.error,
                app.source_ui.error
            );
            assert!(
                app.process_ui
                    .preview
                    .as_ref()
                    .unwrap()
                    .change
                    .validate_cited_admission(&app.host.document.editor)
                    .is_err(),
                "displayed previews must release their source lock"
            );
            press(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, before);
            press(&ctx, &mut app, "Preview quantified check");
            finish_check_source(&ctx, &mut app);
        } else {
            assert!(
                app.process_ui.preview.is_none(),
                "unconnected original must not run"
            );
            assert!(app.checks_ui.error.contains("Cited originals unavailable"));
            assert_eq!(app.host.document.editor.project, before);
            press(&ctx, &mut app, "Preview definition without running");
        }
        assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
        settle(&ctx, &mut app);
        let frame = layers_frame_output(&ctx, &mut app, vec![]);
        let mut labels = vec![];
        collect_text(&frame.shapes, &mut labels);
        assert!(
            labels.iter().any(|s| s
                == app.language.translate(
                    "Stored numerical values do not verify current original availability."
                )),
            "actual preview labels: {labels:?}"
        );
        if connected {
            let request = app.process_ui.preview.as_ref().unwrap().id.clone();
            let blob = repository
                .root()
                .join(&repository.manifest.sources[0].blob.path);
            let bytes = std::fs::read(&blob).unwrap();
            std::fs::write(&blob, b"Controlled corruption after preview").unwrap();
            press(&ctx, &mut app, "Confirm design change");
            finish_check_source(&ctx, &mut app);
            assert_eq!(app.host.document.editor.project, before);
            let retained = app
                .process_ui
                .preview
                .as_ref()
                .expect("known pre-submission failure retains original preview");
            assert_eq!(retained.id, request);
            assert!(retained.sent_target.is_none(), "no operation was submitted");
            assert!(
                app.checks_ui
                    .error
                    .starts_with("Repository resource changed: sources/")
            );
            let rejected = layers_frame_output(&ctx, &mut app, vec![]);
            assert!(
                rejected.shapes.iter().any(|s| text_center(
                    std::slice::from_ref(s),
                    app.language.translate("Stored original bytes changed")
                )
                .is_some_and(|p| s.clip_rect.contains(p))),
                "normal confirmation refusal must expose a localized source error"
            );
            let mut messages = vec![];
            collect_text(&rejected.shapes, &mut messages);
            assert!(
                !messages
                    .iter()
                    .any(|s| s.starts_with("Repository resource changed: sources/")),
                "raw diagnostics belong to error state/hover, not the main refusal label"
            );
            std::fs::write(&blob, bytes).unwrap();
            press(&ctx, &mut app, "Confirm design change");
            finish_check_source(&ctx, &mut app);
            assert!(
                app.process_ui.preview.is_none(),
                "{} / {}",
                app.process_ui.error,
                app.checks_ui.error
            );
            assert_eq!(
                app.host
                    .document
                    .editor
                    .change_receipt(&request)
                    .unwrap()
                    .request_id,
                request
            );
        } else {
            press(&ctx, &mut app, "Confirm design change");
        }
        assert_eq!(app.host.document.editor.project.elements, original_geometry);
        assert_eq!(
            app.host.document.editor.project.design_process,
            before.design_process
        );
        assert_eq!(
            app.host.document.editor.project.job_archive,
            before.job_archive
        );
        assert_eq!(app.host.document.editor.project.drawings, before.drawings);
        assert_eq!(
            app.host.document.editor.project.rules.checks.len(),
            before.rules.checks.len() + 1
        );
        if connected {
            finish_check_source(&ctx, &mut app);
            let output = layers_frame_output(&ctx, &mut app, vec![]);
            let visible = |label: &str| {
                output.shapes.iter().any(|s| {
                    text_center(std::slice::from_ref(s), app.language.translate(label))
                        .is_some_and(|p| s.clip_rect.contains(p))
                })
            };
            assert!(
                visible("Exact originals verified at this read"),
                "the source result must be visible in the normal check window"
            );
            let initial = app.host.document.editor.project.clone();
            let blob = repository
                .root()
                .join(&repository.manifest.sources[0].blob.path);
            let bytes = std::fs::read(&blob).unwrap();
            // Previously verified source snapshots must become unknown as soon
            // as a later confirmation actually observes changed source bytes.
            press(&ctx, &mut app, "Preview check update");
            finish_check_source(&ctx, &mut app);
            let preview_id = app.process_ui.preview.as_ref().unwrap().id.clone();
            std::fs::write(&blob, b"Changed after a healthy visible snapshot").unwrap();
            press(&ctx, &mut app, "Confirm design change");
            finish_check_source(&ctx, &mut app);
            assert_eq!(app.host.document.editor.project, initial);
            let retained = app.process_ui.preview.as_ref().unwrap();
            assert_eq!(retained.id, preview_id);
            assert!(retained.sent_target.is_none());
            let rejected = layers_frame_output(&ctx, &mut app, vec![]);
            let mut labels = vec![];
            collect_text(&rejected.shapes, &mut labels);
            assert!(
                !labels.iter().any(|s| s
                    == app
                        .language
                        .translate("Exact originals verified at this read")),
                "a known failed confirmation must discard the earlier verified read"
            );
            press(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, initial);
            std::fs::write(&blob, &bytes).unwrap();
            press(&ctx, &mut app, "Refresh cited originals");
            finish_check_source(&ctx, &mut app);
            std::fs::write(&blob, b"Controlled corruption before source refresh").unwrap();
            press(&ctx, &mut app, "Refresh cited originals");
            finish_check_source(&ctx, &mut app);
            assert_eq!(
                app.host.document.editor.project, initial,
                "readonly verification must not change model/history"
            );
            let output = layers_frame_output(&ctx, &mut app, vec![]);
            assert!(
                output.shapes.iter().any(|s| text_center(
                    std::slice::from_ref(s),
                    app.language.translate("Original verification unavailable")
                )
                .is_some_and(|p| s.clip_rect.contains(p))),
                "missing original state must be visible; old values remain"
            );
            std::fs::write(&blob, &bytes).unwrap();
            press(&ctx, &mut app, "Refresh cited originals");
            finish_check_source(&ctx, &mut app);
            assert_eq!(app.host.document.editor.project, initial);
            let restored = layers_frame_output(&ctx, &mut app, vec![]);
            assert!(
                restored.shapes.iter().any(|s| text_center(
                    std::slice::from_ref(s),
                    app.language
                        .translate("Exact originals verified at this read")
                )
                .is_some_and(|p| s.clip_rect.contains(p))),
                "restoring the original must produce a visible verified snapshot"
            );
            let history = app.host.document.editor.undo.len();
            let check_id = initial.rules.checks.last().unwrap().definition.id.clone();
            press(&ctx, &mut app, "Edit check");
            input(
                &ctx,
                &mut app,
                "Check name",
                "Changed controlled check title",
            );
            press(&ctx, &mut app, "Preview definition without running");
            let proposed = &app
                .process_ui
                .preview
                .as_ref()
                .unwrap()
                .change
                .project()
                .rules
                .checks;
            assert_eq!(
                proposed.last().unwrap().definition.id,
                check_id,
                "edit the visible updated check, not an earlier row"
            );
            assert_eq!(
                proposed.last().unwrap().definition.title,
                "Changed controlled check title"
            );
            assert_eq!(
                proposed[..proposed.len() - 1],
                initial.rules.checks[..initial.rules.checks.len() - 1]
            );
            press(&ctx, &mut app, "Confirm design change");
            finish_check_source(&ctx, &mut app);
            let changed = app
                .host
                .document
                .editor
                .project
                .rules
                .checks
                .last()
                .unwrap();
            assert_eq!(changed.report, initial.rules.checks.last().unwrap().report);
            assert_eq!(
                changed.freshness(&app.host.document.editor.project),
                "Check result is out of date"
            );
            assert_eq!(
                changed.numerical_state(&app.host.document.editor.project),
                State::Unknown
            );
            let frame = layers_frame_output(&ctx, &mut app, vec![]);
            let expected = format!(
                "{}: {}",
                app.language.translate("Read result"),
                app.language.translate(State::Unknown.label())
            );
            assert!(
                frame
                    .shapes
                    .iter()
                    .any(|s| text_center(std::slice::from_ref(s), &expected)
                        .is_some_and(|p| s.clip_rect.contains(p))),
                "stale numerical result must be visibly unknown"
            );
            press(&ctx, &mut app, "Preview check update");
            finish_check_source(&ctx, &mut app);
            press(&ctx, &mut app, "Confirm design change");
            finish_check_source(&ctx, &mut app);
            let refreshed = app
                .host
                .document
                .editor
                .project
                .rules
                .checks
                .last()
                .unwrap();
            assert_eq!(
                refreshed.report.as_ref().unwrap().generation,
                initial
                    .rules
                    .checks
                    .last()
                    .unwrap()
                    .report
                    .as_ref()
                    .unwrap()
                    .generation
                    + 1
            );
            assert_eq!(
                refreshed.numerical_state(&app.host.document.editor.project),
                State::Pass
            );
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            finish_check_source(&ctx, &mut app);
            assert_eq!(
                app.host.document.editor.project, initial,
                "discard the bounded stale-definition probe; retain the valid first result"
            );
            assert_eq!(app.host.document.editor.undo.len(), history);
        }
        let p = &app.host.document.editor.project;
        let check = p.rules.checks.last().unwrap();
        assert!(matches!(
            check.definition.basis,
            Basis::CitedRequirement { .. }
        ));
        if connected {
            assert_eq!(check.numerical_state(p), State::Pass);
            assert!(check.report.is_some());
        } else {
            assert!(
                check.report.is_none(),
                "saving a definition must not fabricate an executed report"
            );
        }

        let observation = serde_json::json!({"entry":"normal Open -> Check current design -> cited requirement -> preview -> confirm", "current_state":check.state(p), "source_repository_attached":connected, "report":check.report, "model130_unchanged":p.elements==original_geometry,"synthetic_conditions_only":true,"native_package":"not_run","real_AI":"not_run"});
        std::fs::write(
            root.join("observed.json"),
            serde_json::to_vec_pretty(&observation).unwrap(),
        )
        .unwrap();
        eprintln!("CITED_CHECK_REPRO={}", root.display());
        if let Some(path) = &supplied {
            assert_eq!(std::fs::read(path).unwrap(), supplied_bytes.unwrap());
        }
        assert_eq!(
            check.state(p),
            State::Unknown,
            "A stored report alone cannot certify current original availability"
        );
        let after = app.host.document.editor.project.clone();
        layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
        assert_eq!(app.host.document.editor.project, before);
        layers_frame_output(
            &ctx,
            &mut app,
            vec![key(Key::Z, Modifiers::COMMAND | Modifiers::ALT)],
        );
        assert_eq!(app.host.document.editor.project, after);
        layers_frame_output(&ctx, &mut app, vec![key(Key::S, Modifiers::COMMAND)]);
        assert!(app.file_action.is_none(), "{}", app.file_error);
        settle(&ctx, &mut app);
        assert!(
            app.file_action.is_none(),
            "normal Save confirmation must settle"
        );
        let reopened = Editor::from_stored_document(&file).unwrap();
        assert_eq!(reopened.project.rules, after.rules);
        assert_eq!(reopened.project.elements, after.elements);
        assert_eq!(reopened.project.design_process, after.design_process);
        std::fs::write(root.join("saved-readback.json"),serde_json::to_vec_pretty(&serde_json::json!({"path":file,"checks":reopened.project.rules.checks.len(),"original_geometry_preserved":true,"undo_redo":true,"saved_reopened":true,"separate_os_process":"not_run"})).unwrap()).unwrap();
        if connected {
            let history = app.host.document.editor.undo.len();
            let source = repository.manifest.sources[0].id.clone();
            repository
                .reject_source(
                    &source,
                    "Controlled source rejection after a visible healthy read",
                )
                .unwrap();
            press(&ctx, &mut app, "Refresh cited originals");
            finish_check_source(&ctx, &mut app);
            assert_eq!(app.host.document.editor.project, after);
            assert_eq!(app.host.document.editor.undo.len(), history);
            let frame = layers_frame_output(&ctx, &mut app, vec![]);
            let label = format!(
                "{}: {}",
                app.language.translate("Read result"),
                app.language.translate(State::Unknown.label())
            );
            assert!(
                frame
                    .shapes
                    .iter()
                    .any(|s| text_center(std::slice::from_ref(s), &label)
                        .is_some_and(|p| s.clip_rect.contains(p))),
                "source quality/version change must not retain a visible pass"
            );
            std::fs::write(root.join("source-rejection-readback.json"),serde_json::to_vec_pretty(&serde_json::json!({"normal_entry":"Refresh cited originals","unknown_visible":true,"old_report_preserved":true,"model_and_undo_unchanged":true,"language":if language==UiLanguage::English {"en"}else{"zh-CN"},"rejected_source_is_a_developer_fixture":true})).unwrap()).unwrap();
        }
    }
    fn finish_check_source(ctx: &Context, app: &mut AtelierApp) {
        let start = std::time::Instant::now();
        loop {
            layers_frame_output(ctx, app, vec![]);
            if !app.source_ui.pending_for_ui() {
                settle(ctx, app);
                if !app.source_ui.pending_for_ui() {
                    break;
                }
            }
            assert!(start.elapsed() < std::time::Duration::from_secs(15));
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    fn settle(ctx: &Context, app: &mut AtelierApp) {
        for _ in 0..12 {
            layers_frame_output(ctx, app, vec![]);
        }
    }
    fn select_citation(ctx: &Context, app: &mut AtelierApp, label: &str) {
        // Use the actual dropdown's visible rows and wheel events; never remove
        // earlier project clauses or assign the draft's basis to reach a test row.
        for _ in 0..24 {
            settle(ctx, app);
            let frame = layers_frame_output(ctx, app, vec![]);
            let visible = |text: &str| {
                frame.shapes.iter().find_map(|s| {
                    text_center(std::slice::from_ref(s), text).filter(|p| s.clip_rect.contains(*p))
                })
            };
            if visible(label).is_some() {
                press(ctx, app, label);
                return;
            }
            let at = app
                .host
                .document
                .editor
                .project
                .rules
                .requirements
                .iter()
                .find_map(|r| visible(&r.title))
                .expect("actual cited requirement dropdown row");
            layers_frame_output(
                ctx,
                app,
                vec![
                    Event::PointerMoved(at),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        delta: vec2(0., -160.),
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        panic!("cited requirement remains outside actual dropdown; inspect without commit")
    }
    fn press(ctx: &Context, app: &mut AtelierApp, label: &str) {
        reveal_manual_control(ctx, app, label);
        settle(ctx, app);
        let frame = layers_frame_output(ctx, app, vec![]);
        let mut positions = vec![];
        for s in &frame.shapes {
            if let Some(at) = text_center(std::slice::from_ref(s), app.language.translate(label)) {
                if s.clip_rect.contains(at) {
                    positions.push(at);
                }
            }
        }
        let at = *positions.last().unwrap_or_else(|| {
            let mut text = vec![];
            collect_text(&frame.shapes, &mut text);
            panic!("{label} / {} / {text:?}", app.process_ui.error)
        });
        layers_click(ctx, app, at);
    }
    fn input(ctx: &Context, app: &mut AtelierApp, key_name: &str, value: &str) {
        settle(ctx, app);
        let rect = ctx
            .data(|d| d.get_temp::<Rect>(Id::new(("check_field", key_name))))
            .expect("actual labelled check field");
        assert!(ctx.content_rect().contains(rect.center()));
        layers_click(ctx, app, rect.center());
        layers_frame_output(
            ctx,
            app,
            vec![key(Key::A, Modifiers::COMMAND), Event::Text(value.into())],
        );
        layers_frame_output(ctx, app, vec![key(Key::Tab, Modifiers::NONE)]);
    }
    #[test]
    fn check_draft_blocks_remote_tool_context_and_refuses_user_revision_without_renewal() {
        let (ctx, mut app) = setup();
        ctx.global_style_mut(|s| s.animation_time = 0.);
        press(&ctx, &mut app, "Check current design");
        press(
            &ctx,
            &mut app,
            "Add check for current selection or candidate",
        );
        input(&ctx, &mut app, "Limit (blank means unknown)", "4");
        let before = app.host.document.editor.project.clone();
        #[cfg(unix)]
        {
            let directory = std::path::PathBuf::from("/tmp").join(format!(
                "oa-check-draft-{}",
                &storage::new_document_identity()[..12]
            ));
            std::fs::create_dir(&directory).unwrap();
            let mut endpoint = crate::automation_ipc::Endpoint::bind_for_host(
                directory.join("ipc/app.sock"),
                true,
                Some(directory.join("journal")),
                &mut app.host,
            )
            .unwrap();
            endpoint.set_write_enabled(true, &app.host.document.editor);
            let params=serde_json::from_value(serde_json::json!({"target":app.host.operation_target(),"expected_revision":app.host.context().revision,"request_id":"check-draft-must-not-renew","command_id":"tool.zone","parameters":{}})).unwrap();
            assert_eq!(
                app.execute_remote_workbench(&endpoint, &params)["error"]["code"],
                "USER_INTERACTION_ACTIVE"
            );
            drop(endpoint);
            std::fs::remove_dir_all(directory).unwrap();
        }
        assert_eq!(app.host.document.editor.project, before);
        // The normal shortcut is refused before a save dialog while the user
        // owns unconfirmed check parameters; no file or model state changes.
        layers_frame_output(&ctx, &mut app, vec![key(Key::S, Modifiers::COMMAND)]);
        settle(&ctx, &mut app);
        assert!(app.file_action.is_none());
        assert!(app.checks_ui.draft.is_some());
        assert_eq!(app.host.document.editor.project, before);
        assert!(app.block_report_draft_close(&ctx));
        assert!(app.checks_ui.draft.is_some());
        let mut site = before.site.clone();
        site.control_tolerance_m = Some(0.03);
        app.host
            .dispatch(
                app.host.context(),
                crate::application_host::HostCommand::Apply {
                    request_id: "user-insertion-during-check-draft".into(),
                    commands: vec![crate::command::ModelCommand::SetSiteContext { context: site }],
                },
            )
            .unwrap();
        let user = app.host.document.editor.project.clone();
        press(&ctx, &mut app, "Preview quantified check");
        assert!(app.process_ui.preview.is_none());
        assert_eq!(app.host.document.editor.project, user);
        assert!(app.pending_design_parameters());
        press(&ctx, &mut app, "Cancel check draft");
        assert!(!app.pending_design_parameters());
        assert_eq!(app.host.document.editor.project, user);
    }
    #[test]
    fn normal_project_open_quantified_check_cancel_confirm_locate_edit_stale_update_undo_save_bilingual()
     {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            let root = std::env::temp_dir()
                .join(format!("oa-check-ui-{}", storage::new_document_identity()));
            std::fs::create_dir(&root).unwrap();
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            let mut p = Project::default();
            let id = p.add(
                0,
                Material::Plaster,
                Shape::ConceptMass {
                    spec: crate::concept_mass::ConceptMassSpec {
                        arcs: vec![],
                        hole_arcs: vec![],
                        outline: vec![
                            Point::new(0., 0.),
                            Point::new(5., 0.),
                            Point::new(5., 4.),
                            Point::new(0., 4.),
                        ],
                        holes: vec![],
                        height: 5.,
                        bottom_offset: 0.,
                    },
                },
            );
            let input_path = root.join("starting.openarchi.json");
            storage::save(&p, &input_path).unwrap();
            open_test_project(&ctx, &mut app, &input_path);
            let baseline = app.host.document.editor.project.clone();
            press(&ctx, &mut app, "Check current design");
            press(
                &ctx,
                &mut app,
                "Add check for current selection or candidate",
            );
            assert!(app.pending_design_parameters());
            input(&ctx, &mut app, "Limit (blank means unknown)", "4");
            press(&ctx, &mut app, "Preview quantified check");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            assert_eq!(app.host.document.editor.project, baseline);
            press(&ctx, &mut app, "Cancel design proposal");
            assert!(app.checks_ui.draft.is_some());
            assert_eq!(app.host.document.editor.project, baseline);
            press(&ctx, &mut app, "Preview quantified check");
            press(&ctx, &mut app, "Confirm design change");
            assert!(
                app.checks_ui.draft.is_none(),
                "{} / {}",
                app.process_ui.error,
                app.host.document.editor.status
            );
            let c = &app.host.document.editor.project.rules.checks[0];
            assert_eq!(
                c.state(&app.host.document.editor.project),
                crate::constraints::State::Fail
            );
            assert_eq!(c.report.as_ref().unwrap().rows[0].value, Some(5.));
            assert_eq!(app.host.document.editor.project.elements, baseline.elements);
            // A metric change cannot silently reuse a height limit as m².
            press(&ctx, &mut app, "Edit check");
            settle(&ctx, &mut app);
            let rect = ctx
                .data(|d| d.get_temp::<Rect>(Id::new("check_metric_control")))
                .unwrap();
            layers_click(&ctx, &mut app, rect.center());
            press(
                &ctx,
                &mut app,
                "Authored footprint area sum (overlaps counted)",
            );
            press(&ctx, &mut app, "Preview quantified check");
            let proposed = app
                .process_ui
                .preview
                .as_ref()
                .expect("changed metric unknown-limit preview")
                .change
                .project();
            assert_eq!(proposed.rules.checks[0].definition.limit, None);
            assert_eq!(
                proposed.rules.checks[0].state(proposed),
                crate::constraints::State::Unknown
            );
            assert_eq!(
                proposed.rules.checks[0].report.as_ref().unwrap().rows[0].value,
                Some(20.)
            );
            press(&ctx, &mut app, "Cancel design proposal");
            press(&ctx, &mut app, "Cancel check draft");
            assert_eq!(
                app.host.document.editor.project.rules.checks[0]
                    .definition
                    .metric,
                crate::constraints::Metric::Height
            );
            // A real result control locates the actual shared model object.
            let name = app
                .host
                .document
                .editor
                .project
                .elements
                .iter()
                .find(|e| e.id == id)
                .unwrap()
                .name
                .clone();
            press(&ctx, &mut app, &format!("#{id} · {name}"));
            assert_eq!(app.host.document.editor.selected, Some(id));
            // Move the floating check window aside by its normal title bar before
            // using the existing inspector, rather than assigning model state.
            let area = ctx
                .memory(|m| m.area_rect(Id::new("quantified_checks")))
                .unwrap();
            let from = pos2(area.center().x, area.min.y + 12.);
            let to = pos2(380., 220.);
            layers_frame_output(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(from),
                    Event::PointerButton {
                        pos: from,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
            layers_frame_output(&ctx, &mut app, vec![Event::PointerMoved(to)]);
            layers_frame_output(
                &ctx,
                &mut app,
                vec![Event::PointerButton {
                    pos: to,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                }],
            );
            linked_figure_product_path::height(&ctx, &mut app, 3.5);
            assert_eq!(
                app.host.document.editor.project.rules.checks[0]
                    .state(&app.host.document.editor.project),
                crate::constraints::State::Unknown
            );
            press(&ctx, &mut app, "Preview check update");
            press(&ctx, &mut app, "Confirm design change");
            assert_eq!(
                app.host.document.editor.project.rules.checks[0]
                    .state(&app.host.document.editor.project),
                crate::constraints::State::Pass
            );
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(
                app.host.document.editor.project.rules.checks[0]
                    .state(&app.host.document.editor.project),
                crate::constraints::State::Unknown
            );
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::ALT)],
            );
            assert_eq!(
                app.host.document.editor.project.rules.checks[0]
                    .state(&app.host.document.editor.project),
                crate::constraints::State::Pass
            );
            layers_frame_output(&ctx, &mut app, vec![key(Key::S, Modifiers::COMMAND)]);
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let reopened = Editor::from_stored_document(&input_path).unwrap();
            assert_eq!(
                reopened.project.rules.checks,
                app.host.document.editor.project.rules.checks
            );
            assert_eq!(
                reopened.project.elements,
                app.host.document.editor.project.elements
            );
            assert_eq!(
                reopened.project.rules.checks[0].state(&reopened.project),
                crate::constraints::State::Pass
            );
            drop(app);
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn normal_reference_edge_check_preview_cancel_confirm_undo_save_bilingual() {
        use crate::{
            constraints::{Metric, State},
            site::{
                LengthUnit,
                boundary::{BoundaryOrigin, Placement, SiteBoundary},
            },
        };
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            let persistent = std::env::var_os("OPENARCHI_EDGE_REPRO_OUTPUT");
            let root = persistent
                .as_ref()
                .map(|p| {
                    PathBuf::from(p).join(if language == UiLanguage::English {
                        "en"
                    } else {
                        "zh"
                    })
                })
                .unwrap_or_else(|| {
                    std::env::temp_dir()
                        .join(format!("oa-edge-ui-{}", storage::new_document_identity()))
                });
            std::fs::create_dir_all(&root).unwrap();
            assert!(
                !root.join("starting.openarchi.json").exists(),
                "preserve existing native fixture"
            );
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            let mut p = Project::default();
            p.add(
                0,
                Material::Plaster,
                Shape::ConceptMass {
                    spec: crate::concept_mass::ConceptMassSpec {
                        arcs: vec![],
                        hole_arcs: vec![],
                        outline: vec![
                            Point::new(0., 0.),
                            Point::new(5., 0.),
                            Point::new(5., 4.),
                            Point::new(0., 4.),
                        ],
                        holes: vec![],
                        height: 3.,
                        bottom_offset: 0.,
                    },
                },
            );
            let ring = vec![
                Point::new(-10., -10.),
                Point::new(30., -10.),
                Point::new(30., 30.),
                Point::new(-10., 30.),
            ];
            let mut closed = ring.clone();
            closed.push(closed[0]);
            let id = p.add(
                0,
                Material::Concrete,
                Shape::Polyline {
                    points: closed,
                    arcs: vec![],
                },
            );
            p.site.boundary = Some(SiteBoundary {
                name: "Declared fixture, no approval".into(),
                element_id: id,
                imported_outline_m: ring.iter().map(|q| [q.x, q.y]).collect(),
                origin: BoundaryOrigin {
                    source_sha256: "a".repeat(64),
                    entity_handle: "1A".into(),
                    record_sha256: "b".repeat(64),
                    unit: LengthUnit::Metre,
                    placement: Placement::ModelAxes,
                    centerline_width_native: None,
                },
            });
            let input_path = root.join("starting.openarchi.json");
            storage::save(&p, &input_path).unwrap();
            if persistent.is_some() {
                storage::save(&p, &root.join("baseline.openarchi.json")).unwrap();
            }
            open_test_project(&ctx, &mut app, &input_path);
            let baseline = app.host.document.editor.project.clone();
            press(&ctx, &mut app, "Check current design");
            press(
                &ctx,
                &mut app,
                "Add check for current selection or candidate",
            );
            settle(&ctx, &mut app);
            let metric = ctx
                .data(|d| d.get_temp::<Rect>(Id::new("check_metric_control")))
                .unwrap();
            layers_click(&ctx, &mut app, metric.center());
            press(&ctx, &mut app, "Signed clearance to one reference edge");
            input(&ctx, &mut app, "Limit (blank means unknown)", "8");
            // Choosing another geometric edge must not reuse a limit from edge 1.
            settle(&ctx, &mut app);
            let edge = ctx
                .data(|d| d.get_temp::<Rect>(Id::new("check_edge_control")))
                .unwrap();
            layers_click(&ctx, &mut app, edge.center());
            let label = format!(
                "{} 2: (30.000000, -10.000000) → (30.000000, 30.000000) m",
                language.translate("Reference edge")
            );
            press(&ctx, &mut app, &label);
            press(&ctx, &mut app, "Preview quantified check");
            let proposed = app
                .process_ui
                .preview
                .as_ref()
                .expect("edge unknown preview")
                .change
                .project();
            assert_eq!(proposed.rules.checks[0].definition.limit, None);
            assert_eq!(proposed.rules.checks[0].state(proposed), State::Unknown);
            press(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, baseline);
            input(&ctx, &mut app, "Limit (blank means unknown)", "26");
            press(&ctx, &mut app, "Preview quantified check");
            press(&ctx, &mut app, "Confirm design change");
            let after = app.host.document.editor.project.clone();
            let c = &after.rules.checks[0];
            assert_eq!(
                c.definition.metric,
                Metric::ReferenceBoundaryEdgeClearance { edge: 1 }
            );
            assert_eq!(c.report.as_ref().unwrap().rows[0].value, Some(25.));
            assert_eq!(c.state(&after), State::Fail);
            assert_eq!(after.elements, baseline.elements);
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(app.host.document.editor.project, baseline);
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::ALT)],
            );
            assert_eq!(app.host.document.editor.project, after);
            layers_frame_output(&ctx, &mut app, vec![key(Key::S, Modifiers::COMMAND)]);
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let reopened = Editor::from_stored_document(&input_path).unwrap();
            assert_eq!(reopened.project, after);
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&std::fs::read(&input_path).unwrap())
                    .unwrap()["schema_version"],
                23
            );
            drop(app);
            if persistent.is_none() {
                std::fs::remove_dir_all(root).unwrap();
            }
        }
    }
}
