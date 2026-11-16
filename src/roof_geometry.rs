//! Canonical, read-only single-plane roof snapshots over the original parameters.
use crate::{
    derived_geometry::{self, GeometryDiagnostic, GeometryIdentity, SurfaceMetrics, VolumeSource},
    model::{Element, Point, Shape},
    roof::{RoofMesh, RoofSpec},
};
use glam::DVec3;

pub const ROOF_DERIVATION_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq)]
pub struct RoofGeometry {
    pub mesh: RoofMesh,
    pub bounds: [DVec3; 2],
    pub plan_paths: Vec<Vec<Point>>,
    pub plan_triangles: Vec<[Point; 3]>,
    pub plan_area_m2: f64,
    pub slope_area_m2: f64,
    pub volume_m3: f64,
    pub sampled_volume_m3: f64,
    pub volume_source: VolumeSource,
    spec: RoofSpec,
    base: f64,
}
impl RoofGeometry {
    /// Preserve legacy eight sloped rectangle corners. General footprints use
    /// the actual occupied solid's bounding box, without evaluating blank corners.
    pub fn compatibility_corners(&self) -> [DVec3; 8] {
        if self.spec.contour.is_none() {
            let p = self.spec.outline();
            std::array::from_fn(|i| {
                let p = p[i % 4];
                DVec3::new(
                    p.x,
                    p.y,
                    self.spec.bottom(p, self.base) + if i < 4 { 0. } else { self.spec.thickness },
                )
            })
        } else {
            let [lo, hi] = self.bounds;
            std::array::from_fn(|i| {
                DVec3::new(
                    if matches!(i % 4, 1 | 2) { hi.x } else { lo.x },
                    if matches!(i % 4, 2 | 3) { hi.y } else { lo.y },
                    if i < 4 { lo.z } else { hi.z },
                )
            })
        }
    }

    /// An empty intersection is valid; invalid inputs and broken surfaces are errors.
    pub fn clipped(
        &self,
        polygon: Option<&[Point]>,
        z_range: Option<(f64, f64)>,
    ) -> Result<RoofMesh, String> {
        crate::geometry_validation::validate_clip_context(polygon, z_range)?;
        if let Some(p) = polygon {
            if p.len() < 3 || p.iter().any(|p| !p.finite()) {
                return Err("Invalid roof clipping polygon".into());
            }
        }
        if z_range.is_some_and(|(a, b)| !a.is_finite() || !b.is_finite()) {
            return Err("Invalid roof clipping heights".into());
        }
        if polygon.is_none() && z_range.is_none() {
            return Ok(self.mesh.clone());
        }
        let mesh = crate::sloped_prism::mesh(
            geo::MultiPolygon(vec![self.spec.footprint_contour().polygon()]),
            (self.spec.min, self.spec.max),
            self.spec.thickness,
            |p| self.spec.bottom(p, self.base),
            polygon,
            z_range,
        );
        if !mesh.triangles.is_empty() {
            derived_geometry::inspect_closed_surface(&mesh.triangles)?;
        }
        Ok(mesh)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DerivedRoof {
    pub identity: GeometryIdentity,
    pub geometry: RoofGeometry,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ClippedRoof {
    pub identity: GeometryIdentity,
    pub polygon: Option<Vec<Point>>,
    pub z_range: Option<(f64, f64)>,
    pub mesh: RoofMesh,
    pub metrics: Option<SurfaceMetrics>,
}
impl DerivedRoof {
    pub fn clipped(
        &self,
        polygon: Option<&[Point]>,
        z_range: Option<(f64, f64)>,
    ) -> Result<ClippedRoof, GeometryDiagnostic> {
        let mesh = self.geometry.clipped(polygon, z_range).map_err(|m| {
            GeometryDiagnostic::error(self.identity.element_id, "ROOF_CLIP_FAILED", "solid", m)
        })?;
        let metrics = if mesh.triangles.is_empty() {
            None
        } else {
            Some(
                derived_geometry::inspect_closed_surface(&mesh.triangles).map_err(|m| {
                    GeometryDiagnostic::error(
                        self.identity.element_id,
                        "NON_MANIFOLD_ROOF",
                        "solid",
                        m,
                    )
                })?,
            )
        };
        Ok(ClippedRoof {
            identity: self.identity.clone(),
            polygon: polygon.map(<[Point]>::to_vec),
            z_range,
            mesh,
            metrics,
        })
    }
}

/// Pure shape algorithm. It never fabricates an element or document identity.
pub fn geometry(spec: &RoofSpec, base: f64) -> Result<RoofGeometry, String> {
    spec.validate()?;
    if !base.is_finite() {
        return Err("Invalid roof story elevation".into());
    }
    let contour = spec.footprint_contour();
    let mesh = crate::sloped_prism::mesh(
        geo::MultiPolygon(vec![contour.polygon()]),
        (spec.min, spec.max),
        spec.thickness,
        |p| spec.bottom(p, base),
        None,
        None,
    );
    let metrics = derived_geometry::inspect_closed_surface(&mesh.triangles)?;
    let plan_area_m2 = contour.area();
    Ok(RoofGeometry {
        mesh,
        bounds: metrics.bounds,
        plan_paths: contour.rings().map(<[Point]>::to_vec).collect(),
        plan_triangles: crate::derived_geometry::triangulate_with_boundary(&contour.polygon())
            .into_iter()
            .map(|t| t.map(|p| Point::new(p.x, p.y)))
            .collect(),
        plan_area_m2,
        slope_area_m2: plan_area_m2 / spec.pitch.to_radians().cos(),
        volume_m3: plan_area_m2 * spec.thickness,
        sampled_volume_m3: metrics.signed_volume_m3,
        volume_source: VolumeSource::AnalyticPrism,
        spec: spec.clone(),
        base,
    })
}

pub fn derive(e: &Element, base: f64) -> Result<DerivedRoof, GeometryDiagnostic> {
    let Shape::Roof { spec } = &e.shape else {
        return Err(GeometryDiagnostic::error(
            e.id,
            "NOT_ROOF",
            "element",
            "Element is not a roof",
        ));
    };
    let geometry = geometry(spec, base)
        .map_err(|m| GeometryDiagnostic::error(e.id, "INVALID_ROOF", "solid", m))?;
    let identity = derived_geometry::identity(e.id, spec, base, ROOF_DERIVATION_VERSION)
        .map_err(|m| GeometryDiagnostic::error(e.id, "ROOF_INPUT_IDENTITY", "parameters", m))?;
    Ok(DerivedRoof { identity, geometry })
}
