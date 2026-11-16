// Reuses normal product controls and the shared rule, links and Diagram command.
#[cfg(unix)]
mod linked_zone_function_product_path {
    use super::super::mass_space_product_path as space;
    use super::*;
    use crate::{design_process::parametric, diagrams, mass_spaces};

    fn frames(ctx: &Context, app: &mut AtelierApp) {
        for _ in 0..6 {
            layers_frame_output(ctx, app, vec![]);
        }
    }
    fn choose(ctx: &Context, app: &mut AtelierApp, id: u64) {
        frames(ctx, app);
        let label = app
            .mass_space_canvas_selection()
            .map(|e| format!("#{} · {}", e.id, e.name))
            .unwrap_or_else(|| app.language.translate("Select a source mass").into());
        button(ctx, app, &label);
        let element = app
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .find(|e| e.id == id)
            .unwrap();
        button(ctx, app, &format!("#{id} · {}", element.name));
        assert_eq!(app.host.document.editor.selected, Some(id));
    }
    fn figure(app: &AtelierApp, id: u64) -> diagrams::Diagram {
        app.host
            .document
            .editor
            .project
            .drawings
            .diagrams
            .iter()
            .find(|f| f.id == id)
            .unwrap()
            .clone()
    }
    // Independent local-coordinate shoelace, not the figure's quantity function.
    fn zone_area(element: &Element) -> f64 {
        let Shape::Zone { spec } = &element.shape else {
            panic!("Zone expected")
        };
        let ring = |points: &[Point]| {
            let o = points[0];
            points
                .iter()
                .zip(points.iter().cycle().skip(1))
                .take(points.len())
                .map(|(a, b)| (a.x - o.x) * (b.y - o.y) - (b.x - o.x) * (a.y - o.y))
                .sum::<f64>()
                .abs()
                / 2.
        };
        ring(&spec.outline) - spec.holes.iter().map(|h| ring(h)).sum::<f64>()
    }
    #[test]
    fn comparison_opens_current_candidate_check_without_scrolling_old_checks_bilingual() {
        current_candidate_check_focus(true);
    }
    #[test]
    fn workbench_opens_current_candidate_check_without_scrolling_old_checks_bilingual() {
        current_candidate_check_focus(false);
    }
    fn current_candidate_check_focus(comparison_entry: bool) {
        use crate::command::ModelCommand;
        use crate::constraints::{Basis, Definition, Domain, Edit, Metric, Relation, Scope};
        let (mut author, _, _) = object_nearby_product_path::fixture();
        let task = author.project.design_process.task().unwrap().clone();
        // Several older-scope checks make the real list scroll. They remain
        // available, but are not the object of the current comparison action.
        for (id, title, scope) in (0..4)
            .map(|i| {
                (
                    format!("old-check-{i}"),
                    format!("Earlier study check {i}"),
                    Scope::Candidate {
                        story: 0,
                        option: Some(task.baseline_option),
                    },
                )
            })
            .chain([
                (
                    "current-a".into(),
                    "Current A check".into(),
                    Scope::Candidate {
                        story: 0,
                        option: Some(task.candidates[0].option),
                    },
                ),
                (
                    "current-b".into(),
                    "Current B object check".into(),
                    Scope::Objects {
                        ids: task.candidates[1].initial_mass_ids.clone(),
                    },
                ),
            ])
        {
            author
                .apply(ModelCommand::Constraint {
                    edit: Edit::Put {
                        expected_sha256: None,
                        definition: Definition {
                            id,
                            title,
                            domain: Domain::ConceptMasses,
                            scope,
                            metric: Metric::Height,
                            relation: Relation::AtMost,
                            limit: Some(10.),
                            basis: Basis::DesignAssumption {
                                note: "Public test height assumption, not approval".into(),
                            },
                        },
                    },
                })
                .unwrap();
        }
        let root = std::env::temp_dir().join(format!(
            "oa-candidate-check-focus-{}",
            storage::new_document_identity()
        ));
        std::fs::create_dir(&root).unwrap();
        let input = root.join("current.openarchi.json");
        storage::save_document(&author.project, &input, author.project_identity()).unwrap();
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            for (index, title) in [(0, "Current A check"), (1, "Current B object check")] {
                let (ctx, mut app) = setup();
                ctx.enable_accesskit();
                ctx.global_style_mut(|style| {
                    style.animation_time = 0.;
                    style.scroll_animation = egui::style::ScrollAnimation::none();
                });
                app.language = language;
                open(&ctx, &mut app, &input);
                if index == 1 {
                    button(&ctx, &mut app, &task.candidates[0].name);
                    button(&ctx, &mut app, &task.candidates[1].name);
                }
                let original = app.host.document.editor.project.clone();
                let context = app.host.context();
                let selection = app.host.document.editor.selection_ids();
                if comparison_entry {
                    button(&ctx, &mut app, "Compare options…");
                    button(&ctx, &mut app, "Inspect candidate checks");
                } else {
                    button(&ctx, &mut app, "Check current design");
                }
                frames(&ctx, &mut app);
                let out = layers_frame_output(&ctx, &mut app, vec![]);
                let (clip, text) = out
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == title => {
                            Some((shape.clip_rect, text))
                        }
                        _ => None,
                    })
                    .expect("current check title is painted by the normal window");
                let rect = Rect::from_min_size(text.pos, text.galley.size());
                assert!(
                    clip.contains_rect(rect),
                    "{language:?}: current candidate check must be visible without scrolling past old checks: {rect:?} / {clip:?}"
                );
                assert!(app.checks_ui.open && !app.process_ui.open);
                assert_eq!(app.host.context(), context);
                assert_eq!(app.host.document.editor.project, original);
                assert_eq!(app.host.document.editor.selection_ids(), selection);
            }
        }
    }
    #[test]
    fn mass_task_linked_zone_function_figure_normal_update_protection_reopen_bilingual() {
        path(false);
    }
    #[test]
    #[ignore = "latest protected saved project and unused output required"]
    fn mass_task_linked_zone_function_figure_protected_bilingual() {
        path(true);
    }
    fn path(protected: bool) {
        let root = if protected {
            PathBuf::from(std::env::var_os("OPENARCHI_LINKED_FUNCTION_OUTPUT").unwrap())
        } else {
            std::env::temp_dir().join(format!(
                "oa-linked-functions-{}",
                storage::new_document_identity()
            ))
        };
        std::fs::create_dir(&root)
            .expect("unused output; inspect original operations before retry");
        let input = if protected {
            PathBuf::from(std::env::var_os("OPENARCHI_LINKED_FUNCTION_INPUT").unwrap())
        } else {
            let (mut editor, _, _) = object_nearby_product_path::fixture();
            let mass =
                editor.project.design_process.task().unwrap().candidates[0].initial_mass_ids[1];
            let source = editor
                .project
                .elements
                .iter()
                .find(|e| e.id == mass)
                .unwrap();
            editor
                .apply(crate::command::ModelCommand::MassSpaces {
                    edit: mass_spaces::Edit::Create {
                        mass,
                        expected_source_sha256: mass_spaces::element_stamp(&editor.project, source)
                            .unwrap(),
                        name: "Shared teaching concept".into(),
                        purpose: "Teaching exchange; concept use, approval unknown".into(),
                        height: 3.,
                        assumption: "Public fixture, not measured or AI execution".into(),
                    },
                })
                .unwrap();
            let path = root.join("public-fixture.openarchi.json");
            storage::save_document(&editor.project, &path, editor.project_identity()).unwrap();
            path
        };
        let input_bytes = std::fs::read(&input).unwrap();
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let output = root.join(if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            });
            std::fs::create_dir(&output).unwrap();
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            app.host
                .configure_operation_journal(Some(output.join("journal")))
                .unwrap();
            open(&ctx, &mut app, &input);
            let original = app.host.document.editor.project.clone();
            let task = original.design_process.task().unwrap().clone();
            assert!(task.is_mass());
            let option = original.plan_options.active_design_option.unwrap();
            let links: Vec<_> = original
                .design_process
                .mass_spaces
                .iter()
                .filter(|l| l.option == Some(option))
                .cloned()
                .collect();
            let link = links.last().unwrap();
            choose(&ctx, &mut app, link.mass);
            let out = layers_frame_output(&ctx, &mut app, vec![]);
            let mut labels = vec![];
            collect_text(&out.shapes, &mut labels);
            assert!(
                !labels
                    .iter()
                    .any(|s| s == language.translate("Preview function diagram")),
                "a mass is not a functional Zone"
            );
            button(&ctx, &mut app, "Select linked space");
            assert_eq!(app.host.document.editor.selected, Some(link.zone));
            reveal_manual_control(&ctx, &mut app, "Preview function diagram");
            let out = layers_frame_output(&ctx, &mut app, vec![]);
            let rect = app.drawings_ui.rects["figure.function.task"];
            assert!(
                out.shapes.iter().any(|s| s.clip_rect.contains_rect(rect)),
                "function action must be visible without moving context to another task"
            );
            // Must not require a new Zone task or invoke an internal create helper.
            button(&ctx, &mut app, "Preview function diagram");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, original);
            button(&ctx, &mut app, "Preview function diagram");
            button(&ctx, &mut app, "Confirm design change");
            let created = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .last()
                .unwrap()
                .clone();
            assert!(created.definition.presentation.is_functional());
            assert_eq!(created.definition.domain, diagrams::Domain::Zones);
            assert_eq!(created.definition.option, Some(option));
            assert!(
                created
                    .snapshot
                    .elements
                    .iter()
                    .all(|e| matches!(e.shape, Shape::Zone { .. }))
            );
            let area: f64 = original
                .elements
                .iter()
                .filter(|e| {
                    e.story == task.story
                        && e.design_option == Some(option)
                        && matches!(e.shape, Shape::Zone { .. })
                })
                .map(zone_area)
                .sum();
            assert!((created.total().0 - area).abs() < 1e-6);
            assert_eq!(
                created.status(&app.host.document.editor.project),
                diagrams::Status::Current
            );
            assert_eq!(
                app.host.document.editor.project.design_process,
                original.design_process
            );
            assert_eq!(app.host.document.editor.project.elements, original.elements);
            button(&ctx, &mut app, "Edit source model");
            let old = parametric::current_parameters(&original, &task.id, option).unwrap();
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
                        (old.court_depth_mm + 500) as f64,
                    );
                    break;
                }
                task_scroll(&ctx, &mut app, -8.);
            }
            task_visible_button(&ctx, &mut app, "Preview two-wing parameters");
            button(&ctx, &mut app, "Confirm design change");
            assert_eq!(
                figure(&app, created.id).status(&app.host.document.editor.project),
                diagrams::Status::Stale
            );
            button(&ctx, &mut app, "Show function diagram");
            let before_refused = app.host.document.editor.project.clone();
            button(&ctx, &mut app, "Preview figure update");
            assert!(app.process_ui.preview.is_none());
            assert!(
                app.process_ui.error.contains("Update or detach"),
                "{}",
                app.process_ui.error
            );
            assert_eq!(app.host.document.editor.project, before_refused);
            button(&ctx, &mut app, "Edit source model");
            for link in &links {
                choose(&ctx, &mut app, link.mass);
                space::click(&ctx, &mut app, "Preview space update");
                button(&ctx, &mut app, "Confirm design change");
            }
            // The existing refresh confirmation already focuses its resulting Zone.
            assert_eq!(app.host.document.editor.selected, Some(link.zone));
            button(&ctx, &mut app, "Show function diagram");
            button(&ctx, &mut app, "Preview figure update");
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(figure(&app, created.id), created);
            button(&ctx, &mut app, "Preview figure update");
            button(&ctx, &mut app, "Confirm design change");
            let updated = figure(&app, created.id);
            assert_eq!(updated.generation, created.generation + 1);
            assert_eq!(
                updated.status(&app.host.document.editor.project),
                diagrams::Status::Current
            );
            let after_area: f64 = updated.snapshot.elements.iter().map(zone_area).sum();
            assert!((updated.total().0 - after_area).abs() < 1e-6);
            assert!((after_area - area - old.wing_depth_mm as f64 / 1000. * 0.5).abs() < 1e-6);
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(figure(&app, created.id), created);
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            assert_eq!(figure(&app, created.id), updated);
            button(&ctx, &mut app, "Edit source model");
            choose(&ctx, &mut app, link.mass);
            button(&ctx, &mut app, "Select linked space");
            button(&ctx, &mut app, "Properties");
            space::number(&ctx, &mut app, "Clear height", "2.9");
            space::click(&ctx, &mut app, "Apply changes");
            let user = app.host.document.editor.project.clone();
            assert_eq!(
                figure(&app, created.id).status(&user),
                diagrams::Status::Stale
            );
            space::click(&ctx, &mut app, "Preview space update");
            assert!(app.process_ui.preview.is_none());
            assert_eq!(app.host.document.editor.project, user);
            button(&ctx, &mut app, "Show function diagram");
            button(&ctx, &mut app, "Preview figure update");
            assert!(app.process_ui.preview.is_none());
            assert_eq!(app.host.document.editor.project, user);
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            let saved_state = app.host.document.editor.project.clone();
            assert_eq!(
                figure(&app, created.id).status(&saved_state),
                diagrams::Status::Current
            );
            for element in &original.elements {
                if !task.candidates[0].initial_mass_ids.contains(&element.id)
                    && !links.iter().any(|l| l.zone == element.id)
                {
                    assert_eq!(
                        saved_state.elements.iter().find(|e| e.id == element.id),
                        Some(element)
                    );
                }
            }
            assert_eq!(saved_state.evidence, original.evidence);
            assert_eq!(saved_state.program, original.program);
            assert_eq!(saved_state.site, original.site);
            let saved = output.join("linked-functions.openarchi.json");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let old_path = app.file_dialog_path.clone().unwrap();
            field(&ctx, &mut app, &old_path, &saved.display().to_string());
            button(&ctx, &mut app, "Save");
            assert!(saved.exists(), "{}", app.file_error);
            let (fresh_ctx, mut fresh) = setup();
            fresh_ctx.enable_accesskit();
            fresh.language = language;
            open(&fresh_ctx, &mut fresh, &saved);
            let mut roundtrip = fresh.host.document.editor.project.clone();
            for (before, after) in saved_state
                .drawings
                .pages
                .iter()
                .zip(&mut roundtrip.drawings.pages)
            {
                assert_eq!(
                    std::fs::read(&before.path).unwrap(),
                    std::fs::read(&after.path).unwrap()
                );
                after.path = before.path.clone();
            }
            assert_eq!(roundtrip, saved_state);
            assert!(!fresh.host.operations.borrow().enabled());
            assert_eq!(
                parametric::current_parameters(&roundtrip, &task.id, option)
                    .unwrap()
                    .court_depth_mm,
                old.court_depth_mm + 500
            );
            choose(&fresh_ctx, &mut fresh, link.mass);
            button(&fresh_ctx, &mut fresh, "Select linked space");
            button(&fresh_ctx, &mut fresh, "Show function diagram");
            assert_eq!(figure(&fresh, created.id), updated);
            std::fs::write(output.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"input":input,"saved":saved,"task":task.id,"option":option,"figure":created.id,"generation":updated.generation,"area_before":area,"area_after":after_area,"no_second_task":true,"manual_protection":true,"source_unchanged":true,"actual_AI":false,"new_host_readonly":true})).unwrap()).unwrap();
        }
        assert_eq!(std::fs::read(&input).unwrap(), input_bytes);
    }
}

// Reads the actual painted paths, not a test-only scale or transform.
#[cfg(unix)]
mod comparison_scale_product_path {
    use super::*;
    fn paths(out: &egui::FullOutput) -> Vec<Vec<Pos2>> {
        out.shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Path(p) if (p.stroke.width - 1.4).abs() < 0.001 => {
                    Some(p.points.clone())
                }
                _ => None,
            })
            .collect()
    }
    fn comparison() -> crate::design_process::Comparison {
        let (editor, _, _) = object_nearby_product_path::fixture();
        let mut c = editor
            .project
            .design_process
            .task()
            .unwrap()
            .compare(&editor.project)
            .unwrap();
        // Renderer-only archived snapshot with a real coordinate offset. This
        // is not a newly accepted design, nor a bypass around model commands.
        for e in &mut c.candidates[1].elements {
            e.translate(Point::new(12., 5.));
        }
        c.candidates[1].name = "较长的中文候选名称 · 保留院落交流与教学空间".repeat(3);
        c
    }
    fn paint(
        c: &crate::design_process::Comparison,
        width: f64,
        language: UiLanguage,
    ) -> egui::FullOutput {
        let ctx = Context::default();
        AtelierApp::style(&ctx);
        let mut out = None;
        for _ in 0..2 {
            let mut frame = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 1600.))),
                    ..Default::default()
                },
                |ui| {
                    AtelierApp::design_comparison_plans(ui, language, c);
                },
            );
            frame.textures_delta.clear();
            out = Some(frame);
        }
        out.unwrap()
    }
    #[test]
    fn candidate_comparison_painted_world_scale_and_offsets_match_at_narrow_widths() {
        let c = comparison();
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            for width in [120., 260., 660.] {
                let out = paint(&c, width, language);
                let paths = paths(&out);
                let model = c
                    .candidates
                    .iter()
                    .flat_map(|v| &v.elements)
                    .collect::<Vec<_>>();
                assert_eq!(paths.len(), model.len());
                let mut common_scale: Option<f64> = None;
                let mut common_x: Option<f64> = None;
                let mut origins = vec![];
                for (path, e) in paths.iter().zip(&model) {
                    let (outline, _) = e.authored_prism_rings().unwrap();
                    assert_eq!(path.len(), outline.len() + 1);
                    let world_length =
                        (outline[1].x - outline[0].x).hypot(outline[1].y - outline[0].y);
                    let scale = (path[1] - path[0]).length() as f64 / world_length;
                    if let Some(expected) = common_scale {
                        assert!((scale - expected).abs() < 0.00002);
                    } else {
                        common_scale = Some(scale);
                    }
                    let x = path[0].x as f64 - outline[0].x * scale;
                    if let Some(expected) = common_x {
                        assert!((x - expected).abs() < 0.002);
                    } else {
                        common_x = Some(x);
                    }
                    origins.push((path[0], outline[0]));
                }
                // Independent model points: first wing starts at x30, B at x42.
                assert!(
                    (origins[2].0.x - origins[0].0.x - 12. * common_scale.unwrap() as f32).abs()
                        < 0.001
                );
                assert!(origins[2].0.x > origins[0].0.x);
                let scale_labels = out
                    .shapes
                    .iter()
                    .filter_map(|s| match &s.shape {
                        egui::Shape::Text(t)
                            if t.galley.text().ends_with(" m")
                                && !t.galley.text().contains("X ") =>
                        {
                            Some(t.galley.text())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(scale_labels.len(), 2);
                assert_eq!(scale_labels[0], scale_labels[1]);
                for path in paths {
                    assert!(path.iter().all(|p| p.x.is_finite()
                        && p.y.is_finite()
                        && p.x >= 0.
                        && p.x <= width as f32));
                }
            }
        }
    }
    #[test]
    fn candidate_comparison_empty_snapshot_is_explicit_and_single_preview_stays_unscaled() {
        let mut c = comparison();
        c.candidates[1].elements.clear();
        let out = paint(&c, 260., UiLanguage::English);
        assert!(out.shapes.iter().any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="No supported footprint in this candidate")));
        c.candidates[0].elements.clear();
        let out = paint(&c, 260., UiLanguage::SimplifiedChinese);
        assert!(paths(&out).is_empty());
        assert!(out.shapes.iter().any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="这些候选中没有受支持的平面轮廓")));
        let c = comparison();
        let ctx = Context::default();
        let mut out = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(660., 400.))),
                ..Default::default()
            },
            |ui| {
                AtelierApp::design_snapshot_plan(ui, &c.candidates[0].elements);
            },
        );
        out.textures_delta.clear();
        let p = paths(&out);
        assert_eq!(p.len(), 2);
        let min_y = p
            .iter()
            .flatten()
            .map(|p| p.y)
            .fold(f32::INFINITY, f32::min);
        let max_y = p
            .iter()
            .flatten()
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            (max_y - min_y - 119.).abs() < 0.002,
            "original 135px single preview"
        );
    }
    #[test]
    fn comparison_normal_entry_and_archived_choice_use_shared_scale_bilingual_without_model_writes()
    {
        use crate::design_process::{Entry, service};
        let (mut editor, _, _) = object_nearby_product_path::fixture();
        let task = editor.project.design_process.task().unwrap().clone();
        let c = task.compare(&editor.project).unwrap();
        let reason = "Public saved choice snapshot, not client adoption";
        let choice_id = storage::new_document_identity();
        let change = if c.basis_current {
            service::prepare_choose(
                &editor,
                &choice_id,
                &task.id,
                c.candidates[0].option,
                reason.into(),
                c.fingerprint,
                Entry::ManualInterface,
                "Public fixture author".into(),
            )
            .unwrap()
        } else {
            service::prepare_reviewed_choice(
                &editor,
                &choice_id,
                &task.id,
                c.candidates[0].option,
                reason.into(),
                c.fingerprint.clone(),
                c.fingerprint,
            )
            .unwrap()
        };
        editor
            .commit_change("public-choice-fixture", &change)
            .unwrap();
        let root = std::env::temp_dir().join(format!(
            "oa-comparison-scale-{}",
            storage::new_document_identity()
        ));
        std::fs::create_dir(&root).unwrap();
        let input = root.join("saved.openarchi.json");
        storage::save_document(&editor.project, &input, editor.project_identity()).unwrap();
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| {
                s.animation_time = 0.;
                s.scroll_animation = egui::style::ScrollAnimation::none();
            });
            app.language = language;
            open(&ctx, &mut app, &input);
            let before = app.host.document.editor.project.clone();
            let context = app.host.context();
            let selection = app.host.document.editor.selection_ids();
            button(&ctx, &mut app, "Compare options…");
            button(&ctx, &mut app, "Inspect candidate footprints");
            let out = layers_frame_output(&ctx, &mut app, vec![]);
            let label =
                language.translate("Same scale and model coordinates · read-only plan outlines");
            assert!(
                out.shapes
                    .iter()
                    .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()==label))
            );
            button(&ctx, &mut app, "Choice history and saved versions");
            button(
                &ctx,
                &mut app,
                &format!("{} · {reason}", language.translate("Current choice")),
            );
            // Close the current pair, then scroll the normal window to the
            // archived pair. Offscreen text is intentionally not painted.
            button(&ctx, &mut app, "Inspect candidate footprints");
            let mut visible_archive = false;
            for _ in 0..12 {
                let out = layers_frame_output(
                    &ctx,
                    &mut app,
                    vec![
                        Event::PointerMoved(pos2(350., 500.)),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            phase: egui::TouchPhase::Move,
                            delta: vec2(0., -150.),
                            modifiers: Modifiers::NONE,
                        },
                    ],
                );
                visible_archive |= out.shapes.iter().any(|s| matches!(&s.shape,
                    egui::Shape::Text(t) if t.galley.text()==label && s.clip_rect.contains_rect(Rect::from_min_size(t.pos,t.galley.size()))));
            }
            assert!(
                visible_archive,
                "archived comparison scale explanation must be visible in the normal scroll path"
            );
            button(&ctx, &mut app, "Continue in canvas");
            assert!(!app.process_ui.open);
            assert_eq!(app.host.document.editor.project, before);
            assert_eq!(app.host.context(), context);
            assert_eq!(app.host.document.editor.selection_ids(), selection);
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

mod comparison_3d_product_path {
    use super::*;
    fn pair(app: &AtelierApp) -> crate::design_process::Comparison {
        app.host
            .document
            .editor
            .project
            .design_process
            .task()
            .unwrap()
            .compare(&app.host.document.editor.project)
            .unwrap()
    }
    fn frame(
        ctx: &Context,
        app: &mut AtelierApp,
        c: &crate::design_process::Comparison,
        width: f64,
        events: Vec<Event>,
    ) -> egui::FullOutput {
        let modifiers = events
            .iter()
            .find_map(|e| match e {
                Event::PointerButton { modifiers, .. } | Event::MouseWheel { modifiers, .. } => {
                    Some(*modifiers)
                }
                _ => None,
            })
            .unwrap_or_else(|| ctx.input(|i| i.modifiers));
        let mut events = events;
        events.insert(0, Event::ModifiersChanged(modifiers));
        let mut out = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 1000.))),
                events,
                ..Default::default()
            },
            |ui| app.design_comparison_3d(ui, c),
        );
        out.textures_delta.clear();
        out
    }
    fn views(ctx: &Context) -> (Vec<Rect>, Camera) {
        ctx.data(|d| {
            d.get_temp::<(Vec<Rect>, Camera)>(Id::new("comparison_3d_views"))
                .unwrap()
        })
    }
    fn camera(c: &Camera) -> (f64, f64, f64, glam::DVec3) {
        (c.yaw, c.pitch, c.span, c.target)
    }
    fn drag(
        ctx: &Context,
        app: &mut AtelierApp,
        c: &crate::design_process::Comparison,
        index: usize,
        button: PointerButton,
        shift: bool,
    ) {
        let p = views(ctx).0[index].center();
        let modifiers = Modifiers {
            shift,
            ..Modifiers::NONE
        };
        // Establish the hover position before the press; pre-gesture travel is not a drag.
        frame(ctx, app, c, 900., vec![Event::PointerMoved(p)]);
        frame(
            ctx,
            app,
            c,
            900.,
            vec![
                Event::PointerMoved(p),
                Event::PointerButton {
                    pos: p,
                    button,
                    pressed: true,
                    modifiers,
                },
            ],
        );
        frame(
            ctx,
            app,
            c,
            900.,
            vec![Event::PointerMoved(p + vec2(28., 17.))],
        );
        frame(
            ctx,
            app,
            c,
            900.,
            vec![Event::PointerMoved(p + vec2(50., 29.))],
        );
        frame(
            ctx,
            app,
            c,
            900.,
            vec![Event::PointerButton {
                pos: p + vec2(50., 29.),
                button,
                pressed: false,
                modifiers,
            }],
        );
    }
    #[test]
    fn candidate_3d_same_viewports_shared_navigation_and_no_model_or_canvas_writes_bilingual() {
        for lang in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            let (editor, _, _) = object_nearby_product_path::fixture();
            app.host.document.editor = editor;
            app.language = lang;
            let mut c = pair(&app);
            c.candidates[1].name = "很长的方案名 / A very long candidate title ".repeat(8);
            let project = app.host.document.editor.project.clone();
            let selection = app.host.document.editor.selection_ids();
            let context = app.host.context();
            let main = camera(&app.camera);
            frame(&ctx, &mut app, &c, 900., vec![]);
            let initial = views(&ctx).1;
            for width in [900., 260., 120.] {
                frame(&ctx, &mut app, &c, width, vec![]);
                let out = frame(&ctx, &mut app, &c, width, vec![]);
                let (rects, fitted) = views(&ctx);
                assert_eq!(rects.len(), 2);
                assert_eq!(rects[0].size(), rects[1].size());
                assert!(
                    rects
                        .iter()
                        .all(|r| r.min.x >= 0. && r.max.x <= width as f32 + 0.01)
                );
                assert_eq!(
                    fitted.matrix((rects[0].width() / rects[0].height()).into()),
                    fitted.matrix((rects[1].width() / rects[1].height()).into())
                );
                assert!(out.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()==lang.translate("3D GPU preview unavailable"))));
                let before = camera(&fitted);
                frame(&ctx, &mut app, &c, width, vec![]);
                assert_eq!(
                    camera(&views(&ctx).1),
                    before,
                    "idle must retain the fitted camera"
                );
            }
            frame(&ctx, &mut app, &c, 900., vec![]);
            for index in 0..2 {
                let before = camera(&views(&ctx).1);
                drag(&ctx, &mut app, &c, index, PointerButton::Primary, false);
                assert_eq!(
                    (before.0, before.1, before.3),
                    (views(&ctx).1.yaw, views(&ctx).1.pitch, views(&ctx).1.target),
                    "left drag selects without orbiting; prior wheel inertia is independent"
                );
                app.host.document.editor.set_selection(selection.clone());
                let before = views(&ctx).1.target;
                drag(&ctx, &mut app, &c, index, PointerButton::Secondary, false);
                assert_ne!(before, views(&ctx).1.target);
                let before = views(&ctx).1.target;
                drag(&ctx, &mut app, &c, index, PointerButton::Middle, false);
                assert_ne!(before, views(&ctx).1.target, "middle pan index={index}");
                let before = views(&ctx).1.yaw;
                drag(&ctx, &mut app, &c, index, PointerButton::Middle, true);
                assert_ne!(before, views(&ctx).1.yaw);

                let before = views(&ctx).1.span;
                let pos = views(&ctx).0[index].center();
                frame(
                    &ctx,
                    &mut app,
                    &c,
                    900.,
                    vec![
                        Event::PointerMoved(pos),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            phase: egui::TouchPhase::Move,
                            delta: vec2(0., -80.),
                            modifiers: Modifiers::NONE,
                        },
                    ],
                );
                assert_ne!(before, views(&ctx).1.span);
                assert_eq!(ctx.input(|i| i.smooth_scroll_delta), Vec2::ZERO);
            }

            let orientation = (views(&ctx).1.yaw, views(&ctx).1.pitch);
            let out = frame(&ctx, &mut app, &c, 900., vec![]);
            let id = out
                .platform_output
                .accesskit_update
                .unwrap()
                .nodes
                .iter()
                .find(|(_, n)| {
                    n.label() == Some(lang.translate("Fit both candidates"))
                        && n.supports_action(egui::accesskit::Action::Click)
                })
                .unwrap()
                .0;
            frame(
                &ctx,
                &mut app,
                &c,
                900.,
                vec![Event::AccessKitActionRequest(
                    egui::accesskit::ActionRequest {
                        action: egui::accesskit::Action::Click,
                        target_tree: egui::accesskit::TreeId::ROOT,
                        target_node: id,
                        data: None,
                    },
                )],
            );
            assert_eq!(
                (views(&ctx).1.target, views(&ctx).1.span),
                (initial.target, initial.span)
            );
            assert_eq!((views(&ctx).1.yaw, views(&ctx).1.pitch), orientation);
            assert_eq!(app.host.document.editor.project, project);
            assert_eq!(app.host.context(), context);
            assert_eq!(app.host.document.editor.selection_ids(), selection);
            assert_eq!(camera(&app.camera), main);
            let mut stale = c.clone();
            stale.candidates[0].elements[0].translate(Point::new(1., 0.));
            let out = frame(&ctx, &mut app, &stale, 900., vec![]);
            assert!(out.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()==lang.translate("Comparison objects changed; reopen the current comparison"))));
            stale.candidates[0].elements.clear();
            stale.candidates[1].elements.clear();
            frame(&ctx, &mut app, &stale, 900., vec![]);
            assert!(views(&ctx).1.target.is_finite());
        }
    }
    #[test]
    fn candidate_3d_normal_compare_entry_then_canvas_preserves_project_bilingual() {
        let (editor, _, _) = object_nearby_product_path::fixture();
        let root = std::env::temp_dir().join(format!("oa-3d-{}", storage::new_document_identity()));
        std::fs::create_dir(&root).unwrap();
        let input = root.join("input.openarchi.json");
        storage::save_document(&editor.project, &input, editor.project_identity()).unwrap();
        for lang in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| {
                s.animation_time = 0.;
                s.scroll_animation = egui::style::ScrollAnimation::none();
            });
            app.language = lang;
            open(&ctx, &mut app, &input);
            let before = app.host.document.editor.project.clone();
            let main = camera(&app.camera);
            let context = app.host.context();
            button(&ctx, &mut app, "Compare options…");
            button(&ctx, &mut app, "Compare candidate volumes in 3D");
            let mut painted = false;
            for _ in 0..10 {
                let out = layers_frame_output(
                    &ctx,
                    &mut app,
                    vec![
                        Event::PointerMoved(pos2(350., 500.)),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            phase: egui::TouchPhase::Move,
                            delta: vec2(0., -120.),
                            modifiers: Modifiers::NONE,
                        },
                    ],
                );
                painted |= out.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()==lang.translate("3D GPU preview unavailable") && s.clip_rect.intersects(Rect::from_min_size(t.pos,t.galley.size()))));
            }
            assert!(
                painted,
                "normal comparison scroll path must reach the explicit no-GPU view"
            );
            let (rects, _) = views(&ctx);
            assert_eq!(rects[0].size(), rects[1].size());
            for rect in &rects {
                let p = rect.center();
                assert!(
                    Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.)).contains(p),
                    "actual comparison must be onscreen: {rect:?}"
                );
                layers_frame_output(&ctx, &mut app, vec![Event::PointerMoved(p)]);
                let yaw = views(&ctx).1.yaw;
                layers_frame_output(
                    &ctx,
                    &mut app,
                    vec![Event::PointerButton {
                        pos: p,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: Modifiers::NONE,
                    }],
                );
                layers_frame_output(
                    &ctx,
                    &mut app,
                    vec![Event::PointerMoved(p + vec2(45., 20.))],
                );
                layers_frame_output(
                    &ctx,
                    &mut app,
                    vec![Event::PointerMoved(p + vec2(80., 35.))],
                );
                layers_frame_output(
                    &ctx,
                    &mut app,
                    vec![Event::PointerButton {
                        pos: p + vec2(80., 35.),
                        button: PointerButton::Primary,
                        pressed: false,
                        modifiers: Modifiers::NONE,
                    }],
                );
                assert_eq!(
                    views(&ctx).1.yaw,
                    yaw,
                    "normal comparison left drag selects and keeps the camera fixed"
                );
            }

            button(&ctx, &mut app, "Continue in canvas");
            assert!(!app.process_ui.open);
            assert_eq!(app.host.document.editor.project, before);
            assert_eq!(app.host.context(), context);
            assert_eq!(camera(&app.camera), main);
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn candidate_selection_uses_each_displayed_option_and_shared_state() {
        let (ctx, mut app) = setup();
        let (editor, _, _) = object_nearby_product_path::fixture();
        app.host.document.editor = editor;
        let c = pair(&app);
        let before = app.host.document.editor.project.clone();
        let history = app.host.document.editor.undo.len();
        frame(&ctx, &mut app, &c, 900., vec![]);
        for index in 0..2 {
            app.host.document.editor.set_selection(vec![]);
            let (rects, camera_before) = views(&ctx);
            let a = rects[index].min + vec2(1., 1.);
            let b = rects[index].max - vec2(1., 1.);
            frame(&ctx, &mut app, &c, 900., vec![Event::PointerMoved(a)]);
            frame(
                &ctx,
                &mut app,
                &c,
                900.,
                vec![Event::PointerButton {
                    pos: a,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                }],
            );
            frame(&ctx, &mut app, &c, 900., vec![Event::PointerMoved(b)]);
            assert!(app.canvas_selection_box.is_some());
            frame(
                &ctx,
                &mut app,
                &c,
                900.,
                vec![Event::PointerButton {
                    pos: b,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                }],
            );
            let mut expected = c.candidates[index]
                .elements
                .iter()
                .map(|e| e.id)
                .collect::<Vec<_>>();
            expected.sort_unstable();
            assert_eq!(
                app.host.document.editor.selection_ids(),
                expected,
                "candidate index {index}"
            );
            assert_eq!(camera(&views(&ctx).1), camera(&camera_before));
            assert_eq!(app.host.document.editor.project, before);
            assert_eq!(app.host.document.editor.undo.len(), history);
        }
    }
}
