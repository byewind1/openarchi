mod drawing_preview_regression {
    use super::*;

    fn segments(shapes: &[egui::epaint::ClippedShape]) -> Vec<[Pos2; 2]> {
        fn collect(s: &egui::Shape, out: &mut Vec<[Pos2; 2]>) {
            match s {
                egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
                egui::Shape::LineSegment { points, stroke }
                    if stroke.color == ACCENT && stroke.width > 0. =>
                {
                    out.push(*points)
                }
                egui::Shape::Path(p)
                    if p.stroke.width > 0.
                        && p.stroke.color == egui::epaint::ColorMode::Solid(ACCENT) =>
                {
                    out.extend(p.points.windows(2).map(|w| [w[0], w[1]]));
                    if p.closed && p.points.len() > 2 {
                        out.push([*p.points.last().unwrap(), p.points[0]]);
                    }
                }
                _ => {}
            }
        }
        let mut out = vec![];
        for s in shapes {
            collect(&s.shape, &mut out);
        }
        out
    }
    fn same(a: [Pos2; 2], b: [Pos2; 2]) -> bool {
        (a[0].distance(b[0]) < 0.02 && a[1].distance(b[1]) < 0.02)
            || (a[0].distance(b[1]) < 0.02 && a[1].distance(b[0]) < 0.02)
    }
    fn expect(out: &egui::FullOutput, points: &[Pos2], closed: bool) {
        let edges = segments(&out.shapes);
        let expected: Vec<_> = points
            .windows(2)
            .map(|w| [w[0], w[1]])
            .chain((closed && points.len() > 2).then(|| [*points.last().unwrap(), points[0]]))
            .collect();
        for e in &expected {
            assert_eq!(
                edges.iter().filter(|a| same(**a, *e)).count(),
                1,
                "one visible edge {e:?}; actual {edges:?}"
            );
        }
        // Reject every extra contour-length stroke within the drawing region,
        // including a closing stroke from a fill layer or duplicate layer.
        let region = Rect::from_min_max(pos2(250., 100.), pos2(650., 450.));
        for edge in edges
            .iter()
            .filter(|e| e[0].distance(e[1]) > 20. && region.contains(e[0]) && region.contains(e[1]))
        {
            assert!(
                expected.iter().any(|e| same(*e, *edge)),
                "unexpected stroke {edge:?}; expected {expected:?}"
            );
        }
    }
    fn configure(app: &mut AtelierApp, tool: Tool, polygon: bool) {
        app.host.document.editor.set_tool(tool);
        app.host.document.editor.snap = false;
        app.host.document.editor.zone_polygon = polygon;
        app.host.document.editor.slab_method = if polygon {
            crate::slab_creation::SlabGeometryMethod::Polygonal
        } else {
            crate::slab_creation::SlabGeometryMethod::Rectangular
        };
    }
    fn moved(ctx: &Context, app: &mut AtelierApp, pos: Pos2) -> egui::FullOutput {
        plan_frame_output(ctx, app, vec![Event::PointerMoved(pos)])
    }
    fn roundtrip(app: &mut AtelierApp, before: &Project) {
        let after = app.host.document.editor.project.clone();
        assert_eq!(after.elements.len(), before.elements.len() + 1);
        app.host.document.editor.undo();
        assert_eq!(&app.host.document.editor.project, before);
        app.host.document.editor.redo();
        assert_eq!(app.host.document.editor.project, after);
        let path = std::env::temp_dir().join(format!(
            "oa-preview-{}-{:?}.json",
            storage::new_document_identity(),
            app.host.document.editor.tool
        ));
        storage::save(&after, &path).unwrap();
        assert_eq!(storage::load(&path).unwrap(), after);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn drawing_preview_rectangles_numeric_backtrack_cancel_history_storage() {
        for tool in [Tool::Slab, Tool::Zone, Tool::ConceptMass] {
            let (ctx, mut app) = setup();
            configure(&mut app, tool, false);
            let before = app.host.document.editor.project.clone();
            let a = pos2(300., 350.);
            let q = pos2(550., 200.);
            click(&ctx, &mut app, a);
            expect(
                &moved(&ctx, &mut app, q),
                &[a, pos2(q.x, a.y), q, pos2(a.x, q.y)],
                true,
            );
            assert_eq!(app.host.document.editor.project, before);
            // Locked values use the existing numeric target while the mouse is elsewhere.
            app.tracker_locked = true;
            app.tracker_a = 4.;
            app.tracker_b = 2.;
            let target = pos2(500., 250.);
            expect(
                &moved(&ctx, &mut app, q),
                &[a, pos2(target.x, a.y), target, pos2(a.x, target.y)],
                true,
            );
            plan_frame(&ctx, &mut app, vec![key(Key::Backspace, Modifiers::NONE)]);
            expect(&moved(&ctx, &mut app, q), &[], false);
            click(&ctx, &mut app, a);
            plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
            expect(&moved(&ctx, &mut app, q), &[], false);
            assert_eq!(app.host.document.editor.project, before);
            configure(&mut app, tool, false);
            click(&ctx, &mut app, a);
            click(&ctx, &mut app, q);
            let points = match &app
                .host
                .document
                .editor
                .project
                .elements
                .last()
                .unwrap()
                .shape
            {
                Shape::Zone { spec } => spec.outline.clone(),
                Shape::ConceptMass { spec } => spec.outline.clone(),
                _ => geometry::footprint(app.host.document.editor.project.elements.last().unwrap())
                    .to_vec(),
            };
            let wanted = [a, pos2(q.x, a.y), q, pos2(a.x, q.y)]
                .map(|p| app.world(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.)), p));
            assert_eq!(points.len(), 4);
            for p in wanted {
                assert!(points.iter().any(|v| v.distance(p) < 1e-9));
            }
            roundtrip(&mut app, &before);
        }
    }
    #[test]
    fn drawing_preview_chains_first_segment_multiple_nodes_backtrack_close_or_finish() {
        for tool in [Tool::Slab, Tool::Zone, Tool::ConceptMass, Tool::Polyline] {
            let (ctx, mut app) = setup();
            configure(&mut app, tool, true);
            let before = app.host.document.editor.project.clone();
            let a = pos2(300., 350.);
            let b = pos2(550., 350.);
            let c = pos2(550., 150.);
            let q = pos2(400., 180.);
            click(&ctx, &mut app, a);
            expect(&moved(&ctx, &mut app, b), &[a, b], false);
            click(&ctx, &mut app, b);
            expect(&moved(&ctx, &mut app, c), &[a, b, c], false);
            click(&ctx, &mut app, c);
            expect(&moved(&ctx, &mut app, q), &[a, b, c, q], false);
            assert_eq!(app.host.document.editor.project, before);
            plan_frame(&ctx, &mut app, vec![key(Key::Backspace, Modifiers::NONE)]);
            expect(&moved(&ctx, &mut app, q), &[a, b, q], false);
            click(&ctx, &mut app, c);
            // At the exact first node only the current segment returns to it.
            expect(&moved(&ctx, &mut app, a), &[a, b, c, a], false);
            if tool == Tool::Polyline {
                plan_frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
                let Shape::Polyline { points, .. } = &app
                    .host
                    .document
                    .editor
                    .project
                    .elements
                    .last()
                    .unwrap()
                    .shape
                else {
                    panic!()
                };
                assert_eq!(points.len(), 3);
                assert_ne!(points.first(), points.last());
            } else {
                click(&ctx, &mut app, a);
            }
            roundtrip(&mut app, &before);
            configure(&mut app, tool, true);
            click(&ctx, &mut app, a);
            click(&ctx, &mut app, b);
            plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
            expect(&moved(&ctx, &mut app, q), &[], false);
        }
    }
    #[test]
    fn drawing_preview_rotated_rectangle_keeps_construction_baseline() {
        let (ctx, mut app) = setup();
        configure(&mut app, Tool::Slab, false);
        app.host.document.editor.slab_method =
            crate::slab_creation::SlabGeometryMethod::RotatedRectangle;
        let a = pos2(300., 350.);
        let b = pos2(550., 300.);
        click(&ctx, &mut app, a);
        expect(&moved(&ctx, &mut app, b), &[a, b], false);
        click(&ctx, &mut app, b);
        let q = pos2(520., 150.);
        let out = moved(&ctx, &mut app, q);
        let candidate = app
            .host
            .document
            .editor
            .rotated_slab_preview(app.hover_point)
            .unwrap();
        let derived = crate::slab_geometry::derive(&candidate, 0.).unwrap();
        let corners = &derived.plan_paths[0];
        let screen: Vec<_> = corners
            .iter()
            .map(|p| app.screen(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.)), *p))
            .collect();
        let edges = segments(&out.shapes);
        for pair in screen.iter().zip(screen.iter().cycle().skip(1)).take(4) {
            assert!(edges.iter().any(|e| same(*e, [*pair.0, *pair.1])));
        }
        assert!(!edges.iter().any(|e| same(*e, [a, q])));
        plan_frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        expect(&moved(&ctx, &mut app, q), &[], false);
    }
    #[test]
    fn drawing_preview_roof_contour_keeps_open_input_and_candidate_fill() {
        let (ctx, mut app) = setup();
        app.host.document.editor.set_tool(Tool::Roof);
        app.host.document.editor.snap = false;
        for point in [Point::new(-2., 0.), Point::new(8., 0.), Point::new(0., 2.)] {
            app.host.document.editor.click(point, 0.01);
        }
        app.host.document.editor.begin_roof_polygon().unwrap();
        let a = pos2(300., 350.);
        let b = pos2(550., 350.);
        let q = pos2(550., 150.);
        click(&ctx, &mut app, a);
        click(&ctx, &mut app, b);
        let out = moved(&ctx, &mut app, q);
        expect(&out, &[a, b, q], false);
        assert!(out.shapes.iter().any(|s| matches!(&s.shape,egui::Shape::Mesh(m) if !m.indices.is_empty() && m.vertices.iter().all(|v|v.color==Color32::from_rgba_unmultiplied(39,146,137,50)))));
    }
    // Optional review artifact: exact egui draw-output strokes, never a mockup.
    fn capture(out: &egui::FullOutput, name: &str) {
        let Ok(root) = std::env::var("OPENARCHI_PREVIEW_CAPTURE_DIR") else {
            return;
        };
        let edges = segments(&out.shapes);
        std::fs::create_dir_all(&root).unwrap();
        let mut svg = String::from(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"250 100 400 350\"><rect x=\"250\" y=\"100\" width=\"400\" height=\"350\" fill=\"#fff\"/>",
        );
        for [a, b] in &edges {
            svg.push_str(&format!("<path d=\"M {} {} L {} {}\" fill=\"none\" stroke=\"#279289\" stroke-width=\"1.5\"/>",a.x,a.y,b.x,b.y));
        }
        svg.push_str("</svg>");
        std::fs::write(std::path::Path::new(&root).join(format!("{name}.svg")), svg).unwrap();
        let data: Vec<_> = edges.iter().map(|[a, b]| [a.x, a.y, b.x, b.y]).collect();
        std::fs::write(
            std::path::Path::new(&root).join(format!("{name}.json")),
            serde_json::to_vec_pretty(&data).unwrap(),
        )
        .unwrap();
    }
    #[test]
    fn drawing_preview_review_frames_do_not_mutate_document() {
        for tool in [Tool::Slab, Tool::Zone, Tool::ConceptMass, Tool::Polyline] {
            for polygon in [false, true] {
                if tool == Tool::Polyline && !polygon {
                    continue;
                }
                let (ctx, mut app) = setup();
                configure(&mut app, tool, polygon);
                let before = app.host.document.editor.project.clone();
                click(&ctx, &mut app, pos2(300., 350.));
                let out = moved(&ctx, &mut app, pos2(550., 200.));
                capture(
                    &out,
                    &format!("{tool:?}-{}-第一段", if polygon { "逐点" } else { "矩形" }),
                );
                if polygon {
                    click(&ctx, &mut app, pos2(550., 350.));
                    let out = moved(&ctx, &mut app, pos2(550., 150.));
                    capture(&out, &format!("{tool:?}-逐点-两节点"));
                }
                assert_eq!(app.host.document.editor.project, before);
                assert!(app.host.document.editor.undo.is_empty());
            }
        }
    }
}
