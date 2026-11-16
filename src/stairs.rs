//! Independent parametric straight-flight stair geometry.
//!
//! The input is a small OpenArchi domain object, not an AutoCAD entity set. The
//! generator returns plan/detail curves and a closed, triangulated 3D solid.

use crate::model::Point;
use glam::DVec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StairSpec {
    pub start: Point,
    pub end: Point,
    pub width: f64,
    pub total_rise: f64,
    pub risers: u16,
    pub slab_thickness: f64,
    pub handrail_height: f64,
}

impl Default for StairSpec {
    fn default() -> Self {
        Self {
            start: Point::new(0.0, 0.0),
            end: Point::new(4.8, 0.0),
            width: 1.1,
            total_rise: 3.2,
            risers: 18,
            slab_thickness: 0.10,
            handrail_height: 1.0,
        }
    }
}

impl StairSpec {
    pub fn plan_area_m2(&self) -> f64 {
        self.start.distance(self.end) * self.width
    }
    /// Exact prism section integral; call after parameter validation.
    pub fn material_volume_m3(&self) -> f64 {
        let run = self.start.distance(self.end);
        self.width
            * run
            * (self.slab_thickness * run.hypot(self.total_rise) / run
                + self.total_rise / (2. * f64::from(self.risers)))
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        let run = self.start.distance(self.end);
        if !self.start.finite()
            || !self.end.finite()
            || !run.is_finite()
            || !crate::precision::length_meets_minimum(
                run,
                0.3,
                self.start
                    .x
                    .abs()
                    .max(self.start.y.abs())
                    .max(self.end.x.abs())
                    .max(self.end.y.abs()),
            )
        {
            return Err("stair run must be at least 0.30 m and have finite endpoints");
        }
        if !self.width.is_finite() || !(0.5..=5.0).contains(&self.width) {
            return Err("stair width must be between 0.50 m and 5.00 m");
        }
        if !self.total_rise.is_finite() || !(0.1..=10.0).contains(&self.total_rise) {
            return Err("stair rise must be between 0.10 m and 10.00 m");
        }
        if !(2..=100).contains(&self.risers) {
            return Err("stair must have between 2 and 100 risers");
        }
        if !self.slab_thickness.is_finite() || !(0.05..=0.5).contains(&self.slab_thickness) {
            return Err("stair slab thickness must be between 0.05 m and 0.50 m");
        }
        if !self.handrail_height.is_finite() || !(0.5..=1.5).contains(&self.handrail_height) {
            return Err("handrail height must be between 0.50 m and 1.50 m");
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StairGeometry {
    /// Four corners of the plan footprint, ordered around the boundary.
    pub footprint: [Point; 4],
    /// One line across the stair width at each riser.
    pub tread_lines: Vec<[Point; 2]>,
    /// Closed prism surface triangles in project coordinates.
    pub triangles: Vec<[DVec3; 3]>,
    /// Architectural section edges without triangulation diagonals.
    pub edges: Vec<[DVec3; 2]>,
    /// Two sloped handrail centerlines, one on each side of the flight.
    pub handrails: [[DVec3; 2]; 2],
    /// Stair side section profile in (run distance, elevation above base).
    pub section: Vec<[f64; 2]>,
    /// One cap connectivity shared by body and clipping cells.
    pub section_triangles: Vec<[usize; 3]>,
}

pub(crate) fn generate_source(
    spec: &StairSpec,
    base_elevation: f64,
) -> Result<StairGeometry, &'static str> {
    spec.validate()?;
    if !base_elevation.is_finite() {
        return Err("base elevation must be finite");
    }

    let dx = spec.end.x - spec.start.x;
    let dy = spec.end.y - spec.start.y;
    let run = dx.hypot(dy);
    let ux = dx / run;
    let uy = dy / run;
    let nx = -uy;
    let ny = ux;
    let world = |along: f64, across: f64| {
        Point::new(
            spec.start.x + ux * along + nx * across,
            spec.start.y + uy * along + ny * across,
        )
    };
    let step_run = run / f64::from(spec.risers);
    let step_rise = spec.total_rise / f64::from(spec.risers);
    let soffit_drop = spec.slab_thickness * run.hypot(spec.total_rise) / run;

    // Walk the stepped upper profile and close it with a sloped soffit. This
    // yields a solid stair body with flat treads, vertical risers and a soffit
    // parallel to the flight slope at the requested normal thickness.
    let mut section = vec![[0.0, -soffit_drop], [0.0, 0.0], [0.0, step_rise]];
    for i in 1..=spec.risers {
        let x = f64::from(i) * step_run;
        let z = f64::from(i) * step_rise;
        section.push([x, z]);
        if i < spec.risers {
            section.push([x, f64::from(i + 1) * step_rise]);
        }
    }
    section.push([run, spec.total_rise - soffit_drop]);

    let footprint = [
        world(0.0, 0.0),
        world(run, 0.0),
        world(run, spec.width),
        world(0.0, spec.width),
    ];
    let tread_lines = (1..=spec.risers)
        .map(|i| {
            let p = world(f64::from(i) * step_run, 0.0);
            let q = world(f64::from(i) * step_run, spec.width);
            [p, q]
        })
        .collect();

    let at = |run_pos: f64, across: f64, z: f64| {
        let p = world(run_pos, across);
        DVec3::new(p.x, p.y, base_elevation + z)
    };
    let mut triangles = Vec::new();
    let mut edges = Vec::new();
    // Side faces along the complete section perimeter.
    for i in 0..section.len() {
        let a = section[i];
        let b = section[(i + 1) % section.len()];
        let a0 = at(a[0], 0.0, a[1]);
        let b0 = at(b[0], 0.0, b[1]);
        let b1 = at(b[0], spec.width, b[1]);
        let a1 = at(a[0], spec.width, a[1]);
        triangles.extend([[a0, b0, b1], [a0, b1, a1]]);
        edges.extend([[a0, b0], [a1, b1]]);
    }

    // Ear clipping triangulates the concave side profile for both end caps.
    let cap_tris = triangulate(&section).ok_or("could not triangulate stair profile")?;
    for &[a, b, c] in &cap_tris {
        let p = section[a];
        let q = section[b];
        let r = section[c];
        triangles.push([
            at(r[0], 0.0, r[1]),
            at(q[0], 0.0, q[1]),
            at(p[0], 0.0, p[1]),
        ]);
        triangles.push([
            at(p[0], spec.width, p[1]),
            at(q[0], spec.width, q[1]),
            at(r[0], spec.width, r[1]),
        ]);
    }
    for point in &section {
        edges.push([
            at(point[0], 0.0, point[1]),
            at(point[0], spec.width, point[1]),
        ]);
    }

    let rail_start_z = step_rise + spec.handrail_height;
    let rail_end_z = spec.total_rise + spec.handrail_height;
    let inset = (spec.width * 0.08).min(0.08);
    let handrails = [
        [at(0.0, inset, rail_start_z), at(run, inset, rail_end_z)],
        [
            at(0.0, spec.width - inset, rail_start_z),
            at(run, spec.width - inset, rail_end_z),
        ],
    ];

    Ok(StairGeometry {
        footprint,
        tread_lines,
        triangles,
        edges,
        handrails,
        section,
        section_triangles: cap_tris,
    })
}

pub fn generate(spec: &StairSpec, base: f64) -> Result<StairGeometry, &'static str> {
    spec.validate()?;
    if !base.is_finite() {
        return Err("base elevation must be finite");
    }
    crate::stair_geometry::snapshot(spec, base)
        .map(|g| g.source)
        .map_err(|_| "Invalid stair surface")
}
/// Compatibility display adapter over the shared stair snapshot.
pub fn clipped_geometry(
    spec: &StairSpec,
    base: f64,
    clip: Option<&[Point]>,
    range: Option<(f64, f64)>,
) -> Result<(crate::column::ColumnMesh, Vec<[DVec3; 2]>), String> {
    crate::stair_geometry::snapshot(spec, base)?
        .clipped(clip, range)
        .map(|g| (g.mesh, g.handrails))
}

/// Display geometry from the same stair section, optionally intersected with a marquee.
pub(crate) fn clip_source(
    spec: &StairSpec,
    base: f64,
    geometry: &StairGeometry,
    clip: Option<&[Point]>,
    z_range: Option<(f64, f64)>,
) -> Result<(crate::column::ColumnMesh, Vec<[DVec3; 2]>), String> {
    use crate::column::ColumnMesh;
    use geo::BooleanOps;
    if clip.is_none() && z_range.is_none() {
        return Ok((
            ColumnMesh {
                triangles: geometry.triangles.clone(),
                edges: geometry.edges.clone(),
            },
            geometry.handrails.to_vec(),
        ));
    }
    let run = spec.start.distance(spec.end);
    let direction = DVec3::new(
        (spec.end.x - spec.start.x) / run,
        (spec.end.y - spec.start.y) / run,
        0.,
    );
    let across = DVec3::new(-direction.y, direction.x, 0.);
    let origin = DVec3::new(spec.start.x, spec.start.y, base);
    let at = |i: usize, width: f64| {
        origin
            + direction * geometry.section[i][0]
            + across * width
            + DVec3::Z * geometry.section[i][1]
    };
    let mut mesh = ColumnMesh::default();
    for &indices in &geometry.section_triangles {
        let [a, b, c] = indices;
        let center = (at(a, 0.)
            + at(b, 0.)
            + at(c, 0.)
            + at(a, spec.width)
            + at(b, spec.width)
            + at(c, spec.width))
            / 6.;
        let mut part = ColumnMesh::default();
        let mut hidden = Vec::new();
        let mut seams = Vec::new();
        let mut add = |mut t: [DVec3; 3], internal: bool| {
            if (t[1] - t[0])
                .cross(t[2] - t[0])
                .dot((t[0] + t[1] + t[2]) / 3. - center)
                < 0.
            {
                t.swap(1, 2);
            }
            part.triangles.push(t);
            hidden.push(internal);
        };
        add([at(a, 0.), at(b, 0.), at(c, 0.)], false);
        add(
            [at(a, spec.width), at(b, spec.width), at(c, spec.width)],
            false,
        );
        for j in 0..3 {
            let a = indices[j];
            let b = indices[(j + 1) % 3];
            let internal =
                (a + 1) % geometry.section.len() != b && (b + 1) % geometry.section.len() != a;
            let t = [at(a, 0.), at(b, 0.), at(b, spec.width)];
            add(t, internal);
            add([at(a, 0.), at(b, spec.width), at(a, spec.width)], internal);
            if internal {
                seams.push(t);
            } else {
                part.edges.extend([
                    [at(a, 0.), at(b, 0.)],
                    [at(a, spec.width), at(b, spec.width)],
                ]);
            }
        }
        // Only real section vertices produce cross-width edges, never diagonals.
        for i in indices {
            part.edges.push([at(i, 0.), at(i, spec.width)]);
        }
        let clipped = crate::column_clip::clip_convex_mesh(part, clip, z_range, &hidden, &seams)?;
        mesh.triangles.extend(clipped.triangles);
        mesh.edges.extend(clipped.edges);
    }
    let polygon = clip.map(|points| {
        geo::Polygon::new(
            geo::LineString::from(
                points
                    .iter()
                    .chain(points.first())
                    .map(|p| (f64::from(p.x), f64::from(p.y)))
                    .collect::<Vec<_>>(),
            ),
            vec![],
        )
    });
    let mut rails = Vec::new();
    for [a, b] in geometry.handrails {
        let input = geo::MultiLineString(vec![geo::LineString::from(vec![
            (f64::from(a.x), f64::from(a.y)),
            (f64::from(b.x), f64::from(b.y)),
        ])]);
        let segments = polygon
            .as_ref()
            .map_or(input.clone(), |p| p.clip(&input, false));
        let d = b - a;
        for segment in segments {
            for pair in segment.0.windows(2) {
                let parameter = |p: geo::Coord<f64>| {
                    (((p.x - f64::from(a.x)) * f64::from(d.x)
                        + (p.y - f64::from(a.y)) * f64::from(d.y))
                        / (f64::from(d.x).powi(2) + f64::from(d.y).powi(2)))
                        as f64
                };
                let t0 = parameter(pair[0]);
                let t1 = parameter(pair[1]);
                let mut lo = t0.min(t1).max(0.);
                let mut hi = t0.max(t1).min(1.);
                if let Some((z0, z1)) = z_range {
                    lo = lo.max((z0.min(z1) - a.z) / d.z);
                    hi = hi.min((z0.max(z1) - a.z) / d.z);
                }
                if hi > lo {
                    rails.push([a + d * lo, a + d * hi]);
                }
            }
        }
    }
    Ok((mesh, rails))
}

fn triangulate(poly: &[[f64; 2]]) -> Option<Vec<[usize; 3]>> {
    if poly.len() < 3 {
        return None;
    }
    let area: f64 = (0..poly.len())
        .map(|i| {
            let a = poly[i];
            let b = poly[(i + 1) % poly.len()];
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    let orientation = area.signum();
    let mut remaining: Vec<usize> = (0..poly.len()).collect();
    let mut out = Vec::with_capacity(poly.len() - 2);
    let mut guard = poly.len() * poly.len();
    while remaining.len() > 3 && guard > 0 {
        guard -= 1;
        let mut found = false;
        for i in 0..remaining.len() {
            let prev = remaining[(i + remaining.len() - 1) % remaining.len()];
            let curr = remaining[i];
            let next = remaining[(i + 1) % remaining.len()];
            if cross(poly[prev], poly[curr], poly[next]) * orientation
                <= crate::precision::GeometryContext::TOPOLOGY.triangle_double_area_m2
            {
                continue;
            }
            if remaining.iter().copied().any(|p| {
                p != prev
                    && p != curr
                    && p != next
                    && point_in_triangle(poly[p], poly[prev], poly[curr], poly[next])
            }) {
                continue;
            }
            out.push([prev, curr, next]);
            remaining.remove(i);
            found = true;
            break;
        }
        if !found {
            return None;
        }
    }
    if remaining.len() == 3 {
        out.push([remaining[0], remaining[1], remaining[2]]);
    }
    Some(out)
}

fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn point_in_triangle(p: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> bool {
    let ab = cross(a, b, p);
    let bc = cross(b, c, p);
    let ca = cross(c, a, p);
    let epsilon = crate::precision::GeometryContext::TOPOLOGY.triangle_double_area_m2;
    (ab >= -epsilon && bc >= -epsilon && ca >= -epsilon)
        || (ab <= epsilon && bc <= epsilon && ca <= epsilon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_closed_stair_geometry_and_plan_lines() {
        let spec = StairSpec::default();
        let geometry = generate(&spec, 0.0).unwrap();
        assert_eq!(geometry.tread_lines.len(), 18);
        assert_eq!(geometry.handrails.len(), 2);
        assert_eq!(geometry.footprint.len(), 4);
        assert!(geometry.triangles.len() > 100);
        assert!(geometry.triangles.iter().flatten().all(|p| p.is_finite()));
        let soffit_drop = spec.slab_thickness
            * spec.end.distance(spec.start).hypot(spec.total_rise)
            / spec.end.distance(spec.start);
        assert!((geometry.section.last().unwrap()[1] - (3.2 - soffit_drop)).abs() < 1e-5);
    }

    #[test]
    fn stair_follows_rotated_run_and_checks_bad_inputs() {
        let mut spec = StairSpec {
            end: Point::new(0.0, 4.8),
            ..Default::default()
        };
        let geometry = generate(&spec, 2.5).unwrap();
        assert!((geometry.footprint[1].x).abs() < 1e-5);
        assert!((geometry.footprint[1].y - 4.8).abs() < 1e-5);
        let soffit_drop = spec.slab_thickness
            * spec.end.distance(spec.start).hypot(spec.total_rise)
            / spec.end.distance(spec.start);
        assert!(
            geometry
                .triangles
                .iter()
                .flatten()
                .all(|p| p.z >= 2.5 - soffit_drop - 1e-5)
        );
        spec.risers = 1;
        assert!(generate(&spec, 0.0).is_err());
    }
}
