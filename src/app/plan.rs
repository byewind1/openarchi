use super::drawing_preview::DrawingPreview;
use super::wall_node_ui::WallPetCommand;
use super::*;
use crate::layers::{ARCHICAD_LAYER, ElementLayer};

impl AtelierApp {
    pub(super) fn paint_geometry_error(
        painter: &Painter,
        language: UiLanguage,
        d: &crate::derived_geometry::GeometryDiagnostic,
    ) {
        let color = Color32::from_rgb(177, 60, 40);
        let galley = painter.layout(
            language.geometry_diagnostic_message(d),
            FontId::proportional(13.),
            color,
            (painter.clip_rect().width() - 24.).max(40.),
        );
        let pos = painter.clip_rect().min + vec2(8., 8.);
        painter.rect_filled(
            Rect::from_min_size(pos, galley.size()).expand(4.),
            3.,
            Color32::from_rgb(255, 247, 241),
        );
        painter.galley(pos, galley, color);
    }

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

    pub(super) fn paint_stair_snapshot(
        painter: &Painter,
        g: &crate::stair_geometry::StairSnapshot,
        screen: impl Fn(Point) -> Pos2,
        fill: Color32,
        stroke: Stroke,
        tread_stroke: Option<Stroke>,
    ) {
        let mut mesh = egui::Mesh::default();
        for t in &g.plan_triangles {
            let i = mesh.vertices.len() as u32;
            for &p in t {
                mesh.colored_vertex(screen(p), fill);
            }
            mesh.add_triangle(i, i + 1, i + 2);
        }
        painter.add(egui::Shape::mesh(mesh));
        painter.add(egui::Shape::closed_line(
            g.source.footprint.iter().copied().map(&screen).collect(),
            stroke,
        ));
        if let Some(stroke) = tread_stroke {
            for [a, b] in &g.source.tread_lines {
                painter.line_segment([screen(*a), screen(*b)], stroke);
            }
        }
    }

    /// Gross plan representation from the same cells as the material solid.
    /// Hosted opening symbols erase their own shared void triangles afterwards.
    pub(super) fn paint_wall_snapshot(
        painter: &Painter,
        geometry: &crate::wall_solid::WallSnapshot,
        screen: impl Fn(Point) -> Pos2,
        fill: Color32,
        stroke: Stroke,
    ) {
        let mut mesh = egui::Mesh::default();
        for triangle in &geometry.plan_triangles {
            let i = mesh.vertices.len() as u32;
            for &point in triangle {
                mesh.colored_vertex(screen(point), fill);
            }
            mesh.add_triangle(i, i + 1, i + 2);
        }
        painter.add(egui::Shape::mesh(mesh));
        painter.add(egui::Shape::closed_line(
            geometry.plan_path.iter().copied().map(screen).collect(),
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
        language: UiLanguage,
    ) {
        let geometry = match crate::slab_geometry::derive(element, 0.) {
            Ok(g) => g,
            Err(d) => {
                Self::paint_geometry_error(painter, language, &d);
                return;
            }
        };
        Self::paint_slab_snapshot(painter, &geometry, screen, fill, stroke);
    }
    pub(super) fn paint_slab_snapshot(
        painter: &Painter,
        geometry: &crate::slab_geometry::DerivedSlab,
        screen: impl Fn(Point) -> Pos2,
        fill: Color32,
        stroke: Stroke,
    ) {
        let mut mesh = egui::Mesh::default();
        for triangle in &geometry.plan_triangles {
            let index = mesh.vertices.len() as u32;
            for point in triangle {
                mesh.colored_vertex(screen(*point), fill);
            }
            mesh.add_triangle(index, index + 1, index + 2);
        }
        painter.add(egui::Shape::mesh(mesh));
        for ring in &geometry.plan_paths {
            painter.add(egui::Shape::closed_line(
                ring.iter().copied().map(&screen).collect(),
                stroke,
            ));
        }
    }

    /// Share actual roof triangles and every ring between the active plan and Trace.
    pub(super) fn paint_roof_polygon(
        painter: &egui::Painter,
        spec: &crate::roof::RoofSpec,
        screen: impl Fn(Point) -> Pos2,
        fill: Color32,
        stroke: Stroke,
        language: UiLanguage,
    ) {
        let geometry = match crate::roof_geometry::geometry(spec, 0.) {
            Ok(g) => g,
            Err(m) => {
                let d = crate::derived_geometry::GeometryDiagnostic::error(
                    0,
                    "INVALID_ROOF",
                    "plan",
                    m,
                );
                Self::paint_geometry_error(painter, language, &d);
                return;
            }
        };
        Self::paint_roof_snapshot(painter, &geometry, screen, fill, stroke);
    }
    pub(super) fn paint_roof_snapshot(
        painter: &Painter,
        geometry: &crate::roof_geometry::RoofGeometry,
        screen: impl Fn(Point) -> Pos2,
        fill: Color32,
        stroke: Stroke,
    ) {
        let mut mesh = egui::Mesh::default();
        for triangle in &geometry.plan_triangles {
            let base = mesh.vertices.len() as u32;
            for point in triangle {
                mesh.colored_vertex(screen(*point), fill);
            }
            mesh.add_triangle(base, base + 1, base + 2);
        }
        painter.add(egui::Shape::mesh(mesh));
        for ring in &geometry.plan_paths {
            for i in 0..ring.len() {
                painter.line_segment(
                    [screen(ring[i]), screen(ring[(i + 1) % ring.len()])],
                    stroke,
                );
            }
        }
    }

    /// Beam plan/Trace/preview use the same cap triangulation as its solid.
    pub(super) fn paint_beam_polygon(
        painter: &egui::Painter,
        project: &Project,
        element: &Element,
        screen: impl Fn(Point) -> Pos2,
        fill: Color32,
        stroke: Stroke,
        language: UiLanguage,
    ) {
        let g = match crate::beam_geometry::derive(project, element) {
            Ok(g) => g,
            Err(d) => {
                Self::paint_geometry_error(painter, language, &d);
                return;
            }
        };
        Self::paint_beam_snapshot(painter, &g.geometry, screen, fill, stroke);
    }
    pub(super) fn paint_beam_snapshot(
        painter: &Painter,
        geometry: &crate::beam_geometry::BeamSnapshot,
        screen: impl Fn(Point) -> Pos2,
        fill: Color32,
        stroke: Stroke,
    ) {
        let mut mesh = egui::Mesh::default();
        for t in &geometry.plan_triangles {
            let i = mesh.vertices.len() as u32;
            for p in t {
                mesh.colored_vertex(screen(*p), fill);
            }
            mesh.add_triangle(i, i + 1, i + 2);
        }
        painter.add(egui::Shape::mesh(mesh));
        painter.add(egui::Shape::closed_line(
            geometry.plan_path.iter().copied().map(screen).collect(),
            stroke,
        ));
    }

    pub(super) fn world(&self, r: Rect, p: Pos2) -> Point {
        let radians = self
            .host
            .document
            .editor
            .project
            .plan_options
            .normalized_orientation()
            .to_radians();
        let (sin, cos) = radians.sin_cos();
        let view_x = f64::from(p.x - r.center().x) / self.plan_scale;
        let view_y = -f64::from(p.y - r.center().y) / self.plan_scale;
        Point::new(
            self.plan_center.x + view_x * cos + view_y * sin,
            self.plan_center.y - view_x * sin + view_y * cos,
        )
    }
    pub(super) fn world_precise(&self, r: Rect, p: Pos2) -> crate::creation_input::CreationPoint {
        let angle = f64::from(
            self.host
                .document
                .editor
                .project
                .plan_options
                .normalized_orientation(),
        )
        .to_radians();
        let (sin, cos) = angle.sin_cos();
        let x = (f64::from(p.x) - f64::from(r.center().x)) / f64::from(self.plan_scale);
        let y = -(f64::from(p.y) - f64::from(r.center().y)) / f64::from(self.plan_scale);
        crate::creation_input::CreationPoint {
            x: f64::from(self.plan_center.x) + x * cos + y * sin,
            y: f64::from(self.plan_center.y) - x * sin + y * cos,
        }
    }

    pub(super) fn screen(&self, r: Rect, p: Point) -> Pos2 {
        let radians = self
            .host
            .document
            .editor
            .project
            .plan_options
            .normalized_orientation()
            .to_radians();
        let (sin, cos) = radians.sin_cos();
        let x = p.x - self.plan_center.x;
        let y = p.y - self.plan_center.y;
        pos2(
            f64::from(r.center().x) + (x * cos - y * sin) * self.plan_scale,
            f64::from(r.center().y) - (x * sin + y * cos) * self.plan_scale,
        )
    }
    pub(super) fn plan_pen(&self) -> crate::view_settings::PenPalette {
        self.host
            .document
            .editor
            .project
            .plan_options
            .pen_set
            .palette()
    }
    pub(super) fn plan_style(
        &self,
        element: &Element,
    ) -> Option<crate::view_settings::PlanElementStyle> {
        self.host.document.editor.project.plan_options.style_for(
            element,
            self.plan_pen(),
            &self.host.document.editor.project.layers,
        )
    }
    pub(super) fn plan_pen_px(&self, width_mm: f64) -> f32 {
        let world_width =
            width_mm / 1000. * self.host.document.editor.project.plan_options.drawing_scale as f64;
        (world_width * self.plan_scale).clamp(0.45, 10.) as f32
    }
    pub(super) fn plan_color(rgb: [u8; 3]) -> Color32 {
        Color32::from_rgb(rgb[0], rgb[1], rgb[2])
    }
    pub(super) fn selected_polyline_hit(
        &self,
        point: Point,
        tolerance: f64,
    ) -> Option<(u64, Option<usize>, Option<usize>)> {
        let selected = self.host.document.editor.selection_ids();
        let mut eligible: Vec<(
            u64,
            Vec<Point>,
            Vec<Option<crate::model::ArcSegment>>,
            bool,
            usize,
        )> = Vec::new();
        for e in self
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .rev()
            .filter(|e| {
                selected.contains(&e.id)
                    && e.story == self.host.document.editor.story
                    && self
                        .host
                        .document
                        .editor
                        .project
                        .plan_options
                        .element_visible(e, &self.host.document.editor.project.layers)
            })
        {
            match &e.shape {
                Shape::Polyline { points, arcs } => {
                    eligible.push((e.id, points.clone(), arcs.clone(), false, 0))
                }
                Shape::Slab { .. } | Shape::ConceptMass { .. } => {
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
                        .fold(f64::INFINITY, f64::min);
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
        tolerance: f64,
    ) -> Option<(u64, Option<usize>)> {
        let selected = self.host.document.editor.selection_ids();
        let walls = self
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .rev()
            .filter(|element| {
                selected.contains(&element.id)
                    && element.story == self.host.document.editor.story
                    && matches!(element.shape, Shape::Wall { .. })
                    && self
                        .host
                        .document
                        .editor
                        .project
                        .plan_options
                        .element_visible(element, &self.host.document.editor.project.layers)
            });
        let mut endpoint: Option<(u64, usize, f64)> = None;
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
        let mut segment: Option<(u64, f64)> = None;
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
    fn plan_frame_batch_current(&self, batch: &crate::derivation_cache::GeometryBatch) -> bool {
        batch.revision == self.host.document.editor.revision
            && batch.document_session == self.host.document.editor.document_session()
    }
    fn hit_plan_in_frame(
        &self,
        point: Point,
        tolerance: f64,
        batch: &crate::derivation_cache::GeometryBatch,
    ) -> Option<u64> {
        let e = &self.host.document.editor;
        if self.plan_frame_batch_current(batch) {
            geometry::hit_plan_with_batch(&e.project, e.story, point, tolerance, batch)
        } else {
            geometry::hit_plan(&e.project, e.story, point, tolerance)
        }
    }
    pub(super) fn select_cursor_feedback_in_frame(
        &self,
        point: Point,
        batch: &crate::derivation_cache::GeometryBatch,
    ) -> CursorFeedback {
        if !self.plan_frame_batch_current(batch) {
            return self.select_cursor_feedback(point);
        }
        let e = &self.host.document.editor;
        match geometry::plan_hover_kind_with_batch(
            &e.project,
            e.story,
            point,
            8. / self.plan_scale,
            6. / self.plan_scale,
            batch,
        ) {
            Some(geometry::PlanHoverKind::Body) => CursorFeedback::Arrow,
            Some(geometry::PlanHoverKind::Node) => CursorFeedback::ArrowWithCheck,
            Some(geometry::PlanHoverKind::Edge) => CursorFeedback::ArrowWithTripleRay,
            None => CursorFeedback::Empty,
        }
    }
    pub(super) fn select_cursor_feedback(&self, point: Point) -> CursorFeedback {
        match geometry::plan_hover_kind(
            &self.host.document.editor.project,
            self.host.document.editor.story,
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
            model_canvas: None,
            node,
            command: if node.is_some() {
                WallEditCommand::MoveNode
            } else {
                WallEditCommand::Move
            },
            palette_at: at + vec2(14., 12.),
            insert_anchor: self
                .host
                .document
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
        self.host.document.editor.status = if node.is_some() {
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
            self.host.document.editor.story,
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
    pub(super) fn finish_wall_reference_drag(
        &mut self,
        before: Project,
        command: WallEditCommand,
        joined: bool,
    ) {
        if self.host.document.editor.commit_checked(before) {
            self.host.document.editor.status = match command {
                WallEditCommand::MoveNode if joined => self
                    .language
                    .translate("Wall endpoint joined at reference line")
                    .into(),
                WallEditCommand::MoveNode => "Wall reference-line endpoint moved".into(),
                WallEditCommand::Move => "Wall moved".into(),
            };
        }
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
        self.host.document.editor.project = before.clone();
        if let Err(message) = self.host.document.editor.set_wall_reference_line(
            edit.element_id,
            candidate_start,
            candidate_end,
        ) {
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
    pub(super) fn tracker_lock_applies(&self) -> bool {
        self.tracker_locked
            && self.host.document.editor.slab_polygon_draft().is_none()
            && self.host.document.editor.roof_polygon_draft().is_none()
    }

    pub(super) fn base_target(&self, p: Point, shift: bool) -> (Point, SnapKind) {
        let result = self.base_target_without_trace(p, shift);
        if !self.host.document.editor.snap
            || self.tracker_locked
            || shift
            || self.host.document.editor.orthogonal
            || self.trace.pointer_mode_active()
        {
            return result;
        }
        let tolerance = if matches!(
            result.1,
            SnapKind::Endpoint | SnapKind::Midpoint | SnapKind::SnapPoint
        ) {
            p.distance(result.0)
        } else {
            9. / self.plan_scale
        };
        let result = self
            .trace_cad_snap(p, tolerance)
            .map_or(result, |q| (q, SnapKind::Endpoint));
        let tolerance = if matches!(
            result.1,
            SnapKind::Endpoint | SnapKind::Midpoint | SnapKind::SnapPoint
        ) {
            p.distance(result.0)
        } else {
            tolerance
        };
        self.trace_model_snap(p, tolerance).unwrap_or(result)
    }
    fn base_target_without_trace(&self, p: Point, shift: bool) -> (Point, SnapKind) {
        if self.polygon_creation_active()
            && self.tracker_locked
            && let Ok(target) = self.polygon_creation_target(p)
        {
            return (target.model_point(), SnapKind::Free);
        }

        if self.host.document.editor.tool == Tool::Roof
            && self.host.document.editor.roof_polygon_draft.is_none()
            && self.tracker_locked
            && let Some(draft) = self.host.document.editor.roof_draft
            && let Ok(point) = draft.numeric_target(self.tracker_a, self.tracker_b)
        {
            return (point, SnapKind::Free);
        }
        if self.host.document.editor.tool == Tool::Slab
            && self.tracker_locked
            && let Some(draft) = self.host.document.editor.slab_draft
            && let Ok(point) = draft.numeric_target(self.tracker_a, self.tracker_b)
        {
            return (point, SnapKind::Free);
        }
        // The chain's first node is not part of the committed model yet, so
        // the ordinary model snap cannot find it. Make it a magnetic target
        // while the draft is open: this also keeps grid snap from moving a
        // clicking-the-start gesture onto a different grid point.
        if !self.tracker_locked
            && let Some(start) = self.host.document.editor.wall_close_target()
            && p.distance(start) <= 12. / self.plan_scale
        {
            return (start, SnapKind::Endpoint);
        }
        if self.host.document.editor.tool == Tool::Polyline
            && let Some(anchor) = self.host.document.editor.polyline_points.last().copied()
        {
            if self.tracker_locked {
                return (self.tracker_target(anchor), SnapKind::Free);
            }
            if !self
                .host
                .document
                .editor
                .project
                .guide_settings
                .snap_enabled
            {
                let (base, kind) = geometry::snap(
                    &self.host.document.editor.project,
                    self.host.document.editor.story,
                    p,
                    9. / self.plan_scale,
                    self.host.document.editor.snap,
                );
                return self.apply_grid_snap(p, base, kind);
            }
            return geometry::polyline_snap_target(
                &self.host.document.editor.project,
                self.host.document.editor.story,
                &self.host.document.editor.polyline_points,
                p,
                9. / self.plan_scale,
                self.host.document.editor.snap,
            );
        }
        let (mut q, mut kind) = geometry::snap(
            &self.host.document.editor.project,
            self.host.document.editor.story,
            p,
            9. / self.plan_scale,
            self.host.document.editor.snap,
        );
        // Draft nodes have no model element yet. Include them in the same
        // target used by the preview and pointer acceptance, without closing
        // or committing the polygon here.
        if self.host.document.editor.snap {
            let points = self
                .host
                .document
                .editor
                .slab_polygon_draft()
                .map(|d| d.points())
                .or_else(|| {
                    self.host
                        .document
                        .editor
                        .roof_polygon_draft()
                        .map(|d| d.points())
                });
            let mut best = if matches!(
                kind,
                SnapKind::Endpoint | SnapKind::SnapPoint | SnapKind::Midpoint
            ) {
                p.distance(q)
            } else {
                9. / self.plan_scale
            };
            for &node in points.unwrap_or(&[]) {
                let distance = p.distance(node);
                if distance < best {
                    q = node;
                    kind = SnapKind::Endpoint;
                    best = distance;
                }
            }
        }
        if let Some(a) = self.host.document.editor.anchor {
            if self.tracker_lock_applies() && self.host.document.editor.beam_arc_draft.is_none() {
                return (self.tracker_target(a), SnapKind::Free);
            }
            if self.host.document.editor.tool == Tool::Wall
                && self
                    .host
                    .document
                    .editor
                    .wall_settings
                    .geometry
                    .geometry_method
                    != WallGeometryMethod::Rectangular
                && (self.host.document.editor.orthogonal || shift)
                && !matches!(
                    kind,
                    SnapKind::Endpoint | SnapKind::Midpoint | SnapKind::SnapPoint
                )
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
        if matches!(self.host.document.editor.tool, Tool::Wall | Tool::Polyline) {
            if self.host.document.editor.tool == Tool::Wall
                && self
                    .host
                    .document
                    .editor
                    .wall_settings
                    .geometry
                    .geometry_method
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
        close_tolerance: f64,
    ) -> Vec<(Point, Point)> {
        let method = self
            .host
            .document
            .editor
            .wall_settings
            .geometry
            .geometry_method;
        match method {
            WallGeometryMethod::Single => self
                .host
                .document
                .editor
                .anchor
                .map(|start| {
                    let mut segments: Vec<_> = self
                        .host
                        .document
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
                .host
                .document
                .editor
                .anchor
                .map(|start| {
                    let mut segments: Vec<_> = self
                        .host
                        .document
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
                let points = &self.host.document.editor.wall_points;
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
    pub(super) fn paint_wall_batch_preview(
        &self,
        painter: &Painter,
        viewport: Rect,
        segments: Vec<(Point, Point)>,
    ) {
        let Ok(elements) = self.host.document.editor.wall_creation_candidates(segments) else {
            return;
        };
        let ids: Vec<_> = elements.iter().map(|e| e.id).collect();
        let Ok(candidate) = self.host.document.editor.preview_add_elements(
            elements,
            self.host.document.editor.project.edit_state.autogroup,
        ) else {
            return;
        };
        let mut query = crate::wall_solid::WallQuery::new(&candidate);
        for id in ids {
            let Ok(g) = query.wall(id) else {
                continue;
            };
            Self::paint_wall_snapshot(
                painter,
                &g.geometry,
                |p| self.screen(viewport, p),
                Color32::from_rgba_unmultiplied(39, 146, 137, 50),
                Stroke::new(1.5, ACCENT),
            );
            if let Some(e) = candidate.elements.iter().find(|e| e.id == id) {
                painter.add(egui::Shape::line(
                    crate::wall_geometry::reference_path(e)
                        .into_iter()
                        .map(|p| self.screen(viewport, p))
                        .collect(),
                    Stroke::new(1., ACCENT),
                ));
            }
        }
    }
    pub(super) fn commit_polyline_point(&mut self, point: Point, close_tolerance: f64) {
        let close_to_start = self.host.document.editor.polyline_points.len() >= 2
            && self
                .host
                .document
                .editor
                .polyline_points
                .first()
                .is_some_and(|start| start.distance(point) <= close_tolerance);
        if close_to_start {
            self.close_polyline();
        } else {
            self.host.document.editor.click(point, 6. / self.plan_scale);
        }
        self.tracker_locked = false;
        self.draft_id = None;
    }
    pub(super) fn close_polyline(&mut self) {
        if let Some(start) = self.host.document.editor.polyline_points.first().copied() {
            match self.host.document.editor.finish_polyline(Some(start)) {
                Ok(Some(_)) => self.preview_tool = None,
                Ok(None) => {}
                Err(error) => self.host.document.editor.status = error,
            }
        }
        self.tracker_locked = false;
        self.draft_id = None;
    }
    pub(super) fn finish_polyline(&mut self) {
        match self.host.document.editor.finish_polyline(None) {
            Ok(Some(_)) => self.preview_tool = None,
            Ok(None) => {}
            Err(error) => self.host.document.editor.status = error,
        }
        self.tracker_locked = false;
        self.draft_id = None;
    }
    pub(super) fn finish_wall_draft(&mut self) {
        // The click at the polygon start can already close and commit the
        // draft. A following double-click must not report a second, empty
        // completion or start a replacement wall.
        if !self.host.document.editor.wall_draft_active() {
            self.tracker_locked = false;
            self.draft_id = None;
            return;
        }
        let method = self
            .host
            .document
            .editor
            .wall_settings
            .geometry
            .geometry_method;
        let enough_input = match method {
            crate::wall::WallGeometryMethod::Single
            | crate::wall::WallGeometryMethod::Rectangular => false,
            crate::wall::WallGeometryMethod::Continuous => {
                self.host.document.editor.wall_points.len() >= 2
            }
            crate::wall::WallGeometryMethod::Polygonal => {
                self.host.document.editor.wall_points.len() >= 3
            }
        };
        if !enough_input {
            self.host.document.editor.cancel_wall_draft();
            self.host.document.editor.status = self.language.translate("Wall cancelled").into();
            self.preview_tool = None;
            self.tracker_locked = false;
            self.draft_id = None;
            return;
        }
        match self.host.document.editor.finish_wall(None) {
            Ok(_) => self.preview_tool = None,
            Err(error) => self.host.document.editor.status = localized_error(self.language, &error),
        }
        self.tracker_locked = false;
        self.draft_id = None;
    }
    pub(super) fn plan(&mut self, ui: &mut Ui) {
        let r = ui.available_rect_before_wrap();
        self.fit_parametric_canvas_if_pending(r);
        if let Some((id, wait_for_toolbar)) = self.drawings_ui.fit_pending.take()
            && self
                .host
                .document
                .editor
                .project
                .plan_options
                .active_worksheet
                == Some(id)
        {
            if wait_for_toolbar {
                self.drawings_ui.fit_pending = Some((id, false));
                ui.ctx().request_repaint();
            } else {
                self.fit_worksheet_in_viewport(id, r.size());
            }
        }
        let trace_pointer = self.trace.pointer_mode_active();
        self.trace_splitter_input(ui, r);
        let trace_split = self.trace_split_regions(r);
        let hovered_before = ui.input(|i| i.pointer.hover_pos());
        let over_reference = trace_split
            .is_some_and(|(left, _)| hovered_before.is_some_and(|pos| left.contains(pos)));
        let navigation_mode =
            self.navigation.mode != viewport_navigation::Mode::Edit || self.navigation.exit_frame;
        let over_splitter = self.trace.enabled
            && self.trace.splitter.is_some()
            && (self.trace.split_axis.is_some()
                || hovered_before.is_some_and(|p| {
                    Self::trace_splitter_handles(r)
                        .iter()
                        .any(|(handle, _, _)| handle.contains(p))
                }));
        let interactive = !trace_pointer && !over_reference && !over_splitter && !navigation_mode;
        let response = ui.allocate_rect(
            r,
            if interactive || navigation_mode {
                Sense::click_and_drag()
            } else {
                Sense::hover()
            },
        );
        response.widget_info(|| {
            WidgetInfo::labeled(
                WidgetType::Other,
                true,
                self.language.translate("Floor plan canvas"),
            )
        });
        let navigating = self.navigation_canvas(ui, r, &response, viewport_navigation::Pane::Plan);
        // Navigation owns this response; normal editing sees a hover-only canvas.
        let response = if navigating {
            ui.interact(r, Id::new("plan_navigation_paint"), Sense::hover())
        } else {
            response
        };
        // Resolve the viewport and Trace gesture before emitting any world content.
        // Every layer must use the same current-frame transform, irrespective of stacking.
        let hovered = response.hovered() && !navigating;
        let pointer = ui.input(|i| i.pointer.hover_pos());
        if self.host.document.editor.tool != Tool::Select {
            self.wall_edit = None;
            self.wall_edit_palette_rect = None;
        }
        let interact_position =
            ui.input(|i| i.pointer.press_origin().or_else(|| i.pointer.latest_pos()));
        let over_wall_edit_palette = self
            .wall_edit_palette_rect
            .is_some_and(|rect| interact_position.is_some_and(|position| rect.contains(position)));
        if trace_pointer && !navigating {
            self.trace_pointer_in_plan(ui, r);
        }
        let parameter_dragging = self
            .research_ui
            .canvas_move
            .as_ref()
            .is_some_and(|m| m.start.is_some());
        if hovered && !parameter_dragging {
            let scroll = self.canvas_scroll_delta(ui);
            if scroll != 0.
                && let Some(pos) = pointer
            {
                let before = self.world(r, pos);
                self.plan_scale = (self.plan_scale * (f64::from(scroll) * 0.002).exp()).clamp(
                    if self
                        .host
                        .document
                        .editor
                        .project
                        .drawings
                        .cad_underlays
                        .is_empty()
                    {
                        3.
                    } else {
                        1e-6
                    },
                    self.plan_view_scale_max(250.),
                );
                let after = self.world(r, pos);
                self.plan_center.x += before.x - after.x;
                self.plan_center.y += before.y - after.y;
            }
        }
        let pan = !parameter_dragging
            && (response.dragged_by(PointerButton::Secondary)
                || response.dragged_by(PointerButton::Middle)
                || (response.dragged_by(PointerButton::Primary)
                    && ui.input(|i| i.key_down(Key::Space))));
        if pan {
            let d = ui.input(|i| i.pointer.delta());
            let radians = self
                .host
                .document
                .editor
                .project
                .plan_options
                .normalized_orientation()
                .to_radians();
            let (sin, cos) = radians.sin_cos();
            let view_x = f64::from(d.x) / self.plan_scale;
            let view_y = -f64::from(d.y) / self.plan_scale;
            self.plan_center.x -= view_x * cos + view_y * sin;
            self.plan_center.y -= -view_x * sin + view_y * cos;
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        }
        let background = self.host.document.editor.project.grid_settings.background;
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
        let active_color_start = p.add(egui::Shape::Noop);
        self.paint_placed_drawings(&p, r);
        if self
            .host
            .document
            .editor
            .project
            .plan_options
            .active_worksheet
            .is_none()
        {
            if let Some(terrain) = &self.host.document.editor.project.site.terrain {
                if let Ok(edges) = terrain.geometry.edges() {
                    for [a, b] in edges {
                        p.line_segment(
                            [
                                self.screen(r, Point::new(a[0], a[1])),
                                self.screen(r, Point::new(b[0], b[1])),
                            ],
                            Stroke::new(1., Color32::from_rgb(116, 139, 86)),
                        );
                    }
                }
            }
        }
        let origin_gesture = self.input_origin_canvas(ui, r, &response, interactive && !navigating);
        let parameter_canvas_active = origin_gesture
            || (!navigating && self.parametric_canvas_input(ui, r, &response, interactive));
        // Establish the editing session before Option is routed to parameter transfer.
        // A selected wall reference drag edits its node and commits on release.
        if !parameter_canvas_active
            && self.wall_insert.is_none()
            && self.drawings_ui.mode.is_none()
            && !self.trim_modifier_active(ui.ctx())
            && self.reshape_session.is_none()
            && self.pointer_drag.is_none()
            && self.host.document.editor.tool == Tool::Select
            && interactive
            && !ui.input(|i| i.modifiers.shift)
            && !self.canvas_pick_strict(ui, Id::new("plan_canvas_selection"))
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
                            &self.host.document.editor.project,
                            self.host.document.editor.story,
                            origin,
                            6. / self.plan_scale,
                        )
                    });
                if let Some(id) = hit {
                    let visible = self
                        .host
                        .document
                        .editor
                        .project
                        .elements
                        .iter()
                        .filter(|e| {
                            geometry::element_on_plan_story(
                                &self.host.document.editor.project,
                                e,
                                self.host.document.editor.story,
                            ) && self
                                .host
                                .document
                                .editor
                                .project
                                .plan_options
                                .element_visible(e, &self.host.document.editor.project.layers)
                        })
                        .map(|e| e.id)
                        .collect::<Vec<_>>();
                    let scope = self.canvas_selection_scope(&visible);
                    let already_selected = self.host.document.editor.selection_ids().contains(&id);
                    if already_selected || scope.contains(&id) {
                        if !already_selected {
                            self.host
                                .document
                                .editor
                                .set_scoped_selection(vec![id], &scope);
                        }
                        self.start_direct_pointer_drag(origin);
                    }
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
        let transfer_available = !parameter_canvas_active
            && self.wall_insert.is_none()
            && self.drawings_ui.mode.is_none()
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
                    &self.host.document.editor.project,
                    self.host.document.editor.story,
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
        let grid = &self.host.document.editor.project.grid_settings;
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
                .fold(f64::INFINITY, f64::min);
            let max_u = local
                .iter()
                .map(|point| point.x)
                .fold(f64::NEG_INFINITY, f64::max);
            let min_v = local
                .iter()
                .map(|point| point.y)
                .fold(f64::INFINITY, f64::min);
            let max_v = local
                .iter()
                .map(|point| point.y)
                .fold(f64::NEG_INFINITY, f64::max);
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
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .filter(|e| {
                geometry::element_on_plan_story(
                    &self.host.document.editor.project,
                    e,
                    self.host.document.editor.story,
                ) && self
                    .host
                    .document
                    .editor
                    .project
                    .plan_options
                    .element_visible(e, &self.host.document.editor.project.layers)
            })
            .collect();
        elements.sort_by_key(|e| matches!(e.shape, Shape::Wall { .. }));
        let ids = elements.iter().map(|e| e.id).collect::<Vec<_>>();
        let geometry_batch = match self
            .host
            .document
            .editor
            .geometry_batch(&ids, Some(self.host.document.editor.story))
        {
            Ok(batch) => batch,
            Err(d) => {
                Self::paint_geometry_error(&p, self.language, &d);
                crate::derivation_cache::GeometryBatch::failed(
                    &ids,
                    self.host.document.editor.revision,
                    self.host.document.editor.document_session(),
                    d,
                )
            }
        };
        let selected_ids = self.host.document.editor.selection_ids();
        for e in elements {
            if let Shape::ConceptMass { spec } = &e.shape {
                let selected = selected_ids.contains(&e.id);
                if let Ok(handle) = geometry_batch.source(e.id) {
                    if let crate::derivation_cache::GeometrySource::ConceptMass(g) =
                        handle.source.as_ref()
                    {
                        Self::paint_slab_snapshot(
                            &p,
                            g,
                            |point| self.screen(r, point),
                            self.trace_active_fill(if selected {
                                Color32::from_rgb(195, 225, 218)
                            } else {
                                Color32::from_rgb(215, 211, 199)
                            }),
                            Stroke::new(
                                if selected { 2. } else { 1. },
                                if selected {
                                    ACCENT
                                } else {
                                    Color32::from_rgb(105, 101, 89)
                                },
                            ),
                        );
                        if selected {
                            for point in spec.outline.iter().chain(spec.holes.iter().flatten()) {
                                p.rect_filled(
                                    Rect::from_center_size(self.screen(r, *point), Vec2::splat(5.)),
                                    0.,
                                    Color32::BLACK,
                                );
                            }
                        }
                    }
                }
                continue;
            }
            if let Shape::Zone { spec } = &e.shape {
                let selected = selected_ids.contains(&e.id);
                if let Ok(handle) = geometry_batch.source(e.id) {
                    if let crate::derivation_cache::GeometrySource::Zone(g) = handle.source.as_ref()
                    {
                        let color = if selected {
                            ACCENT
                        } else {
                            Color32::from_rgb(68, 135, 146)
                        };
                        for t in &g.plan_triangles {
                            p.add(egui::Shape::convex_polygon(
                                t.iter().map(|a| self.screen(r, *a)).collect(),
                                self.trace_active_fill(color.gamma_multiply(0.13)),
                                Stroke::NONE,
                            ));
                        }
                        for ring in &g.plan_paths {
                            p.add(egui::Shape::closed_line(
                                ring.iter().map(|a| self.screen(r, *a)).collect(),
                                Stroke::new(if selected { 2. } else { 1. }, color),
                            ));
                        }
                        let label = format!("{}\nZ{:03}\n{:.2} m²", e.name, e.id, g.plan_area_m2);
                        let text_color = Color32::from_rgb(35, 76, 88);
                        let galley = p.layout_no_wrap(
                            label.clone(),
                            egui::FontId::proportional(12.),
                            text_color,
                        );
                        let available_width =
                            ((g.bounds[1].x - g.bounds[0].x) * self.plan_scale * 0.9) as f32;
                        let available_height =
                            ((g.bounds[1].y - g.bounds[0].y) * self.plan_scale * 0.80) as f32;
                        let shrink = (available_width / galley.size().x.max(1.))
                            .min(available_height / galley.size().y.max(1.))
                            .min(1.);
                        let font_size = (12. * shrink).max(8.);
                        let galley = p.layout_no_wrap(
                            label,
                            egui::FontId::proportional(font_size),
                            text_color,
                        );
                        let center = self.screen(r, spec.label_point());
                        let text_rect = Rect::from_center_size(center, galley.size());
                        p.rect_filled(
                            text_rect.expand(2.),
                            2.,
                            Color32::from_rgba_unmultiplied(250, 252, 250, 240),
                        );
                        p.galley(text_rect.min, galley, text_color);
                    }
                }
                continue;
            }
            let selected = selected_ids.contains(&e.id);
            if matches!(e.shape, Shape::Wall { .. } | Shape::Opening { .. }) {
                continue;
            }
            let Some(style) = self.plan_style(e) else {
                continue;
            };
            let source = match geometry_batch.source(e.id) {
                Ok(handle) => handle.source.as_ref(),
                Err(d) => {
                    Self::paint_geometry_error(&p, self.language, &d);
                    continue;
                }
            };
            if let Shape::Polyline {
                ref points,
                arcs: _,
            } = e.shape
            {
                let crate::derivation_cache::GeometrySource::Polyline(g) = source else {
                    continue;
                };
                let tessellated = &g.geometry.plan_path;
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
            // Ordinary rectangles have no explicit contour; the shared slab
            // geometry derives their footprint from min/max as it does in 3D.
            if matches!(e.shape, Shape::Slab { .. }) {
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
                if let crate::derivation_cache::GeometrySource::Slab(g) = source {
                    Self::paint_slab_snapshot(
                        &p,
                        g,
                        |point| self.screen(r, point),
                        self.trace_active_fill(fill),
                        stroke,
                    );
                }
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
            if matches!(e.shape, Shape::Roof { .. }) {
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
                if let crate::derivation_cache::GeometrySource::Roof(g) = source {
                    Self::paint_roof_snapshot(
                        &p,
                        &g.geometry,
                        |point| self.screen(r, point),
                        self.trace_active_fill(fill),
                        stroke,
                    );
                }
                continue;
            }
            if matches!(e.shape, Shape::Column { .. }) {
                let stroke = if selected {
                    Stroke::new(2., ACCENT)
                } else {
                    Stroke::new(
                        self.plan_pen_px(style.line_width_mm),
                        Self::plan_color(style.line),
                    )
                };
                let fill = if selected {
                    Color32::from_rgb(195, 225, 218)
                } else {
                    style.fill.map_or(Color32::TRANSPARENT, Self::plan_color)
                };
                let fill = self.trace_active_fill(fill);
                match geometry_batch.column_plan(e.id) {
                    Ok(plan) => match &plan.view {
                        crate::column_geometry::ColumnPlanView::Projected { geometry, .. } => {
                            self.paint_column_plan_geometry(&p, r, geometry, fill, stroke)
                        }
                        crate::column_geometry::ColumnPlanView::SymbolicFootprint(outline) => {
                            p.add(egui::Shape::convex_polygon(
                                outline.iter().map(|v| self.screen(r, *v)).collect(),
                                fill,
                                stroke,
                            ));
                        }
                        crate::column_geometry::ColumnPlanView::NotOnStory => {}
                    },
                    Err(d) => Self::paint_geometry_error(&p, self.language, &d),
                }
                continue;
            }
            if matches!(e.shape, Shape::Stair { .. }) {
                match source {
                    crate::derivation_cache::GeometrySource::Stair(g) => {
                        let (fill, stroke) = if selected {
                            (Color32::from_rgb(195, 225, 218), Stroke::new(2., ACCENT))
                        } else {
                            (
                                style.fill.map_or(Color32::TRANSPARENT, Self::plan_color),
                                Stroke::new(
                                    self.plan_pen_px(style.line_width_mm),
                                    Self::plan_color(style.line),
                                ),
                            )
                        };
                        Self::paint_stair_snapshot(
                            &p,
                            &g.geometry,
                            |v| self.screen(r, v),
                            self.trace_active_fill(fill),
                            stroke,
                            style.draw_treads.then(|| {
                                Stroke::new(
                                    self.plan_pen_px(style.line_width_mm) * 0.8,
                                    Self::plan_color(style.line),
                                )
                            }),
                        );
                    }
                    _ => {}
                }
                continue;
            }
            if let crate::derivation_cache::GeometrySource::Beam(g) = source {
                let (fill, stroke) = if selected {
                    (Color32::from_rgb(195, 225, 218), Stroke::new(2., ACCENT))
                } else {
                    (
                        style.fill.map_or(Color32::TRANSPARENT, Self::plan_color),
                        Stroke::new(
                            self.plan_pen_px(style.line_width_mm),
                            Self::plan_color(style.line),
                        ),
                    )
                };
                Self::paint_beam_snapshot(
                    &p,
                    &g.geometry,
                    |point| self.screen(r, point),
                    self.trace_active_fill(fill),
                    stroke,
                );
            }
        }
        let wall_queries = &geometry_batch;
        let all_walls: Vec<_> = self
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .filter(|e| {
                e.story == self.host.document.editor.story
                    && e.worksheet
                        == self
                            .host
                            .document
                            .editor
                            .project
                            .plan_options
                            .active_worksheet
                    && matches!(e.shape, Shape::Wall { .. })
            })
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
            let stroke = if selected {
                Stroke::new(1.5, WALL_SELECTION_OUTLINE)
            } else {
                Stroke::new(
                    self.plan_pen_px(style.line_width_mm),
                    Self::plan_color(style.line),
                )
            };
            match wall_queries.wall(wall.id) {
                Ok(g) => {
                    Self::paint_wall_snapshot(
                        &p,
                        &g.geometry,
                        |point| self.screen(r, point),
                        fill,
                        stroke,
                    );
                }
                Err(d) => Self::paint_geometry_error(&p, self.language, &d),
            }
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
        let footprints_by_id = wall_queries
            .context(self.host.document.editor.story)
            .map(|c| c.joined_footprints.clone())
            .unwrap_or_default();
        let story_footprints: Vec<_> = walls
            .iter()
            .map(|wall| footprints_by_id.get(&wall.id).cloned().unwrap_or_default())
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
                for wall in &same_material {
                    if !selected_ids.contains(&wall.id) {
                        let wall_fill = self.trace_active_fill(
                            style
                                .fill
                                .map_or(Self::plan_color(style.line), Self::plan_color),
                        );
                        match wall_queries.wall(wall.id) {
                            Ok(g) => {
                                Self::paint_wall_snapshot(
                                    &p,
                                    &g.geometry,
                                    |point| self.screen(r, point),
                                    wall_fill,
                                    Stroke::new(0.5, wall_fill),
                                );
                            }
                            Err(d) => Self::paint_geometry_error(&p, self.language, &d),
                        }
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
                self.plan_pen_px(width_key as f64 / 1000.0),
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
            match wall_queries.wall(wall.id) {
                Ok(g) => {
                    Self::paint_wall_snapshot(
                        &p,
                        &g.geometry,
                        |point| self.screen(r, point),
                        WALL_SELECTION_FILL,
                        Stroke::new(1.5, WALL_SELECTION_OUTLINE),
                    );
                }
                Err(d) => Self::paint_geometry_error(&p, self.language, &d),
            }
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
        let background = self.host.document.editor.project.grid_settings.background;
        let hole_fill = Color32::from_rgba_unmultiplied(
            background[0],
            background[1],
            background[2],
            background[3],
        );
        let openings: Vec<_> = self
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .filter(|element| {
                element.story == self.host.document.editor.story
                    && matches!(element.shape, Shape::Opening { .. })
                    && self
                        .host
                        .document
                        .editor
                        .project
                        .plan_options
                        .element_visible(element, &self.host.document.editor.project.layers)
            })
            .collect();
        for opening in openings {
            let Some(outline) =
                geometry::opening_footprint(&self.host.document.editor.project, opening)
            else {
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
            let g = match wall_queries.opening(opening.id) {
                Ok(g) => g,
                Err(d) => {
                    Self::paint_geometry_error(&p, self.language, &d);
                    continue;
                }
            };
            for triangle in &g.geometry.plan_triangles {
                p.add(egui::Shape::convex_polygon(
                    triangle
                        .iter()
                        .copied()
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
            let _ = kind;
            for path in &g.geometry.symbol {
                let points = path.points.iter().map(|q| self.screen(r, *q)).collect();
                let pen = Stroke::new(
                    if path.arc {
                        stroke.width * 0.7
                    } else {
                        stroke.width
                    },
                    color,
                );
                p.add(if path.closed {
                    egui::Shape::closed_line(points, pen)
                } else {
                    egui::Shape::line(points, pen)
                });
            }
            if selected {
                for path in &g.geometry.plan_paths {
                    p.add(egui::Shape::closed_line(
                        path.iter()
                            .copied()
                            .map(|point| self.screen(r, point))
                            .collect(),
                        Stroke::new(2.2, ACCENT),
                    ));
                }
            }
        }
        let selection_owner = Id::new("plan_canvas_selection");
        let selection_scope = self.canvas_selection_scope(&ids);
        let selection_paths = geometry_batch
            .selection_plan_paths(&ids)
            .unwrap_or_default()
            .into_iter()
            .map(|mut path| {
                for q in &mut path.points {
                    let s = self.screen(r, *q);
                    *q = Point::new(s.x.into(), s.y.into());
                }
                path
            })
            .collect::<Vec<_>>();
        let selection_hits = pointer
            .map(|pos| {
                crate::screen_selection::pick_paths(
                    &selection_paths,
                    Point::new(pos.x.into(), pos.y.into()),
                    6.,
                    self.canvas_surface_selection(ui),
                )
            })
            .unwrap_or_default();
        let candidate_allowed = interactive
            && !pan
            && !parameter_canvas_active
            && !over_wall_edit_palette
            && self.wall_insert.is_none()
            && self.pointer_drag.is_none()
            && self.reshape_session.is_none()
            && self.drawings_ui.mode.is_none()
            && self.drag.is_none()
            && !transfer_cursor_active
            && !self.trim_modifier_active(ui.ctx())
            && !self.wall_edit.as_ref().is_some_and(|e| e.drag.is_some());
        let selection_consumed = self.canvas_candidate_input(
            ui,
            r,
            &response,
            selection_owner,
            selection_hits,
            &selection_scope,
            candidate_allowed,
        );
        self.tint_trace_active(&p, active_color_start);
        self.paint_canvas_candidate(ui, r, selection_owner, &selection_paths);
        p.set_opacity(1.);
        // AC29 pre-highlights the element the armed cursor will act on.
        if let Some(id) = self.transfer_hover
            && let Some(element) = self
                .host
                .document
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
        self.paint_trace_splitter(ui, r, trace_split);
        // Associative overall dimensions, computed from the current story.
        let walls: Vec<_> = self
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .filter(|e| {
                e.story == self.host.document.editor.story
                    && matches!(e.shape, Shape::Wall { .. })
                    && self
                        .host
                        .document
                        .editor
                        .project
                        .plan_options
                        .element_visible(e, &self.host.document.editor.project.layers)
            })
            .flat_map(geometry::footprint)
            .collect();
        if !walls.is_empty() {
            let min = walls
                .iter()
                .fold(Point::new(f64::INFINITY, f64::INFINITY), |a, b| {
                    Point::new(a.x.min(b.x), a.y.min(b.y))
                });
            let max = walls
                .iter()
                .fold(Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY), |a, b| {
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
                self.host
                    .document
                    .editor
                    .project
                    .plan_options
                    .dimension_style
                    .format_length(max.x - min.x),
                FontId::monospace(11.),
                INK,
            );
        }
        if let Some(area) = &self.host.document.editor.marquee {
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
        self.paint_parametric_canvas(ui, r);
        self.paint_input_origins(ui, r);
        if parameter_canvas_active {
            return;
        }
        if navigating {
            self.navigation_overlay(ui, r, viewport_navigation::Pane::Plan);
            return;
        }
        if self.worksheet_canvas(ui, r, &response) {
            return;
        }
        if selection_consumed {
            return;
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
        if !self.host.document.editor.polyline_points.is_empty() {
            let cursor = pointer.filter(|point| r.contains(*point)).map(|pos| {
                self.target(self.world(r, pos), ui.input(|i| i.modifiers.shift))
                    .0
            });
            DrawingPreview::Chain {
                confirmed: &self.host.document.editor.polyline_points,
                cursor,
            }
            .paint(&p, |point| self.screen(r, point), Stroke::new(1.5, ACCENT));
            for point in &self.host.document.editor.polyline_points {
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
        if self
            .host
            .document
            .editor
            .project
            .grid_settings
            .origin_display
        {
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
            let polygon_target = self.polygon_creation_active().then(|| {
                if !self.tracker_locked && kind == SnapKind::Free && q == raw {
                    Ok(self.world_precise(r, pos))
                } else {
                    self.polygon_creation_target(q)
                }
            });
            let q = polygon_target
                .as_ref()
                .and_then(|v| v.as_ref().ok())
                .map(|p| p.model_point())
                .unwrap_or(q);
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
            if !matches!(self.host.document.editor.tool, Tool::Select | Tool::Marquee) {
                let s = self.screen(r, q);
                if self.host.document.editor.snap {
                    p.rect_stroke(
                        Rect::from_center_size(s, vec2(9., 9.)),
                        0.,
                        Stroke::new(1.5, ACCENT),
                        StrokeKind::Middle,
                    );
                    p.text(
                        s + vec2(12., 12.),
                        Align2::LEFT_TOP,
                        self.language.translate(&format!("{kind:?}")),
                        FontId::proportional(10.),
                        ACCENT,
                    );
                }
                if self.host.document.editor.tool == Tool::Roof {
                    if let Some(draft) = self.host.document.editor.roof_draft {
                        let numeric_stage = self.host.document.editor.roof_polygon_draft.is_none()
                            && draft.numeric_values(q).is_some();
                        if numeric_stage && !self.tracker_locked {
                            (self.tracker_a, self.tracker_b) = draft.numeric_values(q).unwrap();
                        }
                        let numeric_valid = !numeric_stage
                            || !self.tracker_locked
                            || draft.numeric_target(self.tracker_a, self.tracker_b).is_ok();
                        let (start, end) = draft.pivot();
                        let end = end.unwrap_or(q);
                        p.line_segment(
                            [self.screen(r, start), self.screen(r, end)],
                            Stroke::new(1.5, ACCENT),
                        );
                        for point in [start, end] {
                            p.circle_filled(self.screen(r, point), 3., ACCENT);
                        }
                        if let crate::roof::RoofDraft::RotatedContour { rectangle, .. } = draft {
                            let (first, end) = rectangle.baseline();
                            let end = end.unwrap_or(q);
                            p.line_segment(
                                [self.screen(r, first), self.screen(r, end)],
                                Stroke::new(1.5, ACCENT),
                            );
                            for point in [first, end] {
                                p.circle_filled(self.screen(r, point), 3., ACCENT);
                            }
                        }
                        if let Some(polygon) = self.host.document.editor.roof_polygon_draft() {
                            let regions = if self.polygon_creation_active() {
                                polygon_target
                                    .as_ref()
                                    .expect("polygon target")
                                    .clone()
                                    .and_then(|target| {
                                        let analysis = polygon.precise_region_analysis(
                                            Some(target),
                                            self.host.document.editor.roof_size,
                                        )?;
                                        Ok(match *analysis.choice() {
                                            crate::slab_geometry::CreationRegionChoice::Unique(
                                                i,
                                            ) => vec![analysis.candidates()[i].clone()],
                                            _ => analysis.candidates().to_vec(),
                                        })
                                    })
                            } else {
                                self.host.document.editor.roof_polygon_candidates(Some(q))
                            };
                            if let Ok(regions) = regions {
                                for spec in regions {
                                    Self::paint_roof_polygon(
                                        &p,
                                        &spec,
                                        |point| self.screen(r, point),
                                        Color32::from_rgba_unmultiplied(39, 146, 137, 50),
                                        Stroke::NONE,
                                        self.language,
                                    );
                                }
                            }
                            DrawingPreview::Chain {
                                confirmed: polygon.points(),
                                cursor: Some(q),
                            }
                            .paint(
                                &p,
                                |point| self.screen(r, point),
                                Stroke::new(1.5, ACCENT),
                            );
                        } else if numeric_valid
                            && let Some(spec) = self.host.document.editor.roof_preview(q)
                        {
                            Self::paint_roof_polygon(
                                &p,
                                &spec,
                                |point| self.screen(r, point),
                                Color32::from_rgba_unmultiplied(39, 146, 137, 50),
                                Stroke::new(1.5, ACCENT),
                                self.language,
                            );
                        } else if matches!(draft, crate::roof::RoofDraft::RiseSide { .. }) {
                            let mid = Point::new((start.x + end.x) / 2., (start.y + end.y) / 2.);
                            p.line_segment([self.screen(r, mid), s], Stroke::new(1., ACCENT));
                        }
                    }
                } else if self.host.document.editor.tool == Tool::Slab
                    && let Some(draft) = self.host.document.editor.slab_polygon_draft()
                {
                    if let Ok(elements) = if self.host.document.editor.slab_method
                        == crate::slab_creation::SlabGeometryMethod::Polygonal
                    {
                        polygon_target
                            .as_ref()
                            .expect("polygon target")
                            .clone()
                            .and_then(|target| self.polygon_slab_preview_target(target))
                    } else {
                        self.host.document.editor.slab_polygon_preview(q)
                    } {
                        for element in elements {
                            Self::paint_slab_polygon(
                                &p,
                                &element,
                                |point| self.screen(r, point),
                                Color32::from_rgba_unmultiplied(39, 146, 137, 50),
                                Stroke::NONE,
                                self.language,
                            );
                        }
                    }
                    DrawingPreview::Chain {
                        confirmed: draft.points(),
                        cursor: Some(q),
                    }
                    .paint(
                        &p,
                        |point| self.screen(r, point),
                        Stroke::new(1.5, ACCENT),
                    );
                } else if self.host.document.editor.tool == Tool::Slab
                    && self.host.document.editor.slab_method
                        == crate::slab_creation::SlabGeometryMethod::RotatedRectangle
                {
                    if let Some(draft) = self.host.document.editor.slab_draft {
                        if !self.tracker_locked {
                            (self.tracker_a, self.tracker_b) = draft.numeric_values(q);
                        }
                        let numeric_valid = !self.tracker_locked
                            || draft.numeric_target(self.tracker_a, self.tracker_b).is_ok();
                        let (start, end) = draft.baseline();
                        p.line_segment(
                            [self.screen(r, start), self.screen(r, end.unwrap_or(q))],
                            Stroke::new(1.5, ACCENT),
                        );
                        if numeric_valid
                            && let Some(element) = self.host.document.editor.rotated_slab_preview(q)
                        {
                            Self::paint_slab_polygon(
                                &p,
                                &element,
                                |point| self.screen(r, point),
                                Color32::from_rgba_unmultiplied(39, 146, 137, 50),
                                Stroke::new(1.5, ACCENT),
                                self.language,
                            );
                        }
                    }
                } else if self.host.document.editor.tool == Tool::Beam
                    && self.host.document.editor.beam_arc_draft.is_some()
                {
                    let mut draft = self.host.document.editor.beam_arc_draft.unwrap();
                    let valid = if self.tracker_locked {
                        if draft.start().is_some() {
                            draft.set_sweep_degrees(self.tracker_a as f64).is_ok()
                        } else {
                            self.tracker_a.is_finite() && self.tracker_a >= 0.01
                        }
                    } else if draft.start().is_none() {
                        self.tracker_a = draft.center().distance(q);
                        true
                    } else {
                        let valid = draft.update_cursor(q).is_ok();
                        self.tracker_a = draft.sweep_degrees().unwrap_or(0.) as f64;
                        valid
                    };
                    self.host.document.editor.beam_arc_draft = Some(draft);
                    let center = draft.center();
                    let radial_end = if self.tracker_locked
                        && draft.start().is_none()
                        && valid
                        && center.distance(q) > 0.00001
                    {
                        let scale = self.tracker_a / center.distance(q);
                        Point::new(
                            center.x + (q.x - center.x) * scale,
                            center.y + (q.y - center.y) * scale,
                        )
                    } else if valid {
                        draft.candidate().map_or(q, |candidate| candidate.end)
                    } else {
                        q
                    };
                    p.line_segment(
                        [self.screen(r, center), self.screen(r, radial_end)],
                        Stroke::new(1., ACCENT),
                    );
                    p.circle_stroke(self.screen(r, center), 3., Stroke::new(1., ACCENT));
                    if let Some(start) = draft.start() {
                        p.line_segment(
                            [self.screen(r, center), self.screen(r, start)],
                            Stroke::new(1., ACCENT),
                        );
                    }
                    if valid && let Some(element) = self.host.document.editor.beam_arc_preview() {
                        Self::paint_beam_polygon(
                            &p,
                            &self.host.document.editor.project,
                            &element,
                            |point| self.screen(r, point),
                            Color32::from_rgba_unmultiplied(39, 146, 137, 50),
                            Stroke::new(1.5, ACCENT),
                            self.language,
                        );
                    }
                } else if self.host.document.editor.tool == Tool::Column {
                    let candidate = if self.host.document.editor.column_two_click {
                        self.host.document.editor.anchor.map(|base| {
                            p.line_segment(
                                [self.screen(r, base), self.screen(r, q)],
                                Stroke::new(1., ACCENT),
                            );
                            self.host.document.editor.column_axis_preview(base, q)
                        })
                    } else {
                        Some(self.host.document.editor.column_creation_preview(q))
                    };
                    if let Some(Ok(element)) = candidate {
                        if let Some(Ok(plan)) = crate::column_plan::for_element(
                            &self.host.document.editor.project,
                            &element,
                            self.host.document.editor.story,
                        ) {
                            self.paint_column_plan_geometry(
                                &p,
                                r,
                                &plan,
                                Color32::from_rgba_unmultiplied(39, 146, 137, 50),
                                Stroke::new(1.5, ACCENT),
                            );
                        } else {
                            p.add(egui::Shape::convex_polygon(
                                geometry::element_plan_outline(
                                    &self.host.document.editor.project,
                                    &element,
                                )
                                .into_iter()
                                .map(|point| self.screen(r, point))
                                .collect(),
                                Color32::from_rgba_unmultiplied(39, 146, 137, 50),
                                Stroke::new(1.5, ACCENT),
                            ));
                        }
                    }
                } else if self.host.document.editor.tool == Tool::Wall {
                    if let Some(a) = self.host.document.editor.anchor {
                        let rectangular = self
                            .host
                            .document
                            .editor
                            .wall_settings
                            .geometry
                            .geometry_method
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
                        self.paint_wall_batch_preview(
                            &p,
                            r,
                            self.wall_preview_segments(q, 9. / self.plan_scale),
                        );
                        let label = if rectangular {
                            format!(
                                "{} × {}",
                                self.host
                                    .document
                                    .editor
                                    .project
                                    .plan_options
                                    .dimension_style
                                    .format_length((q.x - a.x).abs()),
                                self.host
                                    .document
                                    .editor
                                    .project
                                    .plan_options
                                    .dimension_style
                                    .format_length((q.y - a.y).abs())
                            )
                        } else {
                            format!(
                                "{}  /  {:.1}°",
                                self.host
                                    .document
                                    .editor
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
                } else if matches!(self.host.document.editor.tool, Tool::Door | Tool::Window) {
                    let kind = if self.host.document.editor.tool == Tool::Door {
                        OpeningKind::Door
                    } else {
                        OpeningKind::Window
                    };
                    if let Ok(placement) =
                        self.host
                            .document
                            .editor
                            .opening_placement(kind, q, 9. / self.plan_scale)
                        && let Some(outline) = geometry::opening_footprint(
                            &self.host.document.editor.project,
                            &placement,
                        )
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
                            if let Some((a, b)) = geometry::opening_frame_span(
                                &self.host.document.editor.project,
                                &placement,
                            ) {
                                let base =
                                    self.host.document.editor.project.elevation(placement.story)
                                        + sill;
                                let symbol = crate::opening_joinery::generate(
                                    a,
                                    b,
                                    base,
                                    base + height,
                                    kind,
                                    placement.opening_joinery.unwrap_or_default(),
                                );
                                for path in symbol.plan {
                                    let points = path
                                        .points
                                        .into_iter()
                                        .map(|p| self.screen(r, p))
                                        .collect();
                                    let stroke =
                                        Stroke::new(if path.arc { 0.8 } else { 1.2 }, ACCENT);
                                    p.add(if path.closed {
                                        egui::Shape::closed_line(points, stroke)
                                    } else {
                                        egui::Shape::line(points, stroke)
                                    });
                                }
                            }
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
                } else if let Some(a) = self.host.document.editor.anchor {
                    if !self.tracker_locked {
                        self.tracker_a = q.x - a.x;
                        self.tracker_b = q.y - a.y;
                    }
                    let candidate = match self.host.document.editor.tool {
                        Tool::Wall => unreachable!("walls use the wall preview"),
                        Tool::Beam => self.host.document.editor.beam_path_preview(
                            a,
                            q,
                            self.host.document.editor.beam_slant_angle,
                            None,
                        ),
                        Tool::Slab => self
                            .host
                            .document
                            .editor
                            .rectangular_slab_preview(a, q)
                            .ok(),
                        Tool::Stair => {
                            let mut spec = self.host.document.editor.stair_spec.clone();
                            spec.start = a;
                            spec.end = q;
                            self.host
                                .document
                                .editor
                                .stair_creation_candidate(spec)
                                .ok()
                        }
                        tool => {
                            let shape = if tool == Tool::Polyline {
                                Shape::Polyline {
                                    points: vec![a, q],
                                    arcs: vec![],
                                }
                            } else {
                                Shape::Slab {
                                    min: Point::new(a.x.min(q.x), a.y.min(q.y)),
                                    max: Point::new(a.x.max(q.x), a.y.max(q.y)),
                                    thickness: self.host.document.editor.thickness,
                                }
                            };
                            Some(Element {
                                worksheet: None,
                                id: 0,
                                name: String::new(),
                                story: self.host.document.editor.story,
                                material: self.host.document.editor.material,
                                layer: ElementLayer(ARCHICAD_LAYER),
                                renovation: self.host.document.editor.wall_renovation,
                                design_option: None,
                                wall_contour: None,
                                slab_contour: None,
                                opening_joinery: None,
                                wall_settings: None,
                                shape,
                            })
                        }
                    };
                    if let Some(tmp) = candidate {
                        if matches!(tmp.shape, Shape::Beam { .. }) {
                            Self::paint_beam_polygon(
                                &p,
                                &self.host.document.editor.project,
                                &tmp,
                                |v| self.screen(r, v),
                                Color32::from_rgba_unmultiplied(39, 146, 137, 50),
                                Stroke::new(1.5, ACCENT),
                                self.language,
                            );
                        } else if matches!(tmp.shape, Shape::Stair { .. }) {
                            if let Ok(g) = crate::stair_geometry::derive(
                                &self.host.document.editor.project,
                                &tmp,
                            ) {
                                Self::paint_stair_snapshot(
                                    &p,
                                    &g.geometry,
                                    |v| self.screen(r, v),
                                    Color32::from_rgba_unmultiplied(39, 146, 137, 50),
                                    Stroke::new(1.5, ACCENT),
                                    Some(Stroke::new(0.9, ACCENT)),
                                );
                            }
                        } else {
                            let points = geometry::footprint(&tmp);
                            p.add(egui::Shape::convex_polygon(
                                points.iter().map(|v| self.screen(r, *v)).collect(),
                                Color32::from_rgba_unmultiplied(39, 146, 137, 50),
                                Stroke::NONE,
                            ));
                            DrawingPreview::Boundary(&points).paint(
                                &p,
                                |point| self.screen(r, point),
                                Stroke::new(1.5, ACCENT),
                            );
                        }
                    }
                    // Beam/stair direction is construction input. An area
                    // candidate already supplies its full rectangle boundary.
                    if matches!(self.host.document.editor.tool, Tool::Beam | Tool::Stair) {
                        DrawingPreview::Chain {
                            confirmed: &[a],
                            cursor: Some(q),
                        }
                        .paint(
                            &p,
                            |point| self.screen(r, point),
                            Stroke::new(1., ACCENT),
                        );
                    }
                    let label = format!(
                        "{}  /  {:.1}°",
                        self.host
                            .document
                            .editor
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
                } else if self.host.document.editor.tool == Tool::Polyline
                    && let Some(a) = self.host.document.editor.polyline_points.last().copied()
                {
                    if !self.tracker_locked {
                        self.tracker_a = a.distance(q);
                        self.tracker_b = (q.y - a.y).atan2(q.x - a.x).to_degrees();
                    }
                    let label = format!(
                        "{}  /  {:.1}°",
                        self.host
                            .document
                            .editor
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
            let wall_double = self.host.document.editor.tool == Tool::Wall
                && (response.double_clicked_by(PointerButton::Primary)
                    || response.triple_clicked_by(PointerButton::Primary))
                && matches!(
                    self.host
                        .document
                        .editor
                        .wall_settings
                        .geometry
                        .geometry_method,
                    WallGeometryMethod::Continuous | WallGeometryMethod::Polygonal
                );
            if response.clicked()
                && ui.input(|i| i.focused)
                && !(self.canvas_keyboard_blocked(ui) && self.canvas_selection_context(ui))
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
                    let hit = self.hit_plan_in_frame(raw, 6. / self.plan_scale, &geometry_batch);
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
                            self.host.document.editor.status =
                                "Orientation needs two distinct points".into();
                        }
                    } else {
                        self.orientation_pick_start = Some(q);
                        self.host.document.editor.status =
                            "Set Orientation · click the second point".into();
                    }
                } else if self.rotated_grid_tool.is_some() {
                    self.handle_rotated_grid_click(raw, q, 9. / self.plan_scale);
                } else if self.host.document.editor.tool == Tool::Marquee {
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
                                    self.host.document.editor.status =
                                        "Rotated rectangle needs non-zero sides".into();
                                }
                            }
                        }
                        MarqueeGeometry::Polygon => self.marquee_points.push(q),
                    }
                } else if self.host.document.editor.tool == Tool::Select {
                    let wall_context = (!ui.input(|i| i.modifiers.shift))
                        .then(|| self.selected_wall_baseline_hit(raw, 9. / self.plan_scale))
                        .flatten();
                    if !ui.input(|i| i.modifiers.shift)
                        && let Some((origin, ids)) =
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
                        let context = (!ui.input(|i| i.modifiers.shift))
                            .then(|| self.selected_polyline_hit(raw, 9. / self.plan_scale))
                            .flatten();
                        if let Some((id, node, edge)) = context {
                            self.drag = None;
                            self.start_polyline_pet(id, node, edge, pos, raw, false);
                            ui.ctx().request_repaint();
                        } else {
                            self.commit_canvas_candidate(ui, selection_owner, &selection_scope);
                        }
                    }
                    self.tracker_locked = false;
                    self.draft_id = None;
                } else if self.host.document.editor.tool == Tool::Polyline {
                    let close_start = self.host.document.editor.polyline_points.len() >= 2
                        && self
                            .host
                            .document
                            .editor
                            .polyline_points
                            .first()
                            .is_some_and(|start| self.screen(r, *start).distance(pos) <= 9.);
                    if close_start {
                        self.close_polyline();
                    } else {
                        self.commit_polyline_point(q, 0.01);
                    }
                } else if self.host.document.editor.tool == Tool::Wall {
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
                            .host
                            .document
                            .editor
                            .wall_close_target()
                            .filter(|start| self.screen(r, *start).distance(pos) <= 12.);
                        if let Some(start) = close_start {
                            self.host.document.editor.click(start, 2. / self.plan_scale);
                        } else {
                            self.host.document.editor.click(q, 6. / self.plan_scale);
                        }
                    }
                    self.tracker_locked = false;
                    self.draft_id = None;
                } else if matches!(self.host.document.editor.tool, Tool::Door | Tool::Window) {
                    let defaults_ok = !self
                        .info_box_opening_tool
                        .as_ref()
                        .is_some_and(|state| state.dirty)
                        || self.commit_info_box_opening_tool();
                    if defaults_ok {
                        self.host.document.editor.click(q, 6. / self.plan_scale);
                    }
                    self.tracker_locked = false;
                    self.draft_id = None;
                } else {
                    if self.host.document.editor.tool == Tool::Beam
                        && (self.host.document.editor.beam_arc_draft.is_some()
                            || self.host.document.editor.beam_method
                                == crate::beam_creation::BeamGeometryMethod::CenterRadius)
                    {
                        self.advance_beam_arc_input(q);
                    } else if self.polygon_creation_active() {
                        match polygon_target.as_ref().expect("polygon target").clone() {
                            Ok(target) => self.advance_polygon_creation_precise_input(
                                target,
                                6. / self.plan_scale,
                            ),
                            Err(error) => self.host.document.editor.status = error,
                        }
                    } else if self.host.document.editor.tool == Tool::Slab
                        && self.host.document.editor.slab_method
                            == crate::slab_creation::SlabGeometryMethod::RotatedRectangle
                    {
                        self.advance_rotated_slab_input(q);
                    } else if self.host.document.editor.tool == Tool::Roof
                        && self.host.document.editor.roof_polygon_draft.is_none()
                    {
                        self.advance_roof_input(q);
                    } else {
                        self.advance_simple_creation_input(q, 6. / self.plan_scale);
                    }
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
        if self.host.document.editor.tool == Tool::ConceptMass
            && self.host.document.editor.zone_polygon
            && !self.host.document.editor.polyline_points.is_empty()
            && response.clicked_by(PointerButton::Secondary)
        {
            self.finish_mass_polygon_input();
        }
        if self.host.document.editor.tool == Tool::Polyline
            && response.clicked_by(PointerButton::Secondary)
        {
            self.finish_polyline();
        }
        if self.host.document.editor.tool == Tool::Wall
            && response.clicked_by(PointerButton::Secondary)
        {
            self.finish_wall_draft();
        }
        if self.host.document.editor.tool == Tool::Marquee
            && self.marquee_geometry == MarqueeGeometry::Polygon
            && response.clicked_by(PointerButton::Secondary)
        {
            if self.marquee_points.len() >= 3 {
                self.place_marquee(self.marquee_points.clone());
            } else {
                self.marquee_points.clear();
                self.host.document.editor.status =
                    "Marquee polygon needs at least three points".into();
            }
        }
        if self.host.document.editor.tool == Tool::Marquee
            && self.marquee_geometry == MarqueeGeometry::Rectangle
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
        if self.host.document.editor.tool == Tool::Select
            && !ui.input(|i| i.modifiers.shift)
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
                    edit.drag = Some((self.host.document.editor.project.clone(), w));
                }
            }
        }
        if self.host.document.editor.tool == Tool::Select
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
                let mut release_valid = false;
                if let Some(pos) = pointer.or_else(|| ui.input(|i| i.pointer.latest_pos())) {
                    let (current, snap) = self.wall_endpoint_drag_target(
                        before,
                        &edit,
                        self.world(r, pos),
                        ui.input(|i| i.modifiers.shift),
                    );
                    release_valid = self.apply_wall_drag_preview(before, &edit, *start, current);
                    if release_valid && let Some(active) = self.wall_edit.as_mut() {
                        active.snap = snap;
                    }
                }
                if let Some(active) = self.wall_edit.as_mut() {
                    active.drag = None;
                }
                if !release_valid {
                    // A rejected release must not commit an earlier valid preview.
                    self.host.document.editor.project = before.clone();
                    if let Some(active) = self.wall_edit.as_ref() {
                        self.host.document.editor.status = active.error.clone();
                    }
                } else if self.host.document.editor.project != *before {
                    let joined = self
                        .wall_edit
                        .as_ref()
                        .is_some_and(|edit| edit.snap.is_some());
                    self.finish_wall_reference_drag(before.clone(), edit.command, joined);
                    self.wall_edit = None;
                }
            }
        }
        let selection_allowed = interactive
            && self.host.document.editor.tool == Tool::Select
            && !pan
            && !parameter_canvas_active
            && !trace_pointer
            && !over_wall_edit_palette
            && self.wall_insert.is_none()
            && self.pointer_drag.is_none()
            && self.reshape_session.is_none()
            && self.drawings_ui.mode.is_none()
            && self.drag.is_none()
            && !self.wall_edit.as_ref().is_some_and(|e| e.drag.is_some())
            && !transfer_cursor_active
            && !self.canvas_keyboard_blocked(ui)
            && !self.trim_modifier_active(ui.ctx());
        if let Some(b) = self.selection_drag(ui, r, &response, selection_owner, selection_allowed) {
            match geometry_batch.selection_plan_paths(&ids) {
                Ok(mut paths) => {
                    for path in &mut paths {
                        for q in &mut path.points {
                            let pixel = self.screen(r, *q);
                            *q = Point::new(pixel.x.into(), pixel.y.into());
                        }
                    }
                    let rectangle = geometry::rectangle_polygon(
                        Point::new(b.start.x.into(), b.start.y.into()),
                        Point::new(b.end.x.into(), b.end.y.into()),
                    );
                    self.apply_scoped_canvas_selection(
                        crate::screen_selection::select_paths(
                            &paths,
                            &rectangle,
                            b.end.x >= b.start.x,
                        ),
                        ui.input(|i| i.modifiers.shift),
                        &selection_scope,
                    );
                }
                Err(d) => {
                    self.host.document.editor.status =
                        crate::geometry_validation::compatibility_message(&d)
                }
            }
        }
        self.paint_selection_box(&p, selection_owner);
        if hovered && !transfer_cursor_active {
            let drawing_tool = matches!(
                self.host.document.editor.tool,
                Tool::Zone
                    | Tool::ConceptMass
                    | Tool::Roof
                    | Tool::Beam
                    | Tool::Column
                    | Tool::Wall
                    | Tool::Slab
                    | Tool::Stair
                    | Tool::Polyline
                    | Tool::Door
                    | Tool::Window
            );
            let interaction_drag = pan
                || self
                    .wall_edit
                    .as_ref()
                    .is_some_and(|edit| edit.drag.is_some())
                || self.drag.is_some();
            if drawing_tool
                && !self.canvas_selection_context(ui)
                && !interaction_drag
                && self.rotated_grid_tool.is_none()
            {
                // AC29 drawing cursor: a square crosshair riding the snap
                // point — endpoint, midpoint or the raw pointer when free.
                let pos = pointer.unwrap_or(r.center());
                let (target, _) = self.target(self.world(r, pos), ui.input(|i| i.modifiers.shift));
                crate::transfer_cursor::paint_crosshair(&p, self.screen(r, target));
                ui.ctx().set_cursor_icon(CursorIcon::None);
            } else {
                let feedback = if self.canvas_selection_context(ui)
                    && self.host.document.editor.tool != Tool::Select
                {
                    CursorFeedback::Arrow
                } else if self.host.document.editor.tool == Tool::Polyline {
                    if ui.input(|i| i.pointer.button_down(PointerButton::Primary)) {
                        CursorFeedback::SolidPencil
                    } else if self.host.document.editor.polyline_points.is_empty() {
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
                } else if self.host.document.editor.tool == Tool::Select
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
                        self.host
                            .document
                            .editor
                            .project
                            .elements
                            .iter()
                            .any(|element| {
                                element.id == *id && matches!(element.shape, Shape::Polyline { .. })
                            })
                    })
                }) {
                    CursorFeedback::Arrow
                } else if self.drag.is_some() {
                    CursorFeedback::Dragging
                } else {
                    let pos = pointer.unwrap_or(r.center());
                    match self.host.document.editor.tool {
                        Tool::Select => {
                            let raw = self.world(r, pos);
                            self.select_cursor_feedback_in_frame(raw, &geometry_batch)
                        }
                        Tool::Marquee => CursorFeedback::Drawing,
                        Tool::Zone
                        | Tool::ConceptMass
                        | Tool::Roof
                        | Tool::Beam
                        | Tool::Column
                        | Tool::Wall
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
            if let Some(sheet) = self
                .host
                .document
                .editor
                .project
                .plan_options
                .active_worksheet
            {
                format!(
                    "{} · {}",
                    self.language.translate("Worksheet"),
                    self.host
                        .document
                        .editor
                        .project
                        .drawings
                        .pages
                        .iter()
                        .find(|p| p.id == sheet)
                        .map_or("", |p| p.name.as_str())
                )
            } else {
                format!(
                    "{:02}  /  {}",
                    self.host.document.editor.story,
                    self.host
                        .document
                        .editor
                        .project
                        .stories
                        .iter()
                        .find(|s| s.id == self.host.document.editor.story)
                        .map_or("Floor plan", |s| s.name.as_str())
                )
            },
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
                .host
                .document
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
                            self.host
                                .document
                                .editor
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
                    .host
                    .document
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
        self.wall_edit_palette(ui.ctx(), false);
    }

    pub(super) fn wall_edit_palette(&mut self, ctx: &Context, model: bool) {
        if let Some(edit) = self.wall_edit.as_mut() {
            if self.view == View::Split && edit.model_canvas.is_some_and(|owner| owner != model) {
                return;
            }
            edit.model_canvas = Some(model);
        }
        let Some(edit) = self.wall_edit.as_ref() else {
            self.wall_edit_palette_rect = None;
            return;
        };
        if edit.drag.is_some() {
            self.wall_edit_palette_rect = None;
            return;
        }
        if !edit.error.is_empty() {
            self.host.document.editor.status = edit.error.clone();
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
    fn advance_simple_creation_input(&mut self, point: Point, tolerance: f64) {
        let anchor = self.host.document.editor.anchor;
        let preserve_rejected = matches!(
            self.host.document.editor.tool,
            Tool::Stair | Tool::Beam | Tool::Slab | Tool::Zone | Tool::ConceptMass
        ) && anchor.is_some();
        self.host.document.editor.click(point, tolerance);
        if !preserve_rejected || self.host.document.editor.anchor != anchor {
            self.tracker_locked = false;
        }
    }

    pub(super) fn tracker(&mut self, ui: &mut Ui) {
        if self.polygon_creation_active() {
            self.polygon_creation_tracker(ui);
            return;
        }
        if self.host.document.editor.slab_polygon_draft().is_some() {
            return;
        }
        if self.host.document.editor.tool == Tool::Beam
            && self.host.document.editor.beam_arc_draft.is_some()
        {
            self.beam_arc_tracker(ui);
            return;
        }
        if self.host.document.editor.tool == Tool::Slab
            && self.host.document.editor.slab_method
                == crate::slab_creation::SlabGeometryMethod::RotatedRectangle
        {
            self.rotated_slab_tracker(ui);
            return;
        }
        if self.host.document.editor.tool == Tool::Roof {
            self.roof_tracker(ui);
            return;
        }
        let polyline = self.host.document.editor.tool == Tool::Polyline;
        let wall = self.host.document.editor.tool == Tool::Wall;
        let wall_method = self
            .host
            .document
            .editor
            .wall_settings
            .geometry
            .geometry_method;
        let rectangular = wall && wall_method == WallGeometryMethod::Rectangular;
        let chained = wall
            && matches!(
                wall_method,
                WallGeometryMethod::Continuous | WallGeometryMethod::Polygonal
            );
        let tracker_anchor = self.host.document.editor.anchor.or_else(|| {
            polyline
                .then(|| self.host.document.editor.polyline_points.last().copied())
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
                            matches!(self.host.document.editor.tool, Tool::Wall | Tool::Polyline)
                                && !rectangular;
                        let a_label = ui.label(self.language.translate(if angular {
                            if wall { "Length" } else { "Distance" }
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
                            .labelled_by(a_label.id)
                            .changed()
                        {
                            self.tracker_locked = true;
                        }
                        let b_label = ui.label(self.language.translate(if angular {
                            "Angle"
                        } else {
                            "ΔY"
                        }));
                        if ui
                            .add(
                                DragValue::new(&mut self.tracker_b)
                                    .speed(0.1)
                                    .fixed_decimals(2)
                                    .suffix(if angular { " °" } else { " m" }),
                            )
                            .labelled_by(b_label.id)
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
                                self.host.document.editor.click(target, 0.);
                                self.tracker_locked = false;
                                self.draft_id = None;
                            } else if wall {
                                if let Err(error) =
                                    self.host.document.editor.finish_wall(Some(target))
                                {
                                    self.host.document.editor.status =
                                        localized_error(self.language, &error);
                                }
                                self.tracker_locked = false;
                                self.draft_id = None;
                            } else {
                                self.advance_simple_creation_input(target, 0.);
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
                            self.host.document.editor.cancel_wall_draft();
                            self.tracker_locked = false;
                        }
                    });
                });
        } else if polyline
            || wall
            || matches!(
                self.host.document.editor.tool,
                Tool::Zone | Tool::ConceptMass
            )
        {
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
