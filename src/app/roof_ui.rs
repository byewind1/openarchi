use super::*;

impl AtelierApp {
    pub(super) fn advance_roof_input(&mut self, pointer: Point) {
        let before = self.host.document.editor.roof_draft;
        let point = if self.tracker_locked
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
        self.host.document.editor.click(point, 0.);
        if self.host.document.editor.roof_draft != before {
            self.tracker_locked = false;
            self.tracker_a = 0.;
            self.tracker_b = 0.;
        }
    }
    pub(super) fn roof_tracker(&mut self, ui: &mut Ui) {
        if self.host.document.editor.roof_polygon_draft.is_some() {
            return;
        }
        let draft = self.host.document.editor.roof_draft;
        let pivot = matches!(draft, Some(crate::roof::RoofDraft::PivotEnd { .. }));
        let baseline = matches!(
            draft,
            Some(crate::roof::RoofDraft::RotatedContour {
                rectangle: crate::slab_creation::SlabDraft::BaselineEnd { .. },
                ..
            })
        );
        let width = matches!(
            draft,
            Some(crate::roof::RoofDraft::RotatedContour {
                rectangle: crate::slab_creation::SlabDraft::Width { .. },
                ..
            })
        );
        let angular = pivot || baseline;
        let numeric =
            angular || width || matches!(draft, Some(crate::roof::RoofDraft::ContourEnd { .. }));
        let enter =
            numeric && ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers == Modifiers::NONE);
        Frame::new()
            .fill(PANEL_ALT)
            .inner_margin(Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.set_min_height(40.);
                ui.set_min_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    if numeric {
                        Self::label(ui, self.language.translate("TRACKER"));
                        let label = ui.label(self.language.translate(if angular {
                            "Length"
                        } else if width {
                            "Signed width"
                        } else {
                            "ΔX"
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
                        if !width {
                            let label = ui.label(self.language.translate(if angular {
                                "Angle"
                            } else {
                                "ΔY"
                            }));
                            if ui
                                .add(
                                    DragValue::new(&mut self.tracker_b)
                                        .speed(0.1)
                                        .fixed_decimals(if angular { 2 } else { 3 })
                                        .suffix(if angular { " °" } else { " m" }),
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
                            .button(self.language.translate(if pivot {
                                "Set pivot [Enter]"
                            } else if baseline {
                                "Set baseline [Enter]"
                            } else {
                                "Create [Enter]"
                            }))
                            .clicked()
                            || enter
                        {
                            self.advance_roof_input(self.hover_point);
                        }
                    } else {
                        self.tracker_locked = false;
                        ui.label(self.language.translate(match draft {
                            Some(crate::roof::RoofDraft::RiseSide { .. }) => {
                                "Roof · Click the slope side"
                            }
                            Some(crate::roof::RoofDraft::ContourStart { .. }) => {
                                "Roof · Click the first rectangle corner"
                            }
                            _ => "Tracker · click a start node",
                        }));
                    }
                    if draft.is_some() && ui.button(self.language.translate("Cancel")).clicked() {
                        self.host.document.editor.cancel_wall_draft();
                        self.tracker_locked = false;
                    }
                });
            });
    }
}
