#[cfg(unix)]
mod menu_toolbars_product_path {
    use super::report_environment_product_path::{frames, named, open, replace};
    use super::*;
    use egui::accesskit::{Action, ActionRequest, TreeId};

    fn action(ctx: &Context, app: &mut AtelierApp, label: &str, last: bool) {
        reveal_manual_control(ctx, app, label);
        frames(ctx, app);
        let out = layers_frame_output(ctx, app, vec![]);
        let mut nodes = out
            .platform_output
            .accesskit_update
            .unwrap()
            .nodes
            .into_iter()
            .filter(|(_, n)| {
                n.supports_action(Action::Click)
                    && !n.is_disabled()
                    && (n.label() == Some(app.language.translate(label))
                        || n.value() == Some(app.language.translate(label)))
            })
            .collect::<Vec<_>>();
        nodes.sort_by(|a, b| {
            a.1.bounds()
                .map(|r| r.y0)
                .unwrap_or_default()
                .total_cmp(&b.1.bounds().map(|r| r.y0).unwrap_or_default())
        });
        let (id, _) = if last { nodes.last() } else { nodes.first() }.expect(label);
        layers_frame_output(
            ctx,
            app,
            vec![Event::AccessKitActionRequest(ActionRequest {
                action: Action::Click,
                target_tree: TreeId::ROOT,
                target_node: *id,
                data: None,
            })],
        );
        frames(ctx, app);
    }
    fn top_menu(ctx: &Context, app: &mut AtelierApp, label: &str) {
        frames(ctx, app);
        let out = layers_frame_output(ctx, app, vec![]);
        let text = app.language.translate(label);
        let at = out
            .shapes
            .iter()
            .filter_map(|s| {
                text_center(std::slice::from_ref(s), text).filter(|p| s.clip_rect.contains(*p))
            })
            .min_by(|a, b| a.y.total_cmp(&b.y))
            .expect("visible main menu label");
        layers_click(ctx, app, at);
        frames(ctx, app);
        assert!(
            egui::Popup::is_any_open(ctx),
            "main menu {label} must actually open at {at:?}; checks_open={}",
            app.checks_ui.open
        );
    }
    fn hover_submenu(ctx: &Context, app: &mut AtelierApp, label: &str) {
        frames(ctx, app);
        let out = layers_frame_output(ctx, app, vec![]);
        let text = app.language.translate(label);
        let at = out
            .shapes
            .iter()
            .filter_map(|s| {
                text_center(std::slice::from_ref(s), text).filter(|p| s.clip_rect.contains(*p))
            })
            .min_by(|a, b| a.y.total_cmp(&b.y));
        assert!(
            at.is_some(),
            "visible submenu {label}; AX={:?}",
            out.platform_output
                .accesskit_update
                .unwrap()
                .nodes
                .iter()
                .filter_map(|(_, n)| n.label().map(|s| (
                    n.role(),
                    s,
                    n.supports_action(Action::Click)
                )))
                .collect::<Vec<_>>()
        );
        layers_frame_output(ctx, app, vec![Event::PointerMoved(at.unwrap())]);
        frames(ctx, app);
    }
    fn close_check(ctx: &Context, app: &mut AtelierApp) {
        frames(ctx, app);
        let out = layers_frame_output(ctx, app, vec![]);
        let nodes = out.platform_output.accesskit_update.unwrap().nodes;
        let matches = nodes
            .iter()
            .filter(|(_, n)| n.supports_action(Action::Click) && n.label() == Some("Close window"))
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1, "only the existing check window is open");
        let id = matches[0].0;
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
        assert!(!app.checks_ui.open);
    }
    fn add_check(ctx: &Context, app: &mut AtelierApp) {
        frames(ctx, app);
        let out = layers_frame_output(ctx, app, vec![]);
        let (id, node) = out
            .platform_output
            .accesskit_update
            .unwrap()
            .nodes
            .into_iter()
            .find(|(_, n)| {
                n.supports_action(Action::Click)
                    && n.label() == Some(app.language.translate("Add project menu command"))
            })
            .expect("existing project menu add button");
        let anchor = node.bounds().unwrap();
        let mut nodes = std::collections::BTreeMap::new();
        let click = Event::AccessKitActionRequest(ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: id,
            data: None,
        });
        for events in std::iter::once(vec![click]).chain((0..6).map(|_| vec![])) {
            for (id, node) in layers_frame_output(ctx, app, events)
                .platform_output
                .accesskit_update
                .unwrap()
                .nodes
            {
                nodes.insert(id, node);
            }
        }
        let id = nodes
            .iter()
            .filter(|(_, n)| {
                n.supports_action(Action::Click)
                    && n.label() == Some(app.language.translate("Check current design"))
            })
            .min_by(|a, b| {
                let distance = |n: &egui::accesskit::Node| {
                    n.bounds()
                        .map(|r| (r.x0 - anchor.x0).powi(2) + (r.y0 - anchor.y0).powi(2))
                        .unwrap_or(f64::MAX)
                };
                distance(a.1).total_cmp(&distance(b.1))
            })
            .map(|(id, _)| *id)
            .expect("normal popup command");
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
    fn model(mut p: Project, base: &std::path::Path) -> Project {
        use sha2::{Digest, Sha256};
        for page in &mut p.drawings.pages {
            let path = if page.path.is_absolute() {
                page.path.clone()
            } else {
                base.join(&page.path)
            };
            assert_eq!(
                format!("{:x}", Sha256::digest(std::fs::read(path).unwrap())),
                page.sha256
            );
            page.path = PathBuf::from(&page.sha256);
        }
        p
    }
    #[test]
    fn personal_project_menu_toolbars_normal_cancel_save_restore_reopen_bilingual() {
        let root = std::env::temp_dir().join(format!(
            "oa-menu-toolbars-{}",
            storage::new_document_identity()
        ));
        std::fs::create_dir(&root).unwrap();
        println!("MENU_UI_ROOT={}", root.display());
        let input = if let Some(p) = std::env::var_os("OPENARCHI_ENV_REVIEW_PROJECT") {
            PathBuf::from(p)
        } else {
            let path = root.join("input.openarchi.json");
            let e = Editor::new(storage::startup_example());
            storage::save_document(&e.project, &path, e.project_identity()).unwrap();
            path
        };
        let input_bytes = std::fs::read(&input).unwrap();
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
            let authority = app.host.object_task_authorization();
            named(&ctx, &mut app, "Customize work environment…", false);
            named(&ctx, &mut app, "Clone as personal mode", false);
            named(&ctx, &mut app, "Standard toolbar", false);
            named(&ctx, &mut app, "Edit toolbar", false);
            assert!(
                app.standard_toolbar_visible && app.edit_toolbar_visible,
                "draft must not hide live controls"
            );
            named(&ctx, &mut app, "Cancel environment edits", false);
            assert!(!config.join("work-environment.json").exists());
            named(&ctx, &mut app, "Customize work environment…", false);
            named(&ctx, &mut app, "Clone as personal mode", false);
            let old = app
                .workbench
                .configuration_draft
                .as_ref()
                .unwrap()
                .1
                .name
                .clone();
            replace(&ctx, &mut app, &old, "Canvas menu QA");
            named(&ctx, &mut app, "Standard toolbar", false);
            named(&ctx, &mut app, "Edit toolbar", false);
            named(&ctx, &mut app, "Project menu", false);
            action(&ctx, &mut app, "Move down", true);
            assert_eq!(
                app.workbench
                    .configuration_draft
                    .as_ref()
                    .unwrap()
                    .1
                    .project_menu,
                vec!["project.conditions", "project.space_options"]
            );
            add_check(&ctx, &mut app);
            assert_eq!(
                app.workbench
                    .configuration_draft
                    .as_ref()
                    .unwrap()
                    .1
                    .project_menu,
                vec![
                    "project.conditions",
                    "project.space_options",
                    "analysis.constraints"
                ]
            );
            named(&ctx, &mut app, "Save and activate mode", false);
            assert!(
                app.workbench.configuration_error.is_empty(),
                "{}",
                app.workbench.configuration_error
            );
            let profile = app.workbench.active_profile.clone();
            named(&ctx, &mut app, "Cancel environment edits", false);
            assert!(!app.standard_toolbar_visible && !app.edit_toolbar_visible);
            let out = layers_frame_output(&ctx, &mut app, vec![]);
            assert!(text_center(&out.shapes, language.translate("EDIT")).is_none());
            assert!(text_center(&out.shapes, language.translate("Guides")).is_none());
            // The normal project menu opens the original check service without a context transfer.
            top_menu(&ctx, &mut app, "Project");
            action(&ctx, &mut app, "Check current design", false);
            assert!(
                !egui::Popup::is_any_open(&ctx),
                "project menu choice closes the actual popup"
            );
            assert!(
                app.checks_ui.open,
                "personal menu invokes the existing service"
            );
            close_check(&ctx, &mut app);
            // Native Window command uses the same guarded toggle as the in-window menu.
            top_menu(&ctx, &mut app, "Window");
            hover_submenu(&ctx, &mut app, "Toolbars");
            named(&ctx, &mut app, "Standard toolbar", false);
            assert!(app.standard_toolbar_visible && !app.edit_toolbar_visible);
            assert!(app.workspace_layout_dirty());
            named(&ctx, &mut app, "Customize work environment…", false);
            assert!(
                app.workbench
                    .configuration_draft
                    .as_ref()
                    .unwrap()
                    .1
                    .panels
                    .standard_toolbar
            );
            named(&ctx, &mut app, "Cancel environment edits", false);
            assert!(app.standard_toolbar_visible);
            // Save the real live change with the original layout action, not by assigning configuration.
            top_menu(&ctx, &mut app, "Window");
            named(&ctx, &mut app, "Save workspace layout", false);
            assert!(!app.workspace_layout_dirty());
            assert_eq!(app.host.document.editor.project, original);
            assert_eq!(app.host.context(), context);
            assert_eq!(app.host.document.editor.selection_ids(), selection);
            assert_eq!(
                (
                    app.host.document.editor.undo.len(),
                    app.host.document.editor.redo.len()
                ),
                history
            );
            assert_eq!(app.host.object_task_authorization(), authority);
            let path = config.join("saved.openarchi.json");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let previous = app.file_dialog_path.clone().unwrap();
            replace(&ctx, &mut app, &previous, &path.display().to_string());
            named(&ctx, &mut app, "Save", false);
            assert!(path.exists(), "{}", app.file_error);
            let (ctx2, mut reopened) = setup();
            ctx2.enable_accesskit();
            ctx2.global_style_mut(|s| {
                s.animation_time = 0.;
                s.scroll_animation = egui::style::ScrollAnimation::none();
            });
            reopened.language = language;
            reopened.initialize_work_mode(&config);
            open(&ctx2, &mut reopened, &path);
            assert_eq!(reopened.workbench.active_profile, profile);
            assert!(reopened.standard_toolbar_visible && !reopened.edit_toolbar_visible);
            assert_eq!(
                reopened.current_environment_profile().project_menu,
                vec![
                    "project.conditions",
                    "project.space_options",
                    "analysis.constraints"
                ]
            );
            assert_eq!(
                model(
                    reopened.host.document.editor.project.clone(),
                    path.parent().unwrap()
                ),
                model(original, input.parent().unwrap())
            );
        }
        assert_eq!(std::fs::read(input).unwrap(), input_bytes);
    }
}
