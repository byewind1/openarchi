// Included in the existing product-path harness; no fixture-only permission fence.

fn assert_discussion_layout_and_type(ctx: &Context, app: &mut AtelierApp) {
    for _ in 0..8 {
        layers_frame_output(ctx, app, vec![]);
    }
    let out = layers_frame_output(ctx, app, vec![]);
    let label = app.language.translate("NAVIGATOR");
    let (clip, text) = out
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == label => Some((s.clip_rect, t)),
            _ => None,
        })
        .expect("normal inspector heading is painted");
    let text_rect = Rect::from_min_size(text.pos, text.galley.size());
    let panel = app.workbench.panel_rects["workspace.navigator"];
    eprintln!(
        "layout {:?}: heading {:?}, clip {:?}, panel {:?}",
        app.language, text_rect, clip, panel
    );
    assert!(
        panel.contains_rect(text_rect),
        "Inspector heading is behind the canvas: {text_rect:?} / {panel:?}"
    );
    assert!(
        panel.right() <= 1400.5,
        "Inspector extends beyond the workspace: {panel:?}"
    );
    assert!(
        clip.contains_rect(text_rect),
        "Inspector heading must also be inside its paint clip"
    );
    let field = ctx
        .data(|d| d.get_temp::<Id>(Id::new("object_research_question")))
        .unwrap();
    let field_rect = ctx.read_response(field).unwrap().rect;
    let scroll = ctx
        .data(|d| d.get_temp::<Rect>(Id::new("inspector_scroll_rect")))
        .unwrap();
    assert!(
        scroll.contains_rect(field_rect),
        "Explicit object question must be directly editable, without scrolling through project images: {field_rect:?} / {scroll:?}"
    );
    layers_click(ctx, app, field_rect.center());
    layers_frame_output(
        ctx,
        app,
        vec![Event::Text(
            "Keep the protected object and study only this candidate.".into(),
        )],
    );
    assert_eq!(
        app.research_ui.draft.as_ref().unwrap().exchange.question,
        "Keep the protected object and study only this candidate."
    );
}

#[test]
fn long_discussion_history_keeps_native_inspector_and_question_usable() {
    use crate::evidence::{EvidenceEdit, Target, topics};
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app) = setup();
        ctx.enable_accesskit();
        ctx.global_style_mut(|s| s.animation_time = 0.);
        app.language = language;
        // Public fixture establishes a saved history and an initial selected object.
        // The regression itself enters and edits through normal workspace controls.
        app.host.document.editor = Editor::new(crate::storage::startup_example());
        let e = &mut app.host.document.editor;
        let id = e
            .project
            .elements
            .iter()
            .find(|e| matches!(e.shape, Shape::Zone { .. }))
            .unwrap()
            .id;
        let question = if language == UiLanguage::English {
            "Compare the teaching rooms with the courtyard while preserving existing buildings and manual changes. Explain assumptions, uncertain capacity, and the consequences of adjusting only the selected candidate."
        } else {
            "请依据已存任务书与当前方案研究教学空间和院落关系，保留已有建筑与人工修改，说明尺寸假设和容量未知，不改变其他候选或未经核准的条件。"
        }.repeat(3);
        let exchange =
            topics::capture(&e.project, &[Target::Element { id }], question.clone()).unwrap();
        let change = e
            .prepare_change(
                e.revision,
                vec![crate::command::ModelCommand::Evidence {
                    edit: EvidenceEdit::RecordTopic {
                        id: storage::new_document_identity(),
                        exchange,
                        expected_previous: None,
                    },
                }],
            )
            .unwrap();
        e.commit_change("public-discussion-layout-history", &change)
            .unwrap();
        e.set_selection(vec![id]);
        let before = e.project.clone();
        button(&ctx, &mut app, "Discuss selected objects");
        assert_discussion_layout_and_type(&ctx, &mut app);
        assert_eq!(app.host.document.editor.project, before);
        let question_now = app
            .research_ui
            .draft
            .as_ref()
            .unwrap()
            .exchange
            .question
            .clone();
        button(&ctx, &mut app, "Preview task for connected Agent");
        assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
        let out = layers_frame_output(&ctx, &mut app, vec![]);
        let mut text = vec![];
        collect_text(&out.shapes, &mut text);
        assert!(
            text.contains(&question_now),
            "Task consent must show this contribution, not only the old design task question"
        );
        assert_eq!(app.host.document.editor.project, before);
        assert!(!app.host.operations.borrow().enabled());
        button(&ctx, &mut app, "Cancel design proposal");
        assert_eq!(
            app.research_ui.draft.as_ref().unwrap().exchange.question,
            question_now
        );
        assert!(!app.host.operations.borrow().enabled());
        assert_eq!(
            before.evidence.topics.last().unwrap().exchanges[0].question,
            question
        );
    }
}

#[test]
#[ignore = "read-only latest grain input; actual saved long discussion history"]
fn grain_discussion_header_and_input_remain_inside_inspector() {
    let file = PathBuf::from(std::env::var_os("OPENARCHI_OBJECT_TASK_INPUT").unwrap());
    let original = std::fs::read(&file).unwrap();
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app) = setup();
        ctx.enable_accesskit();
        ctx.global_style_mut(|s| s.animation_time = 0.);
        app.language = language;
        open(&ctx, &mut app, &file);
        let p = &app.host.document.editor.project;
        let member = p.design_process.task().unwrap().candidates[0].initial_mass_ids[0];
        let name = p
            .elements
            .iter()
            .find(|e| e.id == member)
            .unwrap()
            .name
            .clone();
        button(&ctx, &mut app, "Select a source mass");
        button(&ctx, &mut app, &format!("#{member} · {name}"));
        button(&ctx, &mut app, "Discuss selected objects");
        assert_discussion_layout_and_type(&ctx, &mut app);
        assert_eq!(std::fs::read(&file).unwrap(), original);
    }
}

fn task_scroll_point(ctx: &Context) -> Pos2 {
    let rect = ctx
        .data(|d| d.get_temp::<Rect>(Id::new("inspector_scroll_rect")))
        .expect("measured current assistant scroll body");
    pos2(rect.right() - 4., rect.top() + 8.)
}
fn task_scroll(ctx: &Context, app: &mut AtelierApp, dy: f32) {
    layers_frame_output(
        ctx,
        app,
        vec![
            Event::PointerMoved(task_scroll_point(ctx)),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                phase: egui::TouchPhase::Move,
                delta: vec2(0., dy),
                modifiers: Modifiers::NONE,
            },
        ],
    );
}
fn task_visible_button(ctx: &Context, app: &mut AtelierApp, label: &str) {
    // Latest-first task history can contain the same question from an older task.
    // Start at the top and advance less than the measured visible height.
    for _ in 0..4 {
        task_scroll(ctx, app, 1000.);
    }
    let mut diagnostic = vec![];
    for _ in 0..128 {
        let out = layers_frame_output(ctx, app, vec![]);
        let mut texts = vec![];
        collect_text(&out.shapes, &mut texts);
        let rect = ctx
            .data(|d| d.get_temp::<Rect>(Id::new("inspector_scroll_rect")))
            .unwrap();
        diagnostic.push(serde_json::json!({"wanted":label,"rect":[rect.min.x,rect.min.y,rect.max.x,rect.max.y],"at":[task_scroll_point(ctx).x,task_scroll_point(ctx).y],"texts":texts.iter().map(|s|serde_json::json!({"text":s,"center":text_center(&out.shapes,s).map(|p|[p.x,p.y])})).collect::<Vec<_>>()}));
        if let Some(at) = text_center(&out.shapes, app.language.translate(label)) {
            let rect = ctx
                .data(|d| d.get_temp::<Rect>(Id::new("inspector_scroll_rect")))
                .unwrap();
            let fixed_parameter_action = matches!(
                label,
                "Preview two-wing parameters" | "Cancel parameter draft"
            ) && ctx.data(|d| {
                d.get_temp::<Rect>(Id::new("parametric_actions_rect"))
                    .is_some_and(|footer| footer.contains(at))
            });
            // This existing action now also has a normal candidate-bar entry.
            let candidate_parameter_entry = label == "Adjust generated wings"
                && out.shapes.iter().any(|s| {
                    s.clip_rect.contains(at)
                        && matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == app.language.translate(label))
                });
            if rect.contains(at) || fixed_parameter_action || candidate_parameter_entry {
                let node = out
                    .platform_output
                    .accesskit_update
                    .as_ref()
                    .unwrap()
                    .nodes
                    .iter()
                    .find(|(_, n)| {
                        n.supports_action(Action::Click)
                            && (n.label() == Some(app.language.translate(label))
                                || n.value() == Some(app.language.translate(label)))
                    })
                    .map(|(id, _)| *id)
                    .expect("painted control has stable accessible identity");
                // Scroll inertia must not turn a measured coordinate into a click on an older identical question.
                action(ctx, app, node, Action::Click, None);
                return;
            }
        }
        task_scroll(ctx, app, -(rect.height() * 0.55).clamp(16., 80.));
    }
    if let Some(root) = std::env::var_os("OPENARCHI_OBJECT_TASK_OUTPUT") {
        std::fs::write(
            PathBuf::from(root).join("button-visibility-diagnostic.json"),
            serde_json::to_vec_pretty(&diagnostic).unwrap(),
        )
        .unwrap();
    }
    eprintln!("final assistant layout: {}", diagnostic.last().unwrap());
    eprintln!(
        "parameter footer {:?}",
        ctx.data(|d| d.get_temp::<Rect>(Id::new("parametric_actions_rect")))
    );
    panic!("Button must be visible and clickable in current assistant: {label}");
}
#[cfg(unix)]
fn task_rpc(
    ctx: &Context,
    app: &mut AtelierApp,
    socket: &std::path::Path,
    root: &std::path::Path,
    request: serde_json::Value,
) -> serde_json::Value {
    use std::io::{BufRead, BufReader, Write};
    let stem = root.join(format!("request-{}", storage::new_document_identity()));
    std::fs::write(
        stem.with_extension("json"),
        serde_json::to_vec_pretty(&request).unwrap(),
    )
    .unwrap();
    let socket = socket.to_owned();
    let client = std::thread::spawn(move || {
        let mut stream = std::os::unix::net::UnixStream::connect(socket).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .unwrap();
        writeln!(stream, "{request}").unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        serde_json::from_str::<serde_json::Value>(&line).unwrap()
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !client.is_finished() {
        app.poll_automation(ctx);
        layers_frame_output(ctx, app, vec![]);
        assert!(
            std::time::Instant::now() < deadline,
            "Unknown response: inspect recorded request; never replay"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let response = client.join().unwrap();
    std::fs::write(
        stem.with_extension("response.json"),
        serde_json::to_vec_pretty(&response).unwrap(),
    )
    .unwrap();
    response
}
#[cfg(unix)]
fn task_edit(
    ctx: &Context,
    app: &mut AtelierApp,
    socket: &std::path::Path,
    root: &std::path::Path,
    edit: crate::evidence::EvidenceEdit,
    id: &str,
) -> serde_json::Value {
    use serde_json::json;
    let t = task_rpc(ctx, app, socket, root, json!({"method":"get_target"}))["result"]["target"]
        .clone();
    let c = task_rpc(
        ctx,
        app,
        socket,
        root,
        json!({"method":"get_context","params":t}),
    )["result"]
        .clone();
    let preview = task_rpc(
        ctx,
        app,
        socket,
        root,
        json!({"method":"prepare_native_change","params":{"target":t,"expected_revision":c["revision"],"expected_context_revision":c["context_revision"],"change":{"version":1,"edits":[{"operation":"evidence","edit":edit}]}}}),
    );
    assert!(preview["result"]["preview_id"].is_string(), "{preview}");
    let r = task_rpc(
        ctx,
        app,
        socket,
        root,
        json!({"method":"commit_change","params":{"target":t,"preview_id":preview["result"]["preview_id"],"request_id":id}}),
    );
    assert_eq!(r["result"]["status"], "committed", "{r}");
    r
}
#[cfg(unix)]
fn task_event_edit(
    p: &Project,
    req: &str,
    event: crate::evidence::topic_tasks::Event,
) -> crate::evidence::EvidenceEdit {
    let (t, index, _, _) = crate::evidence::topic_tasks::find(p, req).unwrap();
    crate::evidence::EvidenceEdit::TopicTaskEvent {
        id: t.id.clone(),
        exchange_index: index,
        expected_topic_sha256: crate::automation_journal::canonical_hash(t).unwrap(),
        request_id: req.into(),
        event,
    }
}

#[cfg(unix)]
fn task_native_resume_after_manual_undo_and_reopen(
    ctx: &mut Context,
    app: &mut AtelierApp,
    socket: &mut PathBuf,
    root: &std::path::Path,
    file: &std::path::Path,
    req: &str,
    question: &str,
) {
    use crate::design_process::parametric as pm;
    use serde_json::json;
    let p = app.host.document.editor.project.clone();
    let task = p.design_process.task().unwrap();
    let option = p.plan_options.active_design_option.unwrap();
    let old = pm::current_parameters(&p, &task.id, option).unwrap();
    for _ in 0..6 {
        layers_frame_output(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos2(1250., 600.)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    phase: egui::TouchPhase::Move,
                    delta: vec2(0., 1000.),
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
    button(ctx, app, "Adjust generated wings");
    number(
        ctx,
        app,
        0,
        "Wing depth (mm)",
        (old.wing_depth_mm + 100) as f64,
    );
    button(ctx, app, "Preview two-wing parameters");
    button(ctx, app, "Confirm design change");
    app.poll_automation(ctx);
    assert_eq!(
        app.host.object_task_authorization()["status"],
        "paused_or_finished"
    );
    layers_frame_output(ctx, app, vec![key(Key::Z, Modifiers::COMMAND)]);
    app.poll_automation(ctx);
    assert_eq!(
        pm::current_parameters(&app.host.document.editor.project, &task.id, option).unwrap(),
        old
    );
    assert_ne!(app.host.object_task_authorization()["status"], "authorized");
    // No hidden task renewal from Undo or a fresh context read.
    task_rpc(ctx, app, socket, root, json!({"method":"get_target"}));
    assert_ne!(app.host.object_task_authorization()["status"], "authorized");
    layers_frame_output(ctx, app, vec![key(Key::S, Modifiers::COMMAND)]);
    if app.file_action.is_some() {
        button(ctx, app, "Save");
    }
    let saved = Editor::from_stored_document(file).unwrap();
    assert_eq!(saved.project, app.host.document.editor.project);
    let language = app.language;
    let (newctx, newapp) = setup();
    let previous = std::mem::replace(app, newapp);
    drop(previous);
    *ctx = newctx;
    ctx.enable_accesskit();
    ctx.global_style_mut(|s| s.animation_time = 0.);
    app.language = language;
    app.host
        .configure_operation_journal(Some(root.join("journal")))
        .unwrap();
    app.initialize_source_connections(root);
    open(ctx, app, file);
    wait_sources(ctx, app);
    assert_eq!(app.host.document.editor.project, saved.project);
    assert_eq!(
        app.host.object_task_authorization()["status"],
        "not_granted"
    );
    *socket = socket
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("resumed-ipc/a.sock");
    app.automation = Some(
        crate::automation_ipc::Endpoint::bind_for_host(
            socket.clone(),
            false,
            Some(root.join("journal")),
            &mut app.host,
        )
        .unwrap(),
    );
    button(ctx, app, "AI Assistant");
    task_visible_button(ctx, app, question);
    task_visible_button(ctx, app, "Review and resume original task");
    assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
    assert_eq!(
        app.host.object_task_authorization()["status"],
        "not_granted"
    );
    button(ctx, app, "Cancel design proposal");
    assert_eq!(
        app.host.object_task_authorization()["status"],
        "not_granted"
    );
    task_visible_button(ctx, app, "Review and resume original task");
    button(ctx, app, "Confirm design change");
    assert!(
        app.research_ui.error.is_empty(),
        "{}",
        app.research_ui.error
    );
    assert_eq!(app.host.object_task_authorization()["status"], "authorized");
    assert_eq!(app.host.object_task_authorization()["request_id"], req);
}
#[test]
#[cfg(unix)]
#[ignore = "explicit latest grain controlled input and unused output; optional live main-Agent consumer"]
fn grain_object_task_normal_entry_bilingual_or_live_consumer() {
    use crate::{
        design_process::parametric as pm,
        evidence::topic_tasks::{self, Event as TaskEvent},
    };
    use serde_json::json;
    let input = PathBuf::from(std::env::var_os("OPENARCHI_OBJECT_TASK_INPUT").unwrap());
    let original = std::fs::read(&input).unwrap();
    let parent = PathBuf::from(std::env::var_os("OPENARCHI_OBJECT_TASK_OUTPUT").unwrap());
    assert!(parent.is_absolute());
    std::fs::create_dir(&parent).expect("Inspect prior outcome; do not replay");
    let live = std::env::var_os("OPENARCHI_OBJECT_TASK_LIVE").is_some();
    for language in if live {
        vec![UiLanguage::SimplifiedChinese]
    } else {
        vec![UiLanguage::English, UiLanguage::SimplifiedChinese]
    } {
        let root = parent.join(if language == UiLanguage::English {
            "en"
        } else {
            "zh"
        });
        std::fs::create_dir(&root).unwrap();
        let file = root.join("object-task.openarchi.json");
        let initial = Editor::from_stored_document(&input).unwrap();
        storage::save_document(&initial.project, &file, initial.project_identity()).unwrap();
        let (mut ctx, mut app) = setup();
        ctx.enable_accesskit();
        ctx.global_style_mut(|s| s.animation_time = 0.);
        app.language = language;
        let journal = root.join("journal");
        app.host
            .configure_operation_journal(Some(journal.clone()))
            .unwrap();
        if let Some(source) = std::env::var_os("OPENARCHI_OBJECT_TASK_REPOSITORY") {
            // Restore the already bound controlled source archive, read-only. No recopy/import/classification.
            let repo = crate::project_repository::Repository::open(&PathBuf::from(source)).unwrap();
            app.initialize_source_connections(&root);
            crate::project_repository::connection::remember(
                app.source_ui.connections_directory.as_ref().unwrap(),
                &file,
                initial.project_identity(),
                &storage::FileVersion::read(&file).unwrap().token().unwrap(),
                &repo,
            )
            .unwrap();
        }
        open(&ctx, &mut app, &file);
        wait_sources(&ctx, &mut app);
        let socket_root = std::env::temp_dir().join(format!(
            "oa-task-ui-{}",
            &storage::new_document_identity()[..12]
        ));
        std::fs::create_dir(&socket_root).unwrap();
        let mut socket = socket_root.join("ipc/a.sock");
        app.automation = Some(
            crate::automation_ipc::Endpoint::bind_for_host(
                socket.clone(),
                false,
                Some(journal.clone()),
                &mut app.host,
            )
            .unwrap(),
        );
        // Existing connected stdio client, default read-only. No environment or test-only grant.
        assert!(!app.host.operations.borrow().enabled());
        let before = app.host.document.editor.project.clone();
        let task = before.design_process.task().unwrap();
        let option = before.plan_options.active_design_option.unwrap();
        let member = task.candidates[0].initial_mass_ids[0];
        let name = &before
            .elements
            .iter()
            .find(|e| e.id == member)
            .unwrap()
            .name;
        button(&ctx, &mut app, "Select a source mass");
        button(&ctx, &mut app, &format!("#{member} · {name}"));
        button(&ctx, &mut app, "Discuss selected objects");
        let question = "请依据已存任务书与浅翼试探，把当前A教学双翼的进深适度增加，院落、位置、层数及总高不变；说明收益和未知，更新原检查和关联图，保护保留建筑与B。只做有明确假设的概念研究。";
        let field = ctx
            .data(|d| d.get_temp::<Id>(Id::new("object_research_question")))
            .unwrap();
        action(&ctx, &mut app, field.accesskit_id(), Action::Focus, None);
        layers_frame_output(&ctx, &mut app, vec![Event::Text(question.into())]);
        button(
            &ctx,
            &mut app,
            "Allow parameter updates to the current two wings",
        );
        assert!(app.research_ui.allow_task_parameters);
        button(&ctx, &mut app, "Preview task for connected Agent");
        assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
        assert_eq!(app.host.document.editor.project, before);
        assert!(!app.host.operations.borrow().enabled());
        button(&ctx, &mut app, "Cancel design proposal");
        assert_eq!(app.host.document.editor.project, before);
        button(&ctx, &mut app, "Preview task for connected Agent");
        button(&ctx, &mut app, "Confirm design change");
        assert!(
            app.research_ui.error.is_empty(),
            "{}",
            app.research_ui.error
        );
        let topic = app
            .host
            .document
            .editor
            .project
            .evidence
            .topics
            .last()
            .unwrap()
            .clone();
        let req = topic.exchanges[0].assignment.as_ref().unwrap().id.clone();
        assert!(matches!(
            topic.exchanges[0].assignment.as_ref().unwrap().mode,
            topic_tasks::Mode::TwoWing { .. }
        ));
        assert_eq!(app.host.object_task_authorization()["status"], "authorized");
        assert_eq!(app.host.document.editor.project.elements, before.elements);
        app.poll_automation(&ctx);
        if live {
            std::fs::write(root.join("ready.json"),serde_json::to_vec_pretty(&json!({"socket":socket,"input":input,"output":file,"journal":journal,"task_request_id":req,"topic":topic.id,"task":task.id,"option":option,"question":question,"entry":"normal bilingual-capable GUI event path; default app.poll_automation, task-only native consent","native_os":"not_run","automatic_outbound_call":false})).unwrap()).unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1200);
            while !root.join("finish-requested.json").exists() {
                app.poll_automation(&ctx);
                layers_frame_output(&ctx, &mut app, vec![]);
                assert!(
                    std::time::Instant::now() < deadline,
                    "Keep original task/operation evidence; no replay"
                );
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            assert_eq!(
                topic_tasks::find(&app.host.document.editor.project, &req)
                    .unwrap()
                    .3
                    .state()
                    .unwrap()
                    .0,
                topic_tasks::Phase::Completed
            );
        } else {
            let edit = task_event_edit(
                &app.host.document.editor.project,
                &req,
                TaskEvent::Claim {
                    agent_label: "Default GUI-path client fixture (not AI)".into(),
                },
            );
            task_edit(
                &ctx,
                &mut app,
                &socket,
                &root,
                edit,
                &storage::new_document_identity(),
            );
            task_native_resume_after_manual_undo_and_reopen(
                &mut ctx,
                &mut app,
                &mut socket,
                &root,
                &file,
                &req,
                question,
            );
            let t = task_rpc(
                &ctx,
                &mut app,
                &socket,
                &root,
                json!({"method":"get_target"}),
            )["result"]["target"]
                .clone();
            let c = task_rpc(
                &ctx,
                &mut app,
                &socket,
                &root,
                json!({"method":"get_context","params":t}),
            )["result"]
                .clone();
            let mut parameters =
                pm::current_parameters(&app.host.document.editor.project, &task.id, option)
                    .unwrap();
            parameters.wing_depth_mm += 200;
            let update = storage::new_document_identity();
            let preview = task_rpc(
                &ctx,
                &mut app,
                &socket,
                &root,
                json!({"method":"prepare_parametric_proposal","params":{"target":t,"expected_revision":c["revision"],"expected_context_revision":c["context_revision"],"expected_sources_revision":c["project_sources"]["sources_revision"],"task_id":task.id,"proposal":{"action":"update","update_id":update,"instruction":"Regression concept: only a small wing depth change","parameters":parameters}}}),
            );
            assert!(preview["result"]["preview_id"].is_string(), "{preview}");
            let committed = task_rpc(
                &ctx,
                &mut app,
                &socket,
                &root,
                json!({"method":"commit_change","params":{"target":t,"preview_id":preview["result"]["preview_id"],"request_id":update}}),
            );
            assert_eq!(committed["result"]["status"], "committed", "{committed}");
            assert_eq!(
                pm::current_parameters(&app.host.document.editor.project, &task.id, option)
                    .unwrap(),
                parameters
            );
            let (topic, index, _, _) =
                topic_tasks::find(&app.host.document.editor.project, &req).unwrap();
            let reply=crate::evidence::topics::AgentReply{id:storage::new_document_identity(),agent_label:"Default GUI-path client fixture (not AI)".into(),text:"The permitted teaching wings are slightly deeper; existing retained objects and alternative B remain protected. Regulatory suitability is unknown.".into(),claims:vec![crate::design_process::intent::Claim{kind:crate::design_process::intent::ClaimKind::DesignCompletion,text:"The 200mm depth increase is a concept assumption, not measured data.".into()}]};
            let edit = crate::evidence::EvidenceEdit::RecordTopicReply {
                id: topic.id.clone(),
                exchange_index: index,
                expected_topic_sha256: crate::automation_journal::canonical_hash(topic).unwrap(),
                reply: reply.clone(),
            };
            task_edit(&ctx, &mut app, &socket, &root, edit, &reply.id);
            let edit = task_event_edit(
                &app.host.document.editor.project,
                &req,
                TaskEvent::Complete {
                    operation_ids: vec![update, reply.id],
                },
            );
            task_edit(
                &ctx,
                &mut app,
                &socket,
                &root,
                edit,
                &storage::new_document_identity(),
            );
        }
        let completed = app.host.document.editor.project.clone();
        assert_eq!(
            completed
                .elements
                .iter()
                .filter(|e| !task.candidates[0].initial_mass_ids.contains(&e.id))
                .cloned()
                .collect::<Vec<_>>(),
            before
                .elements
                .iter()
                .filter(|e| !task.candidates[0].initial_mass_ids.contains(&e.id))
                .cloned()
                .collect::<Vec<_>>()
        );
        visible_agent_reply(&ctx, &mut app, question);
        // Return up the same assistant scroll, keep the current canvas and inline parameters.
        let at = pos2(1250., 600.);
        for _ in 0..8 {
            layers_frame_output(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(at),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        delta: vec2(0., 1000.),
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        button(&ctx, &mut app, "Adjust generated wings");
        let depth = pm::current_parameters(&completed, &task.id, option)
            .unwrap()
            .wing_depth_mm;
        number(&ctx, &mut app, 0, "Wing depth (mm)", (depth + 100) as f64);
        button(&ctx, &mut app, "Preview two-wing parameters");
        button(&ctx, &mut app, "Confirm design change");
        assert_eq!(
            pm::current_parameters(&app.host.document.editor.project, &task.id, option)
                .unwrap()
                .wing_depth_mm,
            depth + 100
        );
        layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
        assert_eq!(
            app.host.document.editor.project.elements,
            completed.elements
        );
        assert_eq!(
            pm::current_parameters(&app.host.document.editor.project, &task.id, option)
                .unwrap()
                .wing_depth_mm,
            depth
        );
        layers_frame_output(&ctx, &mut app, vec![key(Key::S, Modifiers::COMMAND)]);
        if app.file_action.is_some() {
            button(&ctx, &mut app, "Save");
        }
        let saved = Editor::from_stored_document(&file).unwrap();
        assert_eq!(saved.project, app.host.document.editor.project);
        drop(app);
        let (fresh_ctx, mut fresh) = setup();
        fresh_ctx.enable_accesskit();
        fresh_ctx.global_style_mut(|s| s.animation_time = 0.);
        fresh.language = language;
        fresh
            .host
            .configure_operation_journal(Some(journal))
            .unwrap();
        open(&fresh_ctx, &mut fresh, &file);
        wait_sources(&fresh_ctx, &mut fresh);
        assert_eq!(fresh.host.document.editor.project, saved.project);
        assert_eq!(
            fresh.host.object_task_authorization()["status"],
            "not_granted"
        );
        button(&fresh_ctx, &mut fresh, "AI Assistant");
        visible_agent_reply(&fresh_ctx, &mut fresh, question);

        if !live {
            button(&fresh_ctx, &mut fresh, "Select a source mass");
            button(&fresh_ctx, &mut fresh, &format!("#{member} · {name}"));
            button(&fresh_ctx, &mut fresh, "Discuss selected objects");
            let field = fresh_ctx
                .data(|d| d.get_temp::<Id>(Id::new("object_research_question")))
                .unwrap();
            action(
                &fresh_ctx,
                &mut fresh,
                field.accesskit_id(),
                Action::Focus,
                None,
            );
            layers_frame_output(
                &fresh_ctx,
                &mut fresh,
                vec![Event::Text("Cancellation probe, no model changes".into())],
            );
            button(&fresh_ctx, &mut fresh, "Preview task for connected Agent");
            button(&fresh_ctx, &mut fresh, "Confirm design change");
            let cancelled = fresh
                .host
                .document
                .editor
                .project
                .evidence
                .topics
                .last()
                .unwrap()
                .clone();
            let request = &cancelled
                .exchanges
                .last()
                .unwrap()
                .assignment
                .as_ref()
                .unwrap()
                .id;
            task_visible_button(
                &fresh_ctx,
                &mut fresh,
                "Cancellation probe, no model changes",
            );
            task_visible_button(&fresh_ctx, &mut fresh, "Stop further task writes");
            button(&fresh_ctx, &mut fresh, "Cancel design proposal");
            assert_eq!(
                topic_tasks::find(&fresh.host.document.editor.project, request)
                    .unwrap()
                    .3
                    .state()
                    .unwrap()
                    .0,
                topic_tasks::Phase::Queued
            );
            task_visible_button(&fresh_ctx, &mut fresh, "Stop further task writes");
            button(&fresh_ctx, &mut fresh, "Confirm design change");
            assert_eq!(
                topic_tasks::find(&fresh.host.document.editor.project, request)
                    .unwrap()
                    .3
                    .state()
                    .unwrap()
                    .0,
                topic_tasks::Phase::Cancelled
            );
            assert_eq!(
                fresh.host.object_task_authorization()["status"],
                "not_granted"
            );
            assert_eq!(
                fresh.host.document.editor.project.elements,
                saved.project.elements
            );
        }
        assert_eq!(std::fs::read(&input).unwrap(), original);
        std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&json!({"input":input,"output":file,"request_id":req,"topic":topic.id,"language":if language==UiLanguage::English{"en"}else{"zh"},"task_scoped_default_app_entry":true,"preview_cancel_confirm":true,"external_consumer_actual_main_agent":live,"manual_local_followup_and_undo":true,"normal_save_reopen":true,"authority_reopened":"not_granted","other_objects_exact":true,"native_os":"not_run","scope":"one existing parametric candidate; all parent gates retained"})).unwrap()).unwrap();
    }
}

#[test]
#[cfg(unix)]
fn object_task_first_create_normal_bilingual_consent_is_optional_and_frozen() {
    use crate::evidence::topic_tasks::Mode;
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app) = setup();
        ctx.enable_accesskit();
        ctx.global_style_mut(|s| s.animation_time = 0.);
        app.language = language;
        app.host.document.editor = Editor::new(crate::storage::startup_example());
        let root = std::env::temp_dir().join(format!(
            "oa-create-consent-{}",
            storage::new_document_identity()
        ));
        std::fs::create_dir(&root).unwrap();
        app.host
            .configure_operation_journal(Some(root.join("journal")))
            .unwrap();
        let id = app
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .find(|e| matches!(e.shape, Shape::Zone { .. }))
            .unwrap()
            .id;
        app.host.document.editor.set_selection(vec![id]);
        let before = app.host.document.editor.project.clone();
        button(&ctx, &mut app, "Discuss selected objects");
        assert!(!app.research_ui.allow_task_creation);
        assert!(!app.research_ui.allow_task_parameters);
        assert_discussion_layout_and_type(&ctx, &mut app);
        button(&ctx, &mut app, "Allow one new two-wing study");
        assert!(app.research_ui.allow_task_creation);
        assert!(!app.research_ui.allow_task_parameters);
        let frozen = app.research_ui.draft.as_ref().unwrap().exchange.clone();
        // Merely selecting a different object must never retarget this explicit task.
        let other = before.elements.iter().find(|e| e.id != id).unwrap().id;
        app.host.document.editor.set_selection(vec![other]);
        button(&ctx, &mut app, "Preview task for connected Agent");
        assert!(app.process_ui.preview.is_some(), "{}", app.process_ui.error);
        let out = layers_frame_output(&ctx, &mut app, vec![]);
        let mut texts = vec![];
        collect_text(&out.shapes, &mut texts);
        assert!(texts.contains(&frozen.question));
        assert!(texts.iter().any(|s| {
            s == language
                .translate("Permission: one new two-wing study; no changes to existing work")
        }));
        assert_eq!(app.host.document.editor.project, before);
        assert!(!app.host.operations.borrow().enabled());
        button(&ctx, &mut app, "Cancel design proposal");
        assert_eq!(app.research_ui.draft.as_ref().unwrap().exchange, frozen);
        assert!(!app.host.operations.borrow().enabled());
        button(&ctx, &mut app, "Preview task for connected Agent");
        button(&ctx, &mut app, "Confirm design change");
        assert!(
            app.research_ui.error.is_empty(),
            "{}",
            app.research_ui.error
        );
        let (_, _, exchange, assignment) = app
            .host
            .document
            .editor
            .project
            .evidence
            .topics
            .iter()
            .flat_map(|t| {
                t.exchanges
                    .iter()
                    .enumerate()
                    .filter_map(move |(i, e)| e.assignment.as_ref().map(|a| (t, i, e, a)))
            })
            .last()
            .unwrap();
        assert_eq!(exchange.objects, frozen.objects);
        assert_eq!(exchange.question, frozen.question);
        assert!(matches!(assignment.mode, Mode::CreateTwoWing { .. }));
        assert_eq!(app.host.object_task_authorization()["status"], "authorized");
        assert_eq!(
            app.host.object_task_authorization()["request_id"],
            assignment.id
        );
        assert_eq!(app.host.document.editor.project.elements, before.elements);
        assert_eq!(
            app.host.document.editor.project.design_process,
            before.design_process
        );
        assert_eq!(app.host.document.editor.project.site, before.site);
    }
}

include!("object_nearby_ui_tests.rs");

include!("object_reviewed_input_ui_tests.rs");

include!("object_teaching_space_ui_tests.rs");

include!("object_linked_space_ui_tests.rs");

include!("object_discussion_restore_ui_tests.rs");

include!("object_function_figure_ui_tests.rs");

include!("object_parametric_check_ui_tests.rs");
