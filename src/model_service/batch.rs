//! Versioned request adapter. Geometry and transactions stay in ModelCommand/Editor.
use super::{ChangeError, PreparedChange, error};
use crate::{
    command::ModelCommand,
    editor::Editor,
    model::{Element, Point, Shape},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

pub const BATCH_CHANGE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ElementReference {
    Existing(u64),
    Temporary(String),
}
/// An ordinary persistent element's fields, with controlled identity placeholders.
/// id must be zero; Opening.host_wall must be zero with an explicit host reference.
/// Lengths are SI metres; angle fields keep their established degree conventions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElementTemplate {
    pub element: Element,
    pub host: Option<ElementReference>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedElement {
    pub name: String,
    pub template: ElementTemplate,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReferencedCommand {
    Constraint {
        edit: crate::constraints::Edit,
    },
    MassSpaces {
        edit: crate::mass_spaces::Edit,
    },
    Diagram {
        edit: crate::diagrams::Edit,
    },
    Rules {
        edit: crate::rules::RuleEdit,
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
    Update {
        target: ElementReference,
        template: ElementTemplate,
    },
    Move {
        targets: Vec<ElementReference>,
        offset: Point,
    },
    Remove {
        targets: Vec<ElementReference>,
    },
}
/// Creates are resolved as one AddElements batch, then edits run in order.
/// Temporary references are local to this request, including forward hosts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchChange {
    pub version: u32,
    pub creates: Vec<NamedElement>,
    pub edits: Vec<ReferencedCommand>,
}
impl Default for BatchChange {
    fn default() -> Self {
        Self {
            version: BATCH_CHANGE_VERSION,
            creates: vec![],
            edits: vec![],
        }
    }
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}
fn resolve(
    reference: &ElementReference,
    names: &BTreeMap<String, u64>,
    existing: &HashSet<u64>,
) -> Result<u64, ChangeError> {
    match reference {
        ElementReference::Existing(id) if *id != 0 && existing.contains(id) => Ok(*id),
        ElementReference::Temporary(name) if names.contains_key(name) => Ok(names[name]),
        _ => Err(error(
            "REFERENCE_NOT_FOUND",
            "Element reference is not present in the baseline or this batch",
        )),
    }
}
fn materialize(
    template: &ElementTemplate,
    id: u64,
    names: &BTreeMap<String, u64>,
    existing: &HashSet<u64>,
) -> Result<Element, ChangeError> {
    if template.element.id != 0 {
        return Err(error(
            "INVALID_TEMPLATE_ID",
            "Element template id must be zero",
        ));
    }
    let mut element = template.element.clone();
    element.id = id;
    match (&mut element.shape, &template.host) {
        (Shape::Opening { host_wall, .. }, Some(reference)) if *host_wall == 0 => {
            *host_wall = resolve(reference, names, existing)?;
        }
        (Shape::Opening { .. }, _) => {
            return Err(error(
                "INVALID_HOST_REFERENCE",
                "Opening template requires host_wall=0 and an explicit host reference",
            ));
        }
        (_, Some(_)) => {
            return Err(error(
                "INVALID_HOST_REFERENCE",
                "Only an opening accepts a host reference",
            ));
        }
        (_, None) => {}
    }
    Ok(element)
}
impl Editor {
    /// Resolve symbolic inputs and delegate all actual operations to prepare_change.
    /// No ID, selection, status, history or authoritative project is changed.
    pub fn prepare_batch_change(
        &self,
        expected_revision: u64,
        request: &BatchChange,
    ) -> Result<PreparedChange, ChangeError> {
        self.prepare_batch_change_admitted(expected_revision, request, None)
    }
    pub(crate) fn prepare_batch_change_admitted(
        &self,
        expected_revision: u64,
        request: &BatchChange,
        admission: Option<&crate::project_repository::constraints::ConstraintCommitGuard>,
    ) -> Result<PreparedChange, ChangeError> {
        let (commands, names) = self.resolve_batch_commands(expected_revision, request)?;
        let mut prepared = match admission {
            Some(guard) => self.prepare_constraint_change(expected_revision, commands, guard)?,
            None => self.prepare_change(expected_revision, commands)?,
        };
        prepared.batch_input = Some(
            crate::derived_geometry::input_fingerprint(request, 0.)
                .map_err(|m| error("INVALID_BATCH_INPUT", m))?,
        );
        prepared.resolved_references = names;
        Ok(prepared)
    }
    pub(crate) fn resolve_batch_commands(
        &self,
        expected_revision: u64,
        request: &BatchChange,
    ) -> Result<(Vec<ModelCommand>, BTreeMap<String, u64>), ChangeError> {
        if expected_revision != self.revision {
            return Err(error(
                "REVISION_CONFLICT",
                "Model changed before preparation",
            ));
        }
        if request.version != BATCH_CHANGE_VERSION {
            return Err(error(
                "UNSUPPORTED_BATCH_VERSION",
                "Unsupported batch request version",
            ));
        }
        if request.creates.len() > 100_000 || request.edits.len() > 100_000 {
            return Err(error(
                "INVALID_BATCH_SIZE",
                "Batch exceeds 100000 creates or edits",
            ));
        }
        let existing: HashSet<_> = self.project.elements.iter().map(|e| e.id).collect();
        let mut names = BTreeMap::new();
        let mut next_id = self.project.next_id;
        for create in &request.creates {
            if !valid_name(&create.name) {
                return Err(error(
                    "INVALID_TEMPORARY_NAME",
                    "Temporary names require 1-64 ASCII letters, digits, underscore, dot or hyphen",
                ));
            }
            if names.contains_key(&create.name) {
                return Err(error(
                    "DUPLICATE_TEMPORARY_NAME",
                    "Temporary name is already declared",
                ));
            }
            if next_id == 0 || existing.contains(&next_id) {
                return Err(error(
                    "INVALID_ELEMENT_ID",
                    "Project next_id cannot allocate a fresh identity",
                ));
            }
            let id = next_id;
            next_id = next_id
                .checked_add(1)
                .ok_or_else(|| error("ID_EXHAUSTED", "Element identity cannot advance"))?;
            names.insert(create.name.clone(), id);
        }
        let mut commands = Vec::new();
        if !request.creates.is_empty() {
            let elements = request
                .creates
                .iter()
                .map(|c| materialize(&c.template, names[&c.name], &names, &existing))
                .collect::<Result<_, _>>()?;
            commands.push(ModelCommand::AddElements {
                elements,
                group: false,
            });
        }
        for edit in &request.edits {
            commands.push(match edit {
                ReferencedCommand::MassSpaces { edit } => {
                    ModelCommand::MassSpaces { edit: edit.clone() }
                }
                ReferencedCommand::Diagram { edit } => ModelCommand::Diagram { edit: edit.clone() },
                ReferencedCommand::Terrain { edit } => ModelCommand::Terrain { edit: edit.clone() },
                ReferencedCommand::Constraint { edit } => {
                    ModelCommand::Constraint { edit: edit.clone() }
                }
                ReferencedCommand::Rules { edit } => ModelCommand::Rules { edit: edit.clone() },
                ReferencedCommand::Evidence { edit } => {
                    ModelCommand::Evidence { edit: edit.clone() }
                }
                ReferencedCommand::Program { edit } => ModelCommand::Program { edit: edit.clone() },
                ReferencedCommand::ApplySiteBoundary { proposal } => {
                    ModelCommand::ApplySiteBoundary {
                        proposal: proposal.clone(),
                    }
                }
                ReferencedCommand::DetachSiteBoundary => ModelCommand::DetachSiteBoundary,
                ReferencedCommand::ApplySiteControlRegion { proposal } => {
                    ModelCommand::ApplySiteControlRegion {
                        proposal: proposal.clone(),
                    }
                }
                ReferencedCommand::DetachSiteControlRegion => ModelCommand::DetachSiteControlRegion,
                ReferencedCommand::SetSiteContext { context } => ModelCommand::SetSiteContext {
                    context: context.clone(),
                },
                ReferencedCommand::Set3dStyle { style } => style.command(&self.project),
                ReferencedCommand::SetStoryElevation { story, elevation } => {
                    ModelCommand::SetStoryElevation {
                        story: *story,
                        elevation: *elevation,
                    }
                }
                ReferencedCommand::Update { target, template } => ModelCommand::UpdateElement {
                    element: materialize(
                        template,
                        resolve(target, &names, &existing)?,
                        &names,
                        &existing,
                    )?,
                },
                ReferencedCommand::Move { targets, offset } => ModelCommand::MoveElements {
                    ids: targets
                        .iter()
                        .map(|r| resolve(r, &names, &existing))
                        .collect::<Result<_, _>>()?,
                    offset: *offset,
                },
                ReferencedCommand::Remove { targets } => ModelCommand::RemoveElements {
                    ids: targets
                        .iter()
                        .map(|r| resolve(r, &names, &existing))
                        .collect::<Result<_, _>>()?,
                },
            });
        }
        Ok((commands, names))
    }
}
