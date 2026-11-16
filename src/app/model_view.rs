use super::*;

impl AtelierApp {
    pub(super) fn show_geometry_diagnostics(
        ui: &mut Ui,
        r: Rect,
        diagnostics: &[crate::derived_geometry::GeometryDiagnostic],
        language: UiLanguage,
    ) {
        if let Some(diagnostic) = diagnostics.first() {
            let message = language
                .translate("Geometry display errors: {count}")
                .replace("{count}", &diagnostics.len().to_string());
            let reason = language.geometry_diagnostic_message(diagnostic);
            ui.scope_builder(egui::UiBuilder::new().max_rect(r.shrink(8.)), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width((r.width() - 32.).clamp(80., 480.));
                    ui.label(egui::RichText::new(message).color(ui.visuals().error_fg_color));
                    ui.label(reason);
                });
            });
        }
    }

    pub(super) fn model_view_filter<'a>(
        &'a self,
        selected: &'a [u64],
    ) -> geometry::ModelViewFilter<'a> {
        geometry::ModelViewFilter {
            story: if self.all_stories {
                None
            } else {
                Some(self.host.document.editor.story)
            },
            selected,
            selection_only: self.show_marquee_3d
                && !selected.is_empty()
                && self.host.document.editor.marquee.is_none(),
            marquee: if self.show_marquee_3d {
                self.host.document.editor.marquee.as_ref()
            } else {
                None
            },
            outside_marquee: self.marquee_outside,
        }
    }
    pub(super) fn model_view(&mut self, ui: &mut Ui) {
        let r = ui.available_rect_before_wrap();
        // Per-frame transfer cursor feedback; re-set below when active. The
        // bitmap cursor is sticky in egui output, so clear it every frame.
        self.transfer_hover = None;
        self.transfer_cursor = None;
        let response = ui.allocate_rect(r, Sense::click_and_drag());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "3D model canvas"));
        let selection_owner = Id::new("model_canvas_selection");
        let mass_controls = self.mass_model_parameters(ui, r);
        let over_mass_controls = ui
            .input(|i| i.pointer.hover_pos())
            .is_some_and(|p| mass_controls.is_some_and(|r| r.contains(p)));
        let strict_selection =
            self.canvas_pick_strict(ui, selection_owner) || self.canvas_keyboard_blocked(ui);
        let mass_push_pull_busy = if strict_selection && !self.mass_push_pull_active() {
            false
        } else {
            self.mass_push_pull_input(ui, r, &response, over_mass_controls)
        };
        let navigating = !mass_push_pull_busy
            && !over_mass_controls
            && self.navigation_canvas(ui, r, &response, viewport_navigation::Pane::Model);
        let mut candidate_paths = Vec::new();
        let (
            pointer_drag_active,
            placing_3d_marquee,
            hovered_model_hit,
            shift_middle_orbit,
            model_pan,
            orbiting,
        ) = if navigating {
            (false, false, None, false, false, false)
        } else {
            let opening_edit_active = if strict_selection && self.opening_resize.is_none() {
                false
            } else {
                self.opening_model_input(ui, r, &response)
            };
            let pointer_drag_active =
                self.pointer_drag.is_some() || opening_edit_active || mass_push_pull_busy;
            if !over_mass_controls
                && self.wall_insert.is_none()
                && !pointer_drag_active
                && self.host.document.editor.tool == Tool::Select
                && !ui.input(|i| i.modifiers.shift || i.modifiers.alt)
                && !self.canvas_pick_strict(ui, selection_owner)
                && !self.trim_modifier_active(ui.ctx())
                && response.drag_started_by(PointerButton::Primary)
                && let Some(pos) = ui.input(|i| i.pointer.press_origin())
            {
                if let Some(hit) = self.selected_mass_model_hit(r, pos) {
                    self.start_mass_model_pet(hit, pos, true);
                } else if let Some((origin, ids, z)) = self.selected_wall_junction_model(r, pos) {
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
            let placing_3d_marquee = self.host.document.editor.tool == Tool::Marquee;
            let base_z = self
                .host
                .document
                .editor
                .project
                .elevation(self.host.document.editor.story);
            let hovered_model_hit = response.interact_pointer_pos().and_then(|pos| {
                self.host
                    .document
                    .editor
                    .pick_model(
                        &self.camera,
                        [
                            ((pos.x - r.left()) / r.width() * 2. - 1.).into(),
                            f64::from(1. - (pos.y - r.top()) / r.height() * 2.),
                        ],
                        r.aspect_ratio().into(),
                        self.model_view_filter(&self.host.document.editor.selection_ids()),
                    )
                    .unwrap_or_else(|diagnostic| {
                        self.host.document.editor.status =
                            crate::geometry_validation::compatibility_message(&diagnostic);
                        None
                    })
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
                    self.host.document.editor.status =
                        "Marquee base must be below the perspective horizon".into();
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
                                self.host.document.editor.status =
                                    "Rotated rectangle needs non-zero sides".into();
                            }
                        }
                        _ => {}
                    }
                } else {
                    self.host.document.editor.status =
                        "Marquee base must be below the perspective horizon".into();
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
                    self.host.document.editor.status =
                        "3D Marquee polygon needs at least three points".into();
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
            let orbiting = shift_middle_orbit && !opening_edit_active;
            let box_allowed = !self.canvas_keyboard_blocked(ui)
                && !self.host.document.editor.model_input_active()
                && !over_mass_controls
                && !wall_insert_active
                && !over_wall_palette
                && !pointer_drag_active
                && !placing_3d_marquee
                && !orbiting
                && !self.pick_armed
                && !self.inject_armed
                && !ui.input(|i| i.modifiers.alt);
            let selection_owner = Id::new("model_canvas_selection");
            if let Some(b) = self.selection_drag(ui, r, &response, selection_owner, box_allowed) {
                let ndc = |p: Pos2| {
                    Point::new(
                        f64::from((p.x - r.left()) / r.width() * 2. - 1.),
                        f64::from(1. - (p.y - r.top()) / r.height() * 2.),
                    )
                };
                let selected = self.host.document.editor.selection_ids();
                match self.host.document.editor.model_box_selection(
                    &self.camera,
                    r.aspect_ratio().into(),
                    ndc(b.start),
                    ndc(b.end),
                    b.end.x >= b.start.x,
                    self.model_view_filter(&selected),
                ) {
                    Ok(ids) => {
                        let visible = self
                            .host
                            .document
                            .editor
                            .model_selection_paths(
                                &self.camera,
                                r.aspect_ratio().into(),
                                self.model_view_filter(&selected),
                            )
                            .unwrap_or_default();
                        let scope = self.canvas_selection_scope(
                            &visible.iter().map(|p| p.id).collect::<Vec<_>>(),
                        );
                        self.apply_scoped_canvas_selection(
                            ids,
                            ui.input(|i| i.modifiers.shift),
                            &scope,
                        );
                    }
                    Err(d) => {
                        self.host.document.editor.status =
                            crate::geometry_validation::compatibility_message(&d)
                    }
                }
            }
            if orbiting {
                let d = ui.input(|i| i.pointer.delta());
                self.camera.orbit_pixels(d.x.into(), d.y.into());
            }
            let model_pan = !opening_edit_active
                && (response.dragged_by(PointerButton::Secondary)
                    || (middle_dragging && !ui.input(|i| i.modifiers.shift)));
            if model_pan {
                let delta = ui.input(|i| i.pointer.delta());
                self.camera.pan_pixels(
                    delta.x.into(),
                    delta.y.into(),
                    r.width().into(),
                    r.height().into(),
                );
            }
            if response.hovered() && !opening_edit_active {
                let scroll = self.canvas_scroll_delta(ui);
                self.camera.zoom_scroll(scroll.into());
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
            let transfer_cursor_active = !over_mass_controls
                && !wall_insert_active
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
            let selected = self.host.document.editor.selection_ids();
            let candidate_enabled = !over_mass_controls
                && !wall_insert_active
                && !over_wall_palette
                && !pointer_drag_active
                && !orbiting
                && !model_pan
                && !placing_3d_marquee
                && !transfer_cursor_active
                && !self.trim_modifier_active(ui.ctx());
            if candidate_enabled && (response.hovered() || response.is_pointer_button_down_on()) {
                candidate_paths = self
                    .host
                    .document
                    .editor
                    .model_selection_paths(
                        &self.camera,
                        r.aspect_ratio().into(),
                        self.model_view_filter(&selected),
                    )
                    .unwrap_or_default();
                for path in &mut candidate_paths {
                    for q in &mut path.points {
                        q.x = f64::from(r.left()) + (q.x + 1.) * 0.5 * f64::from(r.width());
                        q.y = f64::from(r.top()) + (1. - q.y) * 0.5 * f64::from(r.height());
                    }
                }
            }
            let candidate_scope = self
                .canvas_selection_scope(&candidate_paths.iter().map(|p| p.id).collect::<Vec<_>>());
            let hits = if candidate_enabled && self.canvas_selection_context(ui) {
                ui.input(|i| i.pointer.hover_pos())
                    .filter(|p| r.contains(*p))
                    .and_then(|pos| {
                        self.host
                            .document
                            .editor
                            .model_pick_candidates(
                                &self.camera,
                                [
                                    f64::from((pos.x - r.left()) / r.width() * 2. - 1.),
                                    f64::from(1. - (pos.y - r.top()) / r.height() * 2.),
                                ],
                                r.aspect_ratio().into(),
                                self.model_view_filter(&selected),
                                self.canvas_surface_selection(ui),
                                12. / f64::from(r.width().min(r.height()).max(1.)),
                            )
                            .ok()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let selection_consumed = self.canvas_candidate_input(
                ui,
                r,
                &response,
                selection_owner,
                hits,
                &candidate_scope,
                candidate_enabled,
            );
            if (!over_mass_controls
                && !wall_insert_active
                && !over_wall_palette
                && !pointer_drag_active
                && response.clicked_by(PointerButton::Primary))
                && !selection_consumed
                && (transfer_cursor_active || self.canvas_selection_context(ui))
                && !self.canvas_keyboard_blocked(ui)
                && self.host.document.editor.tool != Tool::Marquee
                && let Some(pos) = response.interact_pointer_pos()
            {
                let hit = self
                    .host
                    .document
                    .editor
                    .pick_model(
                        &self.camera,
                        [
                            ((pos.x - r.left()) / r.width() * 2. - 1.).into(),
                            f64::from(1. - (pos.y - r.top()) / r.height() * 2.),
                        ],
                        r.aspect_ratio().into(),
                        self.model_view_filter(&self.host.document.editor.selection_ids()),
                    )
                    .unwrap_or_else(|diagnostic| {
                        self.host.document.editor.status =
                            crate::geometry_validation::compatibility_message(&diagnostic);
                        None
                    });
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
                } else if self.host.document.editor.tool == Tool::Select
                    && !ui.input(|i| i.modifiers.shift)
                    && let Some(hit) = self.selected_mass_model_hit(r, pos)
                {
                    self.start_mass_model_pet(hit, pos, false);
                } else if self.host.document.editor.tool == Tool::Select
                    && !ui.input(|i| i.modifiers.shift)
                    && let Some((origin, ids, z)) = self.selected_wall_junction_model(r, pos)
                {
                    self.start_wall_junction(origin, ids, pos, Some(z), false);
                } else if self.host.document.editor.tool == Tool::Select
                    && !ui.input(|i| i.modifiers.shift)
                    && let Some((id, None, point, z)) = self.selected_wall_model_edge(r, pos)
                {
                    self.open_wall_edit(id, None, pos);
                    self.set_wall_edit_hit(point, Some(z));
                    self.start_wall_insert();
                } else {
                    self.wall_edit = None;
                    self.wall_edit_palette_rect = None;
                    self.commit_canvas_candidate(ui, selection_owner, &candidate_scope);
                }
            }
            (
                pointer_drag_active,
                placing_3d_marquee,
                hovered_model_hit,
                shift_middle_orbit,
                model_pan,
                orbiting,
            )
        };
        let opening_preview = self
            .opening_preview_project()
            .or_else(|| self.mass_push_pull_preview())
            .or_else(|| self.mass_parameters_preview());
        let model_hover = self
            .transfer_hover
            .or_else(|| self.canvas_hover_id(selection_owner));
        if let Some(renderer) = &mut self.renderer {
            let scale = ui.ctx().pixels_per_point();
            let selected = self.host.document.editor.selection_ids();
            let selection_only = self.show_marquee_3d
                && !selected.is_empty()
                && self.host.document.editor.marquee.is_none();
            if let Some(project) = opening_preview.as_ref() {
                renderer.render(
                    project,
                    &selected,
                    model_hover,
                    &self.camera,
                    [(r.width() * scale) as u32, (r.height() * scale) as u32],
                    if self.all_stories {
                        None
                    } else {
                        Some(self.host.document.editor.story)
                    },
                    if self.show_marquee_3d && !selection_only {
                        self.host.document.editor.marquee.as_ref()
                    } else {
                        None
                    },
                    self.marquee_outside,
                    selection_only,
                );
            } else {
                let result = self.host.document.editor.with_geometry_read(|read| {
                    renderer.render_cached(
                        &self.host.document.editor.project,
                        &selected,
                        model_hover,
                        &self.camera,
                        [(r.width() * scale) as u32, (r.height() * scale) as u32],
                        if self.all_stories {
                            None
                        } else {
                            Some(self.host.document.editor.story)
                        },
                        if self.show_marquee_3d && !selection_only {
                            self.host.document.editor.marquee.as_ref()
                        } else {
                            None
                        },
                        self.marquee_outside,
                        selection_only,
                        read,
                    );
                });
                if let Err(diagnostic) = result {
                    renderer.render(
                        &self.host.document.editor.project,
                        &selected,
                        model_hover,
                        &self.camera,
                        [(r.width() * scale) as u32, (r.height() * scale) as u32],
                        if self.all_stories {
                            None
                        } else {
                            Some(self.host.document.editor.story)
                        },
                        if self.show_marquee_3d && !selection_only {
                            self.host.document.editor.marquee.as_ref()
                        } else {
                            None
                        },
                        self.marquee_outside,
                        selection_only,
                    );
                    renderer.diagnostics.insert(0, diagnostic);
                }
            }
            ui.painter().image(
                renderer.texture,
                r,
                Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                Color32::WHITE,
            );
            Self::show_geometry_diagnostics(ui, r, &renderer.diagnostics, self.language);
        } else {
            ui.painter().rect_filled(r, 0., LINE);
        }
        let p = ui.painter_at(r);
        self.paint_canvas_candidate(ui, r, selection_owner, &candidate_paths);
        self.paint_opening_handles(ui, r);
        self.paint_mass_model_handles(ui, r);
        let selected = self.host.document.editor.selection_ids();
        let filter = self.model_view_filter(&selected);
        for wall in self
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .filter(|e| {
                selected.contains(&e.id)
                    && filter.includes(&self.host.document.editor.project, e)
                    && self
                        .host
                        .document
                        .editor
                        .project
                        .plan_options
                        .element_visible(e, &self.host.document.editor.project.layers)
            })
        {
            if matches!(wall.shape, Shape::Wall { .. }) {
                let Some((_, top)) =
                    crate::wall_solid::vertical_range(&self.host.document.editor.project, wall)
                else {
                    continue;
                };
                let path = crate::wall_geometry::reference_path(wall);
                if filter.clip_area().is_none() {
                    let points = path
                        .into_iter()
                        .filter_map(|point| self.wall_project_point(r, point, top))
                        .collect();
                    Self::paint_selected_wall_path(&p, points);
                } else {
                    for pair in path.windows(2) {
                        for [a, b] in filter.reference_segments(pair[0], pair[1], top) {
                            if let (Some(a), Some(b)) = (
                                self.wall_project_point(r, a, top),
                                self.wall_project_point(r, b, top),
                            ) {
                                p.line_segment(
                                    [a, b],
                                    Stroke::new(WALL_REFERENCE_LINE_WIDTH, WALL_REFERENCE_LINE),
                                );
                            }
                        }
                    }
                    for point in path.first().into_iter().chain(path.last()) {
                        if filter.contains_anchor(*point, top)
                            && let Some(handle) = self.wall_project_point(r, *point, top)
                        {
                            p.rect_filled(
                                Rect::from_center_size(
                                    handle,
                                    Vec2::splat(WALL_REFERENCE_HANDLE_SIZE),
                                ),
                                0.,
                                Color32::BLACK,
                            );
                        }
                    }
                }
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
            && let Some(area) = &self.host.document.editor.marquee
        {
            let (bottom, top) = area.z_range.unwrap_or_else(|| {
                let mut lo = f64::INFINITY;
                let mut hi = f64::NEG_INFINITY;
                for element in &self.host.document.editor.project.elements {
                    for corner in geometry::corners(&self.host.document.editor.project, element) {
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
            let matrix = self.camera.matrix(r.aspect_ratio().into());
            let screen_point = |point: Point, z: f64| {
                let ndc = matrix.project_point3(glam::DVec3::new(point.x, point.y, z));
                ndc.is_finite().then(|| {
                    pos2(
                        f64::from(r.left()) + (ndc.x + 1.) * 0.5 * f64::from(r.width()),
                        f64::from(r.top()) + (1. - ndc.y) * 0.5 * f64::from(r.height()),
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
            self.language.translate("3D  /  PERSPECTIVE"),
            FontId::proportional(12.),
            Color32::from_rgb(91, 109, 106),
        );
        p.text(
            r.left_bottom() + vec2(18., -18.),
            Align2::LEFT_BOTTOM,
            self.language.translate(if placing_3d_marquee {
                "3D Marquee · draw base; right-click closes polygon · depth from Info Box · Shift+middle-drag orbits"
            } else {
                "Shift + middle-drag to orbit · Right-drag to pan · Scroll to zoom"
            }),
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
        if navigating {
            self.navigation_overlay(ui, r, viewport_navigation::Pane::Model);
            return;
        }
        if self.wall_insert_canvas(ui, r, &response, true) {
            return;
        }
        self.wall_edit_palette(ui.ctx(), true);
        if !over_mass_controls && !mass_push_pull_busy {
            self.pointer_drag_canvas(ui, r, &response, true);
        }
        if !over_mass_controls
            && !pointer_drag_active
            && response.hovered()
            && self.transfer_cursor.is_none()
        {
            let feedback = if shift_middle_orbit || model_pan {
                CursorFeedback::Dragging
            } else if placing_3d_marquee {
                CursorFeedback::Drawing
            } else if self.host.document.editor.tool == Tool::Select && hovered_model_hit.is_some()
            {
                CursorFeedback::Selectable
            } else {
                CursorFeedback::Empty
            };
            ui.ctx().set_cursor_icon(feedback.icon());
        }
        self.paint_mass_push_pull(ui, r);
        // Paint last so the navigation cursor wins over generic hover/drag feedback.
        self.paint_selection_box(&p, selection_owner);
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
