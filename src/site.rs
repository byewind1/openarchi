//! Declared site reference. Mathematical consistency is separate from source approval.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SiteEvidence {
    pub source_sha256: String,
    pub locator: String,
    pub note: String,
}
impl SiteEvidence {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if !crate::storage::valid_document_identity(&self.source_sha256)
            || self.locator.trim().is_empty()
            || self.locator.len() > 1024
            || self.note.len() > 4096
        {
            return Err("Site values require a source SHA256 and locator; locator/note limits are 1024/4096 bytes".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HorizontalReference {
    pub crs: String,
    pub origin_east_north_m: [f64; 2],
    /// Counterclockwise rotation of model XY into projected east/north axes.
    pub model_to_grid_ccw_deg: f64,
    pub evidence: SiteEvidence,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrueNorth {
    pub clockwise_from_model_y_deg: f64,
    pub evidence: SiteEvidence,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerticalReference {
    pub datum: String,
    pub model_zero_elevation_m: f64,
    pub evidence: SiteEvidence,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoordinateControl {
    pub model_xy_m: [f64; 2],
    pub reference_east_north_m: [f64; 2],
    pub evidence: SiteEvidence,
}
fn version() -> u32 {
    1
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SiteContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terrain: Option<terrain::Terrain>,
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boundary: Option<boundary::SiteBoundary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_region: Option<boundary::SiteBoundary>,
    pub horizontal: Option<HorizontalReference>,
    pub true_north: Option<TrueNorth>,
    pub vertical: Option<VerticalReference>,
    pub controls: Vec<CoordinateControl>,
    pub control_tolerance_m: Option<f64>,
}
impl Default for SiteContext {
    fn default() -> Self {
        Self {
            schema_version: version(),
            terrain: None,
            boundary: None,
            control_region: None,
            horizontal: None,
            true_north: None,
            vertical: None,
            controls: vec![],
            control_tolerance_m: None,
        }
    }
}
#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ControlCheck {
    Insufficient {
        reason: String,
    },
    Measured {
        residuals_m: Vec<f64>,
        max_residual_m: f64,
        tolerance_m: f64,
        consistent: bool,
    },
}
fn finite(values: &[f64], limit: f64) -> bool {
    values.iter().all(|v| v.is_finite() && v.abs() <= limit)
}
fn projected_xy(h: &HorizontalReference, model: [f64; 2]) -> Result<[f64; 2], String> {
    let (s, c) = h.model_to_grid_ccw_deg.to_radians().sin_cos();
    let result = [
        h.origin_east_north_m[0] + c * model[0] - s * model[1],
        h.origin_east_north_m[1] + s * model[0] + c * model[1],
    ];
    if !finite(&result, 1e9) {
        return Err("Invalid projected coordinates".into());
    }
    Ok(result)
}
impl SiteContext {
    pub fn is_unknown(&self) -> bool {
        self == &Self::default()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("Unsupported site context schema".into());
        }
        if let Some(terrain) = &self.terrain {
            terrain.validate()?;
        }
        if let Some(boundary) = &self.boundary {
            boundary.validate()?;
        }
        if let Some(region) = &self.control_region {
            region.validate()?;
            if self
                .boundary
                .as_ref()
                .is_some_and(|b| b.element_id == region.element_id)
            {
                return Err("Site and control references must use distinct polylines".into());
            }
        }
        if self.controls.len() > 1000 {
            return Err("Site control count exceeds 1000".into());
        }
        if let Some(h) = &self.horizontal {
            h.evidence.validate()?;
            if h.crs.trim().is_empty()
                || h.crs.len() > 256
                || !finite(&h.origin_east_north_m, 1e9)
                || !finite(&[h.model_to_grid_ccw_deg], 360.)
            {
                return Err(
                    "Invalid projected reference; metres and an explicit CRS are required".into(),
                );
            }
        }
        if let Some(n) = &self.true_north {
            n.evidence.validate()?;
            if !finite(&[n.clockwise_from_model_y_deg], 360.) {
                return Err("Invalid true north angle".into());
            }
        }
        if let Some(v) = &self.vertical {
            v.evidence.validate()?;
            if v.datum.trim().is_empty()
                || v.datum.len() > 256
                || !finite(&[v.model_zero_elevation_m], 1e5)
            {
                return Err("Invalid vertical datum or elevation".into());
            }
        }
        if self
            .control_tolerance_m
            .is_some_and(|t| !t.is_finite() || t <= 0. || t > 1000.)
        {
            return Err("Control tolerance must be positive and within 1000 metres".into());
        }
        for (i, c) in self.controls.iter().enumerate() {
            c.evidence.validate()?;
            if !finite(&c.model_xy_m, 1e6) || !finite(&c.reference_east_north_m, 1e9) {
                return Err("Invalid site control coordinates".into());
            }
            if self.controls[..i]
                .iter()
                .any(|p| p.model_xy_m == c.model_xy_m)
            {
                return Err("Duplicate model control point".into());
            }
        }
        Ok(())
    }
    pub fn to_reference_xy(&self, model: [f64; 2]) -> Result<[f64; 2], String> {
        self.validate()?;
        if !finite(&model, 1e6) {
            return Err("Invalid model coordinates".into());
        }
        let h = self
            .horizontal
            .as_ref()
            .ok_or("Projected reference is unknown")?;
        projected_xy(h, model)
    }
    pub fn from_reference_xy(&self, reference: [f64; 2]) -> Result<[f64; 2], String> {
        self.validate()?;
        if !finite(&reference, 1e9) {
            return Err("Invalid projected coordinates".into());
        }
        let h = self
            .horizontal
            .as_ref()
            .ok_or("Projected reference is unknown")?;
        let (s, c) = h.model_to_grid_ccw_deg.to_radians().sin_cos();
        let x = reference[0] - h.origin_east_north_m[0];
        let y = reference[1] - h.origin_east_north_m[1];
        let result = [c * x + s * y, -s * x + c * y];
        if !finite(&result, 1e6) {
            return Err("Projected point exceeds model coordinate range".into());
        }
        Ok(result)
    }
    pub fn reference_elevation(&self, model_z_m: f64) -> Result<f64, String> {
        self.validate()?;
        if !finite(&[model_z_m], 1e5) {
            return Err("Invalid model elevation".into());
        }
        Ok(self
            .vertical
            .as_ref()
            .ok_or("Vertical reference is unknown")?
            .model_zero_elevation_m
            + model_z_m)
    }
    pub fn true_north_vector(&self) -> Result<[f64; 2], String> {
        self.validate()?;
        let n = self.true_north.as_ref().ok_or("True north is unknown")?;
        let (s, c) = n.clockwise_from_model_y_deg.to_radians().sin_cos();
        Ok([s, c])
    }
    pub fn control_check(&self) -> Result<ControlCheck, String> {
        self.validate()?;
        if self.horizontal.is_none()
            || self.controls.len() < 2
            || self.control_tolerance_m.is_none()
        {
            return Ok(ControlCheck::Insufficient {
                reason:
                    "Provide a projected reference, two distinct controls and an explicit tolerance"
                        .into(),
            });
        }
        let residuals = self
            .controls
            .iter()
            .map(|p| {
                let h = self.horizontal.as_ref().unwrap();
                let actual = projected_xy(h, p.model_xy_m)?;
                Ok((actual[0] - p.reference_east_north_m[0])
                    .hypot(actual[1] - p.reference_east_north_m[1]))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let max = residuals.iter().copied().fold(0f64, f64::max);
        let tolerance = self.control_tolerance_m.unwrap();
        Ok(ControlCheck::Measured {
            residuals_m: residuals,
            max_residual_m: max,
            tolerance_m: tolerance,
            consistent: max <= tolerance,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LengthUnit {
    Inch,
    Foot,
    Millimetre,
    Centimetre,
    Metre,
}
impl LengthUnit {
    pub const ALL: [Self; 5] = [
        Self::Inch,
        Self::Foot,
        Self::Millimetre,
        Self::Centimetre,
        Self::Metre,
    ];
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Inch => "in",
            Self::Foot => "ft",
            Self::Millimetre => "mm",
            Self::Centimetre => "cm",
            Self::Metre => "m",
        }
    }
    pub fn metres(self) -> f64 {
        match self {
            Self::Inch => 0.0254,
            Self::Foot => 0.3048,
            Self::Millimetre => 0.001,
            Self::Centimetre => 0.01,
            Self::Metre => 1.0,
        }
    }
}

pub mod boundary;
pub mod terrain;
