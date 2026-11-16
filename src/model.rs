use crate::layers::{ARCHICAD_LAYER, ElementLayer};
use crate::stairs::StairSpec;
use crate::view_settings::{DesignOptions, PlanOptions, RenovationStatus};
use crate::wall::WallSettings;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn distance(self, other: Self) -> f64 {
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
    pub elevation: f64,
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

/// Rectangular hosted opening kinds implemented by the first BIM slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpeningKind {
    Door,
    Window,
}

impl OpeningKind {
    pub const ALL: [Self; 2] = [Self::Door, Self::Window];

    pub fn name(self) -> &'static str {
        match self {
            Self::Door => "Door",
            Self::Window => "Window",
        }
    }

    pub fn defaults(self) -> OpeningDefaults {
        match self {
            Self::Door => OpeningDefaults {
                width: 0.9,
                height: 2.1,
                sill: 0.,
            },
            Self::Window => OpeningDefaults {
                width: 1.2,
                height: 1.4,
                sill: 0.9,
            },
        }
    }
}

/// Creation defaults loaded by the Info Box while a Door/Window tool is active.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpeningDefaults {
    pub width: f64,
    pub height: f64,
    pub sill: f64,
}

impl OpeningDefaults {
    pub fn validate(self) -> Result<(), String> {
        if !self.width.is_finite()
            || !self.height.is_finite()
            || !self.sill.is_finite()
            || !(0.05..=100.).contains(&self.width)
            || !(0.05..=100.).contains(&self.height)
            || self.sill < 0.
        {
            return Err("Opening width, height and sill must be positive values".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Shape {
    ConceptMass {
        spec: crate::concept_mass::ConceptMassSpec,
    },
    Zone {
        spec: crate::zones::ZoneSpec,
    },
    Roof {
        spec: crate::roof::RoofSpec,
    },
    Beam {
        start: Point,
        end: Point,
        width: f64,
        height: f64,
        top_offset: f64,
        #[serde(default, skip_serializing_if = "crate::beam::is_zero")]
        slant_angle: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        arc: Option<ArcSegment>,
    },
    Column {
        center: Point,
        #[serde(flatten)]
        section: crate::column::ColumnSection,
        height: f64,
        /// Bottom elevation relative to the home story, in metres.
        #[serde(default)]
        bottom_offset: f64,
        /// Counterclockwise rotation about the centered vertical axis, in degrees.
        #[serde(default)]
        axis_rotation: f64,
        /// Centered constant-section inclination; absent in legacy vertical files.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        inclination: Option<crate::column_inclination::ColumnInclination>,
        #[serde(
            default,
            skip_serializing_if = "crate::column_plan::ColumnPlanSettings::is_default"
        )]
        plan: crate::column_plan::ColumnPlanSettings,
    },
    Wall {
        start: Point,
        end: Point,
        thickness: f64,
        height: f64,
    },
    Slab {
        min: Point,
        max: Point,
        thickness: f64,
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
    /// Rectangular hosted opening. `offset` is the centre distance from the
    /// host wall reference-line start; `sill` is measured from the story base.
    Opening {
        host_wall: u64,
        #[serde(rename = "opening")]
        kind: OpeningKind,
        width: f64,
        height: f64,
        sill: f64,
        offset: f64,
    },
}

/// Exact circular-arc definition for a Polyline edge. Radius and branch flags
/// reconstruct the circle from the edge endpoints; no tessellation is stored.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcSegment {
    pub radius: f64,
    pub clockwise: bool,
    pub large_arc: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SlabContour {
    pub points: Vec<Point>,
    #[serde(default)]
    pub arcs: Vec<Option<ArcSegment>>,
    #[serde(default)]
    pub holes: Vec<SlabContour>,
    #[serde(default)]
    pub elevation: f64,
    #[serde(default)]
    pub edge_settings: Vec<SlabEdgeSettings>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlabEdgeSettings {
    #[serde(default = "slab_right_angle")]
    pub angle_degrees: f64,
    #[serde(default)]
    pub surface: Option<Material>,
}
fn slab_right_angle() -> f64 {
    90.
}
impl Default for SlabEdgeSettings {
    fn default() -> Self {
        Self {
            angle_degrees: 90.,
            surface: None,
        }
    }
}

fn positive_wall_side() -> bool {
    true
}

/// Per-element reference/outer-side geometry, separate from transferable defaults.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum WallContour {
    Arc {
        arc: ArcSegment,
    },
    Trapezoid {
        outer_start: Point,
        outer_end: Point,
        #[serde(default)]
        outer_nodes: Vec<Point>,
        #[serde(default = "positive_wall_side")]
        outer_positive: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Element {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worksheet: Option<u64>,
    pub id: u64,
    pub name: String,
    pub story: u32,
    pub material: Material,
    #[serde(default)]
    pub layer: ElementLayer,
    #[serde(default)]
    pub renovation: RenovationStatus,
    /// Design option assignment; `None` belongs to the main model.
    #[serde(default)]
    pub design_option: Option<u32>,
    /// AC29-style wall settings. Older wall files without this field remain
    /// valid and receive defaults when the settings UI first edits the wall.
    #[serde(default)]
    pub wall_settings: Option<WallSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall_contour: Option<WallContour>,
    /// Optional ordered straight perimeter; legacy slabs keep their rectangle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slab_contour: Option<SlabContour>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opening_joinery: Option<crate::opening_joinery::OpeningJoinery>,
    pub shape: Shape,
}
impl Element {
    pub fn authored_prism_rings(&self) -> Option<(Vec<Point>, Vec<Vec<Point>>)> {
        match &self.shape {
            Shape::ConceptMass { spec } => {
                let mut paths = spec.paths().into_iter();
                Some((paths.next().unwrap_or_default(), paths.collect()))
            }
            Shape::Zone { spec } => Some((spec.outline.clone(), spec.holes.clone())),
            _ => None,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self.shape {
            Shape::ConceptMass { .. } => "Concept mass",
            Shape::Zone { .. } => "Zone",
            Shape::Roof { .. } => "Roof",
            Shape::Beam { .. } => "Beam",
            Shape::Column { .. } => "Column",
            Shape::Wall { .. } => "Wall",
            Shape::Slab { .. } => "Slab",
            Shape::Stair { .. } => "Stair",
            Shape::Polyline { .. } => "Polyline",
            Shape::Opening { kind, .. } => kind.name(),
        }
    }
    pub fn thickness(&self) -> f64 {
        match self.shape {
            Shape::ConceptMass { .. } => 0.,
            Shape::Zone { .. } => 0.,
            Shape::Roof { ref spec } => spec.thickness,
            Shape::Beam { width, .. } => width,
            Shape::Column { section, .. } => match section {
                crate::column::ColumnSection::Rectangular { width, .. } => width,
                crate::column::ColumnSection::Circular { diameter } => diameter,
            },
            Shape::Wall { thickness, .. } | Shape::Slab { thickness, .. } => thickness,
            Shape::Stair { ref spec } => spec.slab_thickness,
            Shape::Polyline { .. } | Shape::Opening { .. } => 0.,
        }
    }
    pub fn area(&self) -> f64 {
        match self.shape {
            Shape::ConceptMass { ref spec } => spec.area(),
            Shape::Zone { ref spec } => spec.area(),
            Shape::Roof { ref spec } => spec.area(),
            Shape::Beam {
                start,
                end,
                width,
                arc,
                ..
            } => crate::beam::plan_area(start, end, width, arc),
            Shape::Column { section, .. } => section.area().unwrap_or(0.),
            Shape::Wall { height, .. } => crate::wall_geometry::reference_length(self) * height,
            Shape::Slab { .. } => crate::slab_geometry::actual_area(self),
            Shape::Stair { ref spec } => spec.plan_area_m2(),
            Shape::Polyline {
                ref points,
                ref arcs,
            } => crate::polyline_geometry::length(points, arcs).unwrap_or(0.),
            Shape::Opening { width, height, .. } => width * height,
        }
    }
    pub fn translate(&mut self, d: Point) {
        crate::wall_geometry::transform(self, Point::default(), 1., 0., false, d);
        if self.slab_contour.is_some() {
            crate::slab_geometry::transform(self, |p| Point::new(p.x + d.x, p.y + d.y), 1., false);
            return;
        }
        match &mut self.shape {
            Shape::ConceptMass { spec } => spec.transform(|p| Point::new(p.x + d.x, p.y + d.y)),
            Shape::Zone { spec } => spec.transform(|p| Point::new(p.x + d.x, p.y + d.y)),
            Shape::Roof { spec } => spec.translate(d),
            Shape::Column { center, .. } => {
                center.x += d.x;
                center.y += d.y;
            }
            Shape::Wall { start, end, .. } | Shape::Beam { start, end, .. } => {
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
            // Openings move along their host wall, which needs project context.
            // See `Project::translate_many`.
            Shape::Opening { .. } => {}
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    #[serde(
        default,
        skip_serializing_if = "crate::trace_document::TraceDocument::is_empty"
    )]
    pub trace: crate::trace_document::TraceDocument,
    #[serde(
        default,
        skip_serializing_if = "crate::plugins::archive::Archive::is_empty"
    )]
    pub job_archive: crate::plugins::archive::Archive,
    #[serde(
        default,
        skip_serializing_if = "crate::design_process::Journal::is_empty"
    )]
    pub design_process: crate::design_process::Journal,
    #[serde(
        default,
        skip_serializing_if = "crate::evidence::EvidenceRegister::is_empty"
    )]
    pub evidence: crate::evidence::EvidenceRegister,
    #[serde(default, skip_serializing_if = "crate::rules::RuleBook::is_empty")]
    pub rules: crate::rules::RuleBook,
    #[serde(default, skip_serializing_if = "crate::program::Program::is_empty")]
    pub program: crate::program::Program,
    #[serde(default, skip_serializing_if = "crate::site::SiteContext::is_unknown")]
    pub site: crate::site::SiteContext,
    #[serde(default)]
    pub drawings: crate::drawings::DrawingDocument,
    #[serde(default)]
    pub edit_state: crate::editing::EditState,
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
    /// AC29-style dynamic layers, categories and layer combinations.
    #[serde(default = "crate::layers::LayerDocument::template")]
    pub layers: crate::layers::LayerDocument,
    pub schema_version: u32,
    pub name: String,
    pub next_id: u64,
    pub stories: Vec<Story>,
    pub elements: Vec<Element>,
    /// Named transfer sets for AC29-style pick up / inject; stored with the
    /// project like the AC29 Element Transfer Settings dialog.
    #[serde(default)]
    pub transfer_settings: crate::transfer::TransferSettings,
}
impl Default for Project {
    fn default() -> Self {
        Self {
            trace: Default::default(),
            job_archive: Default::default(),
            design_process: Default::default(),
            evidence: crate::evidence::EvidenceRegister::default(),
            rules: crate::rules::RuleBook::default(),
            site: crate::site::SiteContext::default(),
            program: crate::program::Program::default(),
            drawings: crate::drawings::DrawingDocument::default(),
            edit_state: crate::editing::EditState::default(),
            grid_settings: crate::grids::GridSettings::default(),
            guide_settings: crate::guides::GuideSettings::default(),
            guides: vec![],
            plan_options: PlanOptions::default(),
            design_options: DesignOptions::default(),
            layers: crate::layers::LayerDocument::template(),
            schema_version: crate::precision::MODEL_SCHEMA_VERSION,
            name: "Courtyard House".into(),
            next_id: 1,
            transfer_settings: crate::transfer::TransferSettings::default(),
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
/// Context attached to the existing input validator, without duplicating it.
struct InputFailure(crate::derived_geometry::GeometryDiagnostic);
impl From<String> for InputFailure {
    fn from(message: String) -> Self {
        Self(crate::derived_geometry::GeometryDiagnostic::error(
            0,
            "INVALID_PROJECT_INPUT",
            "project",
            message,
        ))
    }
}
impl From<&str> for InputFailure {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}
impl InputFailure {
    fn element(id: u64, feature: &str, message: String) -> Self {
        Self(crate::derived_geometry::GeometryDiagnostic::error(
            id,
            "INVALID_ELEMENT_INPUT",
            feature,
            message,
        ))
    }
}

impl Project {
    /// Exact design-state comparison, excluding only validated immutable job
    /// receipts. Geometry, requirements, sources, decisions and locks still count.
    /// Full serialization/journal integrity hashes deliberately remain unchanged.
    pub fn same_model_state(&self, other: &Self) -> bool {
        if self.job_archive == other.job_archive {
            return self == other;
        }
        let mut model = self.clone();
        model.job_archive = other.job_archive.clone();
        &model == other
    }
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
            Shape::ConceptMass { .. } => "Concept mass",
            Shape::Zone { .. } => "Zone",
            Shape::Roof { .. } => "Roof",
            Shape::Beam { .. } => "Beam",
            Shape::Column { .. } => "Column",
            Shape::Wall { .. } => "Wall",
            Shape::Slab { .. } => "Slab",
            Shape::Stair { .. } => "Stair",
            Shape::Polyline { .. } => "Polyline",
            Shape::Opening { kind, .. } => kind.name(),
        };
        // AC29 places new elements on the active layer; the Archicad Layer
        // is the template default until a tool picks another layer.
        let layer = ElementLayer(ARCHICAD_LAYER);
        let wall_settings = matches!(shape, Shape::Wall { .. }).then(WallSettings::default);
        self.elements.push(Element {
            worksheet: self.plan_options.active_worksheet,
            id,
            name: format!("{kind} {id:03}"),
            story,
            material,
            layer,
            renovation,
            design_option: None,
            wall_settings,
            wall_contour: None,
            slab_contour: None,
            opening_joinery: None,
            shape,
        });
        id
    }
    pub fn elevation(&self, story: u32) -> f64 {
        self.stories
            .iter()
            .find(|s| s.id == story)
            .map_or(0., |s| s.elevation)
    }
    /// Translate several elements with project context. Openings slide along
    /// their host wall unless that wall is part of the same move, in which case
    /// the relative offset already follows the wall.
    pub fn translate_many(&mut self, ids: &[u64], delta: Point) {
        if !delta.finite() || ids.is_empty() {
            return;
        }
        let requested: std::collections::HashSet<u64> = ids.iter().copied().collect();
        let mut opening_offsets: Vec<(usize, f64)> = Vec::new();
        for (index, element) in self.elements.iter().enumerate() {
            if !requested.contains(&element.id) {
                continue;
            }
            let Shape::Opening {
                host_wall,
                width,
                offset,
                ..
            } = element.shape
            else {
                continue;
            };
            if requested.contains(&host_wall) {
                continue;
            }
            let Some(host) = self.elements.iter().find(|host| host.id == host_wall) else {
                continue;
            };
            let length = crate::wall_geometry::reference_length(host);
            if length > 1e-6
                && let Some((center, _)) = crate::wall_geometry::point_tangent_at(host, offset)
            {
                let next_point = Point::new(center.x + delta.x, center.y + delta.y);
                let half = width * 0.5;
                let high = (length - half).max(half);
                let next = crate::wall_geometry::project_station(host, next_point)
                    .0
                    .clamp(half, high);
                opening_offsets.push((index, next));
            }
        }
        for element in self.elements.iter_mut() {
            if requested.contains(&element.id) && !matches!(element.shape, Shape::Opening { .. }) {
                element.translate(delta);
            }
        }
        for (index, next) in opening_offsets {
            if let Shape::Opening { offset, .. } = &mut self.elements[index].shape {
                *offset = next;
            }
        }
    }
    /// Validate a candidate reference line for an existing wall without
    /// mutating the project. Wall-body geometry, joins and 3D are derived from
    /// these endpoints, so editing only has to keep the reference line valid
    /// for the wall itself and for hosted openings.
    pub fn validate_wall_reference_line(
        &self,
        id: u64,
        start: Point,
        end: Point,
    ) -> Result<(), String> {
        let Some(wall) = self.elements.iter().find(|element| element.id == id) else {
            return Err("Missing wall".into());
        };
        if !matches!(wall.shape, Shape::Wall { .. }) {
            return Err("Element is not a wall".into());
        }
        let finite = crate::precision::GeometryContext::point_in_range;
        if !finite(start) || !finite(end) {
            return Err("Wall reference line must use finite points".into());
        }
        let mut candidate = wall.clone();
        if let Shape::Wall {
            start: a, end: b, ..
        } = &mut candidate.shape
        {
            *a = start;
            *b = end;
        }
        crate::wall_geometry::validate(&candidate)?;
        let length = crate::wall_geometry::reference_length(&candidate);
        if !crate::precision::segment_meets_minimum(
            start,
            end,
            crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
        ) {
            return Err("Wall must stay at least 0.01 m long".into());
        }
        for element in &self.elements {
            let Shape::Opening {
                host_wall,
                width,
                offset,
                ..
            } = element.shape
            else {
                continue;
            };
            if host_wall != id {
                continue;
            }
            let low = offset - width * 0.5;
            let high = offset + width * 0.5;
            if low < -1e-4 || high > length + 1e-4 {
                return Err(format!(
                    "Opening {} would be outside its host wall",
                    element.name
                ));
            }
        }
        Ok(())
    }
    /// Compatibility validation with unchanged legacy input messages.
    pub fn validate(&self) -> Result<(), String> {
        self.validate_input_only().map_err(|d| d.0.message)?;
        crate::geometry_validation::inspect(self).into_result()
    }
    /// One structural/input/source report; no model, history or ID is changed.
    pub fn validation_report(&self) -> crate::geometry_validation::GeometryReport {
        match self.validate_input_only() {
            Err(d) => crate::geometry_validation::GeometryReport {
                diagnostics: vec![d.0],
                ..Default::default()
            },
            Ok(()) => {
                let mut r = crate::geometry_validation::inspect(self);
                r.input_checked = true;
                r
            }
        }
    }
    pub(crate) fn validate_input_diagnostic(
        &self,
    ) -> Result<(), crate::derived_geometry::GeometryDiagnostic> {
        self.validate_input_only().map_err(|failure| failure.0)
    }
    fn validate_input_only(&self) -> Result<(), InputFailure> {
        for e in &self.elements {
            if let Some(settings) = e.opening_joinery {
                if !matches!(e.shape, Shape::Opening { .. }) {
                    return Err(InputFailure::element(
                        e.id,
                        "joinery",
                        "Joinery requires a door or window".into(),
                    ));
                }
                settings
                    .validate()
                    .map_err(|m| InputFailure::element(e.id, "joinery", m))?;
                if let Shape::Opening {
                    width,
                    height,
                    kind,
                    host_wall,
                    ..
                } = e.shape
                {
                    let clear = width - 2. * settings.frame_width;
                    let leaves = if settings.door_style == crate::opening_joinery::DoorStyle::Double
                    {
                        2.
                    } else {
                        1.
                    };
                    if settings.frame_width * 2. >= width.min(height)
                        || (kind == OpeningKind::Door
                            && settings.leaf_thickness >= clear / leaves * 0.5)
                    {
                        return Err(InputFailure::element(
                            e.id,
                            "joinery",
                            "Opening frame or leaf does not fit the opening".into(),
                        ));
                    }
                    if self
                        .elements
                        .iter()
                        .any(|h| h.id == host_wall && h.wall_contour.is_some())
                    {
                        return Err(InputFailure::element(
                            e.id,
                            "joinery",
                            "Joinery requires a straight wall".into(),
                        ));
                    }
                }
            }
        }
        self.trace.validate()?;
        self.design_process.validate()?;
        self.site.validate()?;
        self.program.validate()?;
        self.evidence.validate()?;
        self.rules.validate()?;
        crate::constraints::validate_project(self)?;
        crate::site::boundary::outline(self)?;
        crate::site::boundary::outline_for(self, crate::site::boundary::Role::ControlRegion)?;
        self.drawings.validate(&self.stories)?;
        crate::external::validate(self)?;
        if self.schema_version != crate::precision::MODEL_SCHEMA_VERSION {
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
            (|| -> Result<(), String> {
                if let Some(sheet) = e.worksheet {
                    if !self.drawings.worksheet_exists(sheet)
                        || !matches!(e.shape, Shape::Polyline { .. })
                    {
                        return Err("Worksheets contain only 2D drawing elements".into());
                    }
                }
                if e.id == 0
                    || e.id >= self.next_id
                    || !ids.insert(e.id)
                    || !stories.contains(&e.story)
                {
                    return Err("Invalid element identity or story".into());
                }
                if let Some(contour) = &e.slab_contour {
                    if !matches!(e.shape, Shape::Slab { .. }) {
                        return Err("Invalid slab contour".into());
                    }
                    crate::slab_geometry::validate_contour(contour)?;
                    crate::slab_geometry::bottom_contour(e)?;
                    let (a, b) = crate::slab_geometry::contour_bounds(contour);
                    if let Shape::Slab { min, max, .. } = e.shape {
                        if a.distance(min) > 1e-4 || b.distance(max) > 1e-4 {
                            return Err("Invalid slab contour bounds".into());
                        }
                    }
                }
                let point_ok = crate::precision::GeometryContext::point_in_range;
                let valid = match e.shape {
                    Shape::ConceptMass { ref spec } => {
                        spec.validate()?;
                        true
                    }
                    Shape::Zone { ref spec } => {
                        spec.validate()?;
                        true
                    }
                    Shape::Roof { ref spec } => {
                        spec.validate()?;
                        true
                    }
                    Shape::Beam { top_offset, .. } => {
                        top_offset.is_finite()
                            && (-100. ..=100.).contains(&top_offset)
                            && crate::beam::geometry(self, e).is_some_and(|g| g.solid().is_ok())
                    }
                    Shape::Column {
                        center,
                        inclination,
                        ..
                    } => {
                        point_ok(center)
                            && if inclination.is_some() {
                                crate::column::inclined_solid(self, e)
                                    .is_some_and(|solid| solid.mesh().is_ok())
                            } else {
                                crate::column::solid(self, e)
                                    .is_some_and(|solid| solid.mesh().is_ok())
                            }
                    }
                    Shape::Wall {
                        start,
                        end,
                        thickness,
                        height,
                    } => {
                        point_ok(start)
                            && point_ok(end)
                            && crate::precision::segment_meets_minimum(
                                start,
                                end,
                                crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
                            )
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
                            && crate::precision::axis_meets_minimum(
                                min.x,
                                max.x,
                                crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
                            )
                            && crate::precision::axis_meets_minimum(
                                min.y,
                                max.y,
                                crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
                            )
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
                            && points.windows(2).all(|pair| {
                                crate::precision::segment_meets_minimum(
                                    pair[0],
                                    pair[1],
                                    crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
                                )
                            })
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
                    Shape::Opening {
                        width,
                        height,
                        sill,
                        offset,
                        ..
                    } => {
                        width.is_finite()
                            && (0.05..=100.).contains(&width)
                            && height.is_finite()
                            && (0.05..=100.).contains(&height)
                            && sill.is_finite()
                            && (0. ..=100.).contains(&sill)
                            && offset.is_finite()
                            && offset.abs() < 100_000.
                    }
                };
                if !valid {
                    if matches!(e.shape, Shape::Beam { .. }) {
                        return Err("Invalid beam dimensions or position".into());
                    }
                    if matches!(e.shape, Shape::Column { .. }) {
                        return Err("Invalid column dimensions or position".into());
                    }
                    return Err(format!("Invalid geometry: {}", e.name));
                }
                crate::wall_geometry::validate(e)?;
                if matches!(e.shape, Shape::Wall { .. })
                    && let Some(settings) = &e.wall_settings
                {
                    settings.validate()?;
                }
                Ok(())
            })()
            .map_err(|m| InputFailure::element(e.id, "parameters", m))?;
        }

        let mut walls = std::collections::HashMap::new();
        for element in &self.elements {
            if let Shape::Wall { height, .. } = element.shape {
                let (bottom_offset, top_offset) = element
                    .wall_settings
                    .as_ref()
                    .map_or((0., 0.), |settings| settings.vertical_offsets());
                walls.insert(
                    element.id,
                    (
                        element.story,
                        crate::wall_geometry::reference_length(element),
                        height,
                        bottom_offset,
                        top_offset,
                    ),
                );
            }
        }
        let mut spans: std::collections::HashMap<u64, Vec<(&str, f64, f64)>> =
            std::collections::HashMap::new();
        for element in &self.elements {
            let Shape::Opening {
                host_wall,
                width,
                height,
                sill,
                offset,
                ..
            } = element.shape
            else {
                continue;
            };
            (|| -> Result<(), String> {
                let Some(&(host_story, length, wall_height, bottom_offset, top_offset)) =
                    walls.get(&host_wall)
                else {
                    return Err(format!(
                        "Opening {} references a missing host wall",
                        element.name
                    ));
                };
                if host_story != element.story {
                    return Err(format!(
                        "Opening {} must be on the same story as its host wall",
                        element.name
                    ));
                }
                let low = offset - width * 0.5;
                let high = offset + width * 0.5;
                if length < 1e-6 || low < -1e-4 || high > length + 1e-4 {
                    return Err(format!("Opening {} is outside its host wall", element.name));
                }
                if sill < bottom_offset - 1e-4 {
                    return Err(format!("Opening {} is below its host wall", element.name));
                }
                if sill + height > wall_height + top_offset + 1e-4 {
                    return Err(format!(
                        "Opening {} is taller than its host wall",
                        element.name
                    ));
                }
                spans
                    .entry(host_wall)
                    .or_default()
                    .push((element.name.as_str(), low, high));
                Ok(())
            })()
            .map_err(|m| InputFailure::element(element.id, "host", m))?;
        }

        for (host_wall, mut list) in spans {
            list.sort_by(|a, b| a.1.total_cmp(&b.1));
            for pair in list.windows(2) {
                if pair[1].1 < pair[0].2 - 1e-4 {
                    return Err(InputFailure::element(
                        host_wall,
                        "openings",
                        format!(
                            "Overlapping openings on wall {host_wall}: {} and {}",
                            pair[0].0, pair[1].0
                        ),
                    ));
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
