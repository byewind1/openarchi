use serde::{Deserialize, Serialize};

pub trait WallChoice: Copy + PartialEq + 'static {
    const ALL: &'static [Self];
    fn name(self) -> &'static str;
}

macro_rules! wall_choice {
    ($name:ident, $default:ident => $default_label:literal { $($variant:ident => $label:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name {
            #[default]
            $default,
            $($variant),+
        }

        impl WallChoice for $name {
            const ALL: &'static [Self] = &[Self::$default, $(Self::$variant),+];

            fn name(self) -> &'static str {
                match self {
                    Self::$default => $default_label,
                    $(Self::$variant => $label),+
                }
            }
        }
    };
}

wall_choice!(WallReferenceLine, Center => "Center" {
    Inside => "Inside",
    Outside => "Outside",
});
wall_choice!(WallGeometryMethod, Single => "Single" {
    Continuous => "Continuous",
    Polygonal => "Polygonal",
    Rectangular => "Rectangular",
});
wall_choice!(WallProfileOrigin, Center => "Center" {
    Top => "Top",
    Bottom => "Bottom",
});
wall_choice!(WallStoryDisplay, AllRelevantStories => "All Relevant Stories" {
    HomeStoryOnly => "Home Story Only",
    Custom => "Custom",
});
wall_choice!(WallFloorPlanDisplay, Projected => "Projected" {
    CutOnly => "Cut Only",
    Symbolic => "Symbolic",
});
wall_choice!(WallProjectionRange, ToFloorPlanRange => "To Floor Plan Range" {
    EntireModel => "Entire Model",
    LowerStory => "Lower Story",
});
wall_choice!(WallUncutLines, Solid => "Solid" {
    Dashed => "Dashed",
    None => "None",
});
wall_choice!(WallEndLines, Both => "Both" {
    StartOnly => "Start Only",
    EndOnly => "End Only",
    None => "None",
});
wall_choice!(WallZoneRelation, ZoneBoundary => "Zone Boundary" {
    NoRelation => "No Relation",
    Manual => "Manual",
});
wall_choice!(StructuralFunction, Undefined => "Undefined" {
    Bearing => "Bearing",
    NonBearing => "Non-Bearing",
    Shear => "Shear",
    NonStructural => "Non-Structural",
});
wall_choice!(StructuralModelType, Shell => "Shell" {
    Beam => "Beam",
    Column => "Column",
    Plate => "Plate",
});
wall_choice!(SupportCondition, Rigid => "Rigid" {
    Free => "Free",
    Hinged => "Hinged",
    Fixed => "Fixed",
});
wall_choice!(ClassificationPosition, Undefined => "Undefined" {
    Interior => "Interior",
    Exterior => "Exterior",
    Above => "Above",
    Below => "Below",
});
wall_choice!(OptionStatus, MainModel => "Main Model" {
    DesignOption => "Design Option",
    NotSet => "Not Set",
});
wall_choice!(RenovationFilterVisibility, AllRelevantFilters => "All Relevant Filters" {
    HomeFilterOnly => "Home Filter Only",
    Hidden => "Hidden",
});
wall_choice!(CombustibleRating, Undefined => "Undefined" {
    Combustible => "Combustible",
    NonCombustible => "Non-Combustible",
});
wall_choice!(WallConstructionType, Undefined => "Undefined" {
    Brick => "Brick",
    Concrete => "Concrete",
    Timber => "Timber",
    Steel => "Steel",
    Composite => "Composite",
});
wall_choice!(WallConstructionTechnology, Undefined => "Undefined" {
    CastInPlace => "Cast in Place",
    Precast => "Precast",
    Masonry => "Masonry",
    Framed => "Framed",
});

impl WallReferenceLine {
    pub fn preset_offset(self, thickness: f32) -> f32 {
        match self {
            Self::Center => 0.,
            Self::Inside => -thickness * 0.5,
            Self::Outside => thickness * 0.5,
        }
    }
}

impl WallGeometryMethod {
    /// Status-bar hint for the active drawing method. Keeping the text on the
    /// method makes the four Info Box buttons drive four real input models.
    pub fn status_key(self) -> &'static str {
        match self {
            Self::Single => "Wall · Single · Click start, then end",
            Self::Continuous => {
                "Wall · Continuous · Click nodes · right-click, double-click or Enter to finish"
            }
            Self::Polygonal => {
                "Wall · Polygonal · Click nodes · click the start or right-click/double-click to close"
            }
            Self::Rectangular => "Wall · Rectangular · Click two diagonal corners",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallGeometrySettings {
    pub top_link_offset: i32,
    pub top_offset: f32,
    pub bottom_offset: f32,
    pub to_project_zero: f32,
    pub geometry_method: WallGeometryMethod,
    pub reference_line: WallReferenceLine,
    pub reference_offset: f32,
}

impl Default for WallGeometrySettings {
    fn default() -> Self {
        Self {
            top_link_offset: 1,
            top_offset: 0.,
            bottom_offset: 0.,
            to_project_zero: 0.,
            geometry_method: WallGeometryMethod::Single,
            reference_line: WallReferenceLine::Center,
            reference_offset: 0.,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallProfileSettings {
    pub name: String,
    pub origin: WallProfileOrigin,
    pub horizontal_offset: f32,
    pub modifier_enabled: bool,
    pub modifier_top: f32,
    pub modifier_bottom: f32,
    pub modifier_left: f32,
    pub modifier_right: f32,
}

impl Default for WallProfileSettings {
    fn default() -> Self {
        Self {
            name: "Generic Wall/Shell".into(),
            origin: WallProfileOrigin::Center,
            horizontal_offset: 0.,
            modifier_enabled: false,
            modifier_top: 0.,
            modifier_bottom: 0.,
            modifier_left: 0.,
            modifier_right: 0.,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallFloorPlanSettings {
    pub show_on_stories: WallStoryDisplay,
    pub floor_plan_display: WallFloorPlanDisplay,
    pub show_projection: WallProjectionRange,
    pub override_cut_fill_pens: bool,
    pub cut_fill_pen: String,
    pub uncut_lines: WallUncutLines,
    pub uncut_line_weight_mm: f32,
    pub uncut_line_pen_index: u8,
    pub wall_end_lines: WallEndLines,
}

impl Default for WallFloorPlanSettings {
    fn default() -> Self {
        Self {
            show_on_stories: WallStoryDisplay::AllRelevantStories,
            floor_plan_display: WallFloorPlanDisplay::Projected,
            show_projection: WallProjectionRange::ToFloorPlanRange,
            override_cut_fill_pens: false,
            cut_fill_pen: "None".into(),
            uncut_lines: WallUncutLines::Solid,
            uncut_line_weight_mm: 0.18,
            uncut_line_pen_index: 87,
            wall_end_lines: WallEndLines::Both,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallModelSettings {
    pub override_surfaces: bool,
    pub left_surface: String,
    pub right_surface: String,
    pub end_surface: String,
    pub junction_order: u16,
    pub end_surfaces_override_using_adjoining_walls: bool,
    pub align_texture_to_wall_edges: bool,
    pub texture_alignment_x: f32,
    pub texture_alignment_y: f32,
    pub texture_alignment_rotation: f32,
    pub relation_to_zones: WallZoneRelation,
}

impl Default for WallModelSettings {
    fn default() -> Self {
        Self {
            override_surfaces: false,
            left_surface: "Stucco - Yellow Rough".into(),
            right_surface: "Stucco - Yellow Rough".into(),
            end_surface: "Stucco - White Rough".into(),
            junction_order: 8,
            end_surfaces_override_using_adjoining_walls: false,
            align_texture_to_wall_edges: false,
            texture_alignment_x: 0.,
            texture_alignment_y: 0.,
            texture_alignment_rotation: 0.,
            relation_to_zones: WallZoneRelation::ZoneBoundary,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallStructuralSettings {
    pub structural_function: StructuralFunction,
    pub structural_member: bool,
    pub ignore_small_openings: bool,
    pub stretch_and_cutback: bool,
    pub offset_to_structural_model: bool,
    pub structural_offset_ez: f32,
    pub model_type: StructuralModelType,
    pub allow_rule_based_modifications: bool,
    pub supports: [SupportCondition; 6],
}

impl Default for WallStructuralSettings {
    fn default() -> Self {
        Self {
            structural_function: StructuralFunction::Undefined,
            structural_member: true,
            ignore_small_openings: true,
            stretch_and_cutback: true,
            offset_to_structural_model: true,
            structural_offset_ez: 0.,
            model_type: StructuralModelType::Shell,
            allow_rule_based_modifications: true,
            supports: [SupportCondition::Rigid; 6],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallClassificationSettings {
    pub property_id: String,
    pub position: ClassificationPosition,
    pub option_status: OptionStatus,
    pub show_on_renovation_filter: RenovationFilterVisibility,
    pub fire_resistance_rating: String,
    pub combustible: CombustibleRating,
    pub thermal_transmittance: f32,
    pub sound_transmission_class: String,
    pub construction_type: WallConstructionType,
    pub technology: WallConstructionTechnology,
    pub concrete_cover_at_main_bars: f32,
    pub lifting_weight: f32,
    pub environmental_class: String,
    pub service_life_years: u32,
    pub stored_energy_kwh_m3: f32,
    pub brick_type: String,
    pub brick_h_size: f32,
    pub brick_w_size: f32,
    pub brick_l_size: f32,
    pub mortar_thickness: f32,
    pub brick_volume: f32,
    pub bricks_to_purchase: u32,
    pub sub_ceiling_thickness: f32,
    pub sub_floor_thickness: f32,
    pub suspended_ceiling_level: f32,
    pub raised_floor_level: f32,
    pub interior_height: f32,
    pub interior_height_detailed: f32,
    pub cost_per_volume: f32,
    pub total_cost_of_volume: f32,
    pub total_cost_usd: f32,
    pub total_cost_symbol: f32,
    pub dynamic_id_by_classification: bool,
    pub dynamic_id_value: String,
    pub classification_id: String,
    pub classification_name: String,
    pub strength_grade: String,
    pub ifc_type: String,
    pub global_id_attribute: String,
    pub name_attribute: String,
    pub predefined_type_attribute: String,
}

impl Default for WallClassificationSettings {
    fn default() -> Self {
        Self {
            property_id: "SW - 014".into(),
            position: ClassificationPosition::Undefined,
            option_status: OptionStatus::MainModel,
            show_on_renovation_filter: RenovationFilterVisibility::AllRelevantFilters,
            fire_resistance_rating: "Undefined".into(),
            combustible: CombustibleRating::Undefined,
            thermal_transmittance: 0.,
            sound_transmission_class: "Undefined".into(),
            construction_type: WallConstructionType::Undefined,
            technology: WallConstructionTechnology::Undefined,
            concrete_cover_at_main_bars: 0.,
            lifting_weight: 0.,
            environmental_class: "Undefined".into(),
            service_life_years: 0,
            stored_energy_kwh_m3: 0.,
            brick_type: "Undefined".into(),
            brick_h_size: 0.,
            brick_w_size: 0.,
            brick_l_size: 0.,
            mortar_thickness: 0.,
            brick_volume: 0.,
            bricks_to_purchase: 0,
            sub_ceiling_thickness: 0.,
            sub_floor_thickness: 0.,
            suspended_ceiling_level: 0.,
            raised_floor_level: 0.,
            interior_height: 0.,
            interior_height_detailed: 0.,
            cost_per_volume: 0.,
            total_cost_of_volume: 0.,
            total_cost_usd: 0.,
            total_cost_symbol: 0.,
            dynamic_id_by_classification: true,
            dynamic_id_value: "SW-WALL-0014".into(),
            classification_id: "Wall".into(),
            classification_name: "Wall".into(),
            strength_grade: "Undefined".into(),
            ifc_type: "IfcWall".into(),
            global_id_attribute: "Not set".into(),
            name_attribute: "Wall".into(),
            predefined_type_attribute: "Not defined".into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallSettings {
    pub geometry: WallGeometrySettings,
    pub profile: WallProfileSettings,
    pub floor_plan: WallFloorPlanSettings,
    pub model: WallModelSettings,
    pub structural: WallStructuralSettings,
    pub classification: WallClassificationSettings,
}

impl WallSettings {
    pub fn reference_offset(&self) -> f32 {
        self.geometry.reference_offset
    }

    /// Offset of the wall body from its reference line. The reference-line
    /// location selects the default inside/outside offset; the numeric
    /// reference offset is added to it. The line itself stays on the model's
    /// start-end anchors, matching Archicad's fixed reference line.
    pub fn body_offset(&self, thickness: f32) -> f32 {
        self.geometry.reference_line.preset_offset(thickness) + self.geometry.reference_offset
    }

    pub fn vertical_offsets(&self) -> (f32, f32) {
        (self.geometry.bottom_offset, self.geometry.top_offset)
    }

    pub fn validate(&self) -> Result<(), String> {
        let finite = |value: f32, label: &str| {
            if value.is_finite() {
                Ok(())
            } else {
                Err(format!("Wall setting {label} must be finite"))
            }
        };
        let in_range = |value: f32, min: f32, max: f32, label: &str| {
            finite(value, label)?;
            if (min..=max).contains(&value) {
                Ok(())
            } else {
                Err(format!("Wall setting {label} is out of range"))
            }
        };
        if !(-20..=20).contains(&self.geometry.top_link_offset) {
            return Err("Wall top link is out of range".into());
        }
        in_range(self.geometry.top_offset, -100., 100., "top offset")?;
        in_range(self.geometry.bottom_offset, -100., 100., "bottom offset")?;
        in_range(
            self.geometry.to_project_zero,
            -10000.,
            10000.,
            "project zero",
        )?;
        in_range(
            self.geometry.reference_offset,
            -10.,
            10.,
            "reference line offset",
        )?;
        in_range(self.profile.horizontal_offset, -10., 10., "profile offset")?;
        for (value, label) in [
            (self.profile.modifier_top, "profile top modifier"),
            (self.profile.modifier_bottom, "profile bottom modifier"),
            (self.profile.modifier_left, "profile left modifier"),
            (self.profile.modifier_right, "profile right modifier"),
        ] {
            in_range(value, -10., 10., label)?;
        }
        in_range(
            self.floor_plan.uncut_line_weight_mm,
            0.01,
            10.,
            "uncut line weight",
        )?;
        in_range(
            self.model.texture_alignment_x,
            -10000.,
            10000.,
            "texture X alignment",
        )?;
        in_range(
            self.model.texture_alignment_y,
            -10000.,
            10000.,
            "texture Y alignment",
        )?;
        in_range(
            self.model.texture_alignment_rotation,
            -3600.,
            3600.,
            "texture rotation",
        )?;
        in_range(
            self.structural.structural_offset_ez,
            -100.,
            100.,
            "structural offset",
        )?;
        for (value, label) in [
            (
                self.classification.thermal_transmittance,
                "thermal transmittance",
            ),
            (
                self.classification.concrete_cover_at_main_bars,
                "concrete cover",
            ),
            (self.classification.lifting_weight, "lifting weight"),
            (self.classification.stored_energy_kwh_m3, "stored energy"),
            (self.classification.mortar_thickness, "mortar thickness"),
            (self.classification.brick_volume, "brick volume"),
            (
                self.classification.sub_ceiling_thickness,
                "sub ceiling thickness",
            ),
            (
                self.classification.sub_floor_thickness,
                "sub floor thickness",
            ),
            (
                self.classification.suspended_ceiling_level,
                "suspended ceiling level",
            ),
            (self.classification.raised_floor_level, "raised floor level"),
            (self.classification.interior_height, "interior height"),
            (
                self.classification.interior_height_detailed,
                "detailed interior height",
            ),
            (self.classification.cost_per_volume, "cost per volume"),
            (
                self.classification.total_cost_of_volume,
                "total cost of volume",
            ),
            (self.classification.total_cost_usd, "total cost USD"),
            (self.classification.total_cost_symbol, "total cost symbol"),
        ] {
            in_range(value, -1_000_000_000., 1_000_000_000., label)?;
        }
        for (value, label) in [
            (self.classification.brick_h_size, "brick H size"),
            (self.classification.brick_w_size, "brick W size"),
            (self.classification.brick_l_size, "brick L size"),
        ] {
            in_range(value, 0., 100_000., label)?;
        }
        for (value, label) in [
            (&self.profile.name, "profile name"),
            (&self.floor_plan.cut_fill_pen, "cut fill pen"),
            (&self.model.left_surface, "left surface"),
            (&self.model.right_surface, "right surface"),
            (&self.model.end_surface, "end surface"),
            (&self.classification.property_id, "property ID"),
            (
                &self.classification.fire_resistance_rating,
                "fire resistance rating",
            ),
            (
                &self.classification.sound_transmission_class,
                "sound transmission class",
            ),
            (
                &self.classification.environmental_class,
                "environmental class",
            ),
            (&self.classification.brick_type, "brick type"),
            (
                &self.classification.dynamic_id_value,
                "dynamic classification ID",
            ),
            (&self.classification.classification_id, "classification ID"),
            (
                &self.classification.classification_name,
                "classification name",
            ),
            (&self.classification.strength_grade, "strength grade"),
            (&self.classification.ifc_type, "IFC type"),
            (
                &self.classification.global_id_attribute,
                "global ID attribute",
            ),
            (&self.classification.name_attribute, "name attribute"),
            (
                &self.classification.predefined_type_attribute,
                "predefined type attribute",
            ),
        ] {
            if value.chars().count() > 256 {
                return Err(format!("Wall setting {label} is too long"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_ac29_wall_default_settings() {
        let settings = WallSettings::default();
        assert_eq!(settings.geometry.top_link_offset, 1);
        assert_eq!(settings.geometry.reference_line, WallReferenceLine::Center);
        assert_eq!(settings.profile.name, "Generic Wall/Shell");
        assert_eq!(settings.model.junction_order, 8);
        assert_eq!(settings.structural.supports, [SupportCondition::Rigid; 6]);
        assert_eq!(settings.classification.classification_name, "Wall");
        assert_eq!(settings.floor_plan.uncut_line_weight_mm, 0.18);
        settings.validate().unwrap();
    }

    #[test]
    fn missing_fields_load_with_defaults_and_validation_rejects_bad_values() {
        let partial: WallSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(partial, WallSettings::default());
        let mut settings = WallSettings::default();
        settings.geometry.reference_offset = f32::NAN;
        assert!(settings.validate().is_err());
        settings.geometry.reference_offset = 0.;
        settings.classification.property_id = "x".repeat(257);
        assert!(settings.validate().is_err());
    }
}
