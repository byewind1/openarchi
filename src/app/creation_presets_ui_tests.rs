#[cfg(unix)]
mod creation_presets_product_path {
    use super::report_environment_product_path::{frames, named, open, replace};
    use super::*;
    use egui::accesskit::{Action, ActionRequest, Role, TreeId};

    fn mode(ctx: &Context, app: &mut AtelierApp, name: &str) {
        let current = app.current_environment_profile();
        let current = if current.builtin_name {
            app.language.translate(&current.name)
        } else {
            &current.name
        };
        let out = layers_frame_output(ctx, app, vec![]);
        let nodes = out.platform_output.accesskit_update.unwrap().nodes;
        let id = nodes
            .iter()
            .find(|(_, n)| {
                n.role() == Role::ComboBox
                    && n.supports_action(Action::Click)
                    && (n.label() == Some(current) || n.value() == Some(current))
            })
            .map(|(id, _)| *id)
            .expect("normal work mode selector");
        let click = |id| {
            Event::AccessKitActionRequest(ActionRequest {
                action: Action::Click,
                target_tree: TreeId::ROOT,
                target_node: id,
                data: None,
            })
        };
        // AccessKit updates are deltas: retain the popup's first-frame items.
        let mut nodes = std::collections::BTreeMap::new();
        for events in std::iter::once(vec![click(id)]).chain((0..6).map(|_| vec![])) {
            for (id, node) in layers_frame_output(ctx, app, events)
                .platform_output
                .accesskit_update
                .unwrap()
                .nodes
            {
                nodes.insert(id, node);
            }
        }
        let name = app.language.translate(name);
        let id = nodes
            .iter()
            .find(|(_, n)| {
                n.role() != Role::ComboBox
                    && n.supports_action(Action::Click)
                    && (n.label() == Some(name) || n.value() == Some(name))
            })
            .map(|(id, _)| *id)
            .unwrap_or_else(|| {
                panic!(
                    "normal selectable mode item {name}: {:?}",
                    nodes
                        .values()
                        .filter(|n| n.supports_action(Action::Click))
                        .map(|n| (n.role(), n.label(), n.value()))
                        .collect::<Vec<_>>()
                )
            });
        layers_frame_output(ctx, app, vec![click(id)]);
        frames(ctx, app);
        assert!(
            !egui::Popup::is_any_open(ctx),
            "work mode choice must close the popup"
        );
    }

    fn number(ctx: &Context, app: &mut AtelierApp, label: &str, suffix: &str, value: &str) {
        frames(ctx, app);
        let out = layers_frame_output(ctx, app, vec![]);
        let label = text_center(&out.shapes, app.language.translate(label))
            .expect("visible preset field label");
        let pos = out
            .shapes
            .iter()
            .filter_map(|s| {
                if let egui::Shape::Text(t) = &s.shape {
                    let p = t.pos + t.galley.size() * 0.5;
                    if t.galley.text().ends_with(suffix)
                        && (p.y - label.y).abs() < 12.
                        && p.x > label.x
                        && s.clip_rect.contains(p)
                    {
                        return Some(p);
                    }
                }
                None
            })
            // Adjacent labelled fields can share this row. Pick the first
            // numeric control after this label, not the last field in the row.
            .min_by(|a, b| a.x.total_cmp(&b.x))
            .expect("visible numeric field immediately after its label");
        layers_click(ctx, app, pos);
        layers_frame_output(
            ctx,
            app,
            vec![
                key(Key::A, Modifiers::COMMAND),
                Event::Text(value.into()),
                key(Key::Enter, Modifiers::NONE),
            ],
        );
        frames(ctx, app);
    }
    fn settings(ctx: &Context, app: &mut AtelierApp) {
        named(ctx, app, "Customize work environment…", false);
        named(ctx, app, "Concept creation presets", false);
    }
    fn pointer_text(ctx: &Context, app: &mut AtelierApp, text: &str) {
        frames(ctx, app);
        let out = layers_frame_output(ctx, app, vec![]);
        let text = app.language.translate(text);
        let at = out
            .shapes
            .iter()
            .filter_map(|s| {
                text_center(std::slice::from_ref(s), text).filter(|p| s.clip_rect.contains(*p))
            })
            .min_by(|a, b| a.y.total_cmp(&b.y))
            .expect("visible existing option control");
        layers_click(ctx, app, at);
        frames(ctx, app);
    }
    fn normalized(mut project: Project, base: &std::path::Path) -> Project {
        use sha2::{Digest, Sha256};
        for page in &mut project.drawings.pages {
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
        project
    }
    #[test]
    fn concept_presets_normal_config_cancel_conflict_create_undo_save_reopen_bilingual() {
        let root = std::env::temp_dir().join(format!(
            "oa-creation-presets-{}",
            storage::new_document_identity()
        ));
        std::fs::create_dir(&root).unwrap();
        println!("PRESET_UI_ROOT={}", root.display());
        let input = if let Some(path) = std::env::var_os("OPENARCHI_ENV_REVIEW_PROJECT") {
            PathBuf::from(path)
        } else {
            let p = root.join("input.openarchi.json");
            let e = Editor::new(storage::startup_example());
            storage::save_document(&e.project, &p, e.project_identity()).unwrap();
            p
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
            let authority = app.host.object_task_authorization();
            let defaults = app.host.creation_defaults();
            settings(&ctx, &mut app);
            named(&ctx, &mut app, "Clone as personal mode", false);
            named(&ctx, &mut app, "Capture current creation defaults", false);
            number(&ctx, &mut app, "New concept mass height", " mm", "6850");
            assert_eq!(
                app.workbench
                    .configuration_draft
                    .as_ref()
                    .unwrap()
                    .1
                    .creation_presets
                    .as_ref()
                    .unwrap()
                    .mass_height_mm,
                6850
            );
            assert_eq!(app.host.creation_defaults(), defaults);
            named(
                &ctx,
                &mut app,
                "Configuration source and differences",
                false,
            );
            named(&ctx, &mut app, "Reset draft to bundled defaults", false);
            let (draft, profile) = app.workbench.configuration_draft.as_ref().unwrap();
            assert!(
                profile.creation_presets.is_none()
                    && draft
                        .resolve(&profile.id)
                        .unwrap()
                        .creation_presets
                        .is_none(),
                "explicit Reset must clear the personal creation preset override"
            );
            assert_eq!(app.host.creation_defaults(), defaults);
            named(&ctx, &mut app, "Cancel environment edits", false);
            assert!(!config.join("work-environment.json").exists());
            assert_eq!(app.host.document.editor.project, original);
            named(&ctx, &mut app, "Customize work environment…", false);
            // The existing collapsing state is retained after cancelling the window.
            named(&ctx, &mut app, "Clone as personal mode", false);
            let old = app
                .workbench
                .configuration_draft
                .as_ref()
                .unwrap()
                .1
                .name
                .clone();
            replace(&ctx, &mut app, &old, "Concept defaults QA");
            assert_eq!(
                app.workbench.configuration_draft.as_ref().unwrap().1.name,
                "Concept defaults QA"
            );
            named(&ctx, &mut app, "Capture current creation defaults", false);
            number(&ctx, &mut app, "New concept mass height", " mm", "6850");
            number(&ctx, &mut app, "New Zone clear height", " mm", "3250");
            replace(&ctx, &mut app, "general", "Concept teaching (not approved)");
            assert_eq!(app.host.creation_defaults(), defaults);
            named(&ctx, &mut app, "Save and activate mode", false);
            assert!(
                app.workbench.configuration_error.is_empty(),
                "{}",
                app.workbench.configuration_error
            );
            let id = app.workbench.active_profile.clone();
            assert_eq!(
                app.host.creation_defaults(),
                (6.85, 3.25, "Concept teaching (not approved)".into())
            );
            layers_frame_output(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
            frames(&ctx, &mut app);
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
            // A later live edit is not authorized by reading the saved profile.
            named(&ctx, &mut app, "Customize work environment…", false);
            let saved = std::fs::read(config.join("work-environment.json")).unwrap();
            let mut changed = app.current_environment_profile().creation_presets.unwrap();
            changed.mass_height_mm = 7100;
            app.host.apply_creation_presets(&changed).unwrap();
            named(&ctx, &mut app, "Save and activate mode", false);
            assert!(
                app.workbench
                    .configuration_error
                    .contains("Live creation defaults changed")
            );
            assert_eq!(
                std::fs::read(config.join("work-environment.json")).unwrap(),
                saved
            );
            assert_eq!(app.host.creation_defaults().0, 7.1);
            named(&ctx, &mut app, "Cancel environment edits", false);
            mode(&ctx, &mut app, "Concept defaults QA");
            assert_eq!(app.host.creation_defaults().0, 6.85);
            // Normal mode changes with no preset leave live defaults alone.
            mode(&ctx, &mut app, "Reports and drawings");
            assert_eq!(app.host.creation_defaults().0, 6.85);
            mode(&ctx, &mut app, "Concept defaults QA");
            assert_eq!(app.workbench.active_profile, id);
            assert_eq!(app.host.document.editor.project, original);
            // Explicitly work in the existing common-model context; do not
            // enlarge or overwrite an owned generated candidate.
            if let Some(option) = app
                .host
                .document
                .editor
                .project
                .plan_options
                .active_design_option
            {
                let name = app
                    .host
                    .document
                    .editor
                    .project
                    .design_options
                    .name(option)
                    .unwrap()
                    .to_owned();
                pointer_text(&ctx, &mut app, &name);
                pointer_text(&ctx, &mut app, "Main Model Only");
            }
            app.plan_center = Point::new(32., 32.);
            app.plan_scale = 40.;
            app.host.document.editor.snap = false;
            named(&ctx, &mut app, "2D Plan", false);
            named(&ctx, &mut app, "Concept mass", false);
            frames(&ctx, &mut app);
            let view = app.workbench.panel_rects["view.canvas"];
            // An explicit mode application may wait for the gesture, but that
            // pending application must not overwrite later direct user input.
            number(&ctx, &mut app, "Height", " m", "7.1");
            let a = app.screen(view, Point::new(30., 30.));
            layers_click(&ctx, &mut app, a);
            assert!(app.host.document.editor.anchor.is_some());
            mode(&ctx, &mut app, "Concept defaults QA");
            assert_eq!(app.host.creation_defaults().0, 7.1);
            number(&ctx, &mut app, "Height", " m", "7.2");
            layers_frame_output(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
            frames(&ctx, &mut app);
            assert_eq!(app.host.creation_defaults().0, 7.2);
            assert!(
                app.workbench
                    .configuration_error
                    .contains("Live creation defaults changed")
            );
            assert_eq!(app.host.document.editor.project.elements, original.elements);
            named(&ctx, &mut app, "Customize work environment…", false);
            assert_eq!(
                app.workbench
                    .configuration_draft
                    .as_ref()
                    .unwrap()
                    .1
                    .creation_presets
                    .as_ref()
                    .unwrap()
                    .mass_height_mm,
                7200,
                "opening settings carries later direct input"
            );
            named(&ctx, &mut app, "Review environment file", false);
            assert!(
                app.workbench
                    .configuration_error
                    .contains("Save or cancel the current environment edits"),
                "{}",
                app.workbench.configuration_error
            );
            assert_eq!(app.host.creation_defaults().0, 7.2);
            named(&ctx, &mut app, "Cancel environment edits", false);
            mode(&ctx, &mut app, "Concept defaults QA");
            assert_eq!(app.host.creation_defaults().0, 6.85);
            named(&ctx, &mut app, "Concept mass", false);
            frames(&ctx, &mut app);
            let view = app.workbench.panel_rects["view.canvas"];
            let a = app.screen(view, Point::new(30., 30.));
            let b = app.screen(view, Point::new(34., 33.));
            layers_click(&ctx, &mut app, a);
            assert!(app.host.document.editor.anchor.is_some());
            layers_frame_output(&ctx, &mut app, vec![Event::PointerMoved(b)]);
            assert_eq!(app.host.document.editor.project.elements, original.elements);
            layers_frame_output(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
            named(&ctx, &mut app, "Concept mass", false);
            frames(&ctx, &mut app);
            let view = app.workbench.panel_rects["view.canvas"];
            layers_click(&ctx, &mut app, app_screen(view, Point::new(30., 30.)));
            layers_click(&ctx, &mut app, app_screen(view, Point::new(34., 33.)));
            let mass = app.host.document.editor.project.elements.last().unwrap();
            assert!(
                matches!(&mass.shape,Shape::ConceptMass{spec} if (spec.height-6.85).abs()<1e-10)
            );
            assert!((mass.area() - 12.).abs() < 1e-7);
            let mass_id = mass.id;
            named(&ctx, &mut app, "Zone", false);
            frames(&ctx, &mut app);
            let view = app.workbench.panel_rects["view.canvas"];
            layers_click(&ctx, &mut app, app_screen(view, Point::new(30., 34.)));
            layers_click(&ctx, &mut app, app_screen(view, Point::new(34., 36.)));
            let zone = app.host.document.editor.project.elements.last().unwrap();
            assert!(
                matches!(&zone.shape,Shape::Zone{spec} if (spec.height-3.25).abs()<1e-10 && spec.purpose=="Concept teaching (not approved)")
            );
            assert!((zone.area() - 8.).abs() < 1e-7);
            let zone_id = zone.id;
            assert_eq!(
                app.host.document.editor.project.elements.len(),
                original.elements.len() + 2
            );
            for old in &original.elements {
                assert_eq!(
                    app.host
                        .document
                        .editor
                        .project
                        .elements
                        .iter()
                        .find(|e| e.id == old.id),
                    Some(old)
                );
            }
            let created = app.host.document.editor.project.clone();
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert!(
                app.host
                    .document
                    .editor
                    .project
                    .elements
                    .iter()
                    .all(|e| e.id != zone_id)
            );
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            assert_eq!(app.host.document.editor.project, created);
            assert!(created.elements.iter().any(|e| e.id == mass_id));
            let output = config.join("created.openarchi.json");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let old = app.file_dialog_path.clone().unwrap();
            replace(&ctx, &mut app, &old, &output.display().to_string());
            named(&ctx, &mut app, "Save", false);
            assert!(app.file_error.is_empty(), "{}", app.file_error);
            let (fresh_ctx, mut fresh) = setup();
            fresh_ctx.enable_accesskit();
            fresh.language = language;
            fresh.initialize_work_mode(&config);
            open(&fresh_ctx, &mut fresh, &output);
            assert_eq!(
                normalized(
                    fresh.host.document.editor.project.clone(),
                    output.parent().unwrap()
                ),
                normalized(created, input.parent().unwrap())
            );
            assert_eq!(fresh.workbench.active_profile, id);
            assert_eq!(
                fresh.host.creation_defaults(),
                (6.85, 3.25, "Concept teaching (not approved)".into())
            );
            assert_eq!(std::fs::read(&input).unwrap(), original_bytes);
        }
        // Same screen transform as the normal test viewport, kept independent
        // of the mutable app borrow needed by pointer events.
        fn app_screen(view: Rect, p: Point) -> Pos2 {
            pos2(
                view.center().x + (p.x - 32.) as f32 * 40.,
                view.center().y - (p.y - 32.) as f32 * 40.,
            )
        }
    }
}
