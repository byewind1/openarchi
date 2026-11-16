//! Explicit geometry-only triangular OBJ admission; no unit or datum inference.
use super::*;
use crate::site::terrain::{Geometry, Origin, Placement, Proposal, Terrain};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainPlacement {
    ModelXyz,
    SiteReference,
}
pub struct TerrainCommitGuard {
    _lock: Lock,
}
pub fn parse_obj(bytes: &[u8]) -> Result<Geometry, String> {
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("Terrain OBJ exceeds 16 MiB".into());
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "Terrain OBJ must be UTF-8")?
        .trim_start_matches('\u{feff}');
    let mut geometry = Geometry {
        vertices: vec![],
        faces: vec![],
    };
    for (line_no, line) in text.lines().enumerate() {
        if line.len() > 16384 {
            return Err("Terrain OBJ line exceeds 16 KiB".into());
        }
        let fields = line
            .split('#')
            .next()
            .unwrap_or("")
            .split_whitespace()
            .collect::<Vec<_>>();
        if fields.is_empty() {
            continue;
        }
        let error = || {
            format!(
                "Unsupported or invalid terrain OBJ record at line {}",
                line_no + 1
            )
        };
        match fields[0] {
            "v" if fields.len() == 4 => {
                let mut p = [0.; 3];
                for (i, field) in fields[1..].iter().enumerate() {
                    p[i] = field.parse::<f64>().map_err(|_| error())?;
                    if !p[i].is_finite() || p[i].abs() > 1e12 {
                        return Err(error());
                    }
                }
                geometry.vertices.push(p);
            }
            "f" if fields.len() == 4 => {
                let mut face = [0; 3];
                for (i, field) in fields[1..].iter().enumerate() {
                    face[i] = field
                        .parse::<u32>()
                        .ok()
                        .filter(|v| *v > 0)
                        .and_then(|v| v.checked_sub(1))
                        .ok_or_else(error)?;
                }
                geometry.faces.push(face);
            }
            _ => return Err(error()),
        }
        if geometry.vertices.len() > 10000 || geometry.faces.len() > 20000 {
            return Err("Terrain OBJ exceeds supported vertex/triangle limits".into());
        }
    }
    for i in 0..geometry.faces.len() {
        let p = geometry.triangle(geometry.faces[i])?;
        let area =
            (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0]);
        if area < 0. {
            geometry.faces[i].swap(1, 2);
        }
    }
    Ok(geometry)
}
fn transformed(bytes: &[u8], unit: LengthUnit, placement: &Placement) -> Result<Geometry, String> {
    let mut geometry = parse_obj(bytes)?;
    let site = match placement {
        Placement::ModelXyz => crate::site::SiteContext::default(),
        Placement::SiteReference {
            horizontal,
            vertical,
        } => crate::site::SiteContext {
            horizontal: Some(horizontal.clone()),
            vertical: Some(vertical.clone()),
            ..Default::default()
        },
    };
    for p in &mut geometry.vertices {
        for v in p.iter_mut() {
            *v *= unit.metres();
        }
        if let Placement::SiteReference { vertical, .. } = placement {
            let xy = site.from_reference_xy([p[0], p[1]])?;
            p[0] = xy[0];
            p[1] = xy[1];
            p[2] -= vertical.model_zero_elevation_m;
        }
    }
    geometry.validate()?;
    Ok(geometry)
}
impl Repository {
    fn terrain_source(&self, id: &str) -> Result<&Source, String> {
        self.unchanged()?;
        let source = self
            .manifest
            .sources
            .iter()
            .find(|s| s.id == id && s.format == "obj")
            .ok_or("Choose a stored triangular OBJ source")?;
        if matches!(source.quality, SourceQuality::Rejected { .. }) {
            return Err("Rejected source cannot supply terrain".into());
        }
        Ok(source)
    }
    pub fn preview_terrain(
        &self,
        site: &crate::site::SiteContext,
        id: &str,
        placement: TerrainPlacement,
        name: String,
    ) -> Result<Proposal, String> {
        if site.terrain.as_ref().is_some_and(Terrain::user_modified) {
            return Err("Reimport refused: terrain contains user edits".into());
        }
        let _lock = Lock::acquire(&self.root)?;
        let source = self.terrain_source(id)?;
        let unit = source
            .confirmed_unit
            .ok_or("Confirm source units before terrain preview")?;
        let placement = match placement {
            TerrainPlacement::ModelXyz => Placement::ModelXyz,
            TerrainPlacement::SiteReference => {
                if !matches!(
                    site.control_check()?,
                    crate::site::ControlCheck::Measured {
                        consistent: true,
                        ..
                    }
                ) {
                    return Err(
                        "Projected terrain requires two consistent controls and a stated tolerance"
                            .into(),
                    );
                }
                Placement::SiteReference {
                    horizontal: site
                        .horizontal
                        .clone()
                        .ok_or("Projected reference is unknown")?,
                    vertical: site.vertical.clone().ok_or("Vertical datum is unknown")?,
                }
            }
        };
        let bytes = resource(&self.root, &source.blob, 16 * 1024 * 1024)?;
        let geometry = transformed(&bytes, unit, &placement)?;
        let terrain = Terrain {
            name,
            origin: Origin {
                source_sha256: id.into(),
                unit,
                placement,
            },
            imported: geometry.clone(),
            geometry,
        };
        terrain.validate()?;
        Ok(Proposal {
            terrain,
            expected_previous: site.terrain.clone(),
        })
    }
    pub fn guard_terrain_commit(&self, proposal: &Proposal) -> Result<TerrainCommitGuard, String> {
        let lock = Lock::acquire(&self.root)?;
        self.verify_terrain_unlocked(proposal)?;
        Ok(TerrainCommitGuard { _lock: lock })
    }
    pub(super) fn verify_terrain_unlocked(&self, proposal: &Proposal) -> Result<(), String> {
        let source = self.terrain_source(&proposal.terrain.origin.source_sha256)?;
        if source.confirmed_unit != Some(proposal.terrain.origin.unit) {
            return Err("Terrain source units changed; preview again".into());
        }
        let bytes = resource(&self.root, &source.blob, 16 * 1024 * 1024)?;
        let expected = transformed(
            &bytes,
            proposal.terrain.origin.unit,
            &proposal.terrain.origin.placement,
        )?;
        if proposal.terrain.geometry != expected || proposal.terrain.imported != expected {
            return Err("Terrain proposal differs from its source geometry".into());
        }
        Ok(())
    }
}
