use super::*;
use crate::app::viewport_navigation::{Mode, Pane};
use egui::accesskit::Action;

fn setup() -> (Context, AtelierApp) {
    let ctx = Context::default();
    ctx.enable_accesskit();
    AtelierApp::style(&ctx);
    let mut app = AtelierApp::from_renderer(None, "Navigation canvas test".into());
    app.recovery_slot = None;
    app.in_window_menus = true;
    app.language = UiLanguage::English;
    app.host.document.editor = Editor::new(Project::default());
    app.host.document.editor.project.add(
        0,
        Material::Concrete,
        Shape::Slab {
            min: Point::new(-2., -2.),
            max: Point::new(2., 2.),
            thickness: 0.2,
        },
    );
    app.ai_workspace.enabled = true;
    (ctx, app)
}

fn frame(ctx: &Context, app: &mut AtelierApp, events: Vec<Event>) -> egui::FullOutput {
    frame_with_modifiers(ctx, app, events, Modifiers::NONE)
}

fn frame_with_modifiers(
    ctx: &Context,
    app: &mut AtelierApp,
    mut events: Vec<Event>,
    modifiers: Modifiers,
) -> egui::FullOutput {
    events.insert(0, Event::ModifiersChanged(modifiers));
    let mut out = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1100., 760.))),
            focused: true,
            events,
            ..Default::default()
        },
        |ui| app.workspace(ui),
    );
    out.textures_delta.clear();
    out
}

fn key_event(key: Key, pressed: bool, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers,
    }
}

fn settle(ctx: &Context, app: &mut AtelierApp) {
    for _ in 0..5 {
        frame(ctx, app, vec![]);
    }
}

fn button(ctx: &Context, app: &mut AtelierApp, label: &str, right: bool) -> Pos2 {
    let out = frame(ctx, app, vec![]);
    let text = app.language.translate(label);
    let controls = app.navigation.control_rects;
    let mut bounds = out
        .platform_output
        .accesskit_update
        .unwrap()
        .nodes
        .into_iter()
        .filter(|(_, n)| n.label() == Some(text) && n.supports_action(Action::Click))
        .filter_map(|(_, n)| n.bounds())
        .filter(|r| {
            controls.iter().flatten().any(|control| {
                control.contains(pos2(
                    ((r.x0 + r.x1) / 2.) as f32,
                    ((r.y0 + r.y1) / 2.) as f32,
                ))
            })
        })
        .collect::<Vec<_>>();
    bounds.sort_by(|a, b| a.x0.total_cmp(&b.x0));
    let r = if right { bounds.last() } else { bounds.first() }.expect(label);
    pos2(((r.x0 + r.x1) / 2.) as f32, ((r.y0 + r.y1) / 2.) as f32)
}

fn click(ctx: &Context, app: &mut AtelierApp, pos: Pos2) {
    frame(ctx, app, vec![Event::PointerMoved(pos)]);
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            vec![Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }],
        );
    }
}

#[test]
fn ai_canvas_navigation_is_visible_in_each_pane_bilingual_and_clears_old_layout() {
    for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
        for view in [View::Plan, View::Model, View::Split] {
            let (ctx, mut app) = setup();
            app.language = language;
            app.view = view;
            settle(&ctx, &mut app);
            let canvas = app.workbench.panel_rects["view.canvas"];
            let out = frame(&ctx, &mut app, vec![]);
            let nodes = out.platform_output.accesskit_update.unwrap().nodes;
            let count = |label: &str| {
                nodes
                    .iter()
                    .filter(|(_, n)| {
                        n.label() == Some(language.translate(label))
                            && n.bounds().is_some_and(|r| {
                                app.navigation
                                    .control_rects
                                    .iter()
                                    .flatten()
                                    .any(|control| {
                                        control.contains(pos2(
                                            ((r.x0 + r.x1) / 2.) as f32,
                                            ((r.y0 + r.y1) / 2.) as f32,
                                        ))
                                    })
                            })
                    })
                    .count()
            };
            let pane_count = if view == View::Split { 2 } else { 1 };
            for label in [
                "Previous Zoom",
                "Next Zoom",
                "Zoom Rectangle",
                "Fit in Window",
            ] {
                assert_eq!(count(label), pane_count, "{view:?}: {label}");
            }
            for label in ["Orbit", "Explore Model"] {
                assert_eq!(
                    count(label),
                    usize::from(view != View::Plan),
                    "{view:?}: {label}"
                );
            }
            let active = app.navigation.pane;
            frame(&ctx, &mut app, vec![]);
            assert_eq!(
                app.navigation.pane, active,
                "painting a toolbar must not select it"
            );
            for (index, rect) in app.navigation.control_rects.iter().enumerate() {
                let expected = view == View::Split
                    || (index == 0 && view == View::Plan)
                    || (index == 1 && view == View::Model);
                assert_eq!(rect.is_some(), expected);
                if let Some(rect) = rect {
                    assert!(
                        canvas.contains_rect(*rect),
                        "toolbar stays inside canvas: {rect:?}"
                    );
                    assert!(rect.bottom() <= canvas.bottom() - 7.);
                    if view == View::Split {
                        let divider = canvas.left() + canvas.width() * app.split as f32;
                        if index == 0 {
                            assert!(rect.right() <= divider - 3.);
                        } else {
                            assert!(rect.left() >= divider + 3.);
                        }
                    }
                }
            }
            app.ai_workspace.enabled = false;
            frame(&ctx, &mut app, vec![]);
            assert_eq!(app.navigation.control_rects, [None, None]);
        }
    }
}

#[test]
fn ai_canvas_navigation_split_buttons_target_their_pane_without_model_clicks() {
    let (ctx, mut app) = setup();
    app.view = View::Split;
    app.host.document.editor.set_tool(Tool::Wall);
    app.plan_center = Point::new(1000., 1000.);
    app.camera.target = glam::DVec3::new(900., 800., 700.);
    app.navigation.pane = Pane::Model;
    settle(&ctx, &mut app);
    let before = app.host.document.editor.project.clone();
    let camera = app.camera.target;
    let at = button(&ctx, &mut app, "Fit in Window", false);
    click(&ctx, &mut app, at);
    assert_eq!(app.navigation.pane, Pane::Plan);
    assert!(app.plan_center.x.abs() < 10.);
    assert_eq!(app.camera.target, camera);
    assert_eq!(app.host.document.editor.anchor, None);
    let plan = app.plan_center;
    let at = button(&ctx, &mut app, "Fit in Window", true);
    click(&ctx, &mut app, at);
    assert_eq!(app.navigation.pane, Pane::Model);
    assert_ne!(app.camera.target, camera);
    assert_eq!(app.plan_center, plan);
    let at = button(&ctx, &mut app, "Explore Model", true);
    click(&ctx, &mut app, at);
    assert_eq!(
        app.navigation.mode,
        Mode::Explore,
        "the activation click must not immediately exit"
    );
    let start = app.camera.target;
    frame(
        &ctx,
        &mut app,
        vec![key_event(Key::W, true, Modifiers::NONE)],
    );
    let moved = app.camera.target;
    assert_ne!(
        moved, start,
        "W works while pointer stays on the activation button"
    );
    frame(&ctx, &mut app, vec![]);
    assert_ne!(
        app.camera.target, moved,
        "held W continues moving without key-repeat events"
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(Key::W, false, Modifiers::NONE)],
    );
    let stopped = app.camera.target;
    frame(&ctx, &mut app, vec![]);
    assert_eq!(app.camera.target, stopped, "releasing W stops movement");
    let canvas = app.workbench.panel_rects["view.canvas"];
    // This test is split: the full panel center is its divider, not the model pane.
    let at = pos2(
        canvas.left() + canvas.width() * 0.75,
        canvas.top() + canvas.height() * 0.25,
    );
    frame(&ctx, &mut app, vec![Event::PointerMoved(at)]);
    let direction = (app.camera.yaw, app.camera.pitch);
    let target = app.camera.target;
    frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: at,
            button: PointerButton::Middle,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    let end = at + vec2(30., 20.);
    frame(&ctx, &mut app, vec![Event::PointerMoved(end)]);
    assert_ne!(
        app.camera.target, target,
        "middle pan reaches the AI model canvas"
    );
    frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos: end,
            button: PointerButton::Middle,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert_eq!((app.camera.yaw, app.camera.pitch), direction);
    assert_eq!(app.navigation.mode, Mode::Explore);
    let fly = app.navigation.fly;
    frame(
        &ctx,
        &mut app,
        vec![key_event(Key::F, true, Modifiers::NONE)],
    );
    assert_eq!(app.navigation.fly, !fly);
    frame(
        &ctx,
        &mut app,
        vec![key_event(Key::F, false, Modifiers::NONE)],
    );
    let fly = app.navigation.fly;
    let at = button(&ctx, &mut app, "Fly", true);
    click(&ctx, &mut app, at);
    assert_eq!(app.navigation.fly, !fly);
    let start = app.camera.target;
    frame(
        &ctx,
        &mut app,
        vec![key_event(Key::W, true, Modifiers::NONE)],
    );
    let walking = app.camera.target.distance(start);
    assert!(
        walking > 0.,
        "clicking Fly must return keyboard ownership to exploration"
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(Key::W, false, Modifiers::NONE)],
    );
    let start = app.camera.target;
    frame_with_modifiers(
        &ctx,
        &mut app,
        vec![key_event(Key::W, true, Modifiers::SHIFT)],
        Modifiers::SHIFT,
    );
    let boosted = app.camera.target.distance(start);
    frame_with_modifiers(
        &ctx,
        &mut app,
        vec![key_event(Key::W, false, Modifiers::SHIFT)],
        Modifiers::SHIFT,
    );
    assert!(walking > 0.);
    assert!(boosted > walking * 3.5 && boosted < walking * 4.5);
    assert_eq!(app.host.document.editor.tool, Tool::Wall);
    settle(&ctx, &mut app);
    let toolbar = app.navigation.control_rects[1].unwrap();
    let canvas = app.workbench.panel_rects["view.canvas"];
    assert!(
        canvas.contains_rect(toolbar),
        "expanded navigation controls stay on canvas"
    );
    let at = button(&ctx, &mut app, "Fit in Window", true);
    click(&ctx, &mut app, at);
    assert_eq!(app.navigation.mode, Mode::Edit);
    assert_eq!(app.host.document.editor.project, before);
    assert!(app.host.document.editor.undo.is_empty());
    assert_eq!(app.host.document.editor.anchor, None);
}

#[test]
fn ai_canvas_navigation_header_fit_uses_current_pane_and_ends_explore() {
    let (ctx, mut app) = setup();
    app.view = View::Model;
    app.plan_center = Point::new(1000., 2000.);
    app.camera.span = 1e6;
    settle(&ctx, &mut app);
    let plan = app.plan_center;
    let before = app.host.document.editor.project.clone();
    app.navigation_begin(Mode::Explore);
    let out = frame(&ctx, &mut app, vec![]);
    let bounds = out
        .platform_output
        .accesskit_update
        .unwrap()
        .nodes
        .into_iter()
        .find_map(|(_, n)| {
            (n.label() == Some("Fit") && n.supports_action(Action::Click))
                .then(|| n.bounds())
                .flatten()
        })
        .unwrap();
    let at = pos2(
        ((bounds.x0 + bounds.x1) / 2.) as f32,
        ((bounds.y0 + bounds.y1) / 2.) as f32,
    );
    click(&ctx, &mut app, at);
    assert_eq!(
        app.navigation.mode,
        Mode::Edit,
        "header and canvas fit share exit behavior"
    );
    assert!(app.camera.span < 20., "header frames visible model too");
    assert_eq!(
        app.plan_center, plan,
        "3D header fit does not alter the hidden floor plan"
    );
    assert_eq!(app.host.document.editor.project, before);
    assert!(app.host.document.editor.undo.is_empty());
}
