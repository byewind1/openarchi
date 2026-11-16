//! Analytic column source and its explicitly sampled display, with shared identity.
use crate::{
    column::{ColumnMesh, ColumnSolid},
    column_inclination::InclinedColumnSolid,
    derived_geometry::{self, GeometryDiagnostic, GeometryIdentity, SurfaceMetrics, VolumeSource},
    model::{Element, Point, Project, Shape},
};
use glam::DVec3;
pub const COLUMN_DERIVATION_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColumnSource {
    Vertical(ColumnSolid),
    Inclined(InclinedColumnSolid),
}
impl ColumnSource {
    pub fn column(self) -> ColumnSolid {
        match self {
            Self::Vertical(c) => c,
            Self::Inclined(c) => c.column,
        }
    }
    pub fn bounds(self) -> Result<[DVec3; 2], String> {
        match self {
            Self::Vertical(c) => c.bounds(),
            Self::Inclined(c) => c.bounds(),
        }
    }
    pub fn volume(self) -> Result<f64, String> {
        match self {
            Self::Vertical(c) => Ok(c.section.area()? * c.height),
            Self::Inclined(c) => c.volume(),
        }
    }
    pub fn ray_hit(self, o: DVec3, d: DVec3) -> Option<f64> {
        match self {
            Self::Vertical(c) => c.ray_hit(o, d),
            Self::Inclined(c) => c.ray_hit(o, d),
        }
    }
    fn mesh_source(self) -> Result<ColumnMesh, String> {
        match self {
            Self::Vertical(c) => c.mesh_with_error(crate::column::ColumnSection::CHORD_ERROR),
            Self::Inclined(c) => c.mesh_source(),
        }
    }
    pub fn section_at(self, z: f64) -> Result<Vec<Point>, String> {
        match self {
            Self::Vertical(c) => {
                if !z.is_finite() || z < c.bottom || z > c.bottom + c.height {
                    return Err("Section lies outside the column height".into());
                }
                c.outline()
            }
            Self::Inclined(c) => c.section_at(z),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnSnapshot {
    source: ColumnSource,
    /// Bounds of the analytic circular/rectangular surface, not of its samples.
    pub bounds: [DVec3; 2],
    pub sampled_bounds: [DVec3; 2],
    pub mesh: ColumnMesh,
    pub normal_section_area_m2: f64,
    pub horizontal_section_area_m2: f64,
    pub volume_m3: f64,
    pub sampled_volume_m3: f64,
    pub volume_source: VolumeSource,
}
impl ColumnSnapshot {
    pub fn source(&self) -> ColumnSource {
        self.source
    }
    pub fn ray_hit(&self, o: DVec3, d: DVec3) -> Option<f64> {
        self.source.ray_hit(o, d)
    }
    pub fn section_at(&self, z: f64) -> Result<Vec<Point>, String> {
        self.source.section_at(z)
    }
    pub fn projected_plan(
        &self,
        cut: f64,
        display: crate::column_plan::ColumnPlanDisplay,
        range: crate::column_plan::ColumnProjectionRange,
    ) -> Result<crate::column_plan::ColumnPlanGeometry, String> {
        match self.source {
            ColumnSource::Inclined(c) => c.plan_geometry(cut, display, range),
            ColumnSource::Vertical(_) => {
                Err("Vertical column plan uses its symbolic footprint".into())
            }
        }
    }
    pub fn compatibility_corners(&self) -> Result<[DVec3; 8], String> {
        if let ColumnSource::Vertical(c) = self.source {
            if !c.section.is_circular() {
                let ring = c.outline()?;
                return Ok(std::array::from_fn(|i| {
                    DVec3::new(
                        ring[i % 4].x,
                        ring[i % 4].y,
                        c.bottom + if i < 4 { 0. } else { c.height },
                    )
                }));
            }
        }
        let [lo, hi] = self.bounds;
        Ok(std::array::from_fn(|i| {
            DVec3::new(
                if matches!(i % 4, 1 | 2) { hi.x } else { lo.x },
                if matches!(i % 4, 2 | 3) { hi.y } else { lo.y },
                if i < 4 { lo.z } else { hi.z },
            )
        }))
    }
    pub fn clipped(
        &self,
        p: Option<&[Point]>,
        z: Option<(f64, f64)>,
    ) -> Result<ColumnMesh, String> {
        self.clipped_with_report(p, z).map(|r| r.mesh)
    }
    pub fn clipped_with_report(
        &self,
        p: Option<&[Point]>,
        z: Option<(f64, f64)>,
    ) -> Result<crate::column_clip::ClippedMesh, String> {
        crate::geometry_validation::validate_clip_context(p, z)?;
        if p.is_some_and(|p| p.len() < 3 || p.iter().any(|p| !p.finite()))
            || z.is_some_and(|(a, b)| !a.is_finite() || !b.is_finite())
        {
            return Err("Invalid column clipping polygon or heights".into());
        }
        let result =
            crate::column_clip::clip_convex_mesh_with_report(self.mesh.clone(), p, z, &[], &[])?;
        let mesh = &result.mesh;
        if !mesh.triangles.is_empty() {
            derived_geometry::inspect_closed_surface(&mesh.triangles)?;
        }
        Ok(result)
    }
}
pub fn snapshot(source: ColumnSource) -> Result<ColumnSnapshot, String> {
    let bounds = source.bounds()?;
    let mesh = source.mesh_source()?;
    let SurfaceMetrics {
        bounds: sampled_bounds,
        signed_volume_m3: sampled_volume_m3,
        ..
    } = derived_geometry::inspect_closed_surface(&mesh.triangles)?;
    let c = source.column();
    let normal_section_area_m2 = c.section.area()?;
    let volume_m3 = source.volume()?;
    Ok(ColumnSnapshot {
        source,
        bounds,
        sampled_bounds,
        mesh,
        normal_section_area_m2,
        horizontal_section_area_m2: volume_m3 / c.height,
        volume_m3,
        sampled_volume_m3,
        volume_source: VolumeSource::AnalyticPrism,
    })
}
#[derive(Clone, Debug, PartialEq)]
pub struct DerivedColumn {
    pub identity: GeometryIdentity,
    pub geometry: ColumnSnapshot,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ClippedColumn {
    pub identity: GeometryIdentity,
    pub polygon: Option<Vec<Point>>,
    pub z_range: Option<(f64, f64)>,
    pub mesh: ColumnMesh,
    pub normalization: crate::column_clip::ClipNormalization,
}
impl DerivedColumn {
    pub fn clipped(
        &self,
        p: Option<&[Point]>,
        z: Option<(f64, f64)>,
    ) -> Result<ClippedColumn, GeometryDiagnostic> {
        let result = self.geometry.clipped_with_report(p, z).map_err(|m| {
            GeometryDiagnostic::error(self.identity.element_id, "COLUMN_CLIP_FAILED", "solid", m)
        })?;
        Ok(ClippedColumn {
            identity: self.identity.clone(),
            polygon: p.map(<[Point]>::to_vec),
            z_range: z,
            mesh: result.mesh,
            normalization: result.normalization,
        })
    }
}
pub fn derive(project: &Project, e: &Element) -> Result<DerivedColumn, GeometryDiagnostic> {
    let Shape::Column {
        center,
        section,
        height,
        bottom_offset,
        axis_rotation,
        inclination,
        ..
    } = e.shape
    else {
        return Err(GeometryDiagnostic::error(
            e.id,
            "NOT_COLUMN",
            "element",
            "Element is not a column",
        ));
    };
    let column = ColumnSolid {
        center,
        section,
        height,
        bottom: project.elevation(e.story) + bottom_offset,
        angle_degrees: axis_rotation,
    };
    let source = if let Some(i) = inclination {
        ColumnSource::Inclined(InclinedColumnSolid {
            column,
            slant_degrees: i.slant_degrees,
            direction_degrees: i.direction_degrees,
        })
    } else {
        ColumnSource::Vertical(column)
    };
    let geometry = snapshot(source)
        .map_err(|m| GeometryDiagnostic::error(e.id, "INVALID_COLUMN", "solid", m))?;
    let identity = input_identity(e, project.elevation(e.story))?;
    Ok(DerivedColumn { identity, geometry })
}

pub(crate) fn input_identity(
    e: &Element,
    base: f64,
) -> Result<GeometryIdentity, GeometryDiagnostic> {
    let crate::model::Shape::Column {
        center,
        section,
        height,
        bottom_offset,
        axis_rotation,
        inclination,
        ..
    } = e.shape
    else {
        return Err(GeometryDiagnostic::error(
            e.id,
            "NOT_COLUMN",
            "element",
            "Element is not a column",
        ));
    };
    derived_geometry::identity(
        e.id,
        &(
            center,
            section,
            height,
            bottom_offset,
            axis_rotation,
            inclination,
        ),
        base,
        COLUMN_DERIVATION_VERSION,
    )
    .map_err(|m| GeometryDiagnostic::error(e.id, "COLUMN_INPUT_IDENTITY", "parameters", m))
}
#[derive(Clone, Debug, PartialEq)]
pub enum ColumnPlanView {
    SymbolicFootprint(Vec<Point>),
    Projected {
        cut_elevation_m: f64,
        range: crate::column_plan::ColumnProjectionRange,
        display: crate::column_plan::ColumnPlanDisplay,
        geometry: crate::column_plan::ColumnPlanGeometry,
    },
    NotOnStory,
}
impl ColumnPlanView {
    pub fn visible_outline(&self) -> Vec<Point> {
        match self {
            Self::SymbolicFootprint(p) => p.clone(),
            Self::Projected { geometry, .. } => geometry.visible_outline(),
            Self::NotOnStory => Vec::new(),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct DerivedColumnPlan {
    pub identity: GeometryIdentity,
    pub source_identity: GeometryIdentity,
    pub view_story: u32,
    pub view: ColumnPlanView,
}

pub const COLUMN_PLAN_DERIVATION_VERSION: u32 = 2;
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub(crate) enum ResolvedPlanContext {
    Symbolic,
    NotOnStory,
    Projected {
        cut: f64,
        range: crate::column_plan::ColumnProjectionRange,
        display: crate::column_plan::ColumnPlanDisplay,
    },
}
pub(crate) fn plan_context(
    project: &Project,
    e: &Element,
    story: u32,
) -> Result<ResolvedPlanContext, GeometryDiagnostic> {
    let fail = |m| GeometryDiagnostic::error(e.id, "INVALID_COLUMN_PLAN_CONTEXT", "plan", m);
    let Shape::Column {
        inclination, plan, ..
    } = e.shape
    else {
        return Err(fail("Element is not a column".into()));
    };
    if !project.stories.iter().any(|s| s.id == story) {
        return Err(fail("The current floor plan story does not exist".into()));
    }
    if inclination.is_some() {
        let (cut, range) = project
            .plan_options
            .floor_plan_cut
            .resolve(project, story, plan)
            .map_err(fail)?;
        Ok(ResolvedPlanContext::Projected {
            cut,
            range,
            display: plan.display,
        })
    } else if story == e.story {
        Ok(ResolvedPlanContext::Symbolic)
    } else {
        Ok(ResolvedPlanContext::NotOnStory)
    }
}
pub(crate) fn plan_identity(
    project: &Project,
    e: &Element,
    story: u32,
    source: &GeometryIdentity,
    context: ResolvedPlanContext,
) -> Result<GeometryIdentity, GeometryDiagnostic> {
    derived_geometry::identity(
        e.id,
        &(source.input_fingerprint, e.story, story, context),
        project.elevation(e.story),
        COLUMN_PLAN_DERIVATION_VERSION,
    )
    .map_err(|m| GeometryDiagnostic::error(e.id, "INVALID_COLUMN_PLAN_CONTEXT", "plan", m))
}
pub fn derive_plan(
    project: &Project,
    e: &Element,
    story: u32,
) -> Result<DerivedColumnPlan, GeometryDiagnostic> {
    let source = derive(project, e)?;
    derive_plan_with_source(project, e, story, &source)
}
pub(crate) fn derive_plan_with_source(
    project: &Project,
    e: &Element,
    story: u32,
    source: &DerivedColumn,
) -> Result<DerivedColumnPlan, GeometryDiagnostic> {
    let fail = |m| GeometryDiagnostic::error(e.id, "INVALID_COLUMN_PLAN_CONTEXT", "plan", m);
    let context = plan_context(project, e, story)?;
    let view = match context {
        ResolvedPlanContext::Projected {
            cut,
            range,
            display,
        } => {
            let geometry = source
                .geometry
                .projected_plan(cut, display, range)
                .map_err(fail)?;
            ColumnPlanView::Projected {
                cut_elevation_m: cut,
                range,
                display,
                geometry,
            }
        }
        ResolvedPlanContext::Symbolic => ColumnPlanView::SymbolicFootprint(
            source.geometry.source.column().outline().map_err(fail)?,
        ),
        ResolvedPlanContext::NotOnStory => ColumnPlanView::NotOnStory,
    };
    Ok(DerivedColumnPlan {
        identity: plan_identity(project, e, story, &source.identity, context)?,
        source_identity: source.identity.clone(),
        view_story: story,
        view,
    })
}
