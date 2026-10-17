use crate::{
    geometry,
    model::*,
    view_settings::{Layer, RenovationStatus},
    wall::{WallGeometryMethod, WallSettings},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Marquee,
    Wall,
    Slab,
    Stair,
    Polyline,
}

fn polygon_signed_area(points: &[Point]) -> f32 {
    points.iter().enumerate().fold(0.0, |sum, (index, a)| {
        let b = points[(index + 1) % points.len()];
        sum + a.x * b.y - b.x * a.y
    }) * 0.5
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarqueeScope {
    CurrentStory,
    AllStories,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarqueeArea {
    pub vertices: Vec<Point>,
    pub scope: MarqueeScope,
    pub story: u32,
    /// Optional vertical limits used by a 3D-window marquee volume.
    pub z_range: Option<(f32, f32)>,
}

impl MarqueeArea {
    pub fn new(vertices: Vec<Point>, scope: MarqueeScope, story: u32) -> Option<Self> {
        if vertices.len() < 3 || vertices.iter().any(|p| !p.finite()) {
            return None;
        }
        let area = vertices.iter().enumerate().fold(0.0, |sum, (i, a)| {
            let b = vertices[(i + 1) % vertices.len()];
            sum + a.x * b.y - b.x * a.y
        });
        (area.abs() > 1e-5).then_some(Self {
            vertices,
            scope,
            story,
            z_range: None,
        })
    }

    pub fn includes_story(&self, story: u32) -> bool {
        self.scope == MarqueeScope::AllStories || story == self.story
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MarqueeGeometry {
    #[default]
    Rectangle,
    RotatedRectangle,
    Polygon,
}
pub struct Editor {
    pub project: Project,
    pub selected: Option<u64>,
    pub selected_ids: Vec<u64>,
    pub clipboard: Vec<Element>,
    pub story: u32,
    pub previous_story: u32,
    pub tool: Tool,
    previous_tool: Tool,
    pub marquee: Option<MarqueeArea>,
    pub anchor: Option<Point>,
    pub polyline_points: Vec<Point>,
    /// Wall nodes accumulated by the Continuous and Polygonal geometry
    /// methods. Single and Rectangular keep only `anchor` until they commit.
    pub wall_points: Vec<Point>,
    pub thickness: f32,
    pub height: f32,
    pub stair_spec: crate::stairs::StairSpec,
    pub wall_settings: WallSettings,
    pub wall_layer: Layer,
    pub wall_renovation: RenovationStatus,
    pub material: Material,
    pub snap: bool,
    pub orthogonal: bool,
    pub undo: Vec<Project>,
    pub redo: Vec<Project>,
    pub saved: Project,
    pub status: String,
}
impl Editor {
    pub fn new(project: Project) -> Self {
        Self {
            saved: project.clone(),
            project,
            selected: None,
            selected_ids: Vec::new(),
            clipboard: Vec::new(),
            story: 0,
            previous_story: 0,
            tool: Tool::Select,
            previous_tool: Tool::Select,
            marquee: None,
            anchor: None,
            polyline_points: Vec::new(),
            wall_points: Vec::new(),
            thickness: 0.22,
            height: 3.,
            stair_spec: crate::stairs::StairSpec::default(),
            wall_settings: WallSettings::default(),
            wall_layer: Layer::Architectural,
            wall_renovation: RenovationStatus::New,
            material: Material::Plaster,
            snap: true,
            orthogonal: false,
            undo: vec![],
            redo: vec![],
            status: "Select an element or start drawing".into(),
        }
    }
    pub fn dirty(&self) -> bool {
        self.saved != self.project
    }
    pub fn commit(&mut self, before: Project) {
        if before != self.project {
            self.undo.push(before);
            if self.undo.len() > 100 {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
    }
    pub fn set_tool(&mut self, t: Tool) {
        if self.tool != t {
            self.previous_tool = self.tool;
        } else if t == Tool::Select {
            self.previous_tool = Tool::Select;
        }
        self.tool = t;
        self.anchor = None;
        self.polyline_points.clear();
        self.wall_points.clear();
        self.status = match t {
            Tool::Select => "Select an element · Drag to move · Delete to remove",
            Tool::Marquee => "Marquee · Drag a rectangle or choose Polygon / Rotated Rectangle",
            Tool::Wall => self.wall_settings.geometry.geometry_method.status_key(),
            Tool::Slab => "Slab · Click two opposite corners · Enter commits Tracker values",
            Tool::Stair => "Stair · Set parameters, then click run start and end",
            Tool::Polyline => "Polyline · Click start, left-click nodes, right-click to finish · click start to close",
        }
        .into();
    }
    pub fn return_to_previous_tool(&mut self) -> Tool {
        let target = if self.previous_tool == self.tool {
            Tool::Select
        } else {
            self.previous_tool
        };
        self.set_tool(target);
        // One Escape returns to the last command; the next Escape goes to Select.
        self.previous_tool = Tool::Select;
        target
    }
    pub fn switch_story(&mut self, id: u32) {
        if self.project.stories.iter().any(|s| s.id == id) {
            if self.story != id {
                self.previous_story = self.story;
            }
            self.story = id;
            self.anchor = None;
            self.wall_points.clear();
            self.selected = None;
            self.selected_ids.clear();
        }
    }
    pub fn click(&mut self, p: Point, tolerance: f32) {
        if !p.finite() || p.x.abs() >= 100000. || p.y.abs() >= 100000. {
            self.status = "Coordinates must be finite and within the local modeling range".into();
            return;
        }
        if self.tool == Tool::Marquee {
            return;
        }
        if self.tool == Tool::Polyline {
            if self
                .polyline_points
                .last()
                .is_none_or(|last| last.distance(p) >= 0.01)
            {
                self.polyline_points.push(p);
                self.status = format!(
                    "Polyline · {} node(s) · right-click to finish",
                    self.polyline_points.len()
                );
            }
            return;
        }
        if self.tool == Tool::Wall {
            self.click_wall(p, tolerance);
            return;
        }
        match self.tool {
            Tool::Select => {
                self.selected = geometry::hit_plan(&self.project, self.story, p, tolerance);
                self.selected_ids = self.selected.into_iter().collect();
            }
            _ => {
                if let Some(a) = self.anchor {
                    let shape = match self.tool {
                        Tool::Wall => unreachable!("walls are handled by click_wall"),
                        Tool::Slab => {
                            let min = Point::new(a.x.min(p.x), a.y.min(p.y));
                            let max = Point::new(a.x.max(p.x), a.y.max(p.y));
                            if max.x - min.x < 0.01 || max.y - min.y < 0.01 {
                                self.status = "Slab needs a non-zero width and depth".into();
                                return;
                            }
                            Shape::Slab {
                                min,
                                max,
                                thickness: self.thickness,
                            }
                        }
                        Tool::Stair => {
                            let mut spec = self.stair_spec.clone();
                            spec.start = a;
                            spec.end = p;
                            if let Err(error) = spec.validate() {
                                self.status = error.into();
                                return;
                            }
                            Shape::Stair { spec }
                        }
                        Tool::Polyline => unreachable!("polylines are committed as a node chain"),
                        _ => unreachable!(),
                    };
                    let before = self.project.clone();
                    let is_wall = matches!(shape, Shape::Wall { .. });
                    let id = self.project.add_with_status(
                        self.story,
                        self.material,
                        shape,
                        self.wall_renovation,
                    );
                    if is_wall {
                        if let Some(element) = self
                            .project
                            .elements
                            .iter_mut()
                            .find(|element| element.id == id)
                        {
                            element.wall_settings = Some(self.wall_settings.clone());
                            element.layer = self.wall_layer;
                        }
                    }
                    if let Err(error) = self.project.validate() {
                        self.project = before;
                        self.status = error;
                        return;
                    }
                    self.selected = Some(id);
                    self.selected_ids = vec![id];
                    self.commit(before);
                    self.anchor = None;
                    self.status = format!("Created element {id:03}");
                } else {
                    self.anchor = Some(p)
                }
            }
        }
    }
    pub fn finish_polyline(&mut self, endpoint: Option<Point>) -> Result<Option<u64>, String> {
        if self.tool != Tool::Polyline {
            return Ok(None);
        }
        if let Some(point) = endpoint {
            if !point.finite() || point.x.abs() >= 100000. || point.y.abs() >= 100000. {
                return Err(
                    "Coordinates must be finite and within the local modeling range".into(),
                );
            }
            if self
                .polyline_points
                .last()
                .is_none_or(|last| last.distance(point) >= 0.01)
            {
                self.polyline_points.push(point);
            }
        }
        if self.polyline_points.len() < 2 {
            self.status = "Polyline needs at least two distinct nodes".into();
            return Err(self.status.clone());
        }
        let before = self.project.clone();
        let id = self.project.add(
            self.story,
            self.material,
            Shape::Polyline {
                points: std::mem::take(&mut self.polyline_points),
                arcs: vec![],
            },
        );
        if let Err(error) = self.project.validate() {
            self.project = before;
            self.status = error.clone();
            return Err(error);
        }
        self.selected = Some(id);
        self.selected_ids = vec![id];
        self.commit(before);
        self.status = format!("Created Polyline {id:03}");
        Ok(Some(id))
    }
    pub fn cancel_polyline(&mut self) {
        self.polyline_points.clear();
    }

    pub fn wall_draft_active(&self) -> bool {
        self.tool == Tool::Wall && (self.anchor.is_some() || !self.wall_points.is_empty())
    }

    pub fn cancel_wall_draft(&mut self) {
        self.anchor = None;
        self.wall_points.clear();
    }

    /// Restore the status hint that belongs to the current wall geometry
    /// method. Used after the Info Box changes the creation defaults.
    pub fn refresh_wall_status(&mut self) {
        if self.tool == Tool::Wall && !self.wall_draft_active() {
            self.status = self
                .wall_settings
                .geometry
                .geometry_method
                .status_key()
                .into();
        }
    }

    fn click_wall(&mut self, p: Point, tolerance: f32) {
        match self.wall_settings.geometry.geometry_method {
            WallGeometryMethod::Single => {
                if self.anchor.is_none() {
                    self.anchor = Some(p);
                    self.wall_points.clear();
                    self.status = WallGeometryMethod::Single.status_key().into();
                } else {
                    let _ = self.finish_wall(Some(p));
                }
            }
            WallGeometryMethod::Rectangular => {
                if self.anchor.is_none() {
                    self.anchor = Some(p);
                    self.wall_points.clear();
                    self.status = WallGeometryMethod::Rectangular.status_key().into();
                } else {
                    let _ = self.finish_wall(Some(p));
                }
            }
            WallGeometryMethod::Continuous => {
                if self.anchor.is_none() {
                    self.anchor = Some(p);
                    self.wall_points = vec![p];
                    self.status = WallGeometryMethod::Continuous.status_key().into();
                } else if self
                    .wall_points
                    .last()
                    .is_none_or(|last| last.distance(p) >= 0.01)
                {
                    self.wall_points.push(p);
                    self.anchor = Some(p);
                }
            }
            WallGeometryMethod::Polygonal => {
                if self.anchor.is_none() {
                    self.anchor = Some(p);
                    self.wall_points = vec![p];
                    self.status = WallGeometryMethod::Polygonal.status_key().into();
                } else if self.wall_points.len() >= 3
                    && self.wall_points[0].distance(p) <= tolerance.max(0.001)
                {
                    let _ = self.finish_wall(None);
                } else if self
                    .wall_points
                    .last()
                    .is_none_or(|last| last.distance(p) >= 0.01)
                {
                    self.wall_points.push(p);
                    self.anchor = Some(p);
                }
            }
        }
    }

    /// Complete the active wall geometry method. `endpoint` is the Tracker or
    /// double-click point; Continuous and Polygonal append it as their final
    /// node, while Single and Rectangular use it as the second defining point.
    /// Every method commits one undo transaction.
    pub fn finish_wall(&mut self, endpoint: Option<Point>) -> Result<Vec<u64>, String> {
        if let Some(point) = endpoint {
            if !point.finite() || point.x.abs() >= 100000. || point.y.abs() >= 100000. {
                let error =
                    "Coordinates must be finite and within the local modeling range".to_owned();
                self.status = error.clone();
                return Err(error);
            }
        }
        let method = self.wall_settings.geometry.geometry_method;
        match method {
            WallGeometryMethod::Single => {
                let Some(start) = self.anchor else {
                    return Err("Wall needs a start point".into());
                };
                let Some(end) = endpoint else {
                    return Err("Wall needs an end point".into());
                };
                if start.distance(end) < 0.01 {
                    self.status = "Wall must be at least 10 mm".into();
                    return Err(self.status.clone());
                }
                let ids = self.create_wall_segments(vec![(start, end)])?;
                self.status = format!("Created element {:03}", ids[0]);
                Ok(ids)
            }
            WallGeometryMethod::Rectangular => {
                let Some(a) = self.anchor else {
                    return Err("Rectangle needs a first corner".into());
                };
                let Some(b) = endpoint else {
                    return Err("Rectangle needs an opposite corner".into());
                };
                let min = Point::new(a.x.min(b.x), a.y.min(b.y));
                let max = Point::new(a.x.max(b.x), a.y.max(b.y));
                if max.x - min.x < 0.01 || max.y - min.y < 0.01 {
                    self.status = "Rectangle needs a non-zero width and depth".into();
                    return Err(self.status.clone());
                }
                let corners = [min, Point::new(max.x, min.y), max, Point::new(min.x, max.y)];
                let segments = (0..4)
                    .map(|index| (corners[index], corners[(index + 1) % 4]))
                    .collect();
                let ids = self.create_wall_segments(segments)?;
                self.status = format!("Created {} rectangular wall segments", ids.len());
                Ok(ids)
            }
            WallGeometryMethod::Continuous => {
                if self.wall_points.is_empty() {
                    if let Some(start) = self.anchor {
                        self.wall_points.push(start);
                    }
                }
                if let Some(point) = endpoint {
                    if self
                        .wall_points
                        .last()
                        .is_none_or(|last| last.distance(point) >= 0.01)
                    {
                        self.wall_points.push(point);
                    }
                }
                if self.wall_points.len() < 2 {
                    self.status = "Continuous wall needs at least two nodes".into();
                    return Err(self.status.clone());
                }
                let segments = self
                    .wall_points
                    .windows(2)
                    .map(|pair| (pair[0], pair[1]))
                    .collect();
                let ids = self.create_wall_segments(segments)?;
                self.status = format!("Created {} connected wall segments", ids.len());
                Ok(ids)
            }
            WallGeometryMethod::Polygonal => {
                if self.wall_points.is_empty() {
                    if let Some(start) = self.anchor {
                        self.wall_points.push(start);
                    }
                }
                if let Some(point) = endpoint {
                    let closes_on_start = self
                        .wall_points
                        .first()
                        .is_some_and(|start| start.distance(point) < 0.01);
                    if !closes_on_start
                        && self
                            .wall_points
                            .last()
                            .is_none_or(|last| last.distance(point) >= 0.01)
                    {
                        self.wall_points.push(point);
                    }
                }
                if self.wall_points.len() < 3 {
                    self.status = "Polygon wall needs at least three nodes".into();
                    return Err(self.status.clone());
                }
                let area = polygon_signed_area(&self.wall_points);
                if area.abs() < 1e-5 {
                    self.status = "Polygon wall needs a non-zero area".into();
                    return Err(self.status.clone());
                }
                // Keep the ring counter-clockwise so Inside/Outside reference
                // line offsets consistently describe the enclosed interior.
                if area < 0. {
                    self.wall_points.reverse();
                }
                let count = self.wall_points.len();
                let segments = (0..count)
                    .map(|index| {
                        (
                            self.wall_points[index],
                            self.wall_points[(index + 1) % count],
                        )
                    })
                    .collect();
                let ids = self.create_wall_segments(segments)?;
                self.status = format!("Created {} polygon wall segments", ids.len());
                Ok(ids)
            }
        }
    }

    /// Backspace during a wall draft removes the last input node/segment
    /// without touching the selected model elements.
    pub fn backspace_wall(&mut self) -> bool {
        if !self.wall_draft_active() {
            return false;
        }
        match self.wall_settings.geometry.geometry_method {
            WallGeometryMethod::Single | WallGeometryMethod::Rectangular => {
                self.cancel_wall_draft();
                self.status = "Wall cancelled".into();
            }
            WallGeometryMethod::Continuous | WallGeometryMethod::Polygonal => {
                self.wall_points.pop();
                self.anchor = self.wall_points.last().copied();
                if self.anchor.is_none() {
                    self.status = "Wall cancelled".into();
                } else {
                    self.status = self
                        .wall_settings
                        .geometry
                        .geometry_method
                        .status_key()
                        .into();
                }
            }
        }
        true
    }

    fn create_wall_segments(&mut self, segments: Vec<(Point, Point)>) -> Result<Vec<u64>, String> {
        let before = self.project.clone();
        let mut ids = Vec::with_capacity(segments.len());
        for (start, end) in segments {
            if start.distance(end) < 0.01 {
                self.project = before;
                self.status = "Wall must be at least 10 mm".into();
                return Err(self.status.clone());
            }
            let shape = Shape::Wall {
                start,
                end,
                thickness: self.thickness,
                height: self.height,
            };
            let id = self.project.add_with_status(
                self.story,
                self.material,
                shape,
                self.wall_renovation,
            );
            if let Some(element) = self
                .project
                .elements
                .iter_mut()
                .find(|element| element.id == id)
            {
                element.wall_settings = Some(self.wall_settings.clone());
                element.layer = self.wall_layer;
            }
            ids.push(id);
        }
        if let Err(error) = self.project.validate() {
            self.project = before;
            self.status = error.clone();
            return Err(error);
        }
        self.commit(before);
        self.anchor = None;
        self.wall_points.clear();
        self.selected = ids.last().copied();
        self.selected_ids = self.selected.into_iter().collect();
        Ok(ids)
    }
    pub fn undo(&mut self) {
        if let Some(p) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.project, p));
            self.selected = None;
            self.selected_ids.clear();
            self.anchor = None;
            self.wall_points.clear();
            self.status = "Undo".into();
        }
    }
    pub fn redo(&mut self) {
        if let Some(p) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.project, p));
            self.selected = None;
            self.selected_ids.clear();
            self.anchor = None;
            self.wall_points.clear();
            self.status = "Redo".into();
        }
    }
    pub fn delete(&mut self) {
        let ids = self.selection_ids();
        if !ids.is_empty() {
            let before = self.project.clone();
            self.project.elements.retain(|e| !ids.contains(&e.id));
            self.selected = None;
            self.selected_ids.clear();
            self.commit(before);
            self.status = format!("Deleted {} element(s)", ids.len());
        }
    }
    pub fn selection_ids(&self) -> Vec<u64> {
        if self.selected_ids.is_empty() {
            self.selected.into_iter().collect()
        } else {
            self.selected_ids.clone()
        }
    }
    pub fn set_selection(&mut self, mut ids: Vec<u64>) {
        ids.sort_unstable();
        ids.dedup();
        ids.retain(|id| self.project.elements.iter().any(|e| e.id == *id));
        self.selected = ids.last().copied();
        self.selected_ids = ids;
    }
    pub fn select_all_in_marquee(&mut self) -> usize {
        let Some(area) = &self.marquee else { return 0 };
        let ids = self
            .project
            .elements
            .iter()
            .filter(|e| area.includes_story(e.story))
            .filter(|e| geometry::element_has_node_in_polygon(e, &area.vertices))
            .filter(|e| {
                area.z_range
                    .is_none_or(|range| geometry::element_overlaps_z_range(&self.project, e, range))
            })
            .map(|e| e.id)
            .collect();
        self.set_selection(ids);
        self.selection_ids().len()
    }
    pub fn move_selection(&mut self, delta: Point) {
        let ids = self.selection_ids();
        if ids.is_empty() || !delta.finite() {
            return;
        }
        let before = self.project.clone();
        for e in self
            .project
            .elements
            .iter_mut()
            .filter(|e| ids.contains(&e.id))
        {
            e.translate(delta);
        }
        if let Err(error) = self.project.validate() {
            self.project = before;
            self.status = error;
            return;
        }
        self.commit(before);
        self.status = format!("Moved {} element(s)", ids.len());
    }
    pub fn remove_marquee(&mut self) {
        self.marquee = None;
        self.status = "Marquee removed".into();
    }
    pub fn move_marquee(&mut self, delta: Point) {
        if !delta.finite() {
            return;
        }
        if let Some(area) = &mut self.marquee {
            for p in &mut area.vertices {
                p.x += delta.x;
                p.y += delta.y;
            }
        }
    }
    pub fn stretch_nodes_in_marquee(&mut self, delta: Point) -> Result<usize, String> {
        let Some(area) = self.marquee.clone() else {
            return Ok(0);
        };
        if !delta.finite() {
            return Err("Stretch vector must be finite".into());
        }
        let before = self.project.clone();
        let mut changed = 0;
        for e in self
            .project
            .elements
            .iter_mut()
            .filter(|e| area.includes_story(e.story))
        {
            match &mut e.shape {
                Shape::Wall { start, end, .. } => {
                    let a = geometry::point_in_polygon(*start, &area.vertices);
                    let b = geometry::point_in_polygon(*end, &area.vertices);
                    if a {
                        start.x += delta.x;
                        start.y += delta.y;
                    }
                    if b {
                        end.x += delta.x;
                        end.y += delta.y;
                    }
                    if a || b {
                        changed += 1;
                    }
                }
                Shape::Slab { min, max, .. } => {
                    let points = [
                        *min,
                        Point::new(max.x, min.y),
                        *max,
                        Point::new(min.x, max.y),
                    ];
                    let moved: Vec<Point> = points
                        .iter()
                        .map(|p| {
                            if geometry::point_in_polygon(*p, &area.vertices) {
                                Point::new(p.x + delta.x, p.y + delta.y)
                            } else {
                                *p
                            }
                        })
                        .collect();
                    if moved != points {
                        let lo = moved
                            .iter()
                            .fold(Point::new(f32::INFINITY, f32::INFINITY), |a, b| {
                                Point::new(a.x.min(b.x), a.y.min(b.y))
                            });
                        let hi = moved
                            .iter()
                            .fold(Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY), |a, b| {
                                Point::new(a.x.max(b.x), a.y.max(b.y))
                            });
                        *min = lo;
                        *max = hi;
                        changed += 1;
                    }
                }
                Shape::Stair { spec } => {
                    let a = geometry::point_in_polygon(spec.start, &area.vertices);
                    let b = geometry::point_in_polygon(spec.end, &area.vertices);
                    if a {
                        spec.start.x += delta.x;
                        spec.start.y += delta.y;
                    }
                    if b {
                        spec.end.x += delta.x;
                        spec.end.y += delta.y;
                    }
                    if a || b {
                        changed += 1;
                    }
                }
                Shape::Polyline { points, .. } => {
                    let mut moved = false;
                    for point in points {
                        if geometry::point_in_polygon(*point, &area.vertices) {
                            point.x += delta.x;
                            point.y += delta.y;
                            moved = true;
                        }
                    }
                    if moved {
                        changed += 1;
                    }
                }
            }
        }
        if changed == 0 {
            return Ok(0);
        }
        if let Err(error) = self.project.validate() {
            self.project = before;
            return Err(error);
        }
        self.commit(before);
        self.status = format!("Stretched nodes on {changed} element(s)");
        Ok(changed)
    }
    pub fn duplicate(&mut self) {
        self.copy_selection();
        self.paste(Point::new(0.5, 0.5));
    }
    pub fn copy_selection(&mut self) -> usize {
        let ids = self.selection_ids();
        self.clipboard = self
            .project
            .elements
            .iter()
            .filter(|e| ids.contains(&e.id))
            .cloned()
            .collect();
        self.clipboard.len()
    }
    pub fn cut_selection(&mut self) -> usize {
        let count = self.copy_selection();
        if count > 0 {
            self.delete();
        }
        count
    }
    pub fn paste(&mut self, offset: Point) -> usize {
        if self.clipboard.is_empty() || !offset.finite() {
            return 0;
        }
        let before = self.project.clone();
        let mut copies = Vec::with_capacity(self.clipboard.len());
        for mut copy in self.clipboard.clone() {
            copy.translate(offset);
            copies.push(self.project.add(copy.story, copy.material, copy.shape));
        }
        if self.project.validate().is_err() {
            self.project = before;
            self.status = "Paste exceeds the valid modeling range".into();
            return 0;
        }
        self.set_selection(copies);
        self.commit(before);
        self.selection_ids().len()
    }
    pub fn mirror_selection(&mut self, mirror_x: bool) {
        let ids = self.selection_ids();
        let bounds: Vec<Point> = self
            .project
            .elements
            .iter()
            .filter(|e| ids.contains(&e.id))
            .flat_map(|e| geometry::footprint(e))
            .collect();
        if bounds.is_empty() {
            return;
        }
        let center = bounds.iter().fold(Point::default(), |a, p| {
            Point::new(
                a.x + p.x / bounds.len() as f32,
                a.y + p.y / bounds.len() as f32,
            )
        });
        let before = self.project.clone();
        for e in self
            .project
            .elements
            .iter_mut()
            .filter(|e| ids.contains(&e.id))
        {
            Self::transform_element(e, center, |p| {
                if mirror_x {
                    Point::new(2. * center.x - p.x, p.y)
                } else {
                    Point::new(p.x, 2. * center.y - p.y)
                }
            });
            if let Shape::Polyline { arcs, .. } = &mut e.shape {
                for arc in arcs.iter_mut().flatten() {
                    arc.clockwise = !arc.clockwise;
                }
            }
        }
        if self.project.validate().is_err() {
            self.project = before;
            return;
        }
        self.commit(before);
        self.status = if mirror_x {
            "Mirrored selection across X"
        } else {
            "Mirrored selection across Y"
        }
        .into();
    }
    pub fn rotate_selection_quarter_turn(&mut self, clockwise: bool) {
        let ids = self.selection_ids();
        let bounds: Vec<Point> = self
            .project
            .elements
            .iter()
            .filter(|e| ids.contains(&e.id))
            .flat_map(|e| geometry::footprint(e))
            .collect();
        if bounds.is_empty() {
            return;
        }
        let center = bounds.iter().fold(Point::default(), |a, p| {
            Point::new(
                a.x + p.x / bounds.len() as f32,
                a.y + p.y / bounds.len() as f32,
            )
        });
        let before = self.project.clone();
        for e in self
            .project
            .elements
            .iter_mut()
            .filter(|e| ids.contains(&e.id))
        {
            Self::transform_element(e, center, |p| {
                let x = p.x - center.x;
                let y = p.y - center.y;
                if clockwise {
                    Point::new(center.x + y, center.y - x)
                } else {
                    Point::new(center.x - y, center.y + x)
                }
            });
        }
        if self.project.validate().is_err() {
            self.project = before;
            return;
        }
        self.commit(before);
        self.status = "Rotated selection 90°".into();
    }
    fn transform_element(e: &mut Element, _center: Point, transform: impl Fn(Point) -> Point) {
        match &mut e.shape {
            Shape::Wall { start, end, .. } => {
                *start = transform(*start);
                *end = transform(*end);
            }
            Shape::Slab { min, max, .. } => {
                let points = [
                    *min,
                    Point::new(max.x, min.y),
                    *max,
                    Point::new(min.x, max.y),
                ]
                .map(transform);
                *min = points
                    .iter()
                    .fold(Point::new(f32::INFINITY, f32::INFINITY), |a, p| {
                        Point::new(a.x.min(p.x), a.y.min(p.y))
                    });
                *max = points
                    .iter()
                    .fold(Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY), |a, p| {
                        Point::new(a.x.max(p.x), a.y.max(p.y))
                    });
            }
            Shape::Stair { spec } => {
                spec.start = transform(spec.start);
                spec.end = transform(spec.end);
            }
            Shape::Polyline { points, .. } => {
                for point in points {
                    *point = transform(*point);
                }
            }
        }
    }
    pub fn move_selection_to_story(&mut self, direction: i32) {
        let ids = self.selection_ids();
        if ids.is_empty() || direction == 0 {
            return;
        }
        let mut stories = self.project.stories.clone();
        stories.sort_by(|a, b| a.elevation.total_cmp(&b.elevation));
        let before = self.project.clone();
        for e in self
            .project
            .elements
            .iter_mut()
            .filter(|e| ids.contains(&e.id))
        {
            if let Some(index) = stories.iter().position(|s| s.id == e.story) {
                let next = index as i32 + direction.signum();
                if let Some(story) = usize::try_from(next).ok().and_then(|i| stories.get(i)) {
                    e.story = story.id;
                }
            }
        }
        if self.project != before {
            self.commit(before);
            self.status = "Moved selection to adjacent story".into();
        }
    }
    pub fn replace_element(&mut self, e: Element) -> Result<(), String> {
        let before = self.project.clone();
        if let Some(slot) = self.project.elements.iter_mut().find(|v| v.id == e.id) {
            *slot = e;
        } else {
            return Err("Element no longer exists".into());
        }
        if let Err(err) = self.project.validate() {
            self.project = before;
            return Err(err);
        }
        self.commit(before);
        Ok(())
    }
}
