mod mass_space_product_path {
    use super::*;
    pub(super) fn identity_field(ctx: &Context, app: &mut AtelierApp, field: &str, value: &str) {
        for _ in 0..3 {
            layers_frame_output(ctx, app, vec![]);
        }
        let id = ctx
            .data(|d| d.get_temp::<Id>(Id::new(("mass_space_identity", field))))
            .unwrap();
        let out = layers_frame_output(ctx, app, vec![]);
        let old = if field == "name" {
            format!(
                "{} #{}",
                app.language.translate("Concept space"),
                app.host.document.editor.selected.unwrap()
            )
        } else {
            app.language.translate("Unknown concept use").to_owned()
        };
        let at = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == old => {
                    let rect = Rect::from_min_size(t.pos, t.galley.size());
                    s.clip_rect.contains_rect(rect).then(|| rect.center())
                }
                _ => None,
            })
            .unwrap_or_else(|| {
                let mut text = vec![];
                collect_text(&out.shapes, &mut text);
                panic!("identity {field}, id {id:?}, popup value not painted: {text:?}")
            });
        layers_click(ctx, app, at);
        layers_frame_output(
            ctx,
            app,
            vec![key(Key::A, Modifiers::COMMAND), Event::Text(value.into())],
        );
        layers_frame_output(ctx, app, vec![key(Key::Tab, Modifiers::NONE)]);
    }
    pub(super) fn click(ctx: &Context, app: &mut AtelierApp, key: &str) {
        reveal_manual_control(ctx, app, key);
        for _ in 0..20 {
            layers_frame_output(ctx, app, vec![]);
        }
        for attempt in 0..26 {
            let out = layers_frame_output(ctx, app, vec![]);
            let bounds = if app.process_ui.preview.is_some() {
                ctx.memory(|m| m.area_rect(Id::new("design_process_proposal")))
            } else if app.file_action.is_some() {
                ctx.memory(|m| m.area_rect(Id::new("project_file_dialog")))
            } else {
                None
            };
            if let Some(at) = out.shapes.iter().find_map(|s| {
                let egui::Shape::Text(t) = &s.shape else {
                    return None;
                };
                if t.galley.text() != app.language.translate(key) {
                    return None;
                }
                let mut r = Rect::from_min_size(t.pos, t.galley.size()).intersect(s.clip_rect);
                if let Some(b) = bounds {
                    r = r.intersect(b);
                }
                r.is_positive().then(|| r.center())
            }) {
                layers_click(ctx, app, at);
                eprintln!(
                    "MASS_SPACE_CONTROL {key} at {at:?}; preview={} status={} error={}",
                    app.process_ui.preview.is_some(),
                    app.host.document.editor.status,
                    app.process_ui.error
                );
                return;
            }
            if bounds.is_none() {
                layers_frame_output(
                    ctx,
                    app,
                    vec![
                        Event::PointerMoved(pos2(1250., 740.)),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            phase: egui::TouchPhase::Move,
                            delta: vec2(0., if attempt < 13 { -65. } else { 65. }),
                            modifiers: Modifiers::NONE,
                        },
                    ],
                );
                for _ in 0..20 {
                    layers_frame_output(ctx, app, vec![]);
                }
            }
        }
        let out = layers_frame_output(ctx, app, vec![]);
        let mut texts = vec![];
        collect_text(&out.shapes, &mut texts);
        panic!(
            "{key}: {} / {} / {texts:?}",
            app.process_ui.error, app.host.document.editor.status
        );
    }
    pub(super) fn number(ctx: &Context, app: &mut AtelierApp, label: &str, value: &str) {
        for _ in 0..20 {
            layers_frame_output(ctx, app, vec![]);
        }
        for attempt in 0..26 {
            let f = layers_frame_output(ctx, app, vec![]);
            let l = f.shapes.iter().find_map(|s| {
                text_center(std::slice::from_ref(s), app.language.translate(label))
                    .filter(|p| s.clip_rect.contains(*p) && p.x > 1000.)
            });
            let at = l.and_then(|l| {
                f.shapes.iter().find_map(|s| {
                    let egui::Shape::Text(t) = &s.shape else {
                        return None;
                    };
                    let p = t.pos + t.galley.size() / 2.;
                    (s.clip_rect.contains(p)
                        && (p.y - l.y).abs() < 10.
                        && p.x > l.x
                        && t.galley.text().starts_with(|c: char| c.is_ascii_digit()))
                    .then_some(p)
                })
            });
            if let Some(at) = at {
                layers_click(ctx, app, at);
                layers_frame_output(
                    ctx,
                    app,
                    vec![key(Key::A, Modifiers::COMMAND), Event::Text(value.into())],
                );
                layers_frame_output(ctx, app, vec![key(Key::Tab, Modifiers::NONE)]);
                return;
            }
            layers_frame_output(
                ctx,
                app,
                vec![
                    Event::PointerMoved(pos2(1250., 740.)),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        delta: vec2(0., if attempt < 13 { -45. } else { 45. }),
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
            for _ in 0..20 {
                layers_frame_output(ctx, app, vec![]);
            }
        }
        let f = layers_frame_output(ctx, app, vec![]);
        let mut texts = vec![];
        collect_text(&f.shapes, &mut texts);
        let positions = f
            .shapes
            .iter()
            .filter_map(|s| {
                let egui::Shape::Text(t) = &s.shape else {
                    return None;
                };
                (t.pos.x > 1000.).then(|| (t.galley.text().to_string(), t.pos, s.clip_rect))
            })
            .collect::<Vec<_>>();
        panic!(
            "missing number {label}, selected {:?}, draft {:?}, {texts:?}, positions {positions:?}, panels {:?}",
            app.host.document.editor.selected, app.draft_id, app.workbench.panel_rects
        );
    }

    #[cfg(unix)]
    fn default_gui_poll_protects_pending_parameters(
        app: &mut AtelierApp,
        ctx: &Context,
        journal: &std::path::Path,
    ) {
        use std::{
            io::{BufRead, BufReader, Write},
            os::unix::net::UnixStream,
            sync::mpsc,
            time::{Duration, Instant},
        };
        let path = PathBuf::from(format!(
            "/tmp/oa-ms-gui-{}/app.sock",
            &storage::new_document_identity()[..12]
        ));
        let mut endpoint = crate::automation_ipc::Endpoint::bind_for_host(
            path.clone(),
            true,
            Some(journal.to_owned()),
            &mut app.host,
        )
        .unwrap();
        endpoint.set_write_enabled(true, &app.host.document.editor);
        let target = app.host.operation_target();
        let revision = app.host.document.editor.revision;
        let mass = app.host.document.editor.selected.unwrap();
        let stamp = crate::mass_spaces::element_stamp(
            &app.host.document.editor.project,
            app.host
                .document
                .editor
                .project
                .elements
                .iter()
                .find(|e| e.id == mass)
                .unwrap(),
        )
        .unwrap();
        let request = serde_json::json!({"method":"prepare_native_change","params":{"target":target,"expected_revision":revision,"change":{"version":1,"edits":[{"operation":"mass_space","edit":{"action":"create","mass":mass,"expected_source_sha256":stamp,"name":"Remote attempt","purpose":"Unknown","height":3.,"assumption":"must not overwrite the user's active parameter draft"}}]}}});
        app.automation = Some(endpoint);
        let worker_path = path.clone();
        let (tx, rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut stream = UnixStream::connect(worker_path).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            writeln!(stream, "{request}").unwrap();
            let mut line = String::new();
            BufReader::new(stream).read_line(&mut line).unwrap();
            tx.send(serde_json::from_str::<serde_json::Value>(&line).unwrap())
                .unwrap();
        });
        let before = app.host.document.editor.project.clone();
        let start = Instant::now();
        let response = loop {
            app.poll_automation(ctx);
            if let Ok(r) = rx.try_recv() {
                break r;
            }
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(2));
        };
        worker.join().unwrap();
        assert_eq!(
            response["error"]["code"], "USER_INTERACTION_ACTIVE",
            "{response}"
        );
        assert_eq!(app.host.document.editor.project, before);
        assert!(app.mass_spaces_ui.dirty);
        drop(app.automation.take());
        std::fs::remove_dir(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn mass_space_normal_canvas_context_preview_update_user_protection_save_bilingual() {
        use crate::mass_spaces;
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            let supplied = std::env::var_os("OPENARCHI_MASS_SPACE_INPUT").map(PathBuf::from);
            let destination = std::env::var_os("OPENARCHI_MASS_SPACE_OUTPUT").map(PathBuf::from);
            let root = destination
                .clone()
                .unwrap_or_else(|| {
                    std::env::temp_dir().join(format!(
                        "oa-mass-space-ui-{}",
                        storage::new_document_identity()
                    ))
                })
                .join(if language == UiLanguage::English {
                    "en"
                } else {
                    "zh"
                });
            std::fs::create_dir_all(root.parent().unwrap()).unwrap();
            std::fs::create_dir(&root).unwrap();
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            if let Some(input) = &supplied {
                app.host.document.editor = Editor::from_stored_document(input).unwrap();
            } else {
                let mut p = Project::default();
                p.add(
                    0,
                    Material::Plaster,
                    Shape::ConceptMass {
                        spec: crate::concept_mass::ConceptMassSpec {
                            arcs: vec![],
                            hole_arcs: vec![],
                            outline: vec![
                                Point::new(30., 30.),
                                Point::new(37.65, 30.),
                                Point::new(37.65, 35.),
                                Point::new(30., 35.),
                            ],
                            holes: vec![],
                            height: 5.15,
                            bottom_offset: 0.,
                        },
                    },
                );
                app.host.document.editor = Editor::new(p);
            }
            let initial = app.host.document.editor.project.clone();
            let mass = initial
                .elements
                .iter()
                .find(|e| {
                    matches!(e.shape, Shape::ConceptMass { .. })
                        && e.design_option == initial.plan_options.active_design_option
                })
                .unwrap()
                .clone();
            app.plan_center = Point::new(34., 33.);
            app.plan_scale = 40.;
            std::fs::write(root.join("intent.json"),serde_json::to_vec_pretty(&serde_json::json!({"entry":"normal existing canvas/Inspector/Proposal/File widgets","input":supplied,"source_mass":mass,"initial_elements":initial.elements.len(),"operation_root":root,"expected":"one explicit-assumption editable space, cancel/undo/redo/source freshness/user-edit refusal/save-reopen; no AI/image claim"})).unwrap()).unwrap();
            click(&ctx, &mut app, "Select");
            for _ in 0..3 {
                layers_frame_output(&ctx, &mut app, vec![]);
            }
            let canvas = app.workbench.panel_rects["view.canvas"];
            let at = app.screen(canvas, Point::new(31., 31.));
            layers_click(&ctx, &mut app, at);
            assert_eq!(app.host.document.editor.selected, Some(mass.id));
            click(&ctx, &mut app, "Space name and use");
            identity_field(&ctx, &mut app, "name", "Shared teaching concept");
            identity_field(
                &ctx,
                &mut app,
                "purpose",
                "Teaching exchange; concept use, approval unknown",
            );
            click(&ctx, &mut app, "Space name and use");
            number(&ctx, &mut app, "Assumed clear height", "2.8");
            assert!(app.mass_spaces_ui.dirty);
            assert!(app.pending_design_parameters());
            #[cfg(unix)]
            default_gui_poll_protects_pending_parameters(&mut app, &ctx, &root.join("journal"));
            let path = root.join("continued.openarchi.json");
            app.path = path.to_string_lossy().into();
            app.path_confirmed = false;
            app.file_baseline = None;
            click(&ctx, &mut app, "Save project");
            click(&ctx, &mut app, "Save");
            assert!(app.file_action.is_some());
            assert!(!path.exists());
            click(&ctx, &mut app, "Cancel");
            click(&ctx, &mut app, "Create editable space");
            assert!(
                app.process_ui.preview.is_some(),
                "{} / {}",
                app.host.document.editor.status,
                app.process_ui.error
            );
            assert_eq!(app.host.document.editor.project, initial);
            click(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, initial);
            assert!(app.mass_spaces_ui.dirty);
            click(&ctx, &mut app, "Create editable space");
            click(&ctx, &mut app, "Confirm design change");
            assert!(app.process_ui.preview.is_none(), "{}", app.process_ui.error);
            assert!(!app.mass_spaces_ui.dirty);
            let created = app.host.document.editor.project.clone();
            assert_eq!(created.elements.len(), initial.elements.len() + 1);
            let link = created.design_process.mass_spaces.last().unwrap().clone();
            assert_eq!(app.host.document.editor.selected, Some(link.zone));
            let z = created.elements.iter().find(|e| e.id == link.zone).unwrap();
            let Shape::Zone { spec } = &z.shape else {
                panic!("real Zone");
            };
            assert_eq!(z.name, "Shared teaching concept");
            assert_eq!(
                spec.purpose,
                "Teaching exchange; concept use, approval unknown"
            );
            assert_eq!(spec.height, 2.8);
            assert!((spec.area() - 38.25).abs() < 1e-8);
            for old in &initial.elements {
                assert_eq!(created.elements.iter().find(|e| e.id == old.id), Some(old));
            }
            assert_eq!(created.drawings, initial.drawings);
            assert_eq!(created.job_archive, initial.job_archive);
            assert_eq!(
                created.plan_options.active_design_option,
                initial.plan_options.active_design_option
            );
            assert_eq!(app.view, View::Plan);
            click(&ctx, &mut app, "Undo");
            assert_eq!(app.host.document.editor.project, initial);
            click(&ctx, &mut app, "Redo");
            assert_eq!(app.host.document.editor.project, created);
            layers_click(&ctx, &mut app, at); // canvas selects the source mass at the original overlapping location
            // Explicit nearby source navigation also handles a Zone hit in the overlap.
            if app.host.document.editor.selected == Some(link.zone) {
                click(&ctx, &mut app, "Return to source mass");
            }
            assert_eq!(app.host.document.editor.selected, Some(mass.id));
            number(&ctx, &mut app, "Height", "5.4");
            assert!(
                matches!(&app.draft.as_ref().unwrap().shape,Shape::ConceptMass {spec} if spec.height==5.4 && spec.bottom_offset==0.),
                "numeric row must change height, not elevation"
            );
            click(&ctx, &mut app, "Apply changes");
            assert_eq!(
                mass_spaces::status(&app.host.document.editor.project, &link),
                "Mass-space source changed"
            );
            click(&ctx, &mut app, "Preview space update");
            assert!(
                app.process_ui.preview.is_some(),
                "{} / {}",
                app.host.document.editor.status,
                app.process_ui.error
            );
            click(&ctx, &mut app, "Confirm design change");
            assert_eq!(
                app.host.document.editor.selected,
                Some(link.zone),
                "metadata-only refresh must retain result navigation"
            );
            let refreshed = app.host.document.editor.project.clone();
            let fresh = refreshed.design_process.mass_spaces.last().unwrap().clone();
            assert_eq!(
                mass_spaces::status(&refreshed, &fresh),
                "Mass-space link is current"
            );
            let zone = refreshed
                .elements
                .iter()
                .find(|e| e.id == fresh.zone)
                .unwrap();
            assert_eq!(zone.name, "Shared teaching concept");
            assert!(
                matches!(&zone.shape, Shape::Zone { spec } if spec.purpose == "Teaching exchange; concept use, approval unknown")
            );
            number(&ctx, &mut app, "Clear height", "2.6");
            click(&ctx, &mut app, "Apply changes");
            let user = app.host.document.editor.project.clone();
            assert_eq!(
                mass_spaces::status(&user, &fresh),
                "Linked space has user edits"
            );
            click(&ctx, &mut app, "Preview space update");
            assert!(app.process_ui.preview.is_none());
            assert_eq!(app.host.document.editor.project, user);
            assert!(app.host.document.editor.status.contains("user edits"));
            click(&ctx, &mut app, "Detach space link");
            click(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, user);
            click(&ctx, &mut app, "Detach space link");
            click(&ctx, &mut app, "Confirm design change");
            assert!(
                app.host
                    .document
                    .editor
                    .project
                    .design_process
                    .mass_spaces
                    .is_empty()
            );
            let detached = app.host.document.editor.project.clone();
            click(&ctx, &mut app, "Link existing space to mass");
            let source_button = format!("#{} · {}", mass.id, mass.name);
            click(&ctx, &mut app, &source_button);
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            click(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, detached);
            click(&ctx, &mut app, &source_button);
            click(&ctx, &mut app, "Confirm design change");
            assert_eq!(app.host.document.editor.project.elements, detached.elements);
            assert!(matches!(
                app.host.document.editor.project.design_process.mass_spaces[0].derivation,
                crate::mass_spaces::Derivation::Existing
            ));
            click(&ctx, &mut app, "Preview space update");
            assert!(app.process_ui.preview.is_none());
            assert!(
                app.host
                    .document
                    .editor
                    .status
                    .contains("does not authorize geometry regeneration")
            );
            click(&ctx, &mut app, "Undo");
            assert_eq!(app.host.document.editor.project, detached);
            click(&ctx, &mut app, "Undo");
            assert_eq!(app.host.document.editor.project, user);
            // Restore the explicit refresh before the manual protected edit, then save a current linked result.
            click(&ctx, &mut app, "Undo");
            assert_eq!(app.host.document.editor.project, refreshed);
            for old in &initial.elements {
                if old.id != mass.id {
                    assert_eq!(
                        refreshed.elements.iter().find(|e| e.id == old.id),
                        Some(old)
                    );
                }
            }
            let mut expected_source = mass.clone();
            if let Shape::ConceptMass { spec } = &mut expected_source.shape {
                spec.height = 5.4;
            }
            assert_eq!(
                refreshed.elements.iter().find(|e| e.id == mass.id),
                Some(&expected_source)
            );
            click(&ctx, &mut app, "Save project");
            click(&ctx, &mut app, "Save");
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let reopened = Editor::from_stored_document(&path).unwrap();
            // Save As intentionally relocates content-addressed sidecars. Verify
            // their actual bytes and target binding, then compare every project
            // field exactly with only those verified addresses changed.
            let mut persisted_expected = refreshed.clone();
            assert_eq!(
                reopened.project.drawings.pages.len(),
                persisted_expected.drawings.pages.len()
            );
            let sidecars = PathBuf::from(format!("{}.assets", path.display()));
            for (actual, expected) in reopened
                .project
                .drawings
                .pages
                .iter()
                .zip(&mut persisted_expected.drawings.pages)
            {
                assert!(std::path::Path::new(&actual.path).starts_with(&sidecars));
                let actual_bytes = std::fs::read(&actual.path).unwrap();
                assert_eq!(actual_bytes, std::fs::read(&expected.path).unwrap());
                assert_eq!(crate::drawings::digest(&actual_bytes), expected.sha256);
                expected.path = actual.path.clone();
            }
            assert_eq!(reopened.project, persisted_expected);
            let report = serde_json::json!({"input":supplied,"output":path,"mass_id":mass.id,"zone_id":link.zone,"initial_elements":initial.elements.len(),"final_elements":refreshed.elements.len(),"area_m2":38.25,"height_m":2.8,"language":format!("{language:?}"),"project_option_retained":true,"new_user_windows":1,"window_reason":"existing shared confirmation preview; canvas and Inspector retain context","cross_menu_context_transfers":0,"repeated_inputs":0,"create_preserved_original_elements":true,"final_original_changes":[mass.id],"preview_cancel":true,"undo_redo":true,"source_stale_explicit_refresh":true,"user_edit_refused":true,"detach_cancel_confirm_undo":true,"existing_bind_cancel_confirm_geometry_unchanged":true,"existing_bind_does_not_authorize_regeneration":true,"save_reopen":true,"native_os":"not_run","AI_images":"not_run; outside this finite slice"});
            std::fs::write(
                root.join("walkthrough.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
            println!(
                "MASS_SPACE_WALK={}",
                root.join("walkthrough.json").display()
            );
            // A second new ApplicationHost is a headless persistence check, not a native process claim.
            let mut second = crate::application_host::ApplicationHost::new(Project::default());
            second.document.editor = reopened;
            assert_eq!(second.document.editor.project, persisted_expected);
            if destination.is_none() {
                std::fs::remove_dir_all(root.parent().unwrap()).unwrap();
            }
        }
    }
    #[test]
    fn candidate_linked_space_quantity_normal_open_canvas_edit_stale_save_bilingual() {
        use crate::{
            command::ModelCommand,
            diagrams::{self, Domain, Status},
        };
        let supplied = std::env::var_os("OPENARCHI_SPACE_FIGURE_INPUT").map(PathBuf::from);
        let destination = std::env::var_os("OPENARCHI_SPACE_FIGURE_OUTPUT").map(PathBuf::from);
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            let parent = destination.clone().unwrap_or_else(|| {
                std::env::temp_dir().join(format!(
                    "oa-space-figure-{}",
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
                .expect("preserve existing result; do not replay an unknown write");
            let input = supplied.clone().unwrap_or_else(|| {
                // Public synthetic starting fixture only; all tested actions below use normal widgets.
                let image = root.join("authored-fixture.png");
                image::RgbaImage::from_pixel(40, 30, image::Rgba([220, 205, 170, 255]))
                    .save(&image)
                    .unwrap();
                let mut p = storage::startup_example();
                p.drawings.pages.push(
                    crate::drawings::import(
                        &image,
                        &root.join("source-assets"),
                        p.drawings.next_id().unwrap(),
                    )
                    .unwrap(),
                );
                let mut plan = super::intent_product_path::fixture_mass_plan(&p.drawings.pages[0]);
                // Progressive wire material names differ from the typed Rust draft.
                for group in plan["draft"]["candidates"].as_array_mut().unwrap() {
                    for seed in group.as_array_mut().unwrap() {
                        seed["material"] = serde_json::json!(Material::Plaster);
                    }
                }
                let mut e = Editor::new(p);
                let change = crate::design_process::intent::prepare_mass(
                    &e,
                    plan["task_id"].as_str().unwrap(),
                    serde_json::from_value(plan["draft"].clone()).unwrap(),
                )
                .unwrap();
                e.commit_change("authored-fixture-mass-task", &change)
                    .unwrap();
                let mass = e
                    .project
                    .elements
                    .iter()
                    .find(|v| {
                        v.design_option == e.project.plan_options.active_design_option
                            && matches!(v.shape, Shape::ConceptMass { .. })
                    })
                    .unwrap();
                e.apply(ModelCommand::MassSpaces {
                    edit: crate::mass_spaces::Edit::Create {
                        mass: mass.id,
                        expected_source_sha256: crate::mass_spaces::element_stamp(&e.project, mass)
                            .unwrap(),
                        name: "Authored concept space".into(),
                        purpose: "Unknown".into(),
                        height: 2.8,
                        assumption: "Assumed single concept level; not survey or approved use"
                            .into(),
                    },
                })
                .unwrap();
                let path = root.join("starting.openarchi.json");
                storage::save(&e.project, &path).unwrap();
                path
            });
            let input_bytes = std::fs::read(&input).unwrap();
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            open_test_project(&ctx, &mut app, &input);
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let initial = app.host.document.editor.project.clone();
            let task = initial.design_process.task().unwrap().clone();
            assert!(task.is_mass());
            let link = initial
                .design_process
                .mass_spaces
                .iter()
                .find(|l| l.option == initial.plan_options.active_design_option)
                .unwrap()
                .clone();
            let zone = initial.elements.iter().find(|e| e.id == link.zone).unwrap();
            let Shape::Zone { spec } = &zone.shape else {
                panic!("real Zone")
            };
            let center = spec.label_point();
            let original_height = spec.height;
            app.plan_center = center;
            app.plan_scale = 40.;
            click(&ctx, &mut app, "Select");
            layers_frame_output(&ctx, &mut app, vec![]);
            let at = app.screen(app.workbench.panel_rects["view.canvas"], center);
            layers_click(&ctx, &mut app, at);
            if app.host.document.editor.selected == Some(link.mass) {
                click(&ctx, &mut app, "Select linked space");
            }
            assert_eq!(app.host.document.editor.selected, Some(link.zone));
            click(&ctx, &mut app, "Preview space quantity figure");
            assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
            assert_eq!(app.host.document.editor.project, initial);
            click(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(app.host.document.editor.project, initial);
            click(&ctx, &mut app, "Preview space quantity figure");
            click(&ctx, &mut app, "Confirm design change");
            assert!(app.process_ui.preview.is_none(), "{}", app.process_ui.error);
            let mut figure = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .last()
                .unwrap()
                .clone();
            assert_eq!(figure.definition.domain, Domain::Zones);
            assert_eq!(figure.definition.option, link.option);
            assert_eq!(figure.snapshot.context["task"]["id"], task.id);
            assert_eq!(
                figure.snapshot.context["mass_space_links"],
                serde_json::json!([link])
            );
            assert_eq!(
                figure
                    .snapshot
                    .elements
                    .iter()
                    .map(|v| v.id)
                    .collect::<Vec<_>>(),
                vec![link.zone]
            );
            assert!((figure.total().0 - zone.area()).abs() < 1e-8);
            assert!((figure.total().1 - zone.area() * original_height).abs() < 1e-8);
            assert_eq!(app.view, View::Figure(figure.id));
            assert_eq!(app.host.document.editor.project.elements, initial.elements);
            click(&ctx, &mut app, "Edit source model");
            assert_eq!(app.view, View::Plan);
            assert_eq!(app.host.document.editor.selected, Some(link.zone));
            // First verify source→association→figure in this same normal canvas path.
            click(&ctx, &mut app, "Return to source mass");
            let mass = initial.elements.iter().find(|e| e.id == link.mass).unwrap();
            let Shape::ConceptMass { spec: source_spec } = &mass.shape else {
                panic!("source mass")
            };
            let source_height = source_spec.height + 0.25;
            number(&ctx, &mut app, "Height", &source_height.to_string());
            click(&ctx, &mut app, "Apply changes");
            assert_eq!(
                figure.status(&app.host.document.editor.project),
                Status::Stale
            );
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .elements
                    .iter()
                    .find(|e| e.id == link.zone),
                Some(zone)
            );
            click(&ctx, &mut app, "Select linked space");
            click(&ctx, &mut app, "Show space quantity figure");
            click(&ctx, &mut app, "Preview figure update");
            assert!(app.process_ui.preview.is_none());
            assert!(app.process_ui.error.contains("Update or detach"));
            click(&ctx, &mut app, "Edit source model");
            click(&ctx, &mut app, "Preview space update");
            click(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(
                crate::mass_spaces::status(&app.host.document.editor.project, &link),
                "Mass-space source changed"
            );
            click(&ctx, &mut app, "Preview space update");
            click(&ctx, &mut app, "Confirm design change");
            assert_eq!(app.host.document.editor.selected, Some(link.zone));
            click(&ctx, &mut app, "Show space quantity figure");
            click(&ctx, &mut app, "Preview figure update");
            click(&ctx, &mut app, "Confirm design change");
            let source_refreshed = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .last()
                .unwrap()
                .clone();
            assert_eq!(source_refreshed.id, figure.id);
            assert_eq!(source_refreshed.generation, figure.generation + 1);
            assert_eq!(source_refreshed.total(), figure.total());
            assert_eq!(
                source_refreshed.status(&app.host.document.editor.project),
                Status::Current
            );
            figure = source_refreshed;
            click(&ctx, &mut app, "Edit source model");
            number(&ctx, &mut app, "Clear height", "2.6");
            click(&ctx, &mut app, "Apply changes");
            assert_eq!(
                figure.status(&app.host.document.editor.project),
                Status::Stale
            );
            click(&ctx, &mut app, "Show space quantity figure");
            click(&ctx, &mut app, "Preview figure update");
            assert!(app.process_ui.preview.is_none());
            assert!(app.process_ui.error.contains("Update or detach"));
            assert!(figure.svg(&app.host.document.editor.project).is_err());
            click(&ctx, &mut app, "Edit source model");
            let user_feedback = app.host.document.editor.project.clone();
            click(&ctx, &mut app, "Undo");
            assert!(
                app.process_ui.error.is_empty(),
                "obsolete refusal must not outlive the undone model state"
            );
            assert!(
                matches!(&app.host.document.editor.project.elements.iter().find(|e|e.id==link.zone).unwrap().shape,Shape::Zone{spec} if spec.height==original_height)
            );
            click(&ctx, &mut app, "Redo");
            assert!(app.process_ui.error.is_empty());
            assert_eq!(app.host.document.editor.project, user_feedback);
            click(&ctx, &mut app, "Select");
            for _ in 0..4 {
                layers_frame_output(&ctx, &mut app, vec![]);
            }
            let at = app.screen(app.workbench.panel_rects["view.canvas"], center);
            layers_click(&ctx, &mut app, at);
            if app.host.document.editor.selected == Some(link.mass) {
                click(&ctx, &mut app, "Select linked space");
            }
            assert_eq!(app.host.document.editor.selected, Some(link.zone));
            click(&ctx, &mut app, "Detach space link");
            click(&ctx, &mut app, "Confirm design change");
            assert!(
                app.host
                    .document
                    .editor
                    .project
                    .design_process
                    .mass_spaces
                    .iter()
                    .all(|v| v.zone != link.zone)
            );
            click(&ctx, &mut app, "Show space quantity figure");
            click(&ctx, &mut app, "Preview figure update");
            click(&ctx, &mut app, "Cancel design proposal");
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .last()
                    .unwrap(),
                &figure
            );
            click(&ctx, &mut app, "Preview figure update");
            click(&ctx, &mut app, "Confirm design change");
            let fresh = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .last()
                .unwrap()
                .clone();
            assert_eq!(
                app.host.document.editor.status,
                "Design change recorded; save the project to keep it"
            );
            assert_eq!(fresh.id, figure.id);
            assert_eq!(fresh.generation, figure.generation + 1);
            assert!((fresh.total().1 - zone.area() * 2.6).abs() < 1e-8);
            assert_eq!(
                fresh.status(&app.host.document.editor.project),
                Status::Current
            );
            click(&ctx, &mut app, "Undo");
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .last()
                    .unwrap(),
                &figure
            );
            click(&ctx, &mut app, "Redo");
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .last()
                    .unwrap(),
                &fresh
            );
            let after = app.host.document.editor.project.clone();
            for e in &initial.elements {
                let mut expected = e.clone();
                if expected.id == link.mass {
                    if let Shape::ConceptMass { spec } = &mut expected.shape {
                        spec.height = source_height;
                    }
                }
                if expected.id == link.zone {
                    if let Shape::Zone { spec } = &mut expected.shape {
                        spec.height = 2.6;
                    }
                }
                assert_eq!(
                    after.elements.iter().find(|v| v.id == e.id),
                    Some(&expected)
                );
            }
            assert_eq!(after.design_process.tasks, initial.design_process.tasks);
            assert_eq!(after.job_archive, initial.job_archive);
            for f in &initial.drawings.diagrams {
                assert_eq!(
                    after.drawings.diagrams.iter().find(|v| v.id == f.id),
                    Some(f)
                );
            }
            assert_eq!(after.drawings.reports, initial.drawings.reports);
            // Save As through the actual file text widget, preserving the input file and sidecars.
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let old = app
                .file_dialog_path
                .clone()
                .unwrap_or_else(|| app.path.clone());
            click(&ctx, &mut app, &old);
            let output = root.join("continued.openarchi.json");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![
                    key(Key::A, Modifiers::COMMAND),
                    Event::Text(output.to_string_lossy().into()),
                ],
            );
            click(&ctx, &mut app, "Save");
            assert!(app.file_action.is_none(), "{}", app.file_error);
            assert_eq!(std::fs::read(&input).unwrap(), input_bytes);
            let reopened = Editor::from_stored_document(&output).unwrap();
            let mut expected = after.clone();
            for (a, b) in reopened
                .project
                .drawings
                .pages
                .iter()
                .zip(&mut expected.drawings.pages)
            {
                let bytes = std::fs::read(&a.path).unwrap();
                assert_eq!(bytes, std::fs::read(&b.path).unwrap());
                assert_eq!(crate::drawings::digest(&bytes), b.sha256);
                assert!(
                    PathBuf::from(&a.path)
                        .starts_with(PathBuf::from(format!("{}.assets", output.display())))
                );
                b.path = a.path.clone();
            }
            assert_eq!(reopened.project, expected);
            let query = diagrams::query(
                &reopened.project,
                reopened.project.drawings.diagrams.len() - 1,
                1,
            )
            .unwrap();
            assert_eq!(query["figures"][0]["id"], fresh.id);
            assert_eq!(query["figures"][0]["definition"]["domain"], "zones");
            assert_eq!(
                query["figures"][0]["source_element_ids"],
                serde_json::json!([link.zone])
            );
            assert_eq!(
                query["figures"][0]["mass_space_links"],
                serde_json::Value::Null
            );
            std::fs::write(root.join("walkthrough.json"),serde_json::to_vec_pretty(&serde_json::json!({"entry":"normal CmdO/text/Open, canvas pick, existing quantity toolbar/Proposal, source properties/detach, normal Save As","input":input,"input_sha256":crate::drawings::digest(&input_bytes),"output":output,"figure":fresh.id,"domain":"zones","source_zone":link.zone,"source_task":task.id,"area_m2":fresh.total().0,"volume_m3":fresh.total().1,"initial_elements":initial.elements.len(),"final_elements":after.elements.len(),"all_other_elements_tasks_archives_old_figures_reports":"exactly retained except declared source height and Zone height", "source_height_changed_to_m":source_height,"source_change_then_link_and_figure_explicit_refresh":true,"preview_cancel":true,"user_edit_relation_refusal":true,"explicit_detach_then_figure_refresh":true,"undo_redo":true,"save_reopen":"exact full Project after verified asset address relocation; new Host, not new native OS process","cross_menu_context_transfers":0,"repeated_task_or_source_upload":0,"windows":"existing shared confirmation only; figure uses continuous main canvas","native_os":"not_run current source","AI_images":"not_run outside slice"})).unwrap()).unwrap();
            println!(
                "SPACE_FIGURE_WALK={}",
                root.join("walkthrough.json").display()
            );
            if destination.is_none() {
                std::fs::remove_dir_all(&parent).unwrap();
            }
        }
    }
    #[test]
    fn canvas_mass_space_entry_without_inspector_preserves_context_and_updates_figure_bilingual() {
        use crate::{command::ModelCommand, diagrams::Status};
        fn area(points: &[Point]) -> f64 {
            points
                .iter()
                .zip(points.iter().cycle().skip(1))
                .take(points.len())
                .map(|(a, b)| a.x * b.y - b.x * a.y)
                .sum::<f64>()
                .abs()
                * 0.5
        }
        fn canvas_number(ctx: &Context, app: &mut AtelierApp, value: &str) {
            reveal_manual_control(ctx, app, "Assumed clear height");
            layers_frame_output(ctx, app, vec![]);
            let out = layers_frame_output(ctx, app, vec![]);
            let label = text_center(&out.shapes, app.language.translate("Assumed clear height"))
                .expect("current canvas height label");
            let at = out
                .shapes
                .iter()
                .find_map(|s| {
                    let egui::Shape::Text(t) = &s.shape else {
                        return None;
                    };
                    let p = t.pos + t.galley.size() / 2.;
                    (s.clip_rect.contains(p)
                        && (p.y - label.y).abs() < 10.
                        && p.x > label.x
                        && t.galley.text().starts_with(|c: char| c.is_ascii_digit()))
                    .then_some(p)
                })
                .expect("canvas assumed height field");
            assert!(
                ctx.content_rect().contains(at),
                "on-demand task field is reachable"
            );
            layers_click(ctx, app, at);
            layers_frame_output(
                ctx,
                app,
                vec![key(Key::A, Modifiers::COMMAND), Event::Text(value.into())],
            );
            layers_frame_output(ctx, app, vec![key(Key::Tab, Modifiers::NONE)]);
        }
        fn choose(ctx: &Context, app: &mut AtelierApp, id: u64) {
            layers_frame_output(ctx, app, vec![]);
            let label = app
                .mass_space_canvas_selection()
                .map(|e| format!("#{} · {}", e.id, e.name))
                .unwrap_or_else(|| app.language.translate("Select a source mass").into());
            click(ctx, app, &label);
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
            click(ctx, app, &format!("#{id} · {name}"));
            assert_eq!(app.host.document.editor.selection_ids(), vec![id]);
            assert_eq!(app.host.document.editor.tool, Tool::Select);
        }
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            let supplied = std::env::var_os("OPENARCHI_CANVAS_SPACE_INPUT").map(PathBuf::from);
            let destination = std::env::var_os("OPENARCHI_CANVAS_SPACE_OUTPUT").map(PathBuf::from);
            let parent = destination.clone().unwrap_or_else(|| {
                std::env::temp_dir()
                    .join(format!("oa-canvas-ms-{}", storage::new_document_identity()))
            });
            std::fs::create_dir_all(&parent).unwrap();
            let root = parent.join(if language == UiLanguage::English {
                "en"
            } else {
                "zh"
            });
            std::fs::create_dir(&root)
                .expect("preserve existing result; do not replay an unknown write");
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            let input = supplied.unwrap_or_else(|| {
                let image = root.join("authored.png");
                image::RgbaImage::from_pixel(40, 30, image::Rgba([220, 205, 170, 255]))
                    .save(&image)
                    .unwrap();
                let mut p = storage::startup_example();
                p.drawings.pages.push(
                    crate::drawings::import(
                        &image,
                        &root.join("assets"),
                        p.drawings.next_id().unwrap(),
                    )
                    .unwrap(),
                );
                let mut plan = super::intent_product_path::fixture_mass_plan(&p.drawings.pages[0]);
                for group in plan["draft"]["candidates"].as_array_mut().unwrap() {
                    for seed in group.as_array_mut().unwrap() {
                        seed["material"] = serde_json::json!(Material::Plaster);
                    }
                }
                let mut initial = Editor::new(p);
                let prepared = crate::design_process::intent::prepare_mass(
                    &initial,
                    plan["task_id"].as_str().unwrap(),
                    serde_json::from_value(plan["draft"].clone()).unwrap(),
                )
                .unwrap();
                initial
                    .commit_change("authored-canvas-fixture", &prepared)
                    .unwrap();
                let task = initial.project.design_process.task().unwrap().clone();
                let owned = initial
                    .project
                    .elements
                    .iter()
                    .filter(|e| {
                        e.design_option == Some(task.candidates[0].option)
                            && matches!(e.shape, Shape::ConceptMass { .. })
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                assert_eq!(owned.len(), 2);
                initial
                    .apply(ModelCommand::MassSpaces {
                        edit: crate::mass_spaces::Edit::Create {
                            mass: owned[0].id,
                            expected_source_sha256: crate::mass_spaces::element_stamp(
                                &initial.project,
                                &owned[0],
                            )
                            .unwrap(),
                            name: "Existing authored space".into(),
                            purpose: "Unknown".into(),
                            height: 2.8,
                            assumption: "Authored fixture, not image interpretation or survey"
                                .into(),
                        },
                    })
                    .unwrap();
                let path = root.join("starting.openarchi.json");
                storage::save(&initial.project, &path).unwrap();
                path
            });
            let input_bytes = std::fs::read(&input).unwrap();
            // Everything under test starts from the normal Open dialog.
            open_test_project(&ctx, &mut app, &input);
            click(&ctx, &mut app, "2D Plan");
            click(&ctx, &mut app, "Window");
            let menu = layers_frame_output(&ctx, &mut app, vec![]);
            let at = menu
                .shapes
                .iter()
                .rev()
                .find_map(|shape| {
                    text_center(
                        std::slice::from_ref(shape),
                        language.translate("Navigator & Properties"),
                    )
                    .filter(|p| shape.clip_rect.contains(*p))
                })
                .expect("visible Window palette toggle");
            layers_click(&ctx, &mut app, at);
            assert!(!app.show_right);
            let p = &app.host.document.editor.project;
            let task = p.design_process.task().expect("saved current mass task");
            let masses = p
                .elements
                .iter()
                .filter(|e| {
                    e.design_option == p.plan_options.active_design_option
                        && e.story == task.story
                        && matches!(e.shape, Shape::ConceptMass { .. })
                })
                .cloned()
                .collect::<Vec<_>>();
            let owned = [
                masses
                    .iter()
                    .find(|e| p.design_process.mass_spaces.iter().any(|l| l.mass == e.id))
                    .expect("linked source prerequisite")
                    .clone(),
                masses
                    .iter()
                    .find(|e| !p.design_process.mass_spaces.iter().any(|l| l.mass == e.id))
                    .expect("unlinked source prerequisite")
                    .clone(),
            ];
            let existing_figure = p
                .drawings
                .diagrams
                .iter()
                .find(|f| {
                    f.definition.domain == crate::diagrams::Domain::Zones
                        && f.definition.option == p.plan_options.active_design_option
                        && f.snapshot.context["task"]["id"].as_str() == Some(task.id.as_str())
                })
                .cloned();
            // Overlapping sources remain selectable in the current candidate strip.
            choose(&ctx, &mut app, owned[0].id);
            click(&ctx, &mut app, "Select linked space");
            let figure = if let Some(figure) = existing_figure {
                figure
            } else {
                click(&ctx, &mut app, "Preview space quantity figure");
                click(&ctx, &mut app, "Confirm design change");
                assert!(app.process_ui.preview.is_none(), "{}", app.process_ui.error);
                app.host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .last()
                    .unwrap()
                    .clone()
            };
            assert_eq!(
                figure.status(&app.host.document.editor.project),
                Status::Current
            );
            choose(&ctx, &mut app, owned[1].id);
            assert!(!app.show_right);
            let p = &app.host.document.editor.project;
            let mass_figure_exists = p.drawings.diagrams.iter().any(|f| {
                f.definition.domain == crate::diagrams::Domain::ConceptMasses
                    && f.definition.option == p.plan_options.active_design_option
                    && matches!(
                        f.definition.presentation,
                        crate::diagrams::Presentation::ModelQuantities
                    )
            });
            click(
                &ctx,
                &mut app,
                if mass_figure_exists {
                    "Show linked figure"
                } else {
                    "Preview linked figure"
                },
            );
            if !mass_figure_exists {
                click(&ctx, &mut app, "Confirm design change");
            }
            let source_view = app.view;
            assert!(
                matches!(source_view, View::Figure(_)),
                "create remains accessible on the shared figure canvas"
            );
            let baseline = app.host.document.editor.project.clone();
            let revision = app.host.document.editor.revision;
            let history = app.host.document.editor.undo.len();
            canvas_number(&ctx, &mut app, "2.70");
            assert!(app.mass_spaces_ui.dirty);
            default_gui_poll_protects_pending_parameters(&mut app, &ctx, &root.join("journal"));
            click(&ctx, &mut app, "Cancel space parameters");
            assert!(!app.mass_spaces_ui.dirty);
            assert_eq!(app.host.document.editor.project, baseline);
            assert_eq!(app.host.document.editor.revision, revision);
            assert_eq!(app.view, source_view);
            canvas_number(&ctx, &mut app, "2.70");
            click(&ctx, &mut app, "Create editable space");
            let cancelled = app.process_ui.preview.as_ref().unwrap().id.clone();
            assert_eq!(app.host.document.editor.project, baseline);
            click(&ctx, &mut app, "Cancel design proposal");
            assert!(app.mass_spaces_ui.dirty);
            assert_eq!(app.host.document.editor.undo.len(), history);
            assert_eq!(app.view, source_view);
            click(&ctx, &mut app, "Create editable space");
            assert_ne!(app.process_ui.preview.as_ref().unwrap().id, cancelled);
            click(&ctx, &mut app, "Confirm design change");
            assert!(app.process_ui.preview.is_none(), "{}", app.process_ui.error);
            assert!(!app.mass_spaces_ui.dirty);
            let created = app.host.document.editor.project.clone();
            assert_eq!(created.elements.len(), baseline.elements.len() + 1);
            for old in &baseline.elements {
                assert_eq!(created.elements.iter().find(|e| e.id == old.id), Some(old));
            }
            let link = created
                .design_process
                .mass_spaces
                .iter()
                .find(|l| l.mass == owned[1].id)
                .unwrap();
            let zone = created.elements.iter().find(|e| e.id == link.zone).unwrap();
            let Shape::Zone { spec } = &zone.shape else {
                panic!("real editable Zone")
            };
            let Shape::ConceptMass { spec: mass } = &owned[1].shape else {
                unreachable!()
            };
            let footprint = area(&mass.outline) - mass.holes.iter().map(|h| area(h)).sum::<f64>();
            assert_eq!(spec.height, 2.7);
            assert!((spec.area() - footprint).abs() < 1e-9);
            assert_eq!(spec.outline, mass.outline);
            assert_eq!(spec.holes, mass.holes);
            assert_eq!(zone.design_option, owned[1].design_option);
            assert_eq!(zone.story, owned[1].story);
            assert_eq!(app.host.document.editor.selected, Some(zone.id));
            assert_eq!(app.view, View::Plan);
            assert_eq!(figure.status(&created), Status::Stale);
            click(&ctx, &mut app, "Return to source mass");
            assert_eq!(app.host.document.editor.selected, Some(owned[1].id));
            click(&ctx, &mut app, "Select linked space");
            assert_eq!(app.host.document.editor.selected, Some(zone.id));
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(app.host.document.editor.project, baseline);
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::ALT)],
            );
            assert_eq!(app.host.document.editor.project, created);
            choose(&ctx, &mut app, owned[1].id);
            click(&ctx, &mut app, "Select linked space");
            click(&ctx, &mut app, "Show space quantity figure");
            assert_eq!(app.view, View::Figure(figure.id));
            click(&ctx, &mut app, "Preview figure update");
            click(&ctx, &mut app, "Confirm design change");
            let updated = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .iter()
                .find(|d| d.id == figure.id)
                .unwrap();
            assert_eq!(updated.generation, figure.generation + 1);
            assert_eq!(
                updated.status(&app.host.document.editor.project),
                Status::Current
            );
            let output = root.join("continued.openarchi.json");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let old = app.file_dialog_path.clone().unwrap();
            click(&ctx, &mut app, &old);
            layers_frame_output(
                &ctx,
                &mut app,
                vec![
                    key(Key::A, Modifiers::COMMAND),
                    Event::Text(output.display().to_string()),
                ],
            );
            click(&ctx, &mut app, "Save");
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let reopened = storage::load(&output).unwrap();
            assert_eq!(reopened.elements, app.host.document.editor.project.elements);
            assert_eq!(
                reopened.design_process,
                app.host.document.editor.project.design_process
            );
            assert_eq!(
                reopened.drawings.diagrams,
                app.host.document.editor.project.drawings.diagrams
            );
            assert!(!app.show_right);
            assert_eq!(std::fs::read(&input).unwrap(), input_bytes);
            let reread = Editor::from_stored_document(&output).unwrap();
            for (saved, original) in reread
                .project
                .drawings
                .pages
                .iter()
                .zip(&app.host.document.editor.project.drawings.pages)
            {
                let bytes = std::fs::read(&saved.path).unwrap();
                assert_eq!(bytes, std::fs::read(&original.path).unwrap());
                assert_eq!(crate::drawings::digest(&bytes), original.sha256);
            }
            if destination.is_none() {
                std::fs::remove_dir_all(parent).unwrap();
            }
        }
    }
}
