use super::*;

impl AtelierApp {
    pub(super) fn model_view(&mut self, ui: &mut Ui) {
        let r = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(r, Sense::click_and_drag());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "3D model canvas"));
        let placing_3d_marquee = self.editor.tool == Tool::Marquee;
        let base_z = self.editor.project.elevation(self.editor.story);
        let hovered_model_hit = response.interact_pointer_pos().and_then(|pos| {
            self.camera.pick(
                &self.editor.project,
                [
                    (pos.x - r.left()) / r.width() * 2. - 1.,
                    1. - (pos.y - r.top()) / r.height() * 2.,
                ],
                r.aspect_ratio(),
                if self.all_stories {
                    None
                } else {
                    Some(self.editor.story)
                },
            )
        });
        if placing_3d_marquee
            && self.marquee_geometry == MarqueeGeometry::Rectangle
            && response.drag_started_by(PointerButton::Primary)
            && let Some(pos) = ui.input(|i| i.pointer.press_origin())
        {
            self.marquee_3d_preview = Some((pos, pos));
        }
        if placing_3d_marquee
            && self.marquee_geometry == MarqueeGeometry::Rectangle
            && response.dragged_by(PointerButton::Primary)
            && let (Some((start, _)), Some(pos)) =
                (self.marquee_3d_preview, response.interact_pointer_pos())
        {
            self.marquee_3d_preview = Some((start, pos));
        }
        if placing_3d_marquee
            && self.marquee_geometry == MarqueeGeometry::Rectangle
            && response.drag_stopped()
            && let Some((a, b)) = self.marquee_3d_preview.take()
            && a.distance(b) > 5.
        {
            let points: Option<Vec<Point>> = [
                pos2(a.x, a.y),
                pos2(b.x, a.y),
                pos2(b.x, b.y),
                pos2(a.x, b.y),
            ]
            .into_iter()
            .map(|pos| Self::screen_to_ground(&self.camera, r, pos, base_z))
            .collect();
            if let Some(points) = points {
                self.place_3d_marquee(points, base_z);
            } else {
                self.editor.status = "Marquee base must be below the perspective horizon".into();
            }
        }
        if placing_3d_marquee && self.marquee_geometry != MarqueeGeometry::Rectangle {
            self.marquee_3d_cursor = ui
                .input(|i| i.pointer.hover_pos())
                .filter(|pos| r.contains(*pos));
        }
        if placing_3d_marquee
            && self.marquee_geometry != MarqueeGeometry::Rectangle
            && response.clicked_by(PointerButton::Primary)
            && let Some(pos) = response.interact_pointer_pos()
        {
            if let Some(point) = Self::screen_to_ground(&self.camera, r, pos, base_z) {
                self.marquee_points.push(point);
                self.marquee_3d_points.push(pos);
                match self.marquee_geometry {
                    MarqueeGeometry::Polygon => {}
                    MarqueeGeometry::RotatedRectangle if self.marquee_points.len() == 3 => {
                        let vertices = geometry::rotated_rectangle_polygon(
                            self.marquee_points[0],
                            self.marquee_points[1],
                            self.marquee_points[2],
                        );
                        if let Some(vertices) = vertices {
                            self.place_3d_marquee(vertices, base_z);
                        } else {
                            self.marquee_points.clear();
                            self.marquee_3d_points.clear();
                            self.editor.status = "Rotated rectangle needs non-zero sides".into();
                        }
                    }
                    _ => {}
                }
            } else {
                self.editor.status = "Marquee base must be below the perspective horizon".into();
            }
        }
        if placing_3d_marquee
            && self.marquee_geometry == MarqueeGeometry::Polygon
            && response.clicked_by(PointerButton::Secondary)
        {
            if self.marquee_points.len() >= 3 {
                self.place_3d_marquee(self.marquee_points.clone(), base_z);
            } else {
                self.marquee_points.clear();
                self.marquee_3d_points.clear();
                self.marquee_3d_cursor = None;
                self.editor.status = "3D Marquee polygon needs at least three points".into();
            }
        }
        let middle_dragging = ui.input(|i| {
            i.pointer.button_down(PointerButton::Middle)
                && i.pointer.delta().length_sq() > 0.
                && i.pointer.hover_pos().is_some_and(|pos| r.contains(pos))
        });
        let shift_middle_orbit = middle_dragging && ui.input(|i| i.modifiers.shift);
        if (response.dragged_by(PointerButton::Primary)
            && !placing_3d_marquee
            && self.editor.tool != Tool::Marquee)
            || shift_middle_orbit
        {
            let d = ui.input(|i| i.pointer.delta());
            self.camera.yaw -= d.x * 0.009;
            self.camera.pitch = (self.camera.pitch + d.y * 0.007).clamp(0.08, 1.53);
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        }
        let model_pan = response.dragged_by(PointerButton::Secondary)
            || (middle_dragging && !ui.input(|i| i.modifiers.shift));
        if model_pan {
            let delta = ui.input(|i| i.pointer.delta());
            let right = glam::Vec3::new(-self.camera.yaw.sin(), self.camera.yaw.cos(), 0.);
            let up = glam::Vec3::new(
                -self.camera.yaw.cos() * self.camera.pitch.sin(),
                -self.camera.yaw.sin() * self.camera.pitch.sin(),
                self.camera.pitch.cos(),
            );
            self.camera.target += (-right * delta.x + up * delta.y)
                * self.camera.vertical_span(r.aspect_ratio())
                / r.height();
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            self.camera.span = (self.camera.span * (-scroll * 0.002).exp()).clamp(2., 150.);
        }
        if response.clicked()
            && self.editor.tool != Tool::Marquee
            && let Some(pos) = response.interact_pointer_pos()
        {
            let hit = self.camera.pick(
                &self.editor.project,
                [
                    (pos.x - r.left()) / r.width() * 2. - 1.,
                    1. - (pos.y - r.top()) / r.height() * 2.,
                ],
                r.aspect_ratio(),
                if self.all_stories {
                    None
                } else {
                    Some(self.editor.story)
                },
            );
            self.editor.set_selection(hit.into_iter().collect());
            self.draft_id = None;
        }
        if let Some(renderer) = &mut self.renderer {
            let scale = ui.ctx().pixels_per_point();
            let selected = self.editor.selection_ids();
            let selection_only =
                self.show_marquee_3d && !selected.is_empty() && self.editor.marquee.is_none();
            renderer.render(
                &self.editor.project,
                &selected,
                &self.camera,
                [(r.width() * scale) as u32, (r.height() * scale) as u32],
                if self.all_stories {
                    None
                } else {
                    Some(self.editor.story)
                },
                if self.show_marquee_3d && !selection_only {
                    self.editor.marquee.as_ref()
                } else {
                    None
                },
                self.marquee_outside,
                selection_only,
            );
            ui.painter().image(
                renderer.texture,
                r,
                Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                Color32::WHITE,
            );
        } else {
            ui.painter().rect_filled(r, 0., LINE);
        }
        let p = ui.painter_at(r);
        if let Some((a, b)) = self.marquee_3d_preview {
            let rect = Rect::from_two_pos(a, b);
            p.rect_filled(rect, 0., Color32::from_rgba_unmultiplied(49, 136, 177, 25));
            p.rect_stroke(rect, 0., Stroke::new(1.5, ACCENT), StrokeKind::Inside);
        }
        if self.show_marquee_3d
            && let Some(area) = &self.editor.marquee
        {
            let (bottom, top) = area.z_range.unwrap_or_else(|| {
                let mut lo = f32::INFINITY;
                let mut hi = f32::NEG_INFINITY;
                for element in &self.editor.project.elements {
                    for corner in geometry::corners(&self.editor.project, element) {
                        lo = lo.min(corner.z);
                        hi = hi.max(corner.z);
                    }
                }
                if lo.is_finite() && hi.is_finite() {
                    (lo, hi)
                } else {
                    (0., self.marquee_height)
                }
            });
            let matrix = self.camera.matrix(r.aspect_ratio());
            let screen_point = |point: Point, z: f32| {
                let ndc = matrix.project_point3(glam::Vec3::new(point.x, point.y, z));
                ndc.is_finite().then(|| {
                    pos2(
                        r.left() + (ndc.x + 1.) * 0.5 * r.width(),
                        r.top() + (1. - ndc.y) * 0.5 * r.height(),
                    )
                })
            };
            let base: Vec<_> = area
                .vertices
                .iter()
                .filter_map(|point| screen_point(*point, bottom))
                .collect();
            let cap: Vec<_> = area
                .vertices
                .iter()
                .filter_map(|point| screen_point(*point, top))
                .collect();
            if base.len() == area.vertices.len() && cap.len() == area.vertices.len() {
                for i in 0..base.len() {
                    let next = (i + 1) % base.len();
                    p.line_segment([base[i], base[next]], Stroke::new(1.7, ACCENT));
                    p.line_segment(
                        [cap[i], cap[next]],
                        Stroke::new(1.2, Color32::from_rgb(114, 193, 224)),
                    );
                    p.line_segment(
                        [base[i], cap[i]],
                        Stroke::new(1.1, Color32::from_rgb(114, 193, 224)),
                    );
                }
            }
        }
        if !self.marquee_3d_points.is_empty() {
            let mut preview = self.marquee_3d_points.clone();
            if let Some(cursor) = self.marquee_3d_cursor.filter(|pos| r.contains(*pos)) {
                preview.push(cursor);
                if self.marquee_geometry == MarqueeGeometry::RotatedRectangle && preview.len() >= 3
                {
                    preview = vec![
                        self.marquee_3d_points[0],
                        self.marquee_3d_points[1],
                        cursor,
                        self.marquee_3d_points[0],
                    ];
                }
            }
            if self.marquee_geometry == MarqueeGeometry::Polygon && preview.len() >= 3 {
                p.add(egui::Shape::closed_line(preview, Stroke::new(1.5, ACCENT)));
            } else if preview.len() >= 2 {
                p.add(egui::Shape::line(preview, Stroke::new(1.5, ACCENT)));
            }
            for point in &self.marquee_3d_points {
                p.circle_filled(*point, 3.5, Color32::WHITE);
                p.circle_stroke(*point, 3.5, Stroke::new(1., ACCENT));
            }
        }
        p.text(
            r.left_top() + vec2(18., 18.),
            Align2::LEFT_TOP,
            "3D  /  PERSPECTIVE",
            FontId::proportional(12.),
            Color32::from_rgb(91, 109, 106),
        );
        p.text(
            r.left_bottom() + vec2(18., -18.),
            Align2::LEFT_BOTTOM,
            if placing_3d_marquee { "3D Marquee · draw base; right-click closes polygon · depth from Info Box · Shift+middle-drag orbits" } else { "Shift + middle-drag to orbit · Right-drag to pan · Scroll to zoom" },
            FontId::proportional(10.),
            Color32::from_rgb(100, 115, 109),
        );
        let c = r.right_top() + vec2(-37., 45.);
        for (delta, color, label) in [
            (vec2(22., 8.), Color32::from_rgb(166, 90, 70), "X"),
            (vec2(-18., 12.), ACCENT, "Y"),
            (vec2(0., -22.), Color32::from_rgb(86, 115, 159), "Z"),
        ] {
            p.line_segment([c, c + delta], Stroke::new(1.5, color));
            p.text(
                c + delta * 1.3,
                Align2::CENTER_CENTER,
                label,
                FontId::monospace(10.),
                color,
            );
        }
        if response.hovered() {
            let feedback = if shift_middle_orbit
                || model_pan
                || (response.dragged_by(PointerButton::Primary) && !placing_3d_marquee)
            {
                CursorFeedback::Dragging
            } else if placing_3d_marquee {
                CursorFeedback::Drawing
            } else if self.editor.tool == Tool::Select && hovered_model_hit.is_some() {
                CursorFeedback::Selectable
            } else {
                CursorFeedback::Empty
            };
            ui.ctx().set_cursor_icon(feedback.icon());
        }
    }
}
