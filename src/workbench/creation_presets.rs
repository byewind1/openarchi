//! Optional personal defaults for future concept creation, never project facts.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreationPresets {
    pub mass_height_mm: u32,
    pub zone_height_mm: u32,
    pub zone_purpose: String,
}
impl Default for CreationPresets {
    fn default() -> Self {
        Self {
            mass_height_mm: 3000,
            zone_height_mm: 3000,
            zone_purpose: "general".into(),
        }
    }
}
impl CreationPresets {
    pub fn validate(&self) -> Result<(), String> {
        if !(10..=100_000).contains(&self.mass_height_mm)
            || !(10..=100_000).contains(&self.zone_height_mm)
            || self.zone_purpose.trim().is_empty()
            || self.zone_purpose.len() > 256
            || self.zone_purpose.chars().any(char::is_control)
        {
            return Err("Invalid concept creation presets".into());
        }
        Ok(())
    }
    pub fn from_metres(mass: f64, zone: f64, purpose: &str) -> Result<Self, String> {
        let mm = |m: f64| {
            let v = m * 1000.;
            if !v.is_finite() || !(10. ..=100_000.).contains(&v) || (v - v.round()).abs() > 1e-7 {
                return Err(
                    "Creation preset heights require whole millimetres in the supported range"
                        .to_owned(),
                );
            }
            Ok(v.round() as u32)
        };
        let p = Self {
            mass_height_mm: mm(mass)?,
            zone_height_mm: mm(zone)?,
            zone_purpose: purpose.into(),
        };
        p.validate()?;
        Ok(p)
    }
}
