#[cfg(unix)]
mod reviewed_input_product_path {
    use super::super::project_conditions_product_path as conditions;
    use super::*;
    use crate::design_process::parametric;
    #[test]
    fn normal_condition_change_review_parameter_update_history_and_reopen_bilingual() {
        path(false);
    }
    #[test]
    #[ignore = "protected project input and new output required; never modifies original"]
    fn protected_grain_requirement_review_update_reopen_bilingual() {
        path(true);
    }
    fn path(supplied: bool) {
        let parent = if supplied {
            std::path::PathBuf::from(
                std::env::var_os("OPENARCHI_REVIEW_OUTPUT").expect("new output"),
            )
        } else {
            std::env::temp_dir().join(format!("oa-review-ui-{}", storage::new_document_identity()))
        };
        std::fs::create_dir(&parent).expect("inspect existing result; never replay");
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let editor = if supplied {
                Editor::from_stored_document(std::path::Path::new(
                    &std::env::var_os("OPENARCHI_REVIEW_INPUT").expect("protected source"),
                ))
                .unwrap()
            } else {
                object_nearby_product_path::fixture().0
            };
            let root = parent.join(if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            });
            std::fs::create_dir(&root).unwrap();
            let input = root.join("protected.openarchi.json");
            storage::save_document(&editor.project, &input, editor.project_identity()).unwrap();
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            open(&ctx, &mut app, &input);
            let original = app.host.document.editor.project.clone();
            let task = original.design_process.task().unwrap().clone();
            let option = task.candidates[0].option;
            conditions::press(&ctx, &mut app, "Project conditions");
            if supplied {
                conditions::press(&ctx, &mut app, "Requirements and constraints");
                let sheet = original
                    .program
                    .sheets
                    .iter()
                    .find(|s| s.id == "original-taskbook-functions")
                    .unwrap();
                conditions::press(
                    &ctx,
                    &mut app,
                    &format!("{} ({})", sheet.name, sheet.rows.len()),
                );
                conditions::press(&ctx, &mut app, "Edit requirement");
                let draft = app.program_ui.draft.as_ref().expect("real row edit button");
                assert_eq!(draft.key, "教学培训");
                let note = draft.values.note.clone();
                field(
                    &ctx,
                    &mut app,
                    &note,
                    "开发验收概念假设：当前A教学翼进深6.4m，其余条件、候选与保留对象不变；未核测量或审批。",
                );
                conditions::press(&ctx, &mut app, "Stage condition change");
            } else {
                assert!(original.site.control_tolerance_m.is_none());
                conditions::press(&ctx, &mut app, "Site and orientation");
                conditions::tolerance(&ctx, &mut app);
            }
            conditions::press(&ctx, &mut app, "Preview all condition changes");
            conditions::press(&ctx, &mut app, "Save project conditions");
            assert!(
                app.project_conditions_ui.error.is_empty(),
                "{}",
                app.project_conditions_ui.error
            );
            conditions::press(&ctx, &mut app, "Continue designing");
            let changed = app.host.document.editor.project.clone();
            if supplied {
                assert_eq!(changed.site, original.site);
                assert_ne!(changed.program, original.program);
            } else {
                assert_eq!(changed.site.control_tolerance_m, Some(0.02));
            }
            assert_eq!(changed.elements, original.elements);
            button(&ctx, &mut app, "AI Assistant");
            task_visible_button(&ctx, &mut app, "Adjust generated wings");
            assert!(
                app.research_ui
                    .parameters
                    .as_ref()
                    .unwrap()
                    .input_review
                    .as_ref()
                    .is_some_and(|(_, reviewed)| !reviewed)
            );
            task_visible_button(&ctx, &mut app, "Preview two-wing parameters");
            assert!(app.process_ui.preview.is_none());
            assert!(
                app.process_ui
                    .error
                    .contains("Review changed design inputs")
            );
            assert_eq!(app.host.document.editor.project, changed);
            task_visible_button(
                &ctx,
                &mut app,
                "I reviewed these changed inputs; update only this candidate",
            );
            number(&ctx, &mut app, 0, "Wing depth (mm)", 6400.);
            task_visible_button(&ctx, &mut app, "Preview two-wing parameters");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            assert_eq!(app.host.document.editor.project, changed);
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, changed);
            task_visible_button(&ctx, &mut app, "Preview two-wing parameters");
            button(&ctx, &mut app, "Confirm design change");
            assert!(app.process_ui.preview.is_none(), "{}", app.process_ui.error);
            let after = app.host.document.editor.project.clone();
            let inputs = parametric::current_inputs(&after, &task.id, option).unwrap();
            assert_eq!(
                parametric::current_parameters(&after, &task.id, option)
                    .unwrap()
                    .wing_depth_mm,
                6400
            );
            assert!(inputs.check_current(&after).is_ok());
            assert_eq!(after.design_process.tasks, changed.design_process.tasks);
            assert_eq!(after.evidence, changed.evidence);
            assert!(
                parametric::current_inputs(&after, &task.id, task.candidates[1].option)
                    .unwrap()
                    .check_current(&after)
                    .is_err()
            );
            for old in &changed.elements {
                if !task.candidates[0].initial_mass_ids.contains(&old.id) {
                    assert_eq!(after.elements.iter().find(|e| e.id == old.id), Some(old));
                }
            }
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(app.host.document.editor.project.elements, changed.elements);
            assert!(
                parametric::current_inputs(&app.host.document.editor.project, &task.id, option)
                    .unwrap()
                    .check_current(&app.host.document.editor.project)
                    .is_err()
            );
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            assert_eq!(
                parametric::current_inputs(&app.host.document.editor.project, &task.id, option)
                    .unwrap(),
                inputs
            );
            let output = root.join("reviewed.openarchi.json");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let old = app.file_dialog_path.clone().unwrap();
            field(&ctx, &mut app, &old, &output.display().to_string());
            assert_eq!(app.file_dialog_path.as_deref(), output.to_str());
            assert!(!output.exists());
            button(&ctx, &mut app, "Save");
            assert!(output.exists(), "{}", app.file_error);
            let (fresh_ctx, mut fresh) = setup();
            fresh_ctx.enable_accesskit();
            fresh_ctx.global_style_mut(|s| s.animation_time = 0.);
            fresh.language = language;
            open(&fresh_ctx, &mut fresh, &output);
            assert_eq!(fresh.host.document.editor.project.elements, after.elements);
            assert_eq!(
                parametric::current_inputs(&fresh.host.document.editor.project, &task.id, option)
                    .unwrap(),
                inputs
            );
            assert!(!fresh.host.operations.borrow().enabled());
            button(&fresh_ctx, &mut fresh, "AI Assistant");
            task_visible_button(&fresh_ctx, &mut fresh, "Adjust generated wings");
            assert!(
                fresh
                    .research_ui
                    .parameters
                    .as_ref()
                    .unwrap()
                    .input_review
                    .is_none()
            );
            task_visible_button(&fresh_ctx, &mut fresh, "Cancel parameter draft");
            std::fs::write(root.join("result.json"), serde_json::to_vec_pretty(&serde_json::json!({
                "input": input, "output": output, "task":task.id,"option":option,
                "entry":"normal bilingual egui file open/conditions/parameters/Undo/Redo/SaveAs/newHostOpen",
                "input_review":"explicit exact snapshot; other candidate remains stale",
                "protected_other_elements":true,"old_tasks_and_evidence_exact":true,
                "native_OS":"not_run","actual_AI":"not_run","source_original_written":false
            })).unwrap()).unwrap();
        }
    }
}
