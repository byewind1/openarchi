//! A read-only view over current candidate IDs, never an owned Project.
use super::*;
use crate::design_process::Comparison;

pub(super) struct Comparison3d {
    key: Option<(u64, String)>,
    pub camera: Camera,
    fit: bool,
    renderers: Option<[SceneRenderer; 2]>,
}
impl Comparison3d {
    pub(super) fn release(&mut self) {
        self.renderers = None;
    }
}
impl Default for Comparison3d {
    fn default() -> Self {
        Self {
            key: None,
            camera: Camera::default(),
            fit: true,
            renderers: None,
        }
    }
}
impl AtelierApp {
    pub(super) fn design_comparison_3d(&mut self, ui: &mut Ui, comparison: &Comparison) {
        let language = self.language;
        let mut state = std::mem::take(&mut self.process_ui.comparison_3d);
        let key = (
            self.host.document.editor.document_session(),
            comparison.task_id.clone(),
        );
        if state.key.as_ref() != Some(&key) {
            state.key = Some(key);
            state.camera = self.camera.clone();
            state.fit = true;
        }
        ui.label(
            language
                .translate("Same perspective camera · same materials · candidate geometry only"),
        );
        ui.small(language.translate("Read-only 3D comparison; edit the shared model on the canvas. Not an environmental or compliance result."));
        if ui
            .button(language.translate("Fit both candidates"))
            .clicked()
        {
            state.fit = true;
        }
        ui.small(
            language.translate("Shift + middle-drag to orbit · Right-drag to pan · Scroll to zoom"),
        );
        let available = ui.available_width();
        let side_by_side = available >= 420.;
        let width = if side_by_side {
            (available - ui.spacing().item_spacing.x) / 2.
        } else {
            available
        }
        .max(20.);
        let font = egui::TextStyle::Body.resolve(ui.style());
        let header_height = comparison
            .candidates
            .iter()
            .map(|c| {
                ui.painter()
                    .layout(c.name.clone(), font.clone(), INK, width)
                    .size()
                    .y
            })
            .fold(0_f32, f32::max);
        let mut views = Vec::with_capacity(2);
        let mut allocate = |ui: &mut Ui, index: usize| {
            ui.add_sized(
                egui::vec2(width, header_height),
                egui::Label::new(&comparison.candidates[index].name).wrap(),
            );
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width, 220.), Sense::click_and_drag());
            response.widget_info(|| {
                WidgetInfo::labeled(
                    WidgetType::Other,
                    true,
                    format!(
                        "{} · {}",
                        language.translate("3D candidate comparison"),
                        comparison.candidates[index].name
                    ),
                )
            });
            views.push((rect, response));
        };
        if side_by_side {
            ui.columns(2, |columns| {
                allocate(&mut columns[0], 0);
                allocate(&mut columns[1], 1);
            });
        } else {
            allocate(ui, 0);
            allocate(ui, 1);
        }
        // Collect input from both views before submitting either one: a gesture
        // in B must never leave A one frame behind with a different matrix.
        for (rect, response) in &views {
            let middle = ui.input(|i| {
                i.pointer.button_down(PointerButton::Middle)
                    && i.pointer.hover_pos().is_some_and(|p| rect.contains(p))
            });
            let shift_middle = middle
                && ui.input(|i| {
                    i.modifiers.shift && i.pointer.press_origin().is_some_and(|p| rect.contains(p))
                });
            let delta = ui.input(|i| i.pointer.delta());
            if shift_middle {
                state.camera.orbit_pixels(delta.x.into(), delta.y.into());
            }
            if response.dragged_by(PointerButton::Secondary)
                || (middle && !ui.input(|i| i.modifiers.shift))
            {
                state.camera.pan_pixels(
                    delta.x.into(),
                    delta.y.into(),
                    rect.width().into(),
                    rect.height().into(),
                );
            }
            if response.hovered() {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                state.camera.zoom_scroll(scroll.into());
                // The surrounding comparison window must not scroll while the
                // pointer is zooming one of these two synchronized viewports.
                ui.input_mut(|i| {
                    i.smooth_scroll_delta = egui::Vec2::ZERO;
                });
            }
        }
        if state.renderers.is_none()
            && let Some(renderer) = &self.renderer
        {
            state.renderers = Some([renderer.sibling(), renderer.sibling()]);
        }
        let ids = comparison.candidates.each_ref().map(|v| {
            v.elements
                .iter()
                .filter(|e| {
                    geometry::model_element_layer_visible(&self.host.document.editor.project, e)
                })
                .map(|e| e.id)
                .collect::<Vec<_>>()
        });
        let editor = &self.host.document.editor;
        let current = comparison.candidates.iter().all(|v| {
            !v.elements.is_empty()
                && v.elements.len() <= 32
                && v.elements.windows(2).all(|w| w[0].id < w[1].id)
                && v.elements
                    .iter()
                    .all(|e| editor.project.elements.iter().any(|actual| actual == e))
        });
        let result = if current {
            editor
                .with_geometry_read(|read| {
                    if state.fit {
                        let mut lo = glam::DVec3::splat(f64::INFINITY);
                        let mut hi = glam::DVec3::splat(f64::NEG_INFINITY);
                        for id in ids.iter().flatten() {
                            let handle = read.source(*id)?;
                            let bounds = handle.source.display_bounds().ok_or_else(|| {
                                crate::derived_geometry::GeometryDiagnostic::error(
                                    *id,
                                    "COMPARISON_BOUNDS",
                                    "comparison",
                                    "Comparison geometry has no bounds",
                                )
                            })?;
                            lo = lo.min(bounds[0]);
                            hi = hi.max(bounds[1]);
                        }
                        state.camera.target = (lo + hi) * 0.5;
                        state.camera.span =
                            ((hi - lo).length() / 22.5_f64.to_radians().cos() * 1.1).max(8.);
                        state.fit = false;
                    }
                    if let Some(renderers) = &mut state.renderers {
                        let pixels = ui.ctx().pixels_per_point();
                        for (index, (rect, _)) in views.iter().enumerate() {
                            renderers[index].render_comparison(
                                &editor.project,
                                &ids[index],
                                &editor.selection_ids(),
                                &state.camera,
                                [
                                    (rect.width() * pixels).round() as u32,
                                    (rect.height() * pixels).round() as u32,
                                ],
                                read,
                            );
                        }
                    }
                    Ok::<_, crate::derived_geometry::GeometryDiagnostic>(())
                })
                .and_then(|v| v)
        } else {
            Err(crate::derived_geometry::GeometryDiagnostic::error(
                0,
                "COMPARISON_STALE",
                "comparison",
                "Comparison objects changed; reopen the current comparison",
            ))
        };
        let mut projected_paths = Vec::new();
        for (index, (rect, response)) in views.iter().enumerate() {
            let owner = Id::new(("comparison_selection", comparison.task_id.as_str(), index));
            let mut paths = if current {
                self.host
                    .document
                    .editor
                    .candidate_selection_paths(
                        &state.camera,
                        rect.aspect_ratio().into(),
                        &ids[index],
                    )
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            for path in &mut paths {
                for p in &mut path.points {
                    p.x = f64::from(rect.left()) + (p.x + 1.) * 0.5 * f64::from(rect.width());
                    p.y = f64::from(rect.top()) + (1. - p.y) * 0.5 * f64::from(rect.height());
                }
            }
            let scope =
                self.canvas_selection_scope(&paths.iter().map(|p| p.id).collect::<Vec<_>>());
            let hits = if current {
                ui.input(|i| i.pointer.hover_pos())
                    .filter(|p| rect.contains(*p))
                    .and_then(|p| {
                        self.host
                            .document
                            .editor
                            .candidate_pick_candidates(
                                &state.camera,
                                rect.aspect_ratio().into(),
                                [
                                    f64::from((p.x - rect.left()) / rect.width() * 2. - 1.),
                                    f64::from(1. - (p.y - rect.top()) / rect.height() * 2.),
                                ],
                                &ids[index],
                                self.canvas_surface_selection(ui),
                                12. / f64::from(rect.width().min(rect.height()).max(1.)),
                            )
                            .ok()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let consumed =
                self.canvas_candidate_input(ui, *rect, response, owner, hits, &scope, current);
            let allowed =
                current && !self.canvas_keyboard_blocked(ui) && self.canvas_selection_context(ui);
            if let Some(b) = self.selection_drag(ui, *rect, response, owner, allowed) {
                let rectangle = geometry::rectangle_polygon(
                    Point::new(b.start.x.into(), b.start.y.into()),
                    Point::new(b.end.x.into(), b.end.y.into()),
                );
                self.apply_scoped_canvas_selection(
                    crate::screen_selection::select_paths(&paths, &rectangle, b.end.x >= b.start.x),
                    ui.input(|i| i.modifiers.shift),
                    &scope,
                );
                ui.ctx().request_repaint();
            }
            if allowed && !consumed && response.clicked_by(PointerButton::Primary) {
                self.commit_canvas_candidate(ui, owner, &scope);
                ui.ctx().request_repaint();
            }
            projected_paths.push(paths);
        }
        for (index, (rect, _)) in views.iter().enumerate() {
            if let Err(d) = &result {
                ui.painter().rect_filled(*rect, 0., PANEL_ALT);
                Self::show_geometry_diagnostics(ui, *rect, std::slice::from_ref(d), language);
            } else if let Some(renderers) = &state.renderers {
                let renderer = &renderers[index];
                ui.painter().image(
                    renderer.texture,
                    *rect,
                    Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                    Color32::WHITE,
                );
                Self::show_geometry_diagnostics(ui, *rect, &renderer.diagnostics, language);
            } else {
                ui.painter().rect_filled(*rect, 0., PANEL_ALT);
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    language.translate("3D GPU preview unavailable"),
                    FontId::proportional(12.),
                    INK,
                );
            }
        }
        for (index, (rect, _)) in views.iter().enumerate() {
            self.paint_canvas_candidate(
                ui,
                *rect,
                Id::new(("comparison_selection", comparison.task_id.as_str(), index)),
                &projected_paths[index],
            );
            self.paint_selection_box(
                &ui.painter_at(*rect),
                Id::new(("comparison_selection", comparison.task_id.as_str(), index)),
            );
        }
        ui.small(format!(
            "{}: X {:.3} · Y {:.3} · Z {:.3} m",
            language.translate("Shared camera target"),
            state.camera.target.x,
            state.camera.target.y,
            state.camera.target.z
        ));
        #[cfg(test)]
        ui.ctx().data_mut(|d| {
            d.insert_temp(
                Id::new("comparison_3d_views"),
                (
                    views.iter().map(|v| v.0).collect::<Vec<_>>(),
                    state.camera.clone(),
                ),
            )
        });
        self.process_ui.comparison_3d = state;
    }
}
