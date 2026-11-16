//! Filesystem/source guard around an immutable model boundary proposal.
use super::*;
use crate::{
    model::Project,
    site::boundary::{BoundaryOrigin, BoundaryProposal, Placement},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundaryPlacement {
    ModelAxes,
    ProjectedReference,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BoundaryWidthPolicy {
    #[default]
    RejectNonzero,
    ConstantWidthCenterline,
}
pub struct BoundaryCommitGuard {
    _lock: Lock,
}
impl Repository {
    pub fn boundary_entities(
        &self,
        source_id: &str,
    ) -> Result<Vec<inventory::EntityRecord>, String> {
        self.unchanged()?;
        let source = self
            .manifest
            .sources
            .iter()
            .find(|s| s.id == source_id && s.format == "dxf")
            .ok_or("Choose a stored DXF source")?;
        let bytes = resource(
            &self.root,
            source
                .inventory
                .as_ref()
                .ok_or("Source inventory is unavailable")?,
            MAX_SOURCE,
        )?;
        let inventory: inventory::Inventory =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        Ok(inventory
            .entities
            .into_iter()
            .filter(|r| r.kind == "LWPOLYLINE" && r.handle.is_some())
            .collect())
    }

    pub fn preview_boundary(
        &self,
        project: &Project,
        source_id: &str,
        entity: &str,
        placement: BoundaryPlacement,
        name: String,
        story: u32,
        layer: u32,
    ) -> Result<BoundaryProposal, String> {
        self.preview_boundary_with_width(
            project,
            source_id,
            entity,
            placement,
            name,
            story,
            layer,
            BoundaryWidthPolicy::RejectNonzero,
        )
    }
    pub fn preview_boundary_with_width(
        &self,
        project: &Project,
        source_id: &str,
        entity: &str,
        placement: BoundaryPlacement,
        name: String,
        story: u32,
        layer: u32,
        width_policy: BoundaryWidthPolicy,
    ) -> Result<BoundaryProposal, String> {
        self.preview_reference_region(
            project,
            source_id,
            entity,
            placement,
            name,
            story,
            layer,
            width_policy,
            crate::site::boundary::Role::Site,
        )
    }
    pub fn preview_reference_region(
        &self,
        project: &Project,
        source_id: &str,
        entity: &str,
        placement: BoundaryPlacement,
        name: String,
        story: u32,
        layer: u32,
        width_policy: BoundaryWidthPolicy,
        role: crate::site::boundary::Role,
    ) -> Result<BoundaryProposal, String> {
        if role.get(&project.site).is_some_and(|_| {
            crate::site::boundary::user_modified_for(project, role).unwrap_or(true)
        }) {
            return Err((if role == crate::site::boundary::Role::Site {
                "Reimport refused: the site boundary contains user edits"
            } else {
                "Reimport refused: the control region contains user edits"
            })
            .into());
        }
        let _lock = Lock::acquire(&self.root)?;
        self.unchanged()?;
        let source = self
            .manifest
            .sources
            .iter()
            .find(|s| s.id == source_id && s.format == "dxf")
            .ok_or("Choose a stored DXF source")?;
        if matches!(source.quality, SourceQuality::Rejected { .. }) {
            return Err("Rejected source cannot supply site geometry".into());
        }
        let unit = source
            .confirmed_unit
            .ok_or("Confirm drawing units before boundary preview")?;
        let bytes = resource(&self.root, &source.blob, MAX_SOURCE)?;
        let extracted = crate::dxf::primitives::closed_outline_with_width(
            &bytes,
            entity,
            width_policy == BoundaryWidthPolicy::ConstantWidthCenterline,
        )?;
        let frame = match placement {
            BoundaryPlacement::ModelAxes => Placement::ModelAxes,
            BoundaryPlacement::ProjectedReference => {
                if !matches!(
                    project.site.control_check()?,
                    crate::site::ControlCheck::Measured {
                        consistent: true,
                        ..
                    }
                ) {
                    return Err(
                        "Projected import requires two consistent controls and a stated tolerance"
                            .into(),
                    );
                }
                Placement::Projected {
                    reference: project
                        .site
                        .horizontal
                        .clone()
                        .ok_or("Projected reference is unknown")?,
                }
            }
        };
        let outline = extracted
            .points
            .iter()
            .map(|p| {
                let xy = [p[0] * unit.metres(), p[1] * unit.metres()];
                match placement {
                    BoundaryPlacement::ModelAxes => Ok(xy),
                    BoundaryPlacement::ProjectedReference => project.site.from_reference_xy(xy),
                }
            })
            .collect::<Result<Vec<_>, String>>()?;
        let proposal = BoundaryProposal {
            name,
            story,
            layer,
            outline_m: outline,
            origin: BoundaryOrigin {
                source_sha256: source.id.clone(),
                entity_handle: extracted.handle,
                record_sha256: extracted.record_sha256,
                unit,
                placement: frame,
                centerline_width_native: (extracted.constant_width_native != 0.)
                    .then_some(extracted.constant_width_native),
            },
            expected_previous: role.get(&project.site).cloned(),
        };
        proposal.validate()?;
        self.unchanged()?;
        Ok(proposal)
    }
    /// The ready token is Send and keeps the original repository locked until UI commit/discard.
    pub fn guard_boundary_commit(
        &self,
        proposal: &BoundaryProposal,
    ) -> Result<BoundaryCommitGuard, String> {
        let lock = Lock::acquire(&self.root)?;
        self.verify_boundary_unlocked(proposal)?;
        Ok(BoundaryCommitGuard { _lock: lock })
    }
    pub(super) fn verify_boundary_unlocked(
        &self,
        proposal: &BoundaryProposal,
    ) -> Result<(), String> {
        self.unchanged()?;
        let source = self
            .manifest
            .sources
            .iter()
            .find(|s| s.id == proposal.origin.source_sha256)
            .ok_or("Boundary source no longer exists")?;
        if matches!(source.quality, SourceQuality::Rejected { .. })
            || source.confirmed_unit != Some(proposal.origin.unit)
        {
            return Err("Boundary source quality or units changed; preview again".into());
        }
        let bytes = resource(&self.root, &source.blob, MAX_SOURCE)?;
        let extracted = crate::dxf::primitives::closed_outline_with_width(
            &bytes,
            &proposal.origin.entity_handle,
            proposal.origin.centerline_width_native.is_some(),
        )?;
        if (extracted.constant_width_native != 0.).then_some(extracted.constant_width_native)
            != proposal.origin.centerline_width_native
        {
            return Err("Boundary omitted width differs from its source record".into());
        }
        if extracted.record_sha256 != proposal.origin.record_sha256 {
            return Err("Boundary source record changed; preview again".into());
        }
        let frame = crate::site::SiteContext {
            horizontal: match &proposal.origin.placement {
                Placement::ModelAxes => None,
                Placement::Projected { reference } => Some(reference.clone()),
            },
            ..Default::default()
        };
        let expected = extracted
            .points
            .iter()
            .map(|p| {
                let xy = [
                    p[0] * proposal.origin.unit.metres(),
                    p[1] * proposal.origin.unit.metres(),
                ];
                match &proposal.origin.placement {
                    Placement::ModelAxes => Ok(xy),
                    Placement::Projected { .. } => frame.from_reference_xy(xy),
                }
            })
            .collect::<Result<Vec<_>, String>>()?;
        if expected != proposal.outline_m {
            return Err("Boundary proposal differs from its source record".into());
        }

        Ok(())
    }
}
