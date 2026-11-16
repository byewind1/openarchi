//! Keyboard ownership and derived feedback for the shared drawing transition.
use super::*;
use crate::drawing_input::{DrawingBacktrack, backtrack_point};

impl AtelierApp {
    pub(super) fn drawing_ime_owns_input(&mut self, ctx: &Context) -> bool {
        ctx.input(|i| {
            if !i.focused {
                self.drawing_ime_composing = false;
                return false;
            }
            let mut owned = self.drawing_ime_composing;
            for event in &i.events {
                if let Event::Ime(event) = event {
                    owned = true; // Commit/disabled frames still belong to IME.
                    match event {
                        egui::ImeEvent::Preedit { text, .. } => {
                            self.drawing_ime_composing = !text.is_empty();
                        }
                        egui::ImeEvent::Commit(_) => {
                            self.drawing_ime_composing = false;
                        }
                        _ => {}
                    }
                }
            }
            owned
        })
    }

    pub(super) fn drawing_backtrack_shortcut(&mut self, ctx: &Context, ime: bool) -> bool {
        let presses = ctx.input(|i| {
            i.events
                .iter()
                .filter(|event| {
                    matches!(event,
                        Event::Key { key: Key::Backspace, pressed: true, modifiers, .. }
                            if modifiers.is_none()
                    )
                })
                .count()
        });
        if presses == 0 {
            return false;
        }
        // Leave input events to their owner. This guard precedes every canvas
        // deletion route, including placed drawings and empty drawing sessions.
        if ime
            || self.canvas_keyboard_blocked_ctx(ctx)
            || ctx.memory(|m| !m.is_above_modal_layer(LayerId::background()))
        {
            return true;
        }
        if matches!(self.view, View::Figure(_))
            || self.pointer_drag.is_some()
            || self.drag.is_some()
            || self.wall_edit.as_ref().is_some_and(|e| e.drag.is_some())
            || self.wall_insert.is_some()
            || self.reshape_session.is_some()
            || self.mass_push_pull_active()
            || self.opening_resize.is_some()
            || self.research_ui.canvas_move.is_some()
            || self.trace.pointer_mode_active()
            || self.guide_ui.action.is_some()
            || self.guide_ui.drag.is_some()
            || self.rotated_grid_tool.is_some()
            || self.orientation_pick_active
        {
            return false;
        }
        let mut owned = false;
        for _ in 0..presses {
            let changed = if self.drawings_ui.mode.is_some() {
                owned = true;
                let changed = backtrack_point(&mut self.drawings_ui.points);
                if changed {
                    self.drawings_ui.error.clear();
                }
                changed
            } else if self.drawings_ui.open || self.reference_ui.pending.is_some() {
                // A non-canvas import/settings surface owns the keyboard.
                return true;
            } else if self.host.document.editor.tool == Tool::Marquee {
                owned = true;
                let changed = backtrack_point(&mut self.marquee_points);
                if changed {
                    backtrack_point(&mut self.marquee_3d_points);
                }
                self.marquee_preview = None;
                self.marquee_3d_preview = None;
                if self.marquee_points.is_empty() {
                    self.marquee_3d_cursor = None;
                }
                changed
            } else {
                match self.host.document.editor.backtrack_drawing() {
                    DrawingBacktrack::NotDrawing => break,
                    DrawingBacktrack::Empty => {
                        owned = true;
                        false
                    }
                    DrawingBacktrack::Changed => {
                        owned = true;
                        true
                    }
                }
            };
            if changed {
                // Targets, snap markers and rubber bands are recomputed by
                // plan/model_view from the retained points and current pointer.
                self.tracker_locked = false;
                self.tracker_a = 0.;
                self.tracker_b = 0.;
                self.draft_id = None;
                self.cancel_canvas_pick();
            }
        }
        if owned {
            ctx.input_mut(|i| {
                i.events.retain(|event| {
                    !matches!(event,
                        Event::Key { key: Key::Backspace, modifiers, .. } if modifiers.is_none()
                    )
                })
            });
            ctx.request_repaint();
        }
        owned
    }
}
