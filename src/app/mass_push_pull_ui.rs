//! Transient face gesture adapter; candidates use the shared mass kernel and
//! confirmation uses UpdateElement. No preview writes the authoritative model.
use super::*;
use crate::concept_mass::{MassFace, face_normal, face_triangles, pushed_face};
use glam::DVec3;

#[derive(Default)]
pub(super) struct MassPushPull {
    pub enabled: bool,
    session: Option<Session>,
    hover: Option<(Element, MassFace)>,
}
struct Session {
    source: Element,
    path: String,
    face: MassFace,
    anchor: DVec3,
    origin: Pos2,
    axis: Vec2,
    distance: f64,
    text: String,
    numeric: bool,
    dragging: bool,
    candidate: Result<Element, String>,
}
impl AtelierApp {
    pub(super) fn mass_push_pull_preview(&self) -> Option<Project> {
        let s = self.mass_push_pull.session.as_ref()?;
        let candidate = s.candidate.as_ref().ok()?;
        let mut project = self.host.document.editor.project.clone();
        *project.elements.iter_mut().find(|e| e.id == candidate.id)? = candidate.clone();
        Some(project)
    }
    pub(super) fn cancel_mass_push_pull(&mut self) {
        self.mass_push_pull = MassPushPull::default();
    }
    pub(super) fn mass_push_pull_active(&self) -> bool {
        self.mass_push_pull.session.is_some()
    }
    pub(super) fn mass_face_pick(&self, r: Rect, pos: Pos2) -> Option<(Element, MassFace, DVec3)> {
        if !r.contains(pos) {
            return None;
        }
        let ids = self.host.document.editor.selection_ids();
        let ndc = [
            f64::from((pos.x - r.left()) / r.width() * 2. - 1.),
            f64::from(1. - (pos.y - r.top()) / r.height() * 2.),
        ];
        let editor = &self.host.document.editor;
        // Respect visibility, clipping and nearest scene object before face testing.
        let id = editor
            .pick_model(
                &self.camera,
                ndc,
                r.aspect_ratio().into(),
                self.model_view_filter(&ids),
            )
            .ok()
            .flatten()?;
        if !ids.contains(&id) || editor.locked_element(id) {
            return None;
        }
        let element = editor
            .project
            .elements
            .iter()
            .find(|e| e.id == id && matches!(e.shape, Shape::ConceptMass { .. }))?;
        let inv = self.camera.matrix(r.aspect_ratio().into()).inverse();
        let origin = inv.project_point3(DVec3::new(ndc[0], ndc[1], 0.));
        let direction = (inv.project_point3(DVec3::new(ndc[0], ndc[1], 0.99)) - origin).normalize();
        face_triangles(element, editor.project.elevation(element.story))
            .ok()?
            .into_iter()
            .filter_map(|(face, [a, b, c])| {
                geometry::ray_triangle(origin, direction, a, b, c).map(|t| (t, face))
            })
            .filter(|(t, _)| {
                let p = origin + direction * *t;
                self.model_view_filter(&ids)
                    .contains_anchor(Point::new(p.x, p.y), p.z)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(t, face)| (element.clone(), face, origin + direction * t))
    }
    pub(super) fn mass_push_pull_input(
        &mut self,
        ui: &mut Ui,
        r: Rect,
        response: &Response,
        over_controls: bool,
    ) -> bool {
        self.mass_push_pull.hover = None;
        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Default);
        }
        if self.host.document.editor.tool != Tool::Select
            || self.wall_insert.is_some()
            || !ui.input(|i| i.focused)
            || self.navigation.mode != viewport_navigation::Mode::Edit
        {
            self.cancel_mass_push_pull();
            return false;
        }
        let current = self.mass_push_pull.session.as_ref().is_none_or(|s| {
            s.path == self.path
                && self
                    .host
                    .document
                    .editor
                    .selection_ids()
                    .contains(&s.source.id)
                && !self.host.document.editor.locked_element(s.source.id)
                && self
                    .host
                    .document
                    .editor
                    .project
                    .elements
                    .iter()
                    .any(|e| *e == s.source)
                && self
                    .model_view_filter(&self.host.document.editor.selection_ids())
                    .includes(&self.host.document.editor.project, &s.source)
                && self
                    .host
                    .document
                    .editor
                    .project
                    .plan_options
                    .element_visible(&s.source, &self.host.document.editor.project.layers)
        });
        if !current {
            self.cancel_mass_push_pull();
            return true;
        }
        let at = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| r.contains(*p));
        let modified = ui.input(|i| {
            i.modifiers.alt
                || i.modifiers.shift
                || i.pointer.button_down(PointerButton::Middle)
                || i.pointer.button_down(PointerButton::Secondary)
        });
        if self.mass_push_pull.session.is_none() {
            if over_controls
                || at.is_some_and(|p| ui.ctx().layer_id_at(p) != Some(ui.layer_id()))
                || modified
                || self.mass_parameter_draft_active()
                || self.pick_armed
                || self.inject_armed
                || (!self.mass_push_pull.enabled
                    && at.is_some_and(|p| self.selected_mass_model_hit(r, p).is_some()))
            {
                return false;
            }
            let pick_at = if response.drag_started_by(PointerButton::Primary) {
                ui.input(|i| i.pointer.press_origin())
            } else {
                at
            };
            if let Some((source, face, anchor)) = pick_at.and_then(|p| self.mass_face_pick(r, p)) {
                self.mass_push_pull.hover = Some((source.clone(), face));
                if response.clicked_by(PointerButton::Primary)
                    || response.drag_started_by(PointerButton::Primary)
                {
                    let start = ui
                        .input(|i| i.pointer.press_origin())
                        .unwrap_or(at.unwrap());
                    let (source, face, anchor) = self
                        .mass_face_pick(r, start)
                        .unwrap_or((source, face, anchor));
                    let normal = face_normal(&source, face, anchor).unwrap_or(DVec3::Z);
                    let matrix = self.camera.matrix(r.aspect_ratio().into());
                    let project = |p| {
                        let p = matrix.project_point3(p);
                        pos2(
                            r.left() + (p.x as f32 + 1.) * r.width() * 0.5,
                            r.top() + (1. - p.y as f32) * r.height() * 0.5,
                        )
                    };
                    let mut axis = project(anchor + normal) - project(anchor);
                    // End-on normals have no projected direction: upward drag remains usable.
                    if axis.length() < 2. {
                        axis = vec2(0., -r.height() / self.camera.span as f32);
                    }
                    self.mass_push_pull.session = Some(Session {
                        source: source.clone(),
                        path: self.path.clone(),
                        face,
                        anchor,
                        origin: start,
                        axis,
                        distance: 0.,
                        text: String::new(),
                        numeric: false,
                        dragging: response.drag_started_by(PointerButton::Primary),
                        candidate: Ok(source),
                    });
                }
                return true;
            }
            return false;
        }
        let mut s = self.mass_push_pull.session.take().unwrap();
        let (scale, unit) = drag_tracker::DragTracker::unit(
            self.host
                .document
                .editor
                .project
                .plan_options
                .dimension_style,
        );
        let over_tracker_before = at.is_some_and(|p| {
            ui.ctx()
                .data(|d| d.get_temp::<Rect>(Id::new("mass_push_pull_tracker_rect")))
                .is_some_and(|r| r.contains(p))
        });
        if !s.numeric
            && !ui
                .ctx()
                .memory(|m| m.has_focus(Id::new("mass_push_pull_numeric_input")))
            && !over_tracker_before
            && !over_controls
            && !modified
            && let Some(pos) = at
            && ui.ctx().layer_id_at(pos) == Some(ui.layer_id())
        {
            let n = face_normal(&s.source, s.face, s.anchor).unwrap_or(DVec3::Z);
            let inv = self.camera.matrix(r.aspect_ratio().into()).inverse();
            let ndc = DVec3::new(
                f64::from((pos.x - r.left()) / r.width() * 2. - 1.),
                f64::from(1. - (pos.y - r.top()) / r.height() * 2.),
                0.,
            );
            let origin = inv.project_point3(ndc);
            let ray = (inv.project_point3(DVec3::new(ndc.x, ndc.y, 0.99)) - origin).normalize();
            let b = n.dot(ray);
            let w = s.anchor - origin;
            s.distance = if 1. - b * b > 0.02 {
                (b * ray.dot(w) - n.dot(w)) / (1. - b * b)
            } else {
                f64::from((pos - s.origin).dot(s.axis) / s.axis.length_sq())
            };
        }
        let numeric_id = Id::new("mass_push_pull_numeric_input");
        let field_focused = ui.ctx().memory(|m| m.has_focus(numeric_id));
        let mut enter = false;
        for event in ui.input(|i| i.events.clone()) {
            match event {
                Event::Text(text) | Event::Paste(text)
                    if !field_focused
                        && text
                            .chars()
                            .all(|c| c.is_ascii_digit() || ".-+eE".contains(c)) =>
                {
                    if !s.numeric {
                        s.text.clear();
                    }
                    s.numeric = true;
                    s.text.push_str(&text);
                }
                Event::Key {
                    key: Key::A,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.command && !field_focused => {
                    s.numeric = true;
                    s.text.clear();
                }
                Event::Key {
                    key: Key::Backspace,
                    pressed: true,
                    ..
                } if s.numeric && !field_focused => {
                    s.text.pop();
                }
                Event::Key {
                    key: Key::Enter,
                    pressed: true,
                    ..
                } => enter = true,
                _ => {}
            }
        }
        let value = if s.numeric {
            s.text
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .map(|v| v / scale)
        } else {
            Some(if s.distance.abs() < 1e-7 {
                0.
            } else {
                s.distance
            })
        };
        if let Some(value) = value {
            s.distance = value;
        }
        s.candidate = value
            .ok_or_else(|| "Invalid edit parameters".to_string())
            .and_then(|d| pushed_face(&s.source, s.face, s.anchor, d));
        let mut apply = false;
        let mut cancel = false;
        let language = self.language;
        let area = Area::new(Id::new("mass_push_pull_tracker"))
            .default_size(vec2(254., 160.))
            .order(Order::Foreground)
            .fixed_pos(s.origin + vec2(22., 20.))
            .constrain_to(r)
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(240.);
                    ui.strong(language.translate("Push/Pull face"));
                    ui.horizontal(|ui| {
                        ui.label(language.translate("Distance"));
                        if !s.numeric && !field_focused {
                            s.text = format!("{:.6}", s.distance * scale)
                                .trim_end_matches('0')
                                .trim_end_matches('.')
                                .to_string();
                        }
                        let input = ui.add(
                            TextEdit::singleline(&mut s.text)
                                .id(numeric_id)
                                .desired_width(110.),
                        );
                        input.widget_info(|| {
                            WidgetInfo::labeled(
                                WidgetType::TextEdit,
                                true,
                                language.translate("Distance"),
                            )
                        });
                        ui.ctx().data_mut(|d| {
                            d.insert_temp(Id::new("mass_push_pull_distance"), input.rect)
                        });
                        if input.changed() {
                            s.numeric = true;
                        }
                        ui.label(unit);
                    });
                    if s.numeric && ui.small_button(language.translate("Use mouse")).clicked() {
                        s.numeric = false;
                        ui.memory_mut(|m| m.surrender_focus(numeric_id));
                    }
                    ui.label(
                        language
                            .translate("Move, then click to confirm · Type distance · Esc cancels"),
                    );
                    if let Err(error) = &s.candidate {
                        ui.colored_label(ui.visuals().error_fg_color, language.translate(error));
                    }
                    ui.horizontal(|ui| {
                        let a = ui.add_enabled(
                            s.candidate.is_ok(),
                            Button::new(language.translate("Apply changes")),
                        );
                        let c = ui.button(language.translate("Cancel"));
                        ui.ctx().data_mut(|d| {
                            d.insert_temp(Id::new("mass_push_pull_apply"), a.rect);
                            d.insert_temp(Id::new("mass_push_pull_cancel"), c.rect);
                        });
                        apply = a.clicked();
                        cancel = c.clicked();
                    });
                });
            })
            .response
            .rect;
        if s.numeric {
            s.candidate = s
                .text
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| "Invalid edit parameters".to_owned())
                .and_then(|v| {
                    s.distance = v / scale;
                    pushed_face(&s.source, s.face, s.anchor, s.distance)
                });
        }
        ui.ctx()
            .data_mut(|d| d.insert_temp(Id::new("mass_push_pull_tracker_rect"), area));
        let over_tracker = at.is_some_and(|p| area.contains(p));
        let commit = apply
            || enter
            || (!modified
                && !over_controls
                && !over_tracker
                && at.is_some()
                && (response.clicked_by(PointerButton::Primary)
                    || (s.dragging && response.drag_stopped_by(PointerButton::Primary))));
        if cancel {
            return true;
        }
        if commit && let Ok(element) = &s.candidate {
            if *element != s.source {
                if let Err(error) =
                    self.host
                        .document
                        .editor
                        .apply(crate::command::ModelCommand::UpdateElement {
                            element: element.clone(),
                        })
                {
                    self.host.document.editor.status = error;
                    self.mass_push_pull.session = Some(s);
                    return true;
                }
                self.host.document.editor.status =
                    "Properties updated · Both views synchronized".into();
                self.mass_parameters = None;
                self.draft_id = None;
            }
            return true;
        }
        // Releasing over a panel must not accidentally confirm on a later frame.
        if response.drag_stopped_by(PointerButton::Primary) {
            s.dragging = false;
        }
        self.mass_push_pull.session = Some(s);
        true
    }
    pub(super) fn paint_mass_push_pull(&self, ui: &Ui, r: Rect) {
        let painter = ui.painter_at(r);
        let target = self
            .mass_push_pull
            .session
            .as_ref()
            .map(|s| (s.candidate.as_ref().unwrap_or(&s.source), s.face))
            .or_else(|| self.mass_push_pull.hover.as_ref().map(|(e, f)| (e, *f)));
        if let Some((element, face)) = target
            && let Ok(tris) = face_triangles(
                element,
                self.host.document.editor.project.elevation(element.story),
            )
        {
            for (_, tri) in tris.into_iter().filter(|(f, _)| *f == face) {
                let points: Option<Vec<_>> = tri
                    .into_iter()
                    .map(|p| self.wall_project_point(r, Point::new(p.x, p.y), p.z))
                    .collect();
                if let Some(points) = points {
                    painter.add(egui::Shape::convex_polygon(
                        points,
                        Color32::from_rgba_unmultiplied(41, 169, 211, 62),
                        Stroke::NONE,
                    ));
                }
            }
        }
        if let Some(s) = &self.mass_push_pull.session
            && let Ok(n) = face_normal(&s.source, s.face, s.anchor)
        {
            let end = s.anchor + n * s.distance;
            if let (Some(a), Some(b)) = (
                self.wall_project_point(r, Point::new(s.anchor.x, s.anchor.y), s.anchor.z),
                self.wall_project_point(r, Point::new(end.x, end.y), end.z),
            ) {
                painter.line_segment([a, b], Stroke::new(2., ACCENT));
                painter.circle_filled(b, 3., ACCENT);
            }
        }
        if target.is_some()
            && let Some(pos) = ui
                .input(|i| i.pointer.hover_pos())
                .filter(|p| r.contains(*p))
            && !ui.input(|i| {
                i.modifiers.alt
                    || i.pointer.button_down(PointerButton::Middle)
                    || i.pointer.button_down(PointerButton::Secondary)
            })
            && ui.ctx().layer_id_at(pos) == Some(ui.layer_id())
        {
            ui.ctx().set_cursor_icon(CursorIcon::None);
            crate::cursor::paint_push_pull(&painter, pos, self.mass_push_pull.session.is_some());
        }
    }
}
