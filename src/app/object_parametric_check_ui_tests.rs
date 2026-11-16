#[cfg(unix)]
mod proposed_parameter_check_product_path {
    use super::*;
    use crate::constraints::{
        self, Basis, Definition, Domain, Edit, Metric, Relation, Scope, State,
    };
    #[test]
    fn normal_parameter_preview_keeps_background_navigation_frozen_bilingual() {
        let root = std::env::temp_dir().join(format!(
            "oa-preview-navigation-{}",
            storage::new_document_identity()
        ));
        std::fs::create_dir(&root).unwrap();
        let (editor, _, _) = object_nearby_product_path::fixture();
        let task_id = editor.project.design_process.task().unwrap().id.clone();
        let input = root.join("input.openarchi.json");
        storage::save_document(&editor.project, &input, editor.project_identity()).unwrap();
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            for plan in [true, false] {
                let (ctx, mut app) = setup();
                ctx.enable_accesskit();
                ctx.global_style_mut(|s| s.animation_time = 0.);
                app.language = language;
                open(&ctx, &mut app, &input);
                layers_frame_output(
                    &ctx,
                    &mut app,
                    vec![key(if plan { Key::F2 } else { Key::F3 }, Modifiers::NONE)],
                );
                button(&ctx, &mut app, "Adjust generated wings");
                let option = app
                    .host
                    .document
                    .editor
                    .project
                    .plan_options
                    .active_design_option
                    .unwrap();
                let original = crate::design_process::parametric::current_parameters(
                    &app.host.document.editor.project,
                    &task_id,
                    option,
                )
                .unwrap();
                number(
                    &ctx,
                    &mut app,
                    0,
                    "Origin X (mm)",
                    (original.origin_mm[0] + 1000) as f64,
                );
                task_visible_button(&ctx, &mut app, "Preview two-wing parameters");
                assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
                let project = app.host.document.editor.project.clone();
                let at = app.workbench.panel_rects["view.canvas"].left_bottom() + vec2(10., -10.);
                layers_frame_output(&ctx, &mut app, vec![Event::PointerMoved(at)]);
                let navigation = (
                    app.plan_scale,
                    app.plan_center,
                    app.camera.target,
                    app.camera.yaw,
                    app.camera.pitch,
                    app.camera.span,
                );
                for _ in 0..3 {
                    layers_frame_output(
                        &ctx,
                        &mut app,
                        vec![Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            phase: egui::TouchPhase::Move,
                            delta: vec2(0., -100.),
                            modifiers: Modifiers::NONE,
                        }],
                    );
                }
                assert_eq!(
                    (
                        app.plan_scale,
                        app.plan_center,
                        app.camera.target,
                        app.camera.yaw,
                        app.camera.pitch,
                        app.camera.span
                    ),
                    navigation,
                    "active proposal must isolate underlying navigation ({language:?}, plan={plan})"
                );
                assert_eq!(app.host.document.editor.project, project);
                button(&ctx, &mut app, "Cancel design proposal");
                assert!(app.process_ui.preview.is_none());
                assert_eq!(app.host.document.editor.project, project);
                for _ in 0..4 {
                    layers_frame_output(&ctx, &mut app, vec![]);
                }
                assert_eq!(
                    (
                        app.plan_scale,
                        app.plan_center,
                        app.camera.target,
                        app.camera.yaw,
                        app.camera.pitch,
                        app.camera.span
                    ),
                    navigation,
                    "cancel must not release residual preview scroll into the canvas ({language:?}, plan={plan})"
                );
                layers_frame_output(
                    &ctx,
                    &mut app,
                    vec![Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        delta: vec2(0., -100.),
                        modifiers: Modifiers::NONE,
                    }],
                );
                assert_ne!(
                    (
                        app.plan_scale,
                        app.plan_center,
                        app.camera.target,
                        app.camera.yaw,
                        app.camera.pitch,
                        app.camera.span
                    ),
                    navigation,
                    "fresh canvas wheel must still navigate ({language:?}, plan={plan})"
                );
                assert_eq!(app.host.document.editor.project, project);
            }
        }
    }
    #[test]
    fn common_context_gap_updates_in_normal_parameter_preview_without_editing_common_bilingual() {
        let root = std::env::temp_dir().join(format!(
            "oa-common-gap-ui-{}",
            storage::new_document_identity()
        ));
        std::fs::create_dir(&root).unwrap();
        // Public authored fixture preparation only. The task starts below at normal Open.
        let (mut editor, _, member) = object_nearby_product_path::fixture();
        let common = editor.project.add(
            0,
            Material::Concrete,
            Shape::ConceptMass {
                spec: crate::concept_mass::ConceptMassSpec {
                    arcs: vec![],
                    hole_arcs: vec![],
                    outline: vec![
                        Point::new(60., 43.),
                        Point::new(64., 43.),
                        Point::new(64., 47.),
                        Point::new(60., 47.),
                    ],
                    holes: vec![],
                    height: 3.,
                    bottom_offset: 0.,
                },
            },
        );
        let d = Definition {
            id: "000-public-common-gap".into(),
            title: "Authored shared-context clearance".into(),
            domain: Domain::ConceptMasses,
            scope: Scope::Objects {
                ids: vec![common, member],
            },
            metric: Metric::HorizontalSpacing,
            relation: Relation::AtLeast,
            limit: Some(3.),
            basis: Basis::DesignAssumption {
                note: "Public concept fixture; no measured retained building or legal clearance"
                    .into(),
            },
        };
        editor
            .apply(crate::command::ModelCommand::Constraint {
                edit: Edit::Put {
                    expected_sha256: None,
                    definition: d.clone(),
                },
            })
            .unwrap();
        let stamp = constraints::expected(
            editor
                .project
                .rules
                .checks
                .iter()
                .find(|c| c.definition.id == d.id)
                .unwrap(),
        );
        editor
            .apply(crate::command::ModelCommand::Constraint {
                edit: Edit::Run {
                    id: d.id.clone(),
                    expected_sha256: stamp,
                },
            })
            .unwrap();
        let input = root.join("input.openarchi.json");
        storage::save_document(&editor.project, &input, editor.project_identity()).unwrap();
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| {
                s.animation_time = 0.;
                s.scroll_animation = egui::style::ScrollAnimation::none();
            });
            app.language = language;
            app.host
                .configure_operation_journal(Some(root.join(if language == UiLanguage::English {
                    "en-journal"
                } else {
                    "zh-journal"
                })))
                .unwrap();
            open(&ctx, &mut app, &input);
            let before = app.host.document.editor.project.clone();
            let check = before
                .rules
                .checks
                .iter()
                .find(|c| c.definition.id == d.id)
                .unwrap();
            assert_eq!(check.report.as_ref().unwrap().rows[0].value, Some(4.));
            button(&ctx, &mut app, "Adjust generated wings");
            for _ in 0..6 {
                layers_frame_output(&ctx, &mut app, vec![]);
            }
            number(&ctx, &mut app, 0, "Origin X (mm)", 32000.);
            task_visible_button(&ctx, &mut app, "Preview two-wing parameters");
            let proposed = app
                .process_ui
                .preview
                .as_ref()
                .unwrap()
                .declared_check_previews
                .iter()
                .find(|c| c.definition.id == d.id)
                .unwrap();
            assert_eq!(proposed.state, State::Fail);
            assert_eq!(proposed.rows[0].value, Some(2.));
            let out = layers_frame_output(&ctx, &mut app, vec![]);
            for key in [
                "Declared checks on proposed parameters",
                "Numerical check failed",
            ] {
                assert!(out.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text().contains(language.translate(key)) && s.clip_rect.intersects(Rect::from_min_size(t.pos,t.galley.size())))));
            }
            button(&ctx, &mut app, "Cancel design proposal");
            assert!(app.host.document.editor.project == before);
            task_visible_button(&ctx, &mut app, "Preview two-wing parameters");
            button(&ctx, &mut app, "Confirm design change");
            let after = app.host.document.editor.project.clone();
            assert_eq!(after.rules, before.rules);
            assert_eq!(
                after
                    .rules
                    .checks
                    .iter()
                    .find(|c| c.definition.id == d.id)
                    .unwrap()
                    .freshness(&after),
                "Check result is out of date"
            );
            for e in &before.elements {
                if !before.design_process.task().unwrap().candidates[0]
                    .initial_mass_ids
                    .contains(&e.id)
                {
                    assert_eq!(after.elements.iter().find(|n| n.id == e.id), Some(e));
                }
            }
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            let mut undone = before.clone();
            undone.design_process.updates = after.design_process.updates.clone();
            assert!(app.host.document.editor.project == undone);
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            assert!(app.host.document.editor.project == after);
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let old = app.file_dialog_path.clone().unwrap();
            let output = root.join(if language == UiLanguage::English {
                "en.openarchi.json"
            } else {
                "zh.openarchi.json"
            });
            field(&ctx, &mut app, &old, &output.display().to_string());
            button(&ctx, &mut app, "Save");
            let (ctx2, mut fresh) = setup();
            ctx2.enable_accesskit();
            fresh.language = language;
            open(&ctx2, &mut fresh, &output);
            assert!(fresh.host.document.editor.project == after);
            assert!(!fresh.host.operations.borrow().enabled());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn parameter_proposal_shows_current_declared_conflict_cancel_commit_undo_save_bilingual() {
        let root = std::env::temp_dir().join(format!(
            "oa-param-check-ui-{}",
            storage::new_document_identity()
        ));
        std::fs::create_dir(&root).unwrap();
        let (mut editor, _, _) = object_nearby_product_path::fixture();
        let option = editor.project.plan_options.active_design_option;
        let d = Definition {
            id: "height-preview".into(),
            title: "Declared envelope height".into(),
            domain: Domain::ConceptMasses,
            scope: Scope::Candidate { story: 0, option },
            metric: Metric::Height,
            relation: Relation::AtMost,
            limit: Some(7.),
            basis: Basis::DesignAssumption {
                note: "Public early-design preference; not an approval".into(),
            },
        };
        editor
            .apply(crate::command::ModelCommand::Constraint {
                edit: Edit::Put {
                    expected_sha256: None,
                    definition: d.clone(),
                },
            })
            .unwrap();
        let h = constraints::expected(
            editor
                .project
                .rules
                .checks
                .iter()
                .find(|c| c.definition.id == "height-preview")
                .unwrap(),
        );
        editor
            .apply(crate::command::ModelCommand::Constraint {
                edit: Edit::Run {
                    id: d.id,
                    expected_sha256: h,
                },
            })
            .unwrap();
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
            app.host
                .configure_operation_journal(Some(root.join(if lang == UiLanguage::English {
                    "en-journal"
                } else {
                    "zh-journal"
                })))
                .unwrap();
            open(&ctx, &mut app, &input);
            let before = app.host.document.editor.project.clone();
            button(&ctx, &mut app, "Adjust generated wings");
            for _ in 0..6 {
                layers_frame_output(&ctx, &mut app, vec![]);
            }
            number(&ctx, &mut app, 0, "Floor height (mm)", 4000.);
            task_visible_button(&ctx, &mut app, "Preview two-wing parameters");
            let p = app.process_ui.preview.as_ref().unwrap();
            assert_eq!(p.declared_check_previews.len(), 1);
            assert_eq!(p.declared_check_previews[0].state, State::Fail);
            assert!(
                p.declared_check_previews[0]
                    .rows
                    .iter()
                    .all(|r| r.value == Some(8.))
            );
            let out = layers_frame_output(&ctx, &mut app, vec![]);
            for label in [
                "Declared checks on proposed parameters",
                "Numerical check failed",
                "Read-only numerical preview. Saved reports are unchanged; this is not a compliance verdict.",
            ] {
                assert!(out.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text().contains(lang.translate(label)) && s.clip_rect.intersects(Rect::from_min_size(t.pos,t.galley.size())))),"missing visible {label}");
            }
            assert_eq!(app.host.document.editor.project, before);
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, before);
            task_visible_button(&ctx, &mut app, "Preview two-wing parameters");
            button(&ctx, &mut app, "Confirm design change");
            let after = app.host.document.editor.project.clone();
            assert_eq!(after.rules, before.rules);
            assert_eq!(
                after
                    .rules
                    .checks
                    .iter()
                    .find(|c| c.definition.id == "height-preview")
                    .unwrap()
                    .freshness(&after),
                "Check result is out of date"
            );
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            let mut undone = before.clone();
            undone.design_process.updates = after.design_process.updates.clone();
            assert!(
                app.host.document.editor.project == undone,
                "Undo restores geometry/rules/parameters while retaining immutable exploration history"
            );
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            assert_eq!(app.host.document.editor.project, after);
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let old = app.file_dialog_path.clone().unwrap();
            let output = root.join(if lang == UiLanguage::English {
                "en.openarchi.json"
            } else {
                "zh.openarchi.json"
            });
            field(&ctx, &mut app, &old, &output.display().to_string());
            button(&ctx, &mut app, "Save");
            let (ctx2, mut fresh) = setup();
            ctx2.enable_accesskit();
            fresh.language = lang;
            open(&ctx2, &mut fresh, &output);
            assert_eq!(fresh.host.document.editor.project, after);
            assert!(!fresh.host.operations.borrow().enabled());
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "explicit protected grain input and unused evidence directory; original never modified"]
    fn protected_parameter_check_proposal_visible_before_cancel_bilingual() {
        use crate::design_process::parametric as pm;
        let path = PathBuf::from(std::env::var_os("OPENARCHI_PARAMETER_CHECK_INPUT").unwrap());
        let bytes = std::fs::read(&path).unwrap();
        let root = PathBuf::from(std::env::var_os("OPENARCHI_PARAMETER_CHECK_OUTPUT").unwrap());
        std::fs::create_dir(&root).expect("inspect old evidence; never replay");
        for lang in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            ctx.enable_accesskit();
            ctx.global_style_mut(|s| {
                s.animation_time = 0.;
                s.scroll_animation = egui::style::ScrollAnimation::none();
            });
            app.language = lang;
            open(&ctx, &mut app, &path);
            let before = app.host.document.editor.project.clone();
            let task = before.design_process.task().unwrap();
            let option = before.plan_options.active_design_option.unwrap();
            let initial = pm::current_parameters(&before, &task.id, option).unwrap();
            button(&ctx, &mut app, "Adjust generated wings");
            for _ in 0..6 {
                layers_frame_output(&ctx, &mut app, vec![]);
            }
            number(
                &ctx,
                &mut app,
                0,
                "Origin X (mm)",
                (initial.origin_mm[0] + 1_000_000) as f64,
            );
            task_visible_button(&ctx, &mut app, "Preview two-wing parameters");
            let proposal = app
                .process_ui
                .preview
                .as_ref()
                .expect("normal preview prepared");
            let check = proposal
                .declared_check_previews
                .iter()
                .find(|c| c.definition.metric == Metric::ReferenceControlRegionClearance)
                .expect("original control definition");
            assert_eq!(check.state, State::Fail);
            assert!(
                check
                    .rows
                    .iter()
                    .all(|r| r.state == State::Fail && r.value.is_none())
            );
            let region = crate::site::boundary::outline_for(
                &before,
                crate::site::boundary::Role::ControlRegion,
            )
            .unwrap()
            .unwrap();
            let max_x = region
                .iter()
                .map(|p| p[0])
                .fold(f64::NEG_INFINITY, f64::max);
            for id in proposal.change.affected_ids() {
                let e = proposal
                    .change
                    .project()
                    .elements
                    .iter()
                    .find(|e| e.id == id)
                    .unwrap();
                assert!(
                    e.authored_prism_rings()
                        .unwrap()
                        .0
                        .iter()
                        .all(|p| p.x > max_x)
                );
            }
            let out = layers_frame_output(&ctx, &mut app, vec![]);
            for key in [
                "Declared checks on proposed parameters",
                "Confirm design change",
                "Cancel design proposal",
            ] {
                assert!(out.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()==lang.translate(key)&&s.clip_rect.contains_rect(Rect::from_min_size(t.pos,t.galley.size()))&&Rect::from_min_size(Pos2::ZERO,vec2(1400.,900.)).contains_rect(Rect::from_min_size(t.pos,t.galley.size())))),"{key} must be visible without leaving the proposal");
            }
            let observations = app
                .process_ui
                .preview
                .as_ref()
                .unwrap()
                .declared_check_previews
                .clone();
            button(&ctx, &mut app, "Cancel design proposal");
            task_visible_button(&ctx, &mut app, "Cancel parameter draft");
            assert!(app.host.document.editor.project == before);
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            std::fs::write(root.join(if lang==UiLanguage::English{"en.json"}else{"zh.json"}),serde_json::to_vec_pretty(&serde_json::json!({"checks":observations,"entry":"normal Open/current-candidate parameters/proposal/cancel","visible_controls":true,"original_model_and_file_unchanged":true,"native_OS":"not_run"})).unwrap()).unwrap();
        }
    }
}

#[cfg(unix)]
mod common_context_complete_product_path {
    use super::*;
    use crate::constraints::{Metric, State};
    use crate::diagrams::{self, Presentation, Status};

    fn settle(ctx: &Context, app: &mut AtelierApp) {
        for _ in 0..8 {
            layers_frame_output(ctx, app, vec![]);
        }
    }
    fn check_input(ctx: &Context, app: &mut AtelierApp, label: &str, text: &str) {
        settle(ctx, app);
        let r = ctx
            .data(|d| d.get_temp::<Rect>(Id::new(("check_field", label))))
            .unwrap();
        layers_click(ctx, app, r.center());
        layers_frame_output(
            ctx,
            app,
            vec![key(Key::A, Modifiers::COMMAND), Event::Text(text.into())],
        );
        layers_frame_output(ctx, app, vec![key(Key::Tab, Modifiers::NONE)]);
    }
    fn shift_click(ctx: &Context, app: &mut AtelierApp, point: Point) {
        settle(ctx, app);
        let r = app.workbench.panel_rects["view.canvas"];
        let p = app.screen(r, point);
        assert!(
            r.contains(p),
            "shared context must be visible on the normal canvas"
        );
        // Hold the modifier through the normal event stream, as the existing
        // canvas-selection regression does; a click's local modifiers alone
        // do not change egui's global modifier state.
        for pressed in [true, false] {
            layers_frame_output(
                ctx,
                app,
                vec![
                    Event::ModifiersChanged(Modifiers::SHIFT),
                    Event::PointerMoved(p),
                    Event::PointerButton {
                        pos: p,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::SHIFT,
                    },
                ],
            );
        }
        layers_frame_output(ctx, app, vec![Event::ModifiersChanged(Modifiers::NONE)]);
        settle(ctx, app);
    }
    fn run_check(ctx: &Context, app: &mut AtelierApp, id: &str) {
        for _ in 0..64 {
            let out = layers_frame_output(ctx, app, vec![]);
            let r = ctx
                .data(|d| d.get_temp::<Rect>(Id::new(("check_run", id))))
                .unwrap();
            if out.shapes.iter().any(|s| {
                s.clip_rect.contains(r.center())
                    && text_center(
                        std::slice::from_ref(s),
                        app.language.translate("Preview check update"),
                    )
                    .is_some_and(|p| r.contains(p))
            }) {
                let widget = ctx
                    .data(|d| d.get_temp::<Id>(Id::new(("check_run_id", id))))
                    .unwrap();
                action(ctx, app, widget.accesskit_id(), Action::Click, None);
                assert!(
                    app.process_ui.preview.is_some(),
                    "visible check button must prepare: {} / {}",
                    app.checks_ui.error,
                    app.process_ui.error
                );
                return;
            }
            let window = ctx
                .memory(|m| m.area_rect(Id::new("quantified_checks")))
                .unwrap();
            layers_frame_output(
                ctx,
                app,
                vec![
                    Event::PointerMoved(pos2(window.right() - 15., window.bottom() - 80.)),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        delta: vec2(0., -140.),
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        panic!("normal check row {id} not visible");
    }
    fn gap(a: &Element, b: &Element) -> f64 {
        let (a, _) = a.authored_prism_rings().unwrap();
        let (b, _) = b.authored_prism_rings().unwrap();
        fn edge(p: Point, a: Point, b: Point) -> f64 {
            let (x, y) = (b.x - a.x, b.y - a.y);
            let t = (((p.x - a.x) * x + (p.y - a.y) * y) / (x * x + y * y)).clamp(0., 1.);
            ((p.x - a.x - t * x).powi(2) + (p.y - a.y - t * y).powi(2)).sqrt()
        }
        a.iter()
            .flat_map(|p| {
                b.iter()
                    .zip(b.iter().cycle().skip(1))
                    .take(b.len())
                    .map(move |(u, v)| edge(*p, *u, *v))
            })
            .chain(b.iter().flat_map(|p| {
                a.iter()
                    .zip(a.iter().cycle().skip(1))
                    .take(a.len())
                    .map(move |(u, v)| edge(*p, *u, *v))
            }))
            .fold(f64::INFINITY, f64::min)
    }
    #[test]
    fn normal_pair_selection_check_figure_parameter_lifecycle_bilingual() {
        let supplied = std::env::var_os("OPENARCHI_COMMON_CONTEXT_INPUT").map(PathBuf::from);
        let root = std::env::var_os("OPENARCHI_COMMON_CONTEXT_OUTPUT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!(
                    "oa-common-loop-{}",
                    storage::new_document_identity()
                ))
            });
        std::fs::create_dir(&root).expect("new operation output; never replace an unknown result");
        let (input, common, axis, delta, limit) = if let Some(input) = supplied {
            (
                input,
                std::env::var("OPENARCHI_COMMON_CONTEXT_COMMON")
                    .unwrap()
                    .parse::<u64>()
                    .unwrap(),
                1,
                1000,
                6.,
            )
        } else {
            let (mut e, _, _) = object_nearby_product_path::fixture();
            let common = e.project.add(
                0,
                Material::Concrete,
                Shape::ConceptMass {
                    spec: crate::concept_mass::ConceptMassSpec {
                        arcs: vec![],
                        hole_arcs: vec![],
                        outline: vec![
                            Point::new(60., 43.),
                            Point::new(64., 43.),
                            Point::new(64., 47.),
                            Point::new(60., 47.),
                        ],
                        holes: vec![],
                        height: 3.6,
                        bottom_offset: 0.,
                    },
                },
            );
            let path = root.join("authored-input.openarchi.json");
            storage::save_document(&e.project, &path, e.project_identity()).unwrap();
            (path, common, 0, 2000, 3.)
        };
        let original_bytes = std::fs::read(&input).unwrap();
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
            app.host
                .configure_operation_journal(Some(dir.join("journal")))
                .unwrap();
            open(&ctx, &mut app, &input);
            layers_frame_output(&ctx, &mut app, vec![key(Key::F2, Modifiers::NONE)]);
            let before = app.host.document.editor.project.clone();
            let task = before.design_process.task().unwrap();
            let option = before.plan_options.active_design_option.unwrap();
            let candidate = task.candidates.iter().find(|c| c.option == option).unwrap();
            let member = candidate.initial_mass_ids[0];
            let old = |id| before.elements.iter().find(|e| e.id == id).unwrap();
            let common_element = old(common).clone();
            let initial_gap = gap(old(member), &common_element);
            let (ring, _) = common_element.authored_prism_rings().unwrap();
            let center = Point::new(
                ring.iter().map(|p| p.x).sum::<f64>() / ring.len() as f64,
                ring.iter().map(|p| p.y).sum::<f64>() / ring.len() as f64,
            );
            // Viewport framing only; selection/model/task must come from ordinary controls.
            app.plan_center = center;
            app.plan_scale = 5.;
            settle(&ctx, &mut app);
            let caption = app
                .mass_space_canvas_selection()
                .map(|e| format!("#{} · {}", e.id, e.name))
                .unwrap_or_else(|| language.translate("Select a source mass").into());
            button(&ctx, &mut app, &caption);
            button(&ctx, &mut app, &format!("#{member} · {}", old(member).name));
            shift_click(&ctx, &mut app, center);
            let mut selected = vec![common, member];
            selected.sort_unstable();
            assert_eq!(
                app.host.document.editor.selection_ids(),
                selected,
                "normal Shift selection must preserve both targets"
            );
            assert_eq!(app.host.document.editor.project, before);
            button(&ctx, &mut app, "Check current design");
            button(
                &ctx,
                &mut app,
                "Add check for current selection or candidate",
            );
            settle(&ctx, &mut app);
            check_input(
                &ctx,
                &mut app,
                "Check name",
                "Shared context study clearance",
            );
            let metric = ctx
                .data(|d| d.get_temp::<Rect>(Id::new("check_metric_control")))
                .unwrap();
            layers_click(&ctx, &mut app, metric.center());
            button(&ctx, &mut app, Metric::HorizontalSpacing.label());
            button(&ctx, &mut app, "At least");
            check_input(
                &ctx,
                &mut app,
                "Limit (blank means unknown)",
                &limit.to_string(),
            );
            check_input(
                &ctx,
                &mut app,
                "Optional assumption note",
                "Authored retained-study role and design preference only; surveyed identity, use and statutory clearance unknown. Preserve the common object.",
            );
            button(&ctx, &mut app, "Preview quantified check");
            button(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, before);
            button(&ctx, &mut app, "Preview quantified check");
            button(&ctx, &mut app, "Confirm design change");
            settle(&ctx, &mut app);
            let c = app
                .host
                .document
                .editor
                .project
                .rules
                .checks
                .iter()
                .find(|c| {
                    !before
                        .rules
                        .checks
                        .iter()
                        .any(|old| old.definition.id == c.definition.id)
                })
                .unwrap()
                .clone();
            let id = c.definition.id.clone();
            assert_eq!(
                c.definition.scope,
                crate::constraints::Scope::Objects {
                    ids: selected.clone()
                }
            );
            assert_eq!(c.state(&app.host.document.editor.project), State::Pass);
            assert!((c.report.as_ref().unwrap().rows[0].value.unwrap() - initial_gap).abs() < 1e-8);
            if before.drawings.diagrams.iter().any(|f| {
                f.definition.presentation == Presentation::ConstraintOverlay
                    && f.definition.story == 0
                    && f.definition.option == Some(option)
            }) {
                button(&ctx, &mut app, "Show constraint figure");
                button(&ctx, &mut app, "Preview figure update");
                assert!(
                    app.process_ui.preview.is_some(),
                    "existing figure refresh must prepare: view={:?}, error={}, status={}",
                    app.view,
                    app.process_ui.error,
                    app.host.document.editor.status
                );
            } else {
                button(&ctx, &mut app, "Preview constraint figure");
            }
            button(&ctx, &mut app, "Confirm design change");
            settle(&ctx, &mut app);
            let figure = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .iter()
                .rev()
                .find(|f| f.definition.presentation == Presentation::ConstraintOverlay)
                .unwrap()
                .clone();
            let figure_id = figure.id;
            assert_eq!(
                diagrams::checks::Source::read(&figure.snapshot)
                    .unwrap()
                    .common_context,
                vec![common_element.clone()]
            );
            assert!(
                diagrams::site::Page::new(&figure)
                    .unwrap()
                    .paths
                    .iter()
                    .any(|p| p.element_id == Some(common))
            );
            button(&ctx, &mut app, "Edit source model");
            button(&ctx, &mut app, "Adjust generated wings");
            settle(&ctx, &mut app);
            let params = crate::design_process::parametric::current_parameters(
                &app.host.document.editor.project,
                &task.id,
                option,
            )
            .unwrap();
            let key_name = if axis == 0 {
                "Origin X (mm)"
            } else {
                "Origin Y (mm)"
            };
            number(
                &ctx,
                &mut app,
                0,
                key_name,
                (params.origin_mm[axis] + delta) as f64,
            );
            task_visible_button(&ctx, &mut app, "Preview two-wing parameters");
            let previews = &app
                .process_ui
                .preview
                .as_ref()
                .unwrap()
                .declared_check_previews;
            let value = previews.iter().find(|p| p.definition.id == id).unwrap();
            assert_eq!(value.state, State::Fail);
            assert!(
                (value.rows[0].value.unwrap() - (initial_gap - delta as f64 / 1000.)).abs() < 1e-8
            );
            button(&ctx, &mut app, "Cancel design proposal");
            let unmodified = app.host.document.editor.project.clone();
            task_visible_button(&ctx, &mut app, "Preview two-wing parameters");
            button(&ctx, &mut app, "Confirm design change");
            let changed = app.host.document.editor.project.clone();
            assert_eq!(
                changed.elements.iter().find(|e| e.id == common),
                Some(&common_element)
            );
            assert_eq!(
                changed
                    .drawings
                    .diagrams
                    .iter()
                    .find(|f| f.id == figure_id)
                    .unwrap()
                    .status(&changed),
                Status::Stale
            );
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(
                app.host.document.editor.project.elements,
                unmodified.elements
            );
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            assert_eq!(app.host.document.editor.project, changed);
            button(&ctx, &mut app, "Check current design");
            run_check(&ctx, &mut app, &id);
            button(&ctx, &mut app, "Confirm design change");
            button(&ctx, &mut app, "Show constraint figure");
            button(&ctx, &mut app, "Preview figure update");
            button(&ctx, &mut app, "Confirm design change");
            settle(&ctx, &mut app);
            let final_project = app.host.document.editor.project.clone();
            let f = final_project
                .drawings
                .diagrams
                .iter()
                .find(|f| f.id == figure_id)
                .unwrap();
            assert_eq!(f.status(&final_project), Status::Current);
            assert!(f.generation > figure.generation);
            assert_eq!(
                diagrams::checks::Source::read(&f.snapshot)
                    .unwrap()
                    .common_context,
                vec![common_element.clone()]
            );
            for e in &before.elements {
                if !candidate.initial_mass_ids.contains(&e.id) {
                    assert_eq!(
                        final_project.elements.iter().find(|n| n.id == e.id),
                        Some(e)
                    );
                }
            }
            let output = dir.join("common-loop.openarchi.json");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let old_path = app.file_dialog_path.clone().unwrap();
            field(&ctx, &mut app, &old_path, &output.display().to_string());
            button(&ctx, &mut app, "Save");
            let (fresh_ctx, mut fresh) = setup();
            fresh_ctx.enable_accesskit();
            fresh.language = language;
            open(&fresh_ctx, &mut fresh, &output);
            assert_eq!(
                fresh.host.document.editor.project.elements,
                final_project.elements
            );
            assert_eq!(
                fresh.host.document.editor.project.rules,
                final_project.rules
            );
            assert_eq!(
                fresh.host.document.editor.project.evidence,
                final_project.evidence
            );
            assert!(!fresh.host.operations.borrow().enabled());
            std::fs::write(dir.join("figure.svg"), f.svg(&final_project).unwrap()).unwrap();
            std::fs::write(dir.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"pass","entry":"normal Open/source selector/Shift/check draft/preview/cancel/confirm/figure/parameters/UndoRedo/explicit refresh/SaveAs/new host","common":common,"member":member,"selection":selected,"check":id,"figure":figure_id,"initial_gap_m":initial_gap,"final_gap_m":initial_gap-delta as f64/1000.,"limit_m":limit,"output":output,"native_OS":"not_run","AI":"not_run","protected_other_objects":true})).unwrap()).unwrap();
        }
        assert_eq!(std::fs::read(input).unwrap(), original_bytes);
    }
}
