use super::wall_node_ui::WallPetCommand;
use super::*;
use crate::layers::{ARCHICAD_LAYER, ElementLayer};

impl AtelierApp {
    /// Triangulate wall junction caps: a multi-way footprint may be concave.
    /// Stroke only the perimeter, never the internal triangulation edges.
    pub(super) fn paint_wall_polygon(
        painter: &egui::Painter,
        polygon: &[Point],
        screen: impl Fn(Point) -> Pos2,
        fill: Color32,
        stroke: Stroke,
    ) {
        let mut mesh = egui::Mesh::default();
        for triangle in geometry::triangulate_polygon(polygon) {
            let index = mesh.vertices.len() as u32;
            for point in triangle {
                mesh.colored_vertex(screen(point), fill);
            }
            mesh.add_triangle(index, index + 1, index + 2);
        }
        painter.add(egui::Shape::mesh(mesh));
        painter.add(egui::Shape::closed_line(
            polygon.iter().copied().map(screen).collect(),
            stroke,
        ));
    }

    /// The same hole-aware triangles and sampled ring boundaries feed plan and trace.
    pub(super) fn paint_slab_polygon(
        painter: &egui::Painter,
        element: &Element,
        screen: impl Fn(Point) -> Pos2,
        fill: Color32,
        stroke: Stroke,
    ) {
        let mut mesh = egui::Mesh::default();
        for triangle in crate::slab_geometry::triangles(element) {
            let index = mesh.vertices.len() as u32;
            for point in triangle {
                mesh.colored_vertex(screen(point), fill);
            }
            mesh.add_triangle(index, index + 1, index + 2);
        }
        painter.add(egui::Shape::mesh(mesh));
        for ring in crate::slab_geometry::outlines(element) {
            painter.add(egui::Shape::closed_line(
                ring.into_iter().map(&screen).collect(),
                stroke,
            ));
        }
    }

    pub(super) fn world(&self, r: Rect, p: Pos2) -> Point {
        let radians = self
            .editor
            .project
            .plan_options
            .normalized_orientation()
            .to_radians();
        let (sin, cos) = radians.sin_cos();
        let view_x = (p.x - r.center().x) / self.plan_scale;
        let view_y = -(p.y - r.center().y) / self.plan_scale;
        Point::new(
            self.plan_center.x + view_x * cos + view_y * sin,
            self.plan_center.y - view_x * sin + view_y * cos,
        )
    }
    pub(super) fn screen(&self, r: Rect, p: Point) -> Pos2 {
        let radians = self
            .editor
            .project
            .plan_options
            .normalized_orientation()
            .to_radians();
        let (sin, cos) = radians.sin_cos();
        let x = p.x - self.plan_center.x;
        let y = p.y - self.plan_center.y;
        pos2(
            r.center().x + (x * cos - y * sin) * self.plan_scale,
            r.center().y - (x * sin + y * cos) * self.plan_scale,
        )
    }
    pub(super) fn plan_pen(&self) -> crate::view_settings::PenPalette {
        self.editor.project.plan_options.pen_set.palette()
    }
    pub(super) fn plan_style(
        &self,
        element: &Element,
    ) -> Option<crate::view_settings::PlanElementStyle> {
        self.editor.project.plan_options.style_for(
            element,
            self.plan_pen(),
            &self.editor.project.layers,
        )
    }
    pub(super) fn plan_pen_px(&self, width_mm: f32) -> f32 {
        let world_width = width_mm / 1000. * self.editor.project.plan_options.drawing_scale as f32;
        (world_width * self.plan_scale).clamp(0.45, 10.)
    }
    pub(super) fn plan_color(rgb: [u8; 3]) -> Color32 {
        Color32::from_rgb(rgb[0], rgb[1], rgb[2])
    }
    pub(super) fn selected_polyline_hit(
        &self,
        point: Point,
        tolerance: f32,
    ) -> Option<(u64, Option<usize>, Option<usize>)> {
        let selected = self.editor.selection_ids();
        let mut eligible: Vec<(
            u64,
            Vec<Point>,
            Vec<Option<crate::model::ArcSegment>>,
            bool,
            usize,
        )> = Vec::new();
        for e in self.editor.project.elements.iter().rev().filter(|e| {
            selected.contains(&e.id)
                && e.story == self.editor.story
                && self
                    .editor
                    .project
                    .plan_options
                    .element_visible(e, &self.editor.project.layers)
        }) {
            match &e.shape {
                Shape::Polyline { points, arcs } => {
                    eligible.push((e.id, points.clone(), arcs.clone(), false, 0))
                }
                Shape::Slab { .. } => {
                    let mut base = 0;
                    for ring in crate::slab_geometry::rings(e) {
                        let n = ring.points.len();
                        eligible.push((e.id, ring.points, ring.arcs, true, base));
                        base += n;
                    }
                }
                _ => {}
            }
        }
        let node = eligible
            .iter()
            .flat_map(|(id, points, _, _, base)| {
                points
                    .iter()
                    .enumerate()
                    .map(move |(i, p)| (*id, *base + i, p.distance(point)))
            })
            .filter(|(_, _, d)| *d <= tolerance)
            .min_by(|a, b| a.2.total_cmp(&b.2));
        if let Some((id, node, _)) = node {
            return Some((id, Some(node), None));
        }
        eligible
            .iter()
            .flat_map(|(id, points, arcs, closed, base)| {
                let count = points.len().saturating_sub(if *closed { 0 } else { 1 });
                (0..count).map(move |i| {
                    let a = points[i];
                    let b = points[(i + 1) % points.len()];
                    let path = arcs
                        .get(i)
                        .and_then(|a| *a)
                        .and_then(|arc| geometry::arc_points(a, b, arc))
                        .unwrap_or_else(|| vec![a, b]);
                    let distance = path
                        .windows(2)
                        .map(|p| geometry::segment_distance(point, p[0], p[1]))
                        .fold(f32::INFINITY, f32::min);
                    (*id, *base + i, distance)
                })
            })
            .filter(|(_, _, d)| *d <= tolerance)
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(id, edge, _)| (id, None, Some(edge)))
    }
    /// Hit-test the reference line of an already selected wall. Endpoints win
    /// over the segment so a handle just inside a thick wall still stretches
    /// the wall instead of moving it.
    pub(super) fn selected_wall_baseline_hit(
        &self,
        point: Point,
        tolerance: f32,
    ) -> Option<(u64, Option<usize>)> {
        let selected = self.editor.selection_ids();
        let walls = self.editor.project.elements.iter().rev().filter(|element| {
            selected.contains(&element.id)
                && element.story == self.editor.story
                && matches!(element.shape, Shape::Wall { .. })
                && self
                    .editor
                    .project
                    .plan_options
                    .element_visible(element, &self.editor.project.layers)
        });
        let mut endpoint: Option<(u64, usize, f32)> = None;
        for wall in walls.clone() {
            let Some((start, end)) = geometry::wall_reference_line(wall) else {
                continue;
            };
            for (node, candidate) in [start, end].into_iter().enumerate() {
                let distance = candidate.distance(point);
                if distance <= tolerance && endpoint.is_none_or(|(_, _, best)| distance < best) {
                    endpoint = Some((wall.id, node, distance));
                }
            }
        }
        if let Some((id, node, _)) = endpoint {
            return Some((id, Some(node)));
        }
        let mut segment: Option<(u64, f32)> = None;
        for wall in walls {
            let distance = crate::wall_geometry::project_station(wall, point).1;
            if distance <= tolerance && segment.is_none_or(|(_, best)| distance < best) {
                segment = Some((wall.id, distance));
            }
        }
        segment.map(|(id, _)| (id, None))
    }
    pub(super) fn paint_selected_wall_path(painter: &Painter, points: Vec<Pos2>) {
        if points.len() == 2 {
            paint_selected_wall_baseline(painter, points[0], points[1]);
        } else if let (Some(start), Some(end)) = (points.first().copied(), points.last().copied()) {
            painter.add(egui::Shape::line(
                points,
                Stroke::new(WALL_REFERENCE_LINE_WIDTH, WALL_REFERENCE_LINE),
            ));
            for handle in [start, end] {
                painter.rect_filled(
                    Rect::from_center_size(handle, Vec2::splat(WALL_REFERENCE_HANDLE_SIZE)),
                    0.,
                    Color32::BLACK,
                );
            }
        }
    }
    pub(super) fn select_cursor_feedback(&self, point: Point) -> CursorFeedback {
        match geometry::plan_hover_kind(
            &self.editor.project,
            self.editor.story,
            point,
            8. / self.plan_scale,
            6. / self.plan_scale,
        ) {
            Some(geometry::PlanHoverKind::Body) => CursorFeedback::Arrow,
            Some(geometry::PlanHoverKind::Node) => CursorFeedback::ArrowWithCheck,
            Some(geometry::PlanHoverKind::Edge) => CursorFeedback::ArrowWithTripleRay,
            None => CursorFeedback::Empty,
        }
    }

    pub(super) fn open_wall_edit(&mut self, id: u64, node: Option<usize>, at: Pos2) {
        self.wall_edit = Some(WallEdit {
            element_id: id,
            node,
            command: if node.is_some() {
                WallEditCommand::MoveNode
            } else {
                WallEditCommand::Move
            },
            palette_at: at + vec2(14., 12.),
            insert_anchor: self
                .editor
                .project
                .elements
                .iter()
                .find(|e| e.id == id)
                .and_then(geometry::wall_reference_line)
                .map(|(a, b)| Point::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5))
                .unwrap_or_default(),
            elevation: None,
            drag: None,
            snap: None,
            error: String::new(),
        });
        self.editor.status = if node.is_some() {
            "Wall reference-line endpoint edit · drag the handle to reshape the wall".into()
        } else {
            "Wall reference-line edit · drag the line to move the wall".into()
        };
    }
    /// Resolve a nearby wall reference line for the endpoint currently being
    /// dragged. Only endpoint edits reshape the wall; whole-line moves keep
    /// their length and relative geometry. The target is taken from the
    /// gesture-start project so other walls cannot move under the gesture.
    pub(super) fn wall_endpoint_snap(
        &self,
        before: &Project,
        edit: &WallEdit,
        pointer: Point,
        along: Point,
    ) -> Option<WallSnap> {
        if edit.command != WallEditCommand::MoveNode {
            return None;
        }
        let snap = geometry::wall_reference_snap(
            before,
            self.editor.story,
            edit.element_id,
            pointer,
            12. / self.plan_scale,
        )?;
        // Capture the target line from the raw pointer, but take the
        // along-line coordinate from the ordinary snap target. This keeps grid
        // or endpoint snapping along the wall while still landing exactly on
        // the neighbour's reference line.
        let point = before
            .elements
            .iter()
            .find(|element| element.id == snap.wall_id)
            .and_then(|wall| geometry::wall_reference_projection(wall, along))
            .unwrap_or(snap.point);
        Some(WallSnap {
            element_id: snap.wall_id,
            point,
        })
    }
    /// Resolve a live endpoint drag. Reference-line capture is evaluated from
    /// the raw pointer first: grid or object snapping must not pull the point
    /// away from a nearby wall baseline before the wall snap can see it.
    fn wall_endpoint_drag_target(
        &self,
        before: &Project,
        edit: &WallEdit,
        pointer: Point,
        shift: bool,
    ) -> (Point, Option<WallSnap>) {
        let (target, _) = self.target(pointer, shift);
        let snap = self.wall_endpoint_snap(before, edit, pointer, target);
        let target = snap.map_or(target, |snap| snap.point);
        (target, snap)
    }
    /// Preview a reference-line edit from the gesture-start snapshot. The
    /// stored endpoints are the only edited state: body faces, joins, openings
    /// and 3D all derive from them. Invalid candidates leave the last valid
    /// preview visible and report why they were rejected.
    pub(super) fn apply_wall_drag_preview(
        &mut self,
        before: &Project,
        edit: &WallEdit,
        start: Point,
        current: Point,
    ) -> bool {
        let Some(original) = before
            .elements
            .iter()
            .find(|element| element.id == edit.element_id)
        else {
            return false;
        };
        let Shape::Wall {
            start: source_start,
            end: source_end,
            ..
        } = original.shape
        else {
            return false;
        };
        let delta = Point::new(current.x - start.x, current.y - start.y);
        let (candidate_start, candidate_end) = match edit.command {
            WallEditCommand::MoveNode => match edit.node {
                Some(0) => (current, source_end),
                Some(1) => (source_start, current),
                _ => return false,
            },
            WallEditCommand::Move => (
                Point::new(source_start.x + delta.x, source_start.y + delta.y),
                Point::new(source_end.x + delta.x, source_end.y + delta.y),
            ),
        };
        if let Err(message) =
            before.validate_wall_reference_line(edit.element_id, candidate_start, candidate_end)
        {
            if let Some(active) = self.wall_edit.as_mut() {
                active.error = localized_error(self.language, &message);
            }
            return false;
        }
        self.editor.project = before.clone();
        if let Err(message) =
            self.editor
                .set_wall_reference_line(edit.element_id, candidate_start, candidate_end)
        {
            if let Some(active) = self.wall_edit.as_mut() {
                active.error = localized_error(self.language, &message);
            }
            return false;
        }
        if let Some(active) = self.wall_edit.as_mut() {
            active.error.clear();
        }
        true
    }
    pub(super) fn base_target(&self, p: Point, shift: bool) -> (Point, SnapKind) {
        // The chain's first node is not part of the committed model yet, so
        // the ordinary model snap cannot find it. Make it a magnetic target
        // while the draft is open: this also keeps grid snap from moving a
        // clicking-the-start gesture onto a different grid point.
        if !self.tracker_locked
            && let Some(start) = self.editor.wall_close_target()
            && p.distance(start) <= 12. / self.plan_scale
        {
            return (start, SnapKind::Endpoint);
        }
        if self.editor.tool == Tool::Polyline
            && let Some(anchor) = self.editor.polyline_points.last().copied()
        {
            if self.tracker_locked {
                return (self.tracker_target(anchor), SnapKind::Free);
            }
            if !self.editor.project.guide_settings.snap_enabled {
                let (base, kind) = geometry::snap(
                    &self.editor.project,
                    self.editor.story,
                    p,
                    9. / self.plan_scale,
                    self.editor.snap,
                );
                return self.apply_grid_snap(p, base, kind);
            }
            return geometry::polyline_snap_target(
                &self.editor.project,
                self.editor.story,
                &self.editor.polyline_points,
                p,
                9. / self.plan_scale,
                self.editor.snap,
            );
        }
        let (mut q, kind) = geometry::snap(
            &self.editor.project,
            self.editor.story,
            p,
            9. / self.plan_scale,
            self.editor.snap,
        );
        if let Some(a) = self.editor.anchor {
            if self.tracker_locked {
                return (self.tracker_target(a), SnapKind::Free);
            }
            if self.editor.tool == Tool::Wall
                && self.editor.wall_settings.geometry.geometry_method
                    != WallGeometryMethod::Rectangular
                && (self.editor.orthogonal || shift)
            {
                if (q.x - a.x).abs() > (q.y - a.y).abs() {
                    q.y = a.y
                } else {
                    q.x = a.x
                }
            }
        }
        (q, kind)
    }
    pub(super) fn tracker_target(&self, a: Point) -> Point {
        if matches!(self.editor.tool, Tool::Wall | Tool::Polyline) {
            if self.editor.tool == Tool::Wall
                && self.editor.wall_settings.geometry.geometry_method
                    == WallGeometryMethod::Rectangular
            {
                return Point::new(a.x + self.tracker_a, a.y + self.tracker_b);
            }
            let angle = self.tracker_b.to_radians();
            Point::new(
                a.x + self.tracker_a * angle.cos(),
                a.y + self.tracker_a * angle.sin(),
            )
        } else {
            Point::new(a.x + self.tracker_a, a.y + self.tracker_b)
        }
    }
    pub(super) fn wall_preview_segments(
        &self,
        cursor: Point,
        close_tolerance: f32,
    ) -> Vec<(Point, Point)> {
        let method = self.editor.wall_settings.geometry.geometry_method;
        match method {
            WallGeometryMethod::Single => self
                .editor
                .anchor
                .map(|start| {
                    let mut segments: Vec<_> = self
                        .editor
                        .wall_points
                        .windows(2)
                        .map(|pair| (pair[0], pair[1]))
                        .collect();
                    segments.push((start, cursor));
                    segments
                })
                .unwrap_or_default(),
            WallGeometryMethod::Rectangular => self
                .editor
                .anchor
                .map(|start| {
                    let mut segments: Vec<_> = self
                        .editor
                        .wall_points
                        .windows(2)
                        .map(|pair| (pair[0], pair[1]))
                        .collect();
                    segments.extend(rectangle_wall_segments(start, cursor));
                    segments
                })
                .unwrap_or_default(),
            WallGeometryMethod::Continuous | WallGeometryMethod::Polygonal => {
                let points = &self.editor.wall_points;
                let mut segments: Vec<_> =
                    points.windows(2).map(|pair| (pair[0], pair[1])).collect();
                let Some(last) = points.last().copied() else {
                    return segments;
                };
                if points.len() >= 3 && points[0].distance(cursor) <= close_tolerance {
                    segments.push((last, points[0]));
                } else if last.distance(cursor) >= 0.01 {
                    segments.push((last, cursor));
                }
                segments
            }
        }
    }
    pub(super) fn paint_wall_preview(
        &self,
        painter: &Painter,
        viewport: Rect,
        start: Point,
        end: Point,
    ) {
        if start.distance(end) < 0.01 {
            return;
        }
        let shape = Shape::Wall {
            start,
            end,
            thickness: self.editor.thickness,
            height: self.editor.height,
        };
        let tmp = Element {
            id: 0,
            name: String::new(),
            story: self.editor.story,
            material: self.editor.material,
            layer: self.editor.wall_layer,
            renovation: self.editor.wall_renovation,
            design_option: None,
            wall_contour: None,
            slab_contour: None,
            wall_settings: Some(self.editor.wall_settings.clone()),
            shape,
        };
        let points = geometry::footprint(&tmp)
            .into_iter()
            .map(|point| self.screen(viewport, point))
            .collect();
        painter.add(egui::Shape::convex_polygon(
            points,
            Color32::from_rgba_unmultiplied(39, 146, 137, 50),
            Stroke::new(1.5, ACCENT),
        ));
        painter.line_segment(
            [self.screen(viewport, start), self.screen(viewport, end)],
            Stroke::new(1., ACCENT),
        );
    }
    pub(super) fn commit_polyline_point(&mut self, point: Point, close_tolerance: f32) {
        let close_to_start = self.editor.polyline_points.len() >= 2
            && self
                .editor
                .polyline_points
                .first()
                .is_some_and(|start| start.distance(point) <= close_tolerance);
        if close_to_start {
            self.close_polyline();
        } else {
            self.editor.click(point, 6. / self.plan_scale);
        }
        self.tracker_locked = false;
        self.draft_id = None;
    }
    pub(super) fn close_polyline(&mut self) {
        if let Some(start) = self.editor.polyline_points.first().copied() {
            match self.editor.finish_polyline(Some(start)) {
                Ok(Some(_)) => self.preview_tool = None,
                Ok(None) => {}
                Err(error) => self.editor.status = error,
            }
        }
        self.tracker_locked = false;
        self.draft_id = None;
    }
    pub(super) fn finish_polyline(&mut self) {
        match self.editor.finish_polyline(None) {
            Ok(Some(_)) => self.preview_tool = None,
            Ok(None) => {}
            Err(error) => self.editor.status = error,
        }
        self.tracker_locked = false;
        self.draft_id = None;
    }
    pub(super) fn finish_wall_draft(&mut self) {
        // The click at the polygon start can already close and commit the
        // draft. A following double-click must not report a second, empty
        // completion or start a replacement wall.
        if !self.editor.wall_draft_active() {
            self.tracker_locked = false;
            self.draft_id = None;
            return;
        }
        let method = self.editor.wall_settings.geometry.geometry_method;
        let enough_input = match method {
            crate::wall::WallGeometryMethod::Single
            | crate::wall::WallGeometryMethod::Rectangular => false,
            crate::wall::WallGeometryMethod::Continuous => self.editor.wall_points.len() >= 2,
            crate::wall::WallGeometryMethod::Polygonal => self.editor.wall_points.len() >= 3,
        };
        if !enough_input {
            self.editor.cancel_wall_draft();
            self.editor.status = self.language.translate("Wall cancelled").into();
            self.preview_tool = None;
            self.tracker_locked = false;
            self.draft_id = None;
            return;
        }
        match self.editor.finish_wall(None) {
            Ok(_) => self.preview_tool = None,
            Err(error) => self.editor.status = localized_error(self.language, &error),
        }
        self.tracker_locked = false;
        self.draft_id = None;
    }
    pub(super) fn plan(&mut self, ui: &mut Ui) {
        let r = ui.available_rect_before_wrap();
        let trace_pointer = self.trace.pointer_mode_active();
        let trace_split = self.trace_split_regions(r);
        let hovered_before = ui.input(|i| i.pointer.hover_pos());
        let over_reference = trace_split
            .is_some_and(|(left, _)| hovered_before.is_some_and(|pos| left.contains(pos)));
        let interactive = !trace_pointer && !over_reference;
        let response = ui.allocate_rect(
            r,
            if interactive {
                Sense::click_and_drag()
            } else {
                Sense::hover()
            },
        );
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "Floor plan canvas"));
        let background = self.editor.project.grid_settings.background;
        ui.painter_at(r).rect_filled(
            r,
            0.,
            Color32::from_rgba_unmultiplied(
                background[0],
                background[1],
                background[2],
                background[3],
            ),
        );
        let reference_clip = trace_split.map_or(r, |(left, _)| left);
        let active_clip = trace_split.map_or(r, |(_, right)| right);
        if !self.trace.on_top {
            let reference_painter = ui.painter_at(reference_clip);
            self.paint_trace_reference(&reference_painter, r, Some(reference_clip));
        }
        let mut p = ui.painter_at(active_clip);
        if self.trace.enabled {
            p.set_opacity(f32::from(self.trace.active_opacity) / 255.);
        }
        let hovered = response.hovered();
        let pointer = ui.input(|i| i.pointer.hover_pos());
        if self.editor.tool != Tool::Select {
            self.wall_edit = None;
            self.wall_edit_palette_rect = None;
        }
        let interact_position =
            ui.input(|i| i.pointer.press_origin().or_else(|| i.pointer.latest_pos()));
        let over_wall_edit_palette = self
            .wall_edit_palette_rect
            .is_some_and(|rect| interact_position.is_some_and(|position| rect.contains(position)));
        if trace_pointer {
            self.trace_pointer_in_plan(ui, r);
        }
        if hovered {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.
                && let Some(pos) = pointer
            {
                let before = self.world(r, pos);
                self.plan_scale = (self.plan_scale * (scroll * 0.002).exp()).clamp(3., 250.);
                let after = self.world(r, pos);
                self.plan_center.x += before.x - after.x;
                self.plan_center.y += before.y - after.y;
            }
        }
        let pan = response.dragged_by(PointerButton::Secondary)
            || response.dragged_by(PointerButton::Middle)
            || (response.dragged_by(PointerButton::Primary)
                && ui.input(|i| i.key_down(Key::Space)));
        if pan {
            let d = ui.input(|i| i.pointer.delta());
            let radians = self
                .editor
                .project
                .plan_options
                .normalized_orientation()
                .to_radians();
            let (sin, cos) = radians.sin_cos();
            let view_x = d.x / self.plan_scale;
            let view_y = -d.y / self.plan_scale;
            self.plan_center.x -= view_x * cos + view_y * sin;
            self.plan_center.y -= -view_x * sin + view_y * cos;
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        }
        // Establish the editing session before Option is routed to parameter transfer.
        // A selected wall reference drag edits its node and commits on release.
        if self.wall_insert.is_none()
            && !self.trim_modifier_active(ui.ctx())
            && self.reshape_session.is_none()
            && self.pointer_drag.is_none()
            && self.editor.tool == Tool::Select
            && !pan
            && !trace_pointer
            && !over_wall_edit_palette
            && response.drag_started_by(PointerButton::Primary)
            && let Some(pos) = ui.input(|i| i.pointer.press_origin())
        {
            let origin = self.world(r, pos);
            let baseline = self.selected_wall_baseline_hit(origin, 10. / self.plan_scale);
            if let Some((origin, ids)) =
                self.selected_wall_junction_plan(origin, 10. / self.plan_scale)
            {
                self.start_wall_junction(origin, ids, pos, None, true);
            } else if let Some((id, None)) = baseline {
                self.open_wall_edit(id, None, pos);
                self.set_wall_edit_hit(origin, None);
                self.start_wall_insert();
                if let Some(edit) = &mut self.wall_insert {
                    edit.canvas_drag = true;
                }
            } else if let Some((id, node, edge)) =
                self.selected_polyline_hit(origin, 9. / self.plan_scale)
            {
                self.start_polyline_pet(id, node, edge, pos, origin, true);
            } else if baseline.is_none() {
                let hit = self
                    .selected_polyline_hit(origin, 9. / self.plan_scale)
                    .map(|hit| hit.0)
                    .or_else(|| {
                        geometry::hit_plan(
                            &self.editor.project,
                            self.editor.story,
                            origin,
                            6. / self.plan_scale,
                        )
                    });
                if let Some(id) = hit {
                    if !self.editor.selection_ids().contains(&id) {
                        self.editor.set_selection(vec![id]);
                    }
                    self.start_direct_pointer_drag(origin);
                }
            }
        }
        // Parameter transfer cursor: armed one-shot modes, or holding Option
        // (+Cmd to inject) over the plan, show the eyedropper/syringe. It is
        // primed with liquid while a transferable element is under the
        // pointer, and that element is highlighted for the click to act on.
        self.transfer_hover = None;
        self.transfer_cursor = None;
        let transfer_mode = if self.pick_armed {
            Some(crate::transfer_cursor::TransferCursor::PickUp)
        } else if self.inject_armed {
            Some(crate::transfer_cursor::TransferCursor::Inject)
        } else if hovered && ui.input(|i| i.modifiers.alt) {
            Some(if ui.input(|i| i.modifiers.command) {
                crate::transfer_cursor::TransferCursor::Inject
            } else {
                crate::transfer_cursor::TransferCursor::PickUp
            })
        } else {
            None
        };
        let transfer_available = self.wall_insert.is_none()
            && !self.trim_modifier_active(ui.ctx())
            && self.reshape_session.is_none()
            && self.pointer_drag.is_none()
            && !pan
            && !trace_pointer
            && !ui.input(|i| i.key_down(Key::Space));
        // While the transfer cursor is active it is the ONLY pointer feedback:
        // the snap/hover markers stay hidden until the modifiers are released.
        let transfer_cursor_active =
            transfer_mode.is_some() && transfer_available && response.hovered();
        if let Some(kind) = transfer_mode.filter(|_| transfer_cursor_active) {
            if let Some(pos) = pointer {
                let world = self.world(r, pos);
                let hit = geometry::hit_plan(
                    &self.editor.project,
                    self.editor.story,
                    world,
                    6. / self.plan_scale,
                );
                let primed = self.transfer_hover_state(kind, hit);
                // The glyph itself is painted after the elements; the system
                // cursor is hidden so the painted tool is the only pointer.
                self.transfer_cursor = Some((kind, primed, pos));
                ui.ctx().set_cursor_icon(CursorIcon::None);
            }
        }
        let grid = &self.editor.project.grid_settings;
        if grid.grid_visible_at(self.plan_scale) {
            let local = [
                self.world(r, r.left_top()),
                self.world(r, r.right_top()),
                self.world(r, r.left_bottom()),
                self.world(r, r.right_bottom()),
            ]
            .map(|point| grid.local_point(point));
            let min_u = local
                .iter()
                .map(|point| point.x)
                .fold(f32::INFINITY, f32::min);
            let max_u = local
                .iter()
                .map(|point| point.x)
                .fold(f32::NEG_INFINITY, f32::max);
            let min_v = local
                .iter()
                .map(|point| point.y)
                .fold(f32::INFINITY, f32::min);
            let max_v = local
                .iter()
                .map(|point| point.y)
                .fold(f32::NEG_INFINITY, f32::max);
            for line in grid.lines_in_local(min_u, max_u, min_v, max_v) {
                let color = grid.line_color(line.main);
                p.line_segment(
                    [self.screen(r, line.start), self.screen(r, line.end)],
                    Stroke::new(
                        if line.main { 0.75 } else { 0.45 },
                        Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3]),
                    ),
                );
            }
        }
        // Slabs beneath walls, independently of creation order.
        let mut elements: Vec<_> = self
            .editor
            .project
            .elements
            .iter()
            .filter(|e| {
                e.story == self.editor.story
                    && self
                        .editor
                        .project
                        .plan_options
                        .element_visible(e, &self.editor.project.layers)
            })
            .collect();
        elements.sort_by_key(|e| matches!(e.shape, Shape::Wall { .. }));
        let selected_ids = self.editor.selection_ids();
        for e in elements {
            let selected = selected_ids.contains(&e.id);
            if matches!(e.shape, Shape::Wall { .. } | Shape::Opening { .. }) {
                continue;
            }
            let Some(style) = self.plan_style(e) else {
                continue;
            };
            if let Shape::Polyline {
                ref points,
                ref arcs,
            } = e.shape
            {
                let tessellated = geometry::polyline_path(points, arcs);
                let path: Vec<Pos2> = tessellated
                    .iter()
                    .map(|point| self.screen(r, *point))
                    .collect();
                if selected {
                    Self::paint_selected_wall_path(&p, path);
                    for point in points.iter().skip(1).take(points.len().saturating_sub(2)) {
                        p.rect_filled(
                            Rect::from_center_size(
                                self.screen(r, *point),
                                Vec2::splat(WALL_REFERENCE_HANDLE_SIZE),
                            ),
                            0.,
                            Color32::BLACK,
                        );
                    }
                } else {
                    p.add(egui::Shape::line(
                        path,
                        Stroke::new(
                            self.plan_pen_px(style.line_width_mm),
                            Self::plan_color(style.line),
                        ),
                    ));
                }
                continue;
            }
            if matches!(e.shape, Shape::Slab { .. }) && e.slab_contour.is_some() {
                let fill = if selected {
                    Color32::from_rgb(195, 225, 218)
                } else {
                    style.fill.map_or(Color32::TRANSPARENT, Self::plan_color)
                };
                let stroke = if selected {
                    Stroke::new(2., ACCENT)
                } else {
                    Stroke::new(
                        self.plan_pen_px(style.line_width_mm),
                        Self::plan_color(style.line),
                    )
                };
                Self::paint_slab_polygon(
                    &p,
                    e,
                    |point| self.screen(r, point),
                    self.trace_active_fill(fill),
                    stroke,
                );
                if selected {
                    for point in crate::slab_geometry::rings(e)
                        .into_iter()
                        .flat_map(|r| r.points)
                    {
                        p.rect_filled(
                            Rect::from_center_size(self.screen(r, point), Vec2::splat(5.)),
                            0.,
                            Color32::BLACK,
                        );
                    }
                }
                continue;
            }
            let pts: Vec<_> = geometry::footprint(e)
                .into_iter()
                .map(|v| self.screen(r, v))
                .collect();
            let (fill, stroke) = if selected {
                (Color32::from_rgb(195, 225, 218), Stroke::new(2., ACCENT))
            } else {
                let width = self.plan_pen_px(style.line_width_mm);
                (
                    style.fill.map_or(Color32::TRANSPARENT, Self::plan_color),
                    Stroke::new(width, Self::plan_color(style.line)),
                )
            };
            p.add(egui::Shape::convex_polygon(
                pts.clone(),
                self.trace_active_fill(fill),
                stroke,
            ));
            if style.draw_treads
                && let Shape::Stair { ref spec } = e.shape
                && let Ok(stair) = crate::stairs::generate(spec, 0.0)
            {
                for [a, b] in stair.tread_lines {
                    p.line_segment(
                        [self.screen(r, a), self.screen(r, b)],
                        Stroke::new(
                            self.plan_pen_px(style.line_width_mm) * 0.8,
                            Self::plan_color(style.line),
                        ),
                    );
                }
            }
        }
        let all_walls: Vec<_> = self
            .editor
            .project
            .elements
            .iter()
            .filter(|e| e.story == self.editor.story && matches!(e.shape, Shape::Wall { .. }))
            .collect();
        let walls: Vec<_> = all_walls
            .iter()
            .copied()
            .filter(|wall| wall.wall_contour.is_none())
            .collect();
        for wall in all_walls
            .iter()
            .copied()
            .filter(|wall| wall.wall_contour.is_some())
        {
            let Some(style) = self.plan_style(wall) else {
                continue;
            };
            let selected = selected_ids.contains(&wall.id);
            let fill = if selected {
                WALL_SELECTION_FILL
            } else {
                self.trace_active_fill(
                    style
                        .fill
                        .map_or(Self::plan_color(style.line), Self::plan_color),
                )
            };
            for strip in crate::wall_geometry::strips(wall) {
                p.add(egui::Shape::convex_polygon(
                    strip
                        .into_iter()
                        .map(|point| self.screen(r, point))
                        .collect(),
                    fill,
                    Stroke::new(0.5, fill),
                ));
            }
            let stroke = if selected {
                Stroke::new(1.5, WALL_SELECTION_OUTLINE)
            } else {
                Stroke::new(
                    self.plan_pen_px(style.line_width_mm),
                    Self::plan_color(style.line),
                )
            };
            p.add(egui::Shape::closed_line(
                crate::wall_geometry::outline(wall)
                    .into_iter()
                    .map(|point| self.screen(r, point))
                    .collect(),
                stroke,
            ));
            if selected {
                for point in geometry::wall_edit_nodes(wall).into_iter().skip(2) {
                    p.rect_filled(
                        Rect::from_center_size(
                            self.screen(r, point),
                            Vec2::splat(WALL_REFERENCE_HANDLE_SIZE),
                        ),
                        0.,
                        Color32::BLACK,
                    );
                }
            }
        }
        // Joins are physical wall geometry, not a property of the fill or pen
        // group. Compute them across every wall on the story — including
        // hidden-layer walls, which still trim visible walls per the AC29
        // layer intersection rules — so a new wall can join an existing wall
        // with another material, renovation status or visibility.
        let layer_groups = self.editor.project.layers.wall_groups(&walls);
        let footprints_by_id: std::collections::HashMap<_, _> =
            geometry::plan_wall_polygons_by_id(&walls, &layer_groups)
                .into_iter()
                .collect();
        let story_footprints: Vec<_> = walls
            .iter()
            .map(|wall| footprints_by_id[&wall.id].clone())
            .collect();
        for material in Material::ALL {
            for renovation in [
                RenovationStatus::Existing,
                RenovationStatus::Demolition,
                RenovationStatus::New,
            ] {
                let same_material: Vec<_> = walls
                    .iter()
                    .copied()
                    .filter(|wall| wall.material == material && wall.renovation == renovation)
                    .collect();
                if same_material.is_empty() {
                    continue;
                }
                let Some(style) = self.plan_style(same_material[0]) else {
                    continue;
                };
                let footprints: Vec<_> = same_material
                    .iter()
                    .map(|wall| footprints_by_id[&wall.id].clone())
                    .collect();
                for (wall, footprint) in same_material.iter().zip(&footprints) {
                    if !selected_ids.contains(&wall.id) {
                        let wall_fill = self.trace_active_fill(
                            style
                                .fill
                                .map_or(Self::plan_color(style.line), Self::plan_color),
                        );
                        Self::paint_wall_polygon(
                            &p,
                            footprint,
                            |point| self.screen(r, point),
                            wall_fill,
                            // Slight same-colour bleed seals sub-pixel gaps where
                            // separately editable wall footprints meet.
                            Stroke::new(0.8, wall_fill),
                        );
                    }
                }
            }
        }
        // Resolve the whole story's exposed perimeter before applying pen
        // styles. Fill and renovation groups are presentation batches; they
        // must not split physical joins and stroke an internal wall end.
        let mut outline_segments: std::collections::BTreeMap<([u8; 3], u32), Vec<(Point, Point)>> =
            std::collections::BTreeMap::new();
        for (wall_index, start, end) in
            geometry::wall_polygon_outline_segments_by_wall(&story_footprints)
        {
            let Some(style) = self.plan_style(walls[wall_index]) else {
                continue;
            };
            let width_key = (style.line_width_mm * 1000.0).round() as u32;
            outline_segments
                .entry((style.line, width_key))
                .or_default()
                .push((start, end));
        }
        for ((line, width_key), segments) in outline_segments {
            let wall_stroke = Stroke::new(
                self.plan_pen_px(width_key as f32 / 1000.0),
                Self::plan_color(line),
            );
            for mut path in geometry::wall_outline_paths_from_segments(segments) {
                let closed = path.len() > 2 && path[0].distance(*path.last().unwrap()) <= 1e-3;
                if closed {
                    path.pop();
                }
                let points: Vec<_> = path
                    .into_iter()
                    .map(|point| self.screen(r, point))
                    .collect();
                if closed {
                    p.add(egui::Shape::closed_line(points, wall_stroke));
                } else {
                    p.add(egui::Shape::line(points, wall_stroke));
                }
            }
        }
        // Selected walls retain an individual highlight and editable handles.
        let selected_walls: Vec<_> = walls
            .iter()
            .copied()
            .filter(|wall| selected_ids.contains(&wall.id))
            .collect();
        for wall in &selected_walls {
            let joined = footprints_by_id
                .get(&wall.id)
                .cloned()
                .unwrap_or_else(|| geometry::footprint(wall).to_vec());
            Self::paint_wall_polygon(
                &p,
                &joined,
                |point| self.screen(r, point),
                WALL_SELECTION_FILL,
                Stroke::new(1.5, WALL_SELECTION_OUTLINE),
            );
        }
        // Paint every baseline after the fills so adjacently selected walls
        // cannot hide each other's reference lines at a junction.
        for wall in all_walls
            .iter()
            .copied()
            .filter(|wall| selected_ids.contains(&wall.id) && self.plan_style(wall).is_some())
        {
            Self::paint_selected_wall_path(
                &p,
                crate::wall_geometry::reference_path(wall)
                    .into_iter()
                    .map(|point| self.screen(r, point))
                    .collect(),
            );
        }
        // Hosted openings erase the wall band and draw their plan symbol.
        let background = self.editor.project.grid_settings.background;
        let hole_fill = Color32::from_rgba_unmultiplied(
            background[0],
            background[1],
            background[2],
            background[3],
        );
        let openings: Vec<_> = self
            .editor
            .project
            .elements
            .iter()
            .filter(|element| {
                element.story == self.editor.story
                    && matches!(element.shape, Shape::Opening { .. })
                    && self
                        .editor
                        .project
                        .plan_options
                        .element_visible(element, &self.editor.project.layers)
            })
            .collect();
        for opening in openings {
            let Some(outline) = geometry::opening_footprint(&self.editor.project, opening) else {
                continue;
            };
            let selected = selected_ids.contains(&opening.id);
            let style = self.plan_style(opening);
            let color = if selected {
                ACCENT
            } else {
                style.map_or(INK, |style| Self::plan_color(style.line))
            };
            let stroke = Stroke::new(
                if selected {
                    2.2
                } else {
                    self.plan_pen_px(style.map_or(0.18, |style| style.line_width_mm))
                },
                color,
            );
            let points: Vec<Pos2> = outline.iter().map(|point| self.screen(r, *point)).collect();
            let actual_outline = geometry::element_plan_outline(&self.editor.project, opening);
            for triangle in geometry::triangulate_polygon(&actual_outline) {
                p.add(egui::Shape::convex_polygon(
                    triangle
                        .into_iter()
                        .map(|point| self.screen(r, point))
                        .collect(),
                    hole_fill,
                    Stroke::new(0.5, hole_fill),
                ));
            }
            p.line_segment([points[0], points[3]], stroke);
            p.line_segment([points[1], points[2]], stroke);
            let Shape::Opening { kind, .. } = opening.shape else {
                continue;
            };
            match kind {
                OpeningKind::Door => {
                    let Some((a, b)) = geometry::opening_span(&self.editor.project, opening) else {
                        continue;
                    };
                    let hinge = a;
                    let other = b;
                    let radius = hinge.distance(other);
                    let face_a = Point::new(
                        (outline[0].x + outline[3].x) * 0.5,
                        (outline[0].y + outline[3].y) * 0.5,
                    );
                    let face_b = Point::new(
                        (outline[1].x + outline[2].x) * 0.5,
                        (outline[1].y + outline[2].y) * 0.5,
                    );
                    let normal_length = face_a.distance(face_b).max(1e-6);
                    let normal = Point::new(
                        (face_a.x - face_b.x) / normal_length,
                        (face_a.y - face_b.y) / normal_length,
                    );
                    let tip = Point::new(hinge.x + normal.x * radius, hinge.y + normal.y * radius);
                    p.line_segment([self.screen(r, hinge), self.screen(r, tip)], stroke);
                    let start_angle = (tip.y - hinge.y).atan2(tip.x - hinge.x);
                    let end_angle = (other.y - hinge.y).atan2(other.x - hinge.x);
                    let mut sweep = end_angle - start_angle;
                    while sweep > std::f32::consts::PI {
                        sweep -= std::f32::consts::TAU;
                    }
                    while sweep < -std::f32::consts::PI {
                        sweep += std::f32::consts::TAU;
                    }
                    let mut previous = self.screen(r, tip);
                    for step in 1..=16 {
                        let angle = start_angle + sweep * step as f32 / 16.;
                        let point = Point::new(
                            hinge.x + angle.cos() * radius,
                            hinge.y + angle.sin() * radius,
                        );
                        let current = self.screen(r, point);
                        p.line_segment([previous, current], Stroke::new(stroke.width * 0.6, color));
                        previous = current;
                    }
                }
                OpeningKind::Window => {
                    let Shape::Opening {
                        host_wall,
                        offset,
                        width,
                        ..
                    } = opening.shape
                    else {
                        unreachable!()
                    };
                    if let Some(host) = self
                        .editor
                        .project
                        .elements
                        .iter()
                        .find(|wall| wall.id == host_wall)
                    {
                        let strips = crate::wall_geometry::strips_between(
                            host,
                            offset - width * 0.5,
                            offset + width * 0.5,
                        );
                        let mut centers = Vec::new();
                        if let Some(first) = strips.first() {
                            centers.push(Point::new(
                                (first[0].x + first[3].x) * 0.5,
                                (first[0].y + first[3].y) * 0.5,
                            ));
                        }
                        centers.extend(strips.iter().map(|strip| {
                            Point::new(
                                (strip[1].x + strip[2].x) * 0.5,
                                (strip[1].y + strip[2].y) * 0.5,
                            )
                        }));
                        p.add(egui::Shape::line(
                            centers
                                .into_iter()
                                .map(|point| self.screen(r, point))
                                .collect(),
                            stroke,
                        ));
                    }
                }
            }
            if selected {
                p.add(egui::Shape::closed_line(
                    actual_outline
                        .into_iter()
                        .map(|point| self.screen(r, point))
                        .collect(),
                    Stroke::new(2.2, ACCENT),
                ));
            }
        }
        p.set_opacity(1.);
        // AC29 pre-highlights the element the armed cursor will act on.
        if let Some(id) = self.transfer_hover
            && let Some(element) = self
                .editor
                .project
                .elements
                .iter()
                .find(|element| element.id == id)
        {
            self.paint_transfer_hover(&p, r, element);
        }
        // The transfer tool glyph is the pointer while the mode is active.
        if let Some((kind, primed, pos)) = self.transfer_cursor {
            match kind {
                crate::transfer_cursor::TransferCursor::PickUp => {
                    crate::transfer_cursor::paint_pick_up(&p, pos, primed);
                }
                crate::transfer_cursor::TransferCursor::Inject => {
                    crate::transfer_cursor::paint_inject(&p, pos, primed);
                }
            }
        }
        if self.trace.on_top {
            let reference_painter = ui.painter_at(reference_clip);
            self.paint_trace_reference(&reference_painter, r, Some(reference_clip));
        }
        if let Some((left, right)) = trace_split {
            let x = left.right();
            ui.painter().line_segment(
                [pos2(x, r.top()), pos2(x, r.bottom())],
                Stroke::new(1.5, LINE),
            );
            let handle = Rect::from_min_max(pos2(x - 3., r.top()), pos2(x + 3., r.bottom()));
            let split_response = ui.interact(handle, Id::new("trace_splitter"), Sense::drag());
            if split_response.dragged() {
                let delta = ui.input(|i| i.pointer.delta().x) / r.width();
                let ratio = (x - r.left()) / r.width() + delta;
                self.trace.splitter = Some(ratio.clamp(0.1, 0.9));
            }
            split_response.on_hover_cursor(CursorIcon::ResizeHorizontal);
            ui.painter().text(
                left.left_top() + vec2(10., 8.),
                Align2::LEFT_TOP,
                self.language.translate("Reference"),
                FontId::proportional(10.),
                MUTED,
            );
            ui.painter().text(
                right.right_top() + vec2(-10., 8.),
                Align2::RIGHT_TOP,
                self.language.translate("Active"),
                FontId::proportional(10.),
                MUTED,
            );
        }
        // Associative overall dimensions, computed from the current story.
        let walls: Vec<_> = self
            .editor
            .project
            .elements
            .iter()
            .filter(|e| {
                e.story == self.editor.story
                    && matches!(e.shape, Shape::Wall { .. })
                    && self
                        .editor
                        .project
                        .plan_options
                        .element_visible(e, &self.editor.project.layers)
            })
            .flat_map(geometry::footprint)
            .collect();
        if !walls.is_empty() {
            let min = walls
                .iter()
                .fold(Point::new(f32::INFINITY, f32::INFINITY), |a, b| {
                    Point::new(a.x.min(b.x), a.y.min(b.y))
                });
            let max = walls
                .iter()
                .fold(Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY), |a, b| {
                    Point::new(a.x.max(b.x), a.y.max(b.y))
                });
            let a = self.screen(r, Point::new(min.x, max.y));
            let b = self.screen(r, max);
            let y = a.y - 28.;
            p.line_segment([pos2(a.x, y), pos2(b.x, y)], Stroke::new(0.8, MUTED));
            for x in [a.x, b.x] {
                p.line_segment(
                    [pos2(x, a.y - 6.), pos2(x, y - 7.)],
                    Stroke::new(0.8, MUTED),
                );
                p.line_segment(
                    [pos2(x - 3., y + 3.), pos2(x + 3., y - 3.)],
                    Stroke::new(1., INK),
                );
            }
            p.text(
                pos2((a.x + b.x) / 2., y - 9.),
                Align2::CENTER_BOTTOM,
                self.editor
                    .project
                    .plan_options
                    .dimension_style
                    .format_length(max.x - min.x),
                FontId::monospace(11.),
                INK,
            );
        }
        if let Some(area) = &self.editor.marquee {
            let pts: Vec<Pos2> = area.vertices.iter().map(|v| self.screen(r, *v)).collect();
            for i in 0..pts.len() {
                let a = pts[i];
                let b = pts[(i + 1) % pts.len()];
                let length = a.distance(b).max(1.);
                let direction = (b - a) / length;
                let mut offset = 0.;
                while offset < length {
                    let end = (offset + 7.).min(length);
                    p.line_segment(
                        [a + direction * offset, a + direction * end],
                        Stroke::new(1.5, ACCENT),
                    );
                    offset += 12.;
                }
            }
        }
        if let Some((a, b)) = self.marquee_preview {
            let pts: Vec<_> = geometry::rectangle_polygon(a, b)
                .iter()
                .map(|v| self.screen(r, *v))
                .collect();
            p.add(egui::Shape::closed_line(pts, Stroke::new(1.2, ACCENT)));
        }
        if self.wall_insert_canvas(ui, r, &response, false) {
            return;
        }
        if self.reshape_canvas(ui, r, &response) {
            return;
        }
        let pointer_drag_active = self.pointer_drag.is_some();
        self.pointer_drag_canvas(ui, r, &response, false);
        if pointer_drag_active {
            return;
        }
        if !self.editor.polyline_points.is_empty() {
            let mut path: Vec<Pos2> = self
                .editor
                .polyline_points
                .iter()
                .map(|point| self.screen(r, *point))
                .collect();
            if let Some(pos) = pointer.filter(|point| r.contains(*point)) {
                let (preview_point, _) =
                    self.target(self.world(r, pos), ui.input(|i| i.modifiers.shift));
                path.push(self.screen(r, preview_point));
            }
            if path.len() >= 2 {
                p.add(egui::Shape::line(path, Stroke::new(1.5, ACCENT)));
            }
            for point in &self.editor.polyline_points {
                let node = self.screen(r, *point);
                p.circle_filled(node, 3.5, Color32::WHITE);
                p.circle_stroke(node, 1.2, Stroke::new(1.2, ACCENT));
            }
        }
        if !self.marquee_points.is_empty() {
            let mut preview = self.marquee_points.clone();
            if let Some(pos) = pointer.filter(|v| r.contains(*v)) {
                let (q, _) = self.target(self.world(r, pos), ui.input(|i| i.modifiers.shift));
                if self.marquee_geometry == MarqueeGeometry::Polygon
                    || self.marquee_geometry == MarqueeGeometry::RotatedRectangle
                {
                    if self.marquee_geometry == MarqueeGeometry::Polygon || preview.len() < 2 {
                        preview.push(q);
                    } else if let Some(poly) =
                        geometry::rotated_rectangle_polygon(preview[0], preview[1], q)
                    {
                        preview = poly;
                    }
                }
            }
            let pts: Vec<_> = preview.iter().map(|v| self.screen(r, *v)).collect();
            if pts.len() >= 2 {
                p.add(egui::Shape::line(pts, Stroke::new(1.3, ACCENT)));
            }
            for v in self.marquee_points.iter() {
                p.circle_filled(self.screen(r, *v), 3.5, Color32::WHITE);
                p.circle_stroke(self.screen(r, *v), 3.5, Stroke::new(1., ACCENT));
            }
        }
        if self.editor.project.grid_settings.origin_display {
            let origin = self.screen(r, Point::default());
            p.line_segment(
                [origin - vec2(8., 0.), origin + vec2(8., 0.)],
                Stroke::new(1., ACCENT),
            );
            p.line_segment(
                [origin - vec2(0., 8.), origin + vec2(0., 8.)],
                Stroke::new(1., ACCENT),
            );
            p.text(
                origin + vec2(10., 10.),
                Align2::LEFT_TOP,
                "0, 0",
                FontId::monospace(9.),
                ACCENT,
            );
        }
        if let Some(tool) = &self.rotated_grid_tool
            && let Some(start) = tool.start
            && let Some(pos) = pointer.filter(|v| r.contains(*v))
        {
            let raw = self.world(r, pos);
            let (end, _) = self.target(raw, ui.input(|i| i.modifiers.shift));
            let a = self.screen(r, start);
            let b = self.screen(r, end);
            p.line_segment([a, b], Stroke::new(1.5, ACCENT));
            if let Some(angle) = crate::grids::GridSettings::angle_from_vector(start, end) {
                p.text(
                    b + vec2(12., -24.),
                    Align2::LEFT_BOTTOM,
                    format!("{angle:.2}°"),
                    FontId::monospace(10.),
                    ACCENT,
                );
            }
        }
        if self.guide_canvas(ui, r, &response) {
            return;
        }
        if let Some(pos) = pointer.filter(|v| r.contains(*v)) {
            let raw = self.world(r, pos);
            let (q, kind) = self.target(raw, ui.input(|i| i.modifiers.shift));
            self.hover_point = q;
            if self.orientation_pick_active
                && let Some(start) = self.orientation_pick_start
            {
                let a = self.screen(r, start);
                let b = self.screen(r, q);
                p.line_segment([a, b], Stroke::new(1.6, ACCENT));
                let angle = (q.y - start.y).atan2(q.x - start.x).to_degrees();
                p.text(
                    b + vec2(12., -22.),
                    Align2::LEFT_BOTTOM,
                    format!("{angle:.2}°"),
                    FontId::monospace(10.),
                    ACCENT,
                );
            }
            if !matches!(self.editor.tool, Tool::Select | Tool::Marquee) {
                let s = self.screen(r, q);
                if self.editor.snap {
                    p.rect_stroke(
                        Rect::from_center_size(s, vec2(9., 9.)),
                        0.,
                        Stroke::new(1.5, ACCENT),
                        StrokeKind::Middle,
                    );
                    p.text(
                        s + vec2(12., 12.),
                        Align2::LEFT_TOP,
                        format!("{kind:?}"),
                        FontId::proportional(10.),
                        ACCENT,
                    );
                }
                if self.editor.tool == Tool::Wall {
                    if let Some(a) = self.editor.anchor {
                        let rectangular = self.editor.wall_settings.geometry.geometry_method
                            == WallGeometryMethod::Rectangular;
                        if !self.tracker_locked {
                            if rectangular {
                                self.tracker_a = q.x - a.x;
                                self.tracker_b = q.y - a.y;
                            } else {
                                self.tracker_a = a.distance(q);
                                self.tracker_b = (q.y - a.y).atan2(q.x - a.x).to_degrees();
                            }
                        }
                        for (start, end) in self.wall_preview_segments(q, 9. / self.plan_scale) {
                            self.paint_wall_preview(&p, r, start, end);
                        }
                        let label = if rectangular {
                            format!(
                                "{} × {}",
                                self.editor
                                    .project
                                    .plan_options
                                    .dimension_style
                                    .format_length((q.x - a.x).abs()),
                                self.editor
                                    .project
                                    .plan_options
                                    .dimension_style
                                    .format_length((q.y - a.y).abs())
                            )
                        } else {
                            format!(
                                "{}  /  {:.1}°",
                                self.editor
                                    .project
                                    .plan_options
                                    .dimension_style
                                    .format_length(a.distance(q)),
                                (q.y - a.y).atan2(q.x - a.x).to_degrees()
                            )
                        };
                        let boxr = Rect::from_min_size(s + vec2(18., -42.), vec2(177., 27.));
                        p.rect_filled(boxr, 4., INK);
                        p.text(
                            boxr.center(),
                            Align2::CENTER_CENTER,
                            label,
                            FontId::monospace(11.),
                            Color32::WHITE,
                        );
                    }
                } else if matches!(self.editor.tool, Tool::Door | Tool::Window) {
                    let kind = if self.editor.tool == Tool::Door {
                        OpeningKind::Door
                    } else {
                        OpeningKind::Window
                    };
                    if let Ok(placement) =
                        self.editor.opening_placement(kind, q, 9. / self.plan_scale)
                        && let Some(outline) =
                            geometry::opening_footprint(&self.editor.project, &placement)
                    {
                        let pts: Vec<Pos2> =
                            outline.iter().map(|point| self.screen(r, *point)).collect();
                        p.add(egui::Shape::convex_polygon(
                            pts,
                            Color32::from_rgba_unmultiplied(39, 146, 137, 50),
                            Stroke::new(1.6, ACCENT),
                        ));
                        if let Shape::Opening {
                            width,
                            height,
                            sill,
                            ..
                        } = placement.shape
                        {
                            let label = format!("{width:.2} × {height:.2}  ·  {sill:.2}");
                            let boxr = Rect::from_min_size(s + vec2(18., -42.), vec2(177., 27.));
                            p.rect_filled(boxr, 4., INK);
                            p.text(
                                boxr.center(),
                                Align2::CENTER_CENTER,
                                label,
                                FontId::monospace(11.),
                                Color32::WHITE,
                            );
                        }
                    }
                } else if let Some(a) = self.editor.anchor {
                    if !self.tracker_locked {
                        self.tracker_a = q.x - a.x;
                        self.tracker_b = q.y - a.y;
                    }
                    let shape = match self.editor.tool {
                        Tool::Wall => unreachable!("walls use the wall preview"),
                        Tool::Slab => Shape::Slab {
                            min: Point::new(a.x.min(q.x), a.y.min(q.y)),
                            max: Point::new(a.x.max(q.x), a.y.max(q.y)),
                            thickness: self.editor.thickness,
                        },
                        Tool::Stair => {
                            let mut spec = self.editor.stair_spec.clone();
                            spec.start = a;
                            spec.end = q;
                            Shape::Stair { spec }
                        }
                        Tool::Polyline => Shape::Polyline {
                            points: vec![a, q],
                            arcs: vec![],
                        },
                        _ => Shape::Slab {
                            min: Point::new(a.x.min(q.x), a.y.min(q.y)),
                            max: Point::new(a.x.max(q.x), a.y.max(q.y)),
                            thickness: self.editor.thickness,
                        },
                    };
                    let tmp = Element {
                        id: 0,
                        name: String::new(),
                        story: self.editor.story,
                        material: self.editor.material,
                        layer: match shape {
                            Shape::Wall { .. } => self.editor.wall_layer,
                            _ => ElementLayer(ARCHICAD_LAYER),
                        },
                        renovation: self.editor.wall_renovation,
                        design_option: None,
                        wall_contour: None,
                        slab_contour: None,
                        wall_settings: if matches!(shape, Shape::Wall { .. }) {
                            Some(self.editor.wall_settings.clone())
                        } else {
                            None
                        },
                        shape,
                    };
                    let pts = geometry::footprint(&tmp)
                        .into_iter()
                        .map(|v| self.screen(r, v))
                        .collect();
                    p.add(egui::Shape::convex_polygon(
                        pts,
                        Color32::from_rgba_unmultiplied(39, 146, 137, 50),
                        Stroke::new(1.5, ACCENT),
                    ));
                    if let Shape::Stair { ref spec } = tmp.shape
                        && let Ok(stair) = crate::stairs::generate(spec, 0.0)
                    {
                        for [ta, tb] in stair.tread_lines {
                            p.line_segment(
                                [self.screen(r, ta), self.screen(r, tb)],
                                Stroke::new(0.9, ACCENT),
                            );
                        }
                    }
                    p.line_segment([self.screen(r, a), s], Stroke::new(1., ACCENT));
                    let label = format!(
                        "{}  /  {:.1}°",
                        self.editor
                            .project
                            .plan_options
                            .dimension_style
                            .format_length(a.distance(q)),
                        (q.y - a.y).atan2(q.x - a.x).to_degrees()
                    );
                    let boxr = Rect::from_min_size(s + vec2(18., -42.), vec2(177., 27.));
                    p.rect_filled(boxr, 4., INK);
                    p.text(
                        boxr.center(),
                        Align2::CENTER_CENTER,
                        label,
                        FontId::monospace(11.),
                        Color32::WHITE,
                    );
                } else if self.editor.tool == Tool::Polyline
                    && let Some(a) = self.editor.polyline_points.last().copied()
                {
                    if !self.tracker_locked {
                        self.tracker_a = a.distance(q);
                        self.tracker_b = (q.y - a.y).atan2(q.x - a.x).to_degrees();
                    }
                    let label = format!(
                        "{}  /  {:.1}°",
                        self.editor
                            .project
                            .plan_options
                            .dimension_style
                            .format_length(a.distance(q)),
                        (q.y - a.y).atan2(q.x - a.x).to_degrees()
                    );
                    let label_rect = Rect::from_min_size(s + vec2(18., -42.), vec2(177., 27.));
                    p.rect_filled(label_rect, 4., INK);
                    p.text(
                        label_rect.center(),
                        Align2::CENTER_CENTER,
                        label,
                        FontId::monospace(11.),
                        Color32::WHITE,
                    );
                }
            }
            let wall_double = self.editor.tool == Tool::Wall
                && (response.double_clicked_by(PointerButton::Primary)
                    || response.triple_clicked_by(PointerButton::Primary))
                && matches!(
                    self.editor.wall_settings.geometry.geometry_method,
                    WallGeometryMethod::Continuous | WallGeometryMethod::Polygonal
                );
            if response.clicked()
                && !wall_double
                && !over_wall_edit_palette
                && !ui.input(|i| i.key_down(Key::Space))
            {
                // AC29 parameter transfer: hold Option to pick up, Option+Cmd
                // to inject; the one-shot armed modes consume one click.
                let alt_click = ui.input(|i| i.modifiers.alt);
                if alt_click || self.pick_armed || self.inject_armed {
                    let inject = if alt_click {
                        ui.input(|i| i.modifiers.command)
                    } else {
                        self.inject_armed
                    };
                    let hit = geometry::hit_plan(
                        &self.editor.project,
                        self.editor.story,
                        raw,
                        6. / self.plan_scale,
                    );
                    self.pick_armed = false;
                    self.inject_armed = false;
                    self.execute_transfer(inject, hit);
                } else if self.orientation_pick_active {
                    if let Some(start) = self.orientation_pick_start.take() {
                        let dx = q.x - start.x;
                        let dy = q.y - start.y;
                        if dx.hypot(dy) > 1e-4 {
                            self.orientation_pick_active = false;
                            self.set_orientation_degrees(dy.atan2(dx).to_degrees());
                        } else {
                            self.orientation_pick_start = Some(start);
                            self.editor.status = "Orientation needs two distinct points".into();
                        }
                    } else {
                        self.orientation_pick_start = Some(q);
                        self.editor.status = "Set Orientation · click the second point".into();
                    }
                } else if self.rotated_grid_tool.is_some() {
                    self.handle_rotated_grid_click(raw, q, 9. / self.plan_scale);
                } else if self.editor.tool == Tool::Marquee {
                    match self.marquee_geometry {
                        MarqueeGeometry::Rectangle => {
                            if let Some(a) = self.marquee_points.first().copied() {
                                self.place_marquee(geometry::rectangle_polygon(a, q));
                            } else {
                                self.marquee_points.push(q);
                            }
                        }
                        MarqueeGeometry::RotatedRectangle => {
                            self.marquee_points.push(q);
                            if self.marquee_points.len() == 3 {
                                if let Some(poly) = geometry::rotated_rectangle_polygon(
                                    self.marquee_points[0],
                                    self.marquee_points[1],
                                    self.marquee_points[2],
                                ) {
                                    self.place_marquee(poly);
                                } else {
                                    self.marquee_points.clear();
                                    self.editor.status =
                                        "Rotated rectangle needs non-zero sides".into();
                                }
                            }
                        }
                        MarqueeGeometry::Polygon => self.marquee_points.push(q),
                    }
                } else if self.editor.tool == Tool::Select {
                    let wall_context = self.selected_wall_baseline_hit(raw, 9. / self.plan_scale);
                    if let Some((origin, ids)) =
                        self.selected_wall_junction_plan(raw, 9. / self.plan_scale)
                    {
                        self.drag = None;

                        self.start_wall_junction(origin, ids, pos, None, false);
                        ui.ctx().request_repaint();
                    } else if let Some((id, node)) = wall_context {
                        self.drag = None;

                        self.open_wall_edit(id, node, pos);
                        self.set_wall_edit_hit(raw, None);
                        if node.is_none() {
                            self.start_wall_insert();
                            ui.ctx().request_repaint();
                        }
                    } else {
                        self.wall_edit = None;
                        let context = self.selected_polyline_hit(raw, 9. / self.plan_scale);
                        if let Some((id, node, edge)) = context {
                            self.drag = None;
                            self.start_polyline_pet(id, node, edge, pos, raw, false);
                            ui.ctx().request_repaint();
                        } else if ui.input(|i| i.modifiers.shift) {
                            let hit = geometry::hit_plan(
                                &self.editor.project,
                                self.editor.story,
                                raw,
                                6. / self.plan_scale,
                            );
                            let mut ids = self.editor.selection_ids();
                            if let Some(id) = hit {
                                if ids.contains(&id) {
                                    ids.retain(|v| *v != id);
                                } else {
                                    ids.push(id);
                                }
                            }
                            self.editor.set_selection(ids);
                        } else {
                            let hit = geometry::hit_plan(
                                &self.editor.project,
                                self.editor.story,
                                raw,
                                6. / self.plan_scale,
                            );
                            self.editor.set_selection(hit.into_iter().collect());
                        }
                    }
                    self.tracker_locked = false;
                    self.draft_id = None;
                } else if self.editor.tool == Tool::Polyline {
                    let close_start = self.editor.polyline_points.len() >= 2
                        && self
                            .editor
                            .polyline_points
                            .first()
                            .is_some_and(|start| self.screen(r, *start).distance(pos) <= 9.);
                    if close_start {
                        self.close_polyline();
                    } else {
                        self.commit_polyline_point(q, 0.01);
                    }
                } else if self.editor.tool == Tool::Wall {
                    let defaults_ok = !self
                        .info_box_wall_tool
                        .as_ref()
                        .is_some_and(|state| state.dirty)
                        || self.commit_info_box_wall_tool();
                    if defaults_ok {
                        // Use the screen-space hit as a fallback for the
                        // magnetic close target: a click within the visible
                        // start marker must end the chain even when grid snap
                        // or a slightly different pointer position moved `q`.
                        let close_start = self
                            .editor
                            .wall_close_target()
                            .filter(|start| self.screen(r, *start).distance(pos) <= 12.);
                        if let Some(start) = close_start {
                            self.editor.click(start, 2. / self.plan_scale);
                        } else {
                            self.editor.click(q, 6. / self.plan_scale);
                        }
                    }
                    self.tracker_locked = false;
                    self.draft_id = None;
                } else if matches!(self.editor.tool, Tool::Door | Tool::Window) {
                    let defaults_ok = !self
                        .info_box_opening_tool
                        .as_ref()
                        .is_some_and(|state| state.dirty)
                        || self.commit_info_box_opening_tool();
                    if defaults_ok {
                        self.editor.click(q, 6. / self.plan_scale);
                    }
                    self.tracker_locked = false;
                    self.draft_id = None;
                } else {
                    self.editor.click(q, 6. / self.plan_scale);
                    self.tracker_locked = false;
                    self.draft_id = None;
                }
            }
            if wall_double && !ui.input(|i| i.key_down(Key::Space)) {
                let defaults_ok = !self
                    .info_box_wall_tool
                    .as_ref()
                    .is_some_and(|state| state.dirty)
                    || self.commit_info_box_wall_tool();
                if defaults_ok {
                    self.finish_wall_draft();
                }
                self.tracker_locked = false;
                self.draft_id = None;
            }
        }
        if self.editor.tool == Tool::Polyline && response.clicked_by(PointerButton::Secondary) {
            self.finish_polyline();
        }
        if self.editor.tool == Tool::Wall && response.clicked_by(PointerButton::Secondary) {
            self.finish_wall_draft();
        }
        if self.editor.tool == Tool::Marquee
            && self.marquee_geometry == MarqueeGeometry::Polygon
            && response.clicked_by(PointerButton::Secondary)
        {
            if self.marquee_points.len() >= 3 {
                self.place_marquee(self.marquee_points.clone());
            } else {
                self.marquee_points.clear();
                self.editor.status = "Marquee polygon needs at least three points".into();
            }
        }
        if self.editor.tool == Tool::Marquee && self.marquee_geometry == MarqueeGeometry::Rectangle
        {
            if response.drag_started_by(PointerButton::Primary)
                && let Some(pos) = ui.input(|i| i.pointer.press_origin())
            {
                let a = self.world(r, pos);
                self.marquee_preview = Some((a, a));
                self.marquee_points.clear();
            }
            if response.dragged_by(PointerButton::Primary)
                && let (Some((a, _)), Some(pos)) = (self.marquee_preview, pointer)
            {
                self.marquee_preview = Some((a, self.world(r, pos)));
            }
            if response.drag_stopped()
                && let Some((a, b)) = self.marquee_preview.take()
                && a.distance(b) > 2. / self.plan_scale
            {
                self.place_marquee(geometry::rectangle_polygon(a, b));
            }
        }
        if self.editor.tool == Tool::Select
            && !pan
            && !over_wall_edit_palette
            && response.drag_started_by(PointerButton::Primary)
            && let Some(pos) = ui.input(|i| i.pointer.press_origin())
        {
            let w = self.world(r, pos);
            if let Some((id, node)) = self.selected_wall_baseline_hit(w, 10. / self.plan_scale) {
                self.drag = None;

                self.open_wall_edit(id, node, pos);
                if let Some(edit) = self.wall_edit.as_mut() {
                    edit.drag = Some((self.editor.project.clone(), w));
                }
            }
        }
        if self.editor.tool == Tool::Select
            && !pan
            && self
                .wall_edit
                .as_ref()
                .is_some_and(|edit| edit.drag.is_some())
        {
            if response.dragged_by(PointerButton::Primary)
                && let (Some(pos), Some(edit)) = (pointer, self.wall_edit.clone())
                && let Some((before, start)) = edit.drag.as_ref()
            {
                let (current, snap) = self.wall_endpoint_drag_target(
                    before,
                    &edit,
                    self.world(r, pos),
                    ui.input(|i| i.modifiers.shift),
                );
                if self.apply_wall_drag_preview(before, &edit, *start, current)
                    && let Some(active) = self.wall_edit.as_mut()
                {
                    active.snap = snap;
                }
                ui.ctx().request_repaint();
            }
            if response.drag_stopped()
                && let Some(edit) = self.wall_edit.clone()
                && let Some((before, start)) = edit.drag.as_ref()
            {
                // Resolve the release position once more so a release inside
                // the snap radius commits the exact reference-line point even
                // when no pointer-move frame was delivered after entering it.
                if let Some(pos) = pointer.or_else(|| ui.input(|i| i.pointer.latest_pos())) {
                    let (current, snap) = self.wall_endpoint_drag_target(
                        before,
                        &edit,
                        self.world(r, pos),
                        ui.input(|i| i.modifiers.shift),
                    );
                    if self.apply_wall_drag_preview(before, &edit, *start, current)
                        && let Some(active) = self.wall_edit.as_mut()
                    {
                        active.snap = snap;
                    }
                }
                if let Some(active) = self.wall_edit.as_mut() {
                    active.drag = None;
                }
                if self.editor.project != *before {
                    let joined = self
                        .wall_edit
                        .as_ref()
                        .is_some_and(|edit| edit.snap.is_some());
                    self.editor.commit(before.clone());
                    self.editor.status = match edit.command {
                        WallEditCommand::MoveNode if joined => self
                            .language
                            .translate("Wall endpoint joined at reference line")
                            .into(),
                        WallEditCommand::MoveNode => "Wall reference-line endpoint moved".into(),
                        WallEditCommand::Move => "Wall moved".into(),
                    };
                    self.wall_edit = None;
                }
            }
        }
        if hovered && !transfer_cursor_active {
            let drawing_tool = matches!(
                self.editor.tool,
                Tool::Wall | Tool::Slab | Tool::Stair | Tool::Polyline | Tool::Door | Tool::Window
            );
            let interaction_drag = pan
                || self
                    .wall_edit
                    .as_ref()
                    .is_some_and(|edit| edit.drag.is_some())
                || self.drag.is_some();
            if drawing_tool && !interaction_drag && self.rotated_grid_tool.is_none() {
                // AC29 drawing cursor: a square crosshair riding the snap
                // point — endpoint, midpoint or the raw pointer when free.
                let pos = pointer.unwrap_or(r.center());
                let (target, _) = self.target(self.world(r, pos), ui.input(|i| i.modifiers.shift));
                crate::transfer_cursor::paint_crosshair(&p, self.screen(r, target));
                ui.ctx().set_cursor_icon(CursorIcon::None);
            } else {
                let feedback = if self.editor.tool == Tool::Polyline {
                    if ui.input(|i| i.pointer.button_down(PointerButton::Primary)) {
                        CursorFeedback::SolidPencil
                    } else if self.editor.polyline_points.is_empty() {
                        CursorFeedback::Drawing
                    } else {
                        let (_, kind) = self.target(
                            self.world(r, pointer.unwrap_or(r.center())),
                            ui.input(|i| i.modifiers.shift),
                        );
                        if matches!(
                            kind,
                            SnapKind::Endpoint
                                | SnapKind::Midpoint
                                | SnapKind::Angle
                                | SnapKind::Perpendicular
                                | SnapKind::Alignment
                        ) {
                            CursorFeedback::HollowPencilWithLine
                        } else {
                            CursorFeedback::HollowPencil
                        }
                    }
                } else if pan {
                    CursorFeedback::Dragging
                } else if self
                    .wall_edit
                    .as_ref()
                    .is_some_and(|edit| edit.drag.is_some())
                {
                    CursorFeedback::Dragging
                } else if self.editor.tool == Tool::Select
                    && self
                        .selected_wall_baseline_hit(
                            self.world(r, pointer.unwrap_or(r.center())),
                            9. / self.plan_scale,
                        )
                        .is_some()
                {
                    match self.selected_wall_baseline_hit(
                        self.world(r, pointer.unwrap_or(r.center())),
                        9. / self.plan_scale,
                    ) {
                        Some((_, Some(_))) => CursorFeedback::ArrowWithCheck,
                        _ => CursorFeedback::ArrowWithTripleRay,
                    }
                } else if self.drag.as_ref().is_some_and(|(_, _, ids)| {
                    ids.iter().any(|id| {
                        self.editor.project.elements.iter().any(|element| {
                            element.id == *id && matches!(element.shape, Shape::Polyline { .. })
                        })
                    })
                }) {
                    CursorFeedback::Arrow
                } else if self.drag.is_some() {
                    CursorFeedback::Dragging
                } else {
                    let pos = pointer.unwrap_or(r.center());
                    match self.editor.tool {
                        Tool::Select => {
                            let raw = self.world(r, pos);
                            self.select_cursor_feedback(raw)
                        }
                        Tool::Marquee => CursorFeedback::Drawing,
                        Tool::Wall
                        | Tool::Slab
                        | Tool::Stair
                        | Tool::Polyline
                        | Tool::Door
                        | Tool::Window => {
                            let (_, snap_kind) =
                                self.target(self.world(r, pos), ui.input(|i| i.modifiers.shift));
                            if matches!(
                                snap_kind,
                                SnapKind::Endpoint
                                    | SnapKind::Midpoint
                                    | SnapKind::SnapPoint
                                    | SnapKind::Guide
                            ) {
                                CursorFeedback::SnapTarget
                            } else {
                                CursorFeedback::Drawing
                            }
                        }
                    }
                };
                ui.ctx()
                    .set_cursor_icon(if self.rotated_grid_tool.is_some() {
                        CursorIcon::Crosshair
                    } else {
                        feedback.icon()
                    });
                feedback.paint(&p, pointer.unwrap_or(r.center()));
            }
        }
        p.text(
            r.left_top() + vec2(18., 18.),
            Align2::LEFT_TOP,
            format!(
                "{:02}  /  {}",
                self.editor.story,
                self.editor
                    .project
                    .stories
                    .iter()
                    .find(|s| s.id == self.editor.story)
                    .map_or("Floor plan", |s| s.name.as_str())
            ),
            FontId::proportional(12.),
            MUTED,
        );
        let b = r.left_bottom() + vec2(20., -35.);
        let len = self.plan_scale * 2.;
        p.line_segment([b, b + vec2(len, 0.)], Stroke::new(2., INK));
        for v in [b, b + vec2(len, 0.)] {
            p.line_segment([v - vec2(0., 4.), v + vec2(0., 4.)], Stroke::new(1., INK));
        }
        p.text(
            b + vec2(len / 2., 8.),
            Align2::CENTER_TOP,
            "2 m",
            FontId::monospace(10.),
            MUTED,
        );
        p.text(
            r.right_top() + vec2(-20., 24.),
            Align2::RIGHT_TOP,
            "N",
            FontId::proportional(12.),
            INK,
        );

        if let Some(edit) = self.wall_edit.as_ref() {
            let wall = self
                .editor
                .project
                .elements
                .iter()
                .find(|element| element.id == edit.element_id);
            if let Some((start, end)) = wall.and_then(geometry::wall_reference_line) {
                if let Some(node) = edit.node {
                    let point = if node == 0 { start } else { end };
                    p.circle_stroke(self.screen(r, point), 7., Stroke::new(2., ACCENT));
                }
                if edit.drag.is_some()
                    && let Some(pos) = pointer
                {
                    let length = start.distance(end);
                    let angle = (end.y - start.y).atan2(end.x - start.x).to_degrees();
                    let boxr = Rect::from_min_size(pos + vec2(18., -42.), vec2(177., 27.));
                    p.rect_filled(boxr, 4., INK);
                    p.text(
                        boxr.center(),
                        Align2::CENTER_CENTER,
                        format!(
                            "{}  /  {:.1}°",
                            self.editor
                                .project
                                .plan_options
                                .dimension_style
                                .format_length(length),
                            angle
                        ),
                        FontId::monospace(11.),
                        Color32::WHITE,
                    );
                }
            }
            if let Some(snap) = edit.snap
                && let Some((start, end)) = self
                    .editor
                    .project
                    .elements
                    .iter()
                    .find(|element| element.id == snap.element_id)
                    .and_then(geometry::wall_reference_line)
            {
                p.line_segment(
                    [self.screen(r, start), self.screen(r, end)],
                    Stroke::new(2.2, ACCENT),
                );
                let point = self.screen(r, snap.point);
                p.circle_stroke(point, 7., Stroke::new(2., ACCENT));
                p.circle_filled(point, 2.5, ACCENT);
                p.text(
                    point + vec2(11., 12.),
                    Align2::LEFT_TOP,
                    self.language.translate("Snap: wall reference line"),
                    FontId::proportional(11.),
                    ACCENT,
                );
            }
        }
        if trace_pointer {
            self.trace_cursor(ui);
        }
        if self.wall_insert_canvas(ui, r, &response, false) {
            return;
        }
        self.wall_edit_palette(ui.ctx());
    }

    pub(super) fn wall_edit_palette(&mut self, ctx: &Context) {
        let Some(edit) = self.wall_edit.as_ref() else {
            self.wall_edit_palette_rect = None;
            return;
        };
        if edit.drag.is_some() {
            self.wall_edit_palette_rect = None;
            return;
        }
        if !edit.error.is_empty() {
            self.editor.status = edit.error.clone();
        }
        let endpoint = edit.node.is_some();
        let (rect, chosen, _) = self.wall_pet_buttons(
            ctx,
            edit.palette_at,
            endpoint.then_some(WallPetCommand::InsertNode),
            endpoint,
        );
        self.wall_edit_palette_rect = Some(rect);
        if let Some(command) = chosen {
            // The first slot is Move node in endpoint context, retaining the
            // existing endpoint drag/snap path rather than inserting at t=0/1.
            if !(endpoint && command == WallPetCommand::InsertNode) {
                self.start_wall_pet(command);
            }
        }
    }
    pub(super) fn tracker(&mut self, ui: &mut Ui) {
        let polyline = self.editor.tool == Tool::Polyline;
        let wall = self.editor.tool == Tool::Wall;
        let wall_method = self.editor.wall_settings.geometry.geometry_method;
        let rectangular = wall && wall_method == WallGeometryMethod::Rectangular;
        let chained = wall
            && matches!(
                wall_method,
                WallGeometryMethod::Continuous | WallGeometryMethod::Polygonal
            );
        let tracker_anchor = self.editor.anchor.or_else(|| {
            polyline
                .then(|| self.editor.polyline_points.last().copied())
                .flatten()
        });
        if let Some(a) = tracker_anchor {
            Frame::new()
                .fill(PANEL_ALT)
                .inner_margin(Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.set_min_height(40.);
                    ui.horizontal_wrapped(|ui| {
                        Self::label(ui, self.language.translate("TRACKER"));
                        let angular =
                            matches!(self.editor.tool, Tool::Wall | Tool::Polyline) && !rectangular;
                        ui.label(self.language.translate(if angular {
                            if wall { "Length" } else { "Distance" }
                        } else {
                            "Δ X"
                        }));
                        if ui
                            .add(
                                DragValue::new(&mut self.tracker_a)
                                    .speed(0.01)
                                    .fixed_decimals(3)
                                    .suffix(" m"),
                            )
                            .changed()
                        {
                            self.tracker_locked = true;
                        }
                        ui.label(
                            self.language
                                .translate(if angular { "Angle" } else { "Δ Y" }),
                        );
                        if ui
                            .add(
                                DragValue::new(&mut self.tracker_b)
                                    .speed(0.1)
                                    .fixed_decimals(2)
                                    .suffix(if angular { " °" } else { " m" }),
                            )
                            .changed()
                        {
                            self.tracker_locked = true;
                        }
                        ui.checkbox(&mut self.tracker_locked, self.language.translate("Lock"));
                        let enter = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                        let action = if polyline || (chained && self.tracker_locked) {
                            "Add point [Enter]"
                        } else if chained {
                            "Finish [Enter]"
                        } else {
                            "Create [Enter]"
                        };
                        if ui.button(self.language.translate(action)).clicked() || enter {
                            let target = self.tracker_target(a);
                            if polyline {
                                self.commit_polyline_point(target, 0.01);
                            } else if chained && !self.tracker_locked {
                                self.finish_wall_draft();
                            } else if chained {
                                self.editor.click(target, 0.);
                                self.tracker_locked = false;
                                self.draft_id = None;
                            } else {
                                if let Err(error) = self.editor.finish_wall(Some(target)) {
                                    self.editor.status = localized_error(self.language, &error);
                                }
                                self.tracker_locked = false;
                                self.draft_id = None;
                            }
                        }
                        if chained
                            && self.tracker_locked
                            && ui.button(self.language.translate("Finish")).clicked()
                        {
                            self.finish_wall_draft();
                        }
                        if !polyline && ui.button(self.language.translate("Cancel")).clicked() {
                            self.editor.cancel_wall_draft();
                            self.tracker_locked = false;
                        }
                    });
                });
        } else if polyline || wall {
            Frame::new()
                .fill(PANEL_ALT)
                .inner_margin(Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.set_min_height(40.);
                    ui.vertical_centered(|ui| {
                        ui.label(self.language.translate("Tracker · click a start node"))
                    });
                });
        }
    }
}
