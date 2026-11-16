//! Original, bounded geometric sunlight calculations; never a compliance solver.
//! Reads existing geometry. No Project ownership, model writes or hidden IO.
use serde::{Deserialize, Serialize};
mod position;
mod snapshot;
mod study;
pub use position::{Position, Utc, position};
pub use snapshot::{Blocker, North, Snapshot, capture, capture_objects};
pub use study::{Cell, Receiver, Request, ResultSet, Segment, State, run, run_with_control};
pub const ALGORITHM: &str = "openarchi-geometric-sun-1";
pub const SEMANTICS: &str = "Approximate geometric solar centre; no atmospheric refraction, topocentric parallax or statutory approval. Only the explicitly captured opaque geometry is included.";
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub latitude_deg: f64,
    /// East of Greenwich is positive.
    pub longitude_deg: f64,
}
impl Location {
    pub fn validate(self) -> Result<(), String> {
        if !self.latitude_deg.is_finite()
            || !self.longitude_deg.is_finite()
            || self.latitude_deg.abs() > 90.
            || self.longitude_deg.abs() > 180.
        {
            return Err(
                "Solar location requires finite latitude [-90,90] and east longitude [-180,180]"
                    .into(),
            );
        }
        Ok(())
    }
}
pub(crate) fn hash<T: Serialize>(v: &T) -> Result<String, String> {
    crate::automation_journal::canonical_hash(v)
}

/// Authored source declaration or a design assumption, never inferred coordinates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LocationBasis {
    Declared { evidence: crate::site::SiteEvidence },
    DesignAssumption { note: String },
}
impl LocationBasis {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Declared { evidence } => evidence.validate(),
            Self::DesignAssumption { note }
                if note.trim().is_empty() || note.len() > 4096 || note.contains('\0') =>
            {
                Err("Solar location assumption requires a bounded explicit note".into())
            }
            _ => Ok(()),
        }
    }
}
