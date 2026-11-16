#[cfg(unix)]
mod editing_shortcuts_product_path {
    use super::report_environment_product_path::{frames, named, open, replace};
    use super::*;
    use crate::workbench::{
        WorkbenchCommand,
        shortcuts::{self, Chord},
    };

    fn painted(ctx: &Context, app: &mut AtelierApp, label: &str) {
        frames(ctx, app);
        let out = layers_frame_output(ctx, app, vec![]);
        let label = app.language.translate(label);
        let at = out
            .shapes
            .iter()
            .rev()
            .find_map(|s| {
                text_center(std::slice::from_ref(s), label).filter(|p| s.clip_rect.contains(*p))
            })
            .unwrap_or_else(|| panic!("visible editing-key control {label}"));
        layers_click(ctx, app, at);
    }
    fn stroke(ctx: &Context, app: &mut AtelierApp, k: Key, mods: Modifiers) {
        layers_frame_output(ctx, app, vec![key(k, mods)]);
        frames(ctx, app);
    }
    #[test]
    fn editing_shortcuts_each_adapter_uses_original_handler_and_guards() {
        for command in [
            WorkbenchCommand::Drag,
            WorkbenchCommand::Rotate,
            WorkbenchCommand::Mirror,
            WorkbenchCommand::Elevate,
            WorkbenchCommand::Resize,
            WorkbenchCommand::Multiply,
            WorkbenchCommand::ToolSettings,
        ] {
            let (ctx, mut keyed) = setup();
            let (_, mut original) = setup();
            for app in [&mut keyed, &mut original] {
                shortcut_test_wall(app);
            }
            let dir = std::env::temp_dir().join(format!(
                "oa-edit-adapter-{}",
                storage::new_document_identity()
            ));
            std::fs::create_dir(&dir).unwrap();
            keyed.initialize_work_mode(&dir);
            let mut config = keyed
                .host
                .environment
                .as_ref()
                .unwrap()
                .configuration
                .clone();
            let mut profile = config.resolve(&config.active).unwrap();
            shortcuts::assign(
                &mut profile.shortcuts,
                command.id(),
                Some(vec![Chord::new("F8", false, false, false)]),
            )
            .unwrap();
            config.put(profile.layer());
            keyed
                .host
                .environment
                .as_mut()
                .unwrap()
                .save(config)
                .unwrap();
            let before = keyed.host.document.editor.project.clone();
            edit_shortcut_frame(&ctx, &mut keyed, Key::F8, Modifiers::NONE);
            original.dispatch_edit(command.label());
            assert_eq!(keyed.edit_dialog, original.edit_dialog, "{}", command.id());
            assert_eq!(
                keyed.element_settings,
                original.element_settings,
                "{}",
                command.id()
            );
            assert_eq!(
                keyed.pointer_drag.is_some(),
                original.pointer_drag.is_some()
            );
            assert_eq!(keyed.host.document.editor.project, before);
            assert_eq!(keyed.host.document.editor.undo.len(), 0);
            assert!(!command.remote_invocation_supported());
            assert!(keyed.edit_menu_label(command.label()).contains("F8"));
        }
    }
    #[test]
    fn editing_shortcuts_personal_drag_respects_text_modals_repeat_and_active_gesture() {
        let (ctx, mut app) = setup();
        shortcut_test_wall(&mut app);
        let dir = std::env::temp_dir().join(format!(
            "oa-edit-guards-{}",
            storage::new_document_identity()
        ));
        std::fs::create_dir(&dir).unwrap();
        app.initialize_work_mode(&dir);
        let mut config = app.host.environment.as_ref().unwrap().configuration.clone();
        let mut profile = config.resolve(&config.active).unwrap();
        shortcuts::assign(
            &mut profile.shortcuts,
            "edit.drag",
            Some(vec![Chord::new("F8", false, false, false)]),
        )
        .unwrap();
        config.put(profile.layer());
        app.host.environment.as_mut().unwrap().save(config).unwrap();
        let before = app.host.document.editor.project.clone();
        app.command_dialog = Some("guard precondition".into());
        edit_shortcut_frame(&ctx, &mut app, Key::F8, Modifiers::NONE);
        assert!(app.pointer_drag.is_none());
        app.command_dialog = None;
        let mut text = String::from("User text");
        let field = Id::new("personal_edit_text_guard");
        let mut output = ctx.run_ui(RawInput::default(), |ui| {
            ui.add(TextEdit::singleline(&mut text).id(field))
                .request_focus();
        });
        output.textures_delta.clear();
        let mut output = ctx.run_ui(
            RawInput {
                events: vec![key(Key::F8, Modifiers::NONE)],
                ..Default::default()
            },
            |ui| {
                app.shortcuts(ui.ctx());
                ui.add(TextEdit::singleline(&mut text).id(field));
            },
        );
        output.textures_delta.clear();
        assert!(app.pointer_drag.is_none());
        assert_eq!(text, "User text");
        ctx.memory_mut(|m| m.surrender_focus(field));
        let mut output = ctx.run_ui(
            RawInput {
                focused: false,
                events: vec![key(Key::F8, Modifiers::NONE)],
                ..Default::default()
            },
            |ui| {
                app.shortcuts(ui.ctx());
            },
        );
        output.textures_delta.clear();
        assert!(app.pointer_drag.is_none());
        let repeat = Event::Key {
            key: Key::F8,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers: Modifiers::NONE,
        };
        plan_frame(&ctx, &mut app, vec![repeat]);
        assert!(app.pointer_drag.is_none());
        edit_shortcut_frame(&ctx, &mut app, Key::W, Modifiers::NONE);
        click(&ctx, &mut app, pos2(400., 300.));
        assert!(app.host.document.editor.anchor.is_some());
        edit_shortcut_frame(&ctx, &mut app, Key::F8, Modifiers::NONE);
        assert!(app.pointer_drag.is_none());
        assert!(app.host.document.editor.anchor.is_some());
        assert_eq!(app.host.document.editor.project, before);
        assert!(app.host.document.editor.undo.is_empty());
    }
    #[test]
    fn editing_shortcuts_normal_settings_canvas_cancel_undo_save_reopen_bilingual() {
        let root =
            std::env::temp_dir().join(format!("oa-edit-keys-{}", storage::new_document_identity()));
        std::fs::create_dir(&root).unwrap();
        let input = if let Some(p) = std::env::var_os("OPENARCHI_ENV_REVIEW_PROJECT") {
            std::path::PathBuf::from(p)
        } else {
            let input = root.join("input.openarchi.json");
            let author = Editor::new(storage::startup_example());
            storage::save_document(&author.project, &input, author.project_identity()).unwrap();
            input
        };
        let input_bytes = std::fs::read(&input).unwrap();
        eprintln!("EDIT_KEYS_ROOT={}", root.display());
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let dir = root.join(if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            });
            std::fs::create_dir(&dir).unwrap();
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| {
                s.animation_time = 0.;
                s.scroll_animation = egui::style::ScrollAnimation::none();
            });
            app.language = language;
            app.initialize_work_mode(&dir);
            open(&ctx, &mut app, &input);
            let original = app.host.document.editor.project.clone();
            let context = app.host.context();
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
            replace(&ctx, &mut app, &old, "My model keys");
            painted(&ctx, &mut app, "Keyboard shortcuts");
            painted(&ctx, &mut app, "Command name or ID");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![
                    Event::Text("edit.drag".into()),
                    key(Key::Tab, Modifiers::NONE),
                ],
            );
            assert_eq!(app.workbench.shortcut_filter, "edit.drag");
            let record = format!(
                "{} · {}",
                language.translate("Record shortcut"),
                language.translate("Drag")
            );
            painted(&ctx, &mut app, &record);
            stroke(&ctx, &mut app, Key::F6, Modifiers::NONE);
            assert_eq!(
                shortcuts::keys(
                    &app.workbench
                        .configuration_draft
                        .as_ref()
                        .unwrap()
                        .1
                        .shortcuts,
                    "edit.drag"
                ),
                vec![Chord::new("F6", false, false, false)]
            );
            named(&ctx, &mut app, "Cancel environment edits", false);
            assert!(!dir.join("work-environment.json").exists());
            assert_eq!(app.host.context(), context);
            assert_eq!(app.host.document.editor.project, original);
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
            replace(&ctx, &mut app, &old, "My model keys");
            // The existing filter and expanded section are retained, not re-entered from another window.
            for (id, label, k) in [
                ("edit.drag", "Drag", Key::F6),
                ("edit.elevate", "Elevate…", Key::F9),
            ] {
                let old = app.workbench.shortcut_filter.clone();
                replace(&ctx, &mut app, &old, id);
                let record = format!(
                    "{} · {}",
                    language.translate("Record shortcut"),
                    language.translate(label)
                );
                painted(&ctx, &mut app, &record);
                stroke(&ctx, &mut app, k, Modifiers::NONE);
            }
            named(&ctx, &mut app, "Save and activate mode", false);
            named(&ctx, &mut app, "Cancel environment edits", false);
            assert_eq!(app.host.document.editor.project, original);
            let config_bytes = std::fs::read(dir.join("work-environment.json")).unwrap();
            let personal = app.workbench.active_profile.clone();
            named(&ctx, &mut app, "2D Plan", false);
            // Actual plan hit-testing selects an existing object; no selection/model assignment.
            plan_frame(
                &ctx,
                &mut app,
                vec![key(Key::V, Modifiers::NONE), key(Key::F, Modifiers::NONE)],
            );
            let viewport = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
            let active = app
                .host
                .document
                .editor
                .project
                .plan_options
                .active_design_option;
            let require_candidate = app
                .host
                .document
                .editor
                .project
                .design_process
                .task()
                .is_some_and(|task| task.candidates.iter().any(|c| Some(c.option) == active));
            let candidates = app
                .host
                .document
                .editor
                .project
                .elements
                .iter()
                .filter(|e| {
                    e.story == app.host.document.editor.story
                        && (!require_candidate || e.design_option == active)
                        && app
                            .host
                            .document
                            .editor
                            .project
                            .plan_options
                            .element_visible(e, &app.host.document.editor.project.layers)
                })
                .map(|e| geometry::element_footprint(&app.host.document.editor.project, e))
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>();
            for polygon in candidates {
                let n = polygon.len() as f64;
                let p = polygon.iter().fold(Point::default(), |a, b| {
                    Point::new(a.x + b.x / n, a.y + b.y / n)
                });
                let at = app.screen(viewport, p);
                if viewport.shrink(30.).contains(at) {
                    click(&ctx, &mut app, at);
                    let ids = app.host.document.editor.selection_ids();
                    eprintln!(
                        "EDIT_KEYS_PICK point={p:?} screen={at:?} center={:?} scale={} ids={ids:?} active={active:?} candidate_required={require_candidate} status={}",
                        app.plan_center, app.plan_scale, app.host.document.editor.status
                    );
                    if ids.len() == 1
                        && app.host.document.editor.project.elements.iter().any(|e| {
                            e.id == ids[0]
                                && (!require_candidate || e.design_option == active)
                                && !matches!(e.shape, Shape::Opening { .. })
                                && !app
                                    .host
                                    .document
                                    .editor
                                    .project
                                    .edit_state
                                    .locked
                                    .contains(&e.id)
                                && !app.host.document.editor.project.layers.is_locked(e.layer.0)
                        })
                    {
                        break;
                    }
                }
            }
            let ids = app.host.document.editor.selection_ids();
            assert_eq!(ids.len(), 1, "normal canvas selection");
            let selected = ids[0];
            assert!(
                !matches!(
                    app.host
                        .document
                        .editor
                        .project
                        .elements
                        .iter()
                        .find(|e| e.id == selected)
                        .unwrap()
                        .shape,
                    Shape::Opening { .. }
                ),
                "finite task needs an independent object, not a hosted opening"
            );
            let selected_before = app
                .host
                .document
                .editor
                .project
                .elements
                .iter()
                .find(|e| e.id == selected)
                .unwrap()
                .clone();
            edit_shortcut_frame(&ctx, &mut app, Key::D, Modifiers::COMMAND);
            assert!(
                app.pointer_drag.is_none(),
                "replaced old drag must not trigger"
            );
            edit_shortcut_frame(&ctx, &mut app, Key::F6, Modifiers::NONE);
            assert!(
                app.pointer_drag.is_some(),
                "first personal Drag status={} ids={:?} tool={:?} gated={} interaction={} params={} selected={:?}",
                app.host.document.editor.status,
                app.host.document.editor.selection_ids(),
                app.host.document.editor.tool,
                app.menu_gated(),
                app.workbench_pointer_active(),
                app.pending_design_parameters(),
                selected_before
            );
            plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(440., 330.))]);
            assert_eq!(app.host.document.editor.project, original);
            edit_shortcut_frame(&ctx, &mut app, Key::Escape, Modifiers::NONE);
            assert!(app.pointer_drag.is_none());
            assert_eq!(app.host.document.editor.project, original);
            edit_shortcut_frame(&ctx, &mut app, Key::F9, Modifiers::NONE);
            assert!(app.edit_dialog.is_some());
            // Normal numeric dialog input and preview remain the original edit implementation.
            frames(&ctx, &mut app);
            named(&ctx, &mut app, "Cancel", false);
            assert!(app.edit_dialog.is_none());
            assert_eq!(app.host.document.editor.project, original);
            // Independent normal pointer commit after explicit drag reference and destination.
            edit_shortcut_frame(&ctx, &mut app, Key::F6, Modifiers::NONE);
            assert!(
                app.pointer_drag.is_some(),
                "drag must start after numeric Cancel: {} selection={:?}",
                app.host.document.editor.status,
                app.host.document.editor.selection_ids()
            );
            let revision_before_drag = app.host.document.editor.revision;
            click(&ctx, &mut app, pos2(400., 300.));
            assert!(
                app.pointer_drag.is_some()
                    && app
                        .host
                        .document
                        .editor
                        .status
                        .starts_with("Enter Drag-to Point"),
                "actual reference press must be accepted: {}",
                app.host.document.editor.status
            );
            plan_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(420., 300.))]);
            click(&ctx, &mut app, pos2(420., 300.));
            assert!(app.pointer_drag.is_none());
            assert_ne!(
                app.host
                    .document
                    .editor
                    .project
                    .elements
                    .iter()
                    .find(|e| e.id == selected)
                    .unwrap(),
                &selected_before,
                "real commit status={} revision_before={} now={} selected={:?} scale={}",
                app.host.document.editor.status,
                revision_before_drag,
                app.host.document.editor.revision,
                app.host.document.editor.selection_ids(),
                app.plan_scale
            );
            let edited = app.host.document.editor.project.clone();
            edit_shortcut_frame(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
            assert_eq!(app.host.document.editor.project.elements, original.elements);
            edit_shortcut_frame(
                &ctx,
                &mut app,
                Key::Z,
                Modifiers::COMMAND | Modifiers::SHIFT,
            );
            assert_eq!(app.host.document.editor.project.elements, edited.elements);
            let output = dir.join("saved.openarchi.json");
            stroke(
                &ctx,
                &mut app,
                Key::S,
                Modifiers::COMMAND | Modifiers::SHIFT,
            );
            let previous = app.file_dialog_path.clone().unwrap();
            replace(&ctx, &mut app, &previous, &output.display().to_string());
            named(&ctx, &mut app, "Save", false);
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let read = storage::load_document(&output).unwrap();
            assert_eq!(read.0.elements, edited.elements);
            let (rectx, mut reopened) = setup();
            rectx.enable_accesskit();
            reopened.language = language;
            reopened.initialize_work_mode(&dir);
            open(&rectx, &mut reopened, &output);
            assert_eq!(reopened.workbench.active_profile, personal);
            assert_eq!(
                reopened.host.document.editor.project.elements,
                edited.elements
            );
            assert_eq!(
                std::fs::read(dir.join("work-environment.json")).unwrap(),
                config_bytes
            );
            assert!(reopened.edit_menu_label("Drag").contains("F6"));
            assert_eq!(std::fs::read(&input).unwrap(), input_bytes);
            let expected = serde_json::json!({"selected":selected,"before":selected_before,"after":edited.elements.iter().find(|e|e.id==selected).unwrap(),"input":input,"output":output,"config":dir.join("work-environment.json"),"expected_offset_m":[20./app.plan_scale,0.],"current_candidate_required":require_candidate,"manual_qa_only":true});
            std::fs::write(
                dir.join("expected.json"),
                serde_json::to_vec_pretty(&expected).unwrap(),
            )
            .unwrap();
        }
    }
}
