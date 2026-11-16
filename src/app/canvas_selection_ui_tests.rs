use crate::screen_selection::SelectionPath;

#[test]
fn selection_plan_rotated_views_use_screen_direction_and_preserve_project() {
    for angle in [0., 37., 90.] {
        let (ctx, mut app) = setup();
        let id = app.host.document.editor.project.add(
            0,
            Material::Concrete,
            Shape::Slab {
                min: Point::new(-2., -2.),
                max: Point::new(2., 2.),
                thickness: 0.2,
            },
        );
        app.host
            .document
            .editor
            .project
            .plan_options
            .orientation_degrees = angle;
        app.host.document.editor.set_tool(Tool::Select);
        let r = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
        let corners = geometry::rectangle_polygon(Point::new(-2., -2.), Point::new(2., 2.))
            .into_iter()
            .map(|p| app.screen(r, p))
            .collect::<Vec<_>>();
        let lo = corners
            .iter()
            .copied()
            .fold(pos2(f32::INFINITY, f32::INFINITY), |a, b| a.min(b));
        let hi = corners
            .iter()
            .copied()
            .fold(pos2(f32::NEG_INFINITY, f32::NEG_INFINITY), |a, b| a.max(b));
        let a = lo - vec2(12., 12.);
        let b = pos2(400., hi.y + 12.);
        let before = app.host.document.editor.project.clone();
        for reverse in [false, true] {
            app.host.document.editor.set_selection(vec![]);
            let (a, b) = if reverse { (b, a) } else { (a, b) };
            plan_frame(&ctx, &mut app, vec![Event::PointerMoved(a)]);
            plan_frame(
                &ctx,
                &mut app,
                vec![Event::PointerButton {
                    pos: a,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                }],
            );
            plan_frame(&ctx, &mut app, vec![Event::PointerMoved(b)]);
            assert!(app.canvas_selection_box.is_some());
            assert!(app.host.document.editor.selection_ids().is_empty());
            plan_frame(
                &ctx,
                &mut app,
                vec![Event::PointerButton {
                    pos: b,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                }],
            );
            assert_eq!(
                app.host.document.editor.selection_ids(),
                if reverse { vec![id] } else { vec![] },
                "angle {angle}, reverse {reverse}"
            );
            assert_eq!(app.host.document.editor.project, before);
            assert!(app.host.document.editor.undo.is_empty());
        }
    }
}

#[test]
fn selection_split_owner_commits_only_in_original_pane_and_escape_cancels() {
    let (ctx, mut app) = setup();
    let id = shortcut_test_wall(&mut app);
    app.host.document.editor.set_selection(vec![]);
    let frame = |app: &mut AtelierApp, events: Vec<Event>| {
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
                events,
                ..Default::default()
            },
            |ui| {
                for index in 0..2 {
                    let r = Rect::from_min_size(pos2(index as f32 * 400., 0.), vec2(400., 600.));
                    let response = ui.allocate_rect(r, Sense::click_and_drag());
                    let points =
                        geometry::rectangle_polygon(Point::new(450., 100.), Point::new(550., 200.));
                    app.selection_paths_input(
                        ui,
                        r,
                        &response,
                        Id::new(("split_test", index)),
                        &[SelectionPath { id, points }],
                        true,
                    );
                }
            },
        );
        output.textures_delta.clear();
    };
    let a = pos2(420., 70.);
    let b = pos2(580., 240.);
    frame(&mut app, vec![Event::PointerMoved(a)]);
    frame(
        &mut app,
        vec![Event::PointerButton {
            pos: a,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    frame(&mut app, vec![Event::PointerMoved(b)]);
    assert_eq!(
        app.canvas_selection_box.unwrap().owner,
        Id::new(("split_test", 1))
    );
    frame(
        &mut app,
        vec![Event::PointerButton {
            pos: b,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert_eq!(app.host.document.editor.selection_ids(), vec![id]);
    frame(&mut app, vec![Event::PointerMoved(a)]);
    frame(
        &mut app,
        vec![Event::PointerButton {
            pos: a,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    frame(&mut app, vec![Event::PointerMoved(b)]);
    frame(&mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.canvas_selection_box.is_none());
    frame(
        &mut app,
        vec![Event::PointerButton {
            pos: b,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert_eq!(app.host.document.editor.selection_ids(), vec![id]);
}

#[test]
fn selection_figures_map_real_fragments_preserve_holes_and_stale_figures_do_not_select() {
    use crate::diagrams::{self, Definition, Domain, Edit, Presentation, Status};
    let (ctx, mut app) = setup();
    let id = app.host.document.editor.project.add(
        0,
        Material::Concrete,
        Shape::ConceptMass {
            spec: crate::concept_mass::ConceptMassSpec {
                arcs: vec![],
                hole_arcs: vec![],
                outline: geometry::rectangle_polygon(Point::new(0., 0.), Point::new(10., 10.)),
                holes: vec![geometry::rectangle_polygon(
                    Point::new(3., 3.),
                    Point::new(7., 7.),
                )],
                height: 3.,
                bottom_offset: 0.,
            },
        },
    );
    let project = &app.host.document.editor.project;
    let drawings = diagrams::apply(
        project,
        &Edit::Create {
            name: "Selection fixture".into(),
            definition: Definition {
                story: 0,
                option: None,
                domain: Domain::ConceptMasses,
                presentation: Presentation::ModelQuantities,
            },
        },
    )
    .unwrap();
    app.host.document.editor.project.drawings = drawings;
    let figure = app.host.document.editor.project.drawings.diagrams[0].clone();
    assert_eq!(
        figure.status(&app.host.document.editor.project),
        Status::Current
    );
    let layout = diagrams::Layout::new(&figure);
    let paths = crate::screen_selection::diagram_selection_paths(&figure).unwrap();
    let pp = |x, y| {
        let p = layout.point(Point::new(x, y));
        Point::new(p[0], p[1])
    };
    let hole = geometry::rectangle_polygon(pp(4., 4.), pp(6., 6.));
    assert!(crate::screen_selection::select_paths(&paths, &hole, false).is_empty());
    let partial = geometry::rectangle_polygon(pp(-1., -1.), pp(2., 11.));
    assert_eq!(
        crate::screen_selection::select_paths(&paths, &partial, false),
        vec![id]
    );
    assert!(crate::screen_selection::select_paths(&paths, &partial, true).is_empty());
    let full = geometry::rectangle_polygon(pp(-1., -1.), pp(11., 11.));
    assert_eq!(
        crate::screen_selection::select_paths(&paths, &full, true),
        vec![id]
    );
    app.host.document.editor.set_selection(vec![]);
    let before = app.host.document.editor.project.clone();
    let click = pp(1., 1.);
    let click = pos2(click.x as f32, click.y as f32);
    let frame = |app: &mut AtelierApp, status, events| {
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200., 800.))),
                events,
                ..Default::default()
            },
            |ui| {
                app.figure_selection_input(
                    ui,
                    &figure,
                    status,
                    Rect::from_min_size(Pos2::ZERO, vec2(1200., 800.)),
                );
            },
        );
        output.textures_delta.clear();
    };
    for status in [Status::Stale, Status::Current] {
        frame(&mut app, status, vec![Event::PointerMoved(click)]);
        frame(
            &mut app,
            status,
            vec![Event::PointerButton {
                pos: click,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            }],
        );
        frame(
            &mut app,
            status,
            vec![Event::PointerButton {
                pos: click,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_eq!(
            app.host.document.editor.selection_ids(),
            if status == Status::Current {
                vec![id]
            } else {
                vec![]
            }
        );
        assert_eq!(app.host.document.editor.project, before);
        assert!(app.host.document.editor.undo.is_empty());
    }
}

#[test]
fn selection_figure_escape_cancels_box_before_returning_to_default_plan() {
    let (ctx, mut app) = setup();
    app.view = View::Figure(12);
    app.host.document.editor.set_tool(Tool::Wall);
    let frame = |app: &mut AtelierApp, events: Vec<Event>| {
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
                events,
                ..Default::default()
            },
            |ui| {
                app.figure_shortcuts(ui.ctx());
                let rect = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
                let response = ui.allocate_rect(rect, Sense::click_and_drag());
                app.selection_paths_input(
                    ui,
                    rect,
                    &response,
                    Id::new("figure_cancel_test"),
                    &[],
                    false,
                );
            },
        );
        output.textures_delta.clear();
    };
    let a = pos2(20., 30.);
    let b = pos2(300., 220.);
    frame(&mut app, vec![Event::PointerMoved(a)]);
    frame(
        &mut app,
        vec![Event::PointerButton {
            pos: a,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    frame(&mut app, vec![Event::PointerMoved(b)]);
    assert!(app.canvas_selection_box.is_some());
    frame(&mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert_eq!(app.view, View::Figure(12));
    assert!(app.canvas_selection_box.is_none());
    frame(
        &mut app,
        vec![Event::PointerButton {
            pos: b,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    frame(
        &mut app,
        vec![Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
    );
    frame(&mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert_eq!(app.view, View::Plan);
    assert_eq!(app.host.document.editor.tool, Tool::Select);
}
