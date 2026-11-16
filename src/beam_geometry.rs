//! A single source of beam surfaces and quantities over the existing parameters.
use crate::{
    beam::{BeamGeometry, BeamMesh, BeamSolid},
    derived_geometry::{self, GeometryDiagnostic, GeometryIdentity, SurfaceMetrics, VolumeSource},
    model::{Element, Point, Project},
};
use glam::DVec3;
pub const BEAM_DERIVATION_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq)]
pub struct BeamSnapshot {
    parameters: BeamGeometry,
    pub solid: BeamSolid,
    pub mesh: BeamMesh,
    pub bounds: [DVec3; 2],
    pub plan_path: Vec<Point>,
    pub plan_triangles: Vec<[Point; 3]>,
    pub plan_area_m2: f64,
    pub volume_m3: f64,
    pub sampled_volume_m3: f64,
    pub volume_source: VolumeSource,
}
impl BeamSnapshot {
    pub fn parameters(&self) -> BeamGeometry {
        self.parameters
    }

    pub fn clipped(
        &self,
        polygon: Option<&[Point]>,
        z_range: Option<(f64, f64)>,
    ) -> Result<BeamMesh, String> {
        crate::geometry_validation::validate_clip_context(polygon, z_range)?;
        if polygon.is_some_and(|p| p.len() < 3 || p.iter().any(|p| !p.finite()))
            || z_range.is_some_and(|(a, b)| !a.is_finite() || !b.is_finite())
        {
            return Err("Invalid beam clipping polygon or heights".into());
        }
        if polygon.is_none() && z_range.is_none() {
            return Ok(self.mesh.clone());
        }
        let mesh = self.parameters.mesh_source(polygon, z_range)?;
        if !mesh.triangles.is_empty() {
            derived_geometry::inspect_closed_surface(&mesh.triangles)?;
        }
        Ok(mesh)
    }
}
pub fn snapshot(parameters: BeamGeometry) -> Result<BeamSnapshot, String> {
    let solid = parameters.solid()?;
    let mesh = parameters.mesh_source(None, None)?;
    let metrics = derived_geometry::inspect_closed_surface(&mesh.triangles)?;
    Ok(BeamSnapshot {
        plan_path: parameters.outline()?,
        plan_triangles: derived_geometry::triangulate_with_boundary(&parameters.plan_polygon()?)
            .into_iter()
            .map(|t| t.map(|p| Point::new(p.x, p.y)))
            .collect(),
        plan_area_m2: crate::beam::plan_area(
            parameters.start,
            parameters.end,
            parameters.width,
            parameters.arc,
        ),
        volume_m3: solid.volume,
        sampled_volume_m3: metrics.signed_volume_m3,
        volume_source: VolumeSource::AnalyticPrism,
        parameters,
        solid,
        mesh,
        bounds: metrics.bounds,
    })
}
#[derive(Clone, Debug, PartialEq)]
pub struct DerivedBeam {
    pub identity: GeometryIdentity,
    pub geometry: BeamSnapshot,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ClippedBeam {
    pub identity: GeometryIdentity,
    pub polygon: Option<Vec<Point>>,
    pub z_range: Option<(f64, f64)>,
    pub mesh: BeamMesh,
    pub metrics: Option<SurfaceMetrics>,
}
impl DerivedBeam {
    pub fn clipped(
        &self,
        polygon: Option<&[Point]>,
        z_range: Option<(f64, f64)>,
    ) -> Result<ClippedBeam, GeometryDiagnostic> {
        let mesh = self.geometry.clipped(polygon, z_range).map_err(|m| {
            GeometryDiagnostic::error(self.identity.element_id, "BEAM_CLIP_FAILED", "solid", m)
        })?;
        let metrics = if mesh.triangles.is_empty() {
            None
        } else {
            Some(
                derived_geometry::inspect_closed_surface(&mesh.triangles).map_err(|m| {
                    GeometryDiagnostic::error(
                        self.identity.element_id,
                        "NON_MANIFOLD_BEAM",
                        "solid",
                        m,
                    )
                })?,
            )
        };
        Ok(ClippedBeam {
            identity: self.identity.clone(),
            polygon: polygon.map(<[Point]>::to_vec),
            z_range,
            mesh,
            metrics,
        })
    }
}
pub fn derive(project: &Project, e: &Element) -> Result<DerivedBeam, GeometryDiagnostic> {
    let parameters = crate::beam::geometry(project, e).ok_or_else(|| {
        GeometryDiagnostic::error(e.id, "NOT_BEAM", "element", "Element is not a beam")
    })?;
    let geometry = snapshot(parameters)
        .map_err(|m| GeometryDiagnostic::error(e.id, "INVALID_BEAM", "solid", m))?;
    let identity = derived_geometry::identity(
        e.id,
        &e.shape,
        project.elevation(e.story),
        BEAM_DERIVATION_VERSION,
    )
    .map_err(|m| GeometryDiagnostic::error(e.id, "BEAM_INPUT_IDENTITY", "parameters", m))?;
    Ok(DerivedBeam { identity, geometry })
}
