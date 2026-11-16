use super::*;

impl AtelierApp {
    pub(super) fn advance_beam_arc_input(&mut self, pointer: Point) {
        let result = if self.tracker_locked
            && let Some(mut draft) = self.host.document.editor.beam_arc_draft
        {
            if draft.start().is_none() {
                draft
                    .accept_numeric_radius(pointer, self.tracker_a)
                    .map(|()| {
                        self.host.document.editor.beam_arc_draft = Some(draft);
                        self.host.document.editor.refresh_beam_status();
                    })
            } else {
                draft
                    .set_sweep_degrees(self.tracker_a as f64)
                    .and_then(|_| {
                        self.host.document.editor.beam_arc_draft = Some(draft);
                        self.host.document.editor.commit_beam_arc().map(|_| ())
                    })
            }
        } else {
            self.host
                .document
                .editor
                .click_beam_arc(pointer)
                .map(|_| ())
        };
        match result {
            Ok(()) => {
                self.tracker_locked = false;
                if let Some(draft) = self.host.document.editor.beam_arc_draft {
                    self.tracker_a = draft.sweep_degrees().unwrap_or(0.) as f64;
                }
            }
            Err(error) => self.host.document.editor.status = error,
        }
    }

    pub(super) fn beam_arc_tracker(&mut self, ui: &mut Ui) {
        let Some(draft) = self.host.document.editor.beam_arc_draft else {
            return;
        };
        let radius = draft.start().is_none();
        // Let the numeric field accept its text first, then consume Enter before
        // focused Lock/action controls can also activate on the same keystroke.
        let enter = ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers == Modifiers::NONE);
        Frame::new()
            .fill(PANEL_ALT)
            .inner_margin(Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.set_min_height(40.);
                ui.horizontal_wrapped(|ui| {
                    Self::label(ui, self.language.translate("TRACKER"));
                    let label = ui.label(self.language.translate(if radius {
                        "Radius"
                    } else {
                        "Arc sweep"
                    }));
                    let input = ui
                        .add(
                            DragValue::new(&mut self.tracker_a)
                                .speed(if radius { 0.01 } else { 0.5 })
                                .fixed_decimals(if radius { 3 } else { 2 })
                                .suffix(if radius { " m" } else { " °" }),
                        )
                        .labelled_by(label.id);
                    if input.changed() {
                        self.tracker_locked = true;
                    }
                    ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                    ui.checkbox(&mut self.tracker_locked, self.language.translate("Lock"));
                    if ui
                        .button(self.language.translate(if radius {
                            "Set radius [Enter]"
                        } else {
                            "Create [Enter]"
                        }))
                        .clicked()
                        || enter
                    {
                        self.advance_beam_arc_input(self.hover_point);
                    }
                    if ui.button(self.language.translate("Cancel")).clicked() {
                        self.host.document.editor.cancel_wall_draft();
                        self.host.document.editor.refresh_beam_status();
                        self.tracker_locked = false;
                    }
                });
            });
    }
}
