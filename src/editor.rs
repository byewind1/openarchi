use crate::{
    command::{CommandRecord, ElementEdit, ModelCommand, ProjectDelta, ProjectHeader},
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
    Door,
    Window,
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
    pub door_defaults: OpeningDefaults,
    pub window_defaults: OpeningDefaults,
    pub wall_settings: WallSettings,
    pub wall_layer: Layer,
    pub wall_renovation: RenovationStatus,
    pub material: Material,
    pub snap: bool,
    pub orthogonal: bool,
    pub undo: Vec<CommandRecord>,
    pub redo: Vec<CommandRecord>,
    pub saved: Project,
    /// Monotonic model revision, incremented by commits and undo/redo.
    pub revision: u64,
    /// True after restoring a crash-recovery snapshot that was never saved.
    pub recovered_unsaved: bool,
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
            door_defaults: OpeningKind::Door.defaults(),
            window_defaults: OpeningKind::Window.defaults(),
            wall_settings: WallSettings::default(),
            wall_layer: Layer::Architectural,
            wall_renovation: RenovationStatus::New,
            material: Material::Plaster,
            snap: true,
            orthogonal: false,
            undo: vec![],
            redo: vec![],
            revision: 0,
            recovered_unsaved: false,
            status: "Select an element or start drawing".into(),
        }
    }
    pub fn dirty(&self) -> bool {
        self.recovered_unsaved || self.saved != self.project
    }

    /// Mark the current project as the saved baseline.
    pub fn mark_saved(&mut self) {
        self.saved = self.project.clone();
        self.recovered_unsaved = false;
    }

    /// Replace the editor with a recovered snapshot that remains dirty until saved.
    pub fn restore_unsaved(&mut self, project: Project) {
        *self = Self::new(project);
        self.recovered_unsaved = true;
        self.status = "Recovered unsaved project".into();
    }
    pub fn commit(&mut self, before: Project) {
        if before != self.project {
            self.revision = self.revision.wrapping_add(1);
            let delta = ProjectDelta::between(&before, &self.project);
            self.undo.push(CommandRecord::from_delta(delta));
            if self.undo.len() > 100 {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
    }

    /// Apply one typed model command, validate it and record a reversible
    /// change. Returns the affected element ids; an empty list means the
    /// command was a no-op. Invalid commands leave the project untouched.
    pub fn apply(&mut self, command: ModelCommand) -> Result<Vec<u64>, String> {
        let record_command = command.clone();
        let mut delta = ProjectDelta::default();
        let inverse;
        match command {
            ModelCommand::AddElement { element } => {
                if self
                    .project
                    .elements
                    .iter()
                    .any(|existing| existing.id == element.id)
                {
                    return Err(format!("Element {} already exists", element.id));
                }
                let header_before = ProjectHeader::capture(&self.project);
                let index = self.project.elements.len();
                self.project.next_id = self.project.next_id.max(element.id + 1);
                self.project.elements.push(element.clone());
                delta.elements.push(ElementEdit {
                    id: element.id,
                    before_index: None,
                    before: None,
                    after_index: Some(index),
                    after: Some(element.clone()),
                });
                if self.project.next_id != header_before.next_id {
                    delta.before_header = Some(Box::new(header_before));
                    delta.after_header = Some(Box::new(ProjectHeader::capture(&self.project)));
                }
                inverse = ModelCommand::RemoveElements {
                    ids: vec![element.id],
                };
            }
            ModelCommand::RemoveElements { ids } => {
                let mut requested: std::collections::HashSet<u64> = ids.into_iter().collect();
                if requested.is_empty() {
                    return Ok(Vec::new());
                }
                for element in &self.project.elements {
                    if let Shape::Opening { host_wall, .. } = element.shape
                        && requested.contains(&host_wall)
                    {
                        requested.insert(element.id);
                    }
                }
                let mut removed: Vec<(usize, Element)> = self
                    .project
                    .elements
                    .iter()
                    .enumerate()
                    .filter(|(_, element)| requested.contains(&element.id))
                    .map(|(index, element)| (index, element.clone()))
                    .collect();
                removed.sort_by_key(|(index, _)| *index);
                if removed.is_empty() {
                    return Ok(Vec::new());
                }
                self.project
                    .elements
                    .retain(|element| !requested.contains(&element.id));
                delta.elements = removed
                    .iter()
                    .map(|(index, element)| ElementEdit {
                        id: element.id,
                        before_index: Some(*index),
                        before: Some(element.clone()),
                        after_index: None,
                        after: None,
                    })
                    .collect();
                inverse = ModelCommand::RestoreElements { elements: removed };
            }
            ModelCommand::RestoreElements { elements } => {
                let mut inserts = elements;
                inserts.sort_by_key(|(index, _)| *index);
                let restored: Vec<u64> = inserts.iter().map(|(_, element)| element.id).collect();
                let existing: std::collections::HashSet<u64> = self
                    .project
                    .elements
                    .iter()
                    .map(|element| element.id)
                    .collect();
                for (_, element) in &inserts {
                    if existing.contains(&element.id) {
                        return Err(format!("Element {} already exists", element.id));
                    }
                }
                for (index, element) in inserts {
                    delta.elements.push(ElementEdit {
                        id: element.id,
                        before_index: None,
                        before: None,
                        after_index: Some(index),
                        after: Some(element.clone()),
                    });
                    let index = index.min(self.project.elements.len());
                    self.project.elements.insert(index, element);
                }
                inverse = ModelCommand::RemoveElements { ids: restored };
            }
            ModelCommand::UpdateElement { element } => {
                let Some(index) = self
                    .project
                    .elements
                    .iter()
                    .position(|existing| existing.id == element.id)
                else {
                    return Err(format!("Element {} no longer exists", element.id));
                };
                let previous =
                    std::mem::replace(&mut self.project.elements[index], element.clone());
                if previous != element {
                    delta.elements.push(ElementEdit {
                        id: element.id,
                        before_index: Some(index),
                        before: Some(previous.clone()),
                        after_index: Some(index),
                        after: Some(element),
                    });
                }
                inverse = ModelCommand::UpdateElement { element: previous };
            }
            ModelCommand::MoveElements { ids, offset } => {
                if !offset.finite() {
                    return Err("Move offset must be finite".into());
                }
                if ids.is_empty() {
                    return Ok(Vec::new());
                }
                let requested: std::collections::HashSet<u64> = ids.iter().copied().collect();
                let moved: Vec<(usize, Element)> = self
                    .project
                    .elements
                    .iter()
                    .enumerate()
                    .filter(|(_, element)| requested.contains(&element.id))
                    .map(|(index, element)| (index, element.clone()))
                    .collect();
                if moved.is_empty() {
                    return Ok(Vec::new());
                }
                let moved_ids: Vec<u64> = moved.iter().map(|(_, element)| element.id).collect();
                self.project.translate_many(&moved_ids, offset);
                for (index, previous) in moved {
                    let after = self.project.elements[index].clone();
                    if after != previous {
                        delta.elements.push(ElementEdit {
                            id: previous.id,
                            before_index: Some(index),
                            before: Some(previous),
                            after_index: Some(index),
                            after: Some(after),
                        });
                    }
                }
                inverse = ModelCommand::MoveElements {
                    ids: ids.clone(),
                    offset: Point::new(-offset.x, -offset.y),
                };
            }
            ModelCommand::SetOptions {
                plan,
                design,
                guide,
                grid,
            } => {
                let header_before = ProjectHeader::capture(&self.project);
                inverse = ModelCommand::SetOptions {
                    plan: plan.as_ref().map(|_| self.project.plan_options.clone()),
                    design: design.as_ref().map(|_| self.project.design_options.clone()),
                    guide: guide.as_ref().map(|_| self.project.guide_settings.clone()),
                    grid: grid.as_ref().map(|_| self.project.grid_settings.clone()),
                };
                if let Some(plan) = plan {
                    self.project.plan_options = plan;
                }
                if let Some(design) = design {
                    self.project.design_options = design;
                }
                if let Some(guide) = guide {
                    self.project.guide_settings = guide;
                }
                if let Some(grid) = grid {
                    self.project.grid_settings = grid;
                }
                let header_after = ProjectHeader::capture(&self.project);
                if header_before != header_after {
                    delta.before_header = Some(Box::new(header_before));
                    delta.after_header = Some(Box::new(header_after));
                }
            }
            ModelCommand::ChangeSet => {
                return Err("ChangeSet is recorded internally and is not executable".into());
            }
        }
        if delta.is_empty() {
            return Ok(Vec::new());
        }
        if let Err(error) = self.project.validate() {
            delta.revert(&mut self.project);
            self.status = error.clone();
            return Err(error);
        }
        self.revision = self.revision.wrapping_add(1);
        let affected = delta.affected_ids();
        self.undo
            .push(CommandRecord::new(record_command, inverse, delta));
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
        Ok(affected)
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
            Tool::Door => "Door · Click a wall to place a rectangular door opening",
            Tool::Window => "Window · Click a wall to place a rectangular window opening",
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
        if matches!(self.tool, Tool::Door | Tool::Window) {
            let kind = if self.tool == Tool::Door {
                OpeningKind::Door
            } else {
                OpeningKind::Window
            };
            if let Err(error) = self.click_opening(kind, p, tolerance) {
                self.status = error;
            }
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
                    if is_wall
                        && let Some(element) = self
                            .project
                            .elements
                            .iter_mut()
                            .find(|element| element.id == id)
                    {
                        element.wall_settings = Some(self.wall_settings.clone());
                        element.layer = self.wall_layer;
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

    /// First node of the current wall chain when clicking it should end the
    /// draft. Continuous chains can end after one segment; a polygonal ring
    /// needs at least three nodes before it can close.
    pub fn wall_close_target(&self) -> Option<Point> {
        if self.tool != Tool::Wall {
            return None;
        }
        let minimum = match self.wall_settings.geometry.geometry_method {
            WallGeometryMethod::Continuous => 2,
            WallGeometryMethod::Polygonal => 3,
            WallGeometryMethod::Single | WallGeometryMethod::Rectangular => return None,
        };
        let first = *self.wall_points.first()?;
        (self.wall_points.len() >= minimum).then_some(first)
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
                } else if let Some(start) = self.wall_points.first().copied()
                    && self.wall_points.len() >= 2
                    && start.distance(p) <= tolerance.max(0.001)
                {
                    // Clicking the first node ends the chain. With two nodes
                    // the chain is a single wall, so no zero-length closing
                    // segment is added; with three or more the closing wall
                    // is part of the completed contour.
                    let _ = if self.wall_points.len() >= 3 {
                        self.finish_wall(Some(start))
                    } else {
                        self.finish_wall(None)
                    };
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
                } else if let Some(start) = self.wall_points.first().copied()
                    && self.wall_points.len() >= 2
                    && start.distance(p) <= tolerance.max(0.001)
                {
                    if self.wall_points.len() >= 3 {
                        let _ = self.finish_wall(None);
                    } else {
                        self.status = "Polygon wall needs at least three nodes".into();
                    }
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

    pub fn opening_defaults(&self, kind: OpeningKind) -> OpeningDefaults {
        match kind {
            OpeningKind::Door => self.door_defaults,
            OpeningKind::Window => self.window_defaults,
        }
    }

    pub fn set_opening_defaults(&mut self, kind: OpeningKind, defaults: OpeningDefaults) {
        match kind {
            OpeningKind::Door => self.door_defaults = defaults,
            OpeningKind::Window => self.window_defaults = defaults,
        }
    }

    /// Build a candidate opening for the current pointer position without
    /// changing the model. Shared by the click commit and the plan preview.
    pub fn opening_placement(
        &self,
        kind: OpeningKind,
        p: Point,
        tolerance: f32,
    ) -> Result<Element, String> {
        let host = self
            .project
            .elements
            .iter()
            .rev()
            .find(|element| {
                if element.story != self.story {
                    return false;
                }
                let Shape::Wall {
                    start,
                    end,
                    thickness,
                    ..
                } = element.shape
                else {
                    return false;
                };
                geometry::segment_distance(p, start, end) <= thickness * 0.5 + tolerance
            })
            .ok_or_else(|| format!("Click on a wall to place a {}", kind.name().to_lowercase()))?;
        let Shape::Wall {
            start,
            end,
            height: wall_height,
            ..
        } = host.shape
        else {
            return Err("Opening host must be a wall".into());
        };
        let length = start.distance(end);
        let defaults = self.opening_defaults(kind);
        defaults.validate()?;
        if length + 1e-4 < defaults.width {
            return Err("The host wall is shorter than the opening width".into());
        }
        let (bottom_offset, top_offset) = host
            .wall_settings
            .as_ref()
            .map_or((0., 0.), |settings| settings.vertical_offsets());
        if defaults.sill < bottom_offset - 1e-4 {
            return Err("The opening is below the host wall".into());
        }
        if defaults.sill + defaults.height > wall_height + top_offset + 1e-4 {
            return Err("The opening is taller than the host wall".into());
        }
        let offset = geometry::wall_offset_of(&self.project, host.id, p)
            .unwrap_or(length * 0.5)
            .clamp(defaults.width * 0.5, length - defaults.width * 0.5);
        Ok(Element {
            id: 0,
            name: String::new(),
            story: self.story,
            material: host.material,
            layer: Layer::Architectural,
            renovation: self.wall_renovation,
            design_option: None,
            wall_settings: None,
            shape: Shape::Opening {
                host_wall: host.id,
                kind,
                width: defaults.width,
                height: defaults.height,
                sill: defaults.sill,
                offset,
            },
        })
    }

    fn click_opening(&mut self, kind: OpeningKind, p: Point, tolerance: f32) -> Result<(), String> {
        let placement = self.opening_placement(kind, p, tolerance)?;
        let before = self.project.clone();
        let id = self.project.add_with_status(
            placement.story,
            placement.material,
            placement.shape,
            placement.renovation,
        );
        if let Some(element) = self
            .project
            .elements
            .iter_mut()
            .find(|element| element.id == id)
        {
            element.layer = placement.layer;
        }
        if let Err(error) = self.project.validate() {
            self.project = before;
            return Err(error);
        }
        self.selected = Some(id);
        self.selected_ids = vec![id];
        self.commit(before);
        self.status = format!("Created {} {id:03}", kind.name());
        Ok(())
    }

    /// Complete the active wall geometry method. `endpoint` is the Tracker or
    /// double-click point; Continuous and Polygonal append it as their final
    /// node, while Single and Rectangular use it as the second defining point.
    /// Every method commits one undo transaction.
    pub fn finish_wall(&mut self, endpoint: Option<Point>) -> Result<Vec<u64>, String> {
        if let Some(point) = endpoint
            && (!point.finite() || point.x.abs() >= 100000. || point.y.abs() >= 100000.)
        {
            let error = "Coordinates must be finite and within the local modeling range".to_owned();
            self.status = error.clone();
            return Err(error);
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
                if self.wall_points.is_empty()
                    && let Some(start) = self.anchor
                {
                    self.wall_points.push(start);
                }
                if let Some(point) = endpoint
                    && self
                        .wall_points
                        .last()
                        .is_none_or(|last| last.distance(point) >= 0.01)
                {
                    self.wall_points.push(point);
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
                if self.wall_points.is_empty()
                    && let Some(start) = self.anchor
                {
                    self.wall_points.push(start);
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
        if let Some(record) = self.undo.pop() {
            record.delta.revert(&mut self.project);
            self.redo.push(record);
            self.revision = self.revision.wrapping_add(1);
            self.selected = None;
            self.selected_ids.clear();
            self.anchor = None;
            self.wall_points.clear();
            self.status = "Undo".into();
        }
    }
    pub fn redo(&mut self) {
        if let Some(record) = self.redo.pop() {
            record.delta.replay(&mut self.project);
            self.undo.push(record);
            self.revision = self.revision.wrapping_add(1);
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
            match self.apply(ModelCommand::RemoveElements { ids }) {
                Ok(removed) if !removed.is_empty() => {
                    self.selected = None;
                    self.selected_ids.clear();
                    self.status = format!("Deleted {} element(s)", removed.len());
                }
                Ok(_) => {}
                Err(error) => self.status = error,
            }
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
            .filter(|e| geometry::element_has_node_in_polygon(&self.project, e, &area.vertices))
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
        match self.apply(ModelCommand::MoveElements { ids, offset: delta }) {
            Ok(moved) if !moved.is_empty() => {
                self.status = format!("Moved {} element(s)", moved.len());
            }
            Ok(_) => {}
            Err(error) => self.status = error,
        }
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
                Shape::Opening { .. } => {}
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
        let mut id_map = std::collections::HashMap::new();
        let mut copies = Vec::with_capacity(self.clipboard.len());
        let mut deferred = Vec::new();
        for mut copy in self.clipboard.clone() {
            if matches!(copy.shape, Shape::Opening { .. }) {
                deferred.push(copy);
                continue;
            }
            copy.translate(offset);
            let original_id = copy.id;
            let new_id = self.project.add_with_status(
                copy.story,
                copy.material,
                copy.shape,
                copy.renovation,
            );
            id_map.insert(original_id, new_id);
            copies.push(new_id);
        }
        for mut opening in deferred {
            let Shape::Opening { host_wall, .. } = &mut opening.shape else {
                continue;
            };
            let Some(&new_host) = id_map.get(host_wall) else {
                continue;
            };
            *host_wall = new_host;
            let new_id = self.project.add_with_status(
                opening.story,
                opening.material,
                opening.shape,
                opening.renovation,
            );
            copies.push(new_id);
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
            .flat_map(|e| geometry::element_footprint(&self.project, e))
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
            .flat_map(|e| geometry::element_footprint(&self.project, e))
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
            Shape::Opening { .. } => {}
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
            if let Shape::Opening { host_wall, .. } = e.shape
                && !ids.contains(&host_wall)
            {
                // An opening must stay on its host wall's story.
                continue;
            }
            if let Some(index) = stories.iter().position(|s| s.id == e.story) {
                let next = index as i32 + direction.signum();
                if let Some(story) = usize::try_from(next).ok().and_then(|i| stories.get(i)) {
                    e.story = story.id;
                }
            }
        }
        if self.project != before {
            if let Err(error) = self.project.validate() {
                self.project = before;
                self.status = error;
                return;
            }
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
    /// Replace one wall's stored reference-line endpoints. This is the model
    /// operation behind interactive baseline editing; callers that preview a
    /// drag pass their gesture-start snapshot to `commit` only on release.
    pub fn set_wall_reference_line(
        &mut self,
        id: u64,
        start: Point,
        end: Point,
    ) -> Result<(), String> {
        self.project.validate_wall_reference_line(id, start, end)?;
        let Some(element) = self
            .project
            .elements
            .iter_mut()
            .find(|element| element.id == id)
        else {
            return Err("Missing wall".into());
        };
        let Shape::Wall {
            start: stored_start,
            end: stored_end,
            ..
        } = &mut element.shape
        else {
            return Err("Element is not a wall".into());
        };
        *stored_start = start;
        *stored_end = end;
        Ok(())
    }
}

#[cfg(test)]
mod command_tests {
    use super::*;

    fn wall(id: u64, x: f32) -> Element {
        Element {
            id,
            name: format!("Wall {id}"),
            story: 0,
            material: Material::Plaster,
            layer: Layer::Architectural,
            renovation: RenovationStatus::New,
            design_option: None,
            wall_settings: None,
            shape: Shape::Wall {
                start: Point::new(x, 0.),
                end: Point::new(x + 2., 0.),
                thickness: 0.2,
                height: 3.,
            },
        }
    }

    #[test]
    fn typed_commands_are_one_undo_each_and_redo_exactly() {
        let mut editor = Editor::new(Project::default());
        let first = wall(1, 0.);
        assert_eq!(
            editor
                .apply(ModelCommand::AddElement {
                    element: first.clone()
                })
                .unwrap(),
            vec![1]
        );
        editor
            .apply(ModelCommand::UpdateElement {
                element: wall(1, 3.),
            })
            .unwrap();
        editor
            .apply(ModelCommand::MoveElements {
                ids: vec![1],
                offset: Point::new(2., 1.),
            })
            .unwrap();
        let mut options = editor.project.plan_options.clone();
        options.drawing_scale = 50;
        editor
            .apply(ModelCommand::SetOptions {
                plan: Some(options),
                design: None,
                guide: None,
                grid: None,
            })
            .unwrap();
        assert_eq!(editor.undo.len(), 4);
        let after = editor.project.clone();
        for _ in 0..4 {
            editor.undo();
        }
        assert!(editor.project.elements.is_empty());
        assert_eq!(editor.project.plan_options.drawing_scale, 100);
        assert_eq!(editor.redo.len(), 4);
        for _ in 0..4 {
            editor.redo();
        }
        assert_eq!(editor.project, after);
    }

    #[test]
    fn no_op_and_invalid_commands_leave_history_unchanged() {
        let mut editor = Editor::new(Project::default());
        editor
            .apply(ModelCommand::AddElement {
                element: wall(1, 0.),
            })
            .unwrap();
        let before = editor.project.clone();
        assert!(
            editor
                .apply(ModelCommand::MoveElements {
                    ids: vec![99],
                    offset: Point::new(1., 0.),
                })
                .unwrap()
                .is_empty()
        );
        assert!(
            editor
                .apply(ModelCommand::UpdateElement {
                    element: wall(9, 0.)
                })
                .is_err()
        );
        assert_eq!(editor.project, before);
        assert_eq!(editor.undo.len(), 1);
    }

    #[test]
    fn a_new_typed_command_after_undo_clears_redo() {
        let mut editor = Editor::new(Project::default());
        editor
            .apply(ModelCommand::AddElement {
                element: wall(1, 0.),
            })
            .unwrap();
        editor
            .apply(ModelCommand::MoveElements {
                ids: vec![1],
                offset: Point::new(1., 0.),
            })
            .unwrap();
        editor.undo();
        assert_eq!(editor.redo.len(), 1);
        editor
            .apply(ModelCommand::UpdateElement {
                element: wall(1, 8.),
            })
            .unwrap();
        assert!(editor.redo.is_empty());
        editor.undo();
        assert_eq!(editor.project.elements[0].shape, wall(1, 0.).shape);
    }

    #[test]
    fn delete_uses_the_typed_remove_command_and_undo_restores_order() {
        let mut editor = Editor::new(Project::default());
        for index in 0..3 {
            editor
                .apply(ModelCommand::AddElement {
                    element: wall(index + 1, index as f32 * 5.),
                })
                .unwrap();
        }
        let before = editor.project.clone();
        editor.set_selection(vec![2]);
        editor.delete();
        assert_eq!(editor.undo.len(), 4);
        assert_eq!(editor.project.elements.len(), 2);
        assert_eq!(editor.undo.last().unwrap().affected_ids(), vec![2]);
        editor.undo();
        assert_eq!(editor.project, before);
    }

    #[test]
    fn undo_history_keeps_affected_elements_only_and_caps_at_100() {
        let mut editor = Editor::new(Project::default());
        for index in 0..120 {
            editor
                .apply(ModelCommand::AddElement {
                    element: wall(index + 1, index as f32 * 3.),
                })
                .unwrap();
        }
        assert_eq!(editor.undo.len(), 100);
        assert!(
            editor
                .undo
                .iter()
                .all(|record| record.affected_ids().len() == 1)
        );
    }
}
