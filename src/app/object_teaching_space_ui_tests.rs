// Normal product controls over a public fixture, or an explicitly supplied protected copy.
#[cfg(unix)]
mod object_teaching_space_product_path {
    use super::super::mass_space_product_path as space;
    use super::*;
    use crate::{design_process::parametric, mass_spaces};

    fn frames(ctx: &Context, app: &mut AtelierApp) {
        for _ in 0..8 {
            layers_frame_output(ctx, app, vec![]);
        }
    }
    fn choose(ctx: &Context, app: &mut AtelierApp, id: u64) {
        frames(ctx, app);
        let current = app
            .mass_space_canvas_selection()
            .map(|e| format!("#{} · {}", e.id, e.name))
            .unwrap_or_else(|| app.language.translate("Select a source mass").into());
        button(ctx, app, &current);
        let name = &app
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .find(|e| e.id == id)
            .unwrap()
            .name;
        button(ctx, app, &format!("#{id} · {name}"));
        assert_eq!(app.host.document.editor.selected, Some(id));
    }
    fn named_create(ctx: &Context, app: &mut AtelierApp, name: &str, purpose: &str) {
        space::click(ctx, app, "Space name and use");
        space::identity_field(ctx, app, "name", name);
        space::identity_field(ctx, app, "purpose", purpose);
        space::click(ctx, app, "Space name and use");
        space::click(ctx, app, "Create editable space");
        assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
    }
    fn area(app: &AtelierApp, id: u64) -> f64 {
        match &app
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .find(|e| e.id == id)
            .unwrap()
            .shape
        {
            Shape::Zone { spec } => spec.area(),
            Shape::ConceptMass { spec } => spec.area(),
            _ => panic!("expected actual space or mass"),
        }
    }
    #[test]
    fn generated_wing_named_space_refresh_user_protection_reopen_bilingual() {
        path(false);
    }
    #[test]
    #[ignore = "latest protected project and unused output directory required"]
    fn protected_generated_wing_teaching_space_reopen_bilingual() {
        path(true);
    }

    fn path(supplied: bool) {
        let root = if supplied {
            PathBuf::from(std::env::var_os("OPENARCHI_TEACHING_SPACE_OUTPUT").unwrap())
        } else {
            std::env::temp_dir().join(format!(
                "oa-teaching-space-{}",
                storage::new_document_identity()
            ))
        };
        std::fs::create_dir(&root)
            .expect("unused output: inspect original operation results before retry");
        let input = if supplied {
            PathBuf::from(std::env::var_os("OPENARCHI_TEACHING_SPACE_INPUT").unwrap())
        } else {
            let file = root.join("public-fixture.openarchi.json");
            let editor = object_nearby_product_path::fixture().0;
            storage::save_document(&editor.project, &file, editor.project_identity()).unwrap();
            file
        };
        let original_bytes = std::fs::read(&input).unwrap();
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let language_root = root.join(if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            });
            std::fs::create_dir(&language_root).unwrap();
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            app.host
                .configure_operation_journal(Some(language_root.join("journal")))
                .unwrap();
            open(&ctx, &mut app, &input);
            let original = app.host.document.editor.project.clone();
            let task = original.design_process.task().unwrap().clone();
            let option = original.plan_options.active_design_option.unwrap();
            let candidate = task.candidates.iter().find(|c| c.option == option).unwrap();
            let mass = candidate.initial_mass_ids[1];
            let initial_parameters =
                parametric::current_parameters(&original, &task.id, option).unwrap();
            choose(&ctx, &mut app, mass);
            let source_area = area(&app, mass);
            // Blank identity is optional: the ordinary create path remains immediately usable.
            named_create(&ctx, &mut app, "", "");
            button(&ctx, &mut app, "Confirm design change");
            let blank = app
                .host
                .document
                .editor
                .project
                .design_process
                .mass_spaces
                .last()
                .unwrap();
            let z = app
                .host
                .document
                .editor
                .project
                .elements
                .iter()
                .find(|e| e.id == blank.zone)
                .unwrap();
            assert_eq!(
                z.name,
                format!("{} #{mass}", language.translate("Concept space"))
            );
            assert!(
                matches!(&z.shape, Shape::Zone{spec} if spec.purpose==language.translate("Unknown concept use"))
            );
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(app.host.document.editor.project, original);
            choose(&ctx, &mut app, mass);
            named_create(
                &ctx,
                &mut app,
                "Shared teaching concept",
                "Teaching exchange; concept use, approval unknown",
            );
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, original);
            assert!(app.mass_spaces_ui.dirty);
            space::click(&ctx, &mut app, "Create editable space");
            button(&ctx, &mut app, "Confirm design change");
            assert!(app.process_ui.preview.is_none(), "{}", app.process_ui.error);
            let created = app.host.document.editor.project.clone();
            let link = created.design_process.mass_spaces.last().unwrap().clone();
            assert_eq!(link.mass, mass);
            assert!((area(&app, link.zone) - source_area).abs() < 1e-6);
            assert!(
                matches!(&created.elements.iter().find(|e|e.id==link.zone).unwrap().shape, Shape::Zone{spec} if spec.height==3.)
            );
            assert_eq!(created.elements.len(), original.elements.len() + 1);
            for old in &original.elements {
                assert_eq!(created.elements.iter().find(|e| e.id == old.id), Some(old));
            }

            // Existing candidate-bar parameters, rather than a test-only mutation or second model.
            button(&ctx, &mut app, "Adjust generated wings");
            for _ in 0..100 {
                frames(&ctx, &mut app);
                let id = ctx
                    .data(|d| d.get_temp::<Id>(Id::new(("parametric", 0usize, "Court depth (mm)"))))
                    .unwrap();
                let rect = ctx.read_response(id).unwrap().rect;
                let body = ctx
                    .data(|d| d.get_temp::<Rect>(Id::new("inspector_scroll_rect")))
                    .unwrap();
                if body.contains_rect(rect) {
                    number(
                        &ctx,
                        &mut app,
                        0,
                        "Court depth (mm)",
                        (initial_parameters.court_depth_mm + 500) as f64,
                    );
                    break;
                }
                task_scroll(&ctx, &mut app, -8.);
            }
            assert_eq!(
                app.research_ui
                    .parameters
                    .as_ref()
                    .unwrap()
                    .definition
                    .candidates[0]
                    .court_depth_mm,
                initial_parameters.court_depth_mm + 500
            );
            task_visible_button(&ctx, &mut app, "Preview two-wing parameters");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            button(&ctx, &mut app, "Confirm design change");
            let changed = app.host.document.editor.project.clone();
            assert_eq!(
                mass_spaces::status(&changed, &link),
                "Mass-space source changed"
            );
            let delta = initial_parameters.wing_depth_mm as f64 / 1000. * 0.5;
            assert!((area(&app, mass) - source_area - delta).abs() < 1e-6);
            assert!((area(&app, link.zone) - source_area).abs() < 1e-6);
            choose(&ctx, &mut app, mass);
            space::click(&ctx, &mut app, "Preview space update");
            button(&ctx, &mut app, "Confirm design change");
            let refreshed = app.host.document.editor.project.clone();
            let fresh = refreshed.design_process.mass_spaces.last().unwrap().clone();
            assert_eq!(
                mass_spaces::status(&refreshed, &fresh),
                "Mass-space link is current"
            );
            assert!((area(&app, fresh.zone) - source_area - delta).abs() < 1e-6);
            let zone = refreshed
                .elements
                .iter()
                .find(|e| e.id == fresh.zone)
                .unwrap();
            assert_eq!(zone.name, "Shared teaching concept");
            assert!(
                matches!(&zone.shape,Shape::Zone{spec} if spec.purpose=="Teaching exchange; concept use, approval unknown" && spec.height==3.)
            );
            for old in &original.elements {
                if !candidate.initial_mass_ids.contains(&old.id) {
                    assert_eq!(
                        refreshed.elements.iter().find(|e| e.id == old.id),
                        Some(old)
                    );
                }
            }
            assert_eq!(refreshed.evidence, original.evidence);
            assert_eq!(refreshed.site, original.site);
            // Deliberate user modification: regeneration refuses, and never repairs the user history.
            button(&ctx, &mut app, "Properties");
            space::number(&ctx, &mut app, "Clear height", "2.9");
            space::click(&ctx, &mut app, "Apply changes");
            let user = app.host.document.editor.project.clone();
            assert_eq!(
                mass_spaces::status(&user, &fresh),
                "Linked space has user edits"
            );
            space::click(&ctx, &mut app, "Preview space update");
            assert!(app.process_ui.preview.is_none());
            assert_eq!(app.host.document.editor.project, user);
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(app.host.document.editor.project, refreshed);
            let output = language_root.join("teaching-space.openarchi.json");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let old = app.file_dialog_path.clone().unwrap();
            field(&ctx, &mut app, &old, &output.display().to_string());
            button(&ctx, &mut app, "Save");
            assert!(output.exists(), "{}", app.file_error);
            let (new_ctx, mut reopened) = setup();
            new_ctx.enable_accesskit();
            reopened.language = language;
            open(&new_ctx, &mut reopened, &output);
            assert!(!reopened.host.operations.borrow().enabled());
            assert_eq!(
                reopened.host.document.editor.project.elements,
                refreshed.elements
            );
            assert_eq!(
                reopened.host.document.editor.project.design_process,
                refreshed.design_process
            );
            assert_eq!(
                reopened.host.document.editor.project.evidence,
                refreshed.evidence
            );
            assert_eq!(
                mass_spaces::status(&reopened.host.document.editor.project, &fresh),
                "Mass-space link is current"
            );
            button(&new_ctx, &mut reopened, "Adjust generated wings");
            frames(&new_ctx, &mut reopened);
            assert_eq!(
                reopened
                    .research_ui
                    .parameters
                    .as_ref()
                    .unwrap()
                    .definition
                    .candidates[0]
                    .court_depth_mm,
                initial_parameters.court_depth_mm + 500
            );
            button(&new_ctx, &mut reopened, "Cancel parameter draft");
            std::fs::write(language_root.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"entry":"normal Open/candidate canvas/parameter/space controls/SaveAs/new host","actual_AI":false,"input":input,"output":output,"mass":mass,"zone":fresh.zone,"source_area_before":source_area,"space_area_after":area(&reopened,fresh.zone),"explicit_assumed_height":3.,"user_edit_refusal":true,"unchanged_original_objects_except_candidate":true,"blank_identity_accepted":true,"original_input_unchanged":true})).unwrap()).unwrap();
        }
        assert_eq!(std::fs::read(&input).unwrap(), original_bytes);
    }
}
