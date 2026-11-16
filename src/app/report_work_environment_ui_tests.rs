// Normal workbench/configuration controls; no private project or alternate model.
#[cfg(unix)]
mod report_environment_product_path {
    use super::*;
    use egui::accesskit::{Action, ActionRequest, Role, TreeId};

    pub(super) fn frames(ctx: &Context, app: &mut AtelierApp) {
        for _ in 0..6 {
            layers_frame_output(ctx, app, vec![]);
        }
    }
    pub(super) fn named(ctx: &Context, app: &mut AtelierApp, label: &str, last: bool) {
        reveal_manual_control(ctx, app, label);
        frames(ctx, app);
        let out = layers_frame_output(ctx, app, vec![]);
        let nodes = out.platform_output.accesskit_update.unwrap().nodes;
        let mut matches = nodes
            .iter()
            .filter(|(_, n)| {
                n.supports_action(Action::Click)
                    && (label != "Toolbox" || n.role() == Role::CheckBox)
                    && (n.label() == Some(app.language.translate(label))
                        || n.value() == Some(app.language.translate(label)))
            })
            .map(|(id, node)| (*id, node.bounds()))
            .collect::<Vec<_>>();
        // AccessKit node storage order is not the visual row order. Reordering
        // targets the first visible command, rather than an arbitrary hashed ID.
        if label == "Move down" {
            matches.sort_by(|a, b| {
                a.1.map(|r| r.y0)
                    .unwrap_or(f64::MAX)
                    .total_cmp(&b.1.map(|r| r.y0).unwrap_or(f64::MAX))
            });
        }
        let ids = matches.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        let id = if last { ids.last() } else { ids.first() }
            .copied()
            .unwrap_or_else(|| {
                panic!(
                    "normal action {label}: {}",
                    app.workbench.configuration_error
                )
            });
        layers_frame_output(
            ctx,
            app,
            vec![Event::AccessKitActionRequest(ActionRequest {
                action: Action::Click,
                target_tree: TreeId::ROOT,
                target_node: id,
                data: None,
            })],
        );
        frames(ctx, app);
    }
    pub(super) fn replace(ctx: &Context, app: &mut AtelierApp, previous: &str, value: &str) {
        let out = layers_frame_output(ctx, app, vec![]);
        let id = out
            .platform_output
            .accesskit_update
            .unwrap()
            .nodes
            .iter()
            .find(|(_, n)| {
                n.role() == Role::TextInput
                    && n.value() == Some(previous)
                    && n.supports_action(Action::Focus)
            })
            .map(|(id, _)| *id)
            .expect("normal editable value");
        layers_frame_output(
            ctx,
            app,
            vec![Event::AccessKitActionRequest(ActionRequest {
                action: Action::Focus,
                target_tree: TreeId::ROOT,
                target_node: id,
                data: None,
            })],
        );
        layers_frame_output(
            ctx,
            app,
            vec![
                key(Key::A, Modifiers::COMMAND),
                Event::Text(value.into()),
                key(Key::Tab, Modifiers::NONE),
            ],
        );
        frames(ctx, app);
    }
    pub(super) fn open(ctx: &Context, app: &mut AtelierApp, path: &std::path::Path) {
        open_test_project(ctx, app, &path);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while app.source_ui.pending_for_ui() {
            assert!(std::time::Instant::now() < until);
            layers_frame_output(ctx, app, vec![]);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(app.source_ui.error.is_empty(), "{}", app.source_ui.error);
    }
    #[test]
    fn report_environment_normal_mode_clone_save_and_reopen_bilingual() {
        let root = std::env::temp_dir().join(format!(
            "oa-report-mode-{}",
            storage::new_document_identity()
        ));
        std::fs::create_dir(&root).unwrap();
        let input = if let Some(path) = std::env::var_os("OPENARCHI_ENV_REVIEW_PROJECT") {
            std::path::PathBuf::from(path)
        } else {
            let author = Editor::new(storage::startup_example());
            let input = root.join("source.openarchi.json");
            storage::save_document(&author.project, &input, author.project_identity()).unwrap();
            input
        };
        let original_bytes = std::fs::read(&input).unwrap();
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let config = root.join(if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            });
            std::fs::create_dir(&config).unwrap();
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| {
                s.animation_time = 0.;
                s.scroll_animation = egui::style::ScrollAnimation::none();
            });
            app.language = language;
            app.initialize_work_mode(&config);
            open(&ctx, &mut app, &input);
            let original = app.host.document.editor.project.clone();
            let context = app.host.context();
            let selection = app.host.document.editor.selection_ids();
            let history = (
                app.host.document.editor.undo.len(),
                app.host.document.editor.redo.len(),
            );
            named(&ctx, &mut app, "Architectural design", false);
            named(&ctx, &mut app, "Reports and drawings", false);
            assert_eq!(app.workbench.active_profile, "openarchi.reports-drawings");
            assert_eq!(app.host.context(), context);
            // The task-strip entry follows the existing main-toolbar entry in
            // the normal product tree. Exercise both through shared dispatch.
            named(&ctx, &mut app, "Drawing Manager", true);
            assert!(app.drawings_ui.manager);
            named(&ctx, &mut app, "Drawing Manager", false);
            assert!(!app.drawings_ui.manager);
            named(&ctx, &mut app, "Report templates", false);
            assert!(app.drawings_ui.report_open);
            assert_eq!(
                app.drawings_ui.manager,
                original.drawings.reports.is_empty()
            );
            if app.drawings_ui.manager {
                named(&ctx, &mut app, "Drawing Manager", true);
            }
            named(&ctx, &mut app, "2D Plan", false);
            named(&ctx, &mut app, "Customize work environment…", false);
            let before_cancel = app.workbench.active_profile.clone();
            named(&ctx, &mut app, "Clone as personal mode", false);
            named(&ctx, &mut app, "Cancel environment edits", false);
            assert_eq!(app.workbench.active_profile, before_cancel);
            assert!(!config.join("work-environment.json").exists());
            named(&ctx, &mut app, "Customize work environment…", false);
            named(&ctx, &mut app, "Clone as personal mode", false);
            let old_name = app
                .workbench
                .configuration_draft
                .as_ref()
                .unwrap()
                .1
                .name
                .clone();
            replace(&ctx, &mut app, &old_name, "My figure review");
            named(&ctx, &mut app, "Move down", false);
            let mut expected = crate::workbench::environment::Profile::builtin(
                crate::workbench::WorkMode::ReportsDrawings,
            )
            .commands;
            expected.swap(0, 1);
            assert_eq!(
                app.workbench
                    .configuration_draft
                    .as_ref()
                    .unwrap()
                    .1
                    .commands,
                expected
            );
            named(&ctx, &mut app, "Toolbox", true);
            assert!(
                !app.workbench
                    .configuration_draft
                    .as_ref()
                    .unwrap()
                    .1
                    .panels
                    .toolbox
            );
            named(&ctx, &mut app, "Save and activate mode", false);
            assert!(
                app.workbench.configuration_error.is_empty(),
                "{}",
                app.workbench.configuration_error
            );
            let saved_id = app.workbench.active_profile.clone();
            assert!(saved_id.starts_with("user."));
            assert_eq!(app.current_environment_profile().commands, expected);
            assert_eq!(app.host.context(), context);
            assert_eq!(app.host.document.editor.project, original);
            assert_eq!(app.host.document.editor.selection_ids(), selection);
            assert_eq!(
                (
                    app.host.document.editor.undo.len(),
                    app.host.document.editor.redo.len()
                ),
                history
            );
            // Fresh application state reads the same normal configuration path,
            // then normal Open reads the project; no state is assigned directly.
            let (fresh_ctx, mut fresh) = setup();
            fresh_ctx.enable_accesskit();
            fresh.language = language;
            fresh.initialize_work_mode(&config);
            open(&fresh_ctx, &mut fresh, &input);
            assert_eq!(fresh.workbench.active_profile, saved_id);
            assert_eq!(fresh.current_environment_profile().commands, expected);
            assert!(!fresh.show_left);
            assert_eq!(fresh.host.document.editor.project, original);
            named(&fresh_ctx, &mut fresh, "Drawing Manager", true);
            assert!(fresh.drawings_ui.manager);
            assert_eq!(std::fs::read(&input).unwrap(), original_bytes);
        }
    }
}
