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
    offset_edit: Option<(usize, GuidePrimitive, f64)>,
    error: String,
    references: Vec<GuidePrimitive>,
    points: Vec<Point>,
    points_until: f64,
    hover_reference: Option<(Point, f64)>,
    hovered: Option<(usize, Point)>,
    drag: Option<(Project, usize, Point)>,
    tab_drag: Option<bool>,
    erase_before: Option<Project>,
    context: Option<(u32, Tool, View, u64, usize, usize, Option<u64>, Vec<u64>)>,
    distance_candidates: Vec<crate::distance_guides::DistanceMeasurement>,
    distance_edit: Option<(
        crate::distance_guides::DistanceMeasurement,
        Option<u64>,
        f64,
    )>,
}
impl AtelierApp {
    fn guides_menu(&mut self, ui: &mut Ui) {
        let before = self.host.document.editor.project.clone();
        let lang = self.language;
        let settings = &mut self.host.document.editor.project.guide_settings;
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
            self.host.document.editor.status = lang
                .translate("Guide: click first point, then second point · Esc cancels")
                .into();
            ui.close();
        }
        if ui
            .add_enabled(
                !self.host.document.editor.project.guides.is_empty(),
                Button::new(lang.translate("Remove All Guide Lines")),
            )
            .clicked()
        {
            self.host.document.editor.project.guides.clear();
            self.guide_ui.drag = None;
            self.guide_ui.action = None;
            ui.close();
        }
        if ui
            .add_enabled(
                can_draw && !self.host.document.editor.project.guides.is_empty(),
                Button::new(lang.translate("Erase Guide Lines")),
            )
            .clicked()
        {
            self.guide_ui.action = Some(GuideAction::Erase);
            self.guide_ui.start = None;
            self.guide_ui.arc_end = None;
            self.host.document.editor.status = lang
                .translate("Erase guides: click or drag · Esc / double-click exits")
                .into();
            ui.close();
        }
        ui.separator();
        let settings = &mut self.host.document.editor.project.guide_settings;
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
        self.host.document.editor.commit(before);
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
                                    .host
                                    .document
                                    .editor
                                    .project
                                    .guides
                                    .get(index)
                                    .is_some_and(|g| g.primitive == primitive) =>
                            {
                                let before = self.host.document.editor.project.clone();
                                self.host.document.editor.project.guides[index].primitive = offset;
                                self.host.document.editor.commit(before);
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
                                let before = self.host.document.editor.project.clone();
                                self.host.document.editor.project.guide_settings = values.clone();
                                self.host.document.editor.commit(before);
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
        if let Some((measurement, selected, mut value)) = self.guide_ui.distance_edit {
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
                                &self.host.document.editor.project,
                                &measurement,
                                id,
                                value,
                            )
                            .map(|project| {
                                let before = self.host.document.editor.project.clone();
                                self.host.document.editor.project = project;
                                self.host.document.editor.commit(before);
                                self.draft_id = None;
                            })
                        } else {
                            crate::distance_guides::target_at_distance(&measurement, value).map(
                                |point| {
                                    self.host.document.editor.click(point, 0.01);
                                    self.guide_ui.references.clear();
                                },
                            )
                        };
                        match result {
                            Ok(()) => close = true,
                            Err(error) => self.guide_ui.error = error,
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
                self.host.document.editor.project = before;
            }
            if let Some(before) = self.guide_ui.erase_before.take() {
                self.host.document.editor.commit(before);
            }
            if active {
                return true;
            }
            if !self.guide_ui.references.is_empty() {
                self.guide_ui.references.clear();
                self.guide_ui.points.clear();
                return true;
            }
            if let Some((index, _)) = self.guide_ui.hovered.take()
                && index < self.host.document.editor.project.guides.len()
            {
                let before = self.host.document.editor.project.clone();
                self.host.document.editor.project.guides.remove(index);
                self.host.document.editor.commit(before);
                return true;
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
            && self
                .host
                .document
                .editor
                .project
                .guide_settings
                .snap_enabled
        {
            self.toggle_snap_reference(self.hover_point);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::D))
            && self
                .host
                .document
                .editor
                .project
                .guide_settings
                .distance_guides_enabled
            && let Some(measurement) = self
                .guide_ui
                .distance_candidates
                .iter()
                .min_by(|a, b| a.distance.total_cmp(&b.distance))
                .copied()
        {
            let selected = if self.host.document.editor.tool == Tool::Select {
                self.host.document.editor.selected
            } else {
                None
            };
            self.guide_ui.distance_edit = Some((measurement, selected, measurement.distance));
            self.guide_ui.error.clear();
            return true;
        }
        false
    }
    fn toggle_snap_reference(&mut self, point: Point) {
        let tolerance = 10. / self.plan_scale;
        let (endpoint, kind) = geometry::snap(
            &self.host.document.editor.project,
            self.host.document.editor.story,
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
        let node = if matches!(
            kind,
            SnapKind::Endpoint | SnapKind::Midpoint | SnapKind::SnapPoint
        ) {
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
    fn reference_at(&self, point: Point, tolerance: f64) -> Option<GuidePrimitive> {
        let mut nearest = None;
        let mut distance = tolerance;
        for element in self
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .filter(|e| {
                let project = &self.host.document.editor.project;
                project.plan_options.element_visible(e, &project.layers)
                    && geometry::element_on_plan_story(project, e, self.host.document.editor.story)
                    && e.worksheet == project.plan_options.active_worksheet
            })
        {
            for path in guides::element_paths(element, DistanceReference::ReferenceLines) {
                if let Some(hit) = path.project(point)
                    && hit.distance < distance
                {
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
        nearest
    }
    fn target(&self, p: Point, shift: bool) -> (Point, SnapKind) {
        // The visible draft start is an explicit finish gesture. Resolve it
        // before model, grid, CAD, guide and numeric targets can displace it.
        if let Some(start) = self.mass_polygon_close_target(p) {
            return (start, SnapKind::Endpoint);
        }
        let (base_target, target_kind) = self.base_target(p, shift);
        if self.tracker_lock_applies()
            && (self.host.document.editor.anchor.is_some()
                || !self.host.document.editor.polyline_points.is_empty())
        {
            return (base_target, target_kind);
        }
        self.resolve_guide_target(
            p,
            self.apply_grid_snap(p, base_target, target_kind),
            self.host
                .document
                .editor
                .anchor
                .or_else(|| self.host.document.editor.polyline_points.last().copied()),
            shift,
        )
    }

    /// A transform uses its own edit origin, independent of creation drafts/locks.
    pub(super) fn precise_drag_target(
        &self,
        raw: Point,
        anchor: Point,
        shift: bool,
    ) -> (Point, SnapKind) {
        self.precise_drag_target_for_sources(raw, anchor, shift, &[])
    }

    pub(super) fn precise_drag_target_for_sources(
        &self,
        raw: Point,
        anchor: Point,
        shift: bool,
        source_ids: &[u64],
    ) -> (Point, SnapKind) {
        let base = geometry::snap_filtered(
            &self.host.document.editor.project,
            self.host.document.editor.story,
            raw,
            9. / self.plan_scale,
            self.host.document.editor.snap,
            |element, point, kind| {
                // A wall's body corners are derived from thickness. While moving
                // that operation's source, retain its actual edit nodes instead.
                !matches!(element.shape, Shape::Wall { .. })
                    || !source_ids.contains(&element.id)
                    || kind != SnapKind::Endpoint
                    || geometry::wall_edit_nodes(element)
                        .iter()
                        .any(|node| node.distance(point) < 1e-9)
            },
        );
        self.resolve_guide_target(
            raw,
            self.apply_grid_snap(raw, base.0, base.1),
            Some(anchor),
            shift,
        )
    }

    fn resolve_guide_target(
        &self,
        p: Point,
        base: (Point, SnapKind),
        anchor: Option<Point>,
        shift: bool,
    ) -> (Point, SnapKind) {
        let settings = &self.host.document.editor.project.guide_settings;
        let cache_current = self.guide_cache_current();
        let (mut base, mut kind) = base;
        let tolerance = 9. / self.plan_scale;
        let mut best = if matches!(
            kind,
            SnapKind::Endpoint | SnapKind::Midpoint | SnapKind::SnapPoint
        ) {
            p.distance(base)
        } else {
            tolerance
        };
        if settings.snap_enabled {
            let fresh = guides::snap_candidates(
                &self.host.document.editor.project,
                self.host.document.editor.story,
                p,
                tolerance,
                settings,
            );
            let cached_points = if cache_current {
                self.guide_ui.points.as_slice()
            } else {
                &[]
            };
            for point in fresh.iter().chain(cached_points.iter()) {
                if p.distance(*point) < best {
                    base = *point;
                    best = p.distance(*point);
                    kind = SnapKind::SnapPoint;
                }
            }
        }
        let mut primitives: Vec<GuidePrimitive> = if settings.guide_lines_enabled {
            self.host
                .document
                .editor
                .project
                .guides
                .iter()
                .filter(|g| g.story == self.host.document.editor.story)
                .map(|g| g.primitive)
                .collect()
        } else {
            Vec::new()
        };
        if settings.snap_enabled && cache_current {
            primitives.extend(self.guide_ui.references.clone());
            if let Some(anchor) = anchor {
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
        if let Some(point) = crossing
            && !matches!(
                kind,
                SnapKind::Endpoint | SnapKind::Midpoint | SnapKind::SnapPoint
            )
        {
            return (point, SnapKind::Guide);
        }
        for primitive in &primitives {
            if let Some(point) = primitive.project(p) {
                let distance = p.distance(point);
                if distance < best
                    && !matches!(
                        kind,
                        SnapKind::Endpoint | SnapKind::Midpoint | SnapKind::SnapPoint
                    )
                {
                    base = point;
                    best = distance;
                    kind = SnapKind::Guide;
                }
            }
        }
        // Shift extends the nearest visible guide constraint beyond its snap band.
        if shift
            && !primitives.is_empty()
            && !matches!(
                kind,
                SnapKind::Endpoint | SnapKind::Midpoint | SnapKind::SnapPoint
            )
            && let Some(point) = primitives
                .iter()
                .filter_map(|g| g.project(p))
                .min_by(|a, b| p.distance(*a).total_cmp(&p.distance(*b)))
        {
            base = point;
            kind = SnapKind::Guide;
        }
        // Axis fallback only applies when no explicit point or guide won.
        // Never replace a projected guide while still reporting guide feedback.
        if shift
            && !matches!(
                kind,
                SnapKind::Endpoint | SnapKind::Midpoint | SnapKind::SnapPoint | SnapKind::Guide
            )
            && let Some(anchor) = anchor
        {
            if (p.x - anchor.x).abs() >= (p.y - anchor.y).abs() {
                base.y = anchor.y;
            } else {
                base.x = anchor.x;
            }
            kind = SnapKind::Free;
        }
        (base, kind)
    }
    fn add_guide(&mut self, primitive: GuidePrimitive) {
        if let Err(error) = primitive.validate() {
            self.guide_ui.error = error.into();
            return;
        }
        let before = self.host.document.editor.project.clone();
        self.host
            .document
            .editor
            .project
            .guides
            .push(PersistentGuide {
                story: self.host.document.editor.story,
                primitive,
            });
        if let Err(error) = self.host.document.editor.project.validate() {
            self.host.document.editor.project = before;
            self.guide_ui.error = error;
            return;
        }
        self.host.document.editor.commit(before);
        self.guide_ui.start = None;
        self.guide_ui.arc_end = None;
        self.guide_ui.action = None;
        self.host.document.editor.status = self
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
                let d = vec2(direction.x, -direction.y) / direction.x.hypot(direction.y) as f32;
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
                    let t = i as f64 / 128. * std::f64::consts::TAU;
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
    // Hover and drawing feedback have no project/history side effects.
    fn guide_visible_ids(&self) -> Vec<u64> {
        let project = &self.host.document.editor.project;
        project
            .elements
            .iter()
            .filter(|e| {
                project.plan_options.element_visible(e, &project.layers)
                    && geometry::element_on_plan_story(project, e, self.host.document.editor.story)
                    && e.worksheet == project.plan_options.active_worksheet
            })
            .map(|e| e.id)
            .collect()
    }

    fn guide_cache_current(&self) -> bool {
        self.guide_ui.context.as_ref().is_none_or(|context| {
            context.0 == self.host.document.editor.story
                && context.1 == self.host.document.editor.tool
                && context.2 == self.view
                && context.3 == self.host.document.editor.project.next_id
                && context.4 == self.host.document.editor.undo.len()
                && context.5 == self.host.document.editor.redo.len()
                && context.6
                    == self
                        .host
                        .document
                        .editor
                        .project
                        .plan_options
                        .active_worksheet
                && context.7 == self.guide_visible_ids()
        })
    }

    fn guide_feedback(&mut self, ui: &mut Ui, rect: Rect, raw: Option<Point>, hovered: bool) {
        let painter = ui.painter_at(rect);
        let now = ui.input(|i| i.time);
        let context = (
            self.host.document.editor.story,
            self.host.document.editor.tool,
            self.view,
            self.host.document.editor.project.next_id,
            self.host.document.editor.undo.len(),
            self.host.document.editor.redo.len(),
            self.host
                .document
                .editor
                .project
                .plan_options
                .active_worksheet,
            self.guide_visible_ids(),
        );
        if self
            .guide_ui
            .context
            .as_ref()
            .is_some_and(|old| old != &context)
        {
            self.guide_ui.references.clear();
            self.guide_ui.points.clear();
            self.guide_ui.hover_reference = None;
        }
        self.guide_ui.context = Some(context);
        let settings = self.host.document.editor.project.guide_settings.clone();
        let tolerance = 9. / self.plan_scale;
        if settings.snap_enabled {
            if let Some(raw) = raw {
                let points = guides::snap_candidates(
                    &self.host.document.editor.project,
                    self.host.document.editor.story,
                    raw,
                    tolerance,
                    &settings,
                );
                if !points.is_empty() {
                    self.guide_ui.points = points;
                    self.guide_ui.points_until = now + 5.;
                }
                if hovered && self.guide_ui.action.is_none() && self.guide_ui.drag.is_none() {
                    if let Some((prior, since)) = self.guide_ui.hover_reference {
                        if prior.distance(raw) > tolerance {
                            self.guide_ui.hover_reference = Some((raw, now));
                        } else if now - since > 0.6 && now - since < 1.0 {
                            if let Some(reference) = self.reference_at(raw, tolerance)
                                && !self.guide_ui.references.contains(&reference)
                                && self.guide_ui.references.len() < 64
                            {
                                self.guide_ui.references.push(reference);
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
    }

    pub(super) fn drag_guide_feedback(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        raw: Point,
        anchor: Point,
    ) {
        let hovered = ui
            .input(|i| i.pointer.hover_pos())
            .is_some_and(|p| rect.contains(p));
        self.guide_feedback(ui, rect, Some(raw), hovered);
        self.guide_ui.hovered = None;
        let painter = ui.painter_at(rect);
        let settings = &self.host.document.editor.project.guide_settings;
        if settings.guide_lines_enabled {
            for guide in self
                .host
                .document
                .editor
                .project
                .guides
                .iter()
                .filter(|g| g.story == self.host.document.editor.story)
            {
                self.paint_guide(&painter, rect, &guide.primitive, GUIDE_COLOR);
            }
        }
        // These are the same parallel/perpendicular candidates used by the solver,
        // based on the transform origin rather than a stale creation anchor.
        if settings.snap_enabled {
            for reference in &self.guide_ui.references {
                if let GuidePrimitive::Line { direction, .. } = reference {
                    for direction in [*direction, Point::new(-direction.y, direction.x)] {
                        self.paint_guide(
                            &painter,
                            rect,
                            &GuidePrimitive::Line {
                                origin: anchor,
                                direction,
                            },
                            ACCENT,
                        );
                    }
                }
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
        let pointer = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| rect.contains(*p));
        let raw = pointer.map(|p| self.world(rect, p));
        let settings = self.host.document.editor.project.guide_settings.clone();
        let tolerance = 9. / self.plan_scale;
        self.guide_feedback(ui, rect, raw, response.hovered());
        self.guide_ui.hovered = None;
        let mut nearest = tolerance;
        if settings.guide_lines_enabled {
            for (i, guide) in self
                .host
                .document
                .editor
                .project
                .guides
                .iter()
                .enumerate()
                .filter(|(_, g)| g.story == self.host.document.editor.story)
            {
                self.paint_guide(&painter, rect, &guide.primitive, GUIDE_COLOR);
                if let Some(raw) = raw
                    && let Some(point) = guide.primitive.project(raw)
                {
                    let distance = raw.distance(point);
                    if distance < nearest {
                        self.guide_ui.hovered = Some((i, point));
                        nearest = distance;
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
            if let Some(raw) = raw
                && let Some(original) = before.guides.get(index)
            {
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
                    self.host.document.editor.project.guides[index].primitive = translated;
                }
            }
            if ui.input(|i| i.pointer.any_released()) {
                if pointer.is_some_and(|p| tabs.iter().any(|r| r.contains(p))) {
                    self.host.document.editor.project.guides.remove(index);
                }
                if self.host.document.editor.project.validate().is_ok() {
                    self.host.document.editor.commit(before);
                } else {
                    self.host.document.editor.project = before;
                }
                self.guide_ui.drag = None;
            }
            return true;
        }
        if let Some(action) = self.guide_ui.action {
            if action == GuideAction::Erase {
                ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
                if let Some(raw) = raw
                    && (response.clicked() || response.dragged_by(PointerButton::Primary))
                {
                    if self.guide_ui.erase_before.is_none() {
                        self.guide_ui.erase_before =
                            Some(self.host.document.editor.project.clone());
                    }
                    let delta = ui.input(|i| i.pointer.delta());
                    let previous = Point::new(
                        raw.x - f64::from(delta.x) / self.plan_scale,
                        raw.y + f64::from(delta.y) / self.plan_scale,
                    );
                    let stroke = GuidePrimitive::Segment {
                        start: previous,
                        end: raw,
                    };
                    self.host.document.editor.project.guides.retain(|g| {
                        g.story != self.host.document.editor.story
                            || (g
                                .primitive
                                .project(raw)
                                .is_none_or(|p| p.distance(raw) > tolerance)
                                && g.primitive.intersections(&stroke).is_empty())
                    });
                }
                if ui.input(|i| i.pointer.any_released())
                    && let Some(before) = self.guide_ui.erase_before.take()
                {
                    self.host.document.editor.commit(before);
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
                    if let Some(primitive) = &primitive
                        && primitive.validate().is_ok()
                    {
                        self.paint_guide(&painter, rect, primitive, GUIDE_COLOR);
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
        if self.host.document.editor.tool == Tool::Select
            && settings.guide_lines_enabled
            && response.drag_started_by(PointerButton::Primary)
            && !ui.input(|i| i.key_down(Key::Space))
            && let Some(start) = ui
                .input(|i| i.pointer.press_origin())
                .filter(|p| rect.contains(*p))
                .map(|p| self.world(rect, p))
        {
            let hit = self
                .host
                .document
                .editor
                .project
                .guides
                .iter()
                .enumerate()
                .filter(|(_, g)| g.story == self.host.document.editor.story)
                .filter_map(|(i, g)| g.primitive.project(start).map(|p| (i, p.distance(start))))
                .filter(|(_, d)| *d < tolerance)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((index, _)) = hit {
                self.guide_ui.drag =
                    Some((self.host.document.editor.project.clone(), index, start));
                self.drag = None;
                return true;
            }
        }
        if let Some((index, point)) = self.guide_ui.hovered
            && self.host.document.editor.tool == Tool::Select
        {
            let pos = self.screen(rect, point);
            painter.circle_filled(pos, 4.5, GUIDE_COLOR);
            // Handle is projected under the cursor; picking it must not select a wall.
            if response.drag_started_by(PointerButton::Primary) {
                self.guide_ui.drag = Some((
                    self.host.document.editor.project.clone(),
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
                    self.guide_ui.offset_edit = Some((
                        index,
                        self.host.document.editor.project.guides[index].primitive,
                        0.,
                    ));
                    self.guide_ui.error.clear();
                    ui.close();
                }
                if ui
                    .button(self.language.translate("Remove Guide Line"))
                    .clicked()
                {
                    let before = self.host.document.editor.project.clone();
                    self.host.document.editor.project.guides.remove(index);
                    self.host.document.editor.commit(before);
                    ui.close();
                }
                if ui
                    .button(self.language.translate("Remove All Guide Lines"))
                    .clicked()
                {
                    let before = self.host.document.editor.project.clone();
                    self.host.document.editor.project.guides.clear();
                    self.host.document.editor.commit(before);
                    ui.close();
                }
            });
        }
        self.guide_ui.distance_candidates.clear();
        if settings.distance_guides_enabled {
            let selected = self.host.document.editor.selected.filter(|_| {
                self.host.document.editor.selection_ids().len() == 1
                    && self.host.document.editor.tool == Tool::Select
            });
            let wall_selected = selected.is_some_and(|id| {
                self.host
                    .document
                    .editor
                    .project
                    .elements
                    .iter()
                    .any(|e| e.id == id && matches!(e.shape, Shape::Wall { .. }))
            });
            if wall_selected || self.host.document.editor.tool == Tool::Wall {
                let source = raw.unwrap_or(self.hover_point);
                let measurements = crate::distance_guides::measurements(
                    &self.host.document.editor.project,
                    self.host.document.editor.story,
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
        app.host.document.editor = Editor::new(Project::default());
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
        app.host.document.editor.set_tool(Tool::Wall);
        app.host.document.editor.anchor = Some(Point::new(-4., -4.));
        app.guide_ui.action = Some(GuideAction::Create(GuideDraw::Segment));
        click(&ctx, &mut app, pos2(400., 300.));
        click(&ctx, &mut app, pos2(550., 250.));
        assert_eq!(app.host.document.editor.project.guides.len(), 1);
        assert!(app.host.document.editor.project.elements.is_empty());
        assert_eq!(app.host.document.editor.undo.len(), 1);
        assert_eq!(app.host.document.editor.anchor, Some(Point::new(-4., -4.)));
        let primitive = app.host.document.editor.project.guides[0].primitive;
        assert_eq!(
            primitive,
            GuidePrimitive::Segment {
                start: Point::new(0., 0.),
                end: Point::new(3., 1.)
            }
        );
        app.host.document.editor.undo();
        assert!(app.host.document.editor.project.guides.is_empty());
        app.host.document.editor.redo();
        assert_eq!(
            app.host.document.editor.project.guides[0].primitive,
            primitive
        );
        app.host.document.editor.anchor = Some(Point::new(-4., -4.));
        app.guide_ui.action = Some(GuideAction::Create(GuideDraw::Circle));
        click(&ctx, &mut app, pos2(450., 400.));
        key(&ctx, &mut app, Key::Escape);
        assert_eq!(app.host.document.editor.project.guides.len(), 1);
        assert_eq!(app.host.document.editor.tool, Tool::Wall);
        assert_eq!(app.host.document.editor.anchor, Some(Point::new(-4., -4.)));
    }
    #[test]
    fn circle_and_three_point_arc_create_exact_geometry() {
        let (ctx, mut app) = setup();
        app.host.document.editor.project.guide_settings.snap_enabled = false;
        app.guide_ui.action = Some(GuideAction::Create(GuideDraw::Circle));
        click(&ctx, &mut app, pos2(400., 300.));
        click(&ctx, &mut app, pos2(500., 300.));
        assert_eq!(
            app.host.document.editor.project.guides[0].primitive,
            GuidePrimitive::Circle {
                center: Point::new(0., 0.),
                radius: 2.
            }
        );
        app.host.document.editor.project.guides.clear();
        app.guide_ui.action = Some(GuideAction::Create(GuideDraw::Arc));
        click(&ctx, &mut app, pos2(400., 300.));
        click(&ctx, &mut app, pos2(600., 300.));
        assert!(app.host.document.editor.project.guides.is_empty());
        click(&ctx, &mut app, pos2(500., 200.));
        let GuidePrimitive::Arc { start, end, arc } =
            app.host.document.editor.project.guides[0].primitive
        else {
            panic!("arc expected")
        };
        assert!((arc.radius - 2.).abs() < 1e-5);
        assert_eq!(start, Point::new(0., 0.));
        assert_eq!(end, Point::new(4., 0.));
        assert!(
            app.host.document.editor.project.guides[0]
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
            app.host
                .document
                .editor
                .project
                .guides
                .push(PersistentGuide {
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
        assert!(app.host.document.editor.project.guides.is_empty());
        assert_eq!(app.host.document.editor.undo.len(), 1);
        app.host.document.editor.undo();
        assert_eq!(app.host.document.editor.project.guides.len(), 2);
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
            app.host.document.editor.project.guides[0]
                .primitive
                .project(Point::new(0., 0.))
                .unwrap(),
            Point::new(0., 1.)
        );
        assert_eq!(app.host.document.editor.undo.len(), 2);
        frame(&ctx, &mut app, vec![Event::PointerMoved(b)]);
        frame(&ctx, &mut app, vec![button(b, true)]);
        frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(500., 200.))]);
        key(&ctx, &mut app, Key::Escape);
        assert_eq!(
            app.host.document.editor.project.guides[0]
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
        app.host.document.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(0., 0.), Point::new(3., 0.)],
                arcs: vec![],
            },
        );
        app.host.document.editor.project.guide_settings.method = SnapMethod::Divisions;
        app.host.document.editor.snap = false;
        app.host
            .document
            .editor
            .project
            .grid_settings
            .grid_snap_enabled = false;
        let (point, kind) = app.target(Point::new(1.01, 0.02), false);
        assert_eq!(point, Point::new(1., 0.));
        assert_eq!(kind, SnapKind::SnapPoint);
        app.host.document.editor.project.guide_settings.snap_enabled = false;
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
        app.host
            .document
            .editor
            .project
            .guide_settings
            .guide_lines_enabled = false;
        assert_eq!(app.target(Point::new(1.23, 1.05), false).1, SnapKind::Free);
    }
    #[test]
    fn shift_locks_drawing_to_dominant_axis_through_anchor() {
        let (_, mut app) = setup();
        app.host.document.editor.snap = false;
        app.host
            .document
            .editor
            .project
            .grid_settings
            .grid_snap_enabled = false;
        app.host.document.editor.anchor = Some(Point::new(1., 1.));
        // Pointer moves mostly horizontally: Y locks to the anchor.
        assert_eq!(
            app.target(Point::new(3.5, 2.4), true),
            (Point::new(3.5, 1.), SnapKind::Free)
        );
        // Pointer moves mostly vertically: X locks to the anchor.
        assert_eq!(
            app.target(Point::new(2.4, 4.5), true),
            (Point::new(1., 4.5), SnapKind::Free)
        );
        // Without shift the cursor stays free.
        assert_eq!(
            app.target(Point::new(3.5, 2.4), false),
            (Point::new(3.5, 2.4), SnapKind::Free)
        );
        // The last polyline node anchors the constraint the same way.
        app.host.document.editor.anchor = None;
        app.host.document.editor.polyline_points = vec![Point::new(0., 0.), Point::new(2., 2.)];
        assert_eq!(
            app.target(Point::new(5., 4.), true),
            (Point::new(5., 2.), SnapKind::Free)
        );
        // Exact endpoint snapping still wins over the axis lock.
        app.host.document.editor.anchor = Some(Point::new(0., 0.));
        app.host.document.editor.polyline_points.clear();
        app.host.document.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(3., 0.), Point::new(6., 0.)],
                arcs: vec![],
            },
        );
        app.host.document.editor.snap = true;
        let (point, kind) = app.target(Point::new(5.98, 0.02), true);
        assert_eq!((point, kind), (Point::new(6., 0.), SnapKind::Endpoint));
    }

    #[test]
    fn precise_drag_shift_preserves_diagonal_guide_in_creation_and_transform() {
        let (_, mut app) = setup();
        app.host.document.editor.snap = false;
        app.host
            .document
            .editor
            .project
            .grid_settings
            .grid_snap_enabled = false;
        app.host.document.editor.set_tool(Tool::Wall);
        app.host.document.editor.anchor = Some(Point::default());
        app.host
            .document
            .editor
            .project
            .guides
            .push(PersistentGuide {
                story: 0,
                primitive: GuidePrimitive::Line {
                    origin: Point::default(),
                    direction: Point::new(1., 1.),
                },
            });
        let raw = Point::new(3., 2.);
        let expected = (Point::new(2.5, 2.5), SnapKind::Guide);
        assert_eq!(app.target(raw, true), expected);
        assert_eq!(
            app.precise_drag_target(raw, Point::default(), true),
            expected
        );
    }

    #[test]
    fn precise_drag_sources_filter_derived_wall_corners_before_best_candidate() {
        let (_, mut app) = setup();
        app.host
            .document
            .editor
            .project
            .grid_settings
            .grid_snap_enabled = false;
        let wall = |start, end| Shape::Wall {
            start,
            end,
            thickness: 1.,
            height: 3.,
        };
        let id = app.host.document.editor.project.add(
            0,
            Material::Plaster,
            wall(Point::default(), Point::new(4., 0.)),
        );
        let raw = Point::new(4., -0.4);
        // The shared/default creation snap still sees the physical wall corner.
        assert_eq!(
            geometry::snap(&app.host.document.editor.project, 0, raw, 0.18, true),
            (Point::new(4., -0.5), SnapKind::Endpoint)
        );
        assert_eq!(
            app.precise_drag_target_for_sources(raw, Point::new(2., -0.4), false, &[id]),
            (raw, SnapKind::Free)
        );
        let copy = app.host.document.editor.project.add(
            0,
            Material::Plaster,
            wall(Point::new(2., 0.), Point::new(6., 0.)),
        );
        let next = Point::new(6., -0.4);
        assert_eq!(
            app.precise_drag_target_for_sources(next, Point::new(2., -0.4), false, &[id, copy]),
            (next, SnapKind::Free)
        );
        // Rejecting the nearest derived corner must not hide a slightly farther
        // real node belonging to another element.
        let other = Point::new(4.05, -0.55);
        app.host.document.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![other, Point::new(6., -0.55)],
                arcs: vec![],
            },
        );
        assert_eq!(
            app.precise_drag_target_for_sources(raw, Point::new(2., -0.4), false, &[id, copy]),
            (other, SnapKind::Endpoint)
        );
    }

    #[test]
    fn precise_drag_sources_keep_reference_nodes_midpoints_and_independent_switches() {
        let (_, mut app) = setup();
        app.host
            .document
            .editor
            .project
            .grid_settings
            .grid_snap_enabled = false;
        let id = app.host.document.editor.project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::default(),
                end: Point::new(4., 0.),
                thickness: 1.,
                height: 3.,
            },
        );
        app.host
            .document
            .editor
            .project
            .guides
            .push(PersistentGuide {
                story: 0,
                primitive: GuidePrimitive::Line {
                    origin: Point::default(),
                    direction: Point::new(1., 1.),
                },
            });
        assert_eq!(
            app.precise_drag_target_for_sources(
                Point::new(4., 0.02),
                Point::default(),
                true,
                &[id]
            ),
            (Point::new(4., 0.), SnapKind::Endpoint)
        );
        assert_eq!(
            app.precise_drag_target_for_sources(
                Point::new(2., 0.02),
                Point::default(),
                true,
                &[id]
            ),
            (Point::new(2., 0.), SnapKind::Midpoint)
        );
        app.host.document.editor.project.guide_settings.snap_enabled = false;
        assert_eq!(
            app.precise_drag_target_for_sources(
                Point::new(4., 0.02),
                Point::default(),
                true,
                &[id]
            ),
            (Point::new(4., 0.), SnapKind::Endpoint)
        );
        app.host.document.editor.snap = false;
        assert_eq!(
            app.precise_drag_target_for_sources(Point::new(3., 2.), Point::default(), true, &[id]),
            (Point::new(2.5, 2.5), SnapKind::Guide)
        );
        app.host
            .document
            .editor
            .project
            .guide_settings
            .guide_lines_enabled = false;
        assert_eq!(
            app.precise_drag_target_for_sources(Point::new(3., 2.), Point::default(), false, &[id]),
            (Point::new(3., 2.), SnapKind::Free)
        );
    }

    #[test]
    fn precise_drag_wall_axis_keeps_off_axis_endpoint_and_snap_point_truthful() {
        for (shift, orthogonal) in [(true, false), (false, true)] {
            let (_, mut app) = setup();
            app.host.document.editor.set_tool(Tool::Wall);
            app.host.document.editor.anchor = Some(Point::default());
            app.host.document.editor.orthogonal = orthogonal;
            app.host
                .document
                .editor
                .project
                .grid_settings
                .grid_snap_enabled = false;
            app.host.document.editor.project.add(
                0,
                Material::Plaster,
                Shape::Polyline {
                    points: vec![Point::new(6., 2.), Point::new(9., 2.)],
                    arcs: vec![],
                },
            );
            assert_eq!(
                app.target(Point::new(5.98, 2.02), shift),
                (Point::new(6., 2.), SnapKind::Endpoint)
            );
            app.host.document.editor.project.guide_settings.snap_enabled = false;
            app.host.document.editor.project.add(
                0,
                Material::Concrete,
                Shape::Wall {
                    start: Point::new(20., 2.),
                    end: Point::new(24., 2.),
                    thickness: 0.2,
                    height: 3.,
                },
            );
            assert_eq!(
                app.target(Point::new(22.02, 2.02), shift),
                (Point::new(22., 2.), SnapKind::Midpoint)
            );
        }
        let (_, mut app) = setup();
        app.host.document.editor.set_tool(Tool::Wall);
        app.host.document.editor.anchor = Some(Point::default());
        app.host
            .document
            .editor
            .project
            .grid_settings
            .grid_snap_enabled = false;
        app.host.document.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(4., 2.), Point::new(8., 2.)],
                arcs: vec![],
            },
        );
        assert_eq!(
            app.target(Point::new(5.98, 2.02), true),
            (Point::new(6., 2.), SnapKind::SnapPoint)
        );
    }

    #[test]
    fn precise_drag_anchor_ignores_creation_draft_anchor_and_numeric_lock() {
        let (_, mut app) = setup();
        app.host.document.editor.snap = false;
        app.host
            .document
            .editor
            .project
            .grid_settings
            .grid_snap_enabled = false;
        app.host.document.editor.project.guide_settings.snap_enabled = false;
        app.host.document.editor.set_tool(Tool::Wall);
        app.host.document.editor.anchor = Some(Point::new(100., 100.));
        app.host.document.editor.polyline_points = vec![Point::new(200., 200.)];
        app.tracker_locked = true;
        app.tracker_a = 99.;
        app.tracker_b = 45.;
        let raw = Point::new(5., 4.);
        assert_eq!(
            app.precise_drag_target(raw, Point::new(2., 2.), true),
            (Point::new(5., 2.), SnapKind::Free)
        );
        assert_eq!(
            app.precise_drag_target(raw, Point::new(2., 2.), false),
            (raw, SnapKind::Free)
        );
    }

    #[test]
    fn precise_drag_snap_and_reference_share_visible_worksheet_scope() {
        let (_, mut app) = setup();
        app.host
            .document
            .editor
            .project
            .grid_settings
            .grid_snap_enabled = false;
        let id = app.host.document.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(0., 0.), Point::new(10., 0.)],
                arcs: vec![],
            },
        );
        app.host
            .document
            .editor
            .project
            .elements
            .iter_mut()
            .find(|e| e.id == id)
            .unwrap()
            .worksheet = Some(42);
        let raw = Point::new(5., 0.02);
        assert_eq!(
            app.precise_drag_target(raw, Point::default(), false),
            (raw, SnapKind::Free)
        );
        assert!(app.reference_at(raw, 0.1).is_none());
        app.host
            .document
            .editor
            .project
            .plan_options
            .active_worksheet = Some(42);
        assert_eq!(
            app.precise_drag_target(raw, Point::default(), false),
            (Point::new(5., 0.), SnapKind::SnapPoint)
        );
        assert!(app.reference_at(raw, 0.1).is_some());
        app.host
            .document
            .editor
            .project
            .elements
            .iter_mut()
            .find(|e| e.id == id)
            .unwrap()
            .layer = crate::layers::ElementLayer(crate::layers::HIDDEN_LAYER);
        assert_eq!(
            app.precise_drag_target(raw, Point::default(), false),
            (raw, SnapKind::Free)
        );
        assert!(app.reference_at(raw, 0.1).is_none());
    }

    fn drag_feedback_frame(ctx: &Context, app: &mut AtelierApp, raw: Point) {
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
        let pointer = app.screen(rect, raw);
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(rect),
                events: vec![
                    Event::PointerMoved(pointer),
                    Event::PointerButton {
                        pos: pointer,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ui| app.drag_guide_feedback(ui, rect, raw, Point::default()),
        );
        output.textures_delta.clear();
    }

    #[test]
    fn precise_drag_feedback_clears_scope_cache_and_never_edits_guides() {
        let (ctx, mut app) = setup();
        let id = app.host.document.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(0., 0.), Point::new(10., 0.)],
                arcs: vec![],
            },
        );
        let raw = Point::new(5., 0.02);
        drag_feedback_frame(&ctx, &mut app, raw);
        assert!(!app.guide_ui.points.is_empty());
        app.guide_ui.references.push(GuidePrimitive::Line {
            origin: Point::default(),
            direction: Point::new(1., 1.),
        });
        app.host
            .document
            .editor
            .project
            .plan_options
            .active_worksheet = Some(42);
        // A target query before the next repaint must not use the old sheet cache.
        assert_eq!(
            app.precise_drag_target(raw, Point::default(), false),
            (raw, SnapKind::Free)
        );
        drag_feedback_frame(&ctx, &mut app, raw);
        assert!(app.guide_ui.points.is_empty());
        assert!(app.guide_ui.references.is_empty());
        app.host
            .document
            .editor
            .project
            .plan_options
            .active_worksheet = None;
        drag_feedback_frame(&ctx, &mut app, raw);
        assert!(!app.guide_ui.points.is_empty());
        app.host
            .document
            .editor
            .project
            .elements
            .iter_mut()
            .find(|e| e.id == id)
            .unwrap()
            .layer = crate::layers::ElementLayer(crate::layers::HIDDEN_LAYER);
        assert_eq!(
            app.precise_drag_target(raw, Point::default(), false),
            (raw, SnapKind::Free)
        );
        drag_feedback_frame(&ctx, &mut app, raw);
        assert!(app.guide_ui.points.is_empty());
        app.host
            .document
            .editor
            .project
            .guides
            .push(PersistentGuide {
                story: 0,
                primitive: GuidePrimitive::Line {
                    origin: Point::default(),
                    direction: Point::new(1., 0.),
                },
            });
        app.guide_ui.action = Some(GuideAction::Erase);
        app.guide_ui.hovered = Some((0, Point::default()));
        let before = app.host.document.editor.project.clone();
        let revision = app.host.document.editor.revision;
        drag_feedback_frame(&ctx, &mut app, raw);
        assert_eq!(app.host.document.editor.project, before);
        assert_eq!(app.host.document.editor.revision, revision);
        assert!(app.host.document.editor.undo.is_empty());
        assert!(app.host.document.editor.redo.is_empty());
        assert!(app.guide_ui.hovered.is_none());
        assert_eq!(app.guide_ui.action, Some(GuideAction::Erase));
    }
    #[test]
    fn guide_intersections_snap_to_one_joint_not_one_line_projection() {
        let (_, mut app) = setup();
        app.host.document.editor.snap = false;
        app.host
            .document
            .editor
            .project
            .grid_settings
            .grid_snap_enabled = false;
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
        let before = app.host.document.editor.project.clone();
        let mut draft = before.guide_settings.clone();
        draft.divisions = 17;
        app.guide_ui.values = Some(draft);
        frame(&ctx, &mut app, vec![]);
        key(&ctx, &mut app, Key::Escape);
        assert!(app.guide_ui.values.is_none());
        assert_eq!(app.host.document.editor.project, before);
        assert!(app.host.document.editor.undo.is_empty());
    }
    #[test]
    fn distance_keyboard_entry_freezes_input_reference_and_escape_cancels() {
        let (ctx, mut app) = setup();
        app.host.document.editor.project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(-3., -2.),
                end: Point::new(3., -2.),
                thickness: 0.2,
                height: 3.,
            },
        );
        app.host.document.editor.set_tool(Tool::Wall);
        frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(400., 300.))]);
        assert!(!app.guide_ui.distance_candidates.is_empty());
        let before = app.host.document.editor.project.clone();
        key(&ctx, &mut app, Key::D);
        assert!(app.guide_ui.distance_edit.is_some());
        key(&ctx, &mut app, Key::Escape);
        assert_eq!(app.host.document.editor.project, before);
        assert!(app.host.document.editor.anchor.is_none());
        assert!(app.host.document.editor.undo.is_empty());
    }
    fn click_dialog_label(ctx: &Context, app: &mut AtelierApp, label: &str) {
        frame(ctx, app, vec![]);
        let output = frame(ctx, app, vec![]);
        let pos = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.text() == label
                {
                    return Some(text.pos + vec2(4., 4.));
                }
                None
            })
            .unwrap_or_else(|| panic!("missing dialog label {label}"));
        click(ctx, app, pos);
    }
    #[test]
    fn settings_apply_validates_and_commits_exactly_once_with_real_button_events() {
        let (ctx, mut app) = setup();
        let before = app.host.document.editor.project.clone();
        let mut draft = before.guide_settings.clone();
        draft.percent = 101.;
        app.guide_ui.values = Some(draft);
        click_dialog_label(&ctx, &mut app, "Apply");
        assert_eq!(app.host.document.editor.project, before);
        assert!(!app.guide_ui.error.is_empty());
        app.guide_ui.values.as_mut().unwrap().percent = 25.;
        app.guide_ui.values.as_mut().unwrap().divisions = 8;
        click_dialog_label(&ctx, &mut app, "Apply");
        assert!(app.guide_ui.values.is_none());
        assert_eq!(app.host.document.editor.project.guide_settings.percent, 25.);
        assert_eq!(app.host.document.editor.project.guide_settings.divisions, 8);
        assert_eq!(app.host.document.editor.undo.len(), 1);
        app.host.document.editor.undo();
        assert_eq!(app.host.document.editor.project, before);
    }
    #[test]
    fn distance_apply_button_moves_selected_wall_with_single_undo() {
        let (ctx, mut app) = setup();
        app.host.document.editor.project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(-3., -2.),
                end: Point::new(3., -2.),
                thickness: 0.2,
                height: 3.,
            },
        );
        let id = app.host.document.editor.project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(-2., 0.),
                end: Point::new(2., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        app.host.document.editor.set_selection(vec![id]);
        let measurement = crate::distance_guides::measurements(
            &app.host.document.editor.project,
            0,
            Point::default(),
            Some(id),
            false,
        )[0];
        let before = app.host.document.editor.project.clone();
        app.guide_ui.distance_edit = Some((measurement, Some(id), 3.));
        click_dialog_label(&ctx, &mut app, "Apply");
        assert!(app.guide_ui.distance_edit.is_none());
        assert_eq!(app.host.document.editor.undo.len(), 1);
        assert!(
            matches!(app.host.document.editor.project.elements[1].shape,Shape::Wall {start,end,..} if start.y==1. && end.y==1.)
        );
        app.host.document.editor.undo();
        assert_eq!(app.host.document.editor.project, before);
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
            matches!(app.host.document.editor.project.guides[0].primitive,GuidePrimitive::Line {origin,direction} if origin.y==2. && direction==Point::new(1.,0.))
        );
        let circle = GuidePrimitive::Circle {
            center: Point::default(),
            radius: 2.,
        };
        app.add_guide(circle);
        app.guide_ui.offset_edit = Some((1, circle, -2.));
        click_dialog_label(&ctx, &mut app, "Apply");
        assert!(app.guide_ui.offset_edit.is_some());
        assert_eq!(app.host.document.editor.project.guides[1].primitive, circle);
        app.guide_ui.offset_edit = Some((1, circle, 1.));
        click_dialog_label(&ctx, &mut app, "Apply");
        assert_eq!(
            app.host.document.editor.project.guides[1].primitive,
            GuidePrimitive::Circle {
                center: Point::default(),
                radius: 3.
            }
        );
    }
}
