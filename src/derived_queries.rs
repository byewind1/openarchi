//! Queries over canonical snapshots; primitives and tolerances are shared with
//! geometry.rs. No parameter model or alternative surface generation is stored.
use crate::{
    derivation_cache::GeometrySource, derived_geometry::GeometryDiagnostic, geometry, model::Point,
};
use std::borrow::Cow;
impl GeometrySource {
    pub fn display_bounds(&self) -> Option<[glam::DVec3; 2]> {
        Some(match self {
            Self::Wall(g) => return g.geometry.bounds,
            Self::Opening(g) => return g.geometry.bounds,
            Self::ConceptMass(g) | Self::Zone(g) | Self::Slab(g) => g.bounds,
            Self::Roof(g) => g.geometry.bounds,
            Self::Beam(g) => g.geometry.bounds,
            Self::Column(g) => g.geometry.bounds,
            Self::Stair(g) => g.geometry.display_bounds,
            Self::Polyline(g) => g.geometry.bounds,
        })
    }
    pub(crate) fn plan_outer_outline(&self) -> Result<Cow<'_, [Point]>, GeometryDiagnostic> {
        Ok(match self {
            Self::Wall(g) => Cow::Borrowed(&g.geometry.plan_path),
            Self::Opening(g) => {
                Cow::Borrowed(g.geometry.plan_paths.first().map_or(&[], Vec::as_slice))
            }
            Self::ConceptMass(g) | Self::Zone(g) | Self::Slab(g) => {
                Cow::Borrowed(g.plan_paths.first().map_or(&[], Vec::as_slice))
            }
            Self::Roof(g) => {
                Cow::Borrowed(g.geometry.plan_paths.first().map_or(&[], Vec::as_slice))
            }
            Self::Beam(g) => Cow::Borrowed(&g.geometry.plan_path),
            Self::Stair(g) => Cow::Borrowed(&g.geometry.source.footprint),
            Self::Polyline(g) => Cow::Borrowed(&g.geometry.plan_path),
            Self::Column(g) => {
                let result = match g.geometry.source() {
                    crate::column_geometry::ColumnSource::Vertical(c) => c.outline(),
                    crate::column_geometry::ColumnSource::Inclined(c) => {
                        c.projected_outline(c.column.bottom, c.column.bottom + c.column.height)
                    }
                };
                Cow::Owned(result.map_err(|m| {
                    GeometryDiagnostic::error(
                        g.identity.element_id,
                        "INVALID_COLUMN_PLAN_CONTEXT",
                        "projection",
                        m,
                    )
                })?)
            }
        })
    }
    pub fn intersects_polygon(&self, polygon: &[Point]) -> Result<bool, GeometryDiagnostic> {
        if polygon.len() < 3 {
            return Ok(false);
        }
        Ok(match self {
            Self::ConceptMass(g) | Self::Zone(g) | Self::Slab(g) => {
                geometry::triangles_intersect_polygon(&g.plan_triangles, polygon)
            }
            Self::Roof(g) => {
                geometry::triangles_intersect_polygon(&g.geometry.plan_triangles, polygon)
            }
            Self::Column(g)
                if matches!(
                    g.geometry.source(),
                    crate::column_geometry::ColumnSource::Inclined(_)
                ) =>
            {
                !g.clipped(Some(polygon), None)?.mesh.triangles.is_empty()
            }
            Self::Polyline(g) => {
                let (nodes, _) = g.geometry.parameters();
                nodes.iter().any(|p| geometry::contains(*p, polygon))
                    || g.geometry.plan_path.windows(2).any(|w| {
                        (0..polygon.len()).any(|j| {
                            geometry::segments_intersect(
                                w[0],
                                w[1],
                                polygon[j],
                                polygon[(j + 1) % polygon.len()],
                            )
                        })
                    })
            }
            _ => geometry::closed_path_intersects_polygon(&self.plan_outer_outline()?, polygon),
        })
    }
    pub fn fully_inside_polygon(&self, polygon: &[Point]) -> Result<bool, GeometryDiagnostic> {
        let outline = self.plan_outer_outline()?;
        // The legacy polyline path is open, even when its endpoints coincide.
        Ok(geometry::path_fully_inside_polygon(
            &outline,
            !matches!(self, Self::Polyline(_)),
            polygon,
        ))
    }
}

fn ray_triangles(
    origin: glam::DVec3,
    direction: glam::DVec3,
    triangles: impl IntoIterator<Item = [glam::DVec3; 3]>,
) -> Option<f64> {
    triangles
        .into_iter()
        .filter_map(|[a, b, c]| geometry::ray_triangle(origin, direction, a, b, c))
        .min_by(f64::total_cmp)
}
impl GeometrySource {
    /// Analytic column rays remain distinct from sampled display meshes. Hosted
    /// voids are selectable proxies; display-only glass and rails are not solids.
    pub fn ray_hit(
        &self,
        origin: glam::DVec3,
        direction: glam::DVec3,
        polygon: Option<&[Point]>,
        range: Option<(f64, f64)>,
    ) -> Result<Option<f64>, GeometryDiagnostic> {
        let id = self.identity().element_id;
        if !origin.is_finite()
            || !direction.is_finite()
            || !direction.length_squared().is_finite()
            || direction.length_squared() == 0.
        {
            return Err(GeometryDiagnostic::error(
                id,
                "INVALID_RAY_QUERY",
                "query",
                "Ray origin and direction must be finite and nonzero",
            ));
        }
        crate::geometry_validation::validate_clip_context(polygon, range)
            .map_err(|m| GeometryDiagnostic::error(id, "INVALID_VIEW_CLIP", "clipping", m))?;
        if polygon.is_none() && range.is_none() {
            return Ok(match self {
                Self::Column(g) => g.geometry.ray_hit(origin, direction),
                Self::Wall(g) => {
                    ray_triangles(origin, direction, g.geometry.mesh.triangles.iter().copied())
                }
                Self::Opening(g) => ray_triangles(
                    origin,
                    direction,
                    g.geometry.selectable_proxy.triangles.iter().copied().chain(
                        g.geometry
                            .joinery
                            .iter()
                            .flat_map(|(_, p)| p.mesh.triangles.iter().copied()),
                    ),
                ),
                Self::ConceptMass(g) | Self::Zone(g) | Self::Slab(g) => {
                    ray_triangles(origin, direction, g.triangles())
                }
                Self::Roof(g) => {
                    ray_triangles(origin, direction, g.geometry.mesh.triangles.iter().copied())
                }
                Self::Beam(g) => {
                    ray_triangles(origin, direction, g.geometry.mesh.triangles.iter().copied())
                }
                Self::Stair(g) => ray_triangles(
                    origin,
                    direction,
                    g.geometry.source.triangles.iter().copied(),
                ),
                Self::Polyline(_) => None,
            });
        }
        let clip_failure = |code, m| GeometryDiagnostic::error(id, code, "clipping", m);
        Ok(match self {
            Self::Column(g) => {
                ray_triangles(origin, direction, g.clipped(polygon, range)?.mesh.triangles)
            }
            Self::Wall(g) => ray_triangles(
                origin,
                direction,
                g.geometry
                    .clipped(polygon, range)
                    .map_err(|m| clip_failure("WALL_CLIP_FAILED", m))?
                    .triangles,
            ),
            Self::Opening(g) => ray_triangles(
                origin,
                direction,
                g.geometry
                    .clipped_proxy(polygon, range)
                    .map_err(|m| clip_failure("OPENING_CLIP_FAILED", m))?
                    .triangles,
            ),
            Self::ConceptMass(g) | Self::Zone(g) | Self::Slab(g) => {
                let cover;
                let polygon = if let Some(p) = polygon {
                    p
                } else {
                    let [lo, hi] = g.bounds;
                    cover = vec![
                        Point::new(lo.x, lo.y),
                        Point::new(hi.x, lo.y),
                        Point::new(hi.x, hi.y),
                        Point::new(lo.x, hi.y),
                    ];
                    &cover
                };
                ray_triangles(origin, direction, g.clipped(polygon, range)?.triangles())
            }
            Self::Roof(g) => {
                ray_triangles(origin, direction, g.clipped(polygon, range)?.mesh.triangles)
            }
            Self::Beam(g) => {
                ray_triangles(origin, direction, g.clipped(polygon, range)?.mesh.triangles)
            }
            Self::Stair(g) => ray_triangles(
                origin,
                direction,
                g.clipped(polygon, range)?.geometry.mesh.triangles,
            ),
            Self::Polyline(_) => None,
        })
    }
}

impl GeometrySource {
    /// Matches the existing Inspector's nominal area/line-length convention.
    pub fn nominal_measure(&self) -> crate::derivation_cache::NominalMeasure {
        use crate::derivation_cache::NominalMeasure as M;
        match self {
            Self::Wall(g) => M::AreaM2(g.geometry.nominal_reference_area_m2),
            Self::Opening(g) => M::AreaM2(g.geometry.nominal_area_m2),
            Self::ConceptMass(g) | Self::Zone(g) | Self::Slab(g) => M::AreaM2(g.plan_area_m2),
            Self::Roof(g) => M::AreaM2(g.geometry.slope_area_m2),
            Self::Beam(g) => M::AreaM2(g.geometry.plan_area_m2),
            Self::Column(g) => M::AreaM2(g.geometry.normal_section_area_m2),
            Self::Stair(g) => M::AreaM2(g.geometry.plan_area_m2),
            Self::Polyline(g) => M::LengthM(g.geometry.length_m),
        }
    }
    /// Authored envelope, neither construction material nor habitable-space volume.
    pub fn concept_volume_m3(&self) -> Option<f64> {
        match self {
            Self::ConceptMass(g) => Some(g.volume_m3),
            _ => None,
        }
    }
    pub fn material_volume_m3(&self) -> Option<f64> {
        Some(match self {
            Self::Wall(g) => g.geometry.material_volume_m3,
            Self::Slab(g) => g.volume_m3,
            Self::Roof(g) => g.geometry.volume_m3,
            Self::Beam(g) => g.geometry.volume_m3,
            Self::Column(g) => g.geometry.volume_m3,
            Self::Stair(g) => g.geometry.material_volume_m3,
            Self::ConceptMass(_) | Self::Zone(_) | Self::Opening(_) | Self::Polyline(_) => {
                return None;
            }
        })
    }
    pub fn void_volume_m3(&self) -> Option<f64> {
        match self {
            Self::Opening(g) => Some(g.geometry.sampled_cut_volume_m3),
            _ => None,
        }
    }
}
