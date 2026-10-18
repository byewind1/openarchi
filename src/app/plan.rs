use super::*;

impl AtelierApp {
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
        self.editor
            .project
            .plan_options
            .style_for(element, self.plan_pen())
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
        for element in self
            .editor
            .project
            .elements
            .iter()
            .filter(|e| selected.contains(&e.id))
        {
            let Shape::Polyline { points, arcs } = &element.shape else {
                continue;
            };
            if let Some((index, _)) = points
                .iter()
                .enumerate()
                .map(|(index, node)| (index, node.distance(point)))
                .filter(|(_, distance)| *distance <= tolerance)
                .min_by(|a, b| a.1.total_cmp(&b.1))
            {
                return Some((element.id, Some(index), None));
            }
            if let Some((index, _)) = points
                .windows(2)
                .enumerate()
                .map(|(index, edge)| {
                    let path = arcs
                        .get(index)
                        .and_then(|arc| *arc)
                        .and_then(|arc| geometry::arc_points(edge[0], edge[1], arc))
                        .unwrap_or_else(|| vec![edge[0], edge[1]]);
                    let distance = path
                        .windows(2)
                        .map(|pair| geometry::segment_distance(point, pair[0], pair[1]))
                        .fold(f32::INFINITY, f32::min);
                    (index, distance)
                })
                .filter(|(_, distance)| *distance <= tolerance)
                .min_by(|a, b| a.1.total_cmp(&b.1))
            {
                return Some((element.id, None, Some(index)));
            }
        }
        None
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
    pub(super) fn open_polyline_edit(
        &mut self,
        id: u64,
        node: Option<usize>,
        edge: Option<usize>,
        at: Pos2,
    ) {
        let command = if node.is_some() {
            PolylineEditCommand::MoveNode
        } else {
            PolylineEditCommand::OffsetPolyline
        };
        self.polyline_edit = Some(PolylineEdit {
            element_id: id,
            node,
            edge,
            command,
            palette_at: at + vec2(14., 12.),
            drag: None,
            chamfer_length: 0.25,
            fillet_radius: 0.25,
            multiply_count: 2,
            error: String::new(),
        });
        self.editor.status = if node.is_some() {
            "Polyline node edit · drag the node or choose a palette command".into()
        } else {
            "Polyline edge edit · choose a palette command".into()
        };
    }
    #[cfg(test)]
    pub(super) fn insert_polyline_node_at(
        &mut self,
        id: u64,
        edge_index: usize,
        point: Point,
        at: Pos2,
    ) {
        let before = self.editor.project.clone();
        let Some(element) = before.elements.iter().find(|element| element.id == id) else {
            return;
        };
        let Shape::Polyline { points, arcs } = &element.shape else {
            return;
        };
        if arcs.get(edge_index).is_some_and(Option::is_some) {
            self.open_polyline_edit(id, None, Some(edge_index), at);
            self.editor.status =
                "Curved edge selected · choose Curve edge to adjust its radius".into();
            return;
        }
        let Some((&a, &b)) = points.get(edge_index).zip(points.get(edge_index + 1)) else {
            return;
        };
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let length_squared = dx * dx + dy * dy;
        if length_squared <= 1e-10 {
            self.editor.status = "Cannot insert a node into a zero-length edge".into();
            return;
        }
        let t = (((point.x - a.x) * dx + (point.y - a.y) * dy) / length_squared).clamp(0., 1.);
        let node = Point::new(a.x + t * dx, a.y + t * dy);
        if node.distance(a) < 0.01 || node.distance(b) < 0.01 {
            self.editor.status =
                "A new node must leave at least 0.01 m on both edge segments".into();
            return;
        }
        let node_index = edge_index + 1;
        let Some(slot) = self
            .editor
            .project
            .elements
            .iter_mut()
            .find(|element| element.id == id)
        else {
            return;
        };
        let Shape::Polyline { points, arcs } = &mut slot.shape else {
            return;
        };
        points.insert(node_index, node);
        if !arcs.is_empty() {
            arcs.insert(edge_index + 1, None);
        }
        if self.editor.project.validate().is_err() {
            self.editor.project = before;
            self.editor.status = "Cannot insert a node into invalid Polyline geometry".into();
            return;
        }
        self.editor.commit(before);
        self.editor.set_selection(vec![id]);
        self.open_polyline_edit(id, Some(node_index), Some(edge_index), at);
        self.editor.status = "Polyline node inserted · drag it or choose a palette command".into();
    }
    pub(super) fn apply_polyline_drag_preview(
        &mut self,
        before: &Project,
        edit: &PolylineEdit,
        start: Point,
        current: Point,
    ) {
        self.editor.project = before.clone();
        let Some(original) = before
            .elements
            .iter()
            .find(|element| element.id == edit.element_id)
        else {
            return;
        };
        let Shape::Polyline {
            points: source,
            arcs: source_arcs,
        } = &original.shape
        else {
            return;
        };
        let mut points = source.clone();
        let mut arcs = if source_arcs.is_empty() {
            vec![None; source.len().saturating_sub(1)]
        } else {
            source_arcs.clone()
        };
        let delta = Point::new(current.x - start.x, current.y - start.y);
        match edit.command {
            PolylineEditCommand::InsertNode => {
                if let Some(index) = edit.edge.filter(|index| *index + 1 < source.len()) {
                    let a = source[index];
                    let b = source[index + 1];
                    let (node, split) = if let Some(arc) = arcs[index] {
                        let Some((center, sweep)) = geometry::arc_center_sweep(a, b, arc) else {
                            return;
                        };
                        let angle = (current.y - center.y).atan2(current.x - center.x);
                        let start_angle = (a.y - center.y).atan2(a.x - center.x);
                        let traveled = if sweep > 0. {
                            (angle - start_angle).rem_euclid(std::f32::consts::TAU)
                        } else {
                            (start_angle - angle).rem_euclid(std::f32::consts::TAU)
                        };
                        if traveled >= sweep.abs() {
                            return;
                        }
                        let node = Point::new(
                            center.x + arc.radius * angle.cos(),
                            center.y + arc.radius * angle.sin(),
                        );
                        let mut first = arc;
                        first.large_arc = traveled > std::f32::consts::PI;
                        let mut second = arc;
                        second.large_arc = sweep.abs() - traveled > std::f32::consts::PI;
                        (node, (Some(first), Some(second)))
                    } else {
                        let dx = b.x - a.x;
                        let dy = b.y - a.y;
                        let t = (((current.x - a.x) * dx + (current.y - a.y) * dy)
                            / (dx * dx + dy * dy))
                            .clamp(0., 1.);
                        (Point::new(a.x + t * dx, a.y + t * dy), (None, None))
                    };
                    if node.distance(a) < 0.01 || node.distance(b) < 0.01 {
                        return;
                    }
                    points.insert(index + 1, node);
                    arcs[index] = split.0;
                    arcs.insert(index + 1, split.1);
                }
            }
            PolylineEditCommand::MoveNode => {
                if let Some(node) = edit.node.filter(|index| *index < points.len()) {
                    points[node] = current;
                    if source.len() > 2
                        && source
                            .first()
                            .zip(source.last())
                            .is_some_and(|(a, b)| a.distance(*b) < 0.01)
                    {
                        if node == 0 {
                            let last = points.len() - 1;
                            points[last] = current;
                        }
                        if node + 1 == points.len() {
                            points[0] = current;
                        }
                    }
                }
            }
            PolylineEditCommand::Move => {
                for point in &mut points {
                    point.x += delta.x;
                    point.y += delta.y;
                }
            }
            PolylineEditCommand::Rotate => {
                let pivot = edit
                    .node
                    .and_then(|index| source.get(index).copied())
                    .or_else(|| {
                        edit.edge.and_then(|index| {
                            source
                                .get(index)
                                .zip(source.get(index + 1))
                                .map(|(a, b)| Point::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5))
                        })
                    })
                    .unwrap_or(source[0]);
                let a0 = (start.y - pivot.y).atan2(start.x - pivot.x);
                let a1 = (current.y - pivot.y).atan2(current.x - pivot.x);
                let angle = a1 - a0;
                for point in &mut points {
                    let (x, y) = (point.x - pivot.x, point.y - pivot.y);
                    *point = Point::new(
                        pivot.x + x * angle.cos() - y * angle.sin(),
                        pivot.y + x * angle.sin() + y * angle.cos(),
                    );
                }
            }
            PolylineEditCommand::Mirror => {
                let axis = Point::new(current.x - start.x, current.y - start.y);
                let length = axis.x.hypot(axis.y);
                if length > 1e-5 {
                    let ux = axis.x / length;
                    let uy = axis.y / length;
                    for point in &mut points {
                        let x = point.x - start.x;
                        let y = point.y - start.y;
                        let projection = x * ux + y * uy;
                        *point = Point::new(
                            start.x + 2. * projection * ux - x,
                            start.y + 2. * projection * uy - y,
                        );
                    }
                    for arc in arcs.iter_mut().flatten() {
                        arc.clockwise = !arc.clockwise;
                    }
                }
            }
            PolylineEditCommand::OffsetPolyline => {
                let edge = Self::polyline_context_edge(edit, source);
                if let Some(index) = edge.filter(|index| *index + 1 < source.len()) {
                    let a = source[index];
                    let b = source[index + 1];
                    let length = a.distance(b);
                    if length > 1e-5 {
                        let normal = source_arcs
                            .get(index)
                            .and_then(|arc| *arc)
                            .and_then(|arc| {
                                let (center, _) = geometry::arc_center_sweep(a, b, arc)?;
                                let radial = Point::new(start.x - center.x, start.y - center.y);
                                let len = radial.x.hypot(radial.y);
                                (len > 1e-5).then(|| {
                                    let sign = if arc.clockwise { 1. } else { -1. };
                                    Point::new(sign * radial.x / len, sign * radial.y / len)
                                })
                            })
                            .unwrap_or(Point::new(-(b.y - a.y) / length, (b.x - a.x) / length));
                        let distance = delta.x * normal.x + delta.y * normal.y;
                        if let Some((offset, offset_arcs)) =
                            geometry::offset_polyline_with_arcs(source, source_arcs, distance)
                        {
                            points = offset;
                            arcs = offset_arcs;
                        } else if distance.abs() > 1e-6 {
                            if let Some(active) = self.polyline_edit.as_mut() {
                                active.error = "Offset cannot preserve a connected valid path at this distance".into();
                            }
                            return;
                        }
                    }
                }
            }
            PolylineEditCommand::CurveEdge => {
                if let Some(index) = Self::polyline_context_edge(edit, source)
                    .filter(|index| *index + 1 < source.len())
                    && let Some(arc) =
                        geometry::arc_segment_through(source[index], source[index + 1], current)
                {
                    arcs[index] = Some(arc);
                }
            }
            PolylineEditCommand::Multiply => {
                for index in 1..=edit.multiply_count.clamp(1, 50) {
                    let mut copy = original.clone();
                    copy.id = self.editor.project.next_id;
                    copy.name = format!("Polyline {:03}", copy.id);
                    self.editor.project.next_id += 1;
                    if let Shape::Polyline { points, .. } = &mut copy.shape {
                        for point in points {
                            point.x += delta.x * index as f32;
                            point.y += delta.y * index as f32;
                        }
                    }
                    self.editor.project.elements.push(copy);
                }
            }
            PolylineEditCommand::Chamfer | PolylineEditCommand::Fillet => return,
        }
        if let Some(slot) = self
            .editor
            .project
            .elements
            .iter_mut()
            .find(|element| element.id == edit.element_id)
        {
            slot.shape = Shape::Polyline { points, arcs };
        }
    }
    pub(super) fn polyline_context_edge(edit: &PolylineEdit, points: &[Point]) -> Option<usize> {
        edit.edge
            .filter(|index| *index + 1 < points.len())
            .or_else(|| {
                let node = edit.node?;
                if node + 1 < points.len() {
                    Some(node)
                } else {
                    node.checked_sub(1)
                        .filter(|index| *index + 1 < points.len())
                }
            })
    }
    pub(super) fn apply_polyline_corner_edit(&mut self) {
        let Some(edit) = self.polyline_edit.as_mut() else {
            return;
        };
        let Some(node) = edit.node else {
            edit.error = "Choose a corner node first".into();
            return;
        };
        let before = self.editor.project.clone();
        let Some(element) = self
            .editor
            .project
            .elements
            .iter_mut()
            .find(|element| element.id == edit.element_id)
        else {
            edit.error = "Polyline no longer exists".into();
            return;
        };
        let Shape::Polyline { points, arcs } = &mut element.shape else {
            edit.error = "Selected element is not a Polyline".into();
            return;
        };
        if node == 0 || node + 1 >= points.len() {
            edit.error = "Chamfer and Fillet need an interior node".into();
            return;
        }
        let previous = points[node - 1];
        let corner = points[node];
        let next = points[node + 1];
        let left = corner.distance(previous);
        let right = corner.distance(next);
        let operation = edit.command;
        let (ax, ay) = (previous.x - corner.x, previous.y - corner.y);
        let (bx, by) = (next.x - corner.x, next.y - corner.y);
        let la = ax.hypot(ay);
        let lb = bx.hypot(by);
        let angle = ((ax * bx + ay * by) / (la * lb).max(1e-8))
            .clamp(-1., 1.)
            .acos();
        if angle < 0.01 || (std::f32::consts::PI - angle).abs() < 0.01 {
            edit.error = "Corner edit needs two non-collinear segments".into();
            return;
        }
        let distance = if operation == PolylineEditCommand::Chamfer {
            edit.chamfer_length
        } else {
            edit.fillet_radius * (angle * 0.5).tan()
        };
        if !distance.is_finite()
            || distance <= 0.
            || distance >= left - 0.01
            || distance >= right - 0.01
        {
            edit.error = "Value must be positive and leave 10 mm on both adjacent segments".into();
            return;
        }
        let p1 = Point::new(corner.x + ax / la * distance, corner.y + ay / la * distance);
        let p2 = Point::new(corner.x + bx / lb * distance, corner.y + by / lb * distance);
        let fillet_arc = if operation == PolylineEditCommand::Chamfer {
            None
        } else {
            let center_distance = edit.fillet_radius / (angle * 0.5).sin();
            let bisector = Point::new(ax / la + bx / lb, ay / la + by / lb);
            let bisector_len = bisector.x.hypot(bisector.y);
            let center = Point::new(
                corner.x + bisector.x / bisector_len * center_distance,
                corner.y + bisector.y / bisector_len * center_distance,
            );
            let start_angle = (p1.y - center.y).atan2(p1.x - center.x);
            let end_angle = (p2.y - center.y).atan2(p2.x - center.x);
            let cross =
                (p1.x - center.x) * (p2.y - center.y) - (p1.y - center.y) * (p2.x - center.x);
            let mut sweep = end_angle - start_angle;
            if cross > 0. && sweep < 0. {
                sweep += std::f32::consts::TAU;
            }
            if cross < 0. && sweep > 0. {
                sweep -= std::f32::consts::TAU;
            }
            let middle_angle = start_angle + sweep * 0.5;
            let through = Point::new(
                center.x + edit.fillet_radius * middle_angle.cos(),
                center.y + edit.fillet_radius * middle_angle.sin(),
            );
            geometry::arc_segment_through(p1, p2, through)
        };
        if arcs.is_empty() {
            *arcs = vec![None; points.len() - 1];
        }
        points.splice(node..=node, [p1, p2]);
        let start = node - 1;
        arcs.splice(start..=node, [None, fillet_arc, None]);
        if let Err(error) = self.editor.project.validate() {
            self.editor.project = before;
            if let Some(edit) = self.polyline_edit.as_mut() {
                edit.error = error;
            }
            return;
        }
        self.editor.commit(before);
        self.editor.status = if operation == PolylineEditCommand::Chamfer {
            "Polyline corner chamfered"
        } else {
            "Polyline corner filleted"
        }
        .into();
        self.polyline_edit = None;
    }
    pub(super) fn base_target(&self, p: Point, shift: bool) -> (Point, SnapKind) {
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
                .map(|start| vec![(start, cursor)])
                .unwrap_or_default(),
            WallGeometryMethod::Rectangular => self
                .editor
                .anchor
                .map(|start| rectangle_wall_segments(start, cursor))
                .unwrap_or_default(),
            WallGeometryMethod::Continuous | WallGeometryMethod::Polygonal => {
                let points = &self.editor.wall_points;
                let mut segments: Vec<_> =
                    points.windows(2).map(|pair| (pair[0], pair[1])).collect();
                let Some(last) = points.last().copied() else {
                    return segments;
                };
                if method == WallGeometryMethod::Polygonal
                    && points.len() >= 3
                    && points[0].distance(cursor) <= close_tolerance
                {
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
                e.story == self.editor.story && self.editor.project.plan_options.element_visible(e)
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
                let preview_arc = self
                    .polyline_edit
                    .as_ref()
                    .filter(|edit| {
                        edit.element_id == e.id
                            && edit.command == PolylineEditCommand::CurveEdge
                            && edit.drag.is_some()
                    })
                    .and_then(|edit| {
                        let index = Self::polyline_context_edge(edit, points)?;
                        let current = self.world(r, pointer?);
                        Some((
                            index,
                            geometry::arc_segment_through(
                                points[index],
                                points[index + 1],
                                current,
                            ),
                        ))
                    });
                let mut live_arcs = arcs.clone();
                if let Some((index, arc)) = preview_arc {
                    live_arcs.resize(points.len() - 1, None);
                    live_arcs[index] = arc;
                }
                let arcs = &live_arcs;
                let tessellated = geometry::polyline_path(points, arcs);
                let path: Vec<Pos2> = tessellated
                    .iter()
                    .map(|point| self.screen(r, *point))
                    .collect();
                let color = if selected {
                    Color32::from_rgb(0, 220, 70)
                } else {
                    Self::plan_color(style.line)
                };
                p.add(egui::Shape::line(
                    path,
                    Stroke::new(
                        if selected {
                            2.5
                        } else {
                            self.plan_pen_px(style.line_width_mm)
                        },
                        color,
                    ),
                ));
                if let Some(edit) = self.polyline_edit.as_ref().filter(|edit| {
                    selected
                        && edit.element_id == e.id
                        && edit.command == PolylineEditCommand::CurveEdge
                        && edit.drag.is_some()
                }) {
                    let radius = edit
                        .edge
                        .and_then(|index| arcs.get(index).and_then(|arc| *arc))
                        .map(|arc| arc.radius);
                    if let (Some(radius), Some(pointer)) = (radius, pointer) {
                        p.text(
                            pointer + vec2(14., -18.),
                            Align2::LEFT_BOTTOM,
                            format!("R = {:.3} m", radius),
                            FontId::monospace(11.),
                            INK,
                        );
                    }
                }
                if selected {
                    for point in points {
                        let node = self.screen(r, *point);
                        p.circle_filled(node, 3.5, Color32::BLACK);
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
        let walls: Vec<_> = self
            .editor
            .project
            .elements
            .iter()
            .filter(|e| {
                e.story == self.editor.story
                    && matches!(e.shape, Shape::Wall { .. })
                    && self.editor.project.plan_options.element_visible(e)
            })
            .collect();
        let mut joined_wall_footprints = Vec::new();
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
                let footprints = geometry::plan_wall_footprints(&same_material);
                for (wall, footprint) in same_material.iter().zip(&footprints) {
                    joined_wall_footprints.push((wall.id, *footprint));
                    if !selected_ids.contains(&wall.id) {
                        let pts: Vec<_> = footprint
                            .iter()
                            .map(|point| self.screen(r, *point))
                            .collect();
                        let wall_fill = self.trace_active_fill(
                            style
                                .fill
                                .map_or(Self::plan_color(style.line), Self::plan_color),
                        );
                        p.add(egui::Shape::convex_polygon(
                            pts,
                            wall_fill,
                            // Slight same-colour bleed seals sub-pixel gaps where
                            // separately editable wall footprints meet.
                            Stroke::new(0.8, wall_fill),
                        ));
                    }
                }
                let wall_stroke = Stroke::new(
                    self.plan_pen_px(style.line_width_mm),
                    Self::plan_color(style.line),
                );
                for mut path in geometry::wall_outline_paths(&footprints) {
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
        }
        // Selected walls retain an individual highlight and editable handles.
        let selected_walls: Vec<_> = walls
            .iter()
            .copied()
            .filter(|wall| selected_ids.contains(&wall.id))
            .collect();
        for wall in &selected_walls {
            let joined = joined_wall_footprints
                .iter()
                .find(|(id, _)| *id == wall.id)
                .map(|(_, footprint)| *footprint)
                .unwrap_or_else(|| geometry::footprint(wall));
            let pts: Vec<_> = joined.into_iter().map(|v| self.screen(r, v)).collect();
            p.add(egui::Shape::convex_polygon(
                pts,
                WALL_SELECTION_FILL,
                Stroke::new(1.5, WALL_SELECTION_OUTLINE),
            ));
        }
        // Paint every baseline after the fills so adjacently selected walls
        // cannot hide each other's reference lines at a junction.
        for wall in selected_walls {
            if let Some((start, end)) = geometry::wall_reference_line(wall) {
                let a = self.screen(r, start);
                let b = self.screen(r, end);
                paint_selected_wall_baseline(&p, a, b);
                let mid = a.lerp(b, 0.5);
                p.text(
                    mid + vec2(0., -14.),
                    Align2::CENTER_CENTER,
                    self.editor
                        .project
                        .plan_options
                        .dimension_style
                        .format_length(start.distance(end)),
                    FontId::monospace(11.),
                    WALL_SELECTION_OUTLINE,
                );
            }
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
                    && self.editor.project.plan_options.element_visible(element)
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
            p.add(egui::Shape::convex_polygon(
                points.clone(),
                hole_fill,
                Stroke::NONE,
            ));
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
                    let center_a = Pos2::new(
                        (points[0].x + points[3].x) * 0.5,
                        (points[0].y + points[3].y) * 0.5,
                    );
                    let center_b = Pos2::new(
                        (points[1].x + points[2].x) * 0.5,
                        (points[1].y + points[2].y) * 0.5,
                    );
                    p.line_segment([center_a, center_b], stroke);
                }
            }
            if selected {
                p.add(egui::Shape::closed_line(points, Stroke::new(2.2, ACCENT)));
            }
        }
        p.set_opacity(1.);
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
                    && self.editor.project.plan_options.element_visible(e)
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
        if self.polyline_edit.is_none() && self.guide_canvas(ui, r, &response) {
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
                            Shape::Slab { .. } => Layer::Structural,
                            Shape::Polyline { .. } => Layer::Drafting,
                            Shape::Wall { .. } => self.editor.wall_layer,
                            _ => Layer::Architectural,
                        },
                        renovation: self.editor.wall_renovation,
                        design_option: None,
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
            if response.clicked() && !wall_double && !ui.input(|i| i.key_down(Key::Space)) {
                if self.orientation_pick_active {
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
                    let context = self.selected_polyline_hit(raw, 9. / self.plan_scale);
                    if let Some((id, node, edge)) = context {
                        self.drag = None;
                        self.open_polyline_edit(id, node, edge, pos);
                    } else if ui.input(|i| i.modifiers.shift) {
                        self.polyline_edit = None;
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
                        self.polyline_edit = None;
                        let hit = geometry::hit_plan(
                            &self.editor.project,
                            self.editor.story,
                            raw,
                            6. / self.plan_scale,
                        );
                        self.editor.set_selection(hit.into_iter().collect());
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
                        self.editor.click(q, 6. / self.plan_scale);
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
        if self.editor.tool == Tool::Select && !pan && self.polyline_edit.is_some() {
            if response.drag_started_by(PointerButton::Primary)
                && let Some(pos) = ui.input(|i| i.pointer.press_origin())
            {
                let start = self.world(r, pos);
                let before = self.editor.project.clone();
                if let Some(edit) = self.polyline_edit.as_mut() {
                    edit.drag = Some((before, start));
                }
            }
            if response.dragged_by(PointerButton::Primary)
                && let (Some(pos), Some(edit)) = (pointer, self.polyline_edit.clone())
                && let Some((before, start)) = edit.drag.as_ref()
            {
                let current = self.world(r, pos);
                self.apply_polyline_drag_preview(before, &edit, *start, current);
                ui.ctx().request_repaint();
            }
            if response.drag_stopped()
                && let Some(edit) = self.polyline_edit.as_mut()
                && let Some((before, _)) = edit.drag.take()
            {
                if self.editor.project.validate().is_ok() && self.editor.project != before {
                    if edit.command == PolylineEditCommand::Multiply {
                        let new_ids: Vec<u64> = self
                            .editor
                            .project
                            .elements
                            .iter()
                            .filter(|element| element.id >= before.next_id)
                            .map(|element| element.id)
                            .collect();
                        self.editor.commit(before);
                        self.editor.set_selection(new_ids.clone());
                        self.editor.status = format!("Created {} Polyline copies", new_ids.len());
                    } else {
                        self.editor.commit(before);
                        self.editor.status = match edit.command {
                            PolylineEditCommand::InsertNode => "Polyline node inserted",
                            PolylineEditCommand::MoveNode => "Polyline node moved",
                            PolylineEditCommand::OffsetPolyline => "Polyline offset",
                            PolylineEditCommand::CurveEdge => "Polyline edge curved",
                            PolylineEditCommand::Move => "Polyline moved",
                            PolylineEditCommand::Rotate => "Polyline rotated",
                            PolylineEditCommand::Mirror => "Polyline mirrored",
                            _ => "Polyline edited",
                        }
                        .into();
                    }
                    self.polyline_edit = None;
                } else {
                    self.editor.project = before;
                    edit.error = "Edit produced no valid geometry change".into();
                }
            }
        } else if self.editor.tool == Tool::Select && !pan {
            if response.drag_started_by(PointerButton::Primary)
                && let Some(pos) = ui.input(|i| i.pointer.press_origin())
            {
                let w = self.world(r, pos);
                let id = self
                    .selected_polyline_hit(w, 9. / self.plan_scale)
                    .map(|hit| hit.0)
                    .or_else(|| {
                        geometry::hit_plan(
                            &self.editor.project,
                            self.editor.story,
                            w,
                            6. / self.plan_scale,
                        )
                    });
                if let Some(id) = id {
                    let mut ids = self.editor.selection_ids();
                    if !ids.contains(&id) {
                        ids = vec![id];
                        self.editor.set_selection(ids.clone());
                    }
                    self.drag = Some((self.editor.project.clone(), w, ids));
                }
            }
            if response.dragged_by(PointerButton::Primary)
                && let (Some((before, start, ids)), Some(pos)) = (&self.drag, pointer)
            {
                let w = self.world(r, pos);
                let mut delta = Point::new(w.x - start.x, w.y - start.y);
                if self.editor.snap {
                    delta.x = (delta.x * 10.).round() / 10.;
                    delta.y = (delta.y * 10.).round() / 10.;
                }
                self.editor.project = before.clone();
                self.editor.project.translate_many(ids, delta);
                self.draft_id = None;
            }
        }
        if response.drag_stopped()
            && let Some((before, _, _)) = self.drag.take()
        {
            self.editor.commit(before);
            self.editor.status = "Element moved".into();
        }
        if hovered {
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
                .polyline_edit
                .as_ref()
                .is_some_and(|edit| edit.command == PolylineEditCommand::MoveNode)
            {
                self.select_cursor_feedback(self.world(r, pointer.unwrap_or(r.center())))
            } else if self.drag.as_ref().is_some_and(|(_, _, ids)| {
                ids.iter().any(|id| {
                    self.editor.project.elements.iter().any(|element| {
                        element.id == *id && matches!(element.shape, Shape::Polyline { .. })
                    })
                })
            }) {
                CursorFeedback::Arrow
            } else if self.drag.is_some()
                || self
                    .polyline_edit
                    .as_ref()
                    .is_some_and(|edit| edit.drag.is_some())
            {
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
        if let Some(edit) = self.polyline_edit.as_ref() {
            let active = edit
                .node
                .and_then(|_| {
                    self.editor
                        .project
                        .elements
                        .iter()
                        .find(|element| element.id == edit.element_id)
                })
                .and_then(|element| match &element.shape {
                    Shape::Polyline { points, .. } => points.get(edit.node?).copied(),
                    _ => None,
                });
            if let Some(point) = active {
                p.circle_filled(self.screen(r, point), 5., ACCENT);
            }
        }
        if trace_pointer {
            self.trace_cursor(ui);
        }
        self.polyline_edit_palette(ui.ctx());
    }
    pub(super) fn polyline_edit_palette(&mut self, ctx: &Context) {
        let language = self.language;
        let Some(edit) = self.polyline_edit.as_mut() else {
            return;
        };
        if edit.drag.is_some() {
            return;
        }
        let at = edit.palette_at;
        let is_node = edit.node.is_some();
        let mut apply_corner = false;
        Area::new(Id::new("polyline_edit_palette"))
            .order(Order::Foreground)
            .fixed_pos(at)
            .show(ctx, |ui| {
                Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(190.);
                    ui.strong(language.translate(if is_node {
                        "Polyline node"
                    } else {
                        "Polyline edge"
                    }));
                    ui.separator();
                    let commands: &[(PolylineEditCommand, &str)] = if is_node {
                        &[
                            (
                                PolylineEditCommand::MoveNode,
                                language.translate("Move node"),
                            ),
                            (PolylineEditCommand::Chamfer, language.translate("Chamfer…")),
                            (PolylineEditCommand::Fillet, language.translate("Fillet…")),
                            (
                                PolylineEditCommand::CurveEdge,
                                language.translate("Curve edge"),
                            ),
                            (
                                PolylineEditCommand::OffsetPolyline,
                                language.translate("Offset Polyline"),
                            ),
                            (PolylineEditCommand::Move, language.translate("Move")),
                            (PolylineEditCommand::Rotate, language.translate("Rotate")),
                            (PolylineEditCommand::Mirror, language.translate("Mirror")),
                            (
                                PolylineEditCommand::Multiply,
                                language.translate("Multiply"),
                            ),
                        ]
                    } else {
                        &[
                            (
                                PolylineEditCommand::InsertNode,
                                language.translate("Insert node"),
                            ),
                            (
                                PolylineEditCommand::OffsetPolyline,
                                language.translate("Offset Polyline"),
                            ),
                            (
                                PolylineEditCommand::CurveEdge,
                                language.translate("Curve edge"),
                            ),
                            (PolylineEditCommand::Move, language.translate("Move")),
                            (PolylineEditCommand::Rotate, language.translate("Rotate")),
                            (PolylineEditCommand::Mirror, language.translate("Mirror")),
                            (
                                PolylineEditCommand::Multiply,
                                language.translate("Multiply"),
                            ),
                        ]
                    };
                    for (command, label) in commands {
                        if ui
                            .selectable_label(edit.command == *command, *label)
                            .clicked()
                        {
                            edit.command = *command;
                            edit.error.clear();
                        }
                    }
                    if matches!(
                        edit.command,
                        PolylineEditCommand::Chamfer | PolylineEditCommand::Fillet
                    ) {
                        ui.separator();
                        if edit.command == PolylineEditCommand::Chamfer {
                            ui.label(language.translate("Cut length (m)"));
                            ui.add(
                                DragValue::new(&mut edit.chamfer_length)
                                    .speed(0.01)
                                    .range(0.001..=1000.),
                            );
                        } else {
                            ui.label(language.translate("Radius (m)"));
                            ui.add(
                                DragValue::new(&mut edit.fillet_radius)
                                    .speed(0.01)
                                    .range(0.001..=1000.),
                            );
                        }
                        if ui.button(language.translate("Apply corner edit")).clicked() {
                            apply_corner = true;
                        }
                    } else if edit.command == PolylineEditCommand::Multiply {
                        ui.separator();
                        ui.label(language.translate("Number of copies"));
                        ui.add(DragValue::new(&mut edit.multiply_count).range(1..=50));
                        ui.label(language.translate("Drag to place repeated copies"));
                    } else {
                        ui.separator();
                        ui.label(match edit.command {
                            PolylineEditCommand::InsertNode => {
                                "Drag on the selected edge to place a node"
                            }
                            PolylineEditCommand::MoveNode => {
                                language.translate("Drag the selected node to edit it")
                            }
                            PolylineEditCommand::OffsetPolyline => language
                                .translate("Drag inward or outward to offset the whole Polyline"),
                            PolylineEditCommand::CurveEdge => {
                                language.translate("Drag the segment to define a circular arc")
                            }
                            PolylineEditCommand::Move => {
                                language.translate("Drag to move the Polyline")
                            }
                            PolylineEditCommand::Rotate => {
                                language.translate("Drag around the pivot to rotate")
                            }
                            PolylineEditCommand::Mirror => {
                                language.translate("Drag to define the mirror axis")
                            }
                            PolylineEditCommand::Multiply => {
                                language.translate("Drag to place repeated copies")
                            }
                            _ => language.translate("Choose a corner command"),
                        });
                    }
                    if !edit.error.is_empty() {
                        ui.colored_label(Color32::from_rgb(240, 125, 110), &edit.error);
                    }
                });
            });
        if apply_corner {
            self.apply_polyline_corner_edit();
        }
    }
    pub(super) fn apply_polyline_edit_cursor(&self, ctx: &Context) {
        if self
            .polyline_edit
            .as_ref()
            .is_some_and(|edit| edit.command == PolylineEditCommand::MoveNode)
        {
            let cursor = ctx.input(|input| {
                if input.pointer.button_down(PointerButton::Middle)
                    || input.pointer.button_down(PointerButton::Secondary)
                    || (input.pointer.button_down(PointerButton::Primary)
                        && input.key_down(Key::Space))
                {
                    CursorIcon::Grabbing
                } else {
                    CursorIcon::Default
                }
            });
            ctx.set_cursor_icon(cursor);
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
                        ui.label(if angular {
                            if wall { "Length" } else { "Distance" }
                        } else {
                            "Δ X"
                        });
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
                        ui.label(if angular { "Angle" } else { "Δ Y" });
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
