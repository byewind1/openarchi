use super::*;
use crate::opening_joinery::ResizeEdge;
use crate::opening_joinery::{DoorStyle, OpeningJoinery};
#[derive(Clone, Debug)]
pub(super) struct OpeningResize {
    original: Element,
    edge: ResizeEdge,
    press: glam::DVec3,
    revision: u64,
    session: u64,
    view: View,
    candidate: Result<Element, String>,
}
impl AtelierApp {
    pub(super) fn opening_handles(&self, r: Rect) -> Vec<(ResizeEdge, Pos2)> {
        let Some(e) = self.selected_single_opening() else {
            return vec![];
        };
        if self.host.document.editor.locked_element(e.id)
            || self.host.document.editor.tool != Tool::Select
            || self.show_marquee_3d
        {
            return vec![];
        }
        if !self.all_stories && e.story != self.host.document.editor.story {
            return vec![];
        }
        if !self
            .host
            .document
            .editor
            .project
            .plan_options
            .element_visible(&e, &self.host.document.editor.project.layers)
        {
            return vec![];
        }
        let e = self
            .opening_resize
            .as_ref()
            .and_then(|d| d.candidate.as_ref().ok())
            .unwrap_or(&e);
        let Some((a, b)) = geometry::opening_frame_span(&self.host.document.editor.project, e)
        else {
            return vec![];
        };
        let Shape::Opening { sill, height, .. } = e.shape else {
            return vec![];
        };
        let base = self.host.document.editor.project.elevation(e.story) + sill;
        let c = Point::new((a.x + b.x) / 2., (a.y + b.y) / 2.);
        [
            (ResizeEdge::Left, a, base + height / 2.),
            (ResizeEdge::Right, b, base + height / 2.),
            (ResizeEdge::Top, c, base + height),
            (ResizeEdge::Bottom, c, base),
        ]
        .into_iter()
        .filter_map(|(edge, p, z)| self.wall_project_point(r, p, z).map(|q| (edge, q)))
        .collect()
    }
    fn opening_wall_point(&self, r: Rect, pos: Pos2, e: &Element) -> Option<glam::DVec3> {
        let (a, b) = geometry::opening_frame_span(&self.host.document.editor.project, e)?;
        let normal = glam::DVec3::new(-(b.y - a.y), b.x - a.x, 0.).normalize();
        let inverse = self.camera.matrix(r.aspect_ratio().into()).inverse();
        let ndc = glam::DVec3::new(
            f64::from((pos.x - r.left()) / r.width() * 2. - 1.),
            f64::from(1. - (pos.y - r.top()) / r.height() * 2.),
            0.,
        );
        let near = inverse.project_point3(ndc);
        let far = inverse.project_point3(glam::DVec3::new(ndc.x, ndc.y, 0.99));
        let dir = far - near;
        let den = normal.dot(dir);
        if den.abs() < 1e-8 {
            return None;
        }
        let t = normal.dot(glam::DVec3::new(a.x, a.y, 0.) - near) / den;
        let p = near + dir * t;
        (t >= 0. && p.is_finite()).then_some(p)
    }
    pub(super) fn opening_model_input(
        &mut self,
        ui: &Ui,
        r: Rect,
        response: &egui::Response,
    ) -> bool {
        let mut consumed = self.opening_resize.is_some();
        if self.opening_resize.as_ref().is_some_and(|d| {
            self.view != d.view
                || self.host.document.editor.selection_ids() != [d.original.id]
                || self.host.document.editor.revision != d.revision
                || self.host.document.editor.document_session() != d.session
        }) {
            self.opening_resize = None;
            self.host.document.editor.status = "Opening changed during resize".into();
            return true;
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.opening_resize = None;
            self.host.document.editor.status = "Opening resize cancelled".into();
            return consumed;
        }
        if !ui.input(|i| i.focused) || self.host.document.editor.tool != Tool::Select {
            self.opening_resize = None;
            return consumed;
        }
        if self.opening_resize.is_none()
            && response.drag_started_by(PointerButton::Primary)
            && let Some(pos) = ui.input(|i| i.pointer.press_origin())
        {
            if let Some((edge, _)) = self
                .opening_handles(r)
                .into_iter()
                .filter(|(_, p)| p.distance(pos) <= 10.)
                .min_by(|a, b| a.1.distance(pos).total_cmp(&b.1.distance(pos)))
                && let Some(original) = self.selected_single_opening()
                && let Some(press) = self.opening_wall_point(r, pos, &original)
            {
                self.opening_resize = Some(OpeningResize {
                    candidate: Ok(original.clone()),
                    original,
                    edge,
                    press,
                    revision: self.host.document.editor.revision,
                    session: self.host.document.editor.document_session(),
                    view: self.view,
                });
                consumed = true;
            }
        }
        if let Some(d) = self.opening_resize.clone()
            && let Some(pos) = ui.input(|i| i.pointer.latest_pos())
        {
            if let Some(q) = self.opening_wall_point(r, pos, &d.original) {
                let (a, b) =
                    geometry::opening_frame_span(&self.host.document.editor.project, &d.original)
                        .unwrap();
                let l = a.distance(b);
                let diff = q - d.press;
                let delta = match d.edge {
                    ResizeEdge::Left | ResizeEdge::Right => {
                        diff.x * (b.x - a.x) / l + diff.y * (b.y - a.y) / l
                    }
                    _ => diff.z,
                };
                let candidate = crate::opening_joinery::resized(
                    &self.host.document.editor.project,
                    &d.original,
                    d.edge,
                    delta,
                );
                self.host.document.editor.status = match &candidate {
                    Ok(e) => {
                        if let Shape::Opening {
                            width,
                            height,
                            sill,
                            ..
                        } = e.shape
                        {
                            format!(
                                "{}: {:.3} m · {}: {:.3} m · {}: {:.3} m",
                                self.language.translate("Width"),
                                width,
                                self.language.translate("Height"),
                                height,
                                self.language.translate("Sill Height"),
                                sill
                            )
                        } else {
                            String::new()
                        }
                    }
                    Err(e) => localized_error(self.language, e),
                };
                self.opening_resize.as_mut().unwrap().candidate = candidate;
            } else {
                self.opening_resize.as_mut().unwrap().candidate =
                    Err("Opening edge cannot be projected".into());
            }
        }
        if response.drag_stopped()
            && let Some(d) = self.opening_resize.take()
        {
            consumed = true;
            match d.candidate {
                Ok(e) if e != d.original => {
                    if let Err(err) = self.host.document.editor.replace_element(e) {
                        self.host.document.editor.status = err;
                    } else {
                        self.host.document.editor.status = "Opening dimensions applied".into();
                        self.draft = None;
                        self.info_box_opening = None;
                    }
                }
                Err(e) => self.host.document.editor.status = e,
                _ => {}
            }
        }
        consumed
    }
    pub(super) fn paint_opening_handles(&self, ui: &Ui, r: Rect) {
        let p = ui.painter_at(r);
        for (edge, pos) in self.opening_handles(r) {
            let name = self.language.translate(match edge {
                ResizeEdge::Left => "Resize left edge",
                ResizeEdge::Right => "Resize right edge",
                ResizeEdge::Top => "Resize top edge",
                ResizeEdge::Bottom => "Resize bottom edge",
            });
            let hit = Rect::from_center_size(pos, vec2(18., 18.));
            let response = ui.interact(
                hit,
                ui.id().with(("opening_size_handle", edge as u8)),
                Sense::hover(),
            );
            response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, name));
            response.clone().on_hover_text(name);
            if response.hovered() || self.opening_resize.as_ref().is_some_and(|d| d.edge == edge) {
                ui.ctx()
                    .set_cursor_icon(if matches!(edge, ResizeEdge::Top | ResizeEdge::Bottom) {
                        CursorIcon::ResizeVertical
                    } else {
                        CursorIcon::ResizeHorizontal
                    });
            }
            p.rect_filled(Rect::from_center_size(pos, vec2(8., 8.)), 1., ACCENT);
            p.rect_stroke(
                Rect::from_center_size(pos, vec2(8., 8.)),
                1.,
                Stroke::new(1., Color32::WHITE),
                egui::StrokeKind::Inside,
            );
        }
        if let Some(d) = &self.opening_resize {
            let text = match &d.candidate {
                Ok(_) => self.host.document.editor.status.clone(),
                Err(e) => localized_error(self.language, e),
            };
            let pos = ui.input(|i| i.pointer.latest_pos()).unwrap_or(r.center()) + vec2(18., 18.);
            p.text(
                pos,
                Align2::LEFT_TOP,
                text,
                FontId::proportional(13.),
                if d.candidate.is_ok() {
                    ACCENT
                } else {
                    Color32::LIGHT_RED
                },
            );
        }
    }
    pub(super) fn opening_preview_project(&self) -> Option<Project> {
        let e = self.opening_resize.as_ref()?.candidate.as_ref().ok()?;
        let mut p = self.host.document.editor.project.clone();
        *p.elements.iter_mut().find(|x| x.id == e.id)? = e.clone();
        Some(p)
    }
    pub(super) fn opening_joinery_controls(
        ui: &mut Ui,
        language: UiLanguage,
        s: &mut OpeningJoinery,
    ) -> bool {
        let before = *s;
        egui::ComboBox::from_id_salt(ui.next_auto_id())
            .selected_text(language.translate(if s.door_style == DoorStyle::Single {
                "Single door"
            } else {
                "Double door"
            }))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut s.door_style,
                    DoorStyle::Single,
                    language.translate("Single door"),
                );
                ui.selectable_value(
                    &mut s.door_style,
                    DoorStyle::Double,
                    language.translate("Double door"),
                );
            });
        if s.door_style == DoorStyle::Single
            && ui.button(language.translate("Flip hinge")).clicked()
        {
            s.hinge_at_start = !s.hinge_at_start;
        }
        if ui.button(language.translate("Flip swing")).clicked() {
            s.opens_positive = !s.opens_positive;
        }
        before != *s
    }
}
