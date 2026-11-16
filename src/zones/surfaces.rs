//! Opaque presentation surfaces for authored spaces. Construction material stays
//! independent. Prepared edits use the ordinary model transaction and locks.
use crate::{command::ModelCommand, editor::Editor, model::Shape};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ZoneSurface {
    #[default]
    Inherit,
    Solid {
        name: String,
        color: [u8; 3],
    },
}
impl ZoneSurface {
    pub fn is_inherit(&self) -> bool {
        matches!(self, Self::Inherit)
    }
    pub fn validate(&self) -> Result<(), String> {
        if let Self::Solid { name, .. } = self {
            if name.trim().is_empty() || name.len() > 256 {
                return Err("Surface name must contain 1–256 bytes".into());
            }
        }
        Ok(())
    }
    pub fn color(&self, fallback: [f32; 3]) -> [f32; 3] {
        match self {
            Self::Inherit => fallback,
            Self::Solid { color, .. } => color.map(|v| f32::from(v) / 255.),
        }
    }
}
pub fn element_color(e: &crate::model::Element) -> [f32; 3] {
    match &e.shape {
        Shape::Zone { spec } => spec.surface.color(e.material.color()),
        _ => e.material.color(),
    }
}
/// Generate once on explicit request; actual RGB values are persisted. The seed
/// belongs to this request, not a render frame. No geometry or identity changes.
pub fn random_commands(
    editor: &Editor,
    ids: &[u64],
    seed: &str,
) -> Result<Vec<ModelCommand>, String> {
    use sha2::{Digest, Sha256};
    if ids.is_empty() || ids.len() > 4096 {
        return Err("Select 1–4096 spaces for surface assignment".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut colors = std::collections::BTreeSet::new();
    let mut commands = Vec::with_capacity(ids.len());
    for &id in ids {
        if !seen.insert(id) {
            return Err("Duplicate space in surface assignment".into());
        }
        let mut e = editor
            .project
            .elements
            .iter()
            .find(|e| e.id == id)
            .cloned()
            .ok_or("Space is missing")?;
        let Shape::Zone { spec } = &mut e.shape else {
            return Err("Surface assignment supports spaces only".into());
        };
        let h = Sha256::digest(format!("{seed}/{id}"));
        let mut color = [70 + h[0] % 160, 70 + h[1] % 160, 70 + h[2] % 160];
        while !colors.insert(color) {
            let n =
                ((u32::from(color[0]) << 16) | (u32::from(color[1]) << 8) | u32::from(color[2]))
                    .wrapping_add(1);
            color = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        }
        spec.surface = ZoneSurface::Solid {
            name: format!(
                "Zone {id} · #{:02X}{:02X}{:02X}",
                color[0], color[1], color[2]
            ),
            color,
        };
        commands.push(ModelCommand::UpdateElement { element: e });
    }
    Ok(commands)
}
