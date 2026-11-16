//! Coordinate references and their plan gesture. Model edits stay in ApplicationHost.
use super::*;

impl AtelierApp {
    pub(super) fn input_origins_menu(&mut self, ui: &mut Ui) {
        let language = self.language;
        let origin = self.host.document.editor.project.plan_options.user_origin;
        ui.menu_button(
            language.translate(if origin.is_some() {
                "User origin"
            } else {
                "Project origin"
            }),
            |ui| {
                let (scale, unit) = super::drag_tracker::DragTracker::unit(
                    self.host
                        .document
                        .editor
                        .project
                        .plan_options
                        .dimension_style,
                );
                ui.label(
                    language
                        .translate("Option+Shift: click an element node to set the user origin"),
                );
                ui.separator();
                ui.label(format!(
                    "{}: 0, 0 {unit}",
                    language.translate("Project origin")
                ));
                if let Some(point) = origin {
                    ui.label(format!(
                        "{}: {:.3}, {:.3} {unit}",
                        language.translate("User origin"),
                        point.x * scale,
                        point.y * scale
                    ));
                }
                if let Some(point) = self.pointer_drag_origin() {
                    ui.label(format!(
                        "{}: {:.3}, {:.3} {unit}",
                        language.translate("Edit origin"),
                        point.x * scale,
                        point.y * scale
                    ));
                }
                let grid = &self.host.document.editor.project.grid_settings;
                ui.label(format!(
                    "{}: {:.3}, {:.3} {unit}",
                    language.translate("Grid origin"),
                    grid.origin_x * scale,
                    grid.origin_y * scale
                ));
                ui.label(language.translate("Coordinate references do not move model geometry"));
                if ui
                    .add_enabled(
                        origin.is_some() && !self.menu_gated(),
                        Button::new(language.translate("Restore project origin")),
                    )
                    .clicked()
                {
                    self.change_input_origin(None);
                    ui.close();
                }
            },
        );
    }

    pub(super) fn change_input_origin(&mut self, origin: Option<Point>) {
        if self.host.document.editor.project.plan_options.user_origin == origin {
            return;
        }
        match self.host.set_user_origin(self.host.context(), origin) {
            Ok(_) => {
                self.pointer_drag_origin_changed();
                self.host.document.editor.status = if origin.is_some() {
                    "User origin set; model geometry unchanged"
                } else {
                    "Project origin restored; model geometry unchanged"
                }
                .into();
            }
            Err(error) => self.host.document.editor.status = error,
        }
    }

    pub(super) fn input_origin_node(&self, raw: Point) -> Option<Point> {
        let project = &self.host.document.editor.project;
        project
            .elements
            .iter()
            .filter(|element| {
                project
                    .plan_options
                    .element_visible(element, &project.layers)
                    && geometry::element_on_plan_story(
                        project,
                        element,
                        self.host.document.editor.story,
                    )
                    && element.worksheet == project.plan_options.active_worksheet
            })
            .flat_map(|element| geometry::element_edit_nodes(project, element))
            .filter(|point| point.distance(raw) <= 9. / self.plan_scale)
            .min_by(|a, b| a.distance(raw).total_cmp(&b.distance(raw)))
    }

    /// Own the entire press/release, including a modifier release in the click frame.
    pub(super) fn input_origin_canvas(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        response: &Response,
        interactive: bool,
    ) -> bool {
        let held = ui.input(|i| {
            i.modifiers.alt && i.modifiers.shift && !i.modifiers.command && !i.modifiers.ctrl
        });
        let latched = self.user_origin_gesture;
        if !interactive || self.canvas_keyboard_blocked(ui) {
            if !ui.input(|i| i.pointer.primary_down()) {
                self.user_origin_gesture = false;
            }
            return false;
        }
        let active = held && response.hovered();
        if active {
            ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
            if ui.input(|i| i.pointer.button_pressed(PointerButton::Primary)) {
                self.user_origin_gesture = true;
                let point = ui
                    .input(|i| i.pointer.press_origin())
                    .and_then(|p| rect.contains(p).then(|| self.world(rect, p)))
                    .and_then(|p| self.input_origin_node(p));
                if let Some(point) = point {
                    self.change_input_origin(Some(point));
                } else {
                    self.host.document.editor.status =
                        "Click an element node to set the user origin".into();
                }
            }
        }
        if !ui.input(|i| i.pointer.primary_down()) {
            self.user_origin_gesture = false;
        }
        active || latched
    }

    pub(super) fn paint_input_origins(&self, ui: &Ui, rect: Rect) {
        let painter = ui.painter_at(rect);
        let mark = |point: Point, label: &str, color: Color32| {
            let center = self.screen(rect, point);
            if !rect.expand(14.).contains(center) {
                return;
            }
            for delta in [vec2(5., 0.), vec2(0., 5.)] {
                painter.line_segment([center - delta, center + delta], Stroke::new(1.3, color));
            }
            painter.text(
                center + vec2(7., -7.),
                Align2::LEFT_BOTTOM,
                self.language.translate(label),
                FontId::proportional(10.),
                color,
            );
        };
        let user = self.host.document.editor.project.plan_options.user_origin;
        if user == Some(Point::default()) {
            mark(
                Point::default(),
                "Project / user origin",
                Color32::from_rgb(188, 100, 25),
            );
        } else {
            mark(Point::default(), "Project origin", Color32::from_gray(130));
            if let Some(point) = user {
                mark(point, "User origin", Color32::from_rgb(188, 100, 25));
            }
        }
        if ui.input(|i| i.modifiers.alt && i.modifiers.shift) {
            if let Some(point) = ui
                .input(|i| i.pointer.hover_pos())
                .filter(|p| rect.contains(*p))
                .and_then(|p| self.input_origin_node(self.world(rect, p)))
            {
                painter.circle_stroke(self.screen(rect, point), 6., Stroke::new(2., ACCENT));
            }
        }
    }
}
