//! Strict wire adapter into the same typed Zone/proposal service. No AI geometry side channel.
use super::*;
use crate::{
    automation_registry::XY,
    design_process::{
        Statement,
        intent::{self, Claim, ImageReference, SpaceSeed},
    },
    model::Shape,
};
use schemars::JsonSchema;
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ZoneParameters {
    pub purpose: String,
    pub outline: Vec<XY>,
    #[serde(default)]
    pub holes: Vec<Vec<XY>>,
    pub height: f64,
    #[serde(default)]
    pub bottom_offset: f64,
    #[serde(default)]
    pub label_position: Option<XY>,
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NamedSpace {
    pub name: String,
    pub spec: ZoneParameters,
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub name: String,
    pub question: String,
    pub expected_result: String,
    pub images: Vec<ImageReference>,
    pub claims: Vec<Claim>,
    #[serde(default)]
    pub geometry_assumptions: String,
    #[serde(default)]
    pub statements: Vec<Statement>,
    pub candidate_names: [String; 2],
    pub candidates: [Vec<NamedSpace>; 2],
    pub story: u32,
    pub layer: u32,
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Prepare {
    pub target: Target,
    pub expected_revision: u64,
    pub expected_context_revision: String,
    pub expected_sources_revision: String,
    pub task_id: String,
    pub draft: Draft,
}
impl Draft {
    pub fn lower(self) -> intent::Draft {
        intent::Draft {
            name: self.name,
            question: self.question,
            expected_result: self.expected_result,
            images: self.images,
            claims: self.claims,
            geometry_assumptions: self.geometry_assumptions,
            statements: self.statements,
            candidate_names: self.candidate_names,
            candidates: self.candidates.map(|spaces| {
                spaces
                    .into_iter()
                    .map(|s| SpaceSeed {
                        name: s.name,
                        spec: crate::zones::ZoneSpec {
                            surface: Default::default(),
                            purpose: s.spec.purpose,
                            outline: s.spec.outline.into_iter().map(Into::into).collect(),
                            holes: s
                                .spec
                                .holes
                                .into_iter()
                                .map(|h| h.into_iter().map(Into::into).collect())
                                .collect(),
                            height: s.spec.height,
                            bottom_offset: s.spec.bottom_offset,
                            label_position: s.spec.label_position.map(Into::into),
                        },
                    })
                    .collect()
            }),
            story: self.story,
            layer: self.layer,
        }
    }
}
pub fn schema() -> Value {
    serde_json::to_value(schemars::schema_for!(Prepare)).expect("intent schema serializable")
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MassParameters {
    pub outline: Vec<XY>,
    #[serde(default)]
    pub holes: Vec<Vec<XY>>,
    pub height: f64,
    #[serde(default)]
    pub bottom_offset: f64,
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NamedMass {
    pub name: String,
    pub spec: MassParameters,
    pub material: crate::automation_registry::NativeMaterial,
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MassDraft {
    pub name: String,
    pub question: String,
    pub expected_result: String,
    pub images: Vec<ImageReference>,
    pub claims: Vec<Claim>,
    #[serde(default)]
    pub geometry_assumptions: String,
    #[serde(default)]
    pub statements: Vec<Statement>,
    pub candidate_names: [String; 2],
    pub candidates: [Vec<NamedMass>; 2],
    pub story: u32,
    pub layer: u32,
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrepareMass {
    pub target: Target,
    pub expected_revision: u64,
    pub expected_context_revision: String,
    pub expected_sources_revision: String,
    pub task_id: String,
    pub draft: MassDraft,
}
impl MassDraft {
    pub fn lower(self) -> intent::MassDraft {
        intent::MassDraft {
            name: self.name,
            question: self.question,
            expected_result: self.expected_result,
            images: self.images,
            claims: self.claims,
            geometry_assumptions: self.geometry_assumptions,
            statements: self.statements,
            candidate_names: self.candidate_names,
            story: self.story,
            layer: self.layer,
            candidates: self.candidates.map(|seeds| {
                seeds
                    .into_iter()
                    .map(|s| intent::MassSeed {
                        name: s.name,
                        material: s.material.into(),
                        spec: crate::concept_mass::ConceptMassSpec {
                            arcs: vec![],
                            hole_arcs: vec![],
                            outline: s.spec.outline.into_iter().map(Into::into).collect(),
                            holes: s
                                .spec
                                .holes
                                .into_iter()
                                .map(|r| r.into_iter().map(Into::into).collect())
                                .collect(),
                            height: s.spec.height,
                            bottom_offset: s.spec.bottom_offset,
                        },
                    })
                    .collect()
            }),
        }
    }
}
pub fn mass_schema() -> Value {
    serde_json::to_value(schemars::schema_for!(PrepareMass))
        .expect("mass intent schema serializable")
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UpdateGeometry {
    Zone { spec: ZoneParameters },
    Mass { spec: MassParameters },
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ElementUpdate {
    pub id: u64,
    pub geometry: UpdateGeometry,
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateDraft {
    pub instruction: String,
    pub expected_result: String,
    pub sources: Vec<crate::design_process::update::Source>,
    pub claims: Vec<Claim>,
    #[serde(default)]
    pub statements: Vec<Statement>,
    pub expected_comparison: String,
    pub updates: Vec<ElementUpdate>,
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrepareUpdate {
    pub target: Target,
    pub expected_revision: u64,
    pub expected_context_revision: String,
    pub expected_sources_revision: String,
    pub task_id: String,
    pub update_id: String,
    pub draft: UpdateDraft,
}
impl UpdateDraft {
    pub fn lower(self, editor: &Editor) -> Result<crate::design_process::update::Draft, String> {
        let elements = self
            .updates
            .into_iter()
            .map(|update| {
                let mut element = editor
                    .project
                    .elements
                    .iter()
                    .find(|e| e.id == update.id)
                    .cloned()
                    .ok_or("Task update element is missing")?;
                element.shape = match update.geometry {
                    UpdateGeometry::Mass { spec } => Shape::ConceptMass {
                        spec: crate::concept_mass::ConceptMassSpec {
                            arcs: vec![],
                            hole_arcs: vec![],
                            outline: spec.outline.into_iter().map(Into::into).collect(),
                            holes: spec
                                .holes
                                .into_iter()
                                .map(|ring| ring.into_iter().map(Into::into).collect())
                                .collect(),
                            height: spec.height,
                            bottom_offset: spec.bottom_offset,
                        },
                    },
                    UpdateGeometry::Zone { spec } => Shape::Zone {
                        spec: crate::zones::ZoneSpec {
                            surface: match &element.shape {
                                Shape::Zone { spec } => spec.surface.clone(),
                                _ => Default::default(),
                            },
                            purpose: spec.purpose,
                            outline: spec.outline.into_iter().map(Into::into).collect(),
                            holes: spec
                                .holes
                                .into_iter()
                                .map(|ring| ring.into_iter().map(Into::into).collect())
                                .collect(),
                            height: spec.height,
                            bottom_offset: spec.bottom_offset,
                            label_position: spec.label_position.map(Into::into),
                        },
                    },
                };
                Ok(element)
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(crate::design_process::update::Draft {
            instruction: self.instruction,
            expected_result: self.expected_result,
            sources: self.sources,
            claims: self.claims,
            statements: self.statements,
            expected_comparison: self.expected_comparison,
            elements,
            entry: crate::design_process::Entry::AgentProposal,
        })
    }
}
pub fn update_schema() -> Value {
    serde_json::to_value(schemars::schema_for!(PrepareUpdate))
        .expect("task update schema serializable")
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum ParametricAction {
    Create {
        name: String,
        question: String,
        definition: crate::design_process::parametric::Definition,
        story: u32,
        layer: u32,
    },
    Update {
        update_id: String,
        instruction: String,
        parameters: crate::design_process::parametric::Parameters,
    },
    UpdateReviewedInputs {
        update_id: String,
        instruction: String,
        parameters: crate::design_process::parametric::Parameters,
        inputs: crate::design_process::parametric::Inputs,
    },
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrepareParametric {
    pub target: Target,
    pub expected_revision: u64,
    pub expected_context_revision: String,
    pub expected_sources_revision: String,
    pub task_id: String,
    pub proposal: ParametricAction,
}
impl ParametricAction {
    pub fn prepare(
        self,
        editor: &Editor,
        task_id: &str,
    ) -> Result<crate::model_service::PreparedChange, String> {
        use crate::design_process::parametric;
        match self {
            Self::UpdateReviewedInputs {
                update_id,
                instruction,
                parameters,
                inputs,
            } => parametric::prepare_update_with_inputs(
                editor,
                &update_id,
                task_id,
                parameters,
                instruction,
                Some(inputs),
                crate::design_process::Entry::AgentProposal,
            ),
            Self::Create {
                name,
                question,
                definition,
                story,
                layer,
            } => parametric::prepare_with_entry(
                editor,
                task_id,
                name,
                question,
                definition,
                story,
                layer,
                crate::design_process::Entry::AgentProposal,
            ),
            Self::Update {
                update_id,
                instruction,
                parameters,
            } => parametric::prepare_update_with_entry(
                editor,
                &update_id,
                task_id,
                parameters,
                instruction,
                crate::design_process::Entry::AgentProposal,
            ),
        }
    }
}
pub fn parametric_schema() -> Value {
    serde_json::to_value(schemars::schema_for!(PrepareParametric))
        .expect("parametric schema serializable")
}
