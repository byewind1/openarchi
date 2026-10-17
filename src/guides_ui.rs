// Native guide controls. Geometry and serialized data live outside the event layer.
use crate::guides::{
    self, DistanceReference, GuidePrimitive, GuideSettings, PersistentGuide, SnapMethod,
    SnapOrigin, SnapScope,
};

const GUIDE_COLOR: Color32 = Color32::from_rgb(222, 132, 28);
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum GuideDraw {
    #[default]
    Segment,
    Line,
    Circle,
    Arc,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum GuideAction {
    Create(GuideDraw),
    Erase,
}
#[derive(Default)]
struct GuideUi {
    action: Option<GuideAction>,
    start: Option<Point>,
    arc_end: Option<Point>,
    values: Option<GuideSettings>,
    offset_edit: Option<(usize, GuidePrimitive, f32)>,
    error: String,
    references: Vec<GuidePrimitive>,
    points: Vec<Point>,
    points_until: f64,
    hover_reference: Option<(Point, f64)>,
    hovered: Option<(usize, Point)>,
    drag: Option<(Project, usize, Point)>,
    tab_drag: Option<bool>,
    erase_before: Option<Project>,
    context: Option<(u32, Tool, View, u64, usize, usize)>,
    distance_candidates: Vec<crate::distance_guides::DistanceMeasurement>,
    distance_edit: Option<(
        crate::distance_guides::DistanceMeasurement,
        Option<u64>,
        f32,
    )>,
}
impl AtelierApp {
    fn guides_menu(&mut self, ui: &mut Ui) {
        let before = self.editor.project.clone();
        let lang = self.language;
        let settings = &mut self.editor.project.guide_settings;
        ui.checkbox(
            &mut settings.snap_enabled,
            lang.translate("Snap Guides and Points"),
        );
        ui.separator();
        for (method, label) in [
            (SnapMethod::Half, lang.translate("Half").to_string()),
            (
                SnapMethod::Divisions,
                format!("{} ({})", lang.translate("Divisions"), settings.divisions),
            ),
            (
                SnapMethod::Percent,
                format!("{} ({:.2}%)", lang.translate("Percent"), settings.percent),
            ),
            (
                SnapMethod::Distance,
                format!(
                    "{} ({:.3} m)",
                    lang.translate("Distance"),
                    settings.distance
                ),
            ),
            (
                SnapMethod::BestDivision,
                format!(
                    "{} ({:.3} m)",
                    lang.translate("Best Division by Distance"),
                    settings.distance
                ),
            ),
        ] {
            ui.radio_value(&mut settings.method, method, label);
        }
        if ui
            .button(lang.translate("Set Snap Point Values…"))
            .clicked()
        {
            self.guide_ui.values = Some(settings.clone());
            self.guide_ui.error.clear();
            ui.close();
        }
        ui.separator();
        for (scope, label) in [
            (SnapScope::BetweenNodes, "Between Nodes"),
            (
                SnapScope::BetweenIntersections,
                "Between Intersection Points",
            ),
            (SnapScope::EntireElement, "Along Entire Element"),
        ] {
            ui.radio_value(&mut settings.scope, scope, lang.translate(label));
        }
        if matches!(settings.method, SnapMethod::Percent | SnapMethod::Distance) {
            ui.menu_button(lang.translate("Snap Point Origin"), |ui| {
                ui.radio_value(
                    &mut settings.origin,
                    SnapOrigin::Endpoint,
                    lang.translate("Start from Endpoint"),
                );
                ui.radio_value(
                    &mut settings.origin,
                    SnapOrigin::Midpoint,
                    lang.translate("Start from Midpoint"),
                );
            });
        }
        ui.separator();
        ui.checkbox(
            &mut settings.guide_lines_enabled,
            lang.translate("Guide Lines"),
        );
        let can_draw = settings.guide_lines_enabled && self.view != View::Model;
        if ui
            .add_enabled(
                can_draw,
                Button::new(lang.translate("Create Guide Line Segment")),
            )
            .clicked()
        {
            self.guide_ui.action = Some(GuideAction::Create(GuideDraw::Segment));
            self.guide_ui.start = None;
            self.guide_ui.arc_end = None;
            self.editor.status = lang
                .translate("Guide: click first point, then second point · Esc cancels")
                .into();
            ui.close();
        }
        if ui
            .add_enabled(
                !self.editor.project.guides.is_empty(),
                Button::new(lang.translate("Remove All Guide Lines")),
            )
            .clicked()
        {
            self.editor.project.guides.clear();
            self.guide_ui.drag = None;
            self.guide_ui.action = None;
            ui.close();
        }
        if ui
            .add_enabled(
                can_draw && !self.editor.project.guides.is_empty(),
                Button::new(lang.translate("Erase Guide Lines")),
            )
            .clicked()
        {
            self.guide_ui.action = Some(GuideAction::Erase);
            self.guide_ui.start = None;
            self.guide_ui.arc_end = None;
            self.editor.status = lang
                .translate("Erase guides: click or drag · Esc / double-click exits")
                .into();
            ui.close();
        }
        ui.separator();
        let settings = &mut self.editor.project.guide_settings;
        ui.checkbox(
            &mut settings.distance_guides_enabled,
            lang.translate("Distance Guides"),
        );
        ui.radio_value(
            &mut settings.distance_reference,
            DistanceReference::ReferenceLines,
            lang.translate("To reference lines"),
        );
        ui.radio_value(
            &mut settings.distance_reference,
            DistanceReference::Edges,
            lang.translate("To edges"),
        );
        for label in [
            "To door/window nominal size endpoints",
            "To door/window center",
        ] {
            ui.add_enabled(false, Button::new(lang.translate(label)))
                .on_disabled_hover_text(lang.translate("Door and window model is not implemented"));
        }
        if self.view == View::Model {
            ui.label(lang.translate("Guide editing is available in Floor plan (F2)"));
        }
        self.editor.commit(before);
    }
    fn guides_dialog(&mut self, ctx: &Context) {
        if let Some((index, primitive, mut value)) = self.guide_ui.offset_edit {
            let mut close = false;
            let modal = egui::Modal::new(Id::new("guide_offset")).show(ctx, |ui| {
                ui.heading(self.language.translate("Guide Offset…"));
                ui.add(
                    DragValue::new(&mut value)
                        .speed(0.01)
                        .fixed_decimals(3)
                        .suffix(" m"),
                );
                ui.label(
                    self.language
                        .translate("Positive: left of line / outward from circle"),
                );
                if !self.guide_ui.error.is_empty() {
                    ui.colored_label(Color32::LIGHT_RED, &self.guide_ui.error);
                }
                ui.horizontal(|ui| {
                    if ui.button(self.language.translate("Apply")).clicked() {
                        match primitive.offset(value) {
                            Ok(offset)
                                if self
                                    .editor
                                    .project
                                    .guides
                                    .get(index)
                                    .is_some_and(|g| g.primitive == primitive) =>
                            {
                                let before = self.editor.project.clone();
                                self.editor.project.guides[index].primitive = offset;
                                self.editor.commit(before);
                                close = true;
                            }
                            Ok(_) => self.guide_ui.error = "Guide changed; select it again".into(),
                            Err(error) => self.guide_ui.error = error.into(),
                        }
                    }
                    if ui.button(self.language.translate("Cancel")).clicked() {
                        close = true;
                    }
                });
            });
            self.guide_ui.offset_edit = if close || modal.should_close() {
                None
            } else {
                Some((index, primitive, value))
            };
        }
        if let Some(mut values) = self.guide_ui.values.clone() {
            let mut close = false;
            let modal = egui::Modal::new(Id::new("snap_values")).show(ctx, |ui| {
                ui.heading(self.language.translate("Snap Point Values"));
                Grid::new("snap_values_grid").show(ui, |ui| {
                    ui.label(self.language.translate("Divisions"));
                    ui.add(DragValue::new(&mut values.divisions).range(3..=100));
                    ui.end_row();
                    ui.label(self.language.translate("Percent"));
                    ui.add(DragValue::new(&mut values.percent).speed(0.1).suffix(" %"));
                    ui.end_row();
                    ui.label(self.language.translate("Distance"));
                    ui.add(
                        DragValue::new(&mut values.distance)
                            .speed(0.01)
                            .fixed_decimals(3)
                            .suffix(" m"),
                    );
                    ui.end_row();
                });
                if !self.guide_ui.error.is_empty() {
                    ui.colored_label(Color32::LIGHT_RED, &self.guide_ui.error);
                }
                ui.horizontal(|ui| {
                    if ui.button(self.language.translate("Apply")).clicked() {
                        match values.validate() {
                            Ok(()) => {
                                let before = self.editor.project.clone();
                                self.editor.project.guide_settings = values.clone();
                                self.editor.commit(before);
                                close = true;
                            }
                            Err(error) => self.guide_ui.error = error.into(),
                        }
                    }
                    if ui.button(self.language.translate("Cancel")).clicked() {
                        close = true;
                    }
                });
            });
            self.guide_ui.values = if close || modal.should_close() {
                None
            } else {
                Some(values)
            };
        }
        if let Some((measurement, selected, mut value)) = self.guide_ui.distance_edit.clone() {
            let mut close = false;
            let modal = egui::Modal::new(Id::new("distance_guide_edit")).show(ctx, |ui| {
                ui.heading(self.language.translate("Distance Guides"));
                ui.label(format!(
                    "{} {}",
                    self.language.translate("Reference"),
                    measurement.context_id
                ));
                ui.add(
                    DragValue::new(&mut value)
                        .speed(0.01)
                        .fixed_decimals(3)
                        .suffix(" m"),
                );
                if !self.guide_ui.error.is_empty() {
                    ui.colored_label(Color32::LIGHT_RED, &self.guide_ui.error);
                }
                ui.horizontal(|ui| {
                    if ui.button(self.language.translate("Apply")).clicked() {
                        let result = if let Some(id) = selected {
                            crate::distance_guides::apply_distance(
                                &self.editor.project,
                                &measurement,
                                id,
                                value,
                            )
                            .map(|project| {
                                let before = self.editor.project.clone();
                                self.editor.project = project;
                                self.editor.commit(before);
                                self.draft_id = None;
                            })
                        } else {
                            crate::distance_guides::target_at_distance(&measurement, value).map(
                                |point| {
                                    self.editor.click(point, 0.01);
                                    self.guide_ui.references.clear();
                                },
                            )
                        };
                        match result {
                            Ok(()) => close = true,
                            Err(error) => self.guide_ui.error = error.into(),
                        }
                    }
                    if ui.button(self.language.translate("Cancel")).clicked() {
                        close = true;
                    }
                });
            });
            self.guide_ui.distance_edit = if close || modal.should_close() {
                None
            } else {
                Some((measurement, selected, value))
            };
        }
    }
    fn guide_shortcuts(&mut self, ctx: &Context) -> bool {
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if self.guide_ui.values.take().is_some()
                || self.guide_ui.distance_edit.take().is_some()
                || self.guide_ui.offset_edit.take().is_some()
            {
                return true;
            }
            let active = self.guide_ui.action.take().is_some()
                || self.guide_ui.tab_drag.is_some()
                || self.guide_ui.drag.is_some();
            self.guide_ui.start = None;
            self.guide_ui.arc_end = None;
            self.guide_ui.tab_drag = None;
            if let Some((before, _, _)) = self.guide_ui.drag.take() {
                self.editor.project = before;
            }
            if let Some(before) = self.guide_ui.erase_before.take() {
                self.editor.commit(before);
            }
            if active {
                return true;
            }
            if !self.guide_ui.references.is_empty() {
                self.guide_ui.references.clear();
                self.guide_ui.points.clear();
                return true;
            }
            if let Some((index, _)) = self.guide_ui.hovered.take() {
                if index < self.editor.project.guides.len() {
                    let before = self.editor.project.clone();
                    self.editor.project.guides.remove(index);
                    self.editor.commit(before);
                    return true;
                }
            }
        }
        if self.guide_ui.values.is_some()
            || self.guide_ui.distance_edit.is_some()
            || self.guide_ui.offset_edit.is_some()
        {
            return true;
        }
        if ctx.egui_wants_keyboard_input() {
            return false;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Q))
            && self.editor.project.guide_settings.snap_enabled
        {
            self.toggle_snap_reference(self.hover_point);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::D))
            && self.editor.project.guide_settings.distance_guides_enabled
        {
            if let Some(measurement) = self
                .guide_ui
                .distance_candidates
                .iter()
                .min_by(|a, b| a.distance.total_cmp(&b.distance))
                .copied()
            {
                let selected = if self.editor.tool == Tool::Select {
                    self.editor.selected
                } else {
                    None
                };
                self.guide_ui.distance_edit = Some((measurement, selected, measurement.distance));
                self.guide_ui.error.clear();
                return true;
            }
        }
        false
    }
    fn toggle_snap_reference(&mut self, point: Point) {
        let tolerance = 10. / self.plan_scale;
        let (endpoint, kind) = geometry::snap(
            &self.editor.project,
            self.editor.story,
            point,
            tolerance,
            true,
        );
        let snap_point = self
            .guide_ui
            .points
            .iter()
            .copied()
            .filter(|p| p.distance(point) < tolerance)
            .min_by(|a, b| a.distance(point).total_cmp(&b.distance(point)));
        let node = if kind == SnapKind::Endpoint {
            Some(endpoint)
        } else {
            snap_point
        };
        let references = if let Some(node) = node {
            vec![
                GuidePrimitive::Line {
                    origin: node,
                    direction: Point::new(1., 0.),
                },
                GuidePrimitive::Line {
                    origin: node,
                    direction: Point::new(0., 1.),
                },
            ]
        } else if let Some(reference) = self.reference_at(point, tolerance) {
            vec![reference]
        } else {
            return;
        };
        let remove = references
            .iter()
            .all(|g| self.guide_ui.references.contains(g));
        for reference in references {
            if remove {
                self.guide_ui.references.retain(|g| *g != reference);
            } else if !self.guide_ui.references.contains(&reference)
                && self.guide_ui.references.len() < 64
            {
                self.guide_ui.references.push(reference);
            }
        }
    }
    fn reference_at(&self, point: Point, tolerance: f32) -> Option<GuidePrimitive> {
        let mut nearest = None;
        let mut distance = tolerance;
        for element in self
            .editor
            .project
            .elements
            .iter()
            .filter(|e| e.story == self.editor.story)
        {
            for path in guides::element_paths(element, DistanceReference::ReferenceLines) {
                if let Some(hit) = path.project(point) {
                    if hit.distance < distance {
                        let edge = &path.edges[hit.edge_index];
                        let primitive = if let Some(arc) = edge.arc {
                            // Arc helper returns original circle, never a copied display asset.
                            if let Some((center, _)) =
                                geometry::arc_center_sweep(edge.start, edge.end, arc)
                            {
                                GuidePrimitive::Circle {
                                    center,
                                    radius: arc.radius,
                                }
                            } else {
                                continue;
                            }
                        } else {
                            GuidePrimitive::Line {
                                origin: edge.start,
                                direction: Point::new(
                                    edge.end.x - edge.start.x,
                                    edge.end.y - edge.start.y,
                                ),
                            }
                        };
                        nearest = Some(primitive);
                        distance = hit.distance;
                    }
                }
            }
        }
        nearest
    }
    fn target(&self, p: Point, shift: bool) -> (Point, SnapKind) {
        let settings = &self.editor.project.guide_settings;
        let (base_target, target_kind) = self.base_target(p, shift);
        if self.tracker_locked
            && (self.editor.anchor.is_some() || !self.editor.polyline_points.is_empty())
        {
            return (base_target, target_kind);
        }
        let (mut base, mut kind) = self.apply_grid_snap(p, base_target, target_kind);
        let tolerance = 9. / self.plan_scale;
        let mut best = if kind == SnapKind::Endpoint {
            p.distance(base)
        } else {
            tolerance
        };
        if settings.snap_enabled {
            let fresh = guides::snap_candidates(
                &self.editor.project,
                self.editor.story,
                p,
                tolerance,
                settings,
            );
            for point in fresh.iter().chain(self.guide_ui.points.iter()) {
                if p.distance(*point) < best {
                    base = *point;
                    best = p.distance(*point);
                    kind = SnapKind::SnapPoint;
                }
            }
        }
        let mut primitives: Vec<GuidePrimitive> = if settings.guide_lines_enabled {
            self.editor
                .project
                .guides
                .iter()
                .filter(|g| g.story == self.editor.story)
                .map(|g| g.primitive.clone())
                .collect()
        } else {
            Vec::new()
        };
        if settings.snap_enabled {
            primitives.extend(self.guide_ui.references.clone());
            if let Some(anchor) = self
                .editor
                .anchor
                .or_else(|| self.editor.polyline_points.last().copied())
            {
                for reference in &self.guide_ui.references {
                    if let GuidePrimitive::Line { direction, .. } = reference {
                        primitives.push(GuidePrimitive::Line {
                            origin: anchor,
                            direction: *direction,
                        });
                        primitives.push(GuidePrimitive::Line {
                            origin: anchor,
                            direction: Point::new(-direction.y, direction.x),
                        });
                    }
                }
            }
        }
        let nearby: Vec<_> = primitives
            .iter()
            .filter(|g| g.project(p).is_some_and(|q| q.distance(p) <= tolerance))
            .take(48)
            .collect();
        let mut crossing = None;
        for (i, a) in nearby.iter().enumerate() {
            for b in nearby.iter().skip(i + 1) {
                for point in a.intersections(b) {
                    if point.distance(p) <= tolerance
                        && crossing.is_none_or(|q: Point| point.distance(p) < q.distance(p))
                    {
                        crossing = Some(point);
                    }
                }
            }
        }
        if let Some(point) = crossing {
            if !matches!(kind, SnapKind::Endpoint | SnapKind::SnapPoint) {
                return (point, SnapKind::Guide);
            }
        }
        for primitive in &primitives {
            if let Some(point) = primitive.project(p) {
                let distance = p.distance(point);
                if distance < best && !matches!(kind, SnapKind::Endpoint | SnapKind::SnapPoint) {
                    base = point;
                    best = distance;
                    kind = SnapKind::Guide;
                }
            }
        }
        // Shift extends the nearest visible guide constraint beyond its snap band.
        if shift
            && !primitives.is_empty()
            && !matches!(kind, SnapKind::Endpoint | SnapKind::SnapPoint)
        {
            if let Some(point) = primitives
                .iter()
                .filter_map(|g| g.project(p))
                .min_by(|a, b| p.distance(*a).total_cmp(&p.distance(*b)))
            {
                base = point;
                kind = SnapKind::Guide;
            }
        }
        (base, kind)
    }
    fn add_guide(&mut self, primitive: GuidePrimitive) {
        if let Err(error) = primitive.validate() {
            self.guide_ui.error = error.into();
            return;
        }
        let before = self.editor.project.clone();
        self.editor.project.guides.push(PersistentGuide {
            story: self.editor.story,
            primitive,
        });
        if let Err(error) = self.editor.project.validate() {
            self.editor.project = before;
            self.guide_ui.error = error.into();
            return;
        }
        self.editor.commit(before);
        self.guide_ui.start = None;
        self.guide_ui.arc_end = None;
        self.guide_ui.action = None;
        self.editor.status = self
            .language
            .translate("Guide placed · Undo available")
            .into();
    }
    fn paint_guide(
        &self,
        painter: &Painter,
        rect: Rect,
        primitive: &GuidePrimitive,
        color: Color32,
    ) {
        let points: Vec<Pos2> = match primitive {
            GuidePrimitive::Segment { start, end } => {
                vec![self.screen(rect, *start), self.screen(rect, *end)]
            }
            GuidePrimitive::Line { origin, direction } => {
                let a = self.screen(rect, *origin);
                let d = vec2(direction.x, -direction.y) / direction.x.hypot(direction.y);
                let extent = (a - rect.center()).length() + rect.size().length();
                vec![a - d * extent, a + d * extent]
            }
            GuidePrimitive::Arc { start, end, arc } => geometry::arc_points(*start, *end, *arc)
                .unwrap_or_default()
                .iter()
                .map(|point| self.screen(rect, *point))
                .collect(),
            GuidePrimitive::Circle { center, radius } => (0..=128)
                .map(|i| {
                    let t = i as f32 / 128. * std::f32::consts::TAU;
                    self.screen(
                        rect,
                        Point::new(center.x + radius * t.cos(), center.y + radius * t.sin()),
                    )
                })
                .collect(),
        };
        for pair in points.windows(2) {
            // Clip before dashing: distant guide origins must not allocate unbounded work.
            let a = pair[0];
            let b = pair[1];
            let vector = b - a;
            let length = vector.length();
            if !length.is_finite() || length < 0.01 {
                continue;
            }
            let mut lo: f32 = 0.;
            let mut hi: f32 = 1.;
            for (p, q) in [
                (-vector.x, a.x - rect.left()),
                (vector.x, rect.right() - a.x),
                (-vector.y, a.y - rect.top()),
                (vector.y, rect.bottom() - a.y),
            ] {
                if p.abs() < 1e-7 {
                    if q < 0. {
                        hi = -1.;
                    }
                } else if p < 0. {
                    lo = lo.max(q / p);
                } else {
                    hi = hi.min(q / p);
                }
            }
            if lo > hi {
                continue;
            }
            let direction = vector / length;
            let mut offset = lo * length;
            while offset < hi * length {
                painter.line_segment(
                    [
                        a + direction * offset,
                        a + direction * (offset + 6.).min(hi * length),
                    ],
                    Stroke::new(1., color),
                );
                offset += 10.;
            }
        }
    }
    fn guide_canvas(&mut self, ui: &mut Ui, rect: Rect, response: &Response) -> bool {
        if self.guide_ui.values.is_some()
            || self.guide_ui.distance_edit.is_some()
            || self.guide_ui.offset_edit.is_some()
        {
            return true;
        }
        let painter = ui.painter_at(rect);
        let now = ui.input(|i| i.time);
        let context = (
            self.editor.story,
            self.editor.tool,
            self.view,
            self.editor.project.next_id,
            self.editor.undo.len(),
            self.editor.redo.len(),
        );
        if self.guide_ui.context.is_some_and(|old| old != context) {
            self.guide_ui.references.clear();
            self.guide_ui.points.clear();
            self.guide_ui.hover_reference = None;
        }
        self.guide_ui.context = Some(context);
        let pointer = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| rect.contains(*p));
        let raw = pointer.map(|p| self.world(rect, p));
        let settings = self.editor.project.guide_settings.clone();
        let tolerance = 9. / self.plan_scale;
        if settings.snap_enabled {
            if let Some(raw) = raw {
                let points = guides::snap_candidates(
                    &self.editor.project,
                    self.editor.story,
                    raw,
                    tolerance,
                    &settings,
                );
                if !points.is_empty() {
                    self.guide_ui.points = points;
                    self.guide_ui.points_until = now + 5.;
                }
                if response.hovered()
                    && self.guide_ui.action.is_none()
                    && self.guide_ui.drag.is_none()
                {
                    if let Some((prior, since)) = self.guide_ui.hover_reference {
                        if prior.distance(raw) > tolerance {
                            self.guide_ui.hover_reference = Some((raw, now));
                        } else if now - since > 0.6 && now - since < 1.0 {
                            if let Some(reference) = self.reference_at(raw, tolerance) {
                                if !self.guide_ui.references.contains(&reference) {
                                    if self.guide_ui.references.len() < 64 {
                                        self.guide_ui.references.push(reference);
                                    }
                                }
                            }
                            self.guide_ui.hover_reference = Some((raw, now + 60.));
                        }
                    } else {
                        self.guide_ui.hover_reference = Some((raw, now));
                    }
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(100));
                }
            }
            if now > self.guide_ui.points_until {
                self.guide_ui.points.clear();
            }
            for point in &self.guide_ui.points {
                let p = self.screen(rect, *point);
                painter.line_segment(
                    [p - vec2(3., 3.), p + vec2(3., 3.)],
                    Stroke::new(1.5, ACCENT),
                );
                painter.line_segment(
                    [p + vec2(-3., 3.), p + vec2(3., -3.)],
                    Stroke::new(1.5, ACCENT),
                );
            }
            for reference in &self.guide_ui.references {
                self.paint_guide(&painter, rect, reference, ACCENT);
            }
        } else {
            self.guide_ui.points.clear();
            self.guide_ui.references.clear();
        }
        self.guide_ui.hovered = None;
        let mut nearest = tolerance;
        if settings.guide_lines_enabled {
            for (i, guide) in self
                .editor
                .project
                .guides
                .iter()
                .enumerate()
                .filter(|(_, g)| g.story == self.editor.story)
            {
                self.paint_guide(&painter, rect, &guide.primitive, GUIDE_COLOR);
                if let Some(raw) = raw {
                    if let Some(point) = guide.primitive.project(raw) {
                        let distance = raw.distance(point);
                        if distance < nearest {
                            self.guide_ui.hovered = Some((i, point));
                            nearest = distance;
                        }
                    }
                }
            }
        }
        // Fixed guide tabs: dragging from a horizontal edge creates a horizontal
        // line; vertical edges create vertical lines. Drop on any tab cancels.
        let tabs = [
            Rect::from_center_size(pos2(rect.center().x, rect.top() + 6.), vec2(36., 12.)),
            Rect::from_center_size(pos2(rect.center().x, rect.bottom() - 6.), vec2(36., 12.)),
            Rect::from_center_size(pos2(rect.left() + 6., rect.center().y), vec2(12., 36.)),
            Rect::from_center_size(pos2(rect.right() - 6., rect.center().y), vec2(12., 36.)),
        ];
        if settings.guide_lines_enabled {
            for (index, tab) in tabs.iter().enumerate() {
                painter.rect_filled(*tab, 3., GUIDE_COLOR);
                let tab_response = ui.interact(*tab, Id::new(("guide_tab", index)), Sense::drag());
                tab_response
                    .clone()
                    .on_hover_text(self.language.translate("Drag a guide into the plan"));
                if tab_response.drag_started() {
                    self.guide_ui.tab_drag = Some(index < 2);
                    self.guide_ui.action = None;
                }
                tab_response.context_menu(|ui| self.guides_menu(ui));
            }
        }
        if let Some(horizontal) = self.guide_ui.tab_drag {
            if let Some(raw) = raw {
                let primitive = self
                    .reference_at(raw, tolerance)
                    .unwrap_or(GuidePrimitive::Line {
                        origin: raw,
                        direction: if horizontal {
                            Point::new(1., 0.)
                        } else {
                            Point::new(0., 1.)
                        },
                    });
                self.paint_guide(&painter, rect, &primitive, GUIDE_COLOR);
                if ui.input(|i| i.pointer.any_released()) {
                    if pointer.is_some_and(|p| !tabs.iter().any(|r| r.contains(p))) {
                        self.add_guide(primitive);
                    }
                    self.guide_ui.tab_drag = None;
                }
            } else if ui.input(|i| i.pointer.any_released()) {
                self.guide_ui.tab_drag = None;
            }
            return true;
        }
        if let Some((before, index, start)) = self.guide_ui.drag.clone() {
            if let Some(raw) = raw {
                if let Some(original) = before.guides.get(index) {
                    let delta = Point::new(raw.x - start.x, raw.y - start.y);
                    let translated = match original.primitive {
                        GuidePrimitive::Segment { start, end } => GuidePrimitive::Segment {
                            start: Point::new(start.x + delta.x, start.y + delta.y),
                            end: Point::new(end.x + delta.x, end.y + delta.y),
                        },
                        GuidePrimitive::Line { origin, direction } => GuidePrimitive::Line {
                            origin: Point::new(origin.x + delta.x, origin.y + delta.y),
                            direction,
                        },
                        GuidePrimitive::Circle { center, .. } => GuidePrimitive::Circle {
                            center,
                            radius: center.distance(raw),
                        },
                        GuidePrimitive::Arc { start, end, arc } => GuidePrimitive::Arc {
                            start: Point::new(start.x + delta.x, start.y + delta.y),
                            end: Point::new(end.x + delta.x, end.y + delta.y),
                            arc,
                        },
                    };
                    if translated.validate().is_ok() {
                        self.editor.project.guides[index].primitive = translated;
                    }
                }
            }
            if ui.input(|i| i.pointer.any_released()) {
                if pointer.is_some_and(|p| tabs.iter().any(|r| r.contains(p))) {
                    self.editor.project.guides.remove(index);
                }
                if self.editor.project.validate().is_ok() {
                    self.editor.commit(before);
                } else {
                    self.editor.project = before;
                }
                self.guide_ui.drag = None;
            }
            return true;
        }
        if let Some(action) = self.guide_ui.action {
            if action == GuideAction::Erase {
                ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
                if let Some(raw) = raw {
                    if response.clicked() || response.dragged_by(PointerButton::Primary) {
                        if self.guide_ui.erase_before.is_none() {
                            self.guide_ui.erase_before = Some(self.editor.project.clone());
                        }
                        let delta = ui.input(|i| i.pointer.delta());
                        let previous = Point::new(
                            raw.x - delta.x / self.plan_scale,
                            raw.y + delta.y / self.plan_scale,
                        );
                        let stroke = GuidePrimitive::Segment {
                            start: previous,
                            end: raw,
                        };
                        self.editor.project.guides.retain(|g| {
                            g.story != self.editor.story
                                || (g
                                    .primitive
                                    .project(raw)
                                    .is_none_or(|p| p.distance(raw) > tolerance)
                                    && g.primitive.intersections(&stroke).is_empty())
                        });
                    }
                }
                if ui.input(|i| i.pointer.any_released()) {
                    if let Some(before) = self.guide_ui.erase_before.take() {
                        self.editor.commit(before);
                    }
                }
                if response.double_clicked() {
                    self.guide_ui.action = None;
                }
                return true;
            }
            if let GuideAction::Create(mut draw) = action {
                let mut palette_hovered = false;
                egui::Area::new(Id::new("guide_draw_palette"))
                    .fixed_pos(rect.left_top() + vec2(24., 24.))
                    .show(ui.ctx(), |ui| {
                        Frame::popup(ui.style()).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for (kind, label) in [
                                    (GuideDraw::Segment, "Segment"),
                                    (GuideDraw::Line, "Infinite Line"),
                                    (GuideDraw::Circle, "Circle"),
                                    (GuideDraw::Arc, "Arc (3 points)"),
                                ] {
                                    ui.selectable_value(
                                        &mut draw,
                                        kind,
                                        self.language.translate(label),
                                    );
                                }
                            });
                            ui.label(self.language.translate(
                                "Guide: click first point, then second point · Esc cancels",
                            ));
                            if !self.guide_ui.error.is_empty() {
                                ui.colored_label(Color32::LIGHT_RED, &self.guide_ui.error);
                            }
                            palette_hovered = ui.rect_contains_pointer(ui.min_rect());
                        });
                    });
                self.guide_ui.action = Some(GuideAction::Create(draw));
                if let Some(raw) = raw {
                    let (point, _) = self.target(raw, ui.input(|i| i.modifiers.shift));
                    let primitive = self.guide_ui.start.and_then(|start| {
                        Some(match draw {
                            GuideDraw::Segment => GuidePrimitive::Segment { start, end: point },
                            GuideDraw::Line => GuidePrimitive::Line {
                                origin: start,
                                direction: Point::new(point.x - start.x, point.y - start.y),
                            },
                            GuideDraw::Circle => GuidePrimitive::Circle {
                                center: start,
                                radius: start.distance(point),
                            },
                            GuideDraw::Arc => {
                                if let Some(end) = self.guide_ui.arc_end {
                                    GuidePrimitive::Arc {
                                        start,
                                        end,
                                        arc: geometry::arc_segment_through(start, end, point)?,
                                    }
                                } else {
                                    GuidePrimitive::Segment { start, end: point }
                                }
                            }
                        })
                    });
                    if let Some(primitive) = &primitive {
                        if primitive.validate().is_ok() {
                            self.paint_guide(&painter, rect, primitive, GUIDE_COLOR);
                        }
                    }
                    if response.clicked() && !palette_hovered {
                        if self.guide_ui.start.is_none() {
                            self.guide_ui.start = Some(point);
                        } else if draw == GuideDraw::Arc && self.guide_ui.arc_end.is_none() {
                            if self
                                .guide_ui
                                .start
                                .is_some_and(|start| start.distance(point) > 0.001)
                            {
                                self.guide_ui.arc_end = Some(point);
                            }
                        } else if let Some(primitive) = primitive {
                            self.add_guide(primitive);
                        } else {
                            self.guide_ui.error = self
                                .language
                                .translate("Arc requires three non-collinear points")
                                .into();
                        }
                    }
                }
                ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
                return true;
            }
        }
        if self.editor.tool == Tool::Select
            && settings.guide_lines_enabled
            && response.drag_started_by(PointerButton::Primary)
            && !ui.input(|i| i.key_down(Key::Space))
        {
            if let Some(start) = ui
                .input(|i| i.pointer.press_origin())
                .filter(|p| rect.contains(*p))
                .map(|p| self.world(rect, p))
            {
                let hit = self
                    .editor
                    .project
                    .guides
                    .iter()
                    .enumerate()
                    .filter(|(_, g)| g.story == self.editor.story)
                    .filter_map(|(i, g)| g.primitive.project(start).map(|p| (i, p.distance(start))))
                    .filter(|(_, d)| *d < tolerance)
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((index, _)) = hit {
                    self.guide_ui.drag = Some((self.editor.project.clone(), index, start));
                    self.drag = None;
                    return true;
                }
            }
        }
        if let Some((index, point)) = self.guide_ui.hovered {
            if self.editor.tool == Tool::Select {
                let pos = self.screen(rect, point);
                painter.circle_filled(pos, 4.5, GUIDE_COLOR);
                // Handle is projected under the cursor; picking it must not select a wall.
                if response.drag_started_by(PointerButton::Primary) {
                    self.guide_ui.drag = Some((
                        self.editor.project.clone(),
                        index,
                        ui.input(|i| i.pointer.press_origin())
                            .map(|p| self.world(rect, p))
                            .unwrap_or(point),
                    ));
                    self.drag = None;
                    return true;
                }
                response.context_menu(|ui| {
                    if ui
                        .button(self.language.translate("Guide Offset…"))
                        .clicked()
                    {
                        self.guide_ui.offset_edit =
                            Some((index, self.editor.project.guides[index].primitive, 0.));
                        self.guide_ui.error.clear();
                        ui.close();
                    }
                    if ui
                        .button(self.language.translate("Remove Guide Line"))
                        .clicked()
                    {
                        let before = self.editor.project.clone();
                        self.editor.project.guides.remove(index);
                        self.editor.commit(before);
                        ui.close();
                    }
                    if ui
                        .button(self.language.translate("Remove All Guide Lines"))
                        .clicked()
                    {
                        let before = self.editor.project.clone();
                        self.editor.project.guides.clear();
                        self.editor.commit(before);
                        ui.close();
                    }
                });
            }
        }
        self.guide_ui.distance_candidates.clear();
        if settings.distance_guides_enabled {
            let selected = self.editor.selected.filter(|_| {
                self.editor.selection_ids().len() == 1 && self.editor.tool == Tool::Select
            });
            let wall_selected = selected.is_some_and(|id| {
                self.editor
                    .project
                    .elements
                    .iter()
                    .any(|e| e.id == id && matches!(e.shape, Shape::Wall { .. }))
            });
            if wall_selected || self.editor.tool == Tool::Wall {
                let source = raw.unwrap_or(self.hover_point);
                let measurements = crate::distance_guides::measurements(
                    &self.editor.project,
                    self.editor.story,
                    source,
                    selected,
                    settings.distance_reference == DistanceReference::Edges,
                );
                self.guide_ui.distance_candidates = measurements.clone();
                let mut consumed = false;
                for (i, measurement) in measurements.iter().enumerate() {
                    let a = self.screen(rect, measurement.source);
                    let b = self.screen(rect, measurement.target);
                    if !rect.contains(a) || !rect.contains(b) {
                        continue;
                    }
                    painter.line_segment([a, b], Stroke::new(1., ACCENT));
                    let direction = (b - a).normalized();
                    let side = vec2(-direction.y, direction.x);
                    for (p, d) in [(a, direction), (b, -direction)] {
                        painter.line_segment([p, p + d * 7. + side * 3.], Stroke::new(1., ACCENT));
                        painter.line_segment([p, p + d * 7. - side * 3.], Stroke::new(1., ACCENT));
                    }
                    let label = Rect::from_center_size(a.lerp(b, 0.5), vec2(76., 22.));
                    painter.rect_filled(label, 3., PAPER);
                    painter.text(
                        label.center(),
                        Align2::CENTER_CENTER,
                        format!("{:.3} m", measurement.distance),
                        FontId::monospace(11.),
                        ACCENT,
                    );
                    let label_response =
                        ui.interact(label, Id::new(("distance_label", i)), Sense::click());
                    label_response
                        .clone()
                        .on_hover_text(self.language.translate("Click to edit distance"));
                    if wall_selected
                        && (label_response.clicked()
                            || (response.clicked() && pointer.is_some_and(|p| label.contains(p))))
                    {
                        self.guide_ui.distance_edit =
                            Some((*measurement, selected, measurement.distance));
                        self.guide_ui.error.clear();
                        consumed = true;
                    }
                }
                if consumed {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod guide_ui_tests {
    use super::*;
    fn setup() -> (Context, AtelierApp) {
        let ctx = Context::default();
        AtelierApp::style(&ctx);
        let mut app = AtelierApp::from_renderer(None, "test".into());
        app.editor = Editor::new(Project::default());
        app.plan_center = Point::default();
        app.plan_scale = 50.;
        (ctx, app)
    }
    fn frame(ctx: &Context, app: &mut AtelierApp, events: Vec<Event>) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
                events,
                ..Default::default()
            },
            |ui| {
                app.shortcuts(ui.ctx());
                CentralPanel::default()
                    .frame(Frame::NONE)
                    .show(ui, |ui| app.plan(ui));
                app.guides_dialog(ui.ctx());
            },
        );
        output.textures_delta.clear();
        output
    }
    fn button(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }
    fn click(ctx: &Context, app: &mut AtelierApp, pos: Pos2) {
        frame(ctx, app, vec![Event::PointerMoved(pos)]);
        frame(ctx, app, vec![button(pos, true)]);
        frame(ctx, app, vec![button(pos, false)]);
    }
    fn key(ctx: &Context, app: &mut AtelierApp, key: Key) {
        frame(
            ctx,
            app,
            vec![Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        frame(
            ctx,
            app,
            vec![Event::Key {
                key,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
    }
    #[test]
    fn guide_segment_two_clicks_undo_redo_and_escape_leave_wall_draft_intact() {
        let (ctx, mut app) = setup();
        app.editor.set_tool(Tool::Wall);
        app.editor.anchor = Some(Point::new(-4., -4.));
        app.guide_ui.action = Some(GuideAction::Create(GuideDraw::Segment));
        click(&ctx, &mut app, pos2(400., 300.));
        click(&ctx, &mut app, pos2(550., 250.));
        assert_eq!(app.editor.project.guides.len(), 1);
        assert!(app.editor.project.elements.is_empty());
        assert_eq!(app.editor.undo.len(), 1);
        assert_eq!(app.editor.anchor, Some(Point::new(-4., -4.)));
        let primitive = app.editor.project.guides[0].primitive;
        assert_eq!(
            primitive,
            GuidePrimitive::Segment {
                start: Point::new(0., 0.),
                end: Point::new(3., 1.)
            }
        );
        app.editor.undo();
        assert!(app.editor.project.guides.is_empty());
        app.editor.redo();
        assert_eq!(app.editor.project.guides[0].primitive, primitive);
        app.editor.anchor = Some(Point::new(-4., -4.));
        app.guide_ui.action = Some(GuideAction::Create(GuideDraw::Circle));
        click(&ctx, &mut app, pos2(450., 400.));
        key(&ctx, &mut app, Key::Escape);
        assert_eq!(app.editor.project.guides.len(), 1);
        assert_eq!(app.editor.tool, Tool::Wall);
        assert_eq!(app.editor.anchor, Some(Point::new(-4., -4.)));
    }
    #[test]
    fn circle_and_three_point_arc_create_exact_geometry() {
        let (ctx, mut app) = setup();
        app.editor.project.guide_settings.snap_enabled = false;
        app.guide_ui.action = Some(GuideAction::Create(GuideDraw::Circle));
        click(&ctx, &mut app, pos2(400., 300.));
        click(&ctx, &mut app, pos2(500., 300.));
        assert_eq!(
            app.editor.project.guides[0].primitive,
            GuidePrimitive::Circle {
                center: Point::new(0., 0.),
                radius: 2.
            }
        );
        app.editor.project.guides.clear();
        app.guide_ui.action = Some(GuideAction::Create(GuideDraw::Arc));
        click(&ctx, &mut app, pos2(400., 300.));
        click(&ctx, &mut app, pos2(600., 300.));
        assert!(app.editor.project.guides.is_empty());
        click(&ctx, &mut app, pos2(500., 200.));
        let GuidePrimitive::Arc { start, end, arc } = app.editor.project.guides[0].primitive else {
            panic!("arc expected")
        };
        assert!((arc.radius - 2.).abs() < 1e-5);
        assert_eq!(start, Point::new(0., 0.));
        assert_eq!(end, Point::new(4., 0.));
        assert!(
            app.editor.project.guides[0]
                .primitive
                .project(Point::new(2., 2.))
                .unwrap()
                .distance(Point::new(2., 2.))
                < 1e-5
        );
    }
    #[test]
    fn eraser_sweep_crosses_guides_between_pointer_samples_and_undoes_once() {
        let (ctx, mut app) = setup();
        for x in [-1., 1.] {
            app.editor.project.guides.push(PersistentGuide {
                story: 0,
                primitive: GuidePrimitive::Line {
                    origin: Point::new(x, 0.),
                    direction: Point::new(0., 1.),
                },
            });
        }
        app.guide_ui.action = Some(GuideAction::Erase);
        let a = pos2(300., 300.);
        let b = pos2(500., 300.);
        frame(&ctx, &mut app, vec![Event::PointerMoved(a)]);
        frame(&ctx, &mut app, vec![button(a, true)]);
        frame(&ctx, &mut app, vec![Event::PointerMoved(b)]);
        frame(&ctx, &mut app, vec![button(b, false)]);
        assert!(app.editor.project.guides.is_empty());
        assert_eq!(app.editor.undo.len(), 1);
        app.editor.undo();
        assert_eq!(app.editor.project.guides.len(), 2);
        key(&ctx, &mut app, Key::Escape);
        assert!(app.guide_ui.action.is_none());
    }
    #[test]
    fn guide_handle_drag_previews_commits_and_cancel_restores() {
        let (ctx, mut app) = setup();
        app.add_guide(GuidePrimitive::Line {
            origin: Point::new(0., 0.),
            direction: Point::new(1., 0.),
        });
        let a = pos2(500., 300.);
        let b = pos2(500., 250.);
        frame(&ctx, &mut app, vec![Event::PointerMoved(a)]);
        frame(&ctx, &mut app, vec![button(a, true)]);
        frame(&ctx, &mut app, vec![Event::PointerMoved(b)]);
        frame(&ctx, &mut app, vec![button(b, false)]);
        assert_eq!(
            app.editor.project.guides[0]
                .primitive
                .project(Point::new(0., 0.))
                .unwrap(),
            Point::new(0., 1.)
        );
        assert_eq!(app.editor.undo.len(), 2);
        frame(&ctx, &mut app, vec![Event::PointerMoved(b)]);
        frame(&ctx, &mut app, vec![button(b, true)]);
        frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(500., 200.))]);
        key(&ctx, &mut app, Key::Escape);
        assert_eq!(
            app.editor.project.guides[0]
                .primitive
                .project(Point::new(0., 0.))
                .unwrap(),
            Point::new(0., 1.)
        );
        frame(&ctx, &mut app, vec![button(pos2(500., 200.), false)]);
    }
    #[test]
    fn snap_points_and_fixed_guides_are_independent_from_element_and_grid_snap() {
        let (_, mut app) = setup();
        app.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(0., 0.), Point::new(3., 0.)],
                arcs: vec![],
            },
        );
        app.editor.project.guide_settings.method = SnapMethod::Divisions;
        app.editor.snap = false;
        app.editor.project.grid_settings.grid_snap_enabled = false;
        let (point, kind) = app.target(Point::new(1.01, 0.02), false);
        assert_eq!(point, Point::new(1., 0.));
        assert_eq!(kind, SnapKind::SnapPoint);
        app.editor.project.guide_settings.snap_enabled = false;
        let raw = Point::new(1.01, 0.02);
        assert_eq!(app.target(raw, false), (raw, SnapKind::Free));
        app.add_guide(GuidePrimitive::Line {
            origin: Point::new(0., 1.),
            direction: Point::new(1., 0.),
        });
        assert_eq!(
            app.target(Point::new(1.23, 1.05), false),
            (Point::new(1.23, 1.), SnapKind::Guide)
        );
        app.editor.project.guide_settings.guide_lines_enabled = false;
        assert_eq!(app.target(Point::new(1.23, 1.05), false).1, SnapKind::Free);
    }
    #[test]
    fn guide_intersections_snap_to_one_joint_not_one_line_projection() {
        let (_, mut app) = setup();
        app.editor.snap = false;
        app.editor.project.grid_settings.grid_snap_enabled = false;
        for direction in [Point::new(1., 0.), Point::new(0., 1.)] {
            app.add_guide(GuidePrimitive::Line {
                origin: Point::new(2., 2.),
                direction,
            });
        }
        assert_eq!(
            app.target(Point::new(2.03, 2.04), false),
            (Point::new(2., 2.), SnapKind::Guide)
        );
    }
    #[test]
    fn settings_modal_escape_leaves_project_and_undo_unchanged() {
        let (ctx, mut app) = setup();
        let before = app.editor.project.clone();
        let mut draft = before.guide_settings.clone();
        draft.divisions = 17;
        app.guide_ui.values = Some(draft);
        frame(&ctx, &mut app, vec![]);
        key(&ctx, &mut app, Key::Escape);
        assert!(app.guide_ui.values.is_none());
        assert_eq!(app.editor.project, before);
        assert!(app.editor.undo.is_empty());
    }
    #[test]
    fn distance_keyboard_entry_freezes_input_reference_and_escape_cancels() {
        let (ctx, mut app) = setup();
        app.editor.project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(-3., -2.),
                end: Point::new(3., -2.),
                thickness: 0.2,
                height: 3.,
            },
        );
        app.editor.set_tool(Tool::Wall);
        frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(400., 300.))]);
        assert!(!app.guide_ui.distance_candidates.is_empty());
        let before = app.editor.project.clone();
        key(&ctx, &mut app, Key::D);
        assert!(app.guide_ui.distance_edit.is_some());
        key(&ctx, &mut app, Key::Escape);
        assert_eq!(app.editor.project, before);
        assert!(app.editor.anchor.is_none());
        assert!(app.editor.undo.is_empty());
    }
    fn click_dialog_label(ctx: &Context, app: &mut AtelierApp, label: &str) {
        frame(ctx, app, vec![]);
        let output = frame(ctx, app, vec![]);
        let pos = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    if text.galley.text() == label {
                        return Some(text.pos + vec2(4., 4.));
                    }
                }
                None
            })
            .unwrap_or_else(|| panic!("missing dialog label {label}"));
        click(ctx, app, pos);
    }
    #[test]
    fn settings_apply_validates_and_commits_exactly_once_with_real_button_events() {
        let (ctx, mut app) = setup();
        let before = app.editor.project.clone();
        let mut draft = before.guide_settings.clone();
        draft.percent = 101.;
        app.guide_ui.values = Some(draft);
        click_dialog_label(&ctx, &mut app, "Apply");
        assert_eq!(app.editor.project, before);
        assert!(!app.guide_ui.error.is_empty());
        app.guide_ui.values.as_mut().unwrap().percent = 25.;
        app.guide_ui.values.as_mut().unwrap().divisions = 8;
        click_dialog_label(&ctx, &mut app, "Apply");
        assert!(app.guide_ui.values.is_none());
        assert_eq!(app.editor.project.guide_settings.percent, 25.);
        assert_eq!(app.editor.project.guide_settings.divisions, 8);
        assert_eq!(app.editor.undo.len(), 1);
        app.editor.undo();
        assert_eq!(app.editor.project, before);
    }
    #[test]
    fn distance_apply_button_moves_selected_wall_with_single_undo() {
        let (ctx, mut app) = setup();
        app.editor.project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(-3., -2.),
                end: Point::new(3., -2.),
                thickness: 0.2,
                height: 3.,
            },
        );
        let id = app.editor.project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(-2., 0.),
                end: Point::new(2., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        app.editor.set_selection(vec![id]);
        let measurement = crate::distance_guides::measurements(
            &app.editor.project,
            0,
            Point::default(),
            Some(id),
            false,
        )[0];
        let before = app.editor.project.clone();
        app.guide_ui.distance_edit = Some((measurement, Some(id), 3.));
        click_dialog_label(&ctx, &mut app, "Apply");
        assert!(app.guide_ui.distance_edit.is_none());
        assert_eq!(app.editor.undo.len(), 1);
        assert!(
            matches!(app.editor.project.elements[1].shape,Shape::Wall {start,end,..} if start.y==1. && end.y==1.)
        );
        app.editor.undo();
        assert_eq!(app.editor.project, before);
    }
    #[test]
    fn tab_drag_creates_infinite_line_and_offset_rejects_collapsed_circle() {
        let (ctx, mut app) = setup();
        let tab = pos2(400., 6.);
        let destination = pos2(500., 200.);
        frame(&ctx, &mut app, vec![Event::PointerMoved(tab)]);
        frame(&ctx, &mut app, vec![button(tab, true)]);
        frame(&ctx, &mut app, vec![Event::PointerMoved(destination)]);
        frame(&ctx, &mut app, vec![button(destination, false)]);
        assert!(
            matches!(app.editor.project.guides[0].primitive,GuidePrimitive::Line {origin,direction} if origin.y==2. && direction==Point::new(1.,0.))
        );
        let circle = GuidePrimitive::Circle {
            center: Point::default(),
            radius: 2.,
        };
        app.add_guide(circle);
        app.guide_ui.offset_edit = Some((1, circle, -2.));
        click_dialog_label(&ctx, &mut app, "Apply");
        assert!(app.guide_ui.offset_edit.is_some());
        assert_eq!(app.editor.project.guides[1].primitive, circle);
        app.guide_ui.offset_edit = Some((1, circle, 1.));
        click_dialog_label(&ctx, &mut app, "Apply");
        assert_eq!(
            app.editor.project.guides[1].primitive,
            GuidePrimitive::Circle {
                center: Point::default(),
                radius: 3.
            }
        );
    }
}
