//! Site outline references an ordinary editable Polyline, never a second geometry store.
use super::{HorizontalReference, LengthUnit};
use crate::model::{Material, Point, Project, Shape};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
/// A role selects metadata only; geometry remains in the shared model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Role {
    #[default]
    Site,
    ControlRegion,
}
impl Role {
    pub fn get(self, site: &super::SiteContext) -> Option<&SiteBoundary> {
        match self {
            Self::Site => site.boundary.as_ref(),
            Self::ControlRegion => site.control_region.as_ref(),
        }
    }
    fn set(self, site: &mut super::SiteContext, value: Option<SiteBoundary>) {
        match self {
            Self::Site => site.boundary = value,
            Self::ControlRegion => site.control_region = value,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Placement {
    ModelAxes,
    Projected { reference: HorizontalReference },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoundaryOrigin {
    pub source_sha256: String,
    pub entity_handle: String,
    pub record_sha256: String,
    pub unit: LengthUnit,
    pub placement: Placement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub centerline_width_native: Option<f64>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SiteBoundary {
    pub name: String,
    pub element_id: u64,
    pub origin: BoundaryOrigin,
    pub imported_outline_m: Vec<[f64; 2]>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoundaryProposal {
    pub name: String,
    pub story: u32,
    pub layer: u32,
    pub outline_m: Vec<[f64; 2]>,
    pub origin: BoundaryOrigin,
    pub expected_previous: Option<SiteBoundary>,
}
fn points(input: &[[f64; 2]]) -> Vec<Point> {
    input.iter().map(|p| Point::new(p[0], p[1])).collect()
}
fn validate_outline(input: &[[f64; 2]]) -> Result<(), String> {
    if input.len() > 10000 {
        return Err("Site boundary exceeds 10000 vertices".into());
    }
    crate::slab_geometry::validate(&points(input))
        .map_err(|e| format!("Invalid site boundary: {e}"))
}
fn validate_origin(origin: &BoundaryOrigin) -> Result<(), String> {
    if origin
        .centerline_width_native
        .is_some_and(|w| !w.is_finite() || w <= 0. || w > 1e12)
    {
        return Err("Invalid omitted boundary width".into());
    }
    if !crate::storage::valid_document_identity(&origin.source_sha256)
        || !crate::storage::valid_document_identity(&origin.record_sha256)
        || origin.entity_handle.is_empty()
        || origin.entity_handle.len() > 16
        || u64::from_str_radix(&origin.entity_handle, 16)
            .ok()
            .is_none_or(|n| n == 0)
    {
        return Err("Invalid boundary source identity".into());
    }
    if let Placement::Projected { reference } = &origin.placement {
        let site = super::SiteContext {
            horizontal: Some(reference.clone()),
            ..Default::default()
        };
        site.validate()?;
    }
    Ok(())
}
impl SiteBoundary {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() || self.name.len() > 256 || self.element_id == 0 {
            return Err("Invalid site boundary identity".into());
        }
        validate_origin(&self.origin)?;
        validate_outline(&self.imported_outline_m)
    }
}
impl BoundaryProposal {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() || self.name.len() > 256 {
            return Err("Name the site boundary within 256 bytes".into());
        }
        validate_origin(&self.origin)?;
        validate_outline(&self.outline_m)?;
        if let Some(previous) = &self.expected_previous {
            previous.validate()?;
        }
        Ok(())
    }
}
pub fn outline(project: &Project) -> Result<Option<Vec<[f64; 2]>>, String> {
    outline_for(project, Role::Site)
}
pub fn outline_for(project: &Project, role: Role) -> Result<Option<Vec<[f64; 2]>>, String> {
    let Some(boundary) = role.get(&project.site) else {
        return Ok(None);
    };
    let element = project
        .elements
        .iter()
        .find(|e| e.id == boundary.element_id)
        .ok_or(if role == Role::Site {
            "Detach site boundary before deleting its outline"
        } else {
            "Detach control region before deleting its outline"
        })?;
    if element.worksheet.is_some() {
        return Err("Site boundary must be in model space".into());
    }
    let Shape::Polyline { points, arcs } = &element.shape else {
        return Err("Site boundary must reference a Polyline".into());
    };
    if !arcs.is_empty() || points.len() < 4 || points.first() != points.last() {
        return Err("Site boundary requires a straight closed outline".into());
    }
    let result = points[..points.len() - 1]
        .iter()
        .map(|p| [p.x, p.y])
        .collect::<Vec<_>>();
    validate_outline(&result)?;
    Ok(Some(result))
}
pub fn area(project: &Project) -> Result<Option<f64>, String> {
    Ok(outline(project)?.map(|p| crate::slab_geometry::area(&points(&p))))
}
pub fn user_modified(project: &Project) -> Result<bool, String> {
    user_modified_for(project, Role::Site)
}
pub fn user_modified_for(project: &Project, role: Role) -> Result<bool, String> {
    Ok(
        match (role.get(&project.site), outline_for(project, role)?) {
            (Some(boundary), Some(points)) => points != boundary.imported_outline_m,
            _ => false,
        },
    )
}
pub fn apply(project: &Project, proposal: &BoundaryProposal) -> Result<Project, String> {
    apply_for(project, proposal, Role::Site)
}
pub fn apply_for(
    project: &Project,
    proposal: &BoundaryProposal,
    role: Role,
) -> Result<Project, String> {
    proposal.validate()?;
    if role.get(&project.site) != proposal.expected_previous.as_ref() {
        return Err("Site boundary changed after preview".into());
    }
    if let Placement::Projected { reference } = &proposal.origin.placement {
        if project.site.horizontal.as_ref() != Some(reference) {
            return Err("Site reference changed after boundary preview".into());
        }
        if !matches!(
            project.site.control_check()?,
            super::ControlCheck::Measured {
                consistent: true,
                ..
            }
        ) {
            return Err(
                "Projected import requires two consistent controls and a stated tolerance".into(),
            );
        }
    }
    if user_modified_for(project, role)? {
        return Err((if role == Role::Site {
            "Reimport refused: the site boundary contains user edits"
        } else {
            "Reimport refused: the control region contains user edits"
        })
        .into());
    }
    if !project.stories.iter().any(|s| s.id == proposal.story)
        || !project.layers.layers.iter().any(|l| l.id == proposal.layer)
        || project.layers.is_locked(proposal.layer)
    {
        return Err("Choose an existing unlocked boundary layer and story".into());
    }
    let mut next = project.clone();
    let mut closed = points(&proposal.outline_m);
    closed.push(closed[0]);
    let id = if let Some(previous) = role.get(&project.site) {
        let old = project
            .elements
            .iter()
            .find(|e| e.id == previous.element_id)
            .ok_or("Site boundary outline is missing")?;
        if project.edit_state.locked.contains(&old.id)
            || project.layers.is_locked(old.layer.0)
            || project.drawings.reference_owner(old.id).is_some()
        {
            return Err("Site boundary outline is locked".into());
        }
        let element = next.elements.iter_mut().find(|e| e.id == old.id).unwrap();
        element.shape = Shape::Polyline {
            points: closed,
            arcs: vec![],
        };
        if old.story != proposal.story || old.layer.0 != proposal.layer {
            return Err("Reimport keeps the existing outline layer and story".into());
        }
        old.id
    } else {
        next.next_id
            .checked_add(1)
            .ok_or("Element identity exhausted")?;
        let id = next.add(
            proposal.story,
            Material::Plaster,
            Shape::Polyline {
                points: closed,
                arcs: vec![],
            },
        );
        next.elements.iter_mut().find(|e| e.id == id).unwrap().layer =
            crate::layers::ElementLayer::new(proposal.layer);
        next.elements
            .iter_mut()
            .find(|e| e.id == id)
            .unwrap()
            .worksheet = None;
        if role == Role::ControlRegion {
            next.elements.iter_mut().find(|e| e.id == id).unwrap().name = proposal.name.clone();
        }
        id
    };
    role.set(
        &mut next.site,
        Some(SiteBoundary {
            name: proposal.name.clone(),
            element_id: id,
            origin: proposal.origin.clone(),
            imported_outline_m: proposal.outline_m.clone(),
        }),
    );
    next.validate()?;
    Ok(next)
}
