mod project_conditions_product_path {
    use super::*;
    #[test]
    fn normal_project_menu_opens_one_conditions_window_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            app.language = language;
            let before = app.host.document.editor.project.clone();
            let context = app.host.context();
            let output = layers_frame_output(&ctx, &mut app, vec![]);
            let menu = text_center(&output.shapes, language.translate("Project")).unwrap();
            layers_click(&ctx, &mut app, menu);
            let output = layers_frame_output(&ctx, &mut app, vec![]);
            let mut texts = vec![];
            collect_text(&output.shapes, &mut texts);
            for label in [
                "Project sources",
                "Functional program",
                "Source evidence",
                "Conditions and rules",
            ] {
                assert!(
                    !texts.iter().any(|t| t == language.translate(label)),
                    "duplicate preparation menu: {label}"
                );
            }
            let at = target(&ctx, &mut app, "Project conditions", true);
            layers_click(&ctx, &mut app, at);
            assert!(app.project_conditions_ui.open);
            for group in [
                "Site and orientation",
                "Requirements and constraints",
                "Documents and evidence",
            ] {
                press(&ctx, &mut app, group);
            }
            assert!(!app.project_conditions_has_draft());
            press(&ctx, &mut app, "Continue designing");
            assert!(!app.project_conditions_ui.open);
            assert!(app.site_dialog.is_none());
            assert_eq!(app.host.document.editor.project, before);
            assert_eq!(app.host.context(), context);
            assert!(app.host.document.editor.undo.is_empty());
            press(&ctx, &mut app, "Concept mass");
            assert_eq!(app.host.document.editor.tool, Tool::ConceptMass);
        }
    }
    fn settle(ctx: &Context, app: &mut AtelierApp) {
        for _ in 0..20 {
            layers_frame_output(ctx, app, vec![]);
        }
    }
    fn target(ctx: &Context, app: &mut AtelierApp, text: &str, last: bool) -> Pos2 {
        reveal_manual_control(ctx, app, text);
        settle(ctx, app);
        let out = layers_frame_output(ctx, app, vec![]);
        let bounds = if app.file_action.is_some() {
            ctx.memory(|m| m.area_rect(Id::new("project_file_dialog")))
        } else if app.project_conditions_ui.open {
            ctx.memory(|m| m.area_rect(Id::new("project_conditions")))
        } else {
            None
        };
        let mut found = vec![];
        for s in &out.shapes {
            if let egui::Shape::Text(t) = &s.shape {
                if t.galley.text() == app.language.translate(text) {
                    let mut r = Rect::from_min_size(t.pos, t.galley.size()).intersect(s.clip_rect);
                    if let Some(b) = bounds {
                        r = r.intersect(b);
                    }
                    if r.is_positive() {
                        found.push(r.center());
                    }
                }
            }
        }
        if let Some(p) = if last { found.last() } else { found.first() } {
            return *p;
        }
        let mut texts = vec![];
        collect_text(&out.shapes, &mut texts);
        panic!(
            "{text}: {} / {} / {texts:?}",
            app.project_conditions_ui.error, app.source_ui.error
        );
    }
    pub(super) fn press(ctx: &Context, app: &mut AtelierApp, text: &str) {
        let at = target(ctx, app, text, false);
        layers_click(ctx, app, at);
    }
    fn text(ctx: &Context, app: &mut AtelierApp, old: &str, value: &str) {
        let at = target(ctx, app, old, true);
        layers_click(ctx, app, at);
        layers_frame_output(
            ctx,
            app,
            vec![key(Key::A, Modifiers::COMMAND), Event::Text(value.into())],
        );
        layers_frame_output(ctx, app, vec![key(Key::Tab, Modifiers::NONE)]);
    }
    pub(super) fn tolerance(ctx: &Context, app: &mut AtelierApp) {
        press(ctx, app, "State control tolerance (m)");
        settle(ctx, app);
        let out = layers_frame_output(ctx, app, vec![]);
        let label =
            text_center(&out.shapes, app.language.translate("Control tolerance (m)")).unwrap();
        let bounds = ctx
            .memory(|m| m.area_rect(Id::new("project_conditions")))
            .unwrap();
        let at = out
            .shapes
            .iter()
            .filter_map(|s| {
                let egui::Shape::Text(t) = &s.shape else {
                    return None;
                };
                let r = Rect::from_min_size(t.pos, t.galley.size()).intersect(s.clip_rect);
                (r.is_positive()
                    && bounds.contains(r.center())
                    && (r.center().y - label.y).abs() < 10.
                    && r.center().x > label.x
                    && t.galley.text().starts_with(|c: char| c.is_ascii_digit()))
                .then(|| r.center())
            })
            // The project quantities remain behind this floating window.
            // Select the topmost field, rather than their coincident number.
            .last()
            .unwrap();
        layers_click(ctx, app, at);
        layers_frame_output(
            ctx,
            app,
            vec![key(Key::A, Modifiers::COMMAND), Event::Text("0.02".into())],
        );
        layers_frame_output(ctx, app, vec![key(Key::Tab, Modifiers::NONE)]);
        assert_eq!(
            app.project_conditions_ui
                .site
                .as_ref()
                .unwrap()
                .control_tolerance_m,
            Some(0.02)
        );
    }
    fn setup_input(
        root: &std::path::Path,
        supplied: Option<&std::path::Path>,
    ) -> std::path::PathBuf {
        if let Some(source) = supplied {
            let target = root.join(source.file_name().unwrap());
            std::fs::copy(source, &target).unwrap();
            let p: serde_json::Value =
                serde_json::from_slice(&std::fs::read(source).unwrap()).unwrap();
            for page in p["drawings"]["pages"].as_array().into_iter().flatten() {
                let rel = std::path::PathBuf::from(page["path"].as_str().unwrap());
                assert!(
                    !rel.is_absolute()
                        && !rel
                            .components()
                            .any(|c| matches!(c, std::path::Component::ParentDir))
                );
                let dest = root.join(&rel);
                std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
                std::fs::copy(source.parent().unwrap().join(&rel), dest).unwrap();
            }
            return target;
        }
        let mut p = storage::startup_example();
        p.rules.context.stage = "Existing authored concept stage".into();
        let image = root.join("authored.png");
        image::RgbaImage::from_pixel(40, 30, image::Rgba([220, 205, 170, 255]))
            .save(&image)
            .unwrap();
        p.drawings.pages.push(
            crate::drawings::import(
                &image,
                &root.join("source-assets"),
                p.drawings.next_id().unwrap(),
            )
            .unwrap(),
        );
        let target = root.join("starting.openarchi.json");
        storage::save(&p, &target).unwrap();
        target
    }
    #[test]
    fn normal_conditions_window_carries_existing_unknowns_groups_review_cancel_one_save_and_reopen_bilingual()
     {
        let supplied = std::env::var_os("OPENARCHI_CONDITIONS_INPUT").map(std::path::PathBuf::from);
        let destination =
            std::env::var_os("OPENARCHI_CONDITIONS_OUTPUT").map(std::path::PathBuf::from);
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            let parent = destination.clone().unwrap_or_else(|| {
                std::env::temp_dir().join(format!(
                    "oa-conditions-ui-{}",
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
                .expect("do not overwrite a prior result or replay an unknown write");
            let input = setup_input(&root, supplied.as_deref());
            let source_bytes = supplied.as_ref().map(|p| std::fs::read(p).unwrap());
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            open_test_project(&ctx, &mut app, &input);
            let baseline = app.host.document.editor.project.clone();
            let initial_history = app.host.document.editor.undo.len();
            let expected_context = baseline.rules.context.clone();
            assert!(
                baseline.site.control_tolerance_m.is_none(),
                "finite fixture prerequisite; never reset real conditions"
            );
            // Normal single entry; no assignment of the condition draft or selected group.
            press(&ctx, &mut app, "Project conditions");
            assert!(app.project_conditions_ui.open);
            assert_eq!(app.host.document.editor.project, baseline);
            assert!(!app.project_conditions_has_draft());
            let out = layers_frame_output(&ctx, &mut app, vec![]);
            let mut texts = vec![];
            collect_text(&out.shapes, &mut texts);
            assert!(!texts.iter().any(|text|text==app.language.translate("Sources are copied without changing the model. Save a checkpoint to update the stored model.")),"the collapsed sources catalog must not appear in a second standalone window");
            for page in &baseline.drawings.pages {
                assert!(
                    texts.contains(&page.name),
                    "existing attachment shown without reupload"
                );
            }
            assert!(app.site_dialog.is_none());
            press(&ctx, &mut app, "Site and orientation");
            tolerance(&ctx, &mut app);
            assert_eq!(app.host.document.editor.project, baseline);
            press(&ctx, &mut app, "Requirements and constraints");
            press(&ctx, &mut app, "Edit applicability context");
            assert_eq!(
                app.rules_ui.context_draft.as_ref().unwrap().0,
                expected_context
            );
            settle(&ctx, &mut app);
            let field = ctx
                .data(|data| data.get_temp::<Rect>(Id::new(("rule_context_field", "Design stage"))))
                .unwrap();
            assert!(
                ctx.content_rect().contains(field.center()),
                "actual editable field must be visible"
            );
            layers_click(&ctx, &mut app, field.center());
            layers_frame_output(
                &ctx,
                &mut app,
                vec![
                    key(Key::A, Modifiers::COMMAND),
                    Event::Text("Concept exploration — controlled UI edit".into()),
                ],
            );
            layers_frame_output(&ctx, &mut app, vec![key(Key::Tab, Modifiers::NONE)]);
            assert_eq!(
                app.rules_ui.context_draft.as_ref().unwrap().0.stage,
                "Concept exploration — controlled UI edit"
            );
            assert_eq!(app.host.document.editor.project, baseline);
            press(&ctx, &mut app, "Site and orientation");
            assert_eq!(
                app.project_conditions_ui
                    .site
                    .as_ref()
                    .unwrap()
                    .control_tolerance_m,
                Some(0.02)
            );
            press(&ctx, &mut app, "Preview all condition changes");
            assert!(
                app.project_conditions_ui.preview.is_some(),
                "{} / {}",
                app.project_conditions_ui.error,
                app.source_ui.error
            );
            assert_eq!(app.host.document.editor.project, baseline);
            assert_eq!(app.host.document.editor.undo.len(), initial_history);
            press(&ctx, &mut app, "Return to condition editing");
            assert!(app.project_conditions_ui.preview.is_none());
            assert_eq!(app.host.document.editor.project, baseline);
            assert_eq!(
                app.rules_ui.context_draft.as_ref().unwrap().0.stage,
                "Concept exploration — controlled UI edit"
            );
            press(&ctx, &mut app, "Preview all condition changes");
            let request = app
                .project_conditions_ui
                .preview
                .as_ref()
                .unwrap()
                .request_id
                .clone();
            let original_target = app.host.operation_target();
            press(&ctx, &mut app, "Save project conditions");
            assert!(
                app.project_conditions_ui.error.is_empty(),
                "{}",
                app.project_conditions_ui.error
            );
            assert!(app.project_conditions_ui.preview.is_none());
            assert!(!app.project_conditions_has_draft());
            assert_eq!(app.host.document.editor.undo.len(), initial_history + 1);
            let after = app.host.document.editor.project.clone();
            assert_eq!(after.elements, baseline.elements);
            assert_eq!(after.drawings, baseline.drawings);
            assert_eq!(after.design_process, baseline.design_process);
            assert_eq!(after.site.control_tolerance_m, Some(0.02));
            assert_eq!(after.site.horizontal, baseline.site.horizontal);
            assert_eq!(after.site.true_north, baseline.site.true_north);
            assert_eq!(after.site.vertical, baseline.site.vertical);
            assert_eq!(after.site.controls, baseline.site.controls);
            assert_eq!(
                after.rules.context.stage,
                "Concept exploration — controlled UI edit"
            );
            assert_eq!(
                after.rules.context.jurisdiction,
                expected_context.jurisdiction
            );
            assert_eq!(after.rules.context.uses, expected_context.uses);
            assert_eq!(
                after.rules.context.assessment_date,
                expected_context.assessment_date
            );
            assert_eq!(
                app.host.inspect_operation(original_target, &request)["result"]["status"],
                "committed"
            );
            let stored = Editor::from_stored_document(&input).unwrap();
            assert_eq!(stored.project.elements, after.elements);
            assert_eq!(stored.project.site, after.site);
            assert_eq!(stored.project.rules, after.rules);
            for (a, b) in stored
                .project
                .drawings
                .pages
                .iter()
                .zip(&after.drawings.pages)
            {
                assert_eq!(
                    std::fs::read(&a.path).unwrap(),
                    std::fs::read(&b.path).unwrap()
                );
            }
            press(&ctx, &mut app, "Continue designing");
            assert!(!app.project_conditions_ui.open);
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(app.host.document.editor.project, baseline);
            press(&ctx, &mut app, "Project conditions");
            assert_eq!(
                app.project_conditions_ui
                    .site
                    .as_ref()
                    .unwrap()
                    .control_tolerance_m,
                baseline.site.control_tolerance_m
            );
            assert!(!app.project_conditions_has_draft());
            press(&ctx, &mut app, "Continue designing");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::ALT)],
            );
            assert_eq!(app.host.document.editor.project, after);
            // Site declarations are still unknown; a design tool remains usable.
            press(&ctx, &mut app, "Concept mass");
            assert_eq!(app.host.document.editor.tool, Tool::ConceptMass);
            assert_eq!(app.host.document.editor.project, after);
            if let Some(p) = &supplied {
                assert_eq!(
                    std::fs::read(p).unwrap(),
                    source_bytes.as_ref().unwrap().as_slice()
                );
            }
            std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"input_copy":input,"original":supplied,"model_geometry_changed":false,"site_tolerance_m":0.02,"stage":"Concept exploration — controlled UI edit","other_unknowns_retained":true,"cross_group_review_cancel":true,"one_durable_transaction":request,"same_path_file_save_and_reopen":true,"normal_design_tool_available":true,"native":"not_run","real_AI":"not_run"})).unwrap()).unwrap();
            if destination.is_none() {
                std::fs::remove_dir_all(parent).unwrap();
            }
        }
    }
    #[test]
    fn normal_condition_draft_refuses_user_conflict_and_remote_workbench_without_renewal() {
        let (ctx, mut app) = setup();
        ctx.global_style_mut(|s| s.animation_time = 0.);
        press(&ctx, &mut app, "Project conditions");
        press(&ctx, &mut app, "Site and orientation");
        tolerance(&ctx, &mut app);
        let before = app.host.document.editor.project.clone();
        #[cfg(unix)]
        {
            let directory = std::path::PathBuf::from("/tmp")
                .join(format!("oa-c-{}", &storage::new_document_identity()[..12]));
            std::fs::create_dir_all(&directory).unwrap();
            let mut endpoint = crate::automation_ipc::Endpoint::bind_for_host(
                directory.join("socket/app.sock"),
                true,
                Some(directory.join("journal")),
                &mut app.host,
            )
            .unwrap();
            // Explicitly authorized transport fixture; draft protection must still win.
            endpoint.set_write_enabled(true, &app.host.document.editor);
            let target = app.host.operation_target();
            let params=serde_json::from_value(serde_json::json!({"target":target,"expected_revision":app.host.context().revision,"request_id":"blocked-condition-draft","command_id":"tool.zone","parameters":{}})).unwrap();
            assert_eq!(
                app.execute_remote_workbench(&endpoint, &params)["error"]["code"],
                "USER_INTERACTION_ACTIVE"
            );
            assert_eq!(app.host.document.editor.project, before);
            drop(endpoint);
            std::fs::remove_dir_all(directory).unwrap();
        }
        // Another user edit through the shared host, after this draft was begun.
        let mut user_site = before.site.clone();
        user_site.control_tolerance_m = Some(0.03);
        app.host
            .dispatch(
                app.host.context(),
                crate::application_host::HostCommand::Apply {
                    request_id: "user-condition-insertion".into(),
                    commands: vec![crate::command::ModelCommand::SetSiteContext {
                        context: user_site,
                    }],
                },
            )
            .unwrap();
        let user = app.host.document.editor.project.clone();
        let history = app.host.document.editor.undo.len();
        press(&ctx, &mut app, "Preview all condition changes");
        assert!(app.project_conditions_ui.preview.is_none());
        assert_eq!(
            app.project_conditions_ui.error,
            "Project conditions changed; reload before applying"
        );
        assert_eq!(app.host.document.editor.project, user);
        assert_eq!(app.host.document.editor.undo.len(), history);
        press(&ctx, &mut app, "Discard condition changes");
        assert!(!app.project_conditions_has_draft());
        assert_eq!(
            app.project_conditions_ui
                .site
                .as_ref()
                .unwrap()
                .control_tolerance_m,
            Some(0.03)
        );
        assert_eq!(app.host.document.editor.project, user);
        press(&ctx, &mut app, "Continue designing");
        press(&ctx, &mut app, "Concept mass");
        assert_eq!(app.host.document.editor.tool, Tool::ConceptMass);
    }
    #[test]
    fn normal_continue_design_closes_embedded_preparation_views_without_reopening_legacy_windows() {
        let (ctx, mut app) = setup();
        ctx.global_style_mut(|s| s.animation_time = 0.);
        let before = app.host.document.editor.project.clone();
        press(&ctx, &mut app, "Project conditions");
        press(&ctx, &mut app, "Requirements and constraints");
        press(&ctx, &mut app, "Lifecycle and condition register");
        settle(&ctx, &mut app);
        assert!(app.rules_ui.register.open);
        assert!(app.project_conditions_ui.open);
        // An existing source-report component may also be open inside this root.
        app.source_ui
            .review
            .choose("controlled-component-subject".into());
        press(&ctx, &mut app, "Continue designing");
        settle(&ctx, &mut app);
        assert!(!app.project_conditions_ui.open);
        assert!(!app.rules_ui.register.open);
        assert!(!app.source_ui.review.open);
        assert_eq!(app.source_ui.review.subject, "controlled-component-subject");
        assert_eq!(app.host.document.editor.project, before);
        assert!(app.host.document.editor.undo.is_empty());
        press(&ctx, &mut app, "Concept mass");
        assert_eq!(app.host.document.editor.tool, Tool::ConceptMass);
    }
    #[test]
    fn unified_conditions_boundary_worker_stages_without_live_model_change_and_enters_one_review() {
        use super::super::project_sources_ui::SourceAction;
        use crate::project_repository::{LengthUnit, Repository, boundary::BoundaryPlacement};
        fn finish(app: &mut AtelierApp) {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while app.source_ui.pending_for_test() {
                app.poll_source_job();
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
        let (ctx, mut app) = setup();
        ctx.global_style_mut(|s| s.animation_time = 0.);
        let root = std::env::temp_dir().join(format!(
            "oa-cond-boundary-{}",
            storage::new_document_identity()
        ));
        std::fs::create_dir(&root).unwrap();
        let source = root.join("authored.dxf");
        std::fs::write(&source,b"0\nSECTION\n2\nENTITIES\n0\nLWPOLYLINE\n5\nA1\n8\nSITE\n90\n4\n70\n1\n10\n20\n20\n20\n10\n28\n20\n20\n10\n28\n20\n26\n10\n20\n20\n26\n0\nENDSEC\n0\nEOF\n").unwrap();
        let mut repository =
            Repository::create_for_editor(&root.join("repo"), &app.host.document.editor).unwrap();
        let id = repository.add_source(&source).unwrap();
        repository
            .confirm_unit(&id, Some(LengthUnit::Metre))
            .unwrap();
        app.source_ui.root = repository.root().display().to_string();
        app.source_ui.repository = Some(repository);
        app.source_ui.boundary_handle = "A1".into();
        app.source_ui.boundary_name = "Authored fixture site".into();
        app.source_ui.boundary_layer = 1;
        app.source_ui.boundary_placement = Some(BoundaryPlacement::ModelAxes);
        let before = app.host.document.editor.project.clone();
        press(&ctx, &mut app, "Project conditions");
        // The existing source worker performs its actual original-file checks.
        app.start_source_action(SourceAction::PreviewBoundary(id));
        finish(&mut app);
        let preview = app.source_ui.boundary_preview.clone().unwrap();
        app.start_source_action(SourceAction::CommitBoundary(preview));
        finish(&mut app);
        assert!(app.source_ui.error.is_empty(), "{}", app.source_ui.error);
        assert_eq!(app.host.document.editor.project, before);
        assert!(app.host.document.editor.undo.is_empty());
        assert_eq!(app.project_conditions_ui.commands.len(), 1);
        assert!(matches!(
            &app.project_conditions_ui.commands[0],
            crate::command::ModelCommand::ApplySiteBoundary { .. }
        ));
        press(&ctx, &mut app, "Preview all condition changes");
        finish(&mut app);
        let checked = app
            .project_conditions_ui
            .preview
            .as_ref()
            .expect("source-admitted whole review");
        assert_eq!(
            checked.prepared.project().elements.len(),
            before.elements.len() + 1
        );
        assert!(checked.prepared.project().site.boundary.is_some());
        assert_eq!(app.host.document.editor.project, before);
        press(&ctx, &mut app, "Discard condition changes");
        assert!(!app.project_conditions_has_draft());
        assert_eq!(app.host.document.editor.project, before);
        drop(app);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn normal_source_folder_context_survives_save_as_and_fresh_application() {
        fn finish(app: &mut AtelierApp) {
            let start = std::time::Instant::now();
            while app.source_ui.pending_for_ui() {
                app.poll_source_job();
                assert!(start.elapsed() < std::time::Duration::from_secs(15));
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            assert!(app.source_ui.error.is_empty(), "{}", app.source_ui.error);
        }
        fn empty_field(ctx: &Context, app: &mut AtelierApp, label: &str, value: &str) {
            let at = target(ctx, app, label, false) + vec2(170., 0.);
            layers_click(ctx, app, at);
            layers_frame_output(ctx, app, vec![Event::Text(value.into())]);
            layers_frame_output(ctx, app, vec![key(Key::Tab, Modifiers::NONE)]);
        }
        #[cfg(unix)]
        fn request(
            ep: &mut crate::automation_ipc::Endpoint,
            app: &mut AtelierApp,
            path: &std::path::Path,
            value: serde_json::Value,
        ) -> serde_json::Value {
            use std::io::{BufRead, BufReader, Write};
            let path = path.to_owned();
            let client = std::thread::spawn(move || {
                let mut socket = std::os::unix::net::UnixStream::connect(path).unwrap();
                socket
                    .set_read_timeout(Some(std::time::Duration::from_secs(8)))
                    .unwrap();
                writeln!(socket, "{value}").unwrap();
                let mut line = String::new();
                BufReader::new(socket).read_line(&mut line).unwrap();
                serde_json::from_str(&line).unwrap()
            });
            let start = std::time::Instant::now();
            while !client.is_finished() {
                ep.poll_with_host(&mut app.host, app.source_ui.repository.as_ref());
                assert!(start.elapsed() < std::time::Duration::from_secs(8));
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            client.join().unwrap()
        }
        let parent = std::env::var_os("OPENARCHI_SOURCE_RESUME_OUTPUT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!(
                    "oa-source-resume-{}",
                    storage::new_document_identity()
                ))
            });
        std::fs::create_dir_all(&parent).unwrap();
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let root = parent.join(if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            });
            std::fs::create_dir(&root).expect("preserve prior evidence; do not replay writes");
            let supplied =
                std::env::var_os("OPENARCHI_SOURCE_RESUME_INPUT").map(std::path::PathBuf::from);
            let input = setup_input(&root, supplied.as_deref());
            let repository_path = root.join("repository");
            let source_path = root.join("declared-design-context.md");
            std::fs::write(&source_path, "Controlled software fixture; keep the existing design. No surveyed dimensions or approval are declared.").unwrap();
            let saved = root.join("continued.openarchi.json");
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.initialize_source_connections(&root.join("profile"));
            open_test_project(&ctx, &mut app, &input);
            let original = app.host.document.editor.project.clone();
            press(&ctx, &mut app, "Project conditions");
            press(&ctx, &mut app, "Stored originals and source files");
            empty_field(
                &ctx,
                &mut app,
                "Project folder",
                &repository_path.display().to_string(),
            );
            press(&ctx, &mut app, "Create from current model");
            finish(&mut app);
            empty_field(
                &ctx,
                &mut app,
                "Source file",
                &source_path.display().to_string(),
            );
            press(&ctx, &mut app, "Copy source");
            finish(&mut app);
            assert_eq!(
                app.source_ui
                    .repository
                    .as_ref()
                    .unwrap()
                    .manifest
                    .sources
                    .len(),
                1
            );
            assert_eq!(app.host.document.editor.project, original);
            press(&ctx, &mut app, "Continue designing");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            text(
                &ctx,
                &mut app,
                &input.display().to_string(),
                &saved.display().to_string(),
            );
            press(&ctx, &mut app, "Save");
            assert!(saved.is_file(), "{}", app.file_error);
            let identity = app.host.document.editor.project_identity().to_owned();
            drop(app);
            let (ctx, mut reopened) = setup();
            reopened.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            reopened.initialize_source_connections(&root.join("profile"));
            open_test_project(&ctx, &mut reopened, &saved);
            finish(&mut reopened);
            // SaveAs rebases image sidecars. Compare the actual reopened document
            // against the stored complete project and retain independent asset bytes.
            let stored = storage::load(&saved).unwrap();
            assert!(
                reopened.host.document.editor.project == stored,
                "reopened complete project differs from the saved document"
            );
            let mut normalized = original.clone();
            for (before, after) in normalized
                .drawings
                .pages
                .iter_mut()
                .zip(&stored.drawings.pages)
            {
                assert_eq!(
                    crate::drawings::verified_bytes(before).unwrap(),
                    crate::drawings::verified_bytes(after).unwrap()
                );
                before.path = after.path.clone();
            }
            assert!(
                normalized == stored,
                "SaveAs changed data other than sidecar path bindings"
            );
            assert_eq!(reopened.host.document.editor.project_identity(), identity);
            let public_context = crate::automation::sources::context(
                &reopened.host.document.editor,
                reopened.source_ui.repository.as_ref(),
            );
            std::fs::write(
                root.join("reopen-context.json"),
                serde_json::to_vec_pretty(&public_context).unwrap(),
            )
            .unwrap();
            assert_eq!(
                public_context["repository_attached"],
                true,
                "normal save/reopen lost the existing source folder; input/evidence preserved at {}",
                root.display()
            );
            assert_eq!(public_context["repository_sources"], 1);
            #[cfg(unix)]
            {
                use serde_json::json;
                let socket_root = std::path::PathBuf::from("/tmp")
                    .join(format!("oa-sr-{}", &storage::new_document_identity()[..12]));
                let socket_path = socket_root.join("app.sock");
                let mut endpoint = crate::automation_ipc::Endpoint::bind_application(
                    socket_path.clone(),
                    &mut reopened.host,
                )
                .unwrap();
                let target = request(
                    &mut endpoint,
                    &mut reopened,
                    &socket_path,
                    json!({"method":"get_target"}),
                )["result"]["target"]
                    .clone();
                let context = request(
                    &mut endpoint,
                    &mut reopened,
                    &socket_path,
                    json!({"method":"get_context","params":target}),
                );
                assert_eq!(context["result"]["permissions"], json!(["read"]));
                let revision = context["result"]["revision"].clone();
                let sources_revision =
                    context["result"]["project_sources"]["sources_revision"].clone();
                let search = request(
                    &mut endpoint,
                    &mut reopened,
                    &socket_path,
                    json!({"method":"query_project_sources","params":{"target":target,"expected_revision":revision,"expected_sources_revision":sources_revision,"query":"declared-design-context","offset":0,"limit":20}}),
                );
                assert_eq!(search["result"]["total"], 1);
                let source_id = search["result"]["entries"][0]["source_id"].clone();
                let content = request(
                    &mut endpoint,
                    &mut reopened,
                    &socket_path,
                    json!({"method":"read_project_source","params":{"target":target,"expected_revision":revision,"sources_revision":sources_revision,"source_id":source_id,"representation":"original","offset":0,"limit":32768}}),
                );
                assert!(content.get("error").is_none(), "{content}");
                let bytes: Vec<u8> = content["result"]["data"]
                    .as_str()
                    .unwrap()
                    .as_bytes()
                    .chunks_exact(2)
                    .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                    .collect();
                assert_eq!(bytes, std::fs::read(&source_path).unwrap());
                assert!(reopened.host.document.editor.project == stored);
                std::fs::write(root.join("default-source-read.json"),serde_json::to_vec_pretty(&json!({"context":context,"search":search,"original_read":content,"complete_original_bytes_match":true,"model_unchanged":true})).unwrap()).unwrap();
                drop(endpoint);
                std::fs::remove_dir_all(socket_root).unwrap();
            }
            drop(reopened);
            // Missing source files restrict their reads, never the saved model
            // or unrelated concept design. Only move this test's own copy.
            let unavailable = root.join("temporarily-unavailable-repository");
            std::fs::rename(&repository_path, &unavailable).unwrap();
            let (ctx, mut without_sources) = setup();
            without_sources.language = language;
            ctx.global_style_mut(|style| style.animation_time = 0.);
            without_sources.initialize_source_connections(&root.join("profile"));
            open_test_project(&ctx, &mut without_sources, &saved);
            let start = std::time::Instant::now();
            while without_sources.source_ui.pending_for_ui() {
                without_sources.poll_source_job();
                assert!(start.elapsed() < std::time::Duration::from_secs(15));
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            assert!(without_sources.source_ui.repository.is_none());
            assert!(without_sources.source_ui.error.contains(language.translate("Source folder unavailable; continue designing or reconnect it in Project conditions")));
            press(&ctx, &mut without_sources, "Concept mass");
            assert_eq!(without_sources.host.document.editor.tool, Tool::ConceptMass);
            assert!(without_sources.host.document.editor.project == stored);
            std::fs::write(root.join("missing-source.json"), serde_json::to_vec_pretty(&serde_json::json!({"diagnostic":without_sources.source_ui.error,"concept_tool_available":true,"saved_model_preserved":true})).unwrap()).unwrap();
            drop(without_sources);
            std::fs::rename(&unavailable, &repository_path).unwrap();
        }
        if std::env::var_os("OPENARCHI_SOURCE_RESUME_OUTPUT").is_none() {
            std::fs::remove_dir_all(parent).unwrap();
        }
    }
    #[test]
    fn existing_preparation_command_ids_focus_one_window_without_model_mutation() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            app.language = language;
            let before = app.host.document.editor.project.clone();
            for (id, group) in [
                ("project.sources", ConditionsGroup::Sources),
                ("project.program", ConditionsGroup::Requirements),
                ("project.evidence", ConditionsGroup::Sources),
                ("project.rules", ConditionsGroup::Requirements),
                ("project.terrain", ConditionsGroup::Site),
                ("project.site_reference", ConditionsGroup::Site),
            ] {
                let command = crate::workbench::CommandRegistry::resolve(id).unwrap();
                app.execute_workbench(command).unwrap();
                assert!(app.project_conditions_ui.open);
                assert_eq!(app.project_conditions_ui.group, group);
                settle(&ctx, &mut app);
                assert!(app.site_dialog.is_none());
                assert_eq!(app.host.document.editor.project, before);
            }
            assert!(app.host.document.editor.undo.is_empty());
        }
    }
    #[test]
    fn normal_control_region_role_uses_existing_source_context_saves_and_detaches_bilingual() {
        control_region_normal_path(false);
    }
    #[test]
    #[ignore = "requires explicit protected project/repository/source/handle/output inputs"]
    fn supplied_project_control_region_normal_product_path_bilingual() {
        control_region_normal_path(true);
    }
    fn control_region_normal_path(supplied: bool) {
        use crate::{
            command::ModelCommand,
            project_repository::{LengthUnit, Repository, boundary::BoundaryPlacement},
            site::boundary::Role,
        };
        fn finish(ctx: &Context, app: &mut AtelierApp) {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            loop {
                layers_frame_output(ctx, app, vec![]);
                if !app.source_ui.pending_for_ui() {
                    break;
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            settle(ctx, app);
            assert!(app.source_ui.error.is_empty(), "{}", app.source_ui.error);
        }
        fn reveal(ctx: &Context, app: &mut AtelierApp, label: &str, down: bool) {
            for _ in 0..24 {
                settle(ctx, app);
                let out = layers_frame_output(ctx, app, vec![]);
                let wanted = app.language.translate(label);
                let at = out.shapes.iter().find_map(|s| {
                    text_center(std::slice::from_ref(s), wanted)
                        .filter(|p| s.clip_rect.contains(*p))
                });
                if let Some(at) = at {
                    layers_click(ctx, app, at);
                    return;
                }
                let area = ctx
                    .memory(|m| m.area_rect(Id::new("project_conditions")))
                    .unwrap();
                let at = pos2(area.left() + 12., area.center().y);
                layers_frame_output(
                    ctx,
                    app,
                    vec![
                        Event::PointerMoved(at),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            phase: egui::TouchPhase::Move,
                            delta: vec2(0., if down { -180. } else { 180. }),
                            modifiers: Modifiers::NONE,
                        },
                    ],
                );
            }
            panic!(
                "visible control {}: {}",
                label, app.project_conditions_ui.error
            )
        }
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            let root = if supplied {
                std::path::PathBuf::from(
                    std::env::var_os("OPENARCHI_CONTROL_OUTPUT").expect("explicit new output root"),
                )
                .join(if language == UiLanguage::English {
                    "en"
                } else {
                    "zh"
                })
            } else {
                std::env::temp_dir().join(format!(
                    "oa-control-ui-{}",
                    storage::new_document_identity()
                ))
            };
            std::fs::create_dir(&root).unwrap();
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            app.initialize_source_connections(&root);
            let path = root.join("saved-control.openarchi.json");
            let original = if supplied {
                let input = std::path::PathBuf::from(
                    std::env::var_os("OPENARCHI_CONTROL_MODEL").expect("input model"),
                );
                let repository = std::path::PathBuf::from(
                    std::env::var_os("OPENARCHI_CONTROL_REPOSITORY").expect("input repository"),
                );
                Some((
                    input.clone(),
                    crate::drawings::digest(&std::fs::read(&input).unwrap()),
                    repository.clone(),
                    crate::drawings::digest(
                        &std::fs::read(repository.join("project.json")).unwrap(),
                    ),
                ))
            } else {
                None
            };
            let (repository, id, record_label) = if let Some((input, _, repository_root, _)) =
                &original
            {
                let editor = Editor::from_stored_document(input).unwrap();
                assert!(
                    editor.project.site.control_region.is_none(),
                    "preserve existing control; this script only adds a new declaration"
                );
                let repository = Repository::open(repository_root)
                    .unwrap()
                    .copy_to(&root.join("repo"))
                    .unwrap();
                storage::save_document(&editor.project, &path, editor.project_identity()).unwrap();
                open_test_project(&ctx, &mut app, &path);
                let id = std::env::var("OPENARCHI_CONTROL_SOURCE").expect("exact source identity");
                let handle =
                    std::env::var("OPENARCHI_CONTROL_HANDLE").expect("exact entity handle");
                let entities = repository.boundary_entities(&id).unwrap();
                let record = entities
                    .iter()
                    .find(|r| r.handle.as_deref() == Some(handle.as_str()))
                    .expect("source record");
                let label = format!("{} · {}", record.layer.as_deref().unwrap_or(""), handle);
                (repository, id, label)
            } else {
                let mut bytes = String::from("0\nSECTION\n2\nENTITIES\n");
                for (handle, points) in [
                    (
                        "A1",
                        vec![(0, 0), (20, 0), (20, 8), (12, 8), (12, 20), (0, 20)],
                    ),
                    (
                        "A2",
                        vec![(2, 2), (10, 2), (10, 6), (18, 6), (18, 7), (8, 7), (2, 18)],
                    ),
                ] {
                    bytes += &format!(
                        "0\nLWPOLYLINE\n5\n{handle}\n8\nREFERENCE\n90\n{}\n70\n1\n",
                        points.len()
                    );
                    for (x, y) in points {
                        bytes += &format!("10\n{}\n20\n{}\n", x + 54000, y + 50000);
                    }
                }
                bytes += "0\nENDSEC\n0\nEOF\n";
                let source = root.join("declared.dxf");
                std::fs::write(&source, bytes).unwrap();
                let mut repository =
                    Repository::create_for_editor(&root.join("repo"), &app.host.document.editor)
                        .unwrap();
                let id = repository.add_source(&source).unwrap();
                repository
                    .confirm_unit(&id, Some(LengthUnit::Metre))
                    .unwrap();
                let plot = repository
                    .preview_boundary(
                        &app.host.document.editor.project,
                        &id,
                        "A1",
                        BoundaryPlacement::ModelAxes,
                        "Given existing plot declaration".into(),
                        0,
                        1,
                    )
                    .unwrap();
                app.host
                    .document
                    .editor
                    .apply(ModelCommand::ApplySiteBoundary { proposal: plot })
                    .unwrap();
                (repository, id, "REFERENCE · A2".to_owned())
            };
            // Given already-connected project and previously-read source catalogue.
            app.source_ui.boundary_entities =
                Some((id.clone(), repository.boundary_entities(&id).unwrap()));
            app.source_ui.root = repository.root().display().to_string();
            app.source_ui.repository = Some(repository);
            let before = app.host.document.editor.project.clone();
            let undo_before = app.host.document.editor.undo.len();
            press(&ctx, &mut app, "Project conditions");
            press(&ctx, &mut app, "Stored originals and source files");
            reveal(&ctx, &mut app, "Declared control region", true);
            assert_eq!(app.source_ui.boundary_role, Role::ControlRegion);
            assert!(app.source_ui.boundary_placement.is_none());
            // Real drawings have more than the intentionally bounded first page.
            // Use the existing visible filter rather than reaching into its state.
            reveal(&ctx, &mut app, "Filter boundary layer or handle", true);
            let field = ctx
                .data(|d| d.get_temp::<Rect>(Id::new("boundary_filter_field")))
                .unwrap();
            layers_click(&ctx, &mut app, field.center());
            let handle = record_label.rsplit(" · ").next().unwrap();
            layers_frame_output(
                &ctx,
                &mut app,
                vec![
                    key(Key::A, Modifiers::COMMAND),
                    Event::Text(handle.to_owned()),
                ],
            );
            reveal(&ctx, &mut app, &record_label, true);
            finish(&ctx, &mut app);
            let preview = app
                .source_ui
                .boundary_preview
                .as_ref()
                .expect("actual source-record selection previews control");
            assert_eq!(preview.role, Role::ControlRegion);
            assert!(preview.proposal.expected_previous.is_none());
            assert_eq!(preview.proposal.outline_m.len(), 7);
            assert_eq!(
                app.source_ui.boundary_placement,
                Some(BoundaryPlacement::ModelAxes),
                "reuse declared placement, not repeat form input"
            );
            reveal(&ctx, &mut app, "Use preview as control region", true);
            finish(&ctx, &mut app);
            assert_eq!(app.host.document.editor.project, before);
            assert_eq!(app.host.document.editor.undo.len(), undo_before);
            assert!(matches!(
                app.project_conditions_ui.commands[0],
                ModelCommand::ApplySiteControlRegion { .. }
            ));
            press(&ctx, &mut app, "Preview all condition changes");
            finish(&ctx, &mut app);
            assert_eq!(app.host.document.editor.project, before);
            let proposed = app
                .project_conditions_ui
                .preview
                .as_ref()
                .unwrap()
                .prepared
                .project();
            assert!(proposed.site.control_region.is_some());
            assert_eq!(proposed.site.boundary, before.site.boundary);
            press(&ctx, &mut app, "Save project conditions");
            finish(&ctx, &mut app);
            assert!(
                app.project_conditions_ui.error.is_empty(),
                "{}",
                app.project_conditions_ui.error
            );
            let saved = app.host.document.editor.project.clone();
            assert!(saved.site.control_region.is_some());
            assert_eq!(saved.elements[..before.elements.len()], before.elements);
            assert_eq!(app.host.document.editor.undo.len(), undo_before + 1);
            // The initial save is already in flight: finish its normal file dialog,
            // rather than starting another save or guessing that the window closed.
            if supplied {
                assert!(
                    app.file_action.is_none(),
                    "existing protected file saves without repeating its path"
                );
                assert!(
                    !app.project_conditions_ui.file_save_pending,
                    "{}",
                    app.project_conditions_ui.error
                );
            } else {
                assert!(app.file_action.is_some());
                let old = app.file_dialog_path.clone().unwrap();
                text(&ctx, &mut app, &old, &path.display().to_string());
                press(&ctx, &mut app, "Save");
                finish(&ctx, &mut app);
            }
            assert!(app.file_action.is_none());
            assert_eq!(Editor::from_stored_document(&path).unwrap().project, saved);
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&std::fs::read(&path).unwrap())
                    .unwrap()["schema_version"],
                24
            );
            press(&ctx, &mut app, "Continue designing");
            assert!(!app.project_conditions_ui.open);
            let control_id = saved.site.control_region.as_ref().unwrap().element_id;
            assert_eq!(app.host.document.editor.selection_ids(), vec![control_id]);
            assert_eq!(app.view, View::Plan);
            let viewport = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
            let outline = crate::site::boundary::outline_for(&saved, Role::ControlRegion)
                .unwrap()
                .unwrap();
            for point in outline {
                assert!(
                    viewport.contains(app.screen(viewport, Point::new(point[0], point[1]))),
                    "imported control must be visible on returning to the canvas"
                );
            }
            if supplied {
                // Preserve the accepted import before detach/undo probes below.
                storage::save_document(
                    &saved,
                    &root.join("accepted-control.internal.json"),
                    app.host.document.editor.project_identity(),
                )
                .unwrap();
            }
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(app.host.document.editor.project, before);
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::ALT)],
            );
            assert_eq!(app.host.document.editor.project, saved);
            press(&ctx, &mut app, "Project conditions");
            press(&ctx, &mut app, "Site and orientation");
            press(
                &ctx,
                &mut app,
                "Detach control region and keep its Polyline",
            );
            assert_eq!(app.host.document.editor.project, saved);
            press(&ctx, &mut app, "Preview all condition changes");
            finish(&ctx, &mut app);
            let proposed = app
                .project_conditions_ui
                .preview
                .as_ref()
                .unwrap()
                .prepared
                .project();
            assert!(proposed.site.control_region.is_none());
            assert_eq!(proposed.site.boundary, saved.site.boundary);
            assert_eq!(proposed.elements, saved.elements);
            press(&ctx, &mut app, "Save project conditions");
            finish(&ctx, &mut app);
            assert!(
                app.host
                    .document
                    .editor
                    .project
                    .site
                    .control_region
                    .is_none()
            );
            assert_eq!(app.host.document.editor.project.elements, saved.elements);
            // Shared detach path must also keep the original site command working.
            press(&ctx, &mut app, "Project conditions");
            press(&ctx, &mut app, "Site and orientation");
            press(&ctx, &mut app, "Detach boundary and keep its Polyline");
            press(&ctx, &mut app, "Preview all condition changes");
            finish(&ctx, &mut app);
            assert!(
                app.project_conditions_ui
                    .preview
                    .as_ref()
                    .unwrap()
                    .prepared
                    .project()
                    .site
                    .boundary
                    .is_none()
            );
            press(&ctx, &mut app, "Discard condition changes");
            assert_eq!(
                app.host.document.editor.project.site.boundary,
                saved.site.boundary
            );
            drop(app);
            if let Some((input, input_hash, repository, manifest_hash)) = original {
                assert_eq!(
                    crate::drawings::digest(&std::fs::read(&input).unwrap()),
                    input_hash
                );
                assert_eq!(
                    crate::drawings::digest(
                        &std::fs::read(repository.join("project.json")).unwrap()
                    ),
                    manifest_hash
                );
                std::fs::write(root.join("normal-ui-result.json"),serde_json::to_vec_pretty(&serde_json::json!({"entry":"normal file-open, project-conditions source record/review/save, continue canvas, UndoRedo and detach/discard egui events","input":input,"input_sha256":input_hash,"original_repository":repository,"original_manifest_sha256":manifest_hash,"originals_unchanged":true,"accepted_import":root.join("accepted-control.internal.json"),"current_model_has_detach_probe":true,"native_desktop":"not_run","real_ai":"not_run"})).unwrap()).unwrap();
            } else {
                std::fs::remove_dir_all(root).unwrap();
            }
        }
    }
}
