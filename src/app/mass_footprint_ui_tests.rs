mod mass_footprint_product_path {
    use super::super::wall_node_ui::WallPetCommand as C;
    use super::*;
    use crate::diagrams::{Domain, Status};
    fn press(ctx: &Context, app: &mut AtelierApp, key: &str) {
        reveal_manual_control(ctx, app, key);
        // The existing auto-sized proposal windows need their layout frames
        // before a physical pointer target is stable (same as mass-space tests).
        for _ in 0..20 {
            layers_frame_output(ctx, app, vec![]);
        }
        for _ in 0..12 {
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
                let mut rect = Rect::from_min_size(t.pos, t.galley.size()).intersect(s.clip_rect);
                if let Some(bounds) = bounds {
                    rect = rect.intersect(bounds);
                }
                rect.is_positive().then(|| rect.center())
            }) {
                layers_click(ctx, app, at);
                eprintln!(
                    "MASS_FOOTPRINT_CONTROL {key} preview={:?} error={} status={}",
                    app.process_ui.preview.as_ref().map(|p| p.label),
                    app.process_ui.error,
                    app.host.document.editor.status
                );
                return;
            }
        }
        let out = layers_frame_output(ctx, app, vec![]);
        let mut texts = vec![];
        collect_text(&out.shapes, &mut texts);
        panic!(
            "{key}: {} / {} / {texts:?}",
            app.host.document.editor.status, app.process_ui.error
        );
    }
    fn select_source(ctx: &Context, app: &mut AtelierApp, id: u64) {
        let caption = app
            .mass_space_canvas_selection()
            .map(|e| format!("#{} · {}", e.id, e.name))
            .unwrap_or_else(|| app.language.translate("Select a source mass").into());
        press(ctx, app, &caption);
        let e = app
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .find(|e| e.id == id)
            .unwrap();
        let caption = format!("#{id} · {}", e.name);
        press(ctx, app, &caption);
        assert_eq!(app.host.document.editor.selection_ids(), vec![id]);
    }
    fn at(ctx: &Context, app: &mut AtelierApp, p: Point) {
        for _ in 0..3 {
            layers_frame_output(ctx, app, vec![]);
        }
        let at = app.screen(app.workbench.panel_rects["view.canvas"], p);
        assert!(
            app.workbench.panel_rects["view.canvas"].contains(at),
            "model target outside normal canvas: {p:?}/{at:?}"
        );
        layers_click(ctx, app, at);
    }
    fn pet(ctx: &Context, app: &mut AtelierApp, c: C) {
        for _ in 0..3 {
            layers_frame_output(ctx, app, vec![]);
        }
        let r = ctx
            .data(|d| d.get_temp::<Rect>(Id::new(("wall_pet", c as u8))))
            .expect("actual rendered pet control");
        layers_click(ctx, app, r.center());
        assert_eq!(
            app.wall_insert
                .as_ref()
                .expect(&app.host.document.editor.status)
                .command,
            c
        );
    }
    fn area(r: &[Point]) -> f64 {
        r.iter()
            .zip(r.iter().cycle().skip(1))
            .take(r.len())
            .map(|(a, b)| a.x * b.y - a.y * b.x)
            .sum::<f64>()
            .abs()
            * 0.5
    }
    fn fixture(root: &std::path::Path) -> std::path::PathBuf {
        // Authored public geometry, no image interpretation/AI claim. Tested steps start at Open.
        let image = root.join("authored.png");
        image::RgbaImage::from_pixel(40, 30, image::Rgba([220, 205, 170, 255]))
            .save(&image)
            .unwrap();
        let mut p = storage::synthetic_building_fixture();
        p.drawings
            .pages
            .push(crate::drawings::import(&image, &root.join("source-assets"), 1).unwrap());
        let mut plan = super::intent_product_path::fixture_mass_plan(&p.drawings.pages[0]);
        for group in plan["draft"]["candidates"].as_array_mut().unwrap() {
            for seed in group.as_array_mut().unwrap() {
                seed["material"] = serde_json::json!(Material::Plaster);
            }
        }
        let mut e = Editor::new(p);
        let prepared = crate::design_process::intent::prepare_mass(
            &e,
            plan["task_id"].as_str().unwrap(),
            serde_json::from_value(plan["draft"].clone()).unwrap(),
        )
        .unwrap();
        e.commit_change("authored-footprint-fixture", &prepared)
            .unwrap();
        let m = e
            .project
            .elements
            .iter()
            .find(|m| {
                m.design_option == e.project.plan_options.active_design_option
                    && matches!(m.shape, Shape::ConceptMass { .. })
            })
            .unwrap()
            .clone();
        e.apply(crate::command::ModelCommand::MassSpaces {
            edit: crate::mass_spaces::Edit::Create {
                mass: m.id,
                expected_source_sha256: crate::mass_spaces::element_stamp(&e.project, &m).unwrap(),
                name: "Authored linked concept space".into(),
                purpose: "Unknown".into(),
                height: 2.8,
                assumption: "Authored concept assumption, not survey or approved use".into(),
            },
        })
        .unwrap();
        let path = root.join("starting.openarchi.json");
        storage::save(&e.project, &path).unwrap();
        path
    }
    #[test]
    fn normal_open_mass_courtyard_cancel_inner_node_link_figure_history_and_save_bilingual() {
        let supplied =
            std::env::var_os("OPENARCHI_MASS_FOOTPRINT_INPUT").map(std::path::PathBuf::from);
        let destination =
            std::env::var_os("OPENARCHI_MASS_FOOTPRINT_OUTPUT").map(std::path::PathBuf::from);
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app) = setup();
            app.language = language;
            ctx.global_style_mut(|s| s.animation_time = 0.);
            let parent = destination.clone().unwrap_or_else(|| {
                std::env::temp_dir().join(format!(
                    "oa-mass-footprint-{}",
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
                .expect("preserve previous result, never replay unknown writes");
            let input = supplied.clone().unwrap_or_else(|| fixture(&root));
            let input_bytes = std::fs::read(&input).unwrap();
            app.host
                .configure_operation_journal(Some(root.join("journal")))
                .unwrap();
            open_test_project(&ctx, &mut app, &input);
            press(&ctx, &mut app, "2D Plan");
            layers_frame_output(&ctx, &mut app, vec![key(Key::F, Modifiers::NONE)]);
            let p = &app.host.document.editor.project;
            let task = p.design_process.task().unwrap().clone();
            let (mass, link) = p
                .design_process
                .mass_spaces
                .iter()
                .filter_map(|l| {
                    let m = p.elements.iter().find(|m| {
                        m.id == l.mass && m.design_option == p.plan_options.active_design_option
                    })?;
                    let Shape::ConceptMass { spec } = &m.shape else {
                        return None;
                    };
                    (spec.holes.is_empty()).then_some((m.clone(), l.clone()))
                })
                .max_by(|(a, _), (b, _)| {
                    let Shape::ConceptMass { spec: a } = &a.shape else {
                        unreachable!()
                    };
                    let Shape::ConceptMass { spec: b } = &b.shape else {
                        unreachable!()
                    };
                    a.height.total_cmp(&b.height)
                })
                .expect("current saved candidate with linked straight mass");
            let source_id = mass.id;
            let Shape::ConceptMass {
                spec: original_spec,
            } = &mass.shape
            else {
                unreachable!()
            };
            let (min, max) = crate::slab_geometry::bounds(&original_spec.outline);
            assert!(max.x - min.x >= 5. && max.y - min.y >= 4.);
            let cx = ((min.x + max.x) * 0.5).round() + 1.;
            let cy = ((min.y + max.y) * 0.5).round();
            let hole = [
                Point::new(cx - 1., cy - 1.),
                Point::new(cx + 1., cy - 1.),
                Point::new(cx + 1., cy + 1.),
                Point::new(cx - 1., cy + 1.),
            ];
            assert!(
                hole.iter()
                    .all(|p| p.x > min.x && p.x < max.x && p.y > min.y && p.y < max.y)
            );
            let edge = Point::new((min.x + max.x) * 0.5, min.y);
            select_source(&ctx, &mut app, source_id);
            press(&ctx, &mut app, "Select linked space");
            let existing = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .iter()
                .find(|f| {
                    f.definition.domain == Domain::Zones
                        && f.definition.option == mass.design_option
                        && f.definition.story == mass.story
                })
                .cloned();
            let figure = if let Some(f) = existing {
                f
            } else {
                press(&ctx, &mut app, "Preview space quantity figure");
                press(&ctx, &mut app, "Confirm design change");
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
            press(&ctx, &mut app, "Return to source mass");
            let before = app.host.document.editor.project.clone();
            let history = app.host.document.editor.undo.len();
            at(&ctx, &mut app, edge);
            assert!(
                app.wall_insert.is_some(),
                "{} view={:?} selection={:?} edge={edge:?}",
                app.host.document.editor.status,
                app.view,
                app.host.document.editor.selection_ids()
            );
            pet(&ctx, &mut app, C::SubtractPolygon);
            for p in &hole[..3] {
                at(&ctx, &mut app, *p);
                assert_eq!(app.host.document.editor.project, before);
            }
            layers_frame_output(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
            assert!(app.wall_insert.is_none());
            assert_eq!(app.host.document.editor.project, before);
            assert_eq!(app.host.document.editor.undo.len(), history);
            at(&ctx, &mut app, edge);
            pet(&ctx, &mut app, C::SubtractPolygon);
            for p in hole {
                at(&ctx, &mut app, p);
                assert_eq!(app.host.document.editor.project, before);
            }
            layers_frame_output(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
            assert!(
                app.wall_insert.is_none(),
                "{}",
                app.host.document.editor.status
            );
            let cut = app.host.document.editor.project.clone();
            let edited = cut.elements.iter().find(|e| e.id == source_id).unwrap();
            let Shape::ConceptMass { spec } = &edited.shape else {
                panic!("real mass retained")
            };
            assert_eq!(spec.holes.len(), 1);
            assert!(
                (area(&spec.holes[0]) - 4.).abs() < 1e-4,
                "actual hole {:?}",
                spec.holes[0]
            );
            assert!((spec.area() - (original_spec.area() - 4.)).abs() < 1e-4);
            assert_eq!(spec.height, original_spec.height);
            assert_eq!(spec.bottom_offset, original_spec.bottom_offset);
            assert!(edited.slab_contour.is_none());
            assert_eq!(edited.design_option, mass.design_option);
            assert_eq!(edited.story, mass.story);
            assert_eq!(edited.material, mass.material);
            for e in &before.elements {
                if e.id != source_id {
                    assert_eq!(cut.elements.iter().find(|x| x.id == e.id), Some(e));
                }
            }
            assert_eq!(app.host.document.editor.undo.len(), history + 1);
            assert!(matches!(
                app.host.document.editor.undo.last().unwrap().command,
                crate::command::ModelCommand::UpdateElement { .. }
            ));
            assert_eq!(
                crate::mass_spaces::status(&cut, &link),
                "Mass-space source changed"
            );
            assert_eq!(figure.status(&cut), Status::Stale);
            // Inner handles enter the same real pet session; cancel preserves it.
            at(&ctx, &mut app, spec.holes[0][0]);
            assert_eq!(app.wall_insert.as_ref().unwrap().command, C::MoveNode);
            layers_frame_output(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
            assert_eq!(app.host.document.editor.project, cut);
            layers_frame_output(&ctx, &mut app, vec![key(Key::Z, Modifiers::COMMAND)]);
            assert_eq!(app.host.document.editor.project, before);
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::Z, Modifiers::COMMAND | Modifiers::ALT)],
            );
            assert_eq!(app.host.document.editor.project, cut);
            select_source(&ctx, &mut app, source_id);
            press(&ctx, &mut app, "Preview space update");
            press(&ctx, &mut app, "Confirm design change");
            let p = &app.host.document.editor.project;
            let z = p.elements.iter().find(|e| e.id == link.zone).unwrap();
            let Shape::Zone { spec: z } = &z.shape else {
                panic!()
            };
            assert_eq!(z.outline, spec.outline);
            assert_eq!(z.holes, spec.holes);
            assert_eq!(figure.status(p), Status::Stale);
            press(&ctx, &mut app, "Show space quantity figure");
            press(&ctx, &mut app, "Preview figure update");
            assert_eq!(
                app.process_ui.preview.as_ref().map(|p| p.label),
                Some("Update linked model figure"),
                "{} / {}",
                app.process_ui.error,
                app.host.document.editor.status
            );
            press(&ctx, &mut app, "Confirm design change");
            assert!(
                app.process_ui.preview.is_none(),
                "{} / {}",
                app.process_ui.error,
                app.host.document.editor.status
            );
            let updated = app
                .host
                .document
                .editor
                .project
                .drawings
                .diagrams
                .iter()
                .find(|f| f.id == figure.id)
                .unwrap();
            assert_eq!(updated.generation, figure.generation + 1);
            assert_eq!(
                updated.status(&app.host.document.editor.project),
                Status::Current
            );
            let after = app.host.document.editor.project.clone();
            let candidate = task
                .candidates
                .iter()
                .find(|c| Some(c.option) == mass.design_option)
                .unwrap();
            let other = task
                .candidates
                .iter()
                .find(|c| Some(c.option) != mass.design_option)
                .unwrap();
            let other_name = after.design_options.name(other.option).unwrap().to_string();
            let active_name = after
                .design_options
                .name(candidate.option)
                .unwrap()
                .to_string();
            press(&ctx, &mut app, &active_name);
            press(&ctx, &mut app, &other_name);
            assert_eq!(app.host.document.editor.project.elements, after.elements);
            press(&ctx, &mut app, &other_name);
            press(&ctx, &mut app, &active_name);
            assert_eq!(app.host.document.editor.project.elements, after.elements);
            select_source(&ctx, &mut app, source_id);
            press(&ctx, &mut app, "Select linked space");
            press(&ctx, &mut app, "Show space quantity figure");
            let output = root.join("continued.openarchi.json");
            layers_frame_output(
                &ctx,
                &mut app,
                vec![key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT)],
            );
            let old = app.file_dialog_path.clone().unwrap();
            press(&ctx, &mut app, &old);
            layers_frame_output(
                &ctx,
                &mut app,
                vec![
                    key(Key::A, Modifiers::COMMAND),
                    Event::Text(output.display().to_string()),
                ],
            );
            press(&ctx, &mut app, "Save");
            assert!(app.file_action.is_none(), "{}", app.file_error);
            let stored = Editor::from_stored_document(&output).unwrap();
            assert_eq!(stored.project.elements, after.elements);
            assert_eq!(stored.project.design_process, after.design_process);
            assert_eq!(stored.project.drawings.diagrams, after.drawings.diagrams);
            for (a, b) in stored
                .project
                .drawings
                .pages
                .iter()
                .zip(&app.host.document.editor.project.drawings.pages)
            {
                assert_eq!(
                    std::fs::read(&a.path).unwrap(),
                    std::fs::read(&b.path).unwrap()
                );
            }
            assert_eq!(std::fs::read(&input).unwrap(), input_bytes);
            std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"entry":"normal Open/current candidate/source picker/canvas pet/preview/Enter/shared link and figure update/Save","input":input,"output":output,"source_mass":source_id,"linked_zone":link.zone,"figure":figure.id,"hole_area_m2":area(&spec.holes[0]),"mass_footprint_m2":spec.area(),"original_elements":before.elements.len(),"unchanged_other_elements":true,"native":"not_run","AI":"not_run; authored fixture or protected continuing model"})).unwrap()).unwrap();
            if destination.is_none() {
                std::fs::remove_dir_all(parent).unwrap();
            }
        }
    }
}
