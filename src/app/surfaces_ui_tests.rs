mod zone_surfaces_product_path {
    use super::*;
    fn press(ctx: &Context, app: &mut AtelierApp, key: &str) {
        for _ in 0..4 {
            layers_frame_output(ctx, app, vec![]);
        }
        let out = layers_frame_output(ctx, app, vec![]);
        let at = out
            .shapes
            .iter()
            .rev()
            .find_map(|s| {
                text_center(std::slice::from_ref(s), app.language.translate(key))
                    .filter(|p| s.clip_rect.contains(*p))
            })
            .unwrap_or_else(|| panic!("Missing product control {key}"));
        layers_click(ctx, app, at);
    }
    #[test]
    fn zone_surfaces_canvas_controls_freeze_targets_cancel_commit_undo_and_restore_bilingual() {
        use crate::model::ModelAdapter;
        use crate::view_settings::Model3dStyle;
        use crate::zones::{ZoneSpec, surfaces::ZoneSurface};
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            let root = std::env::temp_dir().join(format!(
                "oa-surface-ui-{}",
                storage::new_document_identity()
            ));
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            app.language = language;
            app.view = View::Model;
            for x in [0., 12.] {
                app.host
                    .document
                    .editor
                    .place_zone(ZoneSpec {
                        surface: Default::default(),
                        purpose: "concept".into(),
                        outline: vec![
                            Point::new(x, 0.),
                            Point::new(x + 10., 0.),
                            Point::new(x + 8., 8.),
                            Point::new(x, 6.),
                        ],
                        holes: vec![],
                        height: 3.,
                        bottom_offset: 0.,
                        label_position: None,
                    })
                    .unwrap();
            }
            app.host.document.editor.set_selection(vec![]);
            let before = app.host.document.editor.project.clone();
            press(&ctx, &mut app, "Wireframe");
            assert_eq!(
                app.host.document.editor.project.plan_options.model_3d_style,
                Model3dStyle::Wireframe
            );
            press(&ctx, &mut app, "Shaded with edges");
            press(&ctx, &mut app, "Randomize space surfaces");
            assert!(app.process_ui.preview.is_some());
            assert_eq!(app.host.document.editor.project, before);
            press(&ctx, &mut app, "Cancel design proposal");
            assert!(app.process_ui.preview.is_none());
            assert_eq!(app.host.document.editor.project, before);
            press(&ctx, &mut app, "Randomize space surfaces");
            let ids: Vec<_> = app
                .process_ui
                .preview
                .as_ref()
                .unwrap()
                .delta
                .elements
                .iter()
                .map(|e| e.id)
                .collect();
            assert_eq!(ids.len(), 2);
            // Selection is context, never authorization to retarget the frozen preview.
            app.host.document.editor.set_selection(vec![ids[0]]);
            press(&ctx, &mut app, "Confirm design change");
            assert!(app.process_ui.preview.is_none(), "{}", app.process_ui.error);
            let colored = app.host.document.editor.project.clone();
            for e in &colored.elements {
                let Shape::Zone { spec } = &e.shape else {
                    panic!()
                };
                assert!(matches!(spec.surface, ZoneSurface::Solid { .. }));
            }
            app.host.document.editor.undo();
            assert_eq!(app.host.document.editor.project, before);
            app.host.document.editor.redo();
            assert_eq!(app.host.document.editor.project, colored);
            let bytes = storage::JsonAdapter.export(&colored).unwrap();
            assert_eq!(storage::JsonAdapter.import(&bytes).unwrap(), colored);
            // A user write after preparation invalidates the proposal rather than silently refreshing it.
            app.host.document.editor.set_selection(vec![]);
            press(&ctx, &mut app, "Randomize space surfaces");
            let e = app.host.document.editor.project.elements[0].clone();
            let mut changed = e.clone();
            changed.name = "User edit".into();
            app.host.document.editor.replace_element(changed).unwrap();
            assert!(app.process_commit_proposal().is_err());
            assert_eq!(
                app.host.document.editor.project.elements[0].name,
                "User edit"
            );
            // A surface label cannot authorize a geometry edit in a standalone project.
            app.process_ui.preview = None;
            let mut forged = app.host.document.editor.project.elements[0].clone();
            let Shape::Zone { spec } = &mut forged.shape else {
                panic!()
            };
            spec.height += 1.;
            let editor = &app.host.document.editor;
            let change = editor
                .prepare_change(
                    editor.revision,
                    vec![crate::command::ModelCommand::UpdateElement { element: forged }],
                )
                .unwrap();
            let response = app.host.confirm_task_prepared(
                app.host.context(),
                &storage::new_document_identity(),
                "Assign space surfaces",
                std::rc::Rc::new(change),
            );
            assert_eq!(response["error"]["code"], "DESIGN_TASK_MISMATCH");
            app.process_ui.preview = None;
            app.host
                .document
                .editor
                .project
                .edit_state
                .locked
                .insert(ids[0]);
            let protected = app.host.document.editor.project.clone();
            press(&ctx, &mut app, "Randomize space surfaces");
            assert!(app.process_ui.preview.is_none());
            assert!(!app.process_ui.error.is_empty());
            assert_eq!(app.host.document.editor.status, app.process_ui.error);
            assert_eq!(
                app.host.document.editor.project, protected,
                "no partial assignment when one space is protected"
            );
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
