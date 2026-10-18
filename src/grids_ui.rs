// Native Grids menu and Grids & Background dialog. Included from app.rs so the
// controls share the application state without exposing extra public fields.
use crate::grids::{GridSettings, GridSnapMode};

#[derive(Clone, Copy, Debug, PartialEq)]
struct RotatedGridTool {
    start: Option<Point>,
}

impl AtelierApp {
    fn apply_grid_snap(&self, point: Point, base: Point, kind: SnapKind) -> (Point, SnapKind) {
        if matches!(kind, SnapKind::Grid | SnapKind::Free) {
            if let Some(snapped) = self.editor.project.grid_settings.snap_point(point) {
                return (snapped, SnapKind::Grid);
            }
            return (point, SnapKind::Free);
        }
        (base, kind)
    }

    fn start_rotated_grid_tool(&mut self) {
        self.view = View::Plan;
        self.rotated_grid_tool = Some(RotatedGridTool { start: None });
        self.editor.status =
            "Set Rotated Grid: click a straight segment, or click two points · Esc cancels".into();
    }

    fn nearest_straight_segment_angle(&self, point: Point, tolerance: f32) -> Option<f32> {
        let mut best: Option<(f32, f32)> = None;
        for element in self
            .editor
            .project
            .elements
            .iter()
            .filter(|element| element.story == self.editor.story)
        {
            let mut segments: Vec<(Point, Point)> = Vec::new();
            match &element.shape {
                Shape::Wall { start, end, .. } => segments.push((*start, *end)),
                Shape::Polyline { points, arcs } => {
                    for (index, pair) in points.windows(2).enumerate() {
                        if arcs.get(index).is_none_or(|arc| arc.is_none()) {
                            segments.push((pair[0], pair[1]));
                        }
                    }
                }
                Shape::Slab { .. } | Shape::Stair { .. } => {
                    let outline = geometry::footprint(element);
                    for index in 0..outline.len() {
                        segments.push((outline[index], outline[(index + 1) % outline.len()]));
                    }
                }
                Shape::Opening { .. } => {}
            }
            for (start, end) in segments {
                let distance = geometry::segment_distance(point, start, end);
                if distance <= tolerance
                    && best.is_none_or(|(best_distance, _)| distance < best_distance)
                    && let Some(angle) = GridSettings::angle_from_vector(start, end)
                {
                    best = Some((distance, angle));
                }
            }
        }
        best.map(|(_, angle)| angle)
    }

    fn handle_rotated_grid_click(&mut self, raw: Point, target: Point, tolerance: f32) {
        let Some(mut tool) = self.rotated_grid_tool else {
            return;
        };
        if let Some(start) = tool.start {
            if let Some(angle) = GridSettings::angle_from_vector(start, target) {
                let settings = &mut self.editor.project.grid_settings;
                settings.rotate_enabled = true;
                settings.rotated_display = true;
                settings.rotation_deg = angle;
                self.rotated_grid_tool = None;
                self.editor.status = format!("Rotated grid set to {angle:.2}°");
            } else {
                self.editor.status = "Rotated grid needs two distinct points".into();
            }
            return;
        }
        if let Some(angle) = self.nearest_straight_segment_angle(raw, tolerance) {
            let settings = &mut self.editor.project.grid_settings;
            settings.rotate_enabled = true;
            settings.rotated_display = true;
            settings.rotation_deg = angle;
            self.rotated_grid_tool = None;
            self.editor.status = format!("Rotated grid aligned to segment · {angle:.2}°");
            return;
        }
        tool.start = Some(target);
        self.rotated_grid_tool = Some(tool);
        self.editor.status = "Set Rotated Grid: click a second point · Esc cancels".into();
    }

    fn grids_menu(&mut self, ui: &mut Ui) {
        let mut set_rotated = false;
        let mut open_dialog = false;
        {
            let settings = &mut self.editor.project.grid_settings;
            ui.checkbox(
                &mut settings.show_construction_grid,
                self.language.translate("Construction Grid Display"),
            );
            ui.separator();
            let mut grid_snap_enabled = settings.grid_snap_enabled;
            if ui
                .checkbox(&mut grid_snap_enabled, self.language.translate("Grid Snap"))
                .changed()
            {
                settings.set_snap_enabled(grid_snap_enabled);
            }
            let active_mode = if settings.grid_snap_enabled {
                Some(settings.snap_mode)
            } else {
                None
            };
            for mode in [GridSnapMode::SnapGrid, GridSnapMode::ConstructionGrid] {
                if ui
                    .selectable_label(
                        active_mode == Some(mode),
                        self.language.translate(mode.label_key()),
                    )
                    .clicked()
                {
                    settings.snap_mode = mode;
                    settings.grid_snap_enabled = true;
                }
            }
            ui.separator();
            ui.checkbox(
                &mut settings.origin_display,
                self.language.translate("Origin Display"),
            );
            ui.separator();
            if ui
                .selectable_label(
                    !settings.rotated_display,
                    self.language.translate("Orthogonal Grid"),
                )
                .clicked()
            {
                settings.rotated_display = false;
            }
            if ui
                .selectable_label(
                    settings.rotated_display,
                    self.language.translate("Rotated Grid"),
                )
                .clicked()
            {
                settings.rotate_enabled = true;
                settings.rotated_display = true;
            }
            if ui
                .button(self.language.translate("Set Rotated Grid"))
                .clicked()
            {
                set_rotated = true;
                ui.close();
            }
            if ui
                .button(self.language.translate("Grids & Background…"))
                .clicked()
            {
                open_dialog = true;
                ui.close();
            }
            ui.separator();
            let mut editing_plane = false;
            ui.add_enabled(
                false,
                Checkbox::new(
                    &mut editing_plane,
                    self.language.translate("Editing Plane Display"),
                ),
            )
            .on_disabled_hover_text(
                self.language
                    .translate("Editing plane display is not implemented"),
            );
            for label in ["Pick Plane", "Horizontal", "Offset", "Snap to Story…"] {
                ui.add_enabled(false, Button::new(self.language.translate(label)))
                    .on_disabled_hover_text(
                        self.language
                            .translate("Editing plane display is not implemented"),
                    );
            }
        }
        if set_rotated {
            self.start_rotated_grid_tool();
        }
        if open_dialog {
            self.grid_dialog = Some(self.editor.project.grid_settings.clone());
            self.grid_dialog_error.clear();
            self.grid_dialog_just_opened = true;
            self.grid_preview_zoom = 25.0;
        }
    }

    fn commit_grid_settings(&mut self, draft: GridSettings) -> Result<(), String> {
        draft.validate()?;
        if draft == self.editor.project.grid_settings {
            self.editor.status = "Grids & Background unchanged".into();
            return Ok(());
        }
        let before = self.editor.project.clone();
        self.editor.project.grid_settings = draft;
        self.editor.commit(before);
        self.editor.status = "Grids & Background applied".into();
        Ok(())
    }

    fn grids_dialog(&mut self, ctx: &Context) {
        let Some(mut draft) = self.grid_dialog.take() else {
            return;
        };
        let just_opened = self.grid_dialog_just_opened;
        self.grid_dialog_just_opened = false;
        let mut error = std::mem::take(&mut self.grid_dialog_error);
        let mut zoom = self.grid_preview_zoom;
        let mut close = false;
        let mut apply = false;
        let modal = egui::Modal::new(Id::new("grids_background_modal")).show(ctx, |ui| {
            ui.set_min_width(680.0);
            ui.heading(self.language.translate("Grids & Background"));
            ui.add_space(4.0);
            ui.checkbox(
                &mut draft.show_construction_grid,
                self.language.translate("Show Construction Grid"),
            );
            ui.add_space(8.0);
            Grid::new("grids_dimensions")
                .num_columns(5)
                .spacing([14.0, 6.0])
                .show(ui, |ui| {
                    ui.label("");
                    ui.strong(self.language.translate("Main Grid"));
                    ui.label("");
                    ui.strong(self.language.translate("Aux. Grid"));
                    ui.label("");
                    ui.end_row();
                    ui.label("");
                    ui.weak(self.language.translate("Spacing"));
                    ui.weak(self.language.translate("Steps"));
                    ui.weak(self.language.translate("Spacing"));
                    ui.weak(self.language.translate("Steps"));
                    ui.end_row();
                    ui.label("X");
                    ui.add(
                        DragValue::new(&mut draft.main_spacing_x)
                            .speed(0.01)
                            .fixed_decimals(3)
                            .suffix(" m"),
                    );
                    ui.add(DragValue::new(&mut draft.main_steps_x).range(0..=100));
                    ui.add(
                        DragValue::new(&mut draft.aux_spacing_x)
                            .speed(0.01)
                            .fixed_decimals(3)
                            .suffix(" m"),
                    );
                    ui.add(DragValue::new(&mut draft.aux_steps_x).range(0..=100));
                    ui.end_row();
                    ui.label("Y");
                    ui.add(
                        DragValue::new(&mut draft.main_spacing_y)
                            .speed(0.01)
                            .fixed_decimals(3)
                            .suffix(" m"),
                    );
                    ui.add(DragValue::new(&mut draft.main_steps_y).range(0..=100));
                    ui.add(
                        DragValue::new(&mut draft.aux_spacing_y)
                            .speed(0.01)
                            .fixed_decimals(3)
                            .suffix(" m"),
                    );
                    ui.add(DragValue::new(&mut draft.aux_steps_y).range(0..=100));
                    ui.end_row();
                });
            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut draft.rotate_enabled,
                    self.language.translate("Rotate Grid"),
                );
                ui.add_space(16.0);
                ui.label(self.language.translate("Local Origin:"));
                ui.label("X");
                ui.add(
                    DragValue::new(&mut draft.origin_x)
                        .speed(0.01)
                        .fixed_decimals(3)
                        .suffix(" m"),
                );
                ui.label("Y");
                ui.add(
                    DragValue::new(&mut draft.origin_y)
                        .speed(0.01)
                        .fixed_decimals(3)
                        .suffix(" m"),
                );
                ui.add(
                    DragValue::new(&mut draft.rotation_deg)
                        .speed(0.1)
                        .fixed_decimals(2)
                        .suffix("°"),
                );
            });
            ui.add_space(6.0);
            ui.separator();
            ui.strong(self.language.translate("Snap Grid / Nudge"));
            ui.horizontal(|ui| {
                ui.label(self.language.translate("Spacing:"));
                ui.label("X");
                ui.add(
                    DragValue::new(&mut draft.snap_spacing_x)
                        .speed(0.001)
                        .fixed_decimals(3)
                        .suffix(" m"),
                );
                ui.label("Y");
                ui.add(
                    DragValue::new(&mut draft.snap_spacing_y)
                        .speed(0.001)
                        .fixed_decimals(3)
                        .suffix(" m"),
                );
            });
            ui.horizontal(|ui| {
                let modes = [
                    (GridSnapMode::None, "No snap"),
                    (GridSnapMode::SnapGrid, "Snap to Snap Grid"),
                    (GridSnapMode::ConstructionGrid, "Snap to Construction Grid"),
                ];
                for (mode, label) in modes {
                    let selected = match mode {
                        GridSnapMode::None => !draft.grid_snap_enabled,
                        _ => draft.grid_snap_enabled && draft.snap_mode == mode,
                    };
                    if ui
                        .selectable_label(selected, self.language.translate(label))
                        .clicked()
                    {
                        match mode {
                            GridSnapMode::None => draft.grid_snap_enabled = false,
                            _ => {
                                draft.snap_mode = mode;
                                draft.grid_snap_enabled = true;
                            }
                        }
                    }
                }
            });
            ui.add_space(6.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.strong(self.language.translate("Background:"));
                let mut color = Color32::from_rgba_unmultiplied(
                    draft.background[0],
                    draft.background[1],
                    draft.background[2],
                    draft.background[3],
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    draft.background = color.to_array();
                }
                ui.add_space(14.0);
                ui.strong(self.language.translate("Grid Lines:"));
                let mut color = Color32::from_rgba_unmultiplied(
                    draft.grid_line_color[0],
                    draft.grid_line_color[1],
                    draft.grid_line_color[2],
                    draft.grid_line_color[3],
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    draft.grid_line_color = color.to_array();
                }
            });
            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(360.0, 200.0), Sense::hover());
                paint_grid_preview(ui.painter(), rect, &draft, zoom);
                ui.vertical(|ui| {
                    ui.label(self.language.translate("Preview"));
                    ui.horizontal(|ui| {
                        if ui.small_button("−").clicked() {
                            zoom = (zoom / 1.25).clamp(5.0, 800.0);
                        }
                        ui.add(
                            DragValue::new(&mut zoom)
                                .speed(1.0)
                                .range(5.0..=800.0)
                                .suffix(" %"),
                        );
                        if ui.small_button("+").clicked() {
                            zoom = (zoom * 1.25).clamp(5.0, 800.0);
                        }
                    });
                    ui.label(
                        self.language
                            .translate("Grid hides below 3 px line spacing"),
                    );
                });
            });
            if !error.is_empty() {
                ui.colored_label(Color32::LIGHT_RED, &error);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(Button::new(self.language.translate("OK")).fill(ACCENT))
                        .clicked()
                    {
                        apply = true;
                    }
                    if ui.button(self.language.translate("Cancel")).clicked() {
                        close = true;
                    }
                });
            });
        });
        if apply {
            match self.commit_grid_settings(draft.clone()) {
                Ok(()) => {
                    close = true;
                    error.clear();
                }
                Err(message) => error = message,
            }
        }
        self.grid_dialog_error = error;
        self.grid_preview_zoom = zoom;
        if close || (!just_opened && modal.should_close()) {
            self.grid_dialog = None;
            self.grid_dialog_error.clear();
        } else {
            self.grid_dialog = Some(draft);
        }
    }
}

fn paint_grid_preview(painter: &Painter, rect: Rect, settings: &GridSettings, zoom: f32) {
    let mut preview = settings.clone();
    preview.origin_x = 0.0;
    preview.origin_y = 0.0;
    preview.show_construction_grid = true;
    preview.rotated_display = preview.rotate_enabled;
    let background = preview.background;
    painter.rect_filled(
        rect,
        0.0,
        Color32::from_rgba_unmultiplied(background[0], background[1], background[2], background[3]),
    );
    let painter = painter.with_clip_rect(rect);
    let pixels_per_meter = 16.0 * (zoom / 100.0);
    if preview
        .minimum_gap(true)
        .is_some_and(|gap| gap * pixels_per_meter >= 1.5)
        && preview
            .minimum_gap(false)
            .is_some_and(|gap| gap * pixels_per_meter >= 1.5)
    {
        let half_u = rect.width() * 0.5 / pixels_per_meter;
        let half_v = rect.height() * 0.5 / pixels_per_meter;
        let center = rect.center();
        for line in preview.lines_in_local(-half_u, half_u, -half_v, half_v) {
            let a = pos2(
                center.x + line.start.x * pixels_per_meter,
                center.y - line.start.y * pixels_per_meter,
            );
            let b = pos2(
                center.x + line.end.x * pixels_per_meter,
                center.y - line.end.y * pixels_per_meter,
            );
            let color = preview.line_color(line.main);
            painter.line_segment(
                [a, b],
                Stroke::new(
                    if line.main { 1.0 } else { 0.6 },
                    Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3]),
                ),
            );
        }
    }
    if preview.origin_display {
        let center = rect.center();
        painter.line_segment(
            [center - vec2(8.0, 0.0), center + vec2(8.0, 0.0)],
            Stroke::new(1.0, ACCENT),
        );
        painter.line_segment(
            [center - vec2(0.0, 8.0), center + vec2(0.0, 8.0)],
            Stroke::new(1.0, ACCENT),
        );
    }
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, LINE), StrokeKind::Inside);
}

#[cfg(test)]
mod grid_ui_tests {
    use super::*;

    fn setup() -> AtelierApp {
        let ctx = Context::default();
        AtelierApp::style(&ctx);
        let mut app = AtelierApp::from_renderer(None, "test".into());
        app.editor = Editor::new(Project::default());
        app.plan_center = Point::default();
        app.plan_scale = 50.0;
        app
    }

    #[test]
    fn grid_snap_uses_project_spacing_and_can_be_disabled() {
        let mut app = setup();
        app.editor.project.grid_settings.grid_snap_enabled = true;
        app.editor.project.grid_settings.snap_mode = crate::grids::GridSnapMode::SnapGrid;
        app.editor.project.grid_settings.snap_spacing_x = 0.05;
        app.editor.project.grid_settings.snap_spacing_y = 0.05;
        let point = Point::new(1.03, 1.02);
        let snapped = app.apply_grid_snap(point, point, SnapKind::Free);
        assert_eq!(snapped.1, SnapKind::Grid);
        assert!((snapped.0.x - 1.05).abs() < 1e-4);
        assert!((snapped.0.y - 1.0).abs() < 1e-4);
        app.editor.project.grid_settings.grid_snap_enabled = false;
        assert_eq!(
            app.apply_grid_snap(point, point, SnapKind::Free),
            (point, SnapKind::Free)
        );
    }

    #[test]
    fn tracker_lock_is_not_overridden_by_grid_snap() {
        let mut app = setup();
        app.editor.tool = Tool::Wall;
        app.editor.anchor = Some(Point::new(0.0, 0.0));
        app.tracker_locked = true;
        app.tracker_a = 1.234;
        app.tracker_b = 33.0;
        let (target, kind) = app.target(Point::new(5.0, 5.0), false);
        let expected = app.tracker_target(Point::new(0.0, 0.0));
        assert_eq!(target, expected);
        assert_eq!(kind, SnapKind::Free);
    }

    #[test]
    fn rotated_grid_tool_accepts_two_clicks() {
        let mut app = setup();
        app.start_rotated_grid_tool();
        app.handle_rotated_grid_click(Point::new(0.0, 0.0), Point::new(0.0, 0.0), 0.1);
        assert_eq!(
            app.rotated_grid_tool,
            Some(RotatedGridTool {
                start: Some(Point::new(0.0, 0.0))
            })
        );
        app.handle_rotated_grid_click(Point::new(2.0, 2.0), Point::new(2.0, 2.0), 0.1);
        assert!(app.rotated_grid_tool.is_none());
        assert!(app.editor.project.grid_settings.rotate_enabled);
        assert!(app.editor.project.grid_settings.rotated_display);
        assert!((app.editor.project.grid_settings.rotation_deg - 45.0).abs() < 1e-4);
    }

    #[test]
    fn rotated_grid_tool_aligns_to_existing_straight_segment() {
        let mut app = setup();
        app.editor.project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(0.0, 0.0),
                end: Point::new(2.0, 1.0),
                thickness: 0.2,
                height: 3.0,
            },
        );
        app.start_rotated_grid_tool();
        app.handle_rotated_grid_click(Point::new(1.0, 0.5), Point::new(1.0, 0.5), 0.2);
        assert!(app.rotated_grid_tool.is_none());
        let expected = 0.5_f32.atan2(1.0).to_degrees();
        assert!((app.editor.project.grid_settings.rotation_deg - expected).abs() < 1e-3);
    }

    fn dialog_frame(ctx: &Context, app: &mut AtelierApp) {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
                ..Default::default()
            },
            |ui| app.grids_dialog(ui.ctx()),
        );
        output.textures_delta.clear();
    }

    #[test]
    fn dialog_survives_the_frame_that_opened_it() {
        let mut app = setup();
        app.grid_dialog = Some(app.editor.project.grid_settings.clone());
        app.grid_dialog_just_opened = true;
        let ctx = Context::default();
        dialog_frame(&ctx, &mut app);
        assert!(app.grid_dialog.is_some());
        assert!(!app.grid_dialog_just_opened);
    }

    #[test]
    fn dialog_draft_persists_across_frames_without_committing() {
        let mut app = setup();
        app.grid_dialog = Some(app.editor.project.grid_settings.clone());
        app.grid_dialog_just_opened = true;
        let ctx = Context::default();
        dialog_frame(&ctx, &mut app);
        let draft = app.grid_dialog.as_mut().unwrap();
        draft.main_spacing_x = 3.0;
        draft.rotate_enabled = true;
        dialog_frame(&ctx, &mut app);
        let draft = app.grid_dialog.as_ref().unwrap();
        assert_eq!(draft.main_spacing_x, 3.0);
        assert!(draft.rotate_enabled);
        assert_eq!(app.editor.project.grid_settings.main_spacing_x, 2.0);
        assert!(!app.editor.project.grid_settings.rotate_enabled);
    }

    #[test]
    fn grids_apply_and_cancel_leave_expected_project_state() {
        let mut app = setup();
        let before = app.editor.project.clone();
        let mut draft = before.grid_settings.clone();
        draft.main_spacing_x = 3.0;
        app.commit_grid_settings(draft.clone()).unwrap();
        assert_eq!(app.editor.project.grid_settings, draft);
        assert_eq!(app.editor.undo.len(), 1);
        app.editor.undo();
        assert_eq!(app.editor.project, before);
        assert_eq!(app.editor.undo.len(), 0);

        let mut invalid = before.grid_settings.clone();
        invalid.main_spacing_x = 0.0;
        assert!(app.commit_grid_settings(invalid).is_err());
        assert_eq!(app.editor.project, before);
        assert!(app.editor.undo.is_empty());
    }
}
