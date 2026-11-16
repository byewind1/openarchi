use super::*;

// Keep the original edge's dash phase, but never walk its off-screen length.
fn visible_column_dashes(a: Pos2, b: Pos2, clip: Rect) -> Vec<[Pos2; 2]> {
    if !a.is_finite() || !b.is_finite() || !clip.is_finite() || !clip.is_positive() {
        return Vec::new();
    }
    let origin = [f64::from(a.x), f64::from(a.y)];
    let delta = [f64::from(b.x) - origin[0], f64::from(b.y) - origin[1]];
    let length = delta[0].hypot(delta[1]);
    if length == 0. {
        return Vec::new();
    }
    let unit = [delta[0] / length, delta[1] / length];
    let mut start: f64 = 0.;
    let mut end = length;
    for axis in 0..2 {
        let low = f64::from(clip.min[axis]);
        let high = f64::from(clip.max[axis]);
        if unit[axis] == 0. {
            if origin[axis] < low || origin[axis] > high {
                return Vec::new();
            }
        } else {
            let p = (low - origin[axis]) / unit[axis];
            let q = (high - origin[axis]) / unit[axis];
            start = start.max(p.min(q));
            end = end.min(p.max(q));
        }
    }
    if start > end {
        return Vec::new();
    }
    let point = |distance: f64| {
        pos2(
            (origin[0] + unit[0] * distance) as f64,
            (origin[1] + unit[1] * distance) as f64,
        )
    };
    let mut result = Vec::new();
    // Use an integer count so floating-point addition cannot stall at large offsets.
    let first = (start / 8.).floor() * 8.;
    let count = ((end - first) / 8.).floor() as usize + 1;
    for index in 0..count {
        let dash = first + index as f64 * 8.;
        let from = dash.max(start);
        let to = (dash + 5.).min(end);
        if from < to {
            result.push([point(from), point(to)]);
        }
    }
    result
}

impl AtelierApp {
    pub(super) fn paint_column_plan_geometry(
        &self,
        p: &egui::Painter,
        r: Rect,
        plan: &crate::column_plan::ColumnPlanGeometry,
        fill: Color32,
        stroke: Stroke,
    ) {
        Self::paint_column_plan_geometry_mapped(
            p,
            plan,
            |point| self.screen(r, point),
            fill,
            stroke,
        );
    }
    pub(super) fn paint_column_plan_geometry_mapped(
        p: &egui::Painter,
        plan: &crate::column_plan::ColumnPlanGeometry,
        screen: impl Fn(Point) -> Pos2,
        fill: Color32,
        stroke: Stroke,
    ) {
        // Projection linework is separate from the actual cut fill.
        for (ring, dashed) in [(&plan.uncut, false), (&plan.overhead, true)] {
            for i in 0..ring.len() {
                let a = screen(ring[i]);
                let b = screen(ring[(i + 1) % ring.len()]);
                if dashed {
                    let clip = p.clip_rect().expand(stroke.width * 0.5 + 1.);
                    for segment in visible_column_dashes(a, b, clip) {
                        p.line_segment(segment, stroke);
                    }
                } else {
                    p.line_segment([a, b], stroke);
                }
            }
        }
        if plan.cut.len() >= 3 {
            p.add(egui::Shape::convex_polygon(
                plan.cut.iter().map(|v| screen(*v)).collect(),
                fill,
                stroke,
            ));
        }
    }
    pub(super) fn column_positioning_menu(&mut self, ui: &mut Ui) {
        egui::containers::menu::MenuButton::new(
            self.language.translate("Column positioning and display"),
        )
        .config(
            egui::containers::menu::MenuConfig::new()
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
        )
        .ui(ui, |ui| {
            ui.add_enabled_ui(self.host.document.editor.anchor.is_none(), |ui| {
                ui.horizontal(|ui| {
                    let before = self.host.document.editor.column_two_click;
                    ui.selectable_value(
                        &mut self.host.document.editor.column_two_click,
                        false,
                        self.language.translate("Single-click column"),
                    );
                    ui.selectable_value(
                        &mut self.host.document.editor.column_two_click,
                        true,
                        self.language.translate("Two-click column"),
                    );
                    if before != self.host.document.editor.column_two_click {
                        self.host.document.editor.status =
                            if self.host.document.editor.column_two_click {
                                "Column · Click the base axis point"
                            } else {
                                "Column · Click to place a centered column"
                            }
                            .into();
                    }
                });
            });
            ui.add_enabled_ui(!self.host.document.editor.column_two_click, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.host.document.editor.column_slanted,
                        false,
                        self.language.translate("Vertical column"),
                    );
                    ui.selectable_value(
                        &mut self.host.document.editor.column_slanted,
                        true,
                        self.language.translate("Slanted column"),
                    );
                });
                if self.host.document.editor.column_slanted {
                    ui.horizontal(|ui| {
                        let label = ui.label(self.language.translate("Column inclination"));
                        ui.add(
                            DragValue::new(
                                &mut self.host.document.editor.column_inclination.slant_degrees,
                            )
                            .speed(1.)
                            .suffix("°"),
                        )
                        .labelled_by(label.id);
                    });
                    ui.horizontal(|ui| {
                        let label = ui.label(self.language.translate("Column direction"));
                        ui.add(
                            DragValue::new(
                                &mut self
                                    .host
                                    .document
                                    .editor
                                    .column_inclination
                                    .direction_degrees,
                            )
                            .speed(1.)
                            .suffix("°"),
                        )
                        .labelled_by(label.id);
                    });
                }
            });
            if (self.host.document.editor.column_slanted
                || self.host.document.editor.column_two_click)
                && let Err(error) = self
                    .host
                    .document
                    .editor
                    .project
                    .plan_options
                    .floor_plan_cut
                    .resolve(
                        &self.host.document.editor.project,
                        self.host.document.editor.story,
                        self.host.document.editor.column_plan,
                    )
            {
                ui.colored_label(
                    Color32::from_rgb(190, 65, 55),
                    self.language.translate(&error),
                );
            }
            ui.separator();
            ui.label(self.language.translate("Column plan display"));
            use crate::column_plan::{ColumnPlanDisplay as D, ColumnProjection as P};
            for (value, label) in [
                (D::Projected, "Projected"),
                (D::ProjectedWithOverhead, "Projected with overhead"),
                (D::CutOnly, "Cut only"),
            ] {
                ui.selectable_value(
                    &mut self.host.document.editor.column_plan.display,
                    value,
                    self.language.translate(label),
                );
            }
            if self.host.document.editor.column_plan.display != D::CutOnly {
                ui.separator();
                ui.label(self.language.translate("Column projection range"));
                for (value, label) in [
                    (P::FloorPlanRange, "To floor plan range"),
                    (P::EntireElement, "Entire element"),
                    (P::AboveAbsoluteLimit, "Above absolute display limit"),
                ] {
                    ui.selectable_value(
                        &mut self.host.document.editor.column_plan.projection,
                        value,
                        self.language.translate(label),
                    );
                }
            }
        });
    }
}

#[cfg(test)]
mod dash_tests {
    use super::*;

    #[test]
    fn column_dashes_preserve_phase_and_reverse_direction_after_clipping() {
        let clip = Rect::from_min_max(pos2(0., 0.), pos2(20., 20.));
        assert_eq!(
            visible_column_dashes(pos2(-11., 10.), pos2(30., 10.), clip),
            vec![
                [pos2(0., 10.), pos2(2., 10.)],
                [pos2(5., 10.), pos2(10., 10.)],
                [pos2(13., 10.), pos2(18., 10.)],
            ]
        );
        assert_eq!(
            visible_column_dashes(pos2(30., 10.), pos2(-11., 10.), clip),
            vec![
                [pos2(20., 10.), pos2(17., 10.)],
                [pos2(14., 10.), pos2(9., 10.)],
                [pos2(6., 10.), pos2(1., 10.)],
            ]
        );
        // A clipped interval lying wholly within a gap must stay empty.
        assert!(
            visible_column_dashes(
                pos2(0., 10.),
                pos2(30., 10.),
                Rect::from_min_max(pos2(5.5, 0.), pos2(7.5, 20.))
            )
            .is_empty()
        );
    }

    #[test]
    fn column_dashes_bound_large_offscreen_edges_and_handle_degenerate_input() {
        let clip = Rect::from_min_max(pos2(0., 0.), pos2(800., 600.));
        let lines =
            visible_column_dashes(pos2(-100_000_000., 300.), pos2(100_000_000., 300.), clip);
        assert_eq!(lines.len(), 100);
        assert_eq!(lines[0], [pos2(0., 300.), pos2(5., 300.)]);
        assert_eq!(lines[99], [pos2(792., 300.), pos2(797., 300.)]);
        let diagonal = visible_column_dashes(
            pos2(-10_000_000., -10_000_000.),
            pos2(10_000_000., 10_000_000.),
            clip,
        );
        assert!(!diagonal.is_empty() && diagonal.len() <= 108);
        for line in diagonal {
            for point in line {
                assert!(clip.contains(point));
                assert!((point.x - point.y).abs() < 0.001);
            }
        }
        for (a, b) in [
            (pos2(-100_000_000., -1.), pos2(100_000_000., -1.)),
            (pos2(10., 10.), pos2(10., 10.)),
            (pos2(f64::NAN, 10.), pos2(20., 10.)),
        ] {
            assert!(visible_column_dashes(a, b, clip).is_empty());
        }
        assert_eq!(
            visible_column_dashes(pos2(2., 2.), pos2(5., 6.), clip),
            vec![[pos2(2., 2.), pos2(5., 6.)]]
        );
    }

    #[test]
    fn column_overhead_painter_only_emits_viewport_sized_linework() {
        let ctx = egui::Context::default();
        let rect = Rect::from_min_max(pos2(0., 0.), pos2(800., 600.));
        let plan = crate::column_plan::ColumnPlanGeometry {
            overhead: vec![
                Point::new(-100_000_000., 300.),
                Point::new(100_000_000., 300.),
            ],
            ..Default::default()
        };
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            },
            |ui| {
                AtelierApp::paint_column_plan_geometry_mapped(
                    &ui.painter().with_clip_rect(rect),
                    &plan,
                    |p| pos2(p.x, p.y),
                    Color32::GRAY,
                    Stroke::new(1., Color32::BLACK),
                );
            },
        );
        output.textures_delta.clear();
        let lines: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|s| {
                if let egui::Shape::LineSegment { points, .. } = &s.shape {
                    Some(points)
                } else {
                    None
                }
            })
            .collect();
        assert!(!lines.is_empty() && lines.len() <= 204);
        for line in lines {
            assert!(line.iter().all(|p| rect.expand(1.5).contains(*p)));
        }
    }
}
