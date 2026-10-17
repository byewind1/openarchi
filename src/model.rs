use crate::stairs::StairSpec;
use crate::view_settings::{DesignOptions, Layer, PlanOptions, RenovationStatus};
use crate::wall::WallSettings;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}
impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn distance(self, other: Self) -> f32 {
        (self.x - other.x).hypot(self.y - other.y)
    }
    pub fn finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Story {
    pub id: u32,
    pub name: String,
    pub elevation: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Material {
    Concrete,
    Plaster,
    Timber,
}
impl Material {
    pub const ALL: [Self; 3] = [Self::Concrete, Self::Plaster, Self::Timber];
    pub fn name(self) -> &'static str {
        match self {
            Self::Concrete => "Concrete",
            Self::Plaster => "Warm plaster",
            Self::Timber => "Oak timber",
        }
    }
    pub fn color(self) -> [f32; 3] {
        match self {
            Self::Concrete => [0.57, 0.62, 0.63],
            Self::Plaster => [0.89, 0.87, 0.80],
            Self::Timber => [0.66, 0.44, 0.25],
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Shape {
    Wall {
        start: Point,
        end: Point,
        thickness: f32,
        height: f32,
    },
    Slab {
        min: Point,
        max: Point,
        thickness: f32,
    },
    Stair {
        spec: StairSpec,
    },
    Polyline {
        points: Vec<Point>,
        /// One optional exact circular arc per consecutive point pair.
        /// Empty is accepted for projects written before arc metadata existed.
        #[serde(default)]
        arcs: Vec<Option<ArcSegment>>,
    },
}

/// Exact circular-arc definition for a Polyline edge. Radius and branch flags
/// reconstruct the circle from the edge endpoints; no tessellation is stored.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcSegment {
    pub radius: f32,
    pub clockwise: bool,
    pub large_arc: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Element {
    pub id: u64,
    pub name: String,
    pub story: u32,
    pub material: Material,
    #[serde(default)]
    pub layer: Layer,
    #[serde(default)]
    pub renovation: RenovationStatus,
    /// Design option assignment; `None` belongs to the main model.
    #[serde(default)]
    pub design_option: Option<u32>,
    /// AC29-style wall settings. Older wall files without this field remain
    /// valid and receive defaults when the settings UI first edits the wall.
    #[serde(default)]
    pub wall_settings: Option<WallSettings>,
    pub shape: Shape,
}
impl Element {
    pub fn kind(&self) -> &'static str {
        match self.shape {
            Shape::Wall { .. } => "Wall",
            Shape::Slab { .. } => "Slab",
            Shape::Stair { .. } => "Stair",
            Shape::Polyline { .. } => "Polyline",
        }
    }
    pub fn thickness(&self) -> f32 {
        match self.shape {
            Shape::Wall { thickness, .. } | Shape::Slab { thickness, .. } => thickness,
            Shape::Stair { ref spec } => spec.slab_thickness,
            Shape::Polyline { .. } => 0.,
        }
    }
    pub fn area(&self) -> f32 {
        match self.shape {
            Shape::Wall {
                start, end, height, ..
            } => start.distance(end) * height,
            Shape::Slab { min, max, .. } => (max.x - min.x) * (max.y - min.y),
            Shape::Stair { ref spec } => spec.start.distance(spec.end) * spec.width,
            Shape::Polyline {
                ref points,
                ref arcs,
            } => points
                .windows(2)
                .enumerate()
                .map(|(index, pair)| {
                    arcs.get(index).and_then(|arc| *arc).map_or_else(
                        || pair[0].distance(pair[1]),
                        |arc| crate::geometry::arc_length(pair[0], pair[1], arc),
                    )
                })
                .sum(),
        }
    }
    pub fn translate(&mut self, d: Point) {
        match &mut self.shape {
            Shape::Wall { start, end, .. } => {
                start.x += d.x;
                start.y += d.y;
                end.x += d.x;
                end.y += d.y;
            }
            Shape::Slab { min, max, .. } => {
                min.x += d.x;
                min.y += d.y;
                max.x += d.x;
                max.y += d.y;
            }
            Shape::Stair { spec } => {
                spec.start.x += d.x;
                spec.start.y += d.y;
                spec.end.x += d.x;
                spec.end.y += d.y;
            }
            Shape::Polyline { points, .. } => {
                for point in points {
                    point.x += d.x;
                    point.y += d.y;
                }
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    #[serde(default)]
    pub grid_settings: crate::grids::GridSettings,
    #[serde(default)]
    pub guide_settings: crate::guides::GuideSettings,
    #[serde(default)]
    pub guides: Vec<crate::guides::PersistentGuide>,
    #[serde(default)]
    pub plan_options: PlanOptions,
    #[serde(default)]
    pub design_options: DesignOptions,
    pub schema_version: u32,
    pub name: String,
    pub next_id: u64,
    pub stories: Vec<Story>,
    pub elements: Vec<Element>,
}
impl Default for Project {
    fn default() -> Self {
        Self {
            grid_settings: crate::grids::GridSettings::default(),
            guide_settings: crate::guides::GuideSettings::default(),
            guides: vec![],
            plan_options: PlanOptions::default(),
            design_options: DesignOptions::default(),
            schema_version: 1,
            name: "Courtyard House".into(),
            next_id: 1,
            stories: vec![
                Story {
                    id: 0,
                    name: "Ground floor".into(),
                    elevation: 0.,
                },
                Story {
                    id: 1,
                    name: "First floor".into(),
                    elevation: 3.2,
                },
                Story {
                    id: 2,
                    name: "Roof level".into(),
                    elevation: 6.4,
                },
            ],
            elements: vec![],
        }
    }
}
impl Project {
    pub fn add(&mut self, story: u32, material: Material, shape: Shape) -> u64 {
        self.add_with_status(story, material, shape, RenovationStatus::New)
    }

    pub fn add_with_status(
        &mut self,
        story: u32,
        material: Material,
        shape: Shape,
        renovation: RenovationStatus,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let kind = match shape {
            Shape::Wall { .. } => "Wall",
            Shape::Slab { .. } => "Slab",
            Shape::Stair { .. } => "Stair",
            Shape::Polyline { .. } => "Polyline",
        };
        let layer = match kind {
            "Slab" => Layer::Structural,
            "Polyline" => Layer::Drafting,
            _ => Layer::Architectural,
        };
        let wall_settings = matches!(shape, Shape::Wall { .. }).then(WallSettings::default);
        self.elements.push(Element {
            id,
            name: format!("{kind} {id:03}"),
            story,
            material,
            layer,
            renovation,
            design_option: None,
            wall_settings,
            shape,
        });
        id
    }
    pub fn elevation(&self, story: u32) -> f32 {
        self.stories
            .iter()
            .find(|s| s.id == story)
            .map_or(0., |s| s.elevation)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("Unsupported project schema".into());
        }
        self.grid_settings.validate()?;
        if self.stories.is_empty() || self.stories.len() > 100 || self.elements.len() > 100_000 {
            return Err("Invalid project size".into());
        }
        let mut stories = std::collections::HashSet::new();
        for s in &self.stories {
            if !stories.insert(s.id) || !s.elevation.is_finite() || s.elevation.abs() > 10000. {
                return Err("Invalid story".into());
            }
        }
        self.guide_settings.validate()?;
        self.plan_options.validate()?;
        if self.guides.len() > 10000 {
            return Err("Too many guide lines".into());
        }
        for guide in &self.guides {
            if !stories.contains(&guide.story) {
                return Err("Invalid guide story".into());
            }
            guide.primitive.validate()?;
        }
        let mut ids = std::collections::HashSet::new();
        for e in &self.elements {
            if e.id == 0 || e.id >= self.next_id || !ids.insert(e.id) || !stories.contains(&e.story)
            {
                return Err("Invalid element identity or story".into());
            }
            let point_ok = |p: Point| p.finite() && p.x.abs() < 100000. && p.y.abs() < 100000.;
            let valid = match e.shape {
                Shape::Wall {
                    start,
                    end,
                    thickness,
                    height,
                } => {
                    point_ok(start)
                        && point_ok(end)
                        && start.distance(end) >= 0.01
                        && thickness.is_finite()
                        && (0.01..=5.).contains(&thickness)
                        && height.is_finite()
                        && (0.01..=100.).contains(&height)
                }
                Shape::Slab {
                    min,
                    max,
                    thickness,
                } => {
                    point_ok(min)
                        && point_ok(max)
                        && max.x - min.x >= 0.01
                        && max.y - min.y >= 0.01
                        && thickness.is_finite()
                        && (0.01..=5.).contains(&thickness)
                }
                Shape::Stair { ref spec } => {
                    point_ok(spec.start) && point_ok(spec.end) && spec.validate().is_ok()
                }
                Shape::Polyline {
                    ref points,
                    ref arcs,
                } => {
                    points.len() >= 2
                        && points.len() <= 10_000
                        && points.iter().all(|point| point_ok(*point))
                        && points
                            .windows(2)
                            .all(|pair| pair[0].distance(pair[1]) >= 0.01)
                        && (arcs.is_empty() || arcs.len() == points.len() - 1)
                        && arcs
                            .iter()
                            .flatten()
                            .all(|arc| arc.radius.is_finite() && arc.radius > 0.)
                        && arcs.iter().enumerate().all(|(index, arc)| {
                            arc.is_none_or(|arc| {
                                points.get(index).zip(points.get(index + 1)).is_some_and(
                                    |(a, b)| {
                                        let chord = a.distance(*b);
                                        arc.radius.is_finite()
                                            && arc.radius >= chord * 0.5 - 1e-4
                                            && crate::geometry::arc_center_sweep(*a, *b, arc)
                                                .is_some()
                                    },
                                )
                            })
                        })
                }
            };
            if !valid {
                return Err(format!("Invalid geometry: {}", e.name));
            }
            if matches!(e.shape, Shape::Wall { .. }) {
                if let Some(settings) = &e.wall_settings {
                    settings.validate()?;
                }
            }
        }
        Ok(())
    }
    pub fn demo() -> Self {
        let mut p = Self::default();
        p.add_with_status(
            0,
            Material::Concrete,
            Shape::Slab {
                min: Point::new(-0.2, -0.2),
                max: Point::new(12.2, 8.2),
                thickness: 0.28,
            },
            RenovationStatus::Existing,
        );
        for (index, (a, b)) in [
            ((0., 0.), (12., 0.)),
            ((12., 0.), (12., 8.)),
            ((12., 8.), (0., 8.)),
            ((0., 8.), (0., 0.)),
            ((7.5, 0.), (7.5, 3.)),
            ((7.5, 4.2), (7.5, 8.)),
            ((7.5, 4.), (12., 4.)),
            ((0., 5.2), (3., 5.2)),
            ((4.2, 5.2), (7.5, 5.2)),
        ]
        .into_iter()
        .enumerate()
        {
            p.add_with_status(
                0,
                Material::Plaster,
                Shape::Wall {
                    start: Point::new(a.0, a.1),
                    end: Point::new(b.0, b.1),
                    thickness: 0.22,
                    height: 3.,
                },
                if index == 4 {
                    RenovationStatus::Demolition
                } else {
                    RenovationStatus::Existing
                },
            );
        }
        p.add_with_status(
            0,
            Material::Timber,
            Shape::Slab {
                min: Point::new(0.3, -2.3),
                max: Point::new(7.2, -0.3),
                thickness: 0.16,
            },
            RenovationStatus::Existing,
        );
        // A small upper-floor balcony gives the default Trace reference visible
        // content in the demo project.
        p.add_with_status(
            1,
            Material::Timber,
            Shape::Slab {
                min: Point::new(8.4, -2.3),
                max: Point::new(12.0, -0.3),
                thickness: 0.16,
            },
            RenovationStatus::New,
        );
        p
    }
}
/// Integration boundary: future formats convert to/from the independent domain model.
/// No IFC, PLN, GSM or BSM implementation is loaded by this demo.
pub trait ModelAdapter {
    fn format_name(&self) -> &str;
    fn import(&self, bytes: &[u8]) -> Result<Project, String>;
    fn export(&self, project: &Project) -> Result<Vec<u8>, String>;
}
