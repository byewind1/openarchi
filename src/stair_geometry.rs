//! Shared finite straight-flight body and display-line representation.
use crate::{
    column_clip::{ClipNormalization, normalize_clipped_mesh},
    derived_geometry::{self, GeometryDiagnostic, GeometryIdentity, SurfaceMesh},
    model::{Element, Point, Project, Shape},
    stairs::{self, StairGeometry, StairSpec},
};
use glam::DVec3;
pub const STAIR_DERIVATION_VERSION: u32 = 1;
#[derive(Clone, Debug, PartialEq)]
pub struct StairSnapshot {
    parameters: StairSpec,
    base: f64,
    pub source: StairGeometry,
    pub body_bounds: [DVec3; 2],
    pub display_bounds: [DVec3; 2],
    pub plan_triangles: [[Point; 3]; 2],
    pub plan_area_m2: f64,
    pub material_volume_m3: f64,
    pub sampled_volume_m3: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ClippedStairGeometry {
    pub mesh: SurfaceMesh,
    pub handrails: Vec<[DVec3; 2]>,
    pub body_bounds: Option<[DVec3; 2]>,
    pub normalization: ClipNormalization,
}
impl StairSnapshot {
    pub fn parameters(&self) -> &StairSpec {
        &self.parameters
    }
    pub fn clipped(
        &self,
        clip: Option<&[Point]>,
        range: Option<(f64, f64)>,
    ) -> Result<ClippedStairGeometry, String> {
        crate::geometry_validation::validate_clip_context(clip, range)?;
        if clip.is_some_and(|p| p.len() < 3 || p.iter().any(|p| !p.finite()))
            || range.is_some_and(|(a, b)| !a.is_finite() || !b.is_finite())
        {
            return Err("Invalid stair clipping context".into());
        }
        let (mesh, handrails) =
            stairs::clip_source(&self.parameters, self.base, &self.source, clip, range)?;
        let (mesh, normalization) = if clip.is_none() && range.is_none() {
            let report = ClipNormalization {
                algorithm_version: crate::column_clip::CLIP_ALGORITHM_VERSION,
                input_triangles: mesh.triangles.len(),
                output_triangles: mesh.triangles.len(),
                ..Default::default()
            };
            (mesh, report)
        } else {
            let result = normalize_clipped_mesh(mesh)?;
            (result.mesh, result.normalization)
        };
        let body_bounds = if mesh.triangles.is_empty() {
            None
        } else {
            Some(derived_geometry::inspect_closed_surface(&mesh.triangles)?.bounds)
        };
        if handrails.iter().flatten().any(|p| !p.is_finite()) {
            return Err("Invalid stair display lines".into());
        }
        Ok(ClippedStairGeometry {
            mesh,
            handrails,
            body_bounds,
            normalization,
        })
    }
}
pub fn snapshot(spec: &StairSpec, base: f64) -> Result<StairSnapshot, String> {
    let source = stairs::generate_source(spec, base).map_err(str::to_owned)?;
    let metrics = derived_geometry::inspect_closed_surface(&source.triangles)?;
    let material_volume_m3 = spec.material_volume_m3();
    if !crate::precision::TolerancePolicy::default()
        .volume_m3
        .accepts(metrics.signed_volume_m3, material_volume_m3)
    {
        return Err("Stair surface volume differs from parameter section".into());
    }
    let mut display_bounds = metrics.bounds;
    for &p in source.handrails.iter().flatten() {
        if !p.is_finite() {
            return Err("Invalid stair handrail".into());
        }
        display_bounds[0] = display_bounds[0].min(p);
        display_bounds[1] = display_bounds[1].max(p);
    }
    let p = source.footprint;
    Ok(StairSnapshot {
        parameters: spec.clone(),
        base,
        source,
        body_bounds: metrics.bounds,
        display_bounds,
        plan_triangles: [[p[0], p[1], p[2]], [p[0], p[2], p[3]]],
        plan_area_m2: spec.plan_area_m2(),
        material_volume_m3,
        sampled_volume_m3: metrics.signed_volume_m3,
    })
}
#[derive(Clone, Debug, PartialEq)]
pub struct DerivedStair {
    pub identity: GeometryIdentity,
    pub geometry: StairSnapshot,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ClippedStair {
    pub identity: GeometryIdentity,
    pub xy_clip: Option<Vec<Point>>,
    pub z_range: Option<(f64, f64)>,
    pub geometry: ClippedStairGeometry,
}
impl DerivedStair {
    pub fn clipped(
        &self,
        clip: Option<&[Point]>,
        range: Option<(f64, f64)>,
    ) -> Result<ClippedStair, GeometryDiagnostic> {
        let geometry = self.geometry.clipped(clip, range).map_err(|m| {
            GeometryDiagnostic::error(self.identity.element_id, "STAIR_CLIP_FAILED", "clipping", m)
        })?;
        Ok(ClippedStair {
            identity: self.identity.clone(),
            xy_clip: clip.map(<[Point]>::to_vec),
            z_range: range.map(|(a, b)| (a.min(b), a.max(b))),
            geometry,
        })
    }
}
pub fn derive(project: &Project, element: &Element) -> Result<DerivedStair, GeometryDiagnostic> {
    let fail = |m| GeometryDiagnostic::error(element.id, "INVALID_STAIR_GEOMETRY", "body", m);
    let Shape::Stair { spec } = &element.shape else {
        return Err(fail("Element is not a stair".into()));
    };
    let base = project
        .stories
        .iter()
        .find(|s| s.id == element.story)
        .ok_or_else(|| fail("Stair story does not exist".into()))?
        .elevation;
    let geometry = snapshot(spec, base).map_err(fail)?;
    let identity = derived_geometry::identity(element.id, spec, base, STAIR_DERIVATION_VERSION)
        .map_err(fail)?;
    Ok(DerivedStair { identity, geometry })
}
