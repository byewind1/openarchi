use super::*;

impl AtelierApp {
    pub(super) fn model_view(&mut self, ui: &mut Ui) {
        let r = ui.available_rect_before_wrap();
        // Per-frame transfer cursor feedback; re-set below when active. The
        // bitmap cursor is sticky in egui output, so clear it every frame.
        self.transfer_hover = None;
        self.transfer_cursor = None;
        let response = ui.allocate_rect(r, Sense::click_and_drag());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "3D model canvas"));
        let pointer_drag_active = self.pointer_drag.is_some();
        if self.wall_insert.is_none()
            && !pointer_drag_active
            && self.editor.tool == Tool::Select
            && !self.trim_modifier_active(ui.ctx())
            && response.drag_started_by(PointerButton::Primary)
            && let Some(pos) = ui.input(|i| i.pointer.press_origin())
        {
            if let Some((origin, ids, z)) = self.selected_wall_junction_model(r, pos) {
                self.start_wall_junction(origin, ids, pos, Some(z), true);
            } else if let Some((id, None, point, z)) = self.selected_wall_model_edge(r, pos) {
                self.open_wall_edit(id, None, pos);
                self.set_wall_edit_hit(point, Some(z));
                self.start_wall_insert();
                if let Some(edit) = &mut self.wall_insert {
                    edit.canvas_drag = true;
                }
            }
        }
        let wall_insert_active = self.wall_insert.is_some();
        let over_wall_palette = ui.input(|i| i.pointer.hover_pos()).is_some_and(|pos| {
            self.wall_edit_palette_rect
                .is_some_and(|rect| rect.contains(pos))
        });
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
            && (!pointer_drag_active && response.drag_started_by(PointerButton::Primary))
            && let Some(pos) = ui.input(|i| i.pointer.press_origin())
        {
            self.marquee_3d_preview = Some((pos, pos));
        }
        if placing_3d_marquee
            && self.marquee_geometry == MarqueeGeometry::Rectangle
            && (!pointer_drag_active && response.dragged_by(PointerButton::Primary))
            && let (Some((start, _)), Some(pos)) =
                (self.marquee_3d_preview, response.interact_pointer_pos())
        {
            self.marquee_3d_preview = Some((start, pos));
        }
        if placing_3d_marquee
            && self.marquee_geometry == MarqueeGeometry::Rectangle
            && (!pointer_drag_active && response.drag_stopped())
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
            && (!pointer_drag_active && response.clicked_by(PointerButton::Primary))
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
            && (!pointer_drag_active && response.clicked_by(PointerButton::Secondary))
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
        let shift_middle_orbit = ui.input(|i| {
            i.modifiers.shift
                && i.pointer.button_down(PointerButton::Middle)
                && i.pointer.press_origin().is_some_and(|pos| r.contains(pos))
                && i.pointer.hover_pos().is_some_and(|pos| r.contains(pos))
        });
        let orbiting = (!wall_insert_active
            && !over_wall_palette
            && !pointer_drag_active
            && response.dragged_by(PointerButton::Primary)
            && !placing_3d_marquee
            && self.editor.tool != Tool::Marquee)
            || shift_middle_orbit;
        if orbiting {
            let d = ui.input(|i| i.pointer.delta());
            self.camera.yaw -= d.x * 0.009;
            // Cross the horizon without rolling; stay clear of the Z-up poles.
            self.camera.pitch = (self.camera.pitch + d.y * 0.007).clamp(-1.53, 1.53);
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
        // Parameter transfer cursor in 3D: armed one-shot modes, or holding
        // Option (+Cmd to inject) over the canvas, show the eyedropper or
        // syringe primed over the element the click would act on. The glyph
        // is painted after the model render; the OS cursor is hidden.
        let transfer_mode = if self.pick_armed {
            Some(crate::transfer_cursor::TransferCursor::PickUp)
        } else if self.inject_armed {
            Some(crate::transfer_cursor::TransferCursor::Inject)
        } else if response.hovered() && ui.input(|i| i.modifiers.alt) {
            Some(if ui.input(|i| i.modifiers.command) {
                crate::transfer_cursor::TransferCursor::Inject
            } else {
                crate::transfer_cursor::TransferCursor::PickUp
            })
        } else {
            None
        };
        let transfer_cursor_active = !wall_insert_active
            && !over_wall_palette
            && !orbiting
            && !pointer_drag_active
            && transfer_mode.is_some()
            && !model_pan
            && !middle_dragging
            && response.hovered();
        if let Some(kind) = transfer_mode.filter(|_| transfer_cursor_active) {
            let primed = self.transfer_hover_state(kind, hovered_model_hit);
            if let Some(pos) = ui
                .input(|i| i.pointer.hover_pos())
                .filter(|pos| r.contains(*pos))
            {
                self.transfer_cursor = Some((kind, primed, pos));
            }
            ui.ctx().set_cursor_icon(CursorIcon::None);
        }
        if (!wall_insert_active && !over_wall_palette && !pointer_drag_active && response.clicked())
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
            // Parameter transfer works in any view: Option picks up,
            // Option+Cmd injects, armed one-shot modes consume the click.
            let alt_click = ui.input(|i| i.modifiers.alt);
            if alt_click || self.pick_armed || self.inject_armed {
                let inject = if alt_click {
                    ui.input(|i| i.modifiers.command)
                } else {
                    self.inject_armed
                };
                self.pick_armed = false;
                self.inject_armed = false;
                self.execute_transfer(inject, hit);
            } else if self.editor.tool == Tool::Select
                && let Some((origin, ids, z)) = self.selected_wall_junction_model(r, pos)
            {
                self.start_wall_junction(origin, ids, pos, Some(z), false);
            } else if self.editor.tool == Tool::Select
                && let Some((id, None, point, z)) = self.selected_wall_model_edge(r, pos)
            {
                self.open_wall_edit(id, None, pos);
                self.set_wall_edit_hit(point, Some(z));
                self.start_wall_insert();
            } else {
                self.wall_edit = None;
                self.wall_edit_palette_rect = None;
                self.editor.set_selection(hit.into_iter().collect());
                self.draft_id = None;
            }
        }
        if let Some(renderer) = &mut self.renderer {
            let scale = ui.ctx().pixels_per_point();
            let selected = self.editor.selection_ids();
            let selection_only =
                self.show_marquee_3d && !selected.is_empty() && self.editor.marquee.is_none();
            renderer.render(
                &self.editor.project,
                &selected,
                self.transfer_hover,
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
        for wall in self.editor.project.elements.iter().filter(|e| {
            self.editor.selection_ids().contains(&e.id)
                && (self.all_stories || e.story == self.editor.story)
                && self
                    .editor
                    .project
                    .plan_options
                    .element_visible(e, &self.editor.project.layers)
        }) {
            if matches!(wall.shape, Shape::Wall { .. }) {
                let top = geometry::corners(&self.editor.project, wall)[4].z;
                let points = crate::wall_geometry::reference_path(wall)
                    .into_iter()
                    .filter_map(|point| self.wall_project_point(r, point, top))
                    .collect();
                Self::paint_selected_wall_path(&p, points);
            }
        }
        // The transfer tool glyph is the pointer while the mode is active.
        if let Some((kind, primed, pos)) = self.transfer_cursor {
            match kind {
                crate::transfer_cursor::TransferCursor::PickUp => {
                    crate::transfer_cursor::paint_pick_up(&p, pos, primed);
                }
                crate::transfer_cursor::TransferCursor::Inject => {
                    crate::transfer_cursor::paint_inject(&p, pos, primed);
                }
            }
        }
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
        if self.wall_insert_canvas(ui, r, &response, true) {
            return;
        }
        self.wall_edit_palette(ui.ctx());
        self.pointer_drag_canvas(ui, r, &response, true);
        if !pointer_drag_active && response.hovered() && self.transfer_cursor.is_none() {
            let feedback = if shift_middle_orbit
                || model_pan
                || ((!pointer_drag_active && response.dragged_by(PointerButton::Primary))
                    && !placing_3d_marquee)
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
        // Paint last so the navigation cursor wins over generic hover/drag feedback.
        if orbiting
            && let Some(pos) = ui
                .input(|i| i.pointer.hover_pos())
                .filter(|p| r.contains(*p))
        {
            ui.ctx().set_cursor_icon(CursorIcon::None);
            crate::transfer_cursor::paint_orbit(&p, pos);
        }
    }
}
