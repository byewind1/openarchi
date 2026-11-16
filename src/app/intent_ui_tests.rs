mod intent_product_path {
    use super::*;
    use serde_json::{Value, json};
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixStream,
        path::{Path, PathBuf},
        thread,
        time::{Duration, Instant},
    };
    fn rpc(path: &Path, method: &str, params: Option<Value>) -> Value {
        let mut stream = UnixStream::connect(path).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(8)))
            .unwrap();
        let mut message = json!({"method":method});
        if let Some(params) = params {
            message["params"] = params;
        }
        writeln!(stream, "{message}").unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }
    fn button(ctx: &Context, app: &mut AtelierApp, key: &str) {
        reveal_manual_control(ctx, app, key);
        let mut point = None;
        for _ in 0..6 {
            let frame = layers_frame_output(ctx, app, vec![]);
            point = frame.shapes.iter().find_map(|shape| {
                text_center(std::slice::from_ref(shape), app.language.translate(key))
                    .filter(|p| shape.clip_rect.contains(*p))
            });
            if point.is_some() {
                break;
            }
        }
        let position = point.unwrap_or_else(|| {
            let frame = layers_frame_output(ctx, app, vec![]);
            let mut texts = vec![];
            collect_text(&frame.shapes, &mut texts);
            panic!(
                "visible control {key}; {}; painted={:?}",
                app.process_ui.error, texts
            )
        });
        layers_click(ctx, app, position);
    }
    fn fixture_plan(page: &crate::drawings::SourcePage) -> Value {
        let seed = |name: &str, x: f64, w: f64| json!({"name":name,"spec":{"purpose":"controlled concept, not approved","outline":[{"x":x,"y":30.},{"x":x+w,"y":30.},{"x":x+w,"y":34.},{"x":x,"y":34.}],"holes":[],"height":3.,"bottom_offset":0.,"label_position":null}});
        json!({"task_id":storage::new_document_identity(),"authorship":"public typed fixture, not AI interpretation","draft":{"name":"Intent product path","question":"Create editable concepts with unknown dimensions; preserve previous work","expected_result":"two editable space candidates","images":[{"page":page.id,"sha256":page.sha256}],"claims":[{"kind":"design_completion","text":"Test rooms and scale are explicitly authored assumptions"}],"geometry_assumptions":"All sizes/height/placement assumed; no survey/code conclusion","statements":[],"candidate_names":["Intent one","Intent two"],"candidates":[[seed("Room",30.,6.),seed("Shared",36.,3.)],[seed("Room",30.,7.),seed("Shared",37.,2.)]],"story":0,"layer":0}})
    }
    pub(super) fn fixture_mass_plan(page: &crate::drawings::SourcePage) -> Value {
        let mut plan = fixture_plan(page);
        plan["draft"]["name"] = json!("Mass intent product path");
        plan["draft"]["expected_result"] = json!("two editable concept mass candidates");
        for group in plan["draft"]["candidates"].as_array_mut().unwrap() {
            for (index, seed) in group.as_array_mut().unwrap().iter_mut().enumerate() {
                seed["name"] = json!(format!("Mass {}", index + 1));
                seed["spec"].as_object_mut().unwrap().remove("purpose");
                seed["spec"]
                    .as_object_mut()
                    .unwrap()
                    .remove("label_position");
                seed["material"] = json!("plaster");
            }
        }
        plan
    }
    #[test]
    fn v4_spatial_intent_default_agent_entry_and_continuous_canvas_save_reopen_bilingual() {
        product_path(false);
    }
    #[test]
    fn v4_mass_intent_default_agent_entry_and_continuous_canvas_save_reopen_bilingual() {
        product_path(true);
    }
    fn product_path(mass: bool) {
        let prefix = if mass {
            "OPENARCHI_MASS_INTENT_AGENT"
        } else {
            "OPENARCHI_INTENT_AGENT"
        };
        let prepare_method = if mass {
            "prepare_mass_intent"
        } else {
            "prepare_spatial_intent"
        };
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let runtime = std::env::var_os(format!("{prefix}_INPUT")).map(PathBuf::from);
            if runtime.is_some() && language == UiLanguage::SimplifiedChinese {
                continue;
            }
            let (ctx, mut app) = setup();
            ctx.global_style_mut(|s| s.animation_time = 0.);
            app.language = language;
            let scratch = std::env::temp_dir()
                .join(format!("oa-if-{}", &storage::new_document_identity()[..12]));
            std::fs::create_dir(&scratch).unwrap();
            let root = if runtime.is_some() {
                let root = PathBuf::from(
                    std::env::var_os(format!("{prefix}_OUTPUT"))
                        .expect("explicit new output required"),
                );
                std::fs::create_dir(&root)
                    .expect("inspect existing output; never replay unknown effects");
                root
            } else {
                scratch.join("case")
            };
            std::fs::create_dir_all(&root).unwrap();
            if let Some(input) = &runtime {
                app.host.document.editor = Editor::from_stored_document(input).unwrap();
                let repository = input.parent().unwrap().join("repository");
                app.source_ui.repository =
                    Some(crate::project_repository::Repository::open(&repository).unwrap());
            } else {
                let mut project = storage::synthetic_building_fixture();
                let image = root.join("fixture.png");
                image::RgbaImage::from_pixel(40, 30, image::Rgba([220, 205, 170, 255]))
                    .save(&image)
                    .unwrap();
                project
                    .drawings
                    .pages
                    .push(crate::drawings::import(&image, &root.join("assets"), 1).unwrap());
                app.host.document.editor = Editor::new(project);
                let source_ids = app
                    .host
                    .document
                    .editor
                    .project
                    .elements
                    .iter()
                    .filter(|e| e.story == 0 && matches!(e.shape, Shape::Zone { .. }))
                    .map(|e| e.id)
                    .collect();
                let proposal = crate::design_process::service::prepare_create(
                    &app.host.document.editor,
                    &storage::new_document_identity(),
                    crate::design_process::service::Draft {
                        name: "Prior project exploration".into(),
                        question: "Retain this context".into(),
                        statements: vec![],
                        candidate_names: ["Prior A".into(), "Prior B".into()],
                        source_ids,
                        entry: crate::design_process::Entry::ManualInterface,
                    },
                )
                .unwrap();
                app.host
                    .document
                    .editor
                    .commit_change(&storage::new_document_identity(), &proposal)
                    .unwrap();
            }
            let input_copy = root.join("input-copy.openarchi.json");
            storage::save_document(
                &app.host.document.editor.project,
                &input_copy,
                app.host.document.editor.project_identity(),
            )
            .unwrap();
            app.host.document.editor = Editor::from_stored_document(&input_copy).unwrap();
            app.switch_work_mode(crate::workbench::WorkMode::SiteConcept);
            app.activate_named_tool("Arrow");
            app.view = View::Plan;
            app.fit();
            let previous = app
                .host
                .document
                .editor
                .project
                .design_process
                .task()
                .unwrap()
                .clone();
            let old_space = previous.candidates[0].initial_zone_ids[2];
            let old_point = match &app
                .host
                .document
                .editor
                .project
                .elements
                .iter()
                .find(|e| e.id == old_space)
                .unwrap()
                .shape
            {
                Shape::Zone { spec } => spec.label_point(),
                Shape::ConceptMass { spec } => Point::new(
                    spec.outline.iter().map(|p| p.x).sum::<f64>() / spec.outline.len() as f64,
                    spec.outline.iter().map(|p| p.y).sum::<f64>() / spec.outline.len() as f64,
                ),
                _ => unreachable!(),
            };
            layers_frame_output(&ctx, &mut app, vec![]);
            layers_frame_output(&ctx, &mut app, vec![]);
            let canvas = app.workbench.panel_rects["view.canvas"];
            let point = app.screen(canvas, old_point);
            layers_click(&ctx, &mut app, point);
            assert_eq!(app.host.document.editor.selection_ids(), vec![old_space]);
            let old_camera = (app.plan_center, app.plan_scale);
            let original = app.host.document.editor.project.clone();
            let path = scratch.join("ipc/app.sock");
            app.automation = Some(
                crate::automation_ipc::Endpoint::bind_for_host(
                    path.clone(),
                    true,
                    Some(root.join("journal")),
                    &mut app.host,
                )
                .unwrap(),
            );
            let plan = if runtime.is_some() {
                serde_json::from_slice(
                    &std::fs::read(
                        std::env::var_os(format!("{prefix}_PLAN"))
                            .expect("explicit Agent plan required"),
                    )
                    .unwrap(),
                )
                .unwrap()
            } else if mass {
                fixture_mass_plan(&original.drawings.pages[0])
            } else {
                fixture_plan(&original.drawings.pages[0])
            };
            std::fs::write(root.join("operation-plan.json"),serde_json::to_vec_pretty(&json!({"input":runtime,"input_copy":input_copy,"plan":plan,"target":"protected same-project copy only","entry":"default shared GUI Endpoint + ordinary egui controls; no TaskGuard","source":"in-flight current source; bind final receipt separately","recovery":"read original requests/journal/output; never replay unknown"})).unwrap()).unwrap();
            let socket = path.clone();
            let output = root.clone();
            let plan_client = plan.clone();
            let is_agent = runtime.is_some();
            let client = thread::spawn(move || {
                if is_agent {
                    let plan_path = output.join("agent-plan.json");
                    std::fs::write(&plan_path, serde_json::to_vec_pretty(&plan_client).unwrap())
                        .unwrap();
                    let python = std::env::var_os(format!("{prefix}_PYTHON"))
                        .expect("explicit existing SDK Python required");
                    let log = std::fs::File::create(output.join("agent-sdk.log")).unwrap();
                    let error = log.try_clone().unwrap();
                    let status = std::process::Command::new(python)
                        .args(["tools/mcp-adapter/drive_spatial_intent.py", "--socket"])
                        .arg(&socket)
                        .arg("--geometry-kind")
                        .arg(if mass { "mass" } else { "space" })
                        .arg("--plan")
                        .arg(&plan_path)
                        .arg("--output")
                        .arg(output.join("agent-sdk"))
                        .stdout(log)
                        .stderr(error)
                        .status()
                        .unwrap();
                    assert!(
                        status.success(),
                        "inspect agent-sdk.log and original journal before retry"
                    );
                } else {
                    let target = rpc(&socket, "get_target", None)["result"]["target"].clone();
                    let context =
                        rpc(&socket, "get_context", Some(target.clone()))["result"].clone();
                    let request = json!({"target":target,"expected_revision":context["revision"],"expected_context_revision":context["context_revision"],"expected_sources_revision":context["project_sources"]["sources_revision"],"task_id":plan_client["task_id"],"draft":plan_client["draft"]});
                    let first = rpc(&socket, prepare_method, Some(request.clone()));
                    assert!(first.get("error").is_none(), "{first}");
                    let cancel = rpc(
                        &socket,
                        "cancel_operation",
                        Some(json!({"target":target,"preview_id":first["result"]["preview_id"]})),
                    );
                    assert!(cancel.get("error").is_none());
                    let second = rpc(&socket, prepare_method, Some(request));
                    assert!(second.get("error").is_none(), "{second}");
                    let committed = rpc(
                        &socket,
                        "commit_change",
                        Some(
                            json!({"target":target,"preview_id":second["result"]["preview_id"],"request_id":plan_client["task_id"]}),
                        ),
                    );
                    assert_eq!(committed["result"]["status"], "committed", "{committed}");
                    let read = rpc(
                        &socket,
                        "get_operation",
                        Some(json!({"target":target,"request_id":plan_client["task_id"]})),
                    );
                    assert_eq!(read["result"]["status"], "committed");
                }
            });
            let start = Instant::now();
            while !client.is_finished() {
                app.poll_automation(&ctx);
                layers_frame_output(&ctx, &mut app, vec![]);
                assert!(
                    start.elapsed() < Duration::from_secs(120),
                    "retain original output/operation before resuming"
                );
                thread::sleep(Duration::from_millis(2));
            }
            client.join().unwrap();
            let task = app
                .host
                .document
                .editor
                .project
                .design_process
                .task()
                .unwrap()
                .clone();
            assert_eq!(task.id, plan["task_id"].as_str().unwrap());
            assert_eq!(
                &app.host.document.editor.project.elements[..original.elements.len()],
                original.elements.as_slice()
            );
            assert!(
                app.host
                    .document
                    .editor
                    .selection_ids()
                    .iter()
                    .all(|id| task.candidates[0].element_ids().contains(id))
            );
            if runtime.is_some() {
                assert!(!app.host.document.editor.redo.is_empty());
                button(&ctx, &mut app, "Redo");
                assert!(app.host.document.editor.redo.is_empty());
            }
            let new_camera = (app.plan_center, app.plan_scale);
            let selected = task.candidates[0].element_ids()[0];
            let target_point = match &app
                .host
                .document
                .editor
                .project
                .elements
                .iter()
                .find(|e| e.id == selected)
                .unwrap()
                .shape
            {
                Shape::Zone { spec } => spec.label_point(),
                Shape::ConceptMass { spec } => Point::new(
                    spec.outline.iter().map(|p| p.x).sum::<f64>() / spec.outline.len() as f64,
                    spec.outline.iter().map(|p| p.y).sum::<f64>() / spec.outline.len() as f64,
                ),
                _ => unreachable!(),
            };
            layers_frame_output(&ctx, &mut app, vec![]);
            let rect = app.workbench.panel_rects["view.canvas"];
            let point = app.screen(rect, target_point);
            assert!(
                rect.contains(point),
                "new editable result must be visible without menu context transfer"
            );
            layers_click(&ctx, &mut app, point);
            assert_eq!(app.host.document.editor.selection_ids(), vec![selected]);
            if mass {
                button(&ctx, &mut app, "Compare options…");
                let frame = layers_frame_output(&ctx, &mut app, vec![]);
                let mut texts = vec![];
                collect_text(&frame.shapes, &mut texts);
                assert!(
                    texts
                        .iter()
                        .any(|s| s == app.language.translate("Mass footprint sum"))
                );
                assert!(
                    texts
                        .iter()
                        .any(|s| s == app.language.translate("Concept envelope volume sum"))
                );
                button(&ctx, &mut app, "Continue in canvas");
            }
            // Same task strip, existing preview/confirm surface, no extra task-management panel.
            button(&ctx, &mut app, &task.name);
            button(&ctx, &mut app, &previous.name);
            button(&ctx, &mut app, "Confirm design change");
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .design_process
                    .current_task
                    .as_deref(),
                Some(previous.id.as_str())
            );
            assert_eq!(app.host.document.editor.selection_ids(), vec![old_space]);
            assert_eq!((app.plan_center, app.plan_scale), old_camera);
            button(&ctx, &mut app, &previous.name);
            button(&ctx, &mut app, &task.name);
            button(&ctx, &mut app, "Confirm design change");
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .design_process
                    .current_task
                    .as_deref(),
                Some(task.id.as_str())
            );
            assert_eq!(app.host.document.editor.selection_ids(), vec![selected]);
            assert_eq!((app.plan_center, app.plan_scale), new_camera);
            if app.process_ui.open {
                button(&ctx, &mut app, "Continue in canvas");
            }
            let expected = app.host.document.editor.project.clone();
            let save = root.join("continued.openarchi.json");
            app.path = save.to_string_lossy().into_owned();
            app.path_confirmed = false;
            app.file_baseline = None;
            button(&ctx, &mut app, "Save project");
            button(&ctx, &mut app, "Save");
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let reopened = Editor::from_stored_document(&save).unwrap();
            assert_eq!(reopened.project.elements, expected.elements);
            assert_eq!(reopened.project.design_process, expected.design_process);
            for page in &reopened.project.drawings.pages {
                crate::drawings::verified_bytes(page).unwrap();
            }
            std::fs::write(root.join("walkthrough.json"),serde_json::to_vec_pretty(&json!({"language":format!("{language:?}"),"input":input_copy,"output":save,"task_id":task.id,"all_old_elements_preserved":true,"new_candidate_visible_and_pickable":true,"prior_selection_camera_restored":true,"new_task_selection_camera_restored":true,"save_reopen_elements_tasks_sources":true,"new_task_management_panels":0,"task_context_modal":"existing optional comparison/proposal surface","entry":"actual default GUI poll IPC + actual egui pointer controls; no native OS/GPU claim","Agent_execution":if runtime.is_some(){"external current-agent plan through actual SDK; see agent-sdk; controlled feedback, independent verdict pending"}else{"public typed fixture; not AI"},"native_desktop":"not_run","package":"older L1; not current source","mass_task":mass,"remaining":"actual user feedback/independent intent judgment, multi-source limited update, full authorization/portable operation recovery and original V4 gates"})).unwrap()).unwrap();
            println!(
                "INTENT_PRODUCT_WALK={}",
                root.join("walkthrough.json").display()
            );
            drop(app.automation.take());
            if runtime.is_none() {
                std::fs::remove_dir_all(&root).unwrap();
            }
            std::fs::remove_dir_all(scratch).unwrap();
        }
    }
    fn drive_request(
        ctx: &Context,
        app: &mut AtelierApp,
        path: &Path,
        method: &str,
        params: Option<Value>,
    ) -> Value {
        let path = path.to_owned();
        let method = method.to_owned();
        let thread = thread::spawn(move || rpc(&path, &method, params));
        let start = Instant::now();
        while !thread.is_finished() {
            app.poll_automation(ctx);
            assert!(start.elapsed() < Duration::from_secs(8));
            std::thread::sleep(Duration::from_millis(2));
        }
        thread.join().unwrap()
    }
    #[test]
    fn v4_spatial_default_gui_blocks_pending_inputs_and_fresh_reads_never_renew_after_user_edit() {
        default_guard(false);
    }
    #[test]
    fn v4_mass_default_gui_blocks_pending_inputs_and_fresh_reads_never_renew_after_user_edit() {
        default_guard(true);
    }
    fn default_guard(mass: bool) {
        let method = if mass {
            "prepare_mass_intent"
        } else {
            "prepare_spatial_intent"
        };
        let (ctx, mut app) = setup();
        let root =
            std::env::temp_dir().join(format!("oa-ib-{}", &storage::new_document_identity()[..12]));
        std::fs::create_dir(&root).unwrap();
        let image = root.join("input.png");
        image::RgbaImage::from_pixel(20, 20, image::Rgba([210, 205, 190, 255]))
            .save(&image)
            .unwrap();
        let mut project = storage::synthetic_building_fixture();
        project
            .drawings
            .pages
            .push(crate::drawings::import(&image, &root.join("assets"), 1).unwrap());
        app.host.document.editor = Editor::new(project);
        let before = app.host.document.editor.project.clone();
        let plan = if mass {
            fixture_mass_plan(&before.drawings.pages[0])
        } else {
            fixture_plan(&before.drawings.pages[0])
        };
        let path = root.join("ipc/app.sock");
        app.automation = Some(
            crate::automation_ipc::Endpoint::bind_for_host(
                path.clone(),
                true,
                Some(root.join("journal")),
                &mut app.host,
            )
            .unwrap(),
        );
        let target =
            drive_request(&ctx, &mut app, &path, "get_target", None)["result"]["target"].clone();
        let arguments = |context: &Value| json!({"target":target,"expected_revision":context["revision"],"expected_context_revision":context["context_revision"],"expected_sources_revision":context["project_sources"]["sources_revision"],"task_id":plan["task_id"],"draft":plan["draft"]});
        // App-owned pointer state is absent from Editor/revision; the actual default GUI poll supplies it.
        app.marquee_points.push(Point::new(1., 1.));
        let context =
            drive_request(&ctx, &mut app, &path, "get_context", Some(target.clone()))["result"]
                .clone();
        assert_eq!(
            context["write_authorization"]["status"],
            "interaction_blocked"
        );
        let blocked = drive_request(&ctx, &mut app, &path, method, Some(arguments(&context)));
        assert_eq!(blocked["error"]["code"], "USER_INTERACTION_ACTIVE");
        assert_eq!(app.host.document.editor.project, before);
        app.marquee_points.clear();
        let element = before.elements[0].clone();
        app.host.document.editor.set_selection(vec![element.id]);
        app.draft_source = Some(element.clone());
        let mut pending = element;
        pending.name.push_str(" pending user property");
        app.draft = Some(pending.clone());
        let context =
            drive_request(&ctx, &mut app, &path, "get_context", Some(target.clone()))["result"]
                .clone();
        let blocked = drive_request(&ctx, &mut app, &path, method, Some(arguments(&context)));
        assert_eq!(blocked["error"]["code"], "USER_INTERACTION_ACTIVE");
        assert_eq!(app.draft, Some(pending));
        assert_eq!(app.host.document.editor.project, before);
        // The injected draft is canceled in the fixture; this temporary block never grants authority.
        app.draft = app.draft_source.clone();
        let context =
            drive_request(&ctx, &mut app, &path, "get_context", Some(target.clone()))["result"]
                .clone();
        assert_eq!(context["write_authorization"]["status"], "authorized");
        let ready = drive_request(&ctx, &mut app, &path, method, Some(arguments(&context)));
        assert!(ready.get("error").is_none(), "{ready}");
        let canceled = drive_request(
            &ctx,
            &mut app,
            &path,
            "cancel_operation",
            Some(json!({"target":target,"preview_id":ready["result"]["preview_id"]})),
        );
        assert!(canceled.get("error").is_none());
        app.host
            .document
            .editor
            .apply(crate::command::ModelCommand::MoveElements {
                ids: vec![before.elements[0].id],
                offset: Point::new(0.1, 0.),
            })
            .unwrap();
        let user = app.host.document.editor.project.clone();
        let context =
            drive_request(&ctx, &mut app, &path, "get_context", Some(target.clone()))["result"]
                .clone();
        assert_eq!(context["write_authorization"]["status"], "suspended");
        let rejected = drive_request(&ctx, &mut app, &path, method, Some(arguments(&context)));
        assert_eq!(rejected["error"]["code"], "WRITE_AUTHORIZATION_SUSPENDED");
        assert_eq!(app.host.document.editor.project, user);
        assert_eq!(app.host.document.editor.undo.len(), 1);
        drop(app.automation.take());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn v4_scoped_task_update_default_entry_history_undo_and_save_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let runtime = std::env::var_os("OPENARCHI_TASK_UPDATE_INPUT").map(PathBuf::from);
            if runtime.is_some() && language == UiLanguage::SimplifiedChinese {
                continue;
            }
            let (ctx, mut app) = setup();
            ctx.global_style_mut(|style| style.animation_time = 0.);
            app.language = language;
            let scratch = std::env::temp_dir()
                .join(format!("oa-tu-{}", &storage::new_document_identity()[..12]));
            std::fs::create_dir(&scratch).unwrap();
            let root = if runtime.is_some() {
                let root = PathBuf::from(
                    std::env::var_os("OPENARCHI_TASK_UPDATE_OUTPUT").expect("new output required"),
                );
                std::fs::create_dir(&root)
                    .expect("inspect existing result/operation; do not replay unknown");
                root
            } else {
                scratch.join("case")
            };
            std::fs::create_dir_all(&root).unwrap();
            if let Some(input) = &runtime {
                app.host.document.editor = Editor::from_stored_document(input).unwrap();
                let repository = PathBuf::from(
                    std::env::var_os("OPENARCHI_TASK_UPDATE_REPOSITORY")
                        .expect("explicit repository required"),
                );
                app.source_ui.repository =
                    Some(crate::project_repository::Repository::open(&repository).unwrap());
            } else {
                let mut project = storage::synthetic_building_fixture();
                let image = root.join("fixture.png");
                image::RgbaImage::from_pixel(40, 30, image::Rgba([180, 205, 220, 255]))
                    .save(&image)
                    .unwrap();
                project
                    .drawings
                    .pages
                    .push(crate::drawings::import(&image, &root.join("assets"), 1).unwrap());
                app.host.document.editor = Editor::new(project);
                let plan = fixture_mass_plan(&app.host.document.editor.project.drawings.pages[0]);
                let wire: crate::automation::intent::MassDraft =
                    serde_json::from_value(plan["draft"].clone()).unwrap();
                let prepared = crate::design_process::intent::prepare_mass(
                    &app.host.document.editor,
                    plan["task_id"].as_str().unwrap(),
                    wire.lower(),
                )
                .unwrap();
                app.host
                    .document
                    .editor
                    .commit_change(plan["task_id"].as_str().unwrap(), &prepared)
                    .unwrap();
            }
            let input_copy = root.join("input-copy.openarchi.json");
            storage::save_document(
                &app.host.document.editor.project,
                &input_copy,
                app.host.document.editor.project_identity(),
            )
            .unwrap();
            app.host.document.editor = Editor::from_stored_document(&input_copy).unwrap();
            app.switch_work_mode(crate::workbench::WorkMode::SiteConcept);
            app.activate_named_tool("Arrow");
            app.view = View::Plan;
            app.fit();
            let original = app.host.document.editor.project.clone();
            let task = original.design_process.task().unwrap().clone();
            let target_id = task.candidates[0].element_ids()[1];
            let socket = scratch.join("ipc/app.sock");
            app.automation = Some(
                crate::automation_ipc::Endpoint::bind_for_host(
                    socket.clone(),
                    true,
                    Some(root.join("journal")),
                    &mut app.host,
                )
                .unwrap(),
            );
            std::fs::write(root.join("operation-plan.json"),serde_json::to_vec_pretty(&json!({"input":runtime,"input_copy":input_copy,"task":task.id,"output":"new protected copy only","entry":"default GUI Endpoint + actual egui controls; initial project/repository fixture loading is explicit","recovery":"requests persisted before effects; inspect original IDs, no replay"})).unwrap()).unwrap();
            let client_root = root.clone();
            let client_socket = socket.clone();
            let is_agent = runtime.is_some();
            let old = original
                .elements
                .iter()
                .find(|e| e.id == target_id)
                .unwrap()
                .clone();
            let client_task = task.id.clone();
            let page = original.drawings.pages[0].clone();
            let client = thread::spawn(move || {
                if is_agent {
                    let python = std::env::var_os("OPENARCHI_TASK_UPDATE_PYTHON")
                        .expect("existing SDK Python required");
                    let plan = std::env::var_os("OPENARCHI_TASK_UPDATE_PLAN")
                        .expect("explicit Agent plan required");
                    let log = std::fs::File::create(client_root.join("agent-sdk.log")).unwrap();
                    let err = log.try_clone().unwrap();
                    let status = std::process::Command::new(python)
                        .args(["tools/mcp-adapter/drive_task_update.py", "--socket"])
                        .arg(&client_socket)
                        .arg("--plan")
                        .arg(plan)
                        .arg("--output")
                        .arg(client_root.join("agent-sdk"))
                        .stdout(log)
                        .stderr(err)
                        .status()
                        .unwrap();
                    assert!(
                        status.success(),
                        "inspect original agent-sdk/journal; no replay"
                    );
                } else {
                    let target =
                        rpc(&client_socket, "get_target", None)["result"]["target"].clone();
                    let context =
                        rpc(&client_socket, "get_context", Some(target.clone()))["result"].clone();
                    let read = rpc(
                        &client_socket,
                        "query_task_updates",
                        Some(
                            json!({"target":target,"expected_revision":context["revision"],"offset":0,"limit":1}),
                        ),
                    );
                    let mut spec = serde_json::to_value(&old.shape).unwrap()["spec"].clone();
                    spec["height"] = json!(spec["height"].as_f64().unwrap() + 0.2);
                    let uid = storage::new_document_identity();
                    let arguments = json!({"target":target,"expected_revision":context["revision"],"expected_context_revision":context["context_revision"],"expected_sources_revision":context["project_sources"]["sources_revision"],"task_id":client_task,"update_id":uid,"draft":{"instruction":"Keep existing work; adjust the declared current mass only","expected_result":"editable local update with project-owned source history","sources":[{"origin":"drawing","page":page.id,"sha256":page.sha256}],"claims":[{"kind":"design_completion","text":"Controlled fixture height, not actual user feedback"}],"statements":[],"expected_comparison":read["result"]["comparison"]["fingerprint"],"updates":[{"id":old.id,"geometry":{"kind":"mass","spec":spec}}]}});
                    std::fs::write(
                        client_root.join("fixture-request.json"),
                        serde_json::to_vec_pretty(&arguments).unwrap(),
                    )
                    .unwrap();
                    let preview = rpc(
                        &client_socket,
                        "prepare_task_update",
                        Some(arguments.clone()),
                    );
                    assert!(preview.get("error").is_none(), "{preview}");
                    let cancel = rpc(
                        &client_socket,
                        "cancel_operation",
                        Some(json!({"target":target,"preview_id":preview["result"]["preview_id"]})),
                    );
                    assert!(cancel.get("error").is_none());
                    let preview = rpc(&client_socket, "prepare_task_update", Some(arguments));
                    assert!(preview.get("error").is_none(), "{preview}");
                    let wrong = rpc(
                        &client_socket,
                        "commit_change",
                        Some(
                            json!({"target":target,"preview_id":preview["result"]["preview_id"],"request_id":storage::new_document_identity()}),
                        ),
                    );
                    assert_eq!(wrong["error"]["code"], "TASK_UPDATE_REQUEST_ID_MISMATCH");
                    let committed = rpc(
                        &client_socket,
                        "commit_change",
                        Some(
                            json!({"target":target,"preview_id":preview["result"]["preview_id"],"request_id":uid}),
                        ),
                    );
                    assert_eq!(committed["result"]["status"], "committed", "{committed}");
                    let context =
                        rpc(&client_socket, "get_context", Some(target.clone()))["result"].clone();
                    let query = rpc(
                        &client_socket,
                        "query_task_updates",
                        Some(
                            json!({"target":target,"expected_revision":context["revision"],"offset":0,"limit":1}),
                        ),
                    );
                    assert_eq!(
                        query["result"]["records"][0]["current_effect"],
                        "Task update matches current elements"
                    );
                    let undone = rpc(
                        &client_socket,
                        "undo_operation",
                        Some(json!({"target":target,"request_id":uid})),
                    );
                    assert!(undone.get("error").is_none(), "{undone}");
                }
            });
            let started = Instant::now();
            while !client.is_finished() {
                app.poll_automation(&ctx);
                let _ = layers_frame_output(&ctx, &mut app, vec![]);
                assert!(
                    started.elapsed() < Duration::from_secs(90),
                    "inspect original handles/result before any retry"
                );
                thread::sleep(Duration::from_millis(2));
            }
            client.join().unwrap();
            assert_eq!(app.host.document.editor.project.elements, original.elements);
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .design_process
                    .updates
                    .len(),
                original.design_process.updates.len() + 1
            );
            button(&ctx, &mut app, "Redo");
            let changed = app.host.document.editor.project.clone();
            let update = changed.design_process.updates.last().unwrap().clone();
            assert_eq!(
                update.effect(&changed),
                "Task update matches current elements"
            );
            let api_target =
                drive_request(&ctx, &mut app, &socket, "get_target", None)["result"]["target"]
                    .clone();
            let context = drive_request(
                &ctx,
                &mut app,
                &socket,
                "get_context",
                Some(api_target.clone()),
            )["result"]
                .clone();
            assert_eq!(context["write_authorization"]["status"], "suspended");
            let query = drive_request(
                &ctx,
                &mut app,
                &socket,
                "query_task_updates",
                Some(
                    json!({"target":api_target,"expected_revision":context["revision"],"offset":0,"limit":1}),
                ),
            );
            let mut spec = serde_json::to_value(
                changed
                    .elements
                    .iter()
                    .find(|e| e.id == update.affected_ids[0])
                    .unwrap(),
            )
            .unwrap()["shape"]["spec"]
                .clone();
            spec["height"] = json!(spec["height"].as_f64().unwrap() + 0.1);
            let blocked = drive_request(
                &ctx,
                &mut app,
                &socket,
                "prepare_task_update",
                Some(
                    json!({"target":api_target,"expected_revision":context["revision"],"expected_context_revision":context["context_revision"],"expected_sources_revision":context["project_sources"]["sources_revision"],"task_id":task.id,"update_id":storage::new_document_identity(),"draft":{"instruction":"must not silently renew after native Redo","expected_result":"no write","sources":update.sources,"claims":update.claims,"statements":[],"expected_comparison":query["result"]["comparison"]["fingerprint"],"updates":[{"id":update.affected_ids[0],"geometry":{"kind":"mass","spec":spec}}]}}),
                ),
            );
            assert_eq!(blocked["error"]["code"], "WRITE_AUTHORIZATION_SUSPENDED");
            assert_eq!(app.host.document.editor.project, changed);
            assert_eq!(changed.elements.len(), original.elements.len());
            assert!(
                original
                    .elements
                    .iter()
                    .filter(|e| !update.affected_ids.contains(&e.id))
                    .all(|e| changed.elements.contains(e))
            );
            button(&ctx, &mut app, "Compare options…");
            button(&ctx, &mut app, "Task updates and sources");
            let frame = layers_frame_output(&ctx, &mut app, vec![]);
            let mut text = vec![];
            collect_text(&frame.shapes, &mut text);
            assert!(
                text.iter().any(|x| x.contains(&update.instruction)),
                "{text:?}"
            );
            button(&ctx, &mut app, "Continue in canvas");
            // Normal file dialog pointer/text entry; no direct draft/path assignment for the saved result.
            let save = root.join("continued.openarchi.json");
            button(&ctx, &mut app, "Save project");
            let path_text = app.path.clone();
            let frame = layers_frame_output(&ctx, &mut app, vec![]);
            let point = text_center(&frame.shapes, &path_text).expect("visible file path control");
            layers_click(&ctx, &mut app, point);
            let _ = layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::A, Modifiers::MAC_CMD | Modifiers::COMMAND)],
            );
            let _ = layers_frame_output(
                &ctx,
                &mut app,
                vec![Event::Text(save.to_string_lossy().into_owned())],
            );
            button(&ctx, &mut app, "Save");
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let reopened = Editor::from_stored_document(&save).unwrap();
            assert_eq!(reopened.project.elements, changed.elements);
            assert_eq!(reopened.project.design_process, changed.design_process);
            for page in &reopened.project.drawings.pages {
                crate::drawings::verified_bytes(page).unwrap();
            }
            std::fs::write(root.join("walkthrough.json"),serde_json::to_vec_pretty(&json!({"input":input_copy,"output":save,"language":format!("{language:?}"),"update_id":update.id,"affected_ids":update.affected_ids,"input_elements":original.elements.len(),"output_elements":changed.elements.len(),"entry":"actual default GUI Endpoint + SDK or fixture RPC; actual egui Redo/history/file text Save","initial_state":"protected project/repository fixture loaded explicitly; not native OS File/Open proof","package":"current source only; prior3BACB577 bundle predates format10","native_desktop":"not_run"})).unwrap()).unwrap();
            println!(
                "TASK_UPDATE_WALK={}",
                root.join("walkthrough.json").display()
            );
            drop(app.automation.take());
            if runtime.is_none() {
                std::fs::remove_dir_all(&root).unwrap();
            }
            std::fs::remove_dir_all(scratch).unwrap();
        }
    }
}
