mod environment_startup_product_path {
    use super::*;
    fn frame(ctx: &Context, app: &mut AtelierApp, events: Vec<Event>) -> egui::FullOutput {
        layers_frame_output(ctx, app, events)
    }
    fn button(ctx: &Context, app: &mut AtelierApp, label: &str) {
        reveal_manual_control(ctx, app, label);
        for _ in 0..3 {
            frame(ctx, app, vec![]);
        }
        let out = frame(ctx, app, vec![]);
        let point = out.shapes.iter().find_map(|shape| {
            let egui::Shape::Text(t) = &shape.shape else {
                return None;
            };
            let visible = Rect::from_min_size(t.pos, t.galley.size()).intersect(shape.clip_rect);
            (t.galley.text() == app.language.translate(label) && visible.is_positive())
                .then(|| visible.center())
        });
        let Some(point) = point else {
            let mut texts = vec![];
            collect_text(&out.shapes, &mut texts);
            panic!(
                "normal startup control {label} missing: {} / {texts:?}",
                app.workbench.configuration_error
            );
        };
        layers_click(ctx, app, point);
    }
    fn open(ctx: &Context, app: &mut AtelierApp, path: &std::path::Path) {
        open_test_project(ctx, app, &path);
        assert!(app.file_action.is_none(), "{}", app.file_error);
    }
    #[test]
    fn damaged_primary_and_backup_normal_startup_recovery_preserves_originals_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            for kind in [
                "invalid-json",
                "oversized",
                "directory",
                "symlink",
                "missing",
            ] {
                #[cfg(not(unix))]
                if kind == "symlink" {
                    continue;
                }
                let root = std::env::temp_dir()
                    .join(format!("oa-startup-{}", storage::new_document_identity()));
                let profile = root.join("profile");
                std::fs::create_dir_all(&profile).unwrap();
                eprintln!("STARTUP_PRODUCT_ROOT={} kind={kind}", root.display());
                let primary = profile.join("work-environment.json");
                let bad = br#"{"schema_version":999,"untrusted":"preserve original"}"#;
                std::fs::write(&primary, bad).unwrap();
                let backup = profile.join("work-environment.json.last-good.json");
                let backup_bytes = if kind == "oversized" {
                    vec![b'x'; 1024 * 1024 + 1]
                } else {
                    b"unusable backup must be preserved".to_vec()
                };
                let target = root.join("protected-backup-target.json");
                match kind {
                    "invalid-json" | "oversized" => std::fs::write(&backup, &backup_bytes).unwrap(),
                    "directory" => std::fs::create_dir(&backup).unwrap(),
                    #[cfg(unix)]
                    "symlink" => {
                        std::fs::write(&target, &backup_bytes).unwrap();
                        std::os::unix::fs::symlink(&target, &backup).unwrap();
                    }
                    _ => {}
                }
                let input = if let Some(p) = std::env::var_os("OPENARCHI_STARTUP_PROJECT") {
                    PathBuf::from(p)
                } else {
                    let path = root.join("public-case.openarchi.json");
                    storage::save(&storage::startup_example(), &path).unwrap();
                    path
                };
                let input_bytes = std::fs::read(&input).unwrap();
                let ctx = Context::default();
                let mut app = AtelierApp::from_renderer(None, "startup-product-event".into());
                app.language = language;
                app.host
                    .configure_operation_journal(Some(root.join("journal")))
                    .unwrap();
                app.initialize_work_mode(&profile);
                open(&ctx, &mut app, &input);
                let before = app.host.document.editor.project.clone();
                let context = app.host.context();
                let history = app.host.document.editor.undo.len();
                let selection = app.host.document.editor.selection_ids();
                let dirty = app.host.document.editor.dirty();
                assert_eq!(std::fs::read(&primary).unwrap(), bad);
                button(&ctx, &mut app, "Customize work environment…");
                button(&ctx, &mut app, "Recover last valid environment");
                assert_eq!(std::fs::read(&primary).unwrap(), bad);
                button(&ctx, &mut app, "Recover with bundled defaults");
                assert!(
                    app.host
                        .environment
                        .as_ref()
                        .is_some_and(|s| s.recovery.is_none()),
                    "{}",
                    app.workbench.configuration_error
                );
                let recovered = std::fs::read(&primary).unwrap();
                assert!(crate::workbench::environment::Configuration::parse(&recovered).is_ok());
                let archives = std::fs::read_dir(&profile)
                    .unwrap()
                    .filter_map(Result::ok)
                    .map(|p| p.path())
                    .filter(|p| {
                        p.file_name()
                            .unwrap()
                            .to_string_lossy()
                            .starts_with("work-environment-preserved-")
                    })
                    .collect::<Vec<_>>();
                assert_eq!(archives.len(), 1);
                assert_eq!(std::fs::read(&archives[0]).unwrap(), bad);
                match kind {
                    "invalid-json" => {
                        let archive=std::fs::read_dir(&profile).unwrap().filter_map(Result::ok).map(|p|p.path()).find(|p|p.file_name().unwrap().to_string_lossy().starts_with("work-environment-backup-preserved-")).expect("normal recovery must archive the original bad backup before updating it");
                        assert_eq!(std::fs::read(archive).unwrap(), backup_bytes);
                    }
                    "oversized" => assert_eq!(std::fs::read(&backup).unwrap(), backup_bytes),
                    "directory" => assert!(backup.is_dir()),
                    #[cfg(unix)]
                    "symlink" => {
                        assert!(
                            std::fs::symlink_metadata(&backup)
                                .unwrap()
                                .file_type()
                                .is_symlink()
                        );
                        assert_eq!(std::fs::read(&target).unwrap(), backup_bytes);
                    }
                    _ => {}
                }
                assert_eq!(app.host.document.editor.project, before);
                assert_eq!(app.host.context(), context);
                assert_eq!(app.host.document.editor.undo.len(), history);
                assert_eq!(app.host.document.editor.selection_ids(), selection);
                assert_eq!(app.host.document.editor.dirty(), dirty);
                assert_eq!(std::fs::read(&input).unwrap(), input_bytes);
                let mut restarted = AtelierApp::from_renderer(None, "startup-reopen-event".into());
                restarted.language = language;
                restarted.initialize_work_mode(&profile);
                assert!(
                    restarted
                        .host
                        .environment
                        .as_ref()
                        .is_some_and(|s| s.recovery.is_none())
                );
                open(&Context::default(), &mut restarted, &input);
                assert_eq!(restarted.host.document.editor.project, before);
                std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"pass","backup_case":kind,"language":language.label(),"normal_entry":"initialize -> normal Open -> Customize -> failed last-valid -> explicit defaults -> fresh application host Open","original_model_history_and_case":"unchanged","new_OS_process":"not_run","native_package":"not_run"})).unwrap()).unwrap();
            }
        }
    }
    #[cfg(unix)]
    #[test]
    fn recovered_backup_warning_is_exposed_by_the_default_readonly_gui_endpoint() {
        use serde_json::json;
        use std::{
            io::{BufRead, BufReader, Write},
            os::unix::net::UnixStream,
            thread,
            time::{Duration, Instant},
        };
        let root = PathBuf::from("/tmp").join(format!(
            "oa-startup-diag-{}",
            &storage::new_document_identity()[..12]
        ));
        std::fs::create_dir(&root).unwrap();
        let profile = root.join("profile");
        std::fs::create_dir(&profile).unwrap();
        std::fs::write(profile.join("work-environment.json"), b"invalid primary").unwrap();
        let backup = profile.join("work-environment.json.last-good.json");
        let original = vec![b'x'; 1024 * 1024 + 1];
        std::fs::write(&backup, &original).unwrap();
        let ctx = Context::default();
        let mut app = AtelierApp::from_renderer(None, "startup-default-endpoint".into());
        app.initialize_work_mode(&profile);
        button(&ctx, &mut app, "Customize work environment…");
        button(&ctx, &mut app, "Recover with bundled defaults");
        let warning = app
            .host
            .environment
            .as_ref()
            .unwrap()
            .backup_warning
            .clone()
            .unwrap();
        let socket = root.join("ipc/app.sock");
        app.automation = Some(
            crate::automation_ipc::Endpoint::bind_for_host(
                socket.clone(),
                false,
                Some(root.join("journal")),
                &mut app.host,
            )
            .unwrap(),
        );
        let wire =
            |ctx: &Context, app: &mut AtelierApp, value: serde_json::Value| -> serde_json::Value {
                let path = socket.clone();
                let client = thread::spawn(move || {
                    let mut s = UnixStream::connect(path).unwrap();
                    s.set_read_timeout(Some(Duration::from_secs(8))).unwrap();
                    writeln!(s, "{value}").unwrap();
                    let mut line = String::new();
                    BufReader::new(s).read_line(&mut line).unwrap();
                    serde_json::from_str(&line).unwrap()
                });
                let start = Instant::now();
                while !client.is_finished() {
                    app.poll_automation(ctx);
                    assert!(start.elapsed() < Duration::from_secs(8));
                    thread::sleep(Duration::from_millis(2));
                }
                client.join().unwrap()
            };
        let before = app.host.document.editor.project.clone();
        let target =
            wire(&ctx, &mut app, json!({"method":"get_target"}))["result"]["target"].clone();
        let context = wire(
            &ctx,
            &mut app,
            json!({"method":"get_context","params":target}),
        );
        assert_eq!(context["result"]["permissions"], json!(["read"]));
        let result = wire(
            &ctx,
            &mut app,
            json!({"method":"inspect_work_environment","params":{"target":target,"expected_revision":0}}),
        );
        assert!(result.get("error").is_none(), "{result}");
        assert_eq!(
            result["result"]["inspection"]["backup_warning"], warning,
            "{result}"
        );
        assert_eq!(app.host.document.editor.project, before);
        assert_eq!(std::fs::read(&backup).unwrap(), original);
        std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&json!({"status":"pass","entry":"normal GUI recovery -> actual default read-only Endpoint/poll_automation, no optional TaskGuard","inspection":result,"model_and_backup":"unchanged by inspection"})).unwrap()).unwrap();
        eprintln!("STARTUP_DEFAULT_ENDPOINT_ROOT={}", root.display());
    }
}
