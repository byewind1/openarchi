//! Transient view operations; never owns or changes project geometry.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Pane {
    #[default]
    Plan,
    Model,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Mode {
    #[default]
    Edit,
    Zoom,
    Orbit,
    Explore,
}
#[derive(Clone, Debug, PartialEq)]
enum Position {
    Plan(Point, f64),
    Model(f64, f64, f64, glam::DVec3),
}
#[derive(Default)]
struct History {
    entries: Vec<Position>,
    at: usize,
    pending: Option<(Position, f64)>,
}
impl History {
    fn push(&mut self, value: Position) {
        self.pending = None;
        if self.entries.get(self.at) == Some(&value) {
            return;
        }
        self.entries.truncate(self.at + 1);
        self.entries.push(value);
        if self.entries.len() > 20 {
            self.entries.remove(0);
        }
        self.at = self.entries.len() - 1;
    }
    fn observe(&mut self, value: Position, now: f64, busy: bool) -> bool {
        if self.entries.is_empty() {
            self.push(value);
            return false;
        }
        if self.entries.get(self.at) == Some(&value) {
            self.pending = None;
            return false;
        }
        if self.pending.as_ref().is_none_or(|(p, _)| *p != value) {
            self.pending = Some((value.clone(), now));
        }
        if !busy && self.pending.as_ref().is_some_and(|(_, t)| now - t >= 0.25) {
            self.push(value);
            return false;
        }
        true
    }
}
pub(super) struct Navigation {
    pub pane: Pane,
    pub mode: Mode,
    owner: Option<Pane>,
    context: String,
    histories: BTreeMap<String, History>,
    plan_rect: Option<Rect>,
    model_rect: Option<Rect>,
    zoom_start: Option<Pos2>,
    pub fly: bool,
    pub speed: f64,
    look_anchor: Option<Pos2>,
    explore_pan_anchor: Option<Pos2>,
    release_entry_focus: bool,
    pub(super) control_rects: [Option<Rect>; 2],
    // Consumes the exit frame so an exit click/Esc cannot reach editing.
    pub exit_frame: bool,
}
impl Default for Navigation {
    fn default() -> Self {
        Self {
            pane: Pane::Plan,
            mode: Mode::Edit,
            owner: None,
            context: String::new(),
            histories: BTreeMap::new(),
            plan_rect: None,
            model_rect: None,
            zoom_start: None,
            fly: true,
            speed: 1.,
            look_anchor: None,
            explore_pan_anchor: None,
            release_entry_focus: false,
            control_rects: [None; 2],
            exit_frame: false,
        }
    }
}
impl Navigation {
    /// Viewport measurements belong to the window, while gestures and history
    /// belong to the document. Keep the former for the new document's first fit.
    pub(super) fn reset_for_document(&mut self) {
        *self = Self {
            plan_rect: self.plan_rect,
            model_rect: self.model_rect,
            ..Self::default()
        };
    }
}
impl AtelierApp {
    pub(super) fn navigation_canvas_rect(&mut self, pane: Pane, r: Rect) {
        match pane {
            Pane::Plan => self.navigation.plan_rect = Some(r),
            Pane::Model => self.navigation.model_rect = Some(r),
        }
    }
    pub(super) fn navigation_model_aspect(&self) -> f64 {
        self.navigation
            .model_rect
            .map_or(4. / 3., |r| f64::from(r.aspect_ratio()))
    }
    pub(super) fn navigation_pane(&self) -> Pane {
        match self.view {
            View::Model => Pane::Model,
            View::Split => self.navigation.pane,
            _ => Pane::Plan,
        }
    }
    fn navigation_key(&self, pane: Pane) -> String {
        format!(
            "{}|{}|{:?}|{}|{:?}",
            self.path,
            self.host.document.editor.project.name,
            pane,
            if pane == Pane::Plan {
                self.host.document.editor.story
            } else {
                0
            },
            if pane == Pane::Plan {
                self.host
                    .document
                    .editor
                    .project
                    .plan_options
                    .active_worksheet
            } else {
                None
            }
        )
    }
    fn navigation_position(&self, pane: Pane) -> Position {
        match pane {
            Pane::Plan => Position::Plan(self.plan_center, self.plan_scale),
            Pane::Model => Position::Model(
                self.camera.yaw,
                self.camera.pitch,
                self.camera.span,
                self.camera.target,
            ),
        }
    }
    fn navigation_restore(&mut self, value: Position) {
        match value {
            Position::Plan(c, s) => {
                self.plan_center = c;
                self.plan_scale = s;
            }
            Position::Model(y, p, s, t) => {
                self.camera.yaw = y;
                self.camera.pitch = p;
                self.camera.span = s;
                self.camera.target = t;
            }
        }
    }
    fn navigation_checkpoint(&mut self, pane: Pane) {
        let position = self.navigation_position(pane);
        let key = self.navigation_key(pane);
        self.navigation
            .histories
            .entry(key)
            .or_default()
            .push(position);
    }
    pub(super) fn navigation_history(&mut self, forward: bool) {
        let pane = self.navigation_pane();
        self.navigation_checkpoint(pane);
        let h = self
            .navigation
            .histories
            .get_mut(&self.navigation_key(pane))
            .unwrap();
        let next = if forward {
            (h.at + 1).min(h.entries.len() - 1)
        } else {
            h.at.saturating_sub(1)
        };
        h.at = next;
        let value = h.entries[next].clone();
        self.navigation_restore(value);
        self.navigation.mode = Mode::Edit;
        self.navigation.zoom_start = None;
        self.host.document.editor.status = self.language.translate("Navigation ended").into();
    }
    pub(super) fn navigation_begin(&mut self, mode: Mode) {
        if self.menu_gated()
            || self.pointer_drag.is_some()
            || self.wall_insert.is_some()
            || self.mass_parameter_draft_active()
            || self.reshape_session.is_some()
            || self.drawings_ui.mode.is_some()
            || self.research_ui.canvas_move.is_some()
        {
            self.host.document.editor.status = self
                .language
                .translate("Finish the current edit before navigating")
                .into();
            return;
        }
        if matches!(mode, Mode::Orbit | Mode::Explore) && self.navigation_pane() != Pane::Model {
            self.dispatch_workbench(crate::workbench::WorkbenchCommand::Model);
        }
        let pane = self.navigation_pane();
        self.navigation_checkpoint(pane);
        self.navigation.mode = if self.navigation.mode == mode {
            Mode::Edit
        } else {
            mode
        };
        self.navigation.release_entry_focus = self.navigation.mode != Mode::Edit;
        self.navigation.owner = Some(pane);
        self.navigation.context = self.navigation_key(pane);
        self.navigation.zoom_start = None;
        self.navigation.look_anchor = None;
        self.navigation.explore_pan_anchor = None;
        if self.navigation.speed <= 0. {
            self.navigation.speed = 1.;
        }
        self.host.document.editor.status = self
            .language
            .translate(match self.navigation.mode {
                Mode::Zoom => "Zoom · drag a rectangle · Esc cancels",
                Mode::Orbit => "Orbit · left-drag · Esc exits",
                Mode::Explore => "Explore · move mouse to look · WASD moves · click or Esc exits",
                Mode::Edit => "Navigation ended",
            })
            .into();
    }
    pub(super) fn navigation_zoom(&mut self, factor: f64) {
        if !factor.is_finite() || factor <= 0. {
            return;
        }
        let pane = self.navigation_pane();
        self.navigation_checkpoint(pane);
        if pane == Pane::Plan {
            self.plan_scale = (self.plan_scale * factor).clamp(1e-6, 100_000.);
        } else {
            self.camera.span = (self.camera.span / factor).clamp(0.001, 1e12);
        }
        self.navigation_checkpoint(pane);
        self.host.document.editor.status = self.language.translate("Zoom").into();
    }
    pub(super) fn navigation_fit(&mut self) {
        let pane = self.navigation_pane();
        self.navigation_checkpoint(pane);
        if pane == Pane::Model {
            self.fit_model_contents();
        } else {
            let size = self
                .navigation
                .plan_rect
                .map_or(vec2(800., 600.), |r| r.size());
            if let Some(id) = self
                .host
                .document
                .editor
                .project
                .plan_options
                .active_worksheet
            {
                self.fit_worksheet_in_viewport(id, size);
            } else {
                let project = &self.host.document.editor.project;
                let mut points: Vec<_> = project
                    .elements
                    .iter()
                    .filter(|e| {
                        e.worksheet.is_none()
                            && geometry::element_on_plan_story(
                                project,
                                e,
                                self.host.document.editor.story,
                            )
                            && project.plan_options.element_visible(e, &project.layers)
                    })
                    .flat_map(|e| {
                        geometry::element_plan_outline_on_story(
                            project,
                            e,
                            self.host.document.editor.story,
                        )
                    })
                    .collect();
                if let Some(terrain) = &project.site.terrain {
                    points.extend(
                        terrain
                            .geometry
                            .vertices
                            .iter()
                            .map(|v| Point::new(v[0], v[1])),
                    );
                }
                for page in &project.drawings.pages {
                    if page
                        .placement
                        .as_ref()
                        .is_some_and(|p| p.visible && p.worksheet.is_none())
                    {
                        points.extend(
                            [
                                [0., 0.],
                                [page.pixels[0] as f64, 0.],
                                [0., page.pixels[1] as f64],
                                [page.pixels[0] as f64, page.pixels[1] as f64],
                            ]
                            .map(|p| self.source_world(page.id, p)),
                        );
                    }
                }
                for u in &project.drawings.cad_underlays {
                    if u.visible_in_plan(None) {
                        if let Some(b) = project
                            .drawings
                            .cad_sources
                            .iter()
                            .find(|s| s.id == u.source)
                            .and_then(|s| s.report.bounds)
                        {
                            points.extend(
                                [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]]
                                    .map(|p| u.transform.world(p)),
                            );
                        }
                    }
                }
                self.fit_plan_points_in_viewport(points, size);
            }
        }
        self.navigation.mode = Mode::Edit;
        self.navigation.zoom_start = None;
        self.navigation.look_anchor = None;
        self.navigation.explore_pan_anchor = None;
        self.navigation_checkpoint(pane);
        self.host.document.editor.status = self.language.translate("Fit in Window").into();
    }
    pub(super) fn navigation_shortcuts(&mut self, ctx: &Context) -> bool {
        self.navigation.exit_frame = false;
        if self.navigation.mode == Mode::Edit {
            return false;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            ctx.input_mut(|i| {
                i.consume_key(Modifiers::NONE, Key::Escape);
            });
            self.navigation.mode = Mode::Edit;
            self.navigation.zoom_start = None;
            self.navigation.look_anchor = None;
            self.navigation.explore_pan_anchor = None;
            self.navigation.exit_frame = true;
            self.host.document.editor.status = self.language.translate("Navigation ended").into();
        }
        // Keys are owned by navigation; model tools cannot see WASD or Escape.
        true
    }
    pub(super) fn navigation_canvas(
        &mut self,
        ui: &mut Ui,
        r: Rect,
        response: &Response,
        pane: Pane,
    ) -> bool {
        self.navigation_canvas_rect(pane, r);
        if self.navigation.mode == Mode::Edit
            && response.hovered()
            && ui.input(|i| i.pointer.any_pressed() || i.smooth_scroll_delta != Vec2::ZERO)
        {
            self.navigation.pane = pane;
        }
        let key = self.navigation_key(pane);
        if self.navigation.mode != Mode::Edit
            && (self.navigation.owner != Some(self.navigation_pane())
                || self.navigation.context != self.navigation_key(self.navigation_pane()))
        {
            self.navigation.mode = Mode::Edit;
            self.navigation.zoom_start = None;
            self.navigation.look_anchor = None;
            self.navigation.explore_pan_anchor = None;
        }
        if self.navigation.histories.len() > 32 && !self.navigation.histories.contains_key(&key) {
            self.navigation.histories.clear();
        }
        let position = self.navigation_position(pane);
        let now = ui.input(|i| i.time);
        let busy = ui.input(|i| i.pointer.any_down());
        if self
            .navigation
            .histories
            .entry(key)
            .or_default()
            .observe(position, now, busy)
        {
            ui.ctx().request_repaint_after(Duration::from_millis(260));
        }
        if self.navigation.exit_frame {
            return true;
        }
        let over_controls = self.navigation.control_rects[if pane == Pane::Plan { 0 } else { 1 }]
            .is_some_and(|rect| {
                ui.input(|i| {
                    i.pointer.hover_pos().is_some_and(|p| rect.contains(p))
                        || (i.pointer.any_down()
                            && i.pointer.press_origin().is_some_and(|p| rect.contains(p)))
                })
            });
        if self.navigation.mode == Mode::Edit {
            return over_controls;
        }
        if self.navigation.owner != Some(pane) {
            return true;
        }
        if self.navigation.release_entry_focus {
            // Starting navigation transfers keyboard ownership from its button/menu.
            // egui_wants_keyboard_input also reports buttons, not only text edits.
            ui.ctx().memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
            self.navigation.release_entry_focus = false;
        }
        if self.menu_gated()
            || egui::Popup::is_any_open(ui.ctx())
            || !ui.input(|i| i.focused)
            || ui.ctx().egui_wants_keyboard_input()
        {
            self.navigation.look_anchor = None;
            self.navigation.explore_pan_anchor = None;
            return true;
        }
        if over_controls && self.navigation.mode != Mode::Explore {
            self.navigation.look_anchor = None;
            return true;
        }
        match self.navigation.mode {
            Mode::Zoom => {
                if response.drag_started_by(PointerButton::Primary) {
                    self.navigation.zoom_start = ui.input(|i| i.pointer.press_origin());
                }
                if response.drag_stopped_by(PointerButton::Primary) {
                    if let Some(a) = self.navigation.zoom_start.take()
                        && let Some(b) = ui.input(|i| i.pointer.latest_pos())
                    {
                        self.navigation_zoom_rect(pane, r, Rect::from_two_pos(a, b).intersect(r));
                    }
                    self.navigation.mode = Mode::Edit;
                    self.host.document.editor.status = self.language.translate("Zoom").into();
                }
            }
            Mode::Orbit => {
                if response.hovered() {
                    let scroll = self.canvas_scroll_delta(ui);
                    self.camera.zoom_scroll(scroll.into());
                }
                if response.dragged_by(PointerButton::Secondary)
                    || response.dragged_by(PointerButton::Middle)
                {
                    let d = ui.input(|i| i.pointer.delta());
                    self.camera.pan_pixels(
                        d.x.into(),
                        d.y.into(),
                        r.width().into(),
                        r.height().into(),
                    );
                }
                if response.dragged_by(PointerButton::Primary) {
                    let d = ui.input(|i| i.pointer.delta());
                    self.camera.orbit_pixels(d.x.into(), d.y.into());
                }
            }
            Mode::Explore => {
                if response.clicked() && !over_controls {
                    self.navigation.mode = Mode::Edit;
                    self.navigation.look_anchor = None;
                    self.navigation.explore_pan_anchor = None;
                    self.host.document.editor.status =
                        self.language.translate("Navigation ended").into();
                    return true;
                }
                let (middle_down, middle_pressed, middle_released, pointer) = ui.input(|i| {
                    (
                        i.pointer.button_down(PointerButton::Middle),
                        i.pointer.button_pressed(PointerButton::Middle),
                        i.pointer.button_released(PointerButton::Middle),
                        i.pointer.hover_pos(),
                    )
                });
                if middle_pressed && response.hovered() && !over_controls {
                    self.navigation.explore_pan_anchor = pointer;
                }
                // A middle drag temporarily owns the mouse, not the navigation mode.
                // Keep capture across the canvas controls; never start it on them.
                if middle_down || middle_released {
                    if let (Some(a), Some(b)) = (self.navigation.explore_pan_anchor, pointer) {
                        let delta = b - a;
                        if delta.length() <= r.size().length() * 0.5 {
                            self.camera.pan_pixels(
                                f64::from(delta.x),
                                f64::from(delta.y),
                                f64::from(r.width()),
                                f64::from(r.height()),
                            );
                        }
                        self.navigation.explore_pan_anchor = Some(b);
                    } else {
                        self.navigation.explore_pan_anchor = None;
                    }
                    self.navigation.look_anchor = None;
                    if middle_released {
                        self.navigation.explore_pan_anchor = None;
                        self.navigation.look_anchor =
                            pointer.filter(|p| r.contains(*p) && !over_controls);
                    }
                    // Looking needs the pointer over the canvas; keyboard movement
                    // belongs to the active pane even while over its controls.
                } else if response.hovered() && !over_controls {
                    self.navigation.explore_pan_anchor = None;
                    let pointer = ui.input(|i| i.pointer.hover_pos());
                    if let (Some(a), Some(b)) = (self.navigation.look_anchor, pointer) {
                        let delta = b - a;
                        // Cross-window/warp discontinuities are not deliberate looking.
                        if delta.length() <= r.size().length() * 0.5 {
                            let scale = 600. / f64::from(r.height().max(1.));
                            self.camera.look_pixels(
                                f64::from(delta.x) * scale,
                                f64::from(delta.y) * scale,
                                r.aspect_ratio().into(),
                            );
                        }
                    }
                    self.navigation.look_anchor = pointer;
                } else {
                    self.navigation.look_anchor = None;
                    self.navigation.explore_pan_anchor = None;
                }
                if ui.input(|i| i.modifiers.command || i.modifiers.ctrl || i.modifiers.alt) {
                    return true;
                }
                if ui.input(|i| i.key_pressed(Key::F)) {
                    self.navigation.fly = !self.navigation.fly;
                }
                let speed_step = ui.input(|i| {
                    i32::from(i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals))
                        - i32::from(i.key_pressed(Key::Minus))
                });
                self.navigation.speed =
                    (self.navigation.speed * 1.25_f64.powi(speed_step)).clamp(0.1, 20.);
                let (x, y, z, boost, dt) = ui.input(|i| {
                    let held = |a, b| {
                        if i.key_down(a) || i.key_down(b) {
                            1.
                        } else {
                            0.
                        }
                    };
                    (
                        held(Key::D, Key::ArrowRight) - held(Key::A, Key::ArrowLeft),
                        held(Key::W, Key::ArrowUp) - held(Key::S, Key::ArrowDown),
                        held(Key::Space, Key::PageUp) - held(Key::C, Key::PageDown),
                        i.modifiers.shift,
                        i.stable_dt.min(0.05),
                    )
                });
                if x != 0. || y != 0. || z != 0. {
                    self.camera.explore_move(
                        x,
                        y,
                        z,
                        f64::from(dt) * self.navigation.speed * if boost { 4. } else { 1. },
                        self.navigation.fly,
                    );
                    ui.ctx().request_repaint();
                }
            }
            Mode::Edit => {}
        }
        true
    }
    fn navigation_zoom_rect(&mut self, pane: Pane, r: Rect, selection: Rect) {
        if selection.width() < 5. || selection.height() < 5. {
            return;
        }
        let factor =
            f64::from((r.width() / selection.width()).min(r.height() / selection.height()));
        if pane == Pane::Plan {
            self.plan_center = self.world(r, selection.center());
            self.plan_scale = (self.plan_scale * factor).clamp(1e-6, 100_000.);
        } else {
            let d = r.center() - selection.center();
            self.camera
                .pan_pixels(d.x.into(), d.y.into(), r.width().into(), r.height().into());
            self.camera.span = (self.camera.span / factor).clamp(0.001, 1e12);
        }
        self.navigation_checkpoint(pane);
    }
    pub(super) fn navigation_overlay(&self, ui: &Ui, r: Rect, pane: Pane) {
        if self.navigation.owner != Some(pane) || self.navigation.mode == Mode::Edit {
            return;
        }
        if ui.input(|i| i.pointer.hover_pos().is_some_and(|p| r.contains(p))) {
            ui.ctx().set_cursor_icon(match self.navigation.mode {
                Mode::Zoom => CursorIcon::ZoomIn,
                Mode::Orbit => CursorIcon::Grab,
                Mode::Explore if self.navigation.explore_pan_anchor.is_some() => {
                    CursorIcon::Grabbing
                }
                Mode::Explore => CursorIcon::Crosshair,
                Mode::Edit => CursorIcon::Default,
            });
        }
        if let Some(a) = self.navigation.zoom_start
            && let Some(b) = ui.input(|i| i.pointer.latest_pos())
        {
            ui.painter_at(r).rect_stroke(
                Rect::from_two_pos(a, b).intersect(r),
                0.,
                Stroke::new(1.5, ACCENT),
                StrokeKind::Inside,
            );
        }
    }
    pub(super) fn navigation_bar(&mut self, ui: &mut Ui) {
        let pane = self.navigation_pane();
        self.navigation_buttons(ui, pane);
        self.navigation_explore_settings(ui, pane);
        ui.separator();
    }
    pub(super) fn navigation_bar_for_pane(&mut self, ui: &mut Ui, pane: Pane) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| self.navigation_buttons(ui, pane));
            self.navigation_explore_settings(ui, pane);
        });
    }
    fn navigation_buttons(&mut self, ui: &mut Ui, pane: Pane) {
        let history = self.navigation.histories.get(&self.navigation_key(pane));
        let back =
            history.is_some_and(|h| h.at > 0 || self.navigation_position(pane) != h.entries[h.at]);
        let forward = history.is_some_and(|h| {
            h.at + 1 < h.entries.len() && self.navigation_position(pane) == h.entries[h.at]
        });
        for (icon, label, enabled) in [
            ("Undo", "Previous Zoom", back),
            ("Redo", "Next Zoom", forward),
            ("Zoom In", "Zoom Rectangle", true),
        ] {
            let tip = self.language.translate(label);
            let (rect, response) = ui.allocate_exact_size(vec2(29., 25.), Sense::click());
            response.widget_info(|| {
                if label == "Zoom Rectangle" {
                    WidgetInfo::selected(
                        WidgetType::Button,
                        enabled,
                        self.navigation.mode == Mode::Zoom && self.navigation.owner == Some(pane),
                        tip,
                    )
                } else {
                    WidgetInfo::labeled(WidgetType::Button, enabled, tip)
                }
            });
            if response.hovered()
                || (label == "Zoom Rectangle"
                    && self.navigation.mode == Mode::Zoom
                    && self.navigation.owner == Some(pane))
            {
                ui.painter().rect_filled(rect, 2., PANEL_ALT);
            }
            Self::paint_tool_icon(
                ui.painter(),
                rect.shrink(5.),
                icon,
                if enabled {
                    INK
                } else {
                    MUTED.gamma_multiply(0.4)
                },
            );
            let response = response.on_hover_text(tip);
            if enabled && response.clicked() {
                self.navigation.pane = pane;
                match label {
                    "Previous Zoom" => self.navigation_history(false),
                    "Next Zoom" => self.navigation_history(true),
                    _ => self.navigation_begin(Mode::Zoom),
                }
            }
            if label == "Zoom Rectangle" && response.double_clicked() {
                self.navigation.pane = pane;
                self.navigation.mode = Mode::Edit;
                self.navigation_zoom(2.);
            }
        }
        if pane == Pane::Model {
            for (icon, label, mode) in [
                ("Orbit", "Orbit", Mode::Orbit),
                ("Explore Model", "Explore Model", Mode::Explore),
            ] {
                if Self::vector_icon_button(
                    ui,
                    icon,
                    self.language.translate(label),
                    self.navigation.mode == mode && self.navigation.owner == Some(pane),
                ) {
                    self.navigation.pane = pane;
                    self.navigation_begin(mode);
                }
            }
        }
        if Self::vector_icon_button(ui, "Fit", self.language.translate("Fit in Window"), false) {
            self.navigation.pane = pane;
            self.navigation_fit();
        }
    }
    fn navigation_explore_settings(&mut self, ui: &mut Ui, pane: Pane) {
        if self.navigation.mode == Mode::Explore && self.navigation.owner == Some(pane) {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 8.;
                ui.spacing_mut().slider_width = 100.;
                let fly = ui.checkbox(&mut self.navigation.fly, self.language.translate("Fly"));
                if fly.clicked() {
                    fly.surrender_focus();
                }
                let speed = ui.add(
                    egui::Slider::new(&mut self.navigation.speed, 0.1..=20.)
                        .logarithmic(true)
                        .text(self.language.translate("Speed")),
                );
                if speed.clicked() || speed.drag_stopped() {
                    speed.surrender_focus();
                }
                ui.label(self.language.translate("Explore keyboard controls"));
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> (Context, AtelierApp) {
        let ctx = Context::default();
        let mut app = AtelierApp::from_renderer(None, "Navigation test".into());
        app.recovery_slot = None;
        app.host.document.editor = Editor::new(Project::default());
        (ctx, app)
    }
    fn frame(
        ctx: &Context,
        app: &mut AtelierApp,
        events: Vec<Event>,
        focused: bool,
    ) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
                events,
                focused,
                ..Default::default()
            },
            |ui| {
                app.shortcuts(ctx);
                egui::CentralPanel::default().show(ui, |ui| match app.view {
                    View::Model => app.model_view(ui),
                    _ => app.plan(ui),
                });
            },
        );
        output.textures_delta.clear();
        output
    }
    fn pointer(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }
    fn key(key: Key, pressed: bool) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }
    #[test]
    fn viewport_navigation_history_branches_and_isolates_plan_model_and_worksheet() {
        let (_, mut a) = app();
        let before = serde_json::to_value(&a.host.document.editor.project).unwrap();
        let base = a.plan_scale;
        a.navigation_zoom(2.);
        a.navigation_zoom(2.);
        a.navigation_history(false);
        assert_eq!(a.plan_scale, base * 2.);
        a.navigation_history(false);
        assert_eq!(a.plan_scale, base);
        a.navigation_history(true);
        assert_eq!(a.plan_scale, base * 2.);
        a.navigation_zoom(3.);
        a.navigation_history(true);
        assert_eq!(a.plan_scale, base * 6.);
        a.view = View::Model;
        let span = a.camera.span;
        a.navigation_zoom(2.);
        a.navigation_history(false);
        assert_eq!(a.camera.span, span);
        a.view = View::Plan;
        a.host.document.editor.project.plan_options.active_worksheet = Some(900);
        let scale = a.plan_scale;
        a.navigation_zoom(2.);
        a.navigation_history(false);
        assert_eq!(a.plan_scale, scale);
        a.host.document.editor.project.plan_options.active_worksheet = None;
        assert_eq!(
            serde_json::to_value(&a.host.document.editor.project).unwrap(),
            before
        );
        assert!(a.host.document.editor.undo.is_empty());
    }
    #[test]
    fn viewport_navigation_history_coalesces_drag_and_caps_twenty_positions() {
        let mut h = History::default();
        for i in 0..30 {
            h.push(Position::Plan(Point::default(), f64::from(i)));
        }
        assert_eq!(h.entries.len(), 20);
        assert_eq!(h.at, 19);
        let value = Position::Plan(Point::new(1., 2.), 100.);
        assert!(h.observe(value.clone(), 1., true));
        assert!(h.observe(value.clone(), 2., true));
        assert!(!h.observe(value.clone(), 2.1, false));
        assert_eq!(h.entries[h.at], value);
    }
    #[test]
    fn viewport_navigation_box_zoom_real_plan_events_keep_survey_coordinate_and_model() {
        let (ctx, mut a) = app();
        a.view = View::Plan;
        a.plan_center = Point::new(46_000_000.125, 3_000_000.875);
        a.plan_scale = 2.;
        a.host
            .document
            .editor
            .project
            .plan_options
            .orientation_degrees = 33.;
        let before = serde_json::to_value(&a.host.document.editor.project).unwrap();
        a.host.document.editor.tool = Tool::Wall;
        frame(&ctx, &mut a, vec![], true);
        let r = a.navigation.plan_rect.unwrap();
        let selected = Rect::from_center_size(r.center() + vec2(70., -20.), vec2(200., 100.));
        let expected = a.world(r, selected.center());
        let old_scale = a.plan_scale;
        a.navigation_begin(Mode::Zoom);
        frame(
            &ctx,
            &mut a,
            vec![
                Event::PointerMoved(selected.min),
                pointer(selected.min, true),
            ],
            true,
        );
        frame(&ctx, &mut a, vec![Event::PointerMoved(selected.max)], true);
        frame(&ctx, &mut a, vec![pointer(selected.max, false)], true);
        assert_eq!(a.navigation.mode, Mode::Edit);
        assert!(a.plan_scale > old_scale * 2.);
        assert!((a.plan_center.x - expected.x).abs() < 1e-6);
        assert!((a.plan_center.y - expected.y).abs() < 1e-6);
        assert!(a.host.document.editor.wall_points.is_empty());
        assert_eq!(
            serde_json::to_value(&a.host.document.editor.project).unwrap(),
            before
        );
        a.navigation_history(false);
        assert_eq!(a.plan_scale, old_scale);
    }
    #[test]
    fn viewport_navigation_cancel_preserves_wall_draft_and_small_box_does_nothing() {
        let (ctx, mut a) = app();
        a.host.document.editor.tool = Tool::Wall;
        a.host.document.editor.wall_points.push(Point::new(1., 2.));
        let draft = a.host.document.editor.wall_points.clone();
        let scale = a.plan_scale;
        a.navigation_begin(Mode::Zoom);
        frame(&ctx, &mut a, vec![key(Key::Escape, true)], true);
        assert_eq!(a.navigation.mode, Mode::Edit);
        assert_eq!(
            a.host.document.editor.status,
            a.language.translate("Navigation ended")
        );
        assert_eq!(a.host.document.editor.wall_points, draft);
        a.navigation_zoom_rect(
            Pane::Plan,
            Rect::from_min_size(Pos2::ZERO, vec2(800., 600.)),
            Rect::from_min_size(pos2(10., 10.), vec2(2., 2.)),
        );
        assert_eq!(a.plan_scale, scale);
    }
    #[test]
    fn viewport_navigation_orbit_explore_events_and_exit_do_not_select_or_edit() {
        let (ctx, mut a) = app();
        a.view = View::Model;
        a.host.document.editor.tool = Tool::Wall;
        let before = serde_json::to_value(&a.host.document.editor.project).unwrap();
        frame(&ctx, &mut a, vec![], true);
        a.navigation_begin(Mode::Orbit);
        let target = a.camera.target;
        let yaw = a.camera.yaw;
        frame(
            &ctx,
            &mut a,
            vec![
                Event::PointerMoved(pos2(300., 250.)),
                pointer(pos2(300., 250.), true),
            ],
            true,
        );
        frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(380., 280.))],
            true,
        );
        frame(&ctx, &mut a, vec![pointer(pos2(380., 280.), false)], true);
        assert_ne!(a.camera.yaw, yaw);
        assert_eq!(a.camera.target, target);
        a.navigation_begin(Mode::Explore);
        frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(400., 300.))],
            true,
        );
        let eye = a
            .camera
            .eye(a.navigation.model_rect.unwrap().aspect_ratio().into());
        frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(420., 310.))],
            true,
        );
        assert!(
            (a.camera
                .eye(a.navigation.model_rect.unwrap().aspect_ratio().into())
                - eye)
                .length()
                < 1e-9
        );
        let target = a.camera.target;
        frame(&ctx, &mut a, vec![key(Key::W, true)], true);
        assert_ne!(a.camera.target, target);
        let target = a.camera.target;
        frame(&ctx, &mut a, vec![], false);
        assert_eq!(a.camera.target, target);
        frame(
            &ctx,
            &mut a,
            vec![key(Key::W, false), pointer(pos2(420., 310.), true)],
            true,
        );
        frame(&ctx, &mut a, vec![pointer(pos2(420., 310.), false)], true);
        assert_eq!(a.navigation.mode, Mode::Edit);
        assert_eq!(
            serde_json::to_value(&a.host.document.editor.project).unwrap(),
            before
        );
        assert!(a.host.document.editor.undo.is_empty());
    }
    #[test]
    fn viewport_navigation_keyboard_owns_entry_focus_and_all_movement_keys() {
        for (key_code, axis, sign) in [
            (Key::W, 1, 1.),
            (Key::ArrowUp, 1, 1.),
            (Key::S, 1, -1.),
            (Key::ArrowDown, 1, -1.),
            (Key::D, 0, 1.),
            (Key::ArrowRight, 0, 1.),
            (Key::A, 0, -1.),
            (Key::ArrowLeft, 0, -1.),
            (Key::Space, 2, 1.),
            (Key::PageUp, 2, 1.),
            (Key::C, 2, -1.),
            (Key::PageDown, 2, -1.),
        ] {
            let (ctx, mut a) = app();
            a.view = View::Model;
            a.host.document.editor.tool = Tool::Wall;
            let model = serde_json::to_value(&a.host.document.editor.project).unwrap();
            frame(&ctx, &mut a, vec![], true);
            // Reproduce a focusable launch control, and leave the pointer outside
            // the canvas. Neither can make an entered navigation mode inert.
            ctx.memory_mut(|m| m.request_focus(Id::new("navigation-launch-button")));
            a.navigation_begin(Mode::Explore);
            a.navigation.fly = false;
            frame(&ctx, &mut a, vec![Event::PointerGone], true);
            let start = a.camera.target;
            frame(&ctx, &mut a, vec![key(key_code, true)], true);
            frame(&ctx, &mut a, vec![], true);
            let delta = a.camera.target - start;
            let mut expected = a.camera.clone();
            expected.target = start;
            let mut direction = [0.; 3];
            direction[axis] = sign;
            expected.explore_move(direction[0], direction[1], direction[2], 1., false);
            assert!(delta.length() > 0., "{key_code:?} must move");
            assert!((delta.normalize() - (expected.target - start).normalize()).length() < 1e-8);
            frame(&ctx, &mut a, vec![key(key_code, false)], true);
            let stopped = a.camera.target;
            frame(&ctx, &mut a, vec![], true);
            assert_eq!(a.camera.target, stopped, "release stops {key_code:?}");
            assert_eq!(a.host.document.editor.tool, Tool::Wall);
            assert_eq!(
                serde_json::to_value(&a.host.document.editor.project).unwrap(),
                model
            );
            assert!(a.host.document.editor.undo.is_empty());
        }
    }

    #[test]
    fn viewport_navigation_speed_fly_focus_pause_and_escape_recovery() {
        let (ctx, mut a) = app();
        a.view = View::Model;
        frame(&ctx, &mut a, vec![], true);
        a.navigation_begin(Mode::Explore);
        frame(&ctx, &mut a, vec![Event::PointerGone], true);
        let fly = a.navigation.fly;
        frame(&ctx, &mut a, vec![key(Key::F, true)], true);
        assert_eq!(a.navigation.fly, !fly);
        frame(
            &ctx,
            &mut a,
            vec![key(Key::F, false), key(Key::Plus, true)],
            true,
        );
        assert_eq!(a.navigation.speed, 1.25);
        frame(
            &ctx,
            &mut a,
            vec![key(Key::Plus, false), key(Key::Minus, true)],
            true,
        );
        assert_eq!(a.navigation.speed, 1.);
        frame(
            &ctx,
            &mut a,
            vec![key(Key::Minus, false), key(Key::W, true)],
            true,
        );
        let position = a.camera.target;
        frame(&ctx, &mut a, vec![], false);
        assert_eq!(a.camera.target, position);
        // A subsequently focused editor/control keeps the keys, unlike entry focus.
        ctx.memory_mut(|m| m.request_focus(Id::new("speed-or-text-input")));
        frame(&ctx, &mut a, vec![key(Key::W, true)], true);
        assert_eq!(a.camera.target, position);
        a.help = true;
        frame(&ctx, &mut a, vec![], true);
        assert_eq!(a.camera.target, position);
        a.help = false;
        ctx.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
        frame(
            &ctx,
            &mut a,
            vec![key(Key::W, false), key(Key::Escape, true)],
            true,
        );
        assert_eq!(a.navigation.mode, Mode::Edit);
        assert_eq!(a.camera.target, position);
        assert!(a.navigation.exit_frame);
        frame(&ctx, &mut a, vec![key(Key::Escape, false)], true);
        assert!(!a.navigation.exit_frame);
    }

    fn reference_scene(a: &mut AtelierApp) {
        use crate::cad_reference::{Data, Source, Underlay};
        let report = crate::cad_reference::Report {
            bounds: Some([0., 0., 1e9, 1e9]),
            ..Default::default()
        };
        let data: Data = serde_json::from_value(serde_json::json!({
            "version": 3, "source_sha256": "test", "report": report,
            "primitives": [
                {"identity":"line", "layer":"visible", "points":[[50000.,50000.],[50010.,50005.]], "endpoints":[], "text":null, "fill":false},
                {"identity":"hidden", "layer":"hidden", "points":[[-1e8,0.],[1e8,0.]], "endpoints":[], "text":null, "fill":false},
                {"identity":"label", "layer":"visible", "points":[[1e9,1e9]], "endpoints":[], "text":{"value":"label","height":5.,"angle":0.}, "fill":false},
                {"identity":"single", "layer":"visible", "points":[[-1e9,-1e9]], "endpoints":[], "text":null, "fill":false}
            ]
        })).unwrap();
        let mut source: Source = serde_json::from_value(serde_json::json!({
            "id":90, "name":"Reference", "original":{"path":"memory","sha256":"original"},
            "dxf":{"path":"memory","sha256":"dxf"}, "parsed":{"path":"memory","sha256":"parsed"},
            "linked_path":"memory", "report": report
        }))
        .unwrap();
        source.data = Some(std::sync::Arc::new(data));
        let u: Underlay = serde_json::from_value(serde_json::json!({
            "id":91,"source":90,"worksheet":92,"transform":{"anchor":[0.,0.],"xy":{"x":0.,"y":0.},"angle":0.,"metres_per_unit":1.,"elevation":0.},
            "visible":true,"show_3d":true,"show_plan":false,"locked":true,"fade":0,"hidden_layers":["hidden"],"partial_accepted":true
        })).unwrap();
        a.host
            .document
            .editor
            .project
            .drawings
            .cad_sources
            .push(source);
        a.host
            .document
            .editor
            .project
            .drawings
            .cad_underlays
            .push(u);
    }

    #[test]
    fn viewport_navigation_fit_frames_model_not_reference_reports_or_grid() {
        let (_, mut a) = app();
        a.view = View::Model;
        reference_scene(&mut a);
        a.host.document.editor.project.add(
            0,
            Material::Concrete,
            Shape::Slab {
                min: Point::new(54530., 50382.),
                max: Point::new(54630., 50456.),
                thickness: 2.,
            },
        );
        let before = a.host.document.editor.project.clone();
        for aspect in [0.5, 1., 2.4] {
            a.fit_model_contents_for_aspect(aspect);
            assert!(
                a.camera.span < 250.,
                "references must not shrink authored objects"
            );
            let matrix = a.camera.matrix(aspect);
            let projected: Vec<_> = geometry::corners(&before, &before.elements[0])
                .into_iter()
                .map(|p| matrix.project_point3(p))
                .collect();
            let coverage = projected
                .iter()
                .map(|p| p.x.abs().max(p.y.abs()))
                .fold(0., f64::max);
            assert!(
                (coverage - 0.84).abs() < 1e-6,
                "fills the actual pane: {coverage}"
            );
            assert!(projected.iter().all(|p| p.z > 0. && p.z < 1.));
        }
        assert_eq!(a.host.document.editor.project, before);
        assert!(a.host.document.editor.undo.is_empty());
        // A real second object is retained even far from the first.
        a.host.document.editor.project.add(
            0,
            Material::Concrete,
            Shape::Slab {
                min: Point::new(60000., 50000.),
                max: Point::new(60100., 50100.),
                thickness: 2.,
            },
        );
        a.fit_model_contents_for_aspect(1.5);
        assert!(a.camera.span > 1000.);
        for e in &a.host.document.editor.project.elements {
            for p in geometry::corners(&a.host.document.editor.project, e) {
                let q = a.camera.matrix(1.5).project_point3(p);
                assert!(q.x.abs() <= 0.840001 && q.y.abs() <= 0.840001);
            }
        }
    }

    #[test]
    fn viewport_navigation_reference_only_fit_uses_visible_model_segments() {
        let (_, mut a) = app();
        reference_scene(&mut a);
        let before = a.host.document.editor.project.clone();
        a.fit_model_contents_for_aspect(1.5);
        assert!(
            a.camera.span < 20.,
            "no report, hidden layer, isolated point, text or construction grid"
        );
        for p in [
            glam::DVec3::new(50000., 50000., 0.),
            glam::DVec3::new(50010., 50005., 0.),
        ] {
            let q = a.camera.matrix(1.5).project_point3(p);
            assert!(q.x.abs() <= 0.840001 && q.y.abs() <= 0.840001 && q.z > 0. && q.z < 1.);
        }
        assert_eq!(a.host.document.editor.project, before);
    }

    #[test]
    fn viewport_navigation_look_is_gentle_and_ignores_pointer_discontinuities() {
        let (ctx, mut a) = app();
        a.view = View::Model;
        frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(200., 250.))],
            true,
        );
        a.navigation_begin(Mode::Explore);
        frame(&ctx, &mut a, vec![], true);
        let r = a.navigation.model_rect.unwrap();
        let eye = a.camera.eye(r.aspect_ratio().into());
        let yaw = a.camera.yaw;
        frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(300., 250.))],
            true,
        );
        let turn = (a.camera.yaw - yaw).abs().to_degrees();
        assert!(
            (6.0..10.0).contains(&turn),
            "100px must not turn ~52 degrees: {turn}"
        );
        assert!((a.camera.eye(r.aspect_ratio().into()) - eye).length() < 1e-9);
        let yaw = a.camera.yaw;
        frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(780., 580.))],
            true,
        );
        assert_eq!(a.camera.yaw, yaw, "window warp does not spin the eye");
        frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(770., 580.))],
            true,
        );
        assert_ne!(a.camera.yaw, yaw, "normal motion resumes immediately");
        frame(&ctx, &mut a, vec![Event::PointerGone], true);
        let yaw = a.camera.yaw;
        frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(100., 100.))],
            true,
        );
        assert_eq!(a.camera.yaw, yaw, "re-entering does not jump");
    }

    fn middle(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Middle,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }
    #[test]
    fn viewport_navigation_explore_middle_pan_preserves_mode_then_resumes_look_and_walk() {
        let (ctx, mut a) = app();
        a.view = View::Model;
        a.host.document.editor.tool = Tool::Wall;
        a.host.document.editor.wall_points = vec![Point::new(10., 20.)];
        let draft = a.host.document.editor.wall_points.clone();
        let before = serde_json::to_value(&a.host.document.editor.project).unwrap();
        frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(200., 250.))],
            true,
        );
        a.navigation_begin(Mode::Explore);
        frame(&ctx, &mut a, vec![], true);
        let r = a.navigation.model_rect.unwrap();
        let original = a.camera.clone();
        let mut expected = original.clone();
        let (fly, speed) = (a.navigation.fly, a.navigation.speed);
        frame(&ctx, &mut a, vec![middle(pos2(200., 250.), true)], true);
        let out = frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(260., 290.))],
            true,
        );
        expected.pan_pixels(60., 40., r.width().into(), r.height().into());
        assert!((a.camera.target - expected.target).length() < 1e-9);
        let shift = a.camera.target - original.target;
        assert!(shift.length() > 0.);
        assert!(
            (a.camera.eye(r.aspect_ratio().into()) - original.eye(r.aspect_ratio().into()) - shift)
                .length()
                < 1e-9
        );
        assert_eq!(
            (a.camera.yaw, a.camera.pitch, a.camera.span),
            (original.yaw, original.pitch, original.span)
        );
        assert_eq!(out.platform_output.cursor_icon, CursorIcon::Grabbing);
        assert_eq!(a.navigation.mode, Mode::Explore);
        assert_eq!((a.navigation.fly, a.navigation.speed), (fly, speed));
        frame(
            &ctx,
            &mut a,
            vec![
                Event::PointerMoved(pos2(270., 295.)),
                middle(pos2(270., 295.), false),
            ],
            true,
        );
        expected.pan_pixels(10., 5., r.width().into(), r.height().into());
        assert!(
            (a.camera.target - expected.target).length() < 1e-9,
            "release movement belongs to pan"
        );
        assert_eq!(a.navigation.mode, Mode::Explore);
        assert!(a.navigation.explore_pan_anchor.is_none());
        assert_eq!(
            (a.camera.yaw, a.camera.pitch),
            (original.yaw, original.pitch)
        );
        let eye = a.camera.eye(r.aspect_ratio().into());
        frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(280., 300.))],
            true,
        );
        assert_ne!(
            a.camera.yaw, original.yaw,
            "looking resumes on the next motion"
        );
        assert!((a.camera.eye(r.aspect_ratio().into()) - eye).length() < 1e-9);
        let target = a.camera.target;
        frame(&ctx, &mut a, vec![key(Key::W, true)], true);
        assert_ne!(a.camera.target, target);
        frame(&ctx, &mut a, vec![key(Key::W, false)], true);
        let target = a.camera.target;
        frame(&ctx, &mut a, vec![], true);
        assert!(
            (a.camera.target - target).length() < 1e-9,
            "released movement key stops"
        );
        frame(&ctx, &mut a, vec![key(Key::Escape, true)], true);
        assert_eq!(a.navigation.mode, Mode::Edit);
        assert_eq!(a.host.document.editor.wall_points, draft);
        assert_eq!(
            serde_json::to_value(&a.host.document.editor.project).unwrap(),
            before
        );
        assert!(a.host.document.editor.undo.is_empty());
    }
    #[test]
    fn viewport_navigation_explore_middle_pan_capture_controls_and_focus_recovery() {
        let (ctx, mut a) = app();
        a.view = View::Model;
        frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(200., 250.))],
            true,
        );
        a.navigation_begin(Mode::Explore);
        frame(&ctx, &mut a, vec![], true);
        a.navigation.control_rects[1] =
            Some(Rect::from_min_max(pos2(300., 260.), pos2(400., 320.)));
        let yaw = a.camera.yaw;
        let target = a.camera.target;
        frame(&ctx, &mut a, vec![middle(pos2(200., 250.), true)], true);
        frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(320., 280.))],
            true,
        );
        assert_ne!(
            a.camera.target, target,
            "canvas drag keeps capture over controls"
        );
        assert_eq!(a.camera.yaw, yaw);
        frame(&ctx, &mut a, vec![middle(pos2(320., 280.), false)], true);
        frame(
            &ctx,
            &mut a,
            vec![
                Event::PointerMoved(pos2(350., 290.)),
                middle(pos2(350., 290.), true),
            ],
            true,
        );
        let target = a.camera.target;
        frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(250., 240.))],
            true,
        );
        assert_eq!(
            a.camera.target, target,
            "control-origin drag never becomes canvas pan"
        );
        assert_eq!(a.camera.yaw, yaw);
        frame(&ctx, &mut a, vec![middle(pos2(250., 240.), false)], true);
        frame(&ctx, &mut a, vec![middle(pos2(250., 240.), true)], true);
        assert!(a.navigation.explore_pan_anchor.is_some());
        frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(270., 250.))],
            false,
        );
        assert!(a.navigation.explore_pan_anchor.is_none());
        let target = a.camera.target;
        frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(280., 255.))],
            true,
        );
        assert_eq!(
            a.camera.target, target,
            "focus recovery cannot revive a held drag"
        );
        frame(&ctx, &mut a, vec![middle(pos2(280., 255.), false)], true);
        frame(&ctx, &mut a, vec![middle(pos2(280., 255.), true)], true);
        frame(&ctx, &mut a, vec![Event::PointerGone], true);
        assert!(a.navigation.explore_pan_anchor.is_none());
        frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(200., 200.))],
            true,
        );
        assert_eq!(a.camera.target, target);
        frame(&ctx, &mut a, vec![middle(pos2(200., 200.), false)], true);
        frame(&ctx, &mut a, vec![middle(pos2(200., 200.), true)], true);
        frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(220., 210.))],
            true,
        );
        assert_ne!(a.camera.target, target, "a new deliberate drag works");
        frame(&ctx, &mut a, vec![key(Key::Escape, true)], true);
        assert!(a.navigation.explore_pan_anchor.is_none());
        assert_eq!(a.navigation.mode, Mode::Edit);
    }
    #[test]
    fn viewport_navigation_camera_walk_fly_and_look_preserve_eye_and_height() {
        let mut c = Camera::default();
        let eye = c.eye(1.5);
        c.look_pixels(100., 40., 1.5);
        assert!((c.eye(1.5) - eye).length() < 1e-10);
        let z = c.target.z;
        c.explore_move(0., 1., 0., 2., false);
        assert_eq!(c.target.z, z);
        c.explore_move(0., 1., 0., 2., true);
        assert_ne!(c.target.z, z);
        let z = c.target.z;
        c.explore_move(0., 0., 1., 1., false);
        assert!((c.target.z - z - 1.).abs() < 1e-10);
        c.span = 30_000.;
        c.zoom_scroll(1.);
        assert!(c.span > 29_000.);
    }
    #[test]
    fn viewport_navigation_split_fit_changes_only_active_pane_preserves_direction() {
        let (_, mut a) = app();
        a.view = View::Split;
        a.navigation.pane = Pane::Plan;
        a.navigation.plan_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(300., 700.)));
        let camera = a.navigation_position(Pane::Model);
        a.navigation_fit();
        assert_eq!(a.navigation_position(Pane::Model), camera);
        let plan = a.navigation_position(Pane::Plan);
        a.navigation.pane = Pane::Model;
        let yaw = a.camera.yaw;
        let pitch = a.camera.pitch;
        a.navigation_fit();
        assert_eq!(a.navigation_position(Pane::Plan), plan);
        assert_eq!(a.camera.yaw, yaw);
        assert_eq!(a.camera.pitch, pitch);
        assert_eq!(
            a.host.document.editor.status,
            a.language.translate("Fit in Window")
        );
    }
    #[test]
    fn viewport_navigation_normal_bar_has_four_or_six_accessible_actions_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            for view in [View::Plan, View::Model] {
                let (ctx, mut a) = app();
                ctx.enable_accesskit();
                a.language = language;
                a.view = view;
                let mut out = ctx.run_ui(
                    RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 200.))),
                        ..Default::default()
                    },
                    |ui| {
                        ui.horizontal(|ui| a.navigation_bar(ui));
                    },
                );
                out.textures_delta.clear();
                let nodes = out.platform_output.accesskit_update.unwrap().nodes;
                let labels: Vec<_> = nodes.iter().filter_map(|(_, n)| n.label()).collect();
                for key in [
                    "Previous Zoom",
                    "Next Zoom",
                    "Zoom Rectangle",
                    "Fit in Window",
                ] {
                    assert!(
                        labels.contains(&language.translate(key)),
                        "{key}: {labels:?}"
                    );
                }
                for key in ["Orbit", "Explore Model"] {
                    assert_eq!(
                        labels.contains(&language.translate(key)),
                        view == View::Model
                    );
                }
            }
        }
    }
    #[test]
    #[ignore = "local protected project; requires OPENARCHI_NAVIGATION_PROJECT and OPENARCHI_NAVIGATION_OUTPUT"]
    fn viewport_navigation_real_project_roundtrip() {
        let path =
            PathBuf::from(std::env::var_os("OPENARCHI_NAVIGATION_PROJECT").expect("input project"));
        let output =
            PathBuf::from(std::env::var_os("OPENARCHI_NAVIGATION_OUTPUT").expect("output project"));
        assert_ne!(
            std::path::absolute(&path).unwrap(),
            std::path::absolute(&output).unwrap()
        );
        let (ctx, mut a) = app();
        a.host.document.editor = Editor::new(storage::load(&path).unwrap());
        let original = serde_json::to_value(&a.host.document.editor.project).unwrap();
        a.view = View::Plan;
        a.navigation.plan_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(460., 780.)));
        let worksheet_ids: Vec<_> = a
            .host
            .document
            .editor
            .project
            .drawings
            .worksheets
            .iter()
            .map(|w| w.id)
            .collect();
        for worksheet in std::iter::once(None).chain(worksheet_ids.into_iter().map(Some)) {
            a.host.document.editor.project.plan_options.active_worksheet = worksheet;
            a.navigation_fit();
            let r = a.navigation.plan_rect.unwrap();
            if let Some(id) = worksheet {
                for u in a
                    .host
                    .document
                    .editor
                    .project
                    .drawings
                    .cad_underlays
                    .iter()
                    .filter(|u| u.worksheet == id && u.visible)
                {
                    let b = a
                        .host
                        .document
                        .editor
                        .project
                        .drawings
                        .cad_sources
                        .iter()
                        .find(|s| s.id == u.source)
                        .unwrap()
                        .report
                        .bounds
                        .unwrap();
                    for p in [[b[0], b[1]], [b[2], b[3]], [b[0], b[3]], [b[2], b[1]]] {
                        assert!(
                            r.contains(a.screen(r, u.transform.world(p))),
                            "drawing must fit actual canvas"
                        );
                    }
                }
            }
            a.navigation_zoom(2.);
            a.navigation_history(false);
            a.navigation_history(true);
        }
        a.host.document.editor.project.plan_options.active_worksheet =
            original["plan_options"]["active_worksheet"].as_u64();
        a.view = View::Model;
        let selected = a.host.document.editor.selection_ids();
        let filter = a.model_view_filter(&selected);
        let actual_scene = crate::render::prepare_scene(
            &a.host.document.editor.project,
            crate::render::SceneView {
                selected: &selected,
                hovered: None,
                story: filter.story,
                marquee: filter.marquee,
                outside_marquee: filter.outside_marquee,
                selection_only: filter.selection_only,
            },
            None,
        );
        let rendered: Vec<_> = actual_scene.framing_points().collect();
        assert!(
            !rendered.is_empty(),
            "actual visible model is required for the protected case"
        );
        for aspect in [0.5, 1., 2.4] {
            a.fit_model_contents_for_aspect(aspect);
            let matrix = a.camera.matrix(aspect);
            let mut extent = 0_f64;
            for p in &rendered {
                let q = matrix.project_point3(*p);
                assert!(
                    q.x.abs() <= 0.84001 && q.y.abs() <= 0.84001 && q.z > 0. && q.z < 1.,
                    "rendered point {p:?} projects to {q:?}"
                );
                extent = extent.max(q.x.abs()).max(q.y.abs());
            }
            let legacy_outside = a
                .host
                .document
                .editor
                .project
                .elements
                .iter()
                .flat_map(|e| geometry::corners(&a.host.document.editor.project, e))
                .filter(|p| {
                    let q = matrix.project_point3(*p);
                    q.x.abs() > 0.84001 || q.y.abs() > 0.84001
                })
                .count();
            assert!(
                extent > 0.8,
                "authored objects fill pane with unchanged reference: {extent}"
            );
            println!(
                "original-project model fit aspect={aspect} extent={extent} span={} rendered_points={} unused_legacy_box_corners_outside={legacy_outside}",
                a.camera.span,
                rendered.len()
            );
        }
        a.navigation_fit();
        a.navigation_zoom(2.);
        a.navigation_history(false);
        a.camera.orbit_pixels(30., -15.);
        a.camera.explore_move(1., 1., 0., 0.2, true);
        frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(200., 250.))],
            true,
        );
        a.navigation_begin(Mode::Explore);
        frame(&ctx, &mut a, vec![], true);
        let target = a.camera.target;
        let direction = (a.camera.yaw, a.camera.pitch);
        frame(&ctx, &mut a, vec![middle(pos2(200., 250.), true)], true);
        frame(
            &ctx,
            &mut a,
            vec![Event::PointerMoved(pos2(260., 290.))],
            true,
        );
        assert_ne!(a.camera.target, target);
        frame(&ctx, &mut a, vec![middle(pos2(260., 290.), false)], true);
        assert_eq!(a.navigation.mode, Mode::Explore);
        assert_eq!((a.camera.yaw, a.camera.pitch), direction);
        println!("original-project middle-pan retained Explore and direction; model unchanged");
        assert_eq!(
            serde_json::to_value(&a.host.document.editor.project).unwrap(),
            original
        );
        storage::save(&a.host.document.editor.project, &output).unwrap();
        let reopened = storage::load(&output).unwrap();
        assert_eq!(reopened.elements, a.host.document.editor.project.elements);
        assert_eq!(
            reopened.drawings.cad_underlays,
            a.host.document.editor.project.drawings.cad_underlays
        );
        assert_eq!(
            reopened.drawings.cad_sources.len(),
            a.host.document.editor.project.drawings.cad_sources.len()
        );
        assert!(
            reopened
                .drawings
                .cad_sources
                .iter()
                .all(|s| s.load_error.is_none())
        );
        println!(
            "project={} elements={} cad_sources={} output={}",
            path.display(),
            reopened.elements.len(),
            reopened.drawings.cad_sources.len(),
            output.display()
        );
    }
    #[test]
    fn viewport_navigation_model_entry_leaves_worksheet_scope_without_losing_drawing() {
        let (_, mut a) = app();
        a.host
            .document
            .editor
            .project
            .drawings
            .worksheets
            .push(crate::drawings::Worksheet {
                id: 900,
                name: "Reference".into(),
            });
        a.host.document.editor.switch_worksheet(Some(900));
        let drawings = a.host.document.editor.project.drawings.clone();
        a.navigation_begin(Mode::Orbit);
        assert_eq!(a.view, View::Model);
        assert_eq!(
            a.host.document.editor.project.plan_options.active_worksheet,
            None
        );
        assert_eq!(a.host.document.editor.project.drawings, drawings);
        assert_eq!(a.navigation.owner, Some(Pane::Model));
        assert_eq!(a.navigation.mode, Mode::Orbit);
        assert!(a.host.document.editor.undo.is_empty());
    }
    #[test]
    fn viewport_navigation_normal_toolbar_uses_shared_model_entry_from_worksheet() {
        use egui::accesskit::{Action, ActionRequest, TreeId};
        let (ctx, mut a) = app();
        ctx.enable_accesskit();
        a.language = UiLanguage::English;
        a.in_window_menus = false;
        a.host
            .document
            .editor
            .project
            .drawings
            .worksheets
            .push(crate::drawings::Worksheet {
                id: 901,
                name: "Survey".into(),
            });
        a.host.document.editor.switch_worksheet(Some(901));
        a.view = View::Plan;
        let drawings = a.host.document.editor.project.drawings.clone();
        let mut out = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1512., 960.))),
                ..Default::default()
            },
            |ui| a.top(ui),
        );
        out.textures_delta.clear();
        let id = out
            .platform_output
            .accesskit_update
            .unwrap()
            .nodes
            .into_iter()
            .find(|(_, n)| n.label() == Some("Perspective 3D · F3"))
            .expect("normal top toolbar model button")
            .0;
        let mut out = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1512., 960.))),
                events: vec![Event::AccessKitActionRequest(ActionRequest {
                    action: Action::Click,
                    target_tree: TreeId::ROOT,
                    target_node: id,
                    data: None,
                })],
                ..Default::default()
            },
            |ui| a.top(ui),
        );
        out.textures_delta.clear();
        assert_eq!(a.view, View::Model);
        assert_eq!(
            a.host.document.editor.project.plan_options.active_worksheet,
            None
        );
        assert_eq!(a.host.document.editor.project.drawings, drawings);
        assert!(a.host.document.editor.undo.is_empty());
    }
}
