use super::*;

impl AtelierApp {
    pub(super) fn advance_rotated_slab_input(&mut self, pointer: Point) {
        let before = self.host.document.editor.slab_draft;
        let target = if self.tracker_locked
            && let Some(draft) = before
        {
            match draft.numeric_target(self.tracker_a, self.tracker_b) {
                Ok(point) => point,
                Err(error) => {
                    self.host.document.editor.status = error;
                    return;
                }
            }
        } else {
            pointer
        };
        self.host.document.editor.click(target, 0.);
        if self.host.document.editor.slab_draft != before {
            self.tracker_locked = false;
            self.tracker_a = 0.;
            self.tracker_b = 0.;
        }
    }

    pub(super) fn rotated_slab_tracker(&mut self, ui: &mut Ui) {
        let Some(draft) = self.host.document.editor.slab_draft else {
            // Reserve the same tracker height before the first point: accepting
            // an anchor must not move the canvas and its projected start point.
            Frame::new()
                .fill(PANEL_ALT)
                .inner_margin(Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.set_min_height(40.);
                    ui.set_min_width(ui.available_width());
                    ui.label(self.language.translate("Tracker · click a start node"));
                });
            return;
        };
        let baseline = matches!(draft, crate::slab_creation::SlabDraft::BaselineEnd { .. });
        let enter = ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers == Modifiers::NONE);
        Frame::new()
            .fill(PANEL_ALT)
            .inner_margin(Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.set_min_height(40.);
                ui.set_min_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    Self::label(ui, self.language.translate("TRACKER"));
                    let label = ui.label(self.language.translate(if baseline {
                        "Length"
                    } else {
                        "Signed width"
                    }));
                    if ui
                        .add(
                            DragValue::new(&mut self.tracker_a)
                                .speed(0.01)
                                .fixed_decimals(3)
                                .suffix(" m"),
                        )
                        .labelled_by(label.id)
                        .changed()
                    {
                        self.tracker_locked = true;
                    }
                    if baseline {
                        let label = ui.label(self.language.translate("Angle"));
                        if ui
                            .add(
                                DragValue::new(&mut self.tracker_b)
                                    .speed(0.1)
                                    .fixed_decimals(2)
                                    .suffix(" °"),
                            )
                            .labelled_by(label.id)
                            .changed()
                        {
                            self.tracker_locked = true;
                        }
                    }
                    ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                    ui.checkbox(&mut self.tracker_locked, self.language.translate("Lock"));
                    if ui
                        .button(self.language.translate(if baseline {
                            "Set baseline [Enter]"
                        } else {
                            "Create [Enter]"
                        }))
                        .clicked()
                        || enter
                    {
                        self.advance_rotated_slab_input(self.hover_point);
                    }
                    if ui.button(self.language.translate("Cancel")).clicked() {
                        self.host.document.editor.cancel_wall_draft();
                        self.tracker_locked = false;
                    }
                });
            });
    }
}

impl AtelierApp {
    pub(super) fn polygon_creation_active(&self) -> bool {
        (self.host.document.editor.tool == Tool::Slab
            && self.host.document.editor.slab_method
                == crate::slab_creation::SlabGeometryMethod::Polygonal)
            || (self.host.document.editor.tool == Tool::Roof
                && self.host.document.editor.roof_method
                    == crate::roof::RoofGeometryMethod::Polygonal
                && self.host.document.editor.roof_polygon_draft().is_some())
    }
    fn polygon_creation_points(
        &self,
    ) -> Option<(&[Point], &[crate::creation_input::CreationPoint])> {
        if self.host.document.editor.tool == Tool::Roof {
            self.host
                .document
                .editor
                .roof_polygon_draft()
                .map(|d| (d.points(), d.precise_points()))
        } else {
            self.host
                .document
                .editor
                .slab_polygon_draft()
                .map(|d| (d.points(), d.precise_points()))
        }
    }
    pub(super) fn polygon_creation_target(
        &self,
        pointer: Point,
    ) -> Result<crate::creation_input::CreationPoint, String> {
        if self.tracker_locked {
            if self.host.document.editor.tool == Tool::Roof {
                if let Some(draft) = self.host.document.editor.roof_polygon_draft() {
                    return draft.precise_numeric_target(
                        self.polygon_creation_length,
                        self.polygon_creation_angle,
                    );
                }
            } else if let Some(draft) = self.host.document.editor.slab_polygon_draft() {
                return draft.precise_numeric_target(
                    self.polygon_creation_length,
                    self.polygon_creation_angle,
                );
            }
        }
        if let Some((points, precise)) = self.polygon_creation_points()
            && let Some(i) = points.iter().position(|&p| p == pointer)
        {
            return Ok(precise[i]);
        }
        Ok(pointer.into())
    }
    pub(super) fn advance_polygon_creation_input(&mut self, pointer: Point, tolerance: f64) {
        match self.polygon_creation_target(pointer) {
            Ok(target) => self.advance_polygon_creation_precise_input(target, tolerance),
            Err(error) => self.host.document.editor.status = error,
        }
    }
    pub(super) fn advance_polygon_creation_precise_input(
        &mut self,
        target: crate::creation_input::CreationPoint,
        tolerance: f64,
    ) {
        if !self.tracker_locked
            && self
                .polygon_creation_points()
                .and_then(|(points, _)| points.first())
                .is_some_and(|p| p.distance(target.model_point()) <= tolerance)
        {
            self.finish_polygon_creation_input();
            return;
        }
        let result = if self.host.document.editor.tool == Tool::Roof {
            self.host
                .document
                .editor
                .accept_roof_polygon_precise_vertex(target)
        } else {
            self.host
                .document
                .editor
                .begin_slab_polygon()
                .and_then(|_| {
                    self.host
                        .document
                        .editor
                        .accept_slab_polygon_precise_vertex(target)
                })
        };
        match result {
            Ok(()) => self.tracker_locked = false,
            Err(error) => self.host.document.editor.status = error,
        }
    }
    pub(super) fn finish_polygon_creation_input(&mut self) {
        let roof = self.host.document.editor.tool == Tool::Roof;
        let result = if roof {
            self.host.document.editor.finish_roof_polygon()
        } else {
            self.host.document.editor.finish_slab_polygon()
        };
        match result {
            Ok(_) => {
                self.tracker_locked = false;
                self.host.document.editor.status = if roof {
                    "Roof · Click the pivot line start"
                } else {
                    self.host.document.editor.slab_method.status_key()
                }
                .into();
            }
            Err(error) => self.host.document.editor.status = error,
        }
    }
    pub(super) fn polygon_slab_preview_target(
        &self,
        target: crate::creation_input::CreationPoint,
    ) -> Result<Vec<crate::model::Element>, String> {
        let draft = self
            .host
            .document
            .editor
            .slab_polygon_draft()
            .ok_or("Invalid edit parameters")?;
        let regions =
            draft.precise_regions(Some(target), self.host.document.editor.slab_elevation)?;
        let mut input = draft.precise_points().to_vec();
        if input.last() != Some(&target) {
            input.push(target);
        }
        let choice = crate::slab_geometry::inspect_creation_region_choice(&regions, &input);
        let contours = match choice {
            crate::slab_geometry::CreationRegionChoice::Unique(i) => {
                vec![regions[i].contour.clone()]
            }
            _ => regions.into_iter().map(|r| r.contour).collect(),
        };
        contours
            .into_iter()
            .map(|c| self.host.document.editor.slab_creation_candidate(c))
            .collect()
    }
    pub(super) fn polygon_creation_tracker(&mut self, ui: &mut Ui) {
        let active = self
            .polygon_creation_points()
            .is_some_and(|(points, _)| !points.is_empty());
        let enter = ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers == Modifiers::NONE);
        Frame::new()
            .fill(PANEL_ALT)
            .inner_margin(Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.set_min_height(40.);
                ui.set_min_width(ui.available_width());
                if !active {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(self.language.translate("Tracker · click a start node"));
                        if self.host.document.editor.roof_polygon_draft().is_some()
                            && ui.button(self.language.translate("Cancel")).clicked()
                        {
                            self.host.document.editor.cancel_wall_draft();
                            self.tracker_locked = false;
                        }
                    });
                    return;
                }
                ui.horizontal_wrapped(|ui| {
                    Self::label(ui, self.language.translate("TRACKER"));
                    let label = ui.label(self.language.translate("Length"));
                    if ui
                        .add(
                            DragValue::new(&mut self.polygon_creation_length)
                                .speed(0.01)
                                .max_decimals(6)
                                .suffix(" m"),
                        )
                        .labelled_by(label.id)
                        .changed()
                    {
                        self.tracker_locked = true;
                    }
                    let label = ui.label(self.language.translate("Angle"));
                    if ui
                        .add(
                            DragValue::new(&mut self.polygon_creation_angle)
                                .speed(0.1)
                                .max_decimals(6)
                                .suffix(" °"),
                        )
                        .labelled_by(label.id)
                        .changed()
                    {
                        self.tracker_locked = true;
                    }
                    ui.checkbox(&mut self.tracker_locked, self.language.translate("Lock"));
                    ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                    if ui
                        .button(self.language.translate("Add point [Enter]"))
                        .clicked()
                        || enter
                    {
                        // Explicit numeric acceptance must use the displayed fields.
                        self.tracker_locked = true;
                        self.advance_polygon_creation_input(self.hover_point, 0.);
                    }
                    if ui.button(self.language.translate("Create")).clicked() {
                        self.finish_polygon_creation_input();
                    }
                    if ui.button(self.language.translate("Cancel")).clicked() {
                        self.host.document.editor.cancel_wall_draft();
                        self.tracker_locked = false;
                    }
                });
            });
    }
}
