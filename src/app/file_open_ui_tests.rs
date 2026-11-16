mod native_project_open_flow {
    use super::*;
    fn button(ctx: &Context, app: &mut AtelierApp, label: &str) {
        for _ in 0..3 {
            layers_frame_output(ctx, app, vec![]);
        }
        let out = layers_frame_output(ctx, app, vec![]);
        let at = text_center(&out.shapes, app.language.translate(label)).expect(label);
        layers_click(ctx, app, at);
    }
    fn fixture() -> (PathBuf, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("oa-file-open-{}", storage::new_document_identity()));
        std::fs::create_dir_all(&root).unwrap();
        let target = root.join("selected.openarchi.json");
        let project = Project {
            name: "Selected project".into(),
            ..Project::default()
        };
        storage::save(&project, &target).unwrap();
        (root, target)
    }
    fn choose(ctx: &Context, app: &mut AtelierApp, target: Option<PathBuf>) {
        app.project_open.choices.push_back(target);
        layers_frame_output(ctx, app, vec![key(Key::O, Modifiers::COMMAND)]);
        layers_frame_output(ctx, app, vec![]);
    }
    #[test]
    fn open_picker_precedes_unsaved_prompt_and_cancel_preserves_full_editor_bilingual() {
        for lang in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (root, target) = fixture();
            let (ctx, mut app) = setup();
            app.language = lang;
            shortcut_test_wall(&mut app);
            let before = app.host.document.editor.project.clone();
            let session = app.host.document.editor.document_session();
            let revision = app.host.document.editor.revision;
            let undo = app.host.document.editor.undo.len();
            let old_path = app.path.clone();
            // The selector has not returned: no app-level preflight/replacement dialog.
            let out = layers_frame_output(&ctx, &mut app, vec![key(Key::O, Modifiers::COMMAND)]);
            assert!(text_center(&out.shapes, lang.translate("Unsaved changes")).is_none());
            assert!(text_center(&out.shapes, lang.translate("Open / replace")).is_none());
            assert!(text_center(&out.shapes, &old_path).is_none());
            app.project_open.choices.push_back(None);
            layers_frame_output(&ctx, &mut app, vec![]);
            assert!(app.file_action.is_none());
            assert_eq!(app.host.document.editor.project, before);
            choose(&ctx, &mut app, Some(target.clone()));
            button(&ctx, &mut app, "Cancel");
            assert_eq!(app.host.document.editor.project, before);
            assert_eq!(app.host.document.editor.document_session(), session);
            assert_eq!(app.host.document.editor.revision, revision);
            assert_eq!(app.host.document.editor.undo.len(), undo);
            assert_eq!(app.path, old_path);
            assert!(app.host.document.editor.dirty());
            choose(&ctx, &mut app, Some(target.clone()));
            let status = app.host.document.editor.status.clone();
            let selection = app.host.document.editor.selection_ids();
            layers_frame_output(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
            assert!(app.file_action.is_none());
            assert_eq!(app.host.document.editor.project, before);
            assert_eq!(app.host.document.editor.document_session(), session);
            assert_eq!(app.host.document.editor.revision, revision);
            assert_eq!(app.host.document.editor.selection_ids(), selection);
            assert_eq!(
                app.host.document.editor.status, status,
                "Escape must not reach the old canvas"
            );
            choose(&ctx, &mut app, Some(target));
            button(&ctx, &mut app, "Don't Save");
            assert!(app.file_action.is_none());
            assert_eq!(app.host.document.editor.project.name, "Selected project");
            assert!(app.host.document.editor.undo.is_empty());
            assert!(!app.host.document.editor.dirty());
            std::fs::remove_dir_all(&root).unwrap();
        }
    }
    #[test]
    fn open_save_first_save_cancel_and_save_failure_never_replace_bilingual() {
        for lang in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (root, target) = fixture();
            let (ctx, mut app) = setup();
            app.language = lang;
            shortcut_test_wall(&mut app);
            let before = app.host.document.editor.project.clone();
            choose(&ctx, &mut app, Some(target.clone()));
            app.project_open.choices.push_back(None);
            button(&ctx, &mut app, "Save");
            assert_eq!(app.host.document.editor.project, before);
            assert!(app.file_action.is_some());
            // Saving under a nonexistent parent fails; selection and old project survive.
            app.project_open
                .choices
                .push_back(Some(root.join("missing/failed.json")));
            button(&ctx, &mut app, "Save");
            assert!(!app.file_error.is_empty());
            assert_eq!(app.host.document.editor.project, before);
            assert!(app.host.document.editor.dirty());
            let saved = root.join("current.openarchi.json");
            app.project_open.choices.push_back(Some(saved.clone()));
            button(&ctx, &mut app, "Save");
            assert!(app.file_action.is_none(), "{}", app.file_error);
            assert_eq!(storage::load(&saved).unwrap(), before);
            assert_eq!(app.path, target.display().to_string());
            assert_eq!(app.host.document.editor.project.name, "Selected project");
            // Reopen a bound dirty document: Save must use the current file, not the target.
            open_test_project(&ctx, &mut app, &saved);
            let mut changed = app.host.document.editor.project.clone();
            changed.name = "Saved bound changes".into();
            let original = std::mem::replace(&mut app.host.document.editor.project, changed);
            app.host.document.editor.commit(original);
            choose(&ctx, &mut app, Some(target.clone()));
            button(&ctx, &mut app, "Save");
            assert_eq!(storage::load(&saved).unwrap().name, "Saved bound changes");
            assert_eq!(storage::load(&target).unwrap().name, "Selected project");
            std::fs::remove_dir_all(&root).unwrap();
        }
    }
    #[test]
    fn open_recent_directory_persists_and_bad_target_cannot_change_it_or_editor() {
        let (root, target) = fixture();
        let (ctx, mut app) = setup();
        let profile = root.join("profile");
        app.project_open.initialize(&profile);
        open_test_project(&ctx, &mut app, &target);
        assert_eq!(app.project_open.directory.as_deref(), Some(root.as_path()));
        let mut next = AtelierApp::from_renderer(None, "directory reload".into());
        next.project_open.initialize(&profile);
        assert_eq!(next.open_project_directory(), root);
        let before = app.host.context();
        let path = app.path.clone();
        let invalid = root.join("broken.json");
        std::fs::write(&invalid, b"{broken").unwrap();
        choose(&ctx, &mut app, Some(invalid));
        assert!(!app.file_error.is_empty());
        assert_eq!(app.path, path);
        assert_eq!(app.host.context(), before);
        button(&ctx, &mut app, "Cancel");
        assert_eq!(app.project_open.directory.as_deref(), Some(root.as_path()));
        std::fs::remove_dir_all(&root).unwrap();
        assert_ne!(
            next.open_project_directory(),
            root,
            "missing history directory falls back"
        );
    }
    #[test]
    #[ignore = "uses an explicitly supplied private project and evidence destination"]
    fn open_external_project_roundtrip_preserves_project_and_assets() {
        let input = PathBuf::from(std::env::var_os("OPENARCHI_OPEN_INPUT").expect("input"));
        let root = PathBuf::from(std::env::var_os("OPENARCHI_OPEN_EVIDENCE").expect("evidence"));
        std::fs::create_dir_all(&root).unwrap();
        let original_bytes = std::fs::read(&input).unwrap();
        let expected = Editor::from_stored_document(&input).unwrap().project;
        for lang in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            app.language = lang;
            open_test_project(&ctx, &mut app, &input);
            assert_eq!(app.host.document.editor.project, expected);
            let out = root.join(format!("project-open-roundtrip-{}.openarchi.json", lang.label()));
            app.save_project_to(&out.to_string_lossy()).unwrap();
            let stored = Editor::from_stored_document(&out).unwrap().project;
            let mut bound_expected = expected.clone();
            assert_eq!(
                stored.drawings.cad_sources.len(),
                bound_expected.drawings.cad_sources.len()
            );
            for (new, old) in stored
                .drawings
                .cad_sources
                .iter()
                .zip(&mut bound_expected.drawings.cad_sources)
            {
                for (new_blob, old_blob, extension) in [
                    (&new.original, &mut old.original, "source"),
                    (&new.dxf, &mut old.dxf, "dxf"),
                    (&new.parsed, &mut old.parsed, "cad.json"),
                ] {
                    assert_eq!(new_blob.bytes().unwrap(), old_blob.bytes().unwrap());
                    assert_eq!(new_blob.sha256, old_blob.sha256);
                    let asset_dir = out.parent().unwrap().join(format!(
                        "{}.assets",
                        out.file_name().unwrap().to_string_lossy()
                    ));
                    assert_eq!(
                        new_blob.path,
                        asset_dir.join(format!("{}.{}", old_blob.sha256, extension))
                    );
                    old_blob.path = new_blob.path.clone();
                }
            }
            assert!(
                stored == bound_expected,
                "only verified storage-owned asset paths may rebind on Save As"
            );
            open_test_project(&ctx, &mut app, &out);
            assert!(app.host.document.editor.project == bound_expected);
        }
        assert_eq!(std::fs::read(&input).unwrap(), original_bytes);
    }
}

#[test]
fn ordinary_save_writes_bound_file_without_dialog_and_retains_failure_protection_bilingual() {
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        let (ctx, mut app) = setup();
        app.language = language;
        let root = std::env::temp_dir().join(format!(
            "oa-direct-save-{}",
            storage::new_document_identity()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("project.json");
        // First save still asks where to write, without creating a default file.
        layers_frame_output(&ctx, &mut app, vec![key(Key::S, Modifiers::COMMAND)]);
        assert!(app.file_action == Some(FileAction::Save));
        app.file_action = None;
        app.save_project_to(&path.to_string_lossy()).unwrap();
        let before = app.host.document.editor.project.clone();
        shortcut_test_wall(&mut app);
        app.host.document.editor.commit(before);
        let expected = app.host.document.editor.project.clone();
        let revision = app.host.document.editor.revision;
        let selection = app.host.document.editor.selection_ids();
        let undo = app.host.document.editor.undo.len();
        layers_frame_output(&ctx, &mut app, vec![]);
        layers_frame_output(&ctx, &mut app, vec![key(Key::S, Modifiers::COMMAND)]);
        let out = layers_frame_output(&ctx, &mut app, vec![]);
        assert!(app.file_action.is_none(), "{}", app.file_error);
        let dialog_caption = language
            .translate("Project file · {format}")
            .replace("{format}", "OpenArchi JSON");
        assert!(text_center(&out.shapes, &dialog_caption).is_none());
        assert_eq!(storage::load(&path).unwrap(), expected);
        assert!(!app.host.document.editor.dirty());
        assert_eq!(app.host.document.editor.revision, revision);
        assert_eq!(app.host.document.editor.selection_ids(), selection);
        assert_eq!(app.host.document.editor.undo.len(), undo);
        // Ordinary workbench/native-menu/custom-shortcut entry uses the same policy.
        app.host.document.editor.undo();
        let undone = app.host.document.editor.project.clone();
        app.dispatch_workbench(crate::workbench::WorkbenchCommand::Save);
        assert!(app.file_action.is_none());
        assert_eq!(storage::load(&path).unwrap(), undone);
        app.host.document.editor.redo();
        assert_eq!(app.host.document.editor.project, expected);
        // An external edit must remain protected even when no confirmation is needed.
        let external = b"external change must not be overwritten";
        std::fs::write(&path, external).unwrap();
        layers_frame_output(&ctx, &mut app, vec![key(Key::S, Modifiers::COMMAND)]);
        assert!(app.file_action == Some(FileAction::Save));
        assert!(!app.file_error.is_empty());
        assert!(app.host.document.editor.dirty());
        assert_eq!(app.host.document.editor.project, expected);
        assert_eq!(std::fs::read(&path).unwrap(), external);
        std::fs::remove_dir_all(root).unwrap();
    }
}
