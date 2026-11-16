//! Discovery envelope only: opaque data is immediately decoded into the same Geometry.
use super::*;
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegisteredGeometry {
    /// Exact command name from search_commands. Inspect its parameters_schema before preparing.
    pub command: String,
    /// Data constrained by the live per-command parameters_schema; no type key or executable code.
    pub parameters: serde_json::Map<String, Value>,
}
impl RegisteredGeometry {
    fn resolve(&self) -> Result<Geometry, String> {
        if !COMMANDS.contains(&self.command.as_str()) {
            return Err(format!("COMMAND_NOT_FOUND: {}", self.command));
        }
        if self.parameters.contains_key("type") {
            return Err("The type discriminator belongs to command, not parameters".into());
        }
        let mut fields = self.parameters.clone();
        fields.insert("type".into(), json!(self.command));
        serde_json::from_value(Value::Object(fields))
            .map_err(|e| format!("INVALID_COMMAND_PARAMETERS {}: {e}", self.command))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegisteredCreate {
    pub name: String,
    pub story: u32,
    pub layer: u32,
    pub material: NativeMaterial,
    pub geometry: RegisteredGeometry,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum RegisteredEdit {
    Constraint {
        edit: crate::constraints::Edit,
    },
    MassSpace {
        edit: crate::mass_spaces::Edit,
    },
    Diagram {
        edit: crate::diagrams::Edit,
    },
    Terrain {
        edit: crate::site::terrain::Edit,
    },
    Evidence {
        edit: crate::evidence::EvidenceEdit,
    },
    Program {
        edit: crate::program::ProgramEdit,
    },
    ApplySiteBoundary {
        proposal: crate::site::boundary::BoundaryProposal,
    },
    DetachSiteBoundary,
    ApplySiteControlRegion {
        proposal: crate::site::boundary::BoundaryProposal,
    },
    DetachSiteControlRegion,
    SetSiteContext {
        context: crate::site::SiteContext,
    },
    Set3dStyle {
        style: crate::view_settings::Model3dStyle,
    },
    SetStoryElevation {
        story: u32,
        elevation: f64,
    },
    Move {
        targets: Vec<Reference>,
        offset: XY,
    },
    Remove {
        targets: Vec<Reference>,
    },
    Update {
        target: Reference,
        geometry: RegisteredGeometry,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiscoveredBatch {
    #[schemars(range(min = 1, max = 1))]
    pub version: u32,
    #[serde(default)]
    pub creates: Vec<RegisteredCreate>,
    #[serde(default)]
    pub edits: Vec<RegisteredEdit>,
}
impl DiscoveredBatch {
    pub fn resolve(&self) -> Result<NativeBatch, String> {
        if self.version != 1 {
            return Err("Unsupported discovered command schema version".into());
        }
        if self.creates.len() + self.edits.len() > 256 {
            return Err("Native batch exceeds 256 commands".into());
        }
        let creates = self
            .creates
            .iter()
            .map(|c| {
                Ok(Create {
                    name: c.name.clone(),
                    story: c.story,
                    layer: c.layer,
                    material: c.material,
                    geometry: c.geometry.resolve()?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let edits = self
            .edits
            .iter()
            .map(|e| {
                Ok(match e {
                    RegisteredEdit::Terrain { edit } => Edit::Terrain { edit: edit.clone() },
                    RegisteredEdit::Evidence { edit } => Edit::Evidence { edit: edit.clone() },
                    RegisteredEdit::MassSpace { edit } => Edit::MassSpace { edit: edit.clone() },
                    RegisteredEdit::Constraint { edit } => Edit::Constraint { edit: edit.clone() },
                    RegisteredEdit::Diagram { edit } => Edit::Diagram { edit: edit.clone() },
                    RegisteredEdit::Program { edit } => Edit::Program { edit: edit.clone() },
                    RegisteredEdit::ApplySiteBoundary { proposal } => Edit::ApplySiteBoundary {
                        proposal: proposal.clone(),
                    },
                    RegisteredEdit::DetachSiteBoundary => Edit::DetachSiteBoundary,
                    RegisteredEdit::ApplySiteControlRegion { proposal } => {
                        Edit::ApplySiteControlRegion {
                            proposal: proposal.clone(),
                        }
                    }
                    RegisteredEdit::DetachSiteControlRegion => Edit::DetachSiteControlRegion,
                    RegisteredEdit::SetSiteContext { context } => Edit::SetSiteContext {
                        context: context.clone(),
                    },
                    RegisteredEdit::Set3dStyle { style } => Edit::Set3dStyle { style: *style },
                    RegisteredEdit::SetStoryElevation { story, elevation } => {
                        Edit::SetStoryElevation {
                            story: *story,
                            elevation: *elevation,
                        }
                    }
                    RegisteredEdit::Move { targets, offset } => Edit::Move {
                        targets: targets.clone(),
                        offset: *offset,
                    },
                    RegisteredEdit::Remove { targets } => Edit::Remove {
                        targets: targets.clone(),
                    },
                    RegisteredEdit::Update { target, geometry } => Edit::Update {
                        target: target.clone(),
                        geometry: geometry.resolve()?,
                    },
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(NativeBatch {
            version: 1,
            creates,
            edits,
        })
    }
}
pub fn discovered_schema() -> Value {
    serde_json::to_value(schemars::schema_for!(DiscoveredBatch)).expect("schema serializes")
}
