use super::*;
use crate::model::{Project, Shape};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum North {
    Declared(crate::site::TrueNorth),
    DesignAssumption {
        clockwise_from_model_y_deg: f64,
        note: String,
    },
}
impl North {
    pub fn angle(&self) -> f64 {
        match self {
            Self::Declared(n) => n.clockwise_from_model_y_deg,
            Self::DesignAssumption {
                clockwise_from_model_y_deg,
                ..
            } => *clockwise_from_model_y_deg,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if !self.angle().is_finite() || self.angle().abs() > 360. {
            return Err("Solar north angle must be finite within +/-360 degrees".into());
        }
        match self {
            Self::Declared(n) => {
                crate::site::SiteContext {
                    true_north: Some(n.clone()),
                    ..Default::default()
                }
                .validate()?;
            }
            Self::DesignAssumption { note, .. }
                if note.trim().is_empty() || note.len() > 4096 || note.contains('\0') =>
            {
                return Err("Assumed solar north requires an explicit bounded note".into());
            }
            _ => {}
        }
        Ok(())
    }
    pub fn model_direction(&self, enu: [f64; 3]) -> [f64; 3] {
        let (s, c) = self.angle().to_radians().sin_cos();
        [c * enu[0] + s * enu[1], -s * enu[0] + c * enu[1], enu[2]]
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Blocker {
    pub element_id: u64,
    pub triangles: Vec<[[f64; 3]; 3]>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub schema_version: u32,
    pub algorithm: String,
    pub source_sha256: String,
    pub north: North,
    pub blockers: Vec<Blocker>,
    /// Complete project IDs omitted from the explicit opaque concept-envelope scope.
    pub excluded_element_ids: Vec<u64>,
}
impl Snapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1
            || self.algorithm != ALGORITHM
            || !crate::storage::valid_document_identity(&self.source_sha256)
            || self.blockers.len() > 256
            || self.excluded_element_ids.len() > 100000
        {
            return Err("Invalid solar snapshot identity or capacity".into());
        }
        self.north.validate()?;
        let mut last = None;
        let mut count = 0;
        for b in &self.blockers {
            if b.element_id == 0
                || last.is_some_and(|id| id >= b.element_id)
                || b.triangles.is_empty()
            {
                return Err(
                    "Solar blockers must have unique sorted IDs and nonempty physical surfaces"
                        .into(),
                );
            }
            last = Some(b.element_id);
            count += b.triangles.len();
            if count > 200000 {
                return Err("Solar surface exceeds 200000 triangles".into());
            }
            for t in &b.triangles {
                let p = t.map(glam::DVec3::from_array);
                if p.iter()
                    .any(|v| !v.is_finite() || v.abs().max_element() > 1e9)
                    || (p[1] - p[0]).cross(p[2] - p[0]).length_squared() <= 1e-20
                {
                    return Err("Solar surface has invalid or degenerate triangles".into());
                }
            }
        }
        if self.excluded_element_ids.windows(2).any(|p| p[0] >= p[1])
            || self
                .excluded_element_ids
                .iter()
                .any(|id| *id == 0 || self.blockers.iter().any(|b| b.element_id == *id))
        {
            return Err("Invalid excluded solar object IDs".into());
        }
        Ok(())
    }
    /// Compare with the authoritative model, never trust a stored snapshot as live input.
    pub fn is_current(&self, project: &Project) -> Result<bool, String> {
        let ids = self
            .blockers
            .iter()
            .map(|b| b.element_id)
            .collect::<Vec<_>>();
        let fallback = match &self.north {
            North::DesignAssumption { .. } => Some(self.north.clone()),
            _ => None,
        };
        self.validate()?;
        Ok(capture(project, &ids, fallback)? == *self)
    }
}
pub fn capture(p: &Project, ids: &[u64], assumed_north: Option<North>) -> Result<Snapshot, String> {
    if ids.len() > 256 || ids.windows(2).any(|p| p[0] >= p[1]) || ids.contains(&0) {
        return Err("Solar blocker IDs must be sorted, unique and at most256".into());
    }
    let north = if let Some(n) = &p.site.true_north {
        North::Declared(n.clone())
    } else {
        match assumed_north{Some(n@North::DesignAssumption{..})=>n,_=>return Err("True north is unknown; supply an explicit design assumption or declared site north".into())}
    };
    north.validate()?;
    let objects = ids
        .iter()
        .map(|id| {
            let e = p
                .elements
                .iter()
                .find(|e| e.id == *id)
                .ok_or("Solar blocker is missing")?;
            let base = p
                .stories
                .iter()
                .find(|s| s.id == e.story)
                .ok_or("Solar blocker story is missing")?
                .elevation;
            Ok((e, base))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut excluded = p
        .elements
        .iter()
        .filter(|e| ids.binary_search(&e.id).is_err())
        .map(|e| e.id)
        .collect::<Vec<_>>();
    excluded.sort_unstable();
    capture_objects(&objects, north, excluded)
}
/// Pure immutable adapter for captured jobs. Does not reconstruct or own a Project.
pub fn capture_objects(
    objects: &[(&crate::model::Element, f64)],
    north: North,
    excluded: Vec<u64>,
) -> Result<Snapshot, String> {
    north.validate()?;
    if objects.len() > 256 || objects.windows(2).any(|v| v[0].0.id >= v[1].0.id) {
        return Err("Solar blocker IDs must be sorted, unique and at most256".into());
    }
    let mut blockers = vec![];
    let mut count = 0;
    for (e, base) in objects {
        if !base.is_finite() || base.abs() > 100000. {
            return Err("Invalid solar source story elevation".into());
        }
        if !matches!(e.shape, Shape::ConceptMass { .. }) {
            return Err("This solar adapter accepts explicit opaque concept masses only; other objects are not silently omitted".into());
        }
        let g = crate::concept_mass::derive(e, *base).map_err(|e| e.message)?;
        count += g.triangles().count();
        if count > 200000 {
            return Err("Solar surface exceeds 200000 triangles".into());
        }
        blockers.push(Blocker {
            element_id: e.id,
            triangles: g.triangles().map(|t| t.map(|v| v.to_array())).collect(),
        });
    }
    let result = Snapshot {
        schema_version: 1,
        algorithm: ALGORITHM.into(),
        source_sha256: hash(&(ALGORITHM, objects, &north, &excluded))?,
        north,
        blockers,
        excluded_element_ids: excluded,
    };
    result.validate()?;
    Ok(result)
}
