mod report_template_product_path {
    use super::*;
    fn frame(ctx: &Context, app: &mut AtelierApp, events: Vec<Event>) -> egui::FullOutput {
        layers_frame_output(ctx, app, events)
    }
    fn click(ctx: &Context, app: &mut AtelierApp, key: &str, index: usize) {
        for attempt in 0..24 {
            let out = frame(ctx, app, vec![]);
            let bounds = if app.process_ui.preview.is_some() {
                ctx.memory(|m| m.area_rect(Id::new("design_process_proposal")))
            } else if app.file_action.is_some() {
                ctx.memory(|m| m.area_rect(Id::new("project_file_dialog")))
            } else if app.drawings_ui.manager && key != "Drawing Manager" {
                ctx.memory(|m| m.area_rect(Id::new("drawing_manager")))
            } else {
                None
            };
            let matches: Vec<_> = out
                .shapes
                .iter()
                .filter_map(|s| {
                    let egui::Shape::Text(text) = &s.shape else {
                        return None;
                    };
                    if text.galley.text() != app.language.translate(key) {
                        return None;
                    }
                    let mut visible =
                        Rect::from_min_size(text.pos, text.galley.size()).intersect(s.clip_rect);
                    if let Some(bounds) = bounds {
                        visible = visible.intersect(bounds);
                    }
                    visible.is_positive().then(|| visible.center())
                })
                .collect();
            if let Some(p) = matches.get(index) {
                layers_click(ctx, app, *p);
                return;
            }
            if let Some(rect) = app
                .drawings_ui
                .rects
                .get("report.scroll")
                .copied()
                .filter(|_| app.process_ui.preview.is_none() && app.file_action.is_none())
            {
                frame(
                    ctx,
                    app,
                    vec![
                        Event::PointerMoved(rect.center()),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            phase: egui::TouchPhase::Move,
                            delta: vec2(0., if attempt < 12 { -70. } else { 70. }),
                            modifiers: Modifiers::NONE,
                        },
                    ],
                );
            }
        }
        let out = frame(ctx, app, vec![]);
        let mut texts = vec![];
        collect_text(&out.shapes, &mut texts);
        panic!(
            "visible control {key}/{index}: {} / {} / {texts:?}",
            app.process_ui.error, app.drawings_ui.error
        );
    }
    fn text(ctx: &Context, app: &mut AtelierApp, old: &str, value: &str) {
        frame(ctx, app, vec![]);
        click(ctx, app, old, 0);
        frame(
            ctx,
            app,
            vec![key(Key::A, Modifiers::COMMAND), Event::Text(value.into())],
        );
    }
    fn open(ctx: &Context, app: &mut AtelierApp, path: &std::path::Path) {
        open_test_project(ctx, app, &path);
        assert!(app.file_action.is_none(), "{}", app.file_error);
    }
    #[test]
    fn report_template_normal_project_entry_preview_cancel_order_history_and_save_bilingual() {
        use crate::{
            command::ModelCommand,
            diagrams::{Definition, Domain, Edit},
        };
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let root = std::env::temp_dir()
                .join(format!("oa-report-ui-{}", storage::new_document_identity()));
            std::fs::create_dir(&root).unwrap();
            let input = if let Some(path) = std::env::var_os("OPENARCHI_REPORT_PROJECT") {
                PathBuf::from(path)
            } else {
                let mut e = Editor::new(storage::synthetic_building_fixture());
                for story in [0, 1] {
                    e.apply(ModelCommand::Diagram {
                        edit: Edit::Create {
                            name: format!("Fixture floor {story}"),
                            definition: Definition {
                                story,
                                option: None,
                                domain: Domain::Zones,
                                presentation: Default::default(),
                            },
                        },
                    })
                    .unwrap();
                }
                let path = root.join("public-fixture.json");
                storage::save(&e.project, &path).unwrap();
                path
            };
            eprintln!("REPORT_PRODUCT_START_ROOT={}", root.display());
            let original_bytes = std::fs::read(&input).unwrap();
            let original = storage::load(&input).unwrap();
            let ctx = Context::default();
            let mut app = AtelierApp::from_renderer(None, "report-product-event".into());
            app.language = language;
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            open(&ctx, &mut app, &input);
            assert_eq!(app.host.document.editor.project, original);
            click(&ctx, &mut app, "Drawing Manager", 0);
            click(&ctx, &mut app, "Report templates", 0);
            click(&ctx, &mut app, "New report template", 0);
            let pending = app.drawings_ui.report_draft.clone();
            #[cfg(target_os = "macos")]
            {
                app.perform_menu_action(super::native_menu::MenuAction::Quit, &ctx);
                assert!(!app.allow_close);
                assert!(app.drawings_ui.report_draft == pending);
                assert!(app.drawings_ui.manager);
            }
            frame(&ctx, &mut app, vec![key(Key::S, Modifiers::COMMAND)]);
            click(&ctx, &mut app, "Save", 0);
            assert!(app.file_action.is_some());
            assert_eq!(
                app.file_error,
                "Confirm or cancel the report template draft before saving or replacing the project"
            );
            assert!(app.drawings_ui.report_draft == pending);
            assert_eq!(std::fs::read(&input).unwrap(), original_bytes);
            click(&ctx, &mut app, "Cancel", 0);
            let selected = PathBuf::from(app.path.clone());
            open_test_project(&ctx, &mut app, &selected);
            assert!(app.file_action.is_some());
            assert!(app.drawings_ui.report_draft == pending);
            assert_eq!(app.host.document.editor.project, original);
            click(&ctx, &mut app, "Cancel", 0);
            text(
                &ctx,
                &mut app,
                language.translate("Concept report"),
                "本轮汇报 · Ordered review",
            );
            let figures: Vec<_> = original.drawings.diagrams.iter().take(2).cloned().collect();
            assert_eq!(figures.len(), 2);
            for f in &figures {
                click(&ctx, &mut app, &format!("+ #{} · {}", f.id, f.name), 0);
            }
            click(&ctx, &mut app, "Preview report template", 0);
            assert!(app.process_ui.preview.is_some());
            assert_eq!(app.host.document.editor.project, original);
            click(&ctx, &mut app, "Cancel design proposal", 0);
            assert_eq!(app.host.document.editor.project, original);
            assert!(app.drawings_ui.report_draft.is_some());
            click(&ctx, &mut app, "Move up", 1);
            click(&ctx, &mut app, "Preview report template", 0);
            click(&ctx, &mut app, "Confirm design change", 0);
            assert!(app.process_ui.preview.is_none(), "{}", app.process_ui.error);
            assert!(app.drawings_ui.report_draft.is_none());
            assert!(
                app.drawings_ui.report_error.is_empty(),
                "resolved draft must not retain the close guard error"
            );
            let report = app.host.document.editor.project.drawings.reports[0].clone();
            assert_eq!(
                report.sections.iter().map(|s| s.figure).collect::<Vec<_>>(),
                vec![figures[1].id, figures[0].id]
            );
            assert_eq!(app.host.document.editor.project.elements, original.elements);
            assert_eq!(
                app.host.document.editor.project.drawings.diagrams,
                original.drawings.diagrams
            );
            click(&ctx, &mut app, "Preview ordered report pages", 0);
            let svg_path = root.join("ordered-report.svg");
            let old_svg = app.drawings_ui.report_export.clone();
            text(&ctx, &mut app, &old_svg, &svg_path.to_string_lossy());
            click(&ctx, &mut app, "Export ordered report SVG", 0);
            let svg = std::fs::read(&svg_path).unwrap();
            let svg_text = std::str::from_utf8(&svg).unwrap();
            let parsed = roxmltree::Document::parse(svg_text).unwrap();
            let page_ids: Vec<_> = parsed
                .descendants()
                .filter(|n| n.attribute("data-report-page").is_some())
                .map(|n| {
                    n.attribute("data-figure-id")
                        .unwrap()
                        .parse::<u64>()
                        .unwrap()
                })
                .collect();
            assert_eq!(
                page_ids,
                report.sections.iter().map(|s| s.figure).collect::<Vec<_>>()
            );
            click(&ctx, &mut app, "Export ordered report SVG", 0);
            assert_eq!(std::fs::read(&svg_path).unwrap(), svg); // Existing output is never overwritten.
            click(&ctx, &mut app, "Show report on canvas", 0);
            assert!(
                !app.drawings_ui.manager,
                "the existing manager must not reopen over the report canvas"
            );
            assert_eq!(app.view, View::Figure(figures[1].id));
            assert_eq!(
                app.drawings_ui.report_navigation,
                Some((report.id, figures[1].id))
            );
            click(&ctx, &mut app, "Next report chapter", 0);
            assert_eq!(app.view, View::Figure(figures[0].id));
            click(&ctx, &mut app, "Previous report chapter", 0);
            assert_eq!(app.view, View::Figure(figures[1].id));
            click(&ctx, &mut app, "Report order…", 0);
            click(&ctx, &mut app, "Open source figure", 0);
            assert_eq!(app.view, View::Figure(figures[1].id));
            click(&ctx, &mut app, "Report order…", 0);
            assert_eq!(app.drawings_ui.report_selected, Some(report.id));
            let focus_before = ctx.egui_wants_keyboard_input();
            click(&ctx, &mut app, "Drawing Manager", 0);
            assert!(!app.drawings_ui.manager);
            frame(&ctx, &mut app, vec![]);
            let canvas = app.workbench.panel_rects["view.canvas"];
            layers_click(&ctx, &mut app, canvas.center());
            frame(&ctx, &mut app, vec![]);
            eprintln!(
                "REPORT_MODEL_FOCUS before={focus_before} after={} undo={}",
                ctx.egui_wants_keyboard_input(),
                app.host.document.editor.undo.len()
            );
            assert!(
                !ctx.egui_wants_keyboard_input(),
                "actual model canvas must own model history keys"
            );
            frame(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert!(app.host.document.editor.project.drawings.reports.is_empty());
            frame(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::ALT)],
            );
            assert_eq!(
                app.host.document.editor.project.drawings.reports,
                vec![report.clone()]
            );
            let output = root.join("continued-report.openarchi.json");
            frame(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let old = app
                .file_dialog_path
                .clone()
                .unwrap_or_else(|| app.path.clone());
            text(&ctx, &mut app, &old, &output.to_string_lossy());
            click(&ctx, &mut app, "Save", 0);
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let saved = storage::load(&output).unwrap();
            assert_eq!(saved.drawings.reports, vec![report.clone()]);
            assert_eq!(saved.elements, original.elements);
            assert_eq!(saved.drawings.diagrams, original.drawings.diagrams);
            assert_eq!(saved.design_process, original.design_process);
            assert_eq!(saved.job_archive, original.job_archive);
            let reopened = Context::default();
            let mut app2 = AtelierApp::from_renderer(None, "report-reopen-event".into());
            app2.language = language;
            open(&reopened, &mut app2, &output);
            assert_eq!(
                app2.host.document.editor.project.drawings.reports,
                vec![report]
            );
            assert_eq!(std::fs::read(&input).unwrap(), original_bytes);
            assert_eq!(
                app2.host.document.editor.project.elements,
                original.elements
            );
            std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"pass","language":language.label(),"input":input,"input_sha256":crate::drawings::digest(&original_bytes),"output":output,"normal_entry":"Open -> Drawing Manager -> Report templates -> actual typed fields/selection -> preview/cancel/reorder/confirm -> keyboard Undo/Redo -> normal Save -> new application event host Open","geometry/task/original_figures/jobs":"unchanged","native_package":"not_run","real_AI":"not_run","new_OS_process":"not_run; this is new application event host"})).unwrap()).unwrap();
            eprintln!("REPORT_PRODUCT_ROOT={}", root.display());
        }
    }
    #[cfg(unix)]
    fn wire(
        ctx: &Context,
        app: &mut AtelierApp,
        path: &std::path::Path,
        value: serde_json::Value,
    ) -> serde_json::Value {
        use std::{
            io::{BufRead, BufReader, Write},
            os::unix::net::UnixStream,
            thread,
            time::{Duration, Instant},
        };
        let path = path.to_owned();
        let client = thread::spawn(move || {
            let mut stream = UnixStream::connect(path).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(8)))
                .unwrap();
            writeln!(stream, "{value}").unwrap();
            let mut line = String::new();
            BufReader::new(stream).read_line(&mut line).unwrap();
            serde_json::from_str(&line).unwrap()
        });
        let start = Instant::now();
        while !client.is_finished() {
            app.poll_automation(ctx);
            assert!(start.elapsed() < Duration::from_secs(8));
            thread::sleep(Duration::from_millis(2));
        }
        client.join().unwrap()
    }
    #[cfg(unix)]
    #[test]
    fn report_default_gui_endpoint_shows_actual_preview_and_protects_unsaved_and_saved_user_edits()
    {
        use crate::{
            command::ModelCommand,
            diagrams::{Definition, Domain, Edit},
        };
        use serde_json::json;
        let root = PathBuf::from("/tmp").join(format!(
            "oa-rpt-{}",
            &storage::new_document_identity()[..12]
        ));
        std::fs::create_dir(&root).unwrap();
        let mut e = Editor::new(storage::synthetic_building_fixture());
        e.apply(ModelCommand::Diagram {
            edit: Edit::Create {
                name: "Actual source figure".into(),
                definition: Definition {
                    story: 0,
                    option: None,
                    domain: Domain::Zones,
                    presentation: Default::default(),
                },
            },
        })
        .unwrap();
        let input = root.join("input.json");
        storage::save(&e.project, &input).unwrap();
        let ctx = Context::default();
        let mut app = AtelierApp::from_renderer(None, "default-report-endpoint".into());
        app.host
            .configure_operation_journal(Some(root.join("journal")))
            .unwrap();
        open(&ctx, &mut app, &input);
        let socket = root.join("ipc/app.sock");
        app.automation = Some(
            crate::automation_ipc::Endpoint::bind_for_host(
                socket.clone(),
                true,
                Some(root.join("journal")),
                &mut app.host,
            )
            .unwrap(),
        );
        let target =
            wire(&ctx, &mut app, &socket, json!({"method":"get_target"}))["result"]["target"]
                .clone();
        let context = wire(
            &ctx,
            &mut app,
            &socket,
            json!({"method":"get_context","params":target}),
        )["result"]
            .clone();
        assert_eq!(context["permissions"], json!(["read", "write"]));
        let figure = app.host.document.editor.project.drawings.diagrams[0].clone();
        let sections = json!([{"heading":"Source-backed review","figure":figure.id}]);
        let request = json!({"method":"prepare_native_change","params":{"target":target,"expected_revision":context["revision"],"change":{"version":1,"edits":[{"operation":"diagram","edit":{"action":"report","edit":{"action":"create","name":"Actual protocol template","sections":sections}}}]}}});
        let preview = wire(&ctx, &mut app, &socket, request.clone());
        assert!(preview.get("error").is_none(), "{preview}");
        assert_eq!(
            preview["result"]["report_template_change"]["before"],
            json!([])
        );
        assert_eq!(
            preview["result"]["report_template_change"]["after"][0]["sections"],
            sections
        );
        assert!(app.host.document.editor.project.drawings.reports.is_empty());
        let commit = json!({"method":"commit_change","params":{"target":target,"request_id":"report-original-default","preview_id":preview["result"]["preview_id"]}});
        std::fs::write(
            root.join("original-commit-intent.json"),
            serde_json::to_vec_pretty(&commit).unwrap(),
        )
        .unwrap();
        let committed = wire(&ctx, &mut app, &socket, commit);
        assert_eq!(committed["result"]["status"], "committed", "{committed}");
        let view_request = json!({"method":"invoke_workbench_command","params":{"target":target,"expected_revision":1,"request_id":"show-original-report","command_id":"project.report_templates","parameters":{}}});
        std::fs::write(
            root.join("view-intent.json"),
            serde_json::to_vec_pretty(&view_request).unwrap(),
        )
        .unwrap();
        let before_view = app.host.document.editor.project.clone();
        let shown = wire(&ctx, &mut app, &socket, view_request);
        assert!(shown.get("error").is_none(), "{shown}");
        assert_eq!(app.view, View::Figure(figure.id));
        assert_eq!(
            app.drawings_ui.report_navigation,
            Some((
                app.host.document.editor.project.drawings.reports[0].id,
                figure.id
            ))
        );
        assert_eq!(app.host.document.editor.project, before_view);
        click(&ctx, &mut app, "Drawing Manager", 0);
        if !app.drawings_ui.report_open {
            click(&ctx, &mut app, "Report templates", 0);
        }
        click(&ctx, &mut app, "New report template", 0);
        let mut next = request;
        next["params"]["expected_revision"] = app.host.document.editor.revision.into();
        let refused = wire(&ctx, &mut app, &socket, next.clone());
        assert_eq!(
            refused["error"]["code"], "USER_INTERACTION_ACTIVE",
            "{refused}"
        );
        // New templates inherit the current canvas figure; no repeated source selection.
        assert_eq!(
            app.drawings_ui.report_draft.as_ref().unwrap().sections,
            vec![crate::diagrams::report::Section {
                heading: figure.name.clone(),
                figure: figure.id,
            }]
        );
        click(&ctx, &mut app, "Preview report template", 0);
        click(&ctx, &mut app, "Confirm design change", 0);
        assert!(
            app.process_ui.preview.is_none(),
            "native report confirmation: {}",
            app.process_ui.error
        );
        click(
            &ctx,
            &mut app,
            &format!(
                "#{} · {}",
                before_view.drawings.reports[0].id, before_view.drawings.reports[0].name
            ),
            0,
        );
        click(&ctx, &mut app, "Show report on canvas", 0);
        text(
            &ctx,
            &mut app,
            &figure.name,
            "Source renamed on report canvas",
        );
        click(&ctx, &mut app, "Preview rename", 0);
        // Let the taller figure preview settle its modal position before pointer input.
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        eprintln!(
            "REPORT_RENAME_PREVIEW name={} proposal={:?} error={}",
            app.drawings_ui.figure_name,
            app.process_ui.preview.as_ref().map(|p| p.label.as_str()),
            app.process_ui.error
        );
        click(&ctx, &mut app, "Confirm design change", 0);
        assert!(
            app.process_ui.preview.is_none(),
            "error={} editor={} proposal={:?}",
            app.process_ui.error,
            app.host.document.editor.status,
            app.process_ui.preview.as_ref().map(|p| p.label.as_str())
        );
        assert_eq!(app.view, View::Figure(figure.id));
        assert_eq!(
            app.drawings_ui.report_navigation,
            Some((before_view.drawings.reports[0].id, figure.id)),
            "confirming an edit to the current source must retain its report chapter"
        );
        let protected = app.host.document.editor.project.clone();
        assert_eq!(protected.drawings.reports.len(), 2);
        let current = wire(
            &ctx,
            &mut app,
            &socket,
            json!({"method":"get_context","params":target}),
        );
        next["params"]["expected_revision"] = current["result"]["revision"].clone();
        let denied = wire(&ctx, &mut app, &socket, next);
        assert_eq!(
            denied["error"]["code"], "WRITE_AUTHORIZATION_SUSPENDED",
            "{denied}"
        );
        let undo = wire(
            &ctx,
            &mut app,
            &socket,
            json!({"method":"undo_operation","params":{"target":target,"request_id":"report-original-default"}}),
        );
        assert_eq!(undo["error"]["code"], "UNDO_CONFLICT", "{undo}");
        assert_eq!(app.host.document.editor.project, protected);
        let original = wire(
            &ctx,
            &mut app,
            &socket,
            json!({"method":"get_operation","params":{"target":target,"request_id":"report-original-default"}}),
        );
        assert_eq!(original["result"]["status"], "committed");
        std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&json!({"status":"pass","entry":"actual GUI host poll -> default Endpoint, original JSON frames; no optional TaskGuard","preview":preview,"unsaved_refusal":refused,"fresh_revision_refusal":denied,"original_undo_conflict":undo,"original_receipt":original,"real_AI":"not_run","native_package":"not_run"})).unwrap()).unwrap();
        eprintln!("REPORT_DEFAULT_ENDPOINT_ROOT={}", root.display());
    }
}
