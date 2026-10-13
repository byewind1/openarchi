use crate::{geometry, model::*};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Marquee,
    Wall,
    Slab,
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
        (area.abs() > 1e-5).then_some(Self { vertices, scope, story, z_range: None })
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
    pub tool: Tool,
    pub marquee: Option<MarqueeArea>,
    pub anchor: Option<Point>,
    pub thickness: f32,
    pub height: f32,
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
            tool: Tool::Select,
            marquee: None,
            anchor: None,
            thickness: 0.22,
            height: 3.,
            material: Material::Plaster,
            snap: true,
            orthogonal: false,
            undo: vec![],
            redo: vec![],
            status: "Ready · Select an element or start drawing".into(),
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
        self.tool = t;
        self.anchor = None;
        self.status = match t {
            Tool::Select => "Select an element · Drag to move · Delete to remove",
            Tool::Marquee => "Marquee · Drag a rectangle or choose Polygon / Rotated Rectangle",
            Tool::Wall => "Wall · Click start, then end · Enter commits Tracker values",
            Tool::Slab => "Slab · Click two opposite corners · Enter commits Tracker values",
        }
        .into();
    }
    pub fn switch_story(&mut self, id: u32) {
        if self.project.stories.iter().any(|s| s.id == id) {
            self.story = id;
            self.anchor = None;
            self.selected = None;
            self.selected_ids.clear();
        }
    }
    pub fn click(&mut self, p: Point, tolerance: f32) {
        if !p.finite() || p.x.abs() >= 100000. || p.y.abs() >= 100000. {
            self.status = "Coordinates must be finite and within the local modeling range".into();
            return;
        }
        if self.tool == Tool::Marquee { return; }
        match self.tool {
            Tool::Select => {
                self.selected = geometry::hit_plan(&self.project, self.story, p, tolerance);
                self.selected_ids = self.selected.into_iter().collect();
            }
            _ => {
                if let Some(a) = self.anchor {
                    let shape = match self.tool {
                        Tool::Wall => {
                            if a.distance(p) < 0.01 {
                                self.status = "Wall must be at least 10 mm".into();
                                return;
                            }
                            Shape::Wall {
                                start: a,
                                end: p,
                                thickness: self.thickness,
                                height: self.height,
                            }
                        }
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
                        _ => unreachable!(),
                    };
                    let before = self.project.clone();
                    let id = self.project.add(self.story, self.material, shape);
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
    pub fn undo(&mut self) {
        if let Some(p) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.project, p));
            self.selected = None;
            self.selected_ids.clear();
            self.anchor = None;
            self.status = "Undo".into();
        }
    }
    pub fn redo(&mut self) {
        if let Some(p) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.project, p));
            self.selected = None;
            self.selected_ids.clear();
            self.anchor = None;
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
        let ids = self.project.elements.iter()
            .filter(|e| area.includes_story(e.story))
            .filter(|e| geometry::element_has_node_in_polygon(e, &area.vertices))
            .filter(|e| area.z_range.is_none_or(|range| geometry::element_overlaps_z_range(&self.project, e, range)))
            .map(|e| e.id)
            .collect();
        self.set_selection(ids);
        self.selection_ids().len()
    }
    pub fn move_selection(&mut self, delta: Point) {
        let ids = self.selection_ids();
        if ids.is_empty() || !delta.finite() { return; }
        let before = self.project.clone();
        for e in self.project.elements.iter_mut().filter(|e| ids.contains(&e.id)) {
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
        if !delta.finite() { return; }
        if let Some(area) = &mut self.marquee {
            for p in &mut area.vertices { p.x += delta.x; p.y += delta.y; }
        }
    }
    pub fn stretch_nodes_in_marquee(&mut self, delta: Point) -> Result<usize, String> {
        let Some(area) = self.marquee.clone() else { return Ok(0) };
        if !delta.finite() { return Err("Stretch vector must be finite".into()); }
        let before = self.project.clone();
        let mut changed = 0;
        for e in self.project.elements.iter_mut().filter(|e| area.includes_story(e.story)) {
            match &mut e.shape {
                Shape::Wall { start, end, .. } => {
                    let a = geometry::point_in_polygon(*start, &area.vertices);
                    let b = geometry::point_in_polygon(*end, &area.vertices);
                    if a { start.x += delta.x; start.y += delta.y; }
                    if b { end.x += delta.x; end.y += delta.y; }
                    if a || b { changed += 1; }
                }
                Shape::Slab { min, max, .. } => {
                    let points = [*min, Point::new(max.x, min.y), *max, Point::new(min.x, max.y)];
                    let moved: Vec<Point> = points.iter().map(|p| {
                        if geometry::point_in_polygon(*p, &area.vertices) { Point::new(p.x + delta.x, p.y + delta.y) } else { *p }
                    }).collect();
                    if moved != points {
                        let lo = moved.iter().fold(Point::new(f32::INFINITY, f32::INFINITY), |a, b| Point::new(a.x.min(b.x), a.y.min(b.y)));
                        let hi = moved.iter().fold(Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY), |a, b| Point::new(a.x.max(b.x), a.y.max(b.y)));
                        *min = lo;
                        *max = hi;
                        changed += 1;
                    }
                }
            }
        }
        if changed == 0 { return Ok(0); }
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
        self.clipboard = self.project.elements.iter()
            .filter(|e| ids.contains(&e.id)).cloned().collect();
        self.clipboard.len()
    }
    pub fn cut_selection(&mut self) -> usize {
        let count = self.copy_selection();
        if count > 0 { self.delete(); }
        count
    }
    pub fn paste(&mut self, offset: Point) -> usize {
        if self.clipboard.is_empty() || !offset.finite() { return 0; }
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
        let bounds: Vec<Point> = self.project.elements.iter().filter(|e| ids.contains(&e.id))
            .flat_map(|e| geometry::footprint(e)).collect();
        if bounds.is_empty() { return; }
        let center = bounds.iter().fold(Point::default(), |a, p| Point::new(a.x + p.x / bounds.len() as f32, a.y + p.y / bounds.len() as f32));
        let before = self.project.clone();
        for e in self.project.elements.iter_mut().filter(|e| ids.contains(&e.id)) {
            Self::transform_element(e, center, |p| if mirror_x { Point::new(2. * center.x - p.x, p.y) } else { Point::new(p.x, 2. * center.y - p.y) });
        }
        if self.project.validate().is_err() { self.project = before; return; }
        self.commit(before);
        self.status = if mirror_x { "Mirrored selection across X" } else { "Mirrored selection across Y" }.into();
    }
    pub fn rotate_selection_quarter_turn(&mut self, clockwise: bool) {
        let ids = self.selection_ids();
        let bounds: Vec<Point> = self.project.elements.iter().filter(|e| ids.contains(&e.id))
            .flat_map(|e| geometry::footprint(e)).collect();
        if bounds.is_empty() { return; }
        let center = bounds.iter().fold(Point::default(), |a, p| Point::new(a.x + p.x / bounds.len() as f32, a.y + p.y / bounds.len() as f32));
        let before = self.project.clone();
        for e in self.project.elements.iter_mut().filter(|e| ids.contains(&e.id)) {
            Self::transform_element(e, center, |p| {
                let x = p.x - center.x;
                let y = p.y - center.y;
                if clockwise { Point::new(center.x + y, center.y - x) } else { Point::new(center.x - y, center.y + x) }
            });
        }
        if self.project.validate().is_err() { self.project = before; return; }
        self.commit(before);
        self.status = "Rotated selection 90°".into();
    }
    fn transform_element(e: &mut Element, _center: Point, transform: impl Fn(Point) -> Point) {
        match &mut e.shape {
            Shape::Wall { start, end, .. } => { *start = transform(*start); *end = transform(*end); }
            Shape::Slab { min, max, .. } => {
                let points = [*min, Point::new(max.x, min.y), *max, Point::new(min.x, max.y)].map(transform);
                *min = points.iter().fold(Point::new(f32::INFINITY, f32::INFINITY), |a, p| Point::new(a.x.min(p.x), a.y.min(p.y)));
                *max = points.iter().fold(Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY), |a, p| Point::new(a.x.max(p.x), a.y.max(p.y)));
            }
        }
    }
    pub fn move_selection_to_story(&mut self, direction: i32) {
        let ids = self.selection_ids();
        if ids.is_empty() || direction == 0 { return; }
        let mut stories = self.project.stories.clone();
        stories.sort_by(|a, b| a.elevation.total_cmp(&b.elevation));
        let before = self.project.clone();
        for e in self.project.elements.iter_mut().filter(|e| ids.contains(&e.id)) {
            if let Some(index) = stories.iter().position(|s| s.id == e.story) {
                let next = index as i32 + direction.signum();
                if let Some(story) = usize::try_from(next).ok().and_then(|i| stories.get(i)) { e.story = story.id; }
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
