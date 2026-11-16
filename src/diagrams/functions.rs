//! Functional expression of exact, authored Zone purpose labels. No use classifier.
use super::{Diagram, Domain, name_ok, quantities};
use crate::model::{Element, Shape};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Presentation {
    #[default]
    ModelQuantities,
    ZoneFunctions {
        #[serde(default)]
        unknown_purposes: Vec<String>,
    },
    ExplodedAxon {
        offsets_mm: Vec<super::axon::DisplayOffset>,
    },
    SiteLocation,
    ConstraintOverlay,
    SetbackStudy {
        distance_mm: u32,
        assumption: String,
    },
}
impl Presentation {
    pub fn is_default(&self) -> bool {
        matches!(self, Self::ModelQuantities)
    }
    pub fn is_functional(&self) -> bool {
        matches!(self, Self::ZoneFunctions { .. })
    }
    pub fn is_axon(&self) -> bool {
        matches!(self, Self::ExplodedAxon { .. })
    }
    pub fn is_site(&self) -> bool {
        matches!(
            self,
            Self::SiteLocation | Self::ConstraintOverlay | Self::SetbackStudy { .. }
        )
    }
    pub fn validate(&self, domain: Domain) -> Result<(), String> {
        if matches!(self, Self::ConstraintOverlay) && domain != Domain::Site {
            return Err("Constraint overlay requires exact mixed site scope".into());
        }

        if domain == Domain::Site && !self.is_site() {
            return Err(
                "Site domain requires a site location or setback study presentation".into(),
            );
        }
        if let Self::SetbackStudy {
            distance_mm,
            assumption,
        } = self
            && (*distance_mm > 100_000 || !name_ok(assumption))
        {
            return Err(
                "Setback study requires 0–100000 mm and a printable design assumption".into(),
            );
        }
        if let Self::ZoneFunctions { unknown_purposes } = self {
            if domain != Domain::Zones
                || unknown_purposes.len() > 32
                || unknown_purposes.iter().any(|p| !name_ok(p))
                || unknown_purposes.iter().collect::<BTreeSet<_>>().len() != unknown_purposes.len()
            {
                return Err("Functional diagrams require Zones and at most 32 distinct printable unknown-purpose declarations".into());
            }
        }
        if let Self::ExplodedAxon { offsets_mm } = self {
            super::axon::validate_offsets(offsets_mm)?;
        }
        Ok(())
    }
    pub fn is_unknown(&self, purpose: &str) -> bool {
        matches!(self,Self::ZoneFunctions{unknown_purposes}if unknown_purposes.iter().any(|p|p==purpose))
    }
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FunctionGroup {
    pub key: String,
    pub purpose: String,
    pub unknown: bool,
    pub color: [u8; 3],
    pub element_ids: Vec<u64>,
    pub area_m2: f64,
    pub volume_m3: f64,
}
// Presentation v1: deterministic exact-label mapping, independent of option/order/language.
// Repeated colours are disambiguated by full labels, keys and element identities.
const BUILTIN_PURPOSE_COLORS: [(&str, [u8; 3]); 7] = [
    ("living", [224, 174, 108]),
    ("dining", [142, 189, 136]),
    ("kitchen", [115, 174, 207]),
    ("circulation", [183, 151, 201]),
    ("stair_hall", [108, 187, 181]),
    ("bedroom", [220, 145, 145]),
    ("bathroom", [194, 178, 112]),
];
pub fn purpose_key(purpose: &str) -> String {
    crate::drawings::digest(purpose.as_bytes())
}
fn purpose_color(purpose: &str, unknown: bool) -> [u8; 3] {
    if unknown {
        return [160, 160, 160];
    }
    // Style keys for the exact built-in authored strings, NOT a purpose classifier:
    // no aliases, text normalization or assignment of an architectural use.
    if let Some((_, color)) = BUILTIN_PURPOSE_COLORS
        .iter()
        .find(|(label, _)| *label == purpose)
    {
        return *color;
    }
    let key = purpose_key(purpose);
    [0, 2, 4].map(|i| 70 + (u16::from_str_radix(&key[i..i + 2], 16).unwrap() * 140 / 255) as u8)
}
impl Diagram {
    pub fn function_groups(&self) -> Vec<FunctionGroup> {
        if !self.definition.presentation.is_functional() {
            return vec![];
        }
        let mut groups: BTreeMap<String, FunctionGroup> = BTreeMap::new();
        for e in &self.snapshot.elements {
            if let Shape::Zone { spec } = &e.shape {
                let unknown = self.definition.presentation.is_unknown(&spec.purpose);
                let (a, v) = quantities(e);
                let group = groups
                    .entry(spec.purpose.clone())
                    .or_insert_with(|| FunctionGroup {
                        key: purpose_key(&spec.purpose),
                        purpose: spec.purpose.clone(),
                        unknown,
                        color: purpose_color(&spec.purpose, unknown),
                        element_ids: vec![],
                        area_m2: 0.,
                        volume_m3: 0.,
                    });
                group.element_ids.push(e.id);
                group.area_m2 += a;
                group.volume_m3 += v;
            }
        }
        groups.into_values().collect()
    }
    pub fn element_color(&self, e: &Element) -> [u8; 3] {
        if self.definition.presentation.is_functional()
            && let Shape::Zone { spec } = &e.shape
        {
            return purpose_color(
                &spec.purpose,
                self.definition.presentation.is_unknown(&spec.purpose),
            );
        }
        e.material
            .color()
            .map(|v| (v.clamp(0., 1.) * 255.).round() as u8)
    }
}
