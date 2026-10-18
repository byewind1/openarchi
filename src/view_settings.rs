use crate::model::{Element, Material};
use serde::{Deserialize, Serialize};

/// Archicad's default wall cut fill is a light neutral grey in floor plan.
const WALL_CUT_FILL: [u8; 3] = [211, 211, 211];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Layer {
    Site,
    Drafting,
    #[default]
    Architectural,
    Structural,
    Mechanical,
    Electrical,
    Rendering,
    Layouting,
}

impl Layer {
    pub const ALL: [Self; 8] = [
        Self::Site,
        Self::Drafting,
        Self::Architectural,
        Self::Structural,
        Self::Mechanical,
        Self::Electrical,
        Self::Rendering,
        Self::Layouting,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Site => "01 Site",
            Self::Drafting => "02 Drafting",
            Self::Architectural => "03 Architectural",
            Self::Structural => "04 Structural",
            Self::Mechanical => "05 Mechanical",
            Self::Electrical => "06 Electrical",
            Self::Rendering => "07 Rendering",
            Self::Layouting => "08 Layouting",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenovationStatus {
    #[default]
    Existing,
    Demolition,
    New,
}

impl RenovationStatus {
    pub fn name(self) -> &'static str {
        match self {
            Self::Existing => "Existing",
            Self::Demolition => "Demolition",
            Self::New => "New",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerCombination {
    Site,
    #[default]
    Drafting,
    PlansPreliminary,
    PlansApproval,
    PlansDetailed,
    PlansMechanical,
    PlansStructural,
    Rendering,
    Layouting,
    Show3DZonesAsSolid,
}

impl LayerCombination {
    pub const ALL: [Self; 10] = [
        Self::Site,
        Self::Drafting,
        Self::PlansPreliminary,
        Self::PlansApproval,
        Self::PlansDetailed,
        Self::PlansMechanical,
        Self::PlansStructural,
        Self::Rendering,
        Self::Layouting,
        Self::Show3DZonesAsSolid,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Site => "01 Site",
            Self::Drafting => "02 Drafting",
            Self::PlansPreliminary => "03 Plans - Preliminary",
            Self::PlansApproval => "04 Plans - Approval",
            Self::PlansDetailed => "05 Plans - Detailed",
            Self::PlansMechanical => "06 Plans - Mechanical",
            Self::PlansStructural => "07 Plans - Structural",
            Self::Rendering => "08 Rendering",
            Self::Layouting => "09 Layouting",
            Self::Show3DZonesAsSolid => "10 Show 3D Zones as Solid",
        }
    }

    pub fn shows(self, layer: Layer) -> bool {
        use Layer::*;
        match self {
            Self::Site => matches!(layer, Site | Architectural | Structural | Rendering),
            Self::Drafting => matches!(layer, Drafting | Architectural | Structural | Rendering),
            Self::PlansPreliminary | Self::PlansApproval | Self::PlansDetailed => true,
            Self::PlansMechanical => {
                matches!(layer, Drafting | Architectural | Structural | Mechanical)
            }
            Self::PlansStructural => matches!(layer, Architectural | Structural),
            Self::Rendering => matches!(layer, Architectural | Structural | Rendering),
            Self::Layouting => matches!(layer, Drafting | Layouting),
            Self::Show3DZonesAsSolid => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PenSet {
    ColorConceptual,
    Site500,
    #[default]
    Architectural100,
    BuildingPlans50,
    Detailing,
    Structural,
    Mechanical,
    Electrical,
    Grayscale,
}

impl PenSet {
    pub const ALL: [Self; 9] = [
        Self::ColorConceptual,
        Self::Site500,
        Self::Architectural100,
        Self::BuildingPlans50,
        Self::Detailing,
        Self::Structural,
        Self::Mechanical,
        Self::Electrical,
        Self::Grayscale,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::ColorConceptual => "01 Color Conceptual",
            Self::Site500 => "02 Site 500",
            Self::Architectural100 => "03 Architectural 100",
            Self::BuildingPlans50 => "04 Building plans 50",
            Self::Detailing => "05 Detailing",
            Self::Structural => "06 Structural",
            Self::Mechanical => "07 Mechanical",
            Self::Electrical => "08 Electrical",
            Self::Grayscale => "09 Grayscale",
        }
    }

    pub fn palette(self) -> PenPalette {
        match self {
            Self::ColorConceptual => PenPalette::new([48, 86, 142], Some([228, 238, 250]), 1.0),
            Self::Site500 => PenPalette::new([74, 102, 66], Some([226, 235, 216]), 1.0),
            Self::Architectural100 => PenPalette::new([52, 60, 66], None, 1.0),
            Self::BuildingPlans50 => PenPalette::new([35, 68, 96], Some([224, 236, 246]), 0.9),
            Self::Detailing => PenPalette::new([32, 32, 35], None, 0.9),
            Self::Structural => PenPalette::new([58, 64, 74], Some([228, 231, 236]), 1.05),
            Self::Mechanical => PenPalette::new([30, 94, 104], Some([220, 240, 240]), 1.0),
            Self::Electrical => PenPalette::new([128, 78, 28], Some([246, 235, 216]), 1.0),
            Self::Grayscale => PenPalette::new([70, 70, 70], Some([232, 232, 232]), 0.9),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenPalette {
    pub line: [u8; 3],
    pub fill_tint: Option<[u8; 3]>,
    pub line_scale: f32,
}

impl PenPalette {
    const fn new(line: [u8; 3], fill_tint: Option<[u8; 3]>, line_scale: f32) -> Self {
        Self {
            line,
            fill_tint,
            line_scale,
        }
    }

    pub fn line_for(self, part: PlanPart) -> [u8; 3] {
        match part {
            PlanPart::Wall => self.line,
            PlanPart::Slab => mix(self.line, [255, 255, 255], 0.18),
            PlanPart::Stair => mix(self.line, [235, 205, 150], 0.12),
            PlanPart::Polyline => mix(self.line, [0, 0, 0], 0.1),
            PlanPart::Opening => self.line,
        }
    }

    pub fn width_mm(self, part: PlanPart) -> f32 {
        let base = match part {
            PlanPart::Wall => 0.35,
            PlanPart::Slab => 0.18,
            PlanPart::Stair => 0.25,
            PlanPart::Polyline => 0.13,
            PlanPart::Opening => 0.25,
        };
        (base * self.line_scale).clamp(0.08, 1.2)
    }

    pub fn tint_fill(self, color: [u8; 3]) -> [u8; 3] {
        self.fill_tint.map_or(color, |tint| mix(color, tint, 0.34))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartialStructureDisplay {
    #[default]
    EntireModel,
    WithoutFinishes,
    CoreOnly,
    CoreOfLoadBearingOnly,
}

impl PartialStructureDisplay {
    pub const ALL: [Self; 4] = [
        Self::EntireModel,
        Self::WithoutFinishes,
        Self::CoreOnly,
        Self::CoreOfLoadBearingOnly,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::EntireModel => "Entire Model",
            Self::WithoutFinishes => "Without Finishes",
            Self::CoreOnly => "Core Only",
            Self::CoreOfLoadBearingOnly => "Core of Load-Bearing Elements Only",
        }
    }

    pub fn shows(self, material: Material) -> bool {
        match self {
            Self::EntireModel => true,
            Self::WithoutFinishes => material != Material::Plaster,
            Self::CoreOnly | Self::CoreOfLoadBearingOnly => material == Material::Concrete,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelViewOptions {
    Site,
    Drafting,
    #[default]
    BuildingPlans,
    CeilingPlans,
    SectionsElevations,
}

impl ModelViewOptions {
    pub const ALL: [Self; 5] = [
        Self::Site,
        Self::Drafting,
        Self::BuildingPlans,
        Self::CeilingPlans,
        Self::SectionsElevations,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Site => "01 Site",
            Self::Drafting => "02 Drafting",
            Self::BuildingPlans => "03 Building Plans",
            Self::CeilingPlans => "04 Ceiling Plans",
            Self::SectionsElevations => "05 Sections and Elevations",
        }
    }

    pub fn shows(self, part: PlanPart) -> bool {
        match self {
            Self::CeilingPlans => part != PlanPart::Slab,
            Self::SectionsElevations => {
                matches!(
                    part,
                    PlanPart::Wall | PlanPart::Polyline | PlanPart::Opening
                )
            }
            _ => true,
        }
    }

    pub fn fills(self, part: PlanPart) -> bool {
        match self {
            Self::Drafting | Self::SectionsElevations => false,
            Self::CeilingPlans => part != PlanPart::Stair,
            _ => true,
        }
    }

    pub fn draw_treads(self) -> bool {
        !matches!(self, Self::CeilingPlans | Self::SectionsElevations)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphicOverrides {
    #[default]
    NoOverrides,
    CeilingPlan,
    FirePlan,
    HighlightProblems,
    SamMembersEditedManually,
    SimplifiedPlan,
    SitePlan,
    StructuralPlan,
}

impl GraphicOverrides {
    pub const ALL: [Self; 8] = [
        Self::NoOverrides,
        Self::CeilingPlan,
        Self::FirePlan,
        Self::HighlightProblems,
        Self::SamMembersEditedManually,
        Self::SimplifiedPlan,
        Self::SitePlan,
        Self::StructuralPlan,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::NoOverrides => "No Overrides",
            Self::CeilingPlan => "Ceiling Plan",
            Self::FirePlan => "Fire Plan",
            Self::HighlightProblems => "Highlight Problems",
            Self::SamMembersEditedManually => "SAM Members Edited Manually",
            Self::SimplifiedPlan => "Simplified Plan",
            Self::SitePlan => "Site Plan",
            Self::StructuralPlan => "Structural Plan",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenovationFilter {
    #[default]
    ShowAllElements,
    ExistingPlan,
    DemolitionPlan,
    AfterDemolition,
    ChangePlan,
    NewConstruction,
    PlannedStatus,
}

impl RenovationFilter {
    pub const ALL: [Self; 7] = [
        Self::ShowAllElements,
        Self::ExistingPlan,
        Self::DemolitionPlan,
        Self::AfterDemolition,
        Self::ChangePlan,
        Self::NewConstruction,
        Self::PlannedStatus,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::ShowAllElements => "00 Show All Elements",
            Self::ExistingPlan => "01 Existing Plan",
            Self::DemolitionPlan => "02 Demolition Plan",
            Self::AfterDemolition => "03 After Demolition",
            Self::ChangePlan => "04 Change Plan",
            Self::NewConstruction => "05 New Construction",
            Self::PlannedStatus => "06 Planned Status",
        }
    }

    pub fn shows(self, status: RenovationStatus) -> bool {
        match self {
            Self::ShowAllElements | Self::ChangePlan | Self::PlannedStatus => true,
            Self::ExistingPlan | Self::DemolitionPlan => {
                matches!(
                    status,
                    RenovationStatus::Existing | RenovationStatus::Demolition
                )
            }
            Self::AfterDemolition | Self::NewConstruction => {
                matches!(status, RenovationStatus::Existing | RenovationStatus::New)
            }
        }
    }

    pub fn highlights_demolition(self) -> bool {
        matches!(self, Self::DemolitionPlan | Self::ChangePlan)
    }

    pub fn highlights_new(self) -> bool {
        matches!(
            self,
            Self::ChangePlan | Self::NewConstruction | Self::PlannedStatus
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DimensionStyle {
    Din,
    #[default]
    PlainMeter,
    PlainMillimeter,
    UsArchitect,
    UsBuilder,
    UsDetailing,
}

impl DimensionStyle {
    pub const ALL: [Self; 6] = [
        Self::Din,
        Self::PlainMeter,
        Self::PlainMillimeter,
        Self::UsArchitect,
        Self::UsBuilder,
        Self::UsDetailing,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Din => "DIN",
            Self::PlainMeter => "Plain Meter",
            Self::PlainMillimeter => "Plain Millimeter",
            Self::UsArchitect => "US Architect",
            Self::UsBuilder => "US Builder",
            Self::UsDetailing => "US Detailing",
        }
    }

    pub fn format_length(self, meters: f32) -> String {
        let meters = meters.max(0.);
        match self {
            Self::Din => format!("{:.0} mm", meters * 1000.),
            Self::PlainMeter => format!("{:.3} m", meters),
            Self::PlainMillimeter => format!("{:.0} mm", meters * 1000.),
            Self::UsArchitect => {
                let total_inches = meters * 39.370_08;
                let feet = (total_inches / 12.).floor();
                let inches = total_inches - feet * 12.;
                format!("{feet:.0}'-{inches:.2}\"")
            }
            Self::UsBuilder => format!("{:.2} ft", meters * 3.280_84),
            Self::UsDetailing => format!("{:.2}\"", meters * 39.370_08),
        }
    }

    pub fn format_signed_length(self, meters: f32) -> String {
        if meters < 0. {
            format!("-{}", self.format_length(-meters))
        } else {
            self.format_length(meters)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignOption {
    pub id: u32,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignOptions {
    #[serde(default)]
    pub options: Vec<DesignOption>,
    #[serde(default)]
    pub next_id: u32,
}

impl DesignOptions {
    pub fn create(&mut self, name: String) -> u32 {
        let id = self.next_id.max(1);
        self.next_id = id + 1;
        self.options.push(DesignOption { id, name });
        id
    }

    pub fn remove(&mut self, id: u32) {
        self.options.retain(|option| option.id != id);
    }

    pub fn name(&self, id: u32) -> Option<&str> {
        self.options
            .iter()
            .find(|option| option.id == id)
            .map(|option| option.name.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanPart {
    Wall,
    Slab,
    Stair,
    Polyline,
    Opening,
}

impl Element {
    pub fn plan_part(&self) -> PlanPart {
        match self.shape {
            crate::model::Shape::Wall { .. } => PlanPart::Wall,
            crate::model::Shape::Slab { .. } => PlanPart::Slab,
            crate::model::Shape::Stair { .. } => PlanPart::Stair,
            crate::model::Shape::Polyline { .. } => PlanPart::Polyline,
            crate::model::Shape::Opening { .. } => PlanPart::Opening,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanElementStyle {
    pub fill: Option<[u8; 3]>,
    pub line: [u8; 3],
    pub line_width_mm: f32,
    pub draw_treads: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanOptions {
    #[serde(default = "default_drawing_scale")]
    pub drawing_scale: u32,
    #[serde(default)]
    pub orientation_degrees: f32,
    #[serde(default)]
    pub pen_set: PenSet,
    #[serde(default)]
    pub partial_structure: PartialStructureDisplay,
    #[serde(default)]
    pub show_all_layers: bool,
    #[serde(default)]
    pub layer_combination: LayerCombination,
    #[serde(default)]
    pub model_view: ModelViewOptions,
    #[serde(default)]
    pub graphic_overrides: GraphicOverrides,
    #[serde(default)]
    pub renovation_filter: RenovationFilter,
    #[serde(default)]
    pub dimension_style: DimensionStyle,
    /// Active design option. `None` means Main Model Only.
    #[serde(default)]
    pub active_design_option: Option<u32>,
}

const fn default_drawing_scale() -> u32 {
    100
}

impl Default for PlanOptions {
    fn default() -> Self {
        Self {
            drawing_scale: default_drawing_scale(),
            orientation_degrees: 0.,
            pen_set: PenSet::default(),
            partial_structure: PartialStructureDisplay::default(),
            show_all_layers: false,
            layer_combination: LayerCombination::default(),
            model_view: ModelViewOptions::default(),
            graphic_overrides: GraphicOverrides::default(),
            renovation_filter: RenovationFilter::default(),
            dimension_style: DimensionStyle::default(),
            active_design_option: None,
        }
    }
}

pub const SCALE_PRESETS: [u32; 16] = [
    300, 75, 30, 150, 1, 2, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000,
];
pub const ORIENTATION_PRESETS: [f32; 6] = [3.22, 183.22, 0., 90., 180., 270.];
pub const ZOOM_PRESETS: [u32; 7] = [1000, 500, 200, 100, 50, 20, 10];

impl PlanOptions {
    pub fn validate(&self) -> Result<(), String> {
        if !self.orientation_degrees.is_finite()
            || !(-3600. ..=3600.).contains(&self.orientation_degrees)
        {
            return Err("Plan orientation must be finite".into());
        }
        if self.drawing_scale == 0 || self.drawing_scale > 10_000 {
            return Err("Drawing scale denominator must be between 1 and 10000".into());
        }
        Ok(())
    }

    pub fn normalized_orientation(&self) -> f32 {
        let value = self.orientation_degrees % 360.;
        if value < 0. { value + 360. } else { value }
    }

    pub fn element_visible(&self, element: &Element) -> bool {
        (self.show_all_layers || self.layer_combination.shows(element.layer))
            && self.partial_structure.shows(element.material)
            && self.renovation_filter.shows(element.renovation)
            && self.model_view.shows(element.plan_part())
            && match self.active_design_option {
                None => element.design_option.is_none(),
                Some(id) => element.design_option.is_none() || element.design_option == Some(id),
            }
    }

    pub fn style_for(&self, element: &Element, pen: PenPalette) -> Option<PlanElementStyle> {
        if !self.element_visible(element) {
            return None;
        }
        let part = element.plan_part();
        let base_fill = match part {
            PlanPart::Wall => Some(WALL_CUT_FILL),
            PlanPart::Slab => Some(match element.material {
                Material::Timber => [233, 219, 193],
                _ => [237, 238, 230],
            }),
            PlanPart::Stair => Some([218, 211, 194]),
            PlanPart::Polyline => None,
            PlanPart::Opening => None,
        };
        let mut fill = if self.model_view.fills(part) {
            base_fill.map(|color| pen.tint_fill(color))
        } else {
            None
        };
        let mut line = pen.line_for(part);
        let mut width = pen.width_mm(part);
        let mut draw_treads = part == PlanPart::Stair && self.model_view.draw_treads();

        match self.graphic_overrides {
            GraphicOverrides::NoOverrides | GraphicOverrides::SamMembersEditedManually => {}
            GraphicOverrides::CeilingPlan => {
                fill = Some([236, 239, 243]);
                line = [48, 70, 102];
            }
            GraphicOverrides::FirePlan => {
                fill = Some([247, 219, 216]);
                line = [176, 45, 42];
                width *= 1.12;
            }
            GraphicOverrides::HighlightProblems => {
                fill = None;
                line = [214, 116, 30];
                width *= 1.45;
            }
            GraphicOverrides::SimplifiedPlan => {
                fill = None;
                line = [92, 98, 104];
                width *= 0.68;
                draw_treads = false;
            }
            GraphicOverrides::SitePlan => {
                fill = Some([220, 231, 207]);
                line = [64, 98, 52];
            }
            GraphicOverrides::StructuralPlan => {
                fill = Some([204, 209, 214]);
                line = [56, 61, 68];
                if part == PlanPart::Polyline {
                    return None;
                }
            }
        }

        if self.renovation_filter.highlights_demolition()
            && element.renovation == RenovationStatus::Demolition
        {
            fill = Some([246, 205, 199]);
            line = [190, 48, 42];
            width *= 1.2;
        }
        if self.renovation_filter.highlights_new() && element.renovation == RenovationStatus::New {
            fill = Some([204, 234, 212]);
            line = [43, 126, 69];
            width *= 1.1;
        }

        Some(PlanElementStyle {
            fill,
            line,
            line_width_mm: width.clamp(0.05, 4.),
            draw_treads,
        })
    }
}

fn mix(a: [u8; 3], b: [u8; 3], amount: f32) -> [u8; 3] {
    let amount = amount.clamp(0., 1.);
    [
        (a[0] as f32 * (1. - amount) + b[0] as f32 * amount) as u8,
        (a[1] as f32 * (1. - amount) + b[1] as f32 * amount) as u8,
        (a[2] as f32 * (1. - amount) + b[2] as f32 * amount) as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Point, Shape};

    fn wall(layer: Layer, renovation: RenovationStatus) -> Element {
        Element {
            id: 1,
            name: "Wall 001".into(),
            story: 0,
            material: Material::Concrete,
            layer,
            renovation,
            design_option: None,
            wall_settings: None,
            shape: Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(1., 0.),
                thickness: 0.2,
                height: 3.,
            },
        }
    }

    #[test]
    fn options_gate_layers_partial_structure_and_renovation() {
        let mut options = PlanOptions {
            layer_combination: LayerCombination::Layouting,
            ..Default::default()
        };
        let element = wall(Layer::Structural, RenovationStatus::Demolition);
        assert!(!options.element_visible(&element));

        options.layer_combination = LayerCombination::PlansStructural;
        options.renovation_filter = RenovationFilter::DemolitionPlan;
        assert!(options.element_visible(&element));

        options.partial_structure = PartialStructureDisplay::CoreOnly;
        assert!(options.element_visible(&element));
        options.partial_structure = PartialStructureDisplay::WithoutFinishes;
        assert!(options.element_visible(&element));

        let mut plaster = element.clone();
        plaster.material = Material::Plaster;
        assert!(!options.element_visible(&plaster));
    }

    #[test]
    fn dimension_style_formats_the_same_length_in_declared_units() {
        assert_eq!(DimensionStyle::PlainMeter.format_length(2.5), "2.500 m");
        assert_eq!(
            DimensionStyle::PlainMillimeter.format_length(2.5),
            "2500 mm"
        );
        assert_eq!(DimensionStyle::UsBuilder.format_length(3.048), "10.00 ft");
        assert!(
            DimensionStyle::UsArchitect
                .format_length(0.3048)
                .contains('\'')
        );
    }

    #[test]
    fn renovation_filters_show_and_highlight_the_expected_statuses() {
        assert!(!RenovationFilter::AfterDemolition.shows(RenovationStatus::Demolition));
        assert!(RenovationFilter::DemolitionPlan.shows(RenovationStatus::Demolition));
        assert!(RenovationFilter::NewConstruction.shows(RenovationStatus::New));
        assert!(!RenovationFilter::ExistingPlan.shows(RenovationStatus::New));
        assert!(RenovationFilter::ChangePlan.highlights_demolition());
        assert!(RenovationFilter::ChangePlan.highlights_new());
    }

    #[test]
    fn default_wall_cut_fill_is_light_gray_for_every_material() {
        let options = PlanOptions::default();
        let pen = PenSet::default().palette();
        for material in Material::ALL {
            let mut element = wall(Layer::Architectural, RenovationStatus::Existing);
            element.material = material;
            let style = options.style_for(&element, pen).expect("wall is visible");
            assert_eq!(style.fill, Some([211, 211, 211]), "{material:?}");
        }
    }
}
