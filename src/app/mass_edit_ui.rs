//! Concept-envelope adapters to the shared contour editor and property service.
//! The project remains authoritative; this module owns only a parameter draft.
use super::*;

#[derive(Clone)]
pub(super) struct MassParameters {
    source: Element,
    draft: Element,
    popup_at: Pos2,
    popup_rect: Option<Rect>,
    focused: bool,
}
impl AtelierApp {
    pub(super) fn mass_parameter_draft_active(&self) -> bool {
        self.mass_push_pull_active()
            || self
                .mass_parameters
                .as_ref()
                .is_some_and(|s| s.draft != s.source)
    }
    pub(super) fn selected_mass_model_hit(
        &self,
        r: Rect,
        pos: Pos2,
    ) -> Option<(u64, Option<usize>, Option<usize>, Point, f64)> {
        let ids = self.host.document.editor.selection_ids();
        let filter = self.model_view_filter(&ids);
        let mut nodes = vec![];
        let mut edges = vec![];
        for element in self
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .filter(|e| {
                ids.contains(&e.id)
                    && !self.host.document.editor.locked_element(e.id)
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
            let Shape::ConceptMass { spec } = &element.shape else {
                continue;
            };
            let base =
                self.host.document.editor.project.elevation(element.story) + spec.bottom_offset;
            for z in [base + spec.height, base] {
                let mut index = 0;
                for ring in crate::slab_geometry::rings(element) {
                    for (i, point) in ring.points.iter().enumerate() {
                        if filter.contains_anchor(*point, z)
                            && let Some(at) = self.wall_project_point(r, *point, z)
                            && pos.distance(at) <= 8.
                        {
                            nodes.push((pos.distance(at), element.id, index + i, *point, z));
                        }
                        let end = ring.points[(i + 1) % ring.points.len()];
                        let path = ring
                            .arcs
                            .get(i)
                            .and_then(|a| *a)
                            .and_then(|arc| geometry::arc_points(*point, end, arc))
                            .unwrap_or_else(|| vec![*point, end]);
                        for pair in path.windows(2) {
                            for [a, b] in filter.reference_segments(pair[0], pair[1], z) {
                                let (Some(pa), Some(pb)) = (
                                    self.wall_project_point(r, a, z),
                                    self.wall_project_point(r, b, z),
                                ) else {
                                    continue;
                                };
                                let (d, t) = crate::reshape::distance_segment(
                                    Point::new(pos.x.into(), pos.y.into()),
                                    Point::new(pa.x.into(), pa.y.into()),
                                    Point::new(pb.x.into(), pb.y.into()),
                                );
                                if d <= 8. {
                                    let q = Self::screen_to_ground(&self.camera, r, pos, z)
                                        .unwrap_or(Point::new(
                                            a.x + (b.x - a.x) * t,
                                            a.y + (b.y - a.y) * t,
                                        ));
                                    edges.push((d, element.id, index + i, q, z));
                                }
                            }
                        }
                    }
                    index += ring.points.len();
                }
            }
        }
        if let Some((_, id, index, q, z)) = nodes.into_iter().min_by(|a, b| a.0.total_cmp(&b.0)) {
            return Some((id, Some(index), None, q, z));
        }
        edges
            .into_iter()
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id, index, q, z)| (id, None, Some(index), q, z))
    }
    pub(super) fn start_mass_model_pet(
        &mut self,
        hit: (u64, Option<usize>, Option<usize>, Point, f64),
        at: Pos2,
        dragging: bool,
    ) {
        let (id, node, edge, q, z) = hit;
        self.start_polyline_pet(id, node, edge, at, q, false);
        if let Some(edit) = &mut self.wall_insert {
            edit.set_model_plane(z, dragging);
        }
    }
    pub(super) fn paint_mass_model_handles(&self, ui: &Ui, r: Rect) {
        let ids = self.host.document.editor.selection_ids();
        let filter = self.model_view_filter(&ids);
        let painter = ui.painter_at(r);
        for e in self
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .filter(|e| {
                ids.contains(&e.id)
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
            let Shape::ConceptMass { spec } = &e.shape else {
                continue;
            };
            let base = self.host.document.editor.project.elevation(e.story) + spec.bottom_offset;
            for z in [base, base + spec.height] {
                for ring in crate::slab_geometry::rings(e) {
                    let path = crate::slab_geometry::ring_path(&ring);
                    for i in 0..path.len() {
                        for [a, b] in
                            filter.reference_segments(path[i], path[(i + 1) % path.len()], z)
                        {
                            if let (Some(a), Some(b)) = (
                                self.wall_project_point(r, a, z),
                                self.wall_project_point(r, b, z),
                            ) {
                                painter.line_segment([a, b], Stroke::new(1.5, ACCENT));
                            }
                        }
                    }
                    for q in ring.points {
                        if filter.contains_anchor(q, z)
                            && let Some(at) = self.wall_project_point(r, q, z)
                        {
                            painter.rect_filled(
                                Rect::from_center_size(at, Vec2::splat(6.)),
                                0.,
                                ACCENT,
                            );
                        }
                    }
                }
            }
        }
    }
    pub(super) fn mass_parameters_preview(&self) -> Option<Project> {
        let state = self.mass_parameters.as_ref()?;
        if state.draft == state.source {
            return None;
        }
        let Shape::ConceptMass { spec } = &state.draft.shape else {
            return None;
        };
        spec.validate().ok()?;
        let mut project = self.host.document.editor.project.clone();
        let e = project.elements.iter_mut().find(|e| **e == state.source)?;
        *e = state.draft.clone();
        Some(project)
    }
    /// Returns the actual overlay rectangle so canvas selection/navigation cannot
    /// consume its mouse events. Drafts never enter the project before Apply.
    pub(super) fn mass_model_parameters(&mut self, ui: &mut Ui, r: Rect) -> Option<Rect> {
        if self.mass_push_pull_active() {
            return None;
        }
        let ids = self.host.document.editor.selection_ids();
        let source = (ids.len() == 1
            && self.wall_insert.is_none()
            && self.host.document.editor.tool == Tool::Select)
            .then(|| {
                self.host
                    .document
                    .editor
                    .project
                    .elements
                    .iter()
                    .find(|e| e.id == ids[0] && matches!(e.shape, Shape::ConceptMass { .. }))
                    .cloned()
            })
            .flatten();
        let Some(source) = source else {
            self.mass_parameters = None;
            return None;
        };
        if !self
            .model_view_filter(&ids)
            .includes(&self.host.document.editor.project, &source)
            || !self
                .host
                .document
                .editor
                .project
                .plan_options
                .element_visible(&source, &self.host.document.editor.project.layers)
        {
            self.mass_parameters = None;
            return None;
        }
        if self.host.document.editor.locked_element(source.id) || !ui.input(|i| i.focused) {
            self.mass_parameters = None;
            return None;
        }
        let pointer = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| r.contains(*p));
        let hovering_face = pointer
            .and_then(|p| self.mass_face_pick(r, p))
            .is_some_and(|(e, _, _)| e.id == source.id);
        let retained = self.mass_parameters.as_ref().filter(|s| s.source == source);
        let reaching_popup = retained.is_some_and(|s| {
            pointer.is_some_and(|p| {
                s.popup_rect.is_some_and(|rect| {
                    rect.expand(8.).contains(p)
                        || Rect::from_two_pos(s.popup_at - vec2(22., 20.), rect.center())
                            .expand(12.)
                            .contains(p)
                })
            })
        });
        let editing = retained.is_some_and(|s| s.draft != s.source || s.focused);
        if !hovering_face && !reaching_popup && !editing {
            self.mass_parameters = None;
            return None;
        }
        let popup_at = pointer.unwrap_or(r.center()) + vec2(22., 20.);
        let mut state = self
            .mass_parameters
            .take()
            .filter(|s| s.source == source)
            .unwrap_or(MassParameters {
                source: source.clone(),
                draft: source.clone(),
                popup_at,
                popup_rect: None,
                focused: false,
            });
        let language = self.language;
        let locked = self.host.document.editor.locked_element(source.id);
        let mut apply = false;
        let mut cancel = false;
        let mut controls_focused = false;
        let area = Area::new(Id::new("mass_model_parameters"))
            .default_size(vec2(240., 176.))
            .order(Order::Foreground)
            .fixed_pos(state.popup_at)
            .constrain_to(r)
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(226.);
                    ui.strong(format!(
                        "{} #{}",
                        language.translate("Concept mass"),
                        source.id
                    ));
                    ui.add_enabled_ui(!locked, |ui| {
                        let toggle = ui.add_enabled(
                            state.draft == state.source,
                            Button::new(language.translate("Push/Pull face")),
                        );
                        ui.ctx().data_mut(|d| {
                            d.insert_temp(Id::new("mass_push_pull_toggle"), toggle.rect)
                        });
                        if toggle.clicked() {
                            self.mass_push_pull.enabled = !self.mass_push_pull.enabled;
                        }
                        Grid::new("mass_model_values")
                            .num_columns(2)
                            .show(ui, |ui| {
                                if let Shape::ConceptMass { spec } = &mut state.draft.shape {
                                    for (label, n) in [
                                        ("Height", &mut spec.height),
                                        ("Bottom offset", &mut spec.bottom_offset),
                                    ] {
                                        let response = Self::number_input(ui, language, label, n);
                                        controls_focused |= response.has_focus()
                                            || response.is_pointer_button_down_on();
                                        ui.ctx().data_mut(|d| {
                                            d.insert_temp(
                                                Id::new(("mass_model_value", label)),
                                                response.rect,
                                            )
                                        });
                                        ui.end_row();
                                    }
                                }
                            });

                        let valid = if let Shape::ConceptMass { spec } = &state.draft.shape {
                            match spec.validate() {
                                Ok(()) => true,
                                Err(error) => {
                                    ui.colored_label(
                                        ui.visuals().error_fg_color,
                                        language.translate(&error),
                                    );
                                    false
                                }
                            }
                        } else {
                            false
                        };
                        ui.horizontal(|ui| {
                            let changed = state.draft != state.source;
                            let a = ui.add_enabled(
                                changed && valid,
                                Button::new(language.translate("Apply changes")),
                            );
                            let c = ui.button(language.translate("Cancel"));
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(Id::new("mass_model_apply"), a.rect);
                                d.insert_temp(Id::new("mass_model_cancel"), c.rect);
                            });
                            apply = a.clicked();
                            cancel = c.clicked();
                            if changed
                                && valid
                                && controls_focused
                                && ui.input(|i| i.key_pressed(Key::Enter))
                            {
                                apply = true;
                            }
                        });
                    });
                    if locked {
                        ui.label(language.translate("Selection contains locked elements"));
                    }
                });
            });
        if controls_focused && ui.input(|i| i.key_pressed(Key::Escape)) {
            cancel = true;
        }
        if cancel {
            state.draft = source.clone();
        }
        if apply {
            match self
                .host
                .document
                .editor
                .apply(crate::command::ModelCommand::UpdateElement {
                    element: state.draft.clone(),
                }) {
                Ok(_) => {
                    self.host.document.editor.status =
                        "Properties updated · Both views synchronized".into();
                    self.draft_id = None;
                    return Some(area.response.rect);
                }
                Err(error) => self.host.document.editor.status = error,
            }
        }
        ui.ctx()
            .data_mut(|d| d.insert_temp(Id::new("mass_model_popup"), area.response.rect));
        state.popup_rect = Some(area.response.rect);
        state.focused = controls_focused;
        self.mass_parameters = Some(state);
        Some(area.response.rect)
    }
}
