use super::*;
use crate::screen_selection::{SelectionPath, select_paths};

pub(super) struct CanvasPickState {
    pub surface: bool,
    hover: Option<CandidateHover>,
    press: Option<CandidatePress>,
    suppressed: Option<(Id, Pos2)>,
}
impl Default for CanvasPickState {
    fn default() -> Self {
        Self {
            surface: true,
            hover: None,
            press: None,
            suppressed: None,
        }
    }
}
struct CandidateHover {
    owner: Id,
    position: Pos2,
    context: (u64, u64, Tool, bool),
    ids: Vec<u64>,
    index: usize,
    cycled: bool,
}
struct CandidatePress {
    owner: Id,
    position: Pos2,
    hit: Option<u64>,
    toggle: bool,
    strict: bool,
    cancelled: bool,
    context: (u64, u64),
}

#[derive(Clone, Copy)]
pub(super) struct SelectionBox {
    pub owner: Id,
    pub start: Pos2,
    pub end: Pos2,
}

impl AtelierApp {
    pub(super) fn canvas_keyboard_blocked(&self, ui: &Ui) -> bool {
        self.canvas_keyboard_blocked_ctx(ui.ctx())
    }
    pub(super) fn canvas_keyboard_blocked_ctx(&self, ctx: &Context) -> bool {
        !ctx.input(|i| i.focused)
            || ctx.text_edit_focused()
            || self.file_action.is_some()
            || self.close_prompt
            || self.recovery_open
            || self.help
            || self.element_settings.is_some()
            || self.layer_settings.is_some()
            || self.stair_settings_open
            || self.grid_dialog.is_some()
            || self.wall_settings_dialog.is_some()
            || self.command_dialog.is_some()
            || self.plan_option_dialog.is_some()
            || self.transfer_dialog.is_some()
            || self.edit_dialog.is_some()
            || self.guide_ui.values.is_some()
            || self.site_dialog.is_some()
            || self.process_ui.preview.is_some()
            || self.workbench.configuration_open
            || egui::Popup::is_any_open(ctx)
    }
    pub(super) fn canvas_editing_owns_input(&self) -> bool {
        self.host.document.editor.model_input_active()
            || self.wall_insert.is_some()
            || self.pointer_drag.is_some()
            || self.reshape_session.is_some()
            || self.drag.is_some()
            || self.opening_resize.is_some()
            || self.mass_push_pull_active()
            || self.guide_ui.action.is_some()
            || self.guide_ui.drag.is_some()
            || self.guide_ui.offset_edit.is_some()
            || self.guide_ui.distance_edit.is_some()
            || self.rotated_grid_tool.is_some()
            || self.orientation_pick_active
            || self.drawings_ui.mode.is_some()
            || self.research_ui.canvas_move.is_some()
            || self.wall_edit.as_ref().is_some_and(|e| e.drag.is_some())
    }
    pub(super) fn canvas_selection_context(&self, ui: &Ui) -> bool {
        !self.canvas_editing_owns_input()
            && self.host.document.editor.tool != Tool::Marquee
            && (self.host.document.editor.tool == Tool::Select || ui.input(|i| i.modifiers.shift))
            && !ui.input(|i| i.modifiers.alt || i.modifiers.ctrl || i.modifiers.command)
    }
    pub(super) fn canvas_surface_selection(&self, ui: &Ui) -> bool {
        self.canvas_pick.surface
            ^ (self.canvas_selection_context(ui) && ui.input(|i| i.key_down(Key::Space)))
    }
    pub(super) fn canvas_pick_strict(&self, ui: &Ui, owner: Id) -> bool {
        self.canvas_pick
            .press
            .as_ref()
            .is_some_and(|p| p.owner == owner && p.strict)
            || self
                .canvas_pick
                .hover
                .as_ref()
                .is_some_and(|h| h.owner == owner && h.cycled)
            || (self.canvas_selection_context(ui)
                && ui.input(|i| i.modifiers.shift || i.key_down(Key::Space)))
    }
    pub(super) fn canvas_pick_idle(&self) -> bool {
        self.canvas_pick.press.is_none()
    }
    pub(super) fn canvas_hover_id(&self, owner: Id) -> Option<u64> {
        self.canvas_pick
            .hover
            .as_ref()
            .filter(|h| h.owner == owner)
            .and_then(|h| h.ids.get(h.index).copied())
    }
    pub(super) fn canvas_selection_scope(&self, ids: &[u64]) -> Vec<u64> {
        let e = &self.host.document.editor;
        let mut ids = ids.to_vec();
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter()
            .filter(|id| {
                e.project.elements.iter().any(|v| {
                    v.id == *id
                        && v.worksheet == e.project.plan_options.active_worksheet
                        && geometry::model_element_layer_visible(&e.project, v)
                }) && (self.select_locked_elements || !e.locked_element(*id))
            })
            .collect()
    }
    pub(super) fn cancel_canvas_pick(&mut self) -> bool {
        let explicit = self.canvas_pick.hover.as_ref().is_some_and(|h| h.cycled)
            || self
                .canvas_pick
                .press
                .as_ref()
                .is_some_and(|p| p.strict && !p.cancelled);
        if let Some(h) = self.canvas_pick.hover.take() {
            self.canvas_pick.suppressed = Some((h.owner, h.position));
        }
        if let Some(p) = &mut self.canvas_pick.press {
            p.cancelled = true;
        }
        explicit
    }
    /// The view supplies only currently displayed candidates. This state owns
    /// a pointer gesture, never model geometry or tool defaults.
    pub(super) fn canvas_candidate_input(
        &mut self,
        ui: &Ui,
        rect: Rect,
        response: &Response,
        owner: Id,
        mut hits: Vec<u64>,
        scope: &[u64],
        enabled: bool,
    ) -> bool {
        let position = ui.input(|i| i.pointer.hover_pos());
        let eligible = enabled && !self.canvas_keyboard_blocked(ui);
        let inside = position.is_some_and(|p| rect.contains(p))
            && (response.hovered() || response.is_pointer_button_down_on());
        let active = eligible && inside && self.canvas_selection_context(ui);
        let had_hover = self
            .canvas_pick
            .hover
            .as_ref()
            .is_some_and(|h| h.owner == owner && !h.ids.is_empty());
        if ui.input(|i| i.pointer.primary_pressed())
            && self.canvas_pick.press.as_ref().is_some_and(|p| p.cancelled)
        {
            self.canvas_pick.press = None;
        }
        let editor = &self.host.document.editor;
        let context = (editor.document_session(), editor.revision);
        hits.retain(|id| scope.contains(id));
        // Even a rejected temporary-selection press owns its release. A
        // modifier released over a modal/text field must never become drawing.
        if enabled
            && !eligible
            && self.canvas_pick_strict(ui, owner)
            && inside
            && self.canvas_selection_context(ui)
            && ui.input(|i| i.pointer.primary_pressed())
        {
            self.canvas_pick.press = Some(CandidatePress {
                owner,
                position: position.unwrap(),
                hit: None,
                toggle: ui.input(|i| i.modifiers.shift),
                strict: true,
                cancelled: true,
                context,
            });
        }
        if !active {
            if self
                .canvas_pick
                .hover
                .as_ref()
                .is_some_and(|h| h.owner == owner)
            {
                self.canvas_pick.hover = None;
            }
        } else if let Some(position) = position {
            let mode = (
                context.0,
                context.1,
                editor.tool,
                self.canvas_surface_selection(ui),
            );
            let suppressed = !ui.input(|i| i.pointer.primary_pressed() || i.key_pressed(Key::Tab))
                && self
                    .canvas_pick
                    .suppressed
                    .is_some_and(|(o, p)| o == owner && p.distance(position) < 3.);
            if !suppressed {
                self.canvas_pick.suppressed = None;
                let reset = self.canvas_pick.hover.as_ref().is_none_or(|h| {
                    h.owner != owner
                        || h.context != mode
                        || h.ids != hits
                        || h.position.distance(position) > 3.
                });
                if reset {
                    self.canvas_pick.hover = Some(CandidateHover {
                        owner,
                        position,
                        context: mode,
                        ids: hits,
                        index: 0,
                        cycled: false,
                    });
                }
                let modifiers = ui.input(|i| i.modifiers);
                if had_hover
                    && self
                        .canvas_pick
                        .hover
                        .as_ref()
                        .is_some_and(|h| !h.ids.is_empty())
                    && ui.input_mut(|i| i.consume_key(modifiers, Key::Tab))
                {
                    let h = self.canvas_pick.hover.as_mut().unwrap();
                    h.index = (h.index + 1) % h.ids.len();
                    h.cycled = true;
                    ui.ctx().request_repaint();
                }
            }
            if ui.input(|i| i.pointer.primary_pressed()) {
                self.canvas_pick.press = Some(CandidatePress {
                    owner,
                    position,
                    hit: self.canvas_hover_id(owner),
                    toggle: ui.input(|i| i.modifiers.shift),
                    strict: self.canvas_pick_strict(ui, owner),
                    cancelled: false,
                    context,
                });
            }
        }
        let mut consume = false;
        if let Some(p) = &mut self.canvas_pick.press {
            if p.owner == owner {
                p.cancelled |= !eligible
                    || p.context != context
                    || !inside
                    || ui.input(|i| i.key_pressed(Key::Escape))
                    || position.is_some_and(|v| v.distance(p.position) > 6.);
                if ui.input(|i| i.pointer.primary_released()) && p.strict {
                    consume = response.clicked_by(PointerButton::Primary)
                        || !response.drag_stopped_by(PointerButton::Primary);
                }
            }
        }
        if consume {
            let p = self.canvas_pick.press.take().unwrap();
            if !p.cancelled && response.clicked_by(PointerButton::Primary) {
                self.apply_scoped_canvas_selection(
                    p.hit.filter(|id| scope.contains(id)).into_iter().collect(),
                    p.toggle,
                    scope,
                );
                ui.ctx().request_repaint();
            }
        } else if !ui.input(|i| i.pointer.primary_down() || i.pointer.primary_released()) {
            self.canvas_pick.press = None;
        }
        consume
    }
    pub(super) fn apply_scoped_canvas_selection(
        &mut self,
        ids: Vec<u64>,
        toggle: bool,
        scope: &[u64],
    ) {
        let mut allowed = scope.to_vec();
        if toggle {
            let editor = &self.host.document.editor;
            allowed.extend(editor.selection_ids().into_iter().filter(|id| {
                editor.project.elements.iter().any(|e| {
                    e.id == *id
                        && e.worksheet == editor.project.plan_options.active_worksheet
                        && geometry::model_element_layer_visible(&editor.project, e)
                }) && (self.select_locked_elements || !editor.locked_element(*id))
            }));
        }
        if toggle {
            self.host
                .document
                .editor
                .toggle_scoped_selection(ids, &allowed);
        } else {
            self.host
                .document
                .editor
                .set_scoped_selection(ids, &allowed);
        }
    }
    pub(super) fn commit_canvas_candidate(&mut self, ui: &Ui, owner: Id, scope: &[u64]) {
        if self.canvas_keyboard_blocked(ui) {
            return;
        }
        let pressed = self.canvas_pick.press.take().filter(|p| p.owner == owner);
        let hit = pressed
            .as_ref()
            .map_or_else(|| self.canvas_hover_id(owner), |p| p.hit);
        if pressed.as_ref().is_some_and(|p| p.cancelled) {
            return;
        }
        self.apply_scoped_canvas_selection(
            hit.filter(|id| scope.contains(id)).into_iter().collect(),
            pressed.map_or_else(|| ui.input(|i| i.modifiers.shift), |p| p.toggle),
            scope,
        );
    }
    pub(super) fn paint_canvas_candidate(
        &self,
        ui: &Ui,
        rect: Rect,
        owner: Id,
        paths: &[SelectionPath],
    ) {
        let Some(h) = self
            .canvas_pick
            .hover
            .as_ref()
            .filter(|h| h.owner == owner && !h.ids.is_empty())
        else {
            return;
        };
        let id = h.ids[h.index];
        let painter = ui.painter_at(rect);
        let color = Color32::from_rgb(40, 135, 210);
        for path in paths.iter().filter(|p| p.id == id) {
            let points: Vec<_> = path
                .points
                .iter()
                .map(|p| pos2(p.x as f32, p.y as f32))
                .collect();
            if points.len() >= 3 {
                painter.add(egui::Shape::convex_polygon(
                    points.clone(),
                    color.gamma_multiply(0.13),
                    Stroke::NONE,
                ));
            }
            if points.len() == 2 {
                painter.add(egui::Shape::line(points, Stroke::new(2., color)));
            }
        }
        let name = self
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .find(|e| e.id == id)
            .map(|e| {
                if e.name.is_empty() {
                    self.language.translate(e.kind()).to_owned()
                } else {
                    e.name.clone()
                }
            })
            .unwrap_or_default();
        let label = if h.ids.len() > 1 {
            self.language
                .translate("Overlapping elements · {index}/{count} · Tab")
                .replace("{index}", &(h.index + 1).to_string())
                .replace("{count}", &h.ids.len().to_string())
        } else {
            self.language.translate("Selection candidate").to_owned()
        };
        let galley = painter.layout(
            format!("{label}\n{name} · #{id}"),
            FontId::proportional(12.),
            INK,
            (rect.width() - 24.).clamp(80., 310.),
        );
        let size = galley.size() + vec2(16., 12.);
        let at = pos2(
            (h.position.x + 16.)
                .min(rect.right() - size.x)
                .max(rect.left()),
            (h.position.y + 18.)
                .min(rect.bottom() - size.y)
                .max(rect.top()),
        );
        painter.rect_filled(
            Rect::from_min_size(at, size),
            4.,
            Color32::from_rgb(250, 252, 255),
        );
        painter.rect_stroke(
            Rect::from_min_size(at, size),
            4.,
            Stroke::new(1., color),
            egui::StrokeKind::Inside,
        );
        painter.galley(at + vec2(8., 6.), galley, INK);
    }
    pub(super) fn ensure_canvas_selection_mode(&mut self) {
        if self.host.document.editor.tool != Tool::Select {
            self.activate_named_tool_legacy("Arrow");
        }
    }

    pub(super) fn figure_selection_input(
        &mut self,
        ui: &Ui,
        figure: &crate::diagrams::Diagram,
        status: crate::diagrams::Status,
        rect: Rect,
    ) {
        let owner = Id::new(("figure_selection", figure.id));
        let response = ui.interact(rect, owner, Sense::click_and_drag());
        if status != crate::diagrams::Status::Current {
            self.selection_drag(ui, rect, &response, owner, false);
            self.canvas_candidate_input(ui, rect, &response, owner, Vec::new(), &[], false);
            return;
        }
        if let Ok(mut paths) = crate::screen_selection::diagram_selection_paths(figure) {
            paths.retain(|p| {
                self.host.document.editor.project.elements.iter().any(|e| {
                    e.id == p.id
                        && geometry::model_element_layer_visible(
                            &self.host.document.editor.project,
                            e,
                        )
                })
            });
            let scale = rect.width() / 1200.;
            for path in &mut paths {
                for p in &mut path.points {
                    p.x = f64::from(rect.left()) + p.x * f64::from(scale);
                    p.y = f64::from(rect.top()) + p.y * f64::from(scale);
                }
            }
            self.selection_paths_input(ui, rect, &response, owner, &paths, true);
            let selected = self.host.document.editor.selection_ids();
            for path in paths.iter().filter(|p| selected.contains(&p.id)) {
                ui.painter_at(rect).add(egui::Shape::convex_polygon(
                    path.points
                        .iter()
                        .map(|p| pos2(p.x as f32, p.y as f32))
                        .collect(),
                    Color32::from_rgba_unmultiplied(25, 120, 65, 65),
                    Stroke::NONE,
                ));
            }
        }
    }
    pub(super) fn selection_drag(
        &mut self,
        ui: &Ui,
        rect: Rect,
        response: &Response,
        owner: Id,
        allowed: bool,
    ) -> Option<SelectionBox> {
        // A release belongs to its original viewport even when another pane
        // runs first. If that pane disappeared, retire the box next idle frame.
        if !ui.input(|i| i.pointer.button_down(PointerButton::Primary))
            && !ui.input(|i| i.pointer.button_released(PointerButton::Primary))
        {
            self.canvas_selection_box = None;
        }
        let owns = self.canvas_selection_box.is_some_and(|b| b.owner == owner);
        if owns
            && (!allowed
                || !ui.input(|i| i.focused)
                || ui.input(|i| i.key_pressed(Key::Escape))
                || (!ui.input(|i| i.pointer.button_down(PointerButton::Primary))
                    && !response.drag_stopped_by(PointerButton::Primary)))
        {
            self.canvas_selection_box = None;
        }
        if allowed
            && ui.input(|i| i.focused && !i.key_pressed(Key::Escape))
            && response.drag_started_by(PointerButton::Primary)
            && let Some(start) = ui
                .input(|i| i.pointer.press_origin())
                .filter(|p| rect.contains(*p))
        {
            self.canvas_selection_box = Some(SelectionBox {
                owner,
                start,
                end: start,
            });
        }
        if let Some(b) = &mut self.canvas_selection_box
            && b.owner == owner
        {
            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                b.end = rect.clamp(pos);
            }
            if response.drag_stopped_by(PointerButton::Primary) {
                ui.ctx().request_repaint();
                return self.canvas_selection_box.take();
            }
        }
        None
    }

    pub(super) fn selection_paths_input(
        &mut self,
        ui: &Ui,
        rect: Rect,
        response: &Response,
        owner: Id,
        paths: &[SelectionPath],
        clicks: bool,
    ) {
        let scope = self.canvas_selection_scope(&paths.iter().map(|p| p.id).collect::<Vec<_>>());
        let hits = ui
            .input(|i| i.pointer.hover_pos())
            .map(|pos| {
                crate::screen_selection::pick_paths(
                    paths,
                    Point::new(pos.x.into(), pos.y.into()),
                    4.,
                    self.canvas_surface_selection(ui),
                )
            })
            .unwrap_or_default();
        let consumed = self.canvas_candidate_input(ui, rect, response, owner, hits, &scope, clicks);
        // Box-only consumers keep their existing tool-independent gesture.
        let allowed =
            !self.canvas_keyboard_blocked(ui) && (!clicks || self.canvas_selection_context(ui));
        if let Some(b) = self.selection_drag(ui, rect, response, owner, allowed) {
            let rectangle = geometry::rectangle_polygon(
                Point::new(b.start.x.into(), b.start.y.into()),
                Point::new(b.end.x.into(), b.end.y.into()),
            );
            self.apply_scoped_canvas_selection(
                select_paths(paths, &rectangle, b.end.x >= b.start.x),
                ui.input(|i| i.modifiers.shift),
                &scope,
            );
        }
        if clicks && allowed && !consumed && response.clicked_by(PointerButton::Primary) {
            self.commit_canvas_candidate(ui, owner, &scope);
            ui.ctx().request_repaint();
        }
        self.paint_selection_box(&ui.painter_at(rect), owner);
        self.paint_canvas_candidate(ui, rect, owner, paths);
    }

    pub(super) fn paint_selection_box(&self, p: &Painter, owner: Id) {
        let Some(b) = self.canvas_selection_box.filter(|b| b.owner == owner) else {
            return;
        };
        let area = Rect::from_two_pos(b.start, b.end);
        p.rect_filled(area, 0., Color32::from_black_alpha(16));
        let points = [
            area.left_top(),
            area.right_top(),
            area.right_bottom(),
            area.left_bottom(),
            area.left_top(),
        ];
        let pattern: &[f32] = if b.end.x >= b.start.x {
            &[8., 3., 1., 3.]
        } else {
            &[1., 3.]
        };
        for edge in points.windows(2) {
            p.line_segment([edge[0], edge[1]], Stroke::new(1., Color32::WHITE));
            let delta = edge[1] - edge[0];
            let length = delta.length();
            if length == 0. {
                continue;
            }
            let direction = delta / length;
            let mut at = 0.;
            let mut index = 0;
            while at < length {
                let end = (at + pattern[index % pattern.len()]).min(length);
                if index % 2 == 0 {
                    p.line_segment(
                        [edge[0] + direction * at, edge[0] + direction * end],
                        Stroke::new(1., Color32::BLACK),
                    );
                }
                at = end;
                index += 1;
            }
        }
    }
}
