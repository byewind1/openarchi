use super::*;
use crate::editing::EditCommand;
use crate::model::WallContour;
use crate::reshape::{self, ReshapeCommand};

// Native AC29 observation and the shared mesh adapter cover the top reference plane.
pub(super) const SLAB_CUSTOM_EDGE_ANGLE_VERIFIED: bool = true;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WallPetCommand {
    InsertNode,
    Curve,
    Tangent,
    OuterSide,
    TrapezoidNode,
    Drag,
    Rotate,
    Mirror,
    Elevate,
    Multiply,
    MoveNode,
    OffsetPolyline,
    Chamfer,
    Fillet,
    OffsetEdge,
    OffsetAll,
    AddPolygon,
    SubtractPolygon,
    CustomEdge,
    InsertArcNode,
}
impl WallPetCommand {
    pub(super) const ALL: [Self; 10] = [
        Self::InsertNode,
        Self::Curve,
        Self::Tangent,
        Self::OuterSide,
        Self::TrapezoidNode,
        Self::Drag,
        Self::Rotate,
        Self::Mirror,
        Self::Elevate,
        Self::Multiply,
    ];
    const POLYLINE_EDGE: [Self; 8] = [
        Self::InsertNode,
        Self::Curve,
        Self::Tangent,
        Self::OffsetPolyline,
        Self::Drag,
        Self::Rotate,
        Self::Mirror,
        Self::Multiply,
    ];
    const POLYLINE_NODE: [Self; 10] = [
        Self::MoveNode,
        Self::Chamfer,
        Self::Fillet,
        Self::Curve,
        Self::Tangent,
        Self::OffsetPolyline,
        Self::Drag,
        Self::Rotate,
        Self::Mirror,
        Self::Multiply,
    ];
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::InsertNode => "Insert node",
            Self::Curve => "Curve edge",
            Self::Tangent => "Edit segment using tangent",
            Self::OuterSide => "Change outer side of trapezoid wall",
            Self::TrapezoidNode => "Insert new point in trapezoid wall",
            Self::Drag => "Drag",
            Self::Rotate => "Rotate",
            Self::Mirror => "Mirror",
            Self::Elevate => "Elevate",
            Self::Multiply => "Multiply…",
            Self::MoveNode => "Move node",
            Self::OffsetPolyline => "Offset Polyline",
            Self::Chamfer => "Chamfer…",
            Self::Fillet => "Fillet…",
            Self::OffsetEdge => "Offset edge",
            Self::OffsetAll => "Offset all edges",
            Self::AddPolygon => "Add to polygon",
            Self::SubtractPolygon => "Subtract from polygon",
            Self::CustomEdge => "Custom slab edge…",
            Self::InsertArcNode => "Insert node on arc",
        }
    }
}
pub(super) struct WallInsert {
    id: u64,
    ids: Vec<u64>,
    junction: Option<Vec<u64>>,
    endpoint: Option<usize>,
    polyline: Option<(Option<usize>, Option<usize>)>,
    slab_edge: Option<usize>,
    slab_node: Option<usize>,
    slab_arc: bool,
    polygon_ready: bool,
    edge_angle: f32,
    edge_surface: Option<Material>,
    edge_scope: crate::slab_geometry::SlabEdgeScope,
    revision: u64,
    anchor: Point,
    split: f32,
    point: Point,
    elevation: Option<f32>,
    palette_at: Pos2,
    numeric: bool,
    pub(super) confirm: bool,
    pub(super) canvas_drag: bool,
    tracker_at: Option<Pos2>,
    tracker_rect: Option<Rect>,
    tracker_focused: bool,
    pub(super) command: WallPetCommand,
    stages: Vec<Point>,
    copies: usize,
    height: f32,
    corner: f32,
}
impl WallInsert {
    pub(super) fn cycle(&mut self, backwards: bool) {
        let commands = self.commands();
        let i = commands.iter().position(|c| *c == self.command).unwrap();
        self.switch(
            commands[(i + if backwards { commands.len() - 1 } else { 1 }) % commands.len()],
        );
    }
    fn commands(&self) -> &'static [WallPetCommand] {
        if self.slab_edge.is_some() {
            if self.slab_node.is_some() {
                return &[
                    WallPetCommand::MoveNode,
                    WallPetCommand::Fillet,
                    WallPetCommand::Chamfer,
                    WallPetCommand::OffsetAll,
                    WallPetCommand::AddPolygon,
                    WallPetCommand::SubtractPolygon,
                    WallPetCommand::Drag,
                    WallPetCommand::Rotate,
                    WallPetCommand::Mirror,
                    WallPetCommand::Elevate,
                    WallPetCommand::Multiply,
                ];
            }
            if self.slab_arc {
                return &[
                    WallPetCommand::InsertNode,
                    WallPetCommand::Curve,
                    WallPetCommand::Tangent,
                    WallPetCommand::InsertArcNode,
                    WallPetCommand::OffsetEdge,
                    WallPetCommand::OffsetAll,
                    WallPetCommand::AddPolygon,
                    WallPetCommand::SubtractPolygon,
                    WallPetCommand::CustomEdge,
                    WallPetCommand::Drag,
                    WallPetCommand::Rotate,
                    WallPetCommand::Mirror,
                    WallPetCommand::Elevate,
                    WallPetCommand::Multiply,
                ];
            }
            return &[
                WallPetCommand::InsertNode,
                WallPetCommand::Curve,
                WallPetCommand::Tangent,
                WallPetCommand::OffsetEdge,
                WallPetCommand::OffsetAll,
                WallPetCommand::AddPolygon,
                WallPetCommand::SubtractPolygon,
                WallPetCommand::CustomEdge,
                WallPetCommand::Drag,
                WallPetCommand::Rotate,
                WallPetCommand::Mirror,
                WallPetCommand::Elevate,
                WallPetCommand::Multiply,
            ];
        }
        match self.polyline {
            Some((Some(_), _)) => &WallPetCommand::POLYLINE_NODE,
            Some(_) => &WallPetCommand::POLYLINE_EDGE,
            None => &WallPetCommand::ALL,
        }
    }
    fn switch(&mut self, command: WallPetCommand) {
        self.command = command;
        self.stages.clear();
        self.polygon_ready = false;
        self.numeric = false;
        self.tracker_focused = false;
        self.confirm = false;
    }
    fn needs_point(&self) -> bool {
        match self.command {
            WallPetCommand::OuterSide | WallPetCommand::Mirror => self.stages.is_empty(),
            WallPetCommand::Rotate => self.stages.len() < 2,
            _ => false,
        }
    }
    fn label(&self) -> &'static str {
        if (self.junction.is_some() || self.endpoint.is_some())
            && self.command == WallPetCommand::InsertNode
        {
            "Move node"
        } else {
            self.command.label()
        }
    }
    fn operation(&self, project: &Project) -> Result<Option<ReshapeCommand>, String> {
        if self.command == WallPetCommand::InsertNode
            && let Some(ids) = &self.junction
        {
            return Ok(Some(ReshapeCommand::MoveWallJunction {
                ids: ids.clone(),
                origin: self.anchor,
                point: self.point,
            }));
        }
        if self.needs_point() {
            return Ok(None);
        }
        if let Some(index) = self.slab_edge {
            use crate::slab_geometry::SlabChange as C;
            let mut polygon = self.stages.clone();
            if !self.polygon_ready
                && polygon
                    .last()
                    .is_none_or(|p| p.distance(self.point) > 0.001)
            {
                polygon.push(self.point);
            }
            let change = match self.command {
                WallPetCommand::InsertNode => Some(C::InsertNode),
                WallPetCommand::InsertArcNode => Some(C::InsertArcNode),
                WallPetCommand::MoveNode => Some(C::MoveNode),
                WallPetCommand::Curve => Some(C::Curve),
                WallPetCommand::Tangent => Some(C::Tangent),
                WallPetCommand::OffsetEdge => Some(C::OffsetEdge),
                WallPetCommand::OffsetAll => Some(C::OffsetAll),
                WallPetCommand::AddPolygon if polygon.len() >= 3 => Some(C::AddPolygon(polygon)),
                WallPetCommand::SubtractPolygon if polygon.len() >= 3 => {
                    Some(C::SubtractPolygon(polygon))
                }
                WallPetCommand::AddPolygon | WallPetCommand::SubtractPolygon => return Ok(None),
                WallPetCommand::CustomEdge => Some(C::SetEdge {
                    angle_degrees: self.edge_angle,
                    surface: self.edge_surface,
                    scope: self.edge_scope,
                }),
                WallPetCommand::Elevate => Some(C::Elevate(self.height)),
                WallPetCommand::Fillet => Some(C::Fillet(self.corner)),
                WallPetCommand::Chamfer => Some(C::Chamfer(self.corner)),
                _ => None,
            };
            if let Some(change) = change {
                return Ok(Some(ReshapeCommand::SlabEdit {
                    id: self.id,
                    index: self.slab_node.unwrap_or(index),
                    change,
                    anchor: self.anchor,
                    point: self.point,
                }));
            }
        }
        if let Some((node, edge)) = self.polyline {
            use crate::polyline_edit::PolylineChange as C;
            let change = match self.command {
                WallPetCommand::MoveNode => Some(C::MoveNode),
                WallPetCommand::InsertNode => Some(C::InsertNode),
                WallPetCommand::Curve => Some(C::CurveEdge),
                WallPetCommand::Tangent => Some(C::Tangent),
                WallPetCommand::OffsetPolyline => Some(C::OffsetPolyline),
                WallPetCommand::Chamfer => Some(C::Chamfer(self.corner)),
                WallPetCommand::Fillet => Some(C::Fillet(self.corner)),
                _ => None,
            };
            if let Some(change) = change {
                return Ok(Some(ReshapeCommand::Polyline {
                    id: self.id,
                    node,
                    edge,
                    change,
                    anchor: self.anchor,
                    point: self.point,
                }));
            }
        }
        let wall = project
            .elements
            .iter()
            .find(|e| e.id == self.id)
            .ok_or("Element not found")?;
        let (a, b) = if let Shape::Polyline { points, .. } = &wall.shape {
            (points[0], *points.last().unwrap())
        } else if let Some(edge) = self.slab_edge {
            let mut remaining = edge;
            let rings = crate::slab_geometry::rings(wall);
            let ring = rings
                .iter()
                .find(|ring| {
                    if remaining < ring.points.len() {
                        true
                    } else {
                        remaining -= ring.points.len();
                        false
                    }
                })
                .ok_or("Invalid edit parameters")?;
            (
                ring.points[remaining],
                ring.points[(remaining + 1) % ring.points.len()],
            )
        } else {
            geometry::wall_reference_line(wall).ok_or("Select a wall")?
        };
        let delta = Point::new(self.point.x - self.anchor.x, self.point.y - self.anchor.y);
        let transform = |offset, origin, angle, mirror, copies| ReshapeCommand::SelectionEdit {
            ids: self.ids.clone(),
            command: EditCommand::Transform {
                offset,
                origin,
                angle,
                scale: 1.,
                mirror,
                copies,
            },
        };
        Ok(Some(match self.command {
            WallPetCommand::InsertNode => ReshapeCommand::InsertWallNode {
                id: self.id,
                point: self.point,
                split: self.split,
            },
            WallPetCommand::TrapezoidNode => ReshapeCommand::WallTrapezoidNode {
                id: self.id,
                point: self.point,
            },
            WallPetCommand::Curve => ReshapeCommand::WallContour {
                id: self.id,
                contour: geometry::arc_segment_through(a, b, self.point)
                    .map(|arc| WallContour::Arc { arc }),
            },
            WallPetCommand::Tangent => ReshapeCommand::WallContour {
                id: self.id,
                contour: crate::polyline_edit::tangent_arc(a, b, self.point, self.split > 0.5)?
                    .map(|arc| WallContour::Arc { arc }),
            },
            WallPetCommand::OuterSide => {
                if matches!(wall.wall_contour, Some(WallContour::Arc { .. })) {
                    return Err("Trapezoid editing requires a straight reference line".into());
                }
                let n = Point::new(-(b.y - a.y), b.x - a.x);
                let c = self.stages[0];
                let d = self.point;
                let intersection = |v: Point| -> Result<Point, String> {
                    let t = reshape::intersect(v, Point::new(v.x + n.x, v.y + n.y), c, d)
                        .ok_or("Outer side must cross both wall ends")?
                        .0;
                    Ok(Point::new(v.x + n.x * t, v.y + n.y * t))
                };
                let positive = match wall.wall_contour {
                    Some(WallContour::Trapezoid { outer_positive, .. }) => outer_positive,
                    _ => true,
                };
                ReshapeCommand::WallContour {
                    id: self.id,
                    contour: Some(WallContour::Trapezoid {
                        outer_start: intersection(a)?,
                        outer_end: intersection(b)?,
                        outer_nodes: vec![],
                        outer_positive: positive,
                    }),
                }
            }
            WallPetCommand::Drag => transform(delta, self.anchor, 0., false, 0),
            WallPetCommand::Multiply => transform(delta, self.anchor, 0., false, self.copies),
            WallPetCommand::Rotate => {
                let origin = self.stages[0];
                let reference = self.stages[1];
                if origin.distance(reference) < 0.001 || origin.distance(self.point) < 0.001 {
                    return Err("Rotation radius must be positive".into());
                }
                let angle = ((self.point.y - origin.y).atan2(self.point.x - origin.x)
                    - (reference.y - origin.y).atan2(reference.x - origin.x))
                .to_degrees();
                transform(Point::new(0., 0.), origin, angle, false, 0)
            }
            WallPetCommand::Mirror => {
                let origin = self.stages[0];
                if origin.distance(self.point) < 0.001 {
                    return Err("Mirror axis must have length".into());
                }
                let angle = (self.point.y - origin.y)
                    .atan2(self.point.x - origin.x)
                    .to_degrees();
                transform(Point::new(0., 0.), origin, 2. * angle - 180., true, 0)
            }
            WallPetCommand::MoveNode
            | WallPetCommand::OffsetPolyline
            | WallPetCommand::Chamfer
            | WallPetCommand::Fillet
            | WallPetCommand::OffsetEdge
            | WallPetCommand::OffsetAll
            | WallPetCommand::AddPolygon
            | WallPetCommand::SubtractPolygon
            | WallPetCommand::CustomEdge => return Err("Invalid edit parameters".into()),
            WallPetCommand::InsertArcNode => return Err("Invalid edit parameters".into()),
            WallPetCommand::Elevate => ReshapeCommand::SelectionEdit {
                ids: self.ids.clone(),
                command: EditCommand::Elevate(self.height),
            },
        }))
    }
}
impl AtelierApp {
    pub(super) fn start_polyline_pet(
        &mut self,
        id: u64,
        node: Option<usize>,
        edge: Option<usize>,
        at: Pos2,
        point: Point,
        dragging: bool,
    ) {
        // Reuse the wall session, with only the object-specific geometry context changed.
        let point = node
            .and_then(|i| {
                self.editor
                    .project
                    .elements
                    .iter()
                    .find(|e| e.id == id)
                    .and_then(|e| {
                        if let Shape::Polyline { points, .. } = &e.shape {
                            points.get(i).copied()
                        } else if matches!(e.shape, Shape::Slab { .. }) {
                            crate::slab_geometry::rings(e)
                                .iter()
                                .flat_map(|r| r.points.iter())
                                .nth(i)
                                .copied()
                        } else {
                            None
                        }
                    })
            })
            .unwrap_or(point);
        let slab = self
            .editor
            .project
            .elements
            .iter()
            .any(|e| e.id == id && matches!(e.shape, Shape::Slab { .. }));
        if slab && dragging && node.is_none() {
            self.start_direct_pointer_drag(point);
            return;
        }
        self.open_wall_edit(id, None, at);
        self.start_wall_pet(WallPetCommand::Drag);

        self.wall_edit = None;
        let slab = self
            .editor
            .project
            .elements
            .iter()
            .any(|e| e.id == id && matches!(e.shape, Shape::Slab { .. }));
        if let Some(edit) = &mut self.wall_insert {
            if slab {
                edit.slab_edge = edge.or(node);
                edit.slab_node = node;
                if let Some(element) = self.editor.project.elements.iter().find(|e| e.id == id) {
                    let contour = crate::slab_geometry::contour(element);
                    edit.elevation =
                        Some(self.editor.project.elevation(element.story) + contour.elevation);
                    let mut remaining = edge.or(node).unwrap_or(0);
                    for ring in crate::slab_geometry::rings(element) {
                        if remaining < ring.points.len() {
                            edit.slab_arc = ring.arcs.get(remaining).is_some_and(Option::is_some);
                            if let Some(settings) = ring.edge_settings.get(remaining) {
                                edit.edge_angle = settings.angle_degrees;
                                edit.edge_surface = settings.surface;
                            }
                            break;
                        }
                        remaining -= ring.points.len();
                    }
                }
            } else {
                edit.polyline = Some((node, edge));
            }
            edit.anchor = point;
            edit.point = point;
            edit.canvas_drag = dragging;
            edit.switch(if node.is_some() {
                WallPetCommand::MoveNode
            } else {
                WallPetCommand::InsertNode
            });
            self.editor.status = edit.label().into();
        }
    }

    pub(super) fn set_wall_edit_hit(&mut self, point: Point, elevation: Option<f32>) {
        if let Some(edit) = self.wall_edit.as_mut() {
            if let Some(wall) = self
                .editor
                .project
                .elements
                .iter()
                .find(|e| e.id == edit.element_id)
            {
                let (station, _) = crate::wall_geometry::project_station(wall, point);
                if let Some((point, _)) = crate::wall_geometry::point_tangent_at(wall, station) {
                    edit.insert_anchor = point;
                }
            }
            edit.elevation = elevation;
        }
    }
    pub(super) fn start_wall_insert(&mut self) {
        self.start_wall_pet(WallPetCommand::InsertNode);
    }
    pub(super) fn start_wall_pet(&mut self, command: WallPetCommand) {
        let Some(edit) = self.wall_edit.take() else {
            return;
        };
        let Some(wall) = self
            .editor
            .project
            .elements
            .iter()
            .find(|e| e.id == edit.element_id)
        else {
            return;
        };
        let anchor = edit
            .node
            .and_then(|node| {
                geometry::wall_reference_line(wall).map(|(a, b)| if node == 0 { a } else { b })
            })
            .unwrap_or(edit.insert_anchor);
        let split = if matches!(wall.shape, Shape::Wall { .. }) {
            crate::wall_geometry::project_station(wall, edit.insert_anchor).0
                / crate::wall_geometry::reference_length(wall)
        } else {
            0.5
        };
        self.wall_insert = Some(WallInsert {
            id: edit.element_id,
            ids: self.editor.selection_ids(),
            junction: None,
            endpoint: edit.node,
            polyline: None,
            slab_edge: None,
            slab_node: None,
            slab_arc: false,
            polygon_ready: false,
            edge_angle: 90.,
            edge_surface: None,
            edge_scope: crate::slab_geometry::SlabEdgeScope::Clicked,
            revision: self.editor.revision,
            anchor,
            split,
            point: anchor,
            elevation: edit.elevation,
            palette_at: edit.palette_at,
            numeric: false,
            confirm: false,
            canvas_drag: false,
            tracker_at: None,
            tracker_rect: None,
            tracker_focused: false,
            command,
            stages: vec![],
            copies: 3,
            height: 0.,
            corner: 0.25,
        });
        self.wall_edit_palette_rect = None;
        self.editor.status = self.wall_insert.as_ref().unwrap().label().into();
    }
    pub(super) fn start_wall_junction(
        &mut self,
        origin: Point,
        ids: Vec<u64>,
        at: Pos2,
        elevation: Option<f32>,
        dragging: bool,
    ) {
        self.open_wall_edit(ids[0], None, at);
        self.set_wall_edit_hit(origin, elevation);
        self.start_wall_insert();
        if let Some(edit) = &mut self.wall_insert {
            edit.junction = Some(ids);
            edit.anchor = origin;
            edit.point = origin;
            edit.canvas_drag = dragging;
        }
        self.editor.status = "Move node".into();
    }
    pub(super) fn selected_wall_junction_plan(
        &self,
        point: Point,
        tolerance: f32,
    ) -> Option<(Point, Vec<u64>)> {
        reshape::wall_junctions(
            &self.editor.project,
            &self.editor.selection_ids(),
            self.editor.story,
        )
        .into_iter()
        .filter(|(q, _)| q.distance(point) <= tolerance)
        .min_by(|(a, _), (b, _)| a.distance(point).total_cmp(&b.distance(point)))
    }
    pub(super) fn selected_wall_junction_model(
        &self,
        r: Rect,
        pos: Pos2,
    ) -> Option<(Point, Vec<u64>, f32)> {
        let mut best: Option<(f32, Point, Vec<u64>, f32)> = None;
        let mut stories: Vec<_> = self
            .editor
            .project
            .elements
            .iter()
            .filter(|e| {
                self.editor.selection_ids().contains(&e.id)
                    && (self.all_stories || e.story == self.editor.story)
            })
            .map(|e| e.story)
            .collect();
        stories.sort_unstable();
        stories.dedup();
        for story in stories {
            for (q, ids) in
                reshape::wall_junctions(&self.editor.project, &self.editor.selection_ids(), story)
            {
                let mut elevations: Vec<_> = self
                    .editor
                    .project
                    .elements
                    .iter()
                    .filter(|e| ids.contains(&e.id))
                    .flat_map(|wall| {
                        let c = geometry::corners(&self.editor.project, wall);
                        [c[0].z, c[4].z]
                    })
                    .collect();
                elevations.sort_by(f32::total_cmp);
                elevations.dedup_by(|a, b| (*a - *b).abs() < 0.0001);
                for z in elevations {
                    let sharing: Vec<_> = ids
                        .iter()
                        .copied()
                        .filter(|id| {
                            let wall = self
                                .editor
                                .project
                                .elements
                                .iter()
                                .find(|e| e.id == *id)
                                .unwrap();
                            let c = geometry::corners(&self.editor.project, wall);
                            (c[0].z - z).abs() < 0.0001 || (c[4].z - z).abs() < 0.0001
                        })
                        .collect();
                    if sharing.len() < 2 {
                        continue;
                    }
                    if let Some(screen) = self.wall_project_point(r, q, z) {
                        let distance = screen.distance(pos);
                        if distance <= 10. && best.as_ref().is_none_or(|(d, _, _, _)| distance < *d)
                        {
                            best = Some((distance, q, sharing, z));
                        }
                    }
                }
            }
        }
        best.map(|(_, q, ids, z)| (q, ids, z))
    }
    pub(super) fn wall_project_point(&self, r: Rect, point: Point, elevation: f32) -> Option<Pos2> {
        let clip =
            self.camera.matrix(r.aspect_ratio()) * glam::Vec4::new(point.x, point.y, elevation, 1.);
        if !clip.is_finite() || clip.w <= 0. {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        (ndc.z >= 0. && ndc.z <= 1.).then(|| {
            pos2(
                r.left() + (ndc.x + 1.) * r.width() * 0.5,
                r.top() + (1. - ndc.y) * r.height() * 0.5,
            )
        })
    }
    pub(super) fn selected_wall_model_edge(
        &self,
        r: Rect,
        pos: Pos2,
    ) -> Option<(u64, Option<usize>, Point, f32)> {
        let ids = self.editor.selection_ids();
        let mut best = None;
        for wall in self.editor.project.elements.iter().filter(|e| {
            ids.contains(&e.id)
                && (self.all_stories || e.story == self.editor.story)
                && self
                    .editor
                    .project
                    .plan_options
                    .element_visible(e, &self.editor.project.layers)
        }) {
            let Shape::Wall { start, end, .. } = wall.shape else {
                continue;
            };
            let corners = geometry::corners(&self.editor.project, wall);
            let path = crate::wall_geometry::reference_path(wall);
            for z in [corners[0].z, corners[4].z] {
                let node = if self
                    .wall_project_point(r, start, z)
                    .is_some_and(|a| pos.distance(a) < 9.)
                {
                    Some(0)
                } else if self
                    .wall_project_point(r, end, z)
                    .is_some_and(|b| pos.distance(b) < 9.)
                {
                    Some(1)
                } else {
                    None
                };
                for segment in path.windows(2) {
                    let (Some(a), Some(b)) = (
                        self.wall_project_point(r, segment[0], z),
                        self.wall_project_point(r, segment[1], z),
                    ) else {
                        continue;
                    };
                    let (distance, t) = reshape::distance_segment(
                        Point::new(pos.x, pos.y),
                        Point::new(a.x, a.y),
                        Point::new(b.x, b.y),
                    );
                    if distance > 10. || best.as_ref().is_some_and(|(d, _, _, _, _)| *d <= distance)
                    {
                        continue;
                    }
                    let point =
                        Self::screen_to_ground(&self.camera, r, pos, z).unwrap_or(Point::new(
                            segment[0].x + (segment[1].x - segment[0].x) * t,
                            segment[0].y + (segment[1].y - segment[0].y) * t,
                        ));
                    best = Some((distance, wall.id, node, point, z));
                }
            }
        }
        best.map(|(_, id, node, point, z)| (id, node, point, z))
    }

    pub(super) fn wall_pet_buttons(
        &self,
        ctx: &Context,
        at: Pos2,
        active: Option<WallPetCommand>,
        junction: bool,
    ) -> (Rect, Option<WallPetCommand>, Option<WallPetCommand>) {
        self.edit_pet_buttons(ctx, at, active, junction, &WallPetCommand::ALL)
    }
    fn edit_pet_buttons(
        &self,
        ctx: &Context,
        at: Pos2,
        active: Option<WallPetCommand>,
        junction: bool,
        commands: &[WallPetCommand],
    ) -> (Rect, Option<WallPetCommand>, Option<WallPetCommand>) {
        let mut chosen = None;
        let mut hovered = None;
        let mut sizing = false;
        let response = Area::new(Id::new("wall_edit_palette"))
            .order(Order::Foreground)
            .fixed_pos(at)
            .show(ctx, |ui| {
                sizing = ui.is_sizing_pass();
                Frame::popup(ui.style()).inner_margin(4.).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(2., 2.);
                    ui.spacing_mut().interact_size.y = 26.;
                    let slab = commands.iter().any(|c| {
                        matches!(
                            c,
                            WallPetCommand::OffsetEdge
                                | WallPetCommand::OffsetAll
                                | WallPetCommand::CustomEdge
                        )
                    });
                    let rows: Vec<&[WallPetCommand]> = if slab && commands.len() > 5 {
                        vec![
                            &commands[..commands.len() - 5],
                            &commands[commands.len() - 5..],
                        ]
                    } else {
                        commands.chunks(5).collect()
                    };
                    for row in rows {
                        ui.horizontal(|ui| {
                            for command in row {
                                let (rect, response) =
                                    ui.allocate_exact_size(vec2(26., 26.), Sense::click());
                                let label = if junction && *command == WallPetCommand::InsertNode {
                                    "Move node"
                                } else {
                                    command.label()
                                };
                                response.widget_info(|| {
                                    WidgetInfo::labeled(
                                        WidgetType::Button,
                                        true,
                                        self.language.translate(label),
                                    )
                                });
                                let response =
                                    response.on_hover_text(self.language.translate(label));
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(Id::new(("wall_pet", *command as u8)), rect)
                                });
                                if *command == WallPetCommand::InsertNode {
                                    ui.ctx().data_mut(|d| {
                                        d.insert_temp(Id::new("wall_insert_button"), rect)
                                    });
                                }
                                if !sizing
                                    && ui
                                        .input(|i| i.pointer.hover_pos())
                                        .is_some_and(|p| rect.contains(p))
                                {
                                    hovered = Some(*command);
                                }
                                if active == Some(*command) || hovered == Some(*command) {
                                    ui.painter().rect_filled(
                                        rect,
                                        3.,
                                        if active == Some(*command) {
                                            Color32::from_gray(24)
                                        } else {
                                            Color32::from_gray(42)
                                        },
                                    );
                                }
                                if active == Some(*command) || hovered == Some(*command) {
                                    ui.painter().rect_stroke(
                                        rect.shrink(0.5),
                                        3.,
                                        Stroke::new(
                                            1.,
                                            Color32::from_gray(if active == Some(*command) {
                                                170
                                            } else {
                                                100
                                            }),
                                        ),
                                        StrokeKind::Inside,
                                    );
                                }
                                paint_pet_icon(
                                    ui.painter(),
                                    rect,
                                    *command,
                                    ui.visuals().text_color(),
                                );
                                if response.clicked() {
                                    chosen = Some(*command)
                                }
                            }
                        });
                    }
                });
            });
        (
            if sizing {
                Rect::NOTHING
            } else {
                response.response.rect
            },
            chosen,
            hovered,
        )
    }
    pub(super) fn wall_insert_canvas(
        &mut self,
        ui: &mut Ui,
        r: Rect,
        response: &Response,
        model: bool,
    ) -> bool {
        let Some(mut edit) = self.wall_insert.take() else {
            return false;
        };
        if edit.revision != self.editor.revision || self.editor.tool != Tool::Select {
            self.wall_edit_palette_rect = None;
            return true;
        }
        let polygon_mode = edit.slab_edge.is_some()
            && matches!(
                edit.command,
                WallPetCommand::AddPolygon | WallPetCommand::SubtractPolygon
            );
        let pointer = ui.input(|i| i.pointer.hover_pos());
        let over_palette = egui::Popup::is_any_open(ui.ctx())
            || pointer.is_some_and(|p| {
                self.wall_edit_palette_rect.is_some_and(|r| r.contains(p))
                    || (!polygon_mode
                        && !edit.canvas_drag
                        && edit.tracker_rect.is_some_and(|r| r.contains(p)))
            });
        let near_tracker =
            pointer.is_some_and(|p| edit.tracker_rect.is_some_and(|r| r.expand(12.).contains(p)));
        if ui.input(|i| i.pointer.button_pressed(PointerButton::Primary)) {
            edit.canvas_drag = pointer.is_some_and(|p| r.contains(p)) && !over_palette;
        }
        let (palette, clicked, _) = self.edit_pet_buttons(
            ui.ctx(),
            edit.palette_at,
            Some(edit.command),
            edit.junction.is_some() || edit.endpoint.is_some(),
            edit.commands(),
        );
        // Hover is presentation only. An icon's completed left click is the
        // sole pointer transition that activates another editing command.
        let chosen = clicked;
        if chosen == Some(WallPetCommand::InsertArcNode) && edit.slab_arc {
            // This native command inserts at the original hit point immediately.
            // Palette movement must never become the geometric insertion point.
            edit.command = WallPetCommand::InsertArcNode;
            edit.point = edit.anchor;
            match edit
                .operation(&self.editor.project)
                .and_then(|op| op.ok_or_else(|| "Invalid edit parameters".into()))
                .and_then(|op| self.editor.reshape(op))
            {
                Ok(()) => {
                    self.wall_edit_palette_rect = None;
                    self.editor.status = "Slab edited".into();
                }
                Err(error) => {
                    self.editor.status = error;
                    self.wall_insert = Some(edit);
                }
            }
            return true;
        }
        if let Some(command) = chosen
            && command != edit.command
        {
            edit.switch(command);
            self.editor.status = edit.label().into();
            ui.ctx().request_repaint();
        }
        if edit.command == WallPetCommand::InsertNode
            && let Some(node) = edit.endpoint
        {
            // Returning from any shared command restores the original endpoint
            // state, including its established magnetic reference-line snap.
            self.open_wall_edit(edit.id, Some(node), edit.palette_at - vec2(14., 12.));
            if let Some(endpoint) = &mut self.wall_edit {
                endpoint.insert_anchor = edit.anchor;
                endpoint.elevation = edit.elevation;
            }
            self.wall_edit_palette_rect = Some(palette);
            return true;
        }
        if (response.hovered() || edit.canvas_drag)
            && !over_palette
            && !edit.numeric
            && !edit.tracker_focused
            && let Some(pos) = pointer
        {
            let q = if model {
                Self::screen_to_ground(&self.camera, r, pos, edit.elevation.unwrap_or(0.))
            } else {
                Some(self.world(r, pos))
            };
            if let Some(q) = q {
                edit.point = if model {
                    q
                } else {
                    self.target(q, ui.input(|i| i.modifiers.shift)).0
                };
            }
        }
        let operation = edit.operation(&self.editor.project);
        let preview = operation.as_ref().map_err(Clone::clone).and_then(|op| {
            op.as_ref()
                .map(|c| reshape::candidate(&self.editor.project, c))
                .transpose()
        });
        let project_point = |p: Point, z: f32| {
            if model {
                self.wall_project_point(r, p, z)
            } else {
                Some(self.screen(r, p))
            }
        };
        let painter = ui.painter_at(r);
        if let Ok(Some((project, ids))) = &preview {
            let straight_walls: Vec<_> = project
                .elements
                .iter()
                .filter(|wall| {
                    wall.story == self.editor.story
                        && matches!(wall.shape, Shape::Wall { .. })
                        && wall.wall_contour.is_none()
                })
                .collect();
            let groups = project.layers.wall_groups(&straight_walls);
            let joined: std::collections::HashMap<_, _> =
                geometry::plan_wall_polygons_by_id(&straight_walls, &groups)
                    .into_iter()
                    .collect();
            for element in project.elements.iter().filter(|e| ids.contains(&e.id)) {
                if matches!(element.shape, Shape::Slab { .. }) {
                    let elevation = project.elevation(element.story)
                        + crate::slab_geometry::contour(element).elevation;
                    for outline in crate::slab_geometry::outlines(element) {
                        painter.add(egui::Shape::closed_line(
                            outline
                                .into_iter()
                                .filter_map(|p| project_point(p, elevation))
                                .collect(),
                            Stroke::new(2., ACCENT),
                        ));
                    }
                    if model && let Ok(bottom) = crate::slab_geometry::bottom_contour(element) {
                        let bottom_elevation = elevation - element.thickness();
                        let top = crate::slab_geometry::contour(element);
                        for (top, bottom) in std::iter::once(&top)
                            .chain(top.holes.iter())
                            .zip(std::iter::once(&bottom).chain(bottom.holes.iter()))
                        {
                            painter.add(egui::Shape::closed_line(
                                crate::slab_geometry::ring_path(bottom)
                                    .into_iter()
                                    .filter_map(|p| project_point(p, bottom_elevation))
                                    .collect(),
                                Stroke::new(1., ACCENT),
                            ));
                            for (top, bottom) in top.points.iter().zip(&bottom.points) {
                                if let (Some(a), Some(b)) = (
                                    project_point(*top, elevation),
                                    project_point(*bottom, bottom_elevation),
                                ) {
                                    painter.line_segment([a, b], Stroke::new(1., ACCENT));
                                }
                            }
                        }
                    }
                    continue;
                }
                if let Shape::Polyline { points, arcs } = &element.shape {
                    painter.add(egui::Shape::line(
                        geometry::polyline_path(points, arcs)
                            .into_iter()
                            .filter_map(|p| project_point(p, 0.))
                            .collect(),
                        Stroke::new(2., ACCENT),
                    ));
                    for p in points {
                        if let Some(p) = project_point(*p, 0.) {
                            painter.rect_filled(
                                Rect::from_center_size(p, vec2(5., 5.)),
                                0.,
                                ACCENT,
                            );
                        }
                    }
                }
            }
            for wall in project
                .elements
                .iter()
                .filter(|e| ids.contains(&e.id) && matches!(e.shape, Shape::Wall { .. }))
            {
                let c = geometry::corners(project, wall);
                for z in if model {
                    vec![c[0].z, c[4].z]
                } else {
                    vec![c[0].z]
                } {
                    let outline = joined
                        .get(&wall.id)
                        .cloned()
                        .unwrap_or_else(|| crate::wall_geometry::outline(wall));
                    let points: Vec<_> = outline
                        .iter()
                        .filter_map(|p| project_point(*p, z))
                        .collect();
                    if points.len() == outline.len() {
                        painter.add(egui::Shape::closed_line(points, Stroke::new(1.5, ACCENT)));
                    }
                    let points: Vec<_> = crate::wall_geometry::reference_path(wall)
                        .into_iter()
                        .filter_map(|p| project_point(p, z))
                        .collect();
                    painter.add(egui::Shape::line(points, Stroke::new(2., ACCENT)));
                }
            }
        }
        let polygon_mode = edit.slab_edge.is_some()
            && matches!(
                edit.command,
                WallPetCommand::AddPolygon | WallPetCommand::SubtractPolygon
            );
        if polygon_mode && !edit.stages.is_empty() {
            let mut path = edit.stages.clone();
            path.push(edit.point);
            painter.add(egui::Shape::line(
                path.into_iter()
                    .filter_map(|p| project_point(p, edit.elevation.unwrap_or(0.)))
                    .collect(),
                Stroke::new(1.5, ACCENT),
            ));
        }
        for origin in edit
            .stages
            .last()
            .copied()
            .into_iter()
            .chain(std::iter::once(edit.anchor))
        {
            if let (Some(a), Some(b)) = (
                project_point(origin, edit.elevation.unwrap_or(0.)),
                project_point(edit.point, edit.elevation.unwrap_or(0.)),
            ) {
                painter.line_segment([a, b], Stroke::new(1., ACCENT));
            }
        }
        let preview_error = preview
            .as_ref()
            .err()
            .map(|e| localized_error(self.language, e));
        let origin = edit.stages.first().copied().unwrap_or(edit.anchor);
        let mut distance = origin.distance(edit.point);
        let mut angle = (edit.point.y - origin.y)
            .atan2(edit.point.x - origin.x)
            .to_degrees();
        let mut changed = false;
        let mut focused = false;
        let size = vec2(
            if edit.command == WallPetCommand::CustomEdge {
                240.
            } else {
                164.
            },
            if edit.command == WallPetCommand::CustomEdge {
                250.
            } else if edit.command == WallPetCommand::Multiply {
                76.
            } else {
                54.
            },
        );
        if edit.tracker_at.is_none()
            || (edit.canvas_drag && !over_palette)
            || (!near_tracker && !edit.tracker_focused && !edit.numeric && !over_palette)
        {
            let pos = pointer.unwrap_or(edit.palette_at);
            let mut at = pos + vec2(22., 24.);
            if Rect::from_min_size(at, size).intersects(palette.expand(5.)) {
                // Stay beside the pointer when the palette occupies its lower-right side.
                at.y = pos.y - size.y - 18.;
                if at.y < r.top() {
                    at.y = palette.bottom() + 8.;
                }
            }
            at.x = at.x.clamp(r.left(), (r.right() - size.x).max(r.left()));
            at.y = at.y.clamp(r.top(), (r.bottom() - size.y).max(r.top()));
            edit.tracker_at = Some(at);
        }
        let mut tracker_sizing = false;
        let mut cancel_custom = false;
        let tracker = Area::new(Id::new("wall_pet_tracker"))
            .order(Order::Foreground)
            .interactable(!polygon_mode)
            .fixed_pos(edit.tracker_at.unwrap())
            .show(ui.ctx(), |ui| {
                tracker_sizing = ui.is_sizing_pass();
                Frame::popup(ui.style())
                    .inner_margin(5.)
                    .corner_radius(3.)
                    .show(ui, |ui| {
                        // A held canvas drag owns the gesture, even when crossing the tracker.
                        // Field editing can begin after releasing that gesture.
                        if edit.canvas_drag {
                            ui.disable();
                        }
                        ui.set_width(150.);
                        ui.style_mut().override_font_id = Some(FontId::proportional(12.));
                        ui.spacing_mut().item_spacing = vec2(5., 2.);
                        ui.spacing_mut().button_padding = vec2(3., 1.);
                        ui.spacing_mut().interact_size.y = 19.;
                        if edit.slab_edge.is_some() && edit.command == WallPetCommand::CustomEdge {
                            ui.set_width(230.);
                            ui.label(self.language.translate("Custom slab edge…"));
                            ui.horizontal(|ui| {
                                ui.label(self.language.translate("Edge angle (degrees)"));
                                let response = ui.add_enabled(
                                    SLAB_CUSTOM_EDGE_ANGLE_VERIFIED,
                                    DragValue::new(&mut edit.edge_angle)
                                        .speed(1.)
                                        .range(1.0..=179.0),
                                );
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(Id::new("slab_edge_angle"), response.rect)
                                });
                                changed |= response.changed();
                                focused |= response.has_focus();
                            });
                            ui.label(self.language.translate("Edge surface"));
                            let surface = egui::ComboBox::from_id_salt("slab_edge_surface")
                                .selected_text(
                                    self.language.translate(
                                        edit.edge_surface
                                            .map_or("Use building material", Material::name),
                                    ),
                                )
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut edit.edge_surface,
                                        None,
                                        self.language.translate("Use building material"),
                                    );
                                    for material in Material::ALL {
                                        let response = ui.selectable_value(
                                            &mut edit.edge_surface,
                                            Some(material),
                                            self.language.translate(material.name()),
                                        );
                                        ui.ctx().data_mut(|data| {
                                            data.insert_temp(
                                                Id::new(("slab_edge_material", material as u8)),
                                                response.rect,
                                            )
                                        });
                                    }
                                });
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(Id::new("slab_edge_surface"), surface.response.rect)
                            });
                            ui.label(self.language.translate("Apply to"));
                            for (scope, label) in [
                                (crate::slab_geometry::SlabEdgeScope::Clicked, "Clicked edge"),
                                (
                                    crate::slab_geometry::SlabEdgeScope::Ring,
                                    "Selected polygon",
                                ),
                                (crate::slab_geometry::SlabEdgeScope::All, "All edges"),
                            ] {
                                let response = ui.radio_value(
                                    &mut edit.edge_scope,
                                    scope,
                                    self.language.translate(label),
                                );
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(
                                        Id::new(("slab_edge_scope", scope as u8)),
                                        response.rect,
                                    )
                                });
                            }
                            let apply = ui.button(self.language.translate("Apply"));
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(Id::new("slab_edge_apply"), apply.rect)
                            });
                            if apply.clicked() {
                                edit.confirm = true;
                            }
                            let cancel = ui.button(self.language.translate("Cancel"));
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(Id::new("slab_edge_cancel"), cancel.rect)
                            });
                            if cancel.clicked() {
                                cancel_custom = true;
                            }
                        } else if !polygon_mode {
                            Grid::new("wall_pet_numeric")
                                .num_columns(2)
                                .spacing(vec2(5., 2.))
                                .show(ui, |ui| {
                                    if matches!(
                                        edit.command,
                                        WallPetCommand::Chamfer | WallPetCommand::Fillet
                                    ) {
                                        ui.label(self.language.translate(
                                            if edit.command == WallPetCommand::Fillet {
                                                "Radius (m)"
                                            } else {
                                                "Cut length (m)"
                                            },
                                        ));
                                        let value = ui.add(
                                            DragValue::new(&mut edit.corner)
                                                .speed(0.01)
                                                .max_decimals(3),
                                        );
                                        changed |= value.changed();
                                        focused |=
                                            value.has_focus() || value.is_pointer_button_down_on();
                                        ui.end_row();
                                    } else if edit.command == WallPetCommand::Elevate {
                                        ui.label(self.language.translate("Elevation offset"));
                                        let value = ui.add(
                                            DragValue::new(&mut edit.height)
                                                .speed(0.05)
                                                .suffix(" m")
                                                .max_decimals(3),
                                        );
                                        changed |= value.changed();
                                        focused |=
                                            value.has_focus() || value.is_pointer_button_down_on();
                                        ui.ctx().data_mut(|d| {
                                            d.insert_temp(Id::new("wall_pet_distance"), value.rect)
                                        });
                                        ui.end_row();
                                    } else {
                                        ui.label(self.language.translate("Distance"));
                                        let value = ui.add(
                                            DragValue::new(&mut distance)
                                                .range(0.0..=10000.)
                                                .speed(0.01)
                                                .suffix(" m")
                                                .max_decimals(3),
                                        );
                                        changed |= value.changed();
                                        focused |=
                                            value.has_focus() || value.is_pointer_button_down_on();
                                        ui.ctx().data_mut(|d| {
                                            d.insert_temp(Id::new("wall_pet_distance"), value.rect)
                                        });
                                        ui.end_row();
                                        ui.label(self.language.translate("Angle"));
                                        let value = ui.add(
                                            DragValue::new(&mut angle)
                                                .speed(1.)
                                                .suffix("°")
                                                .max_decimals(2),
                                        );
                                        changed |= value.changed();
                                        focused |=
                                            value.has_focus() || value.is_pointer_button_down_on();
                                        ui.ctx().data_mut(|d| {
                                            d.insert_temp(Id::new("wall_pet_angle"), value.rect)
                                        });
                                        ui.end_row();
                                    }
                                    if edit.command == WallPetCommand::Multiply {
                                        ui.label(self.language.translate("Copies"));
                                        let value =
                                            ui.add(DragValue::new(&mut edit.copies).range(1..=100));
                                        changed |= value.changed();
                                        focused |=
                                            value.has_focus() || value.is_pointer_button_down_on();
                                        ui.end_row();
                                    }
                                });
                        }
                        if polygon_mode {
                            ui.label(format!(
                                "{} {}",
                                self.language.translate("Polygon vertices"),
                                edit.stages.len()
                            ));
                            ui.label(
                                self.language
                                    .translate("Click first point or press Enter to finish"),
                            );
                        }
                        if matches!(
                            edit.command,
                            WallPetCommand::Curve | WallPetCommand::Tangent
                        ) && let (Some((_, edge)), Ok(Some((project, _)))) =
                            (edit.polyline, &preview)
                            && let Some(element) = project.elements.iter().find(|e| e.id == edit.id)
                            && let Shape::Polyline { arcs, .. } = &element.shape
                            && let Some(arc) = edge
                                .or_else(|| {
                                    edit.polyline
                                        .and_then(|(node, _)| node.map(|i| i.saturating_sub(1)))
                                })
                                .and_then(|i| arcs.get(i))
                                .and_then(|a| *a)
                        {
                            ui.label(format!(
                                "{} {:.3}",
                                self.language.translate("Radius (m)"),
                                arc.radius
                            ));
                        }
                        if matches!(
                            edit.command,
                            WallPetCommand::Curve | WallPetCommand::Tangent
                        ) && let (Some(index), Ok(Some((project, _)))) =
                            (edit.slab_edge, &preview)
                            && let Some(element) = project.elements.iter().find(|e| e.id == edit.id)
                        {
                            let mut remaining = index;
                            for ring in crate::slab_geometry::rings(element) {
                                if remaining < ring.points.len() {
                                    if let Some(Some(arc)) = ring.arcs.get(remaining) {
                                        ui.label(format!(
                                            "{} {:.3}",
                                            self.language.translate("Radius (m)"),
                                            arc.radius
                                        ));
                                    }
                                    break;
                                }
                                remaining -= ring.points.len();
                            }
                        }
                        if let Some(error) = &preview_error {
                            ui.colored_label(Color32::from_rgb(240, 125, 110), error);
                        }
                    });
            });
        let tracker_rect = if tracker_sizing {
            Rect::NOTHING
        } else {
            tracker.response.rect
        };
        edit.tracker_rect = Some(tracker_rect);
        edit.tracker_focused = focused;
        ui.ctx()
            .data_mut(|d| d.insert_temp(Id::new("wall_pet_tracker_rect"), tracker.response.rect));
        let over_controls =
            |p: Pos2| palette.contains(p) || (!polygon_mode && tracker_rect.contains(p));
        let released = ui.input(|i| i.pointer.button_released(PointerButton::Primary));
        let keyboard_confirm = edit.confirm;
        let custom_mode = edit.slab_edge.is_some() && edit.command == WallPetCommand::CustomEdge;
        let confirm = edit.confirm
            || (!custom_mode
                && released
                && edit.canvas_drag
                && pointer.is_some_and(|p| r.contains(p) && !over_controls(p))
                && clicked.is_none());
        if released {
            edit.canvas_drag = false;
        }
        edit.confirm = false;
        if cancel_custom {
            self.wall_edit_palette_rect = None;
            self.editor.status = "Edit cancelled".into();
            return true;
        }
        if changed {
            edit.numeric = true;
            if !matches!(
                edit.command,
                WallPetCommand::Elevate
                    | WallPetCommand::Chamfer
                    | WallPetCommand::Fillet
                    | WallPetCommand::CustomEdge
            ) {
                edit.point = Point::new(
                    origin.x + distance * angle.to_radians().cos(),
                    origin.y + distance * angle.to_radians().sin(),
                );
            }
        }
        let cursor = pointer
            .filter(|pos| r.contains(*pos) && !over_controls(*pos))
            .map(|pos| {
                let on_original = self
                    .editor
                    .project
                    .elements
                    .iter()
                    .filter(|wall| {
                        edit.junction
                            .as_ref()
                            .map_or(wall.id == edit.id, |ids| ids.contains(&wall.id))
                    })
                    .any(|wall| {
                        let paths = if let Shape::Polyline { points, arcs } = &wall.shape {
                            vec![geometry::polyline_path(points, arcs)]
                        } else if matches!(wall.shape, Shape::Slab { .. }) {
                            crate::slab_geometry::outlines(wall)
                                .into_iter()
                                .map(|mut p| {
                                    if let Some(first) = p.first().copied() {
                                        p.push(first);
                                    }
                                    p
                                })
                                .collect()
                        } else {
                            vec![crate::wall_geometry::reference_path(wall)]
                        };
                        paths.into_iter().any(|path| {
                            let points: Vec<_> = path
                                .into_iter()
                                .filter_map(|point| {
                                    project_point(point, edit.elevation.unwrap_or(0.))
                                })
                                .collect();
                            points.windows(2).any(|pair| {
                                reshape::distance_segment(
                                    Point::new(pos.x, pos.y),
                                    Point::new(pair[0].x, pair[0].y),
                                    Point::new(pair[1].x, pair[1].y),
                                )
                                .0 <= 4.
                            })
                        })
                    });
                let feedback = if on_original {
                    CursorFeedback::SolidPencil
                } else {
                    CursorFeedback::HollowPencil
                };
                feedback.paint(&painter, pos);
                ui.ctx().set_cursor_icon(feedback.icon());
                feedback
            });
        ui.ctx()
            .data_mut(|data| data.insert_temp(Id::new("wall_pet_cursor"), cursor));
        ui.ctx()
            .data_mut(|data| data.insert_temp(Id::new("wall_pet_active"), edit.command));
        self.wall_edit_palette_rect = Some(palette);
        if confirm {
            if polygon_mode && !edit.polygon_ready {
                let close = edit.stages.len() >= 3
                    && edit.stages[0].distance(edit.point) <= 8. / self.plan_scale;
                let double = ui.input(|i| i.pointer.button_double_clicked(PointerButton::Primary));
                if close
                    || (keyboard_confirm && edit.stages.len() >= 3)
                    || (double && edit.stages.len() >= 2)
                {
                    if double
                        && !close
                        && edit
                            .stages
                            .last()
                            .is_none_or(|p| p.distance(edit.point) > 0.001)
                    {
                        edit.stages.push(edit.point);
                    }
                    edit.polygon_ready = true;
                } else {
                    if edit
                        .stages
                        .last()
                        .is_none_or(|p| p.distance(edit.point) > 0.001)
                    {
                        edit.stages.push(edit.point);
                    }
                    edit.numeric = false;
                    edit.tracker_focused = false;
                    self.wall_insert = Some(edit);
                    return true;
                }
            }
            if edit.needs_point() {
                edit.stages.push(edit.point);
                edit.numeric = false;
                edit.tracker_focused = false;
                self.editor.status = edit.label().into();
                self.wall_insert = Some(edit);
            } else {
                match edit
                    .operation(&self.editor.project)
                    .and_then(|op| op.ok_or_else(|| "Invalid edit parameters".into()))
                    .and_then(|op| self.editor.reshape(op))
                {
                    Ok(()) => {
                        if edit.junction.is_some() {
                            let mut selected = edit.ids.clone();
                            for id in self.editor.selection_ids() {
                                if !selected.contains(&id) {
                                    selected.push(id);
                                }
                            }
                            self.editor.set_selection(selected);
                        }
                        self.wall_edit_palette_rect = None;
                        self.editor.status = if edit.slab_edge.is_some() {
                            "Slab edited"
                        } else if edit.polyline.is_some() {
                            "Polyline edited"
                        } else {
                            "Wall edit applied"
                        }
                        .into();
                    }
                    Err(error) => {
                        self.editor.status = error;
                        self.wall_insert = Some(edit);
                    }
                }
            }
        } else {
            self.wall_insert = Some(edit);
        }
        true
    }
}
fn paint_pet_icon(p: &Painter, r: Rect, command: WallPetCommand, color: Color32) {
    let pt = |x: f32, y: f32| {
        pos2(
            r.left() + 4. + x * (r.width() - 8.),
            r.top() + 4. + y * (r.height() - 8.),
        )
    };
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([pt(a.0, a.1), pt(b.0, b.1)], Stroke::new(1.1, color));
    };
    let poly = |v: &[(f32, f32)], closed: bool| {
        let points = v.iter().map(|v| pt(v.0, v.1)).collect();
        p.add(if closed {
            egui::Shape::closed_line(points, Stroke::new(1.1, color))
        } else {
            egui::Shape::line(points, Stroke::new(1.1, color))
        });
    };
    match command {
        WallPetCommand::InsertArcNode => {
            poly(
                &[
                    (0., 0.9),
                    (0.15, 0.45),
                    (0.5, 0.15),
                    (0.85, 0.45),
                    (1., 0.9),
                ],
                false,
            );
            p.rect_filled(
                Rect::from_center_size(pt(0.5, 0.15), vec2(4., 4.)),
                0.,
                color,
            );
            line((0.68, 0.05), (0.98, 0.05));
            line((0.83, -0.1), (0.83, 0.2));
        }
        WallPetCommand::OffsetEdge => {
            poly(&[(0., 1.), (0., 0.5), (1., 0.5), (1., 1.)], false);
            line((0., 0.1), (1., 0.1));
            line((0.5, 0.2), (0.5, 0.45));
        }
        WallPetCommand::OffsetAll => {
            poly(&[(0., 0.), (1., 0.), (1., 1.), (0., 1.)], true);
            poly(
                &[(0.25, 0.25), (0.75, 0.25), (0.75, 0.75), (0.25, 0.75)],
                true,
            );
        }
        WallPetCommand::AddPolygon | WallPetCommand::SubtractPolygon => {
            poly(&[(0., 0.2), (0.7, 0.2), (0.7, 1.), (0., 1.)], true);
            poly(&[(0.4, 0.), (1., 0.), (1., 0.6)], false);
            line((0.72, 0.27), (0.98, 0.27));
            if command == WallPetCommand::AddPolygon {
                line((0.85, 0.14), (0.85, 0.4));
            }
        }
        WallPetCommand::CustomEdge => {
            poly(&[(0., 0.), (0.7, 0.), (1., 0.8), (0., 0.8)], true);
            line((0.1, 1.), (0.9, 1.));
            line((0.65, 0.15), (0.85, 0.65));
        }
        WallPetCommand::MoveNode => {
            poly(&[(0., 0.8), (0.5, 0.2), (1., 0.8)], false);
            p.rect_filled(
                Rect::from_center_size(pt(0.5, 0.2), vec2(4., 4.)),
                0.,
                color,
            );
        }
        WallPetCommand::OffsetPolyline => {
            poly(&[(0., 0.8), (0.5, 0.3), (1., 0.8)], false);
            poly(&[(0., 0.5), (0.5, 0.), (1., 0.5)], false);
        }
        WallPetCommand::Chamfer => poly(&[(0., 1.), (0., 0.4), (0.4, 0.), (1., 0.)], false),
        WallPetCommand::Fillet => {
            let v: Vec<_> = (0..=12)
                .map(|i| {
                    let a = i as f32 / 12. * std::f32::consts::FRAC_PI_2;
                    (1. - a.cos(), 1. - a.sin())
                })
                .collect();
            poly(&v, false);
        }
        WallPetCommand::InsertNode => {
            poly(&[(0., 0.8), (0.5, 0.1), (1., 0.8)], false);
            line((0., 0.8), (1., 0.8));
            line((0.38, 0.1), (0.62, 0.1));
            line((0.5, 0.), (0.5, 0.23));
        }
        WallPetCommand::Curve | WallPetCommand::Tangent => {
            let v: Vec<_> = (0..=20)
                .map(|i| {
                    let t = i as f32 / 20.;
                    (t, 0.75 - 0.6 * (std::f32::consts::PI * t).sin())
                })
                .collect();
            poly(&v, false);
            line((0., 0.75), (1., 0.75));
            if command == WallPetCommand::Tangent {
                line((0., 0.75), (0.3, 0.));
            } else {
                line((0.4, 0.15), (0.6, 0.15));
                line((0.5, 0.05), (0.5, 0.25));
            }
        }
        WallPetCommand::OuterSide => poly(&[(0., 0.4), (1., 0.1), (1., 0.85), (0., 0.85)], true),
        WallPetCommand::TrapezoidNode => poly(
            &[(0., 0.5), (0.5, 0.1), (1., 0.5), (1., 0.85), (0., 0.85)],
            true,
        ),
        WallPetCommand::Drag => {
            poly(&[(0., 0.4), (0., 0.), (0.4, 0.)], false);
            line((0.5, 0.2), (0.5, 1.));
            line((0.1, 0.6), (0.9, 0.6));
            for (a, b) in [
                ((0.5, 0.2), (0.35, 0.35)),
                ((0.5, 0.2), (0.65, 0.35)),
                ((0.5, 1.), (0.35, 0.85)),
                ((0.5, 1.), (0.65, 0.85)),
                ((0.1, 0.6), (0.25, 0.45)),
                ((0.9, 0.6), (0.75, 0.75)),
            ] {
                line(a, b);
            }
        }
        WallPetCommand::Rotate => {
            poly(&[(0.1, 0.3), (0.55, 0.05), (0.8, 0.5), (0.35, 0.75)], true);
            poly(&[(0.4, 1.), (0.8, 0.95), (1., 0.7), (0.8, 0.75)], false);
        }
        WallPetCommand::Mirror => {
            line((0., 1.), (1., 0.));
            poly(&[(0., 0.), (0.35, 0.), (0.35, 0.35), (0., 0.35)], true);
            poly(&[(0.65, 0.65), (1., 0.65), (1., 1.), (0.65, 1.)], true);
        }
        WallPetCommand::Elevate => {
            poly(&[(0., 0.2), (0.2, 0.), (1., 0.), (0.8, 0.2)], true);
            poly(&[(0., 1.), (0.2, 0.8), (1., 0.8), (0.8, 1.)], true);
            line((0.5, 0.35), (0.5, 0.85));
            poly(&[(0.35, 0.5), (0.5, 0.35), (0.65, 0.5)], false);
        }
        WallPetCommand::Multiply => {
            for i in 0..3 {
                let t = i as f32 * 0.2;
                poly(
                    &[
                        (t, 0.4 - t),
                        (t + 0.6, 0.4 - t),
                        (t + 0.6, 1. - t),
                        (t, 1. - t),
                    ],
                    true,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(command: WallPetCommand) -> (Project, WallInsert) {
        let mut p = Project::default();
        let id = p.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(4., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        (
            p,
            WallInsert {
                id,
                ids: vec![id],
                junction: None,
                endpoint: None,
                polyline: None,
                slab_edge: None,
                slab_node: None,
                slab_arc: false,
                polygon_ready: false,
                edge_angle: 90.,
                edge_surface: None,
                edge_scope: crate::slab_geometry::SlabEdgeScope::Clicked,
                revision: 0,
                anchor: Point::new(2., 0.),
                split: 0.5,
                point: Point::new(2., 2.),
                elevation: None,
                palette_at: Pos2::ZERO,
                numeric: false,
                confirm: false,
                canvas_drag: false,
                tracker_at: None,
                tracker_rect: None,
                tracker_focused: false,
                command,
                stages: vec![],
                copies: 3,
                height: 1.25,
                corner: 0.25,
            },
        )
    }
    fn commit(p: &Project, s: &WallInsert) -> Project {
        reshape::candidate(p, &s.operation(p).unwrap().unwrap())
            .unwrap()
            .0
    }
    #[test]
    fn pet_labels_are_localized() {
        for command in WallPetCommand::ALL {
            let (_, _session) = fixture(command);
            assert_ne!(
                UiLanguage::SimplifiedChinese.translate(command.label()),
                command.label()
            );
        }
        assert_eq!(
            localized_error(UiLanguage::SimplifiedChinese, "Invalid wall arc"),
            "墙体圆弧无效"
        );
    }
    #[test]
    fn ten_pet_commands_have_real_geometry_and_staged_confirmation() {
        for command in WallPetCommand::ALL {
            let (p, mut s) = fixture(command);
            match command {
                WallPetCommand::OuterSide => {
                    assert!(s.operation(&p).unwrap().is_none());
                    s.stages.push(Point::new(0., 0.5));
                    s.point = Point::new(4., 1.);
                }
                WallPetCommand::Rotate => {
                    assert!(s.operation(&p).unwrap().is_none());
                    s.stages.push(Point::new(0., 0.));
                    assert!(s.operation(&p).unwrap().is_none());
                    s.stages.push(Point::new(4., 0.));
                    s.point = Point::new(0., 4.);
                }
                WallPetCommand::Mirror => {
                    assert!(s.operation(&p).unwrap().is_none());
                    s.stages.push(Point::new(0., 0.));
                    s.point = Point::new(1., 1.);
                }
                _ => {}
            }
            let result = commit(&p, &s);
            result.validate().unwrap();
            assert_ne!(p, result, "{command:?}");
            let wall = &result.elements[0];
            match command {
                WallPetCommand::Curve => {
                    assert_eq!(result.elements.len(), 1);
                    assert!(
                        crate::wall_geometry::project_station(wall, Point::new(2., 2.)).1 < 1e-4
                    );
                }
                WallPetCommand::Tangent => {
                    let (_, t) = crate::wall_geometry::point_tangent_at(wall, 0.).unwrap();
                    assert!((t.x - t.y).abs() < 1e-4);
                }
                WallPetCommand::OuterSide => {
                    let Some(WallContour::Trapezoid {
                        outer_start,
                        outer_end,
                        ..
                    }) = wall.wall_contour
                    else {
                        panic!()
                    };
                    assert_eq!(outer_start, Point::new(0., 0.5));
                    assert_eq!(outer_end, Point::new(4., 1.));
                }
                WallPetCommand::InsertNode | WallPetCommand::TrapezoidNode => {
                    assert_eq!(result.elements.len(), 2)
                }
                WallPetCommand::Drag => assert_eq!(
                    geometry::wall_reference_line(wall).unwrap(),
                    (Point::new(0., 2.), Point::new(4., 2.))
                ),
                WallPetCommand::Rotate | WallPetCommand::Mirror => assert!(
                    geometry::wall_reference_line(wall)
                        .unwrap()
                        .1
                        .distance(Point::new(0., 4.))
                        < 1e-4
                ),
                WallPetCommand::Elevate => {
                    assert!((geometry::corners(&result, wall)[0].z - 1.25).abs() < 1e-4)
                }
                WallPetCommand::Multiply => assert_eq!(result.elements.len(), 4),
                _ => unreachable!(),
            }
        }
    }
    #[test]
    fn tangent_uses_nearer_end_and_cycle_discards_only_pending_points() {
        let (p, mut s) = fixture(WallPetCommand::Tangent);
        s.split = 0.75;
        s.point = Point::new(2., 2.);
        let result = commit(&p, &s);
        let wall = &result.elements[0];
        let (_, t) = crate::wall_geometry::point_tangent_at(
            wall,
            crate::wall_geometry::reference_length(wall),
        )
        .unwrap();
        assert!(t.x > 0. && t.y < 0.);
        assert!((t.x + t.y).abs() < 1e-4);
        s.stages.push(Point::new(1., 1.));
        s.cycle(false);
        assert!(s.stages.is_empty());
        assert_eq!(s.command, WallPetCommand::OuterSide);
        assert_eq!(p.elements.len(), 1);
        assert!(p.elements[0].wall_contour.is_none());
    }
}
