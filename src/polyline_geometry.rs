//! Plan-only exact curve parameters with shared, explicitly sampled display paths.
use crate::{
    derived_geometry::{self, GeometryDiagnostic, GeometryIdentity},
    geometry,
    model::{ArcSegment, Element, Point, Project, Shape},
};
use glam::DVec3;
pub const POLYLINE_DERIVATION_VERSION: u32 = 1;
#[derive(Clone, Debug, PartialEq)]
pub enum CurveSegment {
    Line {
        start: Point,
        end: Point,
        length_m: f64,
    },
    Arc {
        start: Point,
        end: Point,
        parameters: ArcSegment,
        center: Point,
        sweep_rad: f64,
        length_m: f64,
        endpoint_radius_error_m: f64,
    },
}
impl CurveSegment {
    pub fn length_m(&self) -> f64 {
        match self {
            Self::Line { length_m, .. } | Self::Arc { length_m, .. } => *length_m,
        }
    }
    pub fn endpoints(&self) -> [Point; 2] {
        match self {
            Self::Line { start, end, .. } | Self::Arc { start, end, .. } => [*start, *end],
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct PolylineSnapshot {
    points: Vec<Point>,
    arcs: Vec<Option<ArcSegment>>,
    pub segments: Vec<CurveSegment>,
    pub plan_path: Vec<Point>,
    pub closed: bool,
    /// Analytic circle extrema plus the authoritative stored endpoints, at story Z.
    pub bounds: [DVec3; 2],
    pub sampled_bounds: [DVec3; 2],
    pub length_m: f64,
    pub sampled_length_m: f64,
    pub max_chord_error_m: f64,
    /// Existing diameter tolerance is explicit; source parameters are not repaired.
    pub max_endpoint_radius_error_m: f64,
}
impl PolylineSnapshot {
    pub fn parameters(&self) -> (&[Point], &[Option<ArcSegment>]) {
        (&self.points, &self.arcs)
    }
}
fn segments(points: &[Point], arcs: &[Option<ArcSegment>]) -> Result<Vec<CurveSegment>, String> {
    if points.len() < 2
        || points.len() > 10_000
        || (!arcs.is_empty() && arcs.len() != points.len() - 1)
    {
        return Err("Polyline requires 2–10000 nodes and matching curve metadata".into());
    }
    if points.iter().any(|p| !p.finite()) {
        return Err("Polyline nodes must be finite".into());
    }
    points
        .windows(2)
        .enumerate()
        .map(|(i, p)| {
            if !crate::precision::segment_meets_minimum(
                p[0],
                p[1],
                crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
            ) {
                return Err(format!("Polyline segment {i} is shorter than 10 mm"));
            }
            let line = p[0].distance(p[1]);
            if !line.is_finite() {
                return Err(format!("Polyline segment {i} has non-finite length"));
            }
            let Some(parameters) = arcs.get(i).copied().flatten() else {
                return Ok(CurveSegment::Line {
                    start: p[0],
                    end: p[1],
                    length_m: line,
                });
            };
            let (center, sweep_rad) = geometry::arc_center_sweep(p[0], p[1], parameters)
                .ok_or_else(|| format!("Invalid polyline arc {i}"))?;
            let length_m = parameters.radius * sweep_rad.abs();
            if !center.finite() || !sweep_rad.is_finite() || !length_m.is_finite() || length_m <= 0.
            {
                return Err(format!(
                    "Polyline arc {i} has non-finite or degenerate derived geometry"
                ));
            }
            let endpoint_radius_error_m = p
                .iter()
                .map(|p| (p.distance(center) - parameters.radius).abs())
                .fold(0., f64::max);
            Ok(CurveSegment::Arc {
                start: p[0],
                end: p[1],
                parameters,
                center,
                sweep_rad,
                length_m,
                endpoint_radius_error_m,
            })
        })
        .collect()
}
pub fn length(points: &[Point], arcs: &[Option<ArcSegment>]) -> Result<f64, String> {
    let value = segments(points, arcs)?
        .iter()
        .map(CurveSegment::length_m)
        .sum::<f64>();
    if value.is_finite() {
        Ok(value)
    } else {
        Err("Polyline length overflow".into())
    }
}
pub fn snapshot(
    points: &[Point],
    arcs: &[Option<ArcSegment>],
    base: f64,
) -> Result<PolylineSnapshot, String> {
    if !base.is_finite() {
        return Err("Polyline elevation must be finite".into());
    }
    let segments = segments(points, arcs)?;
    let mut extrema = points.to_vec();
    let mut plan_path = Vec::new();
    let mut max_chord_error_m: f64 = 0.;
    let mut max_endpoint_radius_error_m: f64 = 0.;
    for s in &segments {
        let path = match *s {
            CurveSegment::Line { start, end, .. } => vec![start, end],
            CurveSegment::Arc {
                start,
                end,
                parameters,
                center,
                sweep_rad,
                endpoint_radius_error_m,
                ..
            } => {
                let a0 = (start.y - center.y).atan2(start.x - center.x);
                for angle in [
                    0.,
                    std::f64::consts::FRAC_PI_2,
                    std::f64::consts::PI,
                    3. * std::f64::consts::FRAC_PI_2,
                ] {
                    let delta = if sweep_rad > 0. {
                        (angle - a0).rem_euclid(std::f64::consts::TAU)
                    } else {
                        (a0 - angle).rem_euclid(std::f64::consts::TAU)
                    };
                    if delta <= sweep_rad.abs() {
                        extrema.push(Point::new(
                            center.x + parameters.radius * angle.cos(),
                            center.y + parameters.radius * angle.sin(),
                        ));
                    }
                }
                let path = geometry::arc_points(start, end, parameters)
                    .ok_or_else(|| "Polyline arc sampling failed".to_owned())?;
                let step = sweep_rad.abs() / (path.len() - 1) as f64;
                max_chord_error_m = max_chord_error_m
                    .max(parameters.radius * (1. - (step / 2.).cos()) + endpoint_radius_error_m);
                max_endpoint_radius_error_m =
                    max_endpoint_radius_error_m.max(endpoint_radius_error_m);
                path
            }
        };
        if plan_path.is_empty() {
            plan_path.extend(path);
        } else {
            plan_path.extend(path.into_iter().skip(1));
        }
    }
    if extrema.iter().chain(&plan_path).any(|p| !p.finite()) {
        return Err("Polyline bounds/path contains non-finite points".into());
    }
    let bounds = |p: &[Point]| {
        let (lo, hi) = crate::slab_geometry::bounds(p);
        [DVec3::new(lo.x, lo.y, base), DVec3::new(hi.x, hi.y, base)]
    };
    let length_m = segments.iter().map(CurveSegment::length_m).sum::<f64>();
    let sampled_length_m = plan_path
        .windows(2)
        .map(|p| p[0].distance(p[1]))
        .sum::<f64>();
    if !length_m.is_finite() || !sampled_length_m.is_finite() {
        return Err("Polyline accumulated length overflow".into());
    }
    Ok(PolylineSnapshot {
        bounds: bounds(&extrema),
        sampled_bounds: bounds(&plan_path),
        points: points.to_vec(),
        arcs: arcs.to_vec(),
        segments,
        plan_path,
        closed: points.first() == points.last(),
        length_m,
        sampled_length_m,
        max_chord_error_m,
        max_endpoint_radius_error_m,
    })
}
#[derive(Clone, Debug, PartialEq)]
pub struct DerivedPolyline {
    pub identity: GeometryIdentity,
    pub geometry: PolylineSnapshot,
}
pub fn derive(project: &Project, element: &Element) -> Result<DerivedPolyline, GeometryDiagnostic> {
    let fail = |m| GeometryDiagnostic::error(element.id, "INVALID_POLYLINE_GEOMETRY", "curve", m);
    let Shape::Polyline { points, arcs } = &element.shape else {
        return Err(fail("Element is not a polyline".into()));
    };
    let base = project
        .stories
        .iter()
        .find(|s| s.id == element.story)
        .ok_or_else(|| fail("Polyline story does not exist".into()))?
        .elevation;
    let geometry = snapshot(points, arcs, base).map_err(fail)?;
    let identity = derived_geometry::identity(
        element.id,
        &(points, arcs),
        base,
        POLYLINE_DERIVATION_VERSION,
    )
    .map_err(fail)?;
    Ok(DerivedPolyline { identity, geometry })
}
