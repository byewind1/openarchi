//! Shared affine prism mesh. XY booleans run once; caps and sides then share a
//! planar network split at the same height levels, including exact cut vertices.
use crate::model::Point;
use geo::{BooleanOps, Orient, orient::Direction};
use glam::DVec3;

pub use crate::derived_geometry::SurfaceMesh as PrismMesh;
use crate::planar_partition::{Vertex, coincident, crossing, split};
fn segments(a: Vertex, b: Vertex, levels: &[f64]) -> Vec<[Vertex; 2]> {
    let mut parts = vec![[a, b]];
    for &level in levels {
        parts = parts
            .into_iter()
            .flat_map(|[a, b]| {
                if (a.h < level && b.h > level) || (a.h > level && b.h < level) {
                    let p = crossing(a, b, level);
                    vec![[a, p], [p, b]]
                } else {
                    vec![[a, b]]
                }
            })
            .collect();
    }
    parts
}
fn heights(h: f64, thickness: f64, range: Option<(f64, f64)>) -> (f64, f64) {
    if let Some((lo, hi)) = range {
        // Use the same exact level labels as split(). Arithmetic such as
        // (hi-thickness)+thickness need not equal hi in floating point.
        let low = if h <= lo || coincident(h, lo, thickness) {
            lo
        } else {
            h
        };
        let high = if h >= hi - thickness || coincident(h, hi - thickness, thickness) {
            hi
        } else if coincident(h, lo - thickness, thickness) {
            lo
        } else {
            h + thickness
        };
        (low, high)
    } else {
        (h, h + thickness)
    }
}
fn at(v: Vertex, top: bool, thickness: f64, range: Option<(f64, f64)>) -> DVec3 {
    let (lo, hi) = heights(v.h, thickness, range);
    DVec3::new(v.p.x, v.p.y, if top { hi } else { lo })
}
fn triangle(mesh: &mut PrismMesh, t: [DVec3; 3]) {
    if (t[1] - t[0]).cross(t[2] - t[0]).length_squared() > 0. {
        mesh.triangles.push(t);
    }
}
fn polygon(points: impl IntoIterator<Item = Point>) -> geo::Polygon<f64> {
    let mut v: Vec<_> = points.into_iter().map(|p| (p.x, p.y)).collect();
    if let Some(&p) = v.first() {
        v.push(p);
    }
    geo::Polygon::new(geo::LineString::from(v), vec![])
}
/// Finite positive-thickness affine solids. Domain validation belongs to callers.
pub(crate) fn mesh(
    mut region: geo::MultiPolygon<f64>,
    _bounds: (Point, Point),
    thickness: f64,
    bottom: impl Fn(Point) -> f64,
    clip: Option<&[Point]>,
    z_range: Option<(f64, f64)>,
) -> PrismMesh {
    let mut mesh = PrismMesh::default();
    if !thickness.is_finite() || thickness <= 0. {
        return mesh;
    }
    if let Some(clip) = clip {
        if clip.len() < 3 || clip.iter().any(|p| !p.finite()) {
            return mesh;
        }
        region = region.intersection(&polygon(clip.iter().copied()));
    }
    region = region.orient(Direction::Default);
    let mut range = z_range.map(|(a, b)| (a.min(b), a.max(b)));
    if range.is_some_and(|(lo, hi)| !lo.is_finite() || !hi.is_finite() || hi <= lo) {
        return mesh;
    }
    // A boundary-only contact is not a volume. Compare actual source heights,
    // before subtracting thickness to construct any planar transition levels.
    if let Some((lo, hi)) = range {
        let (min, max) = region
            .0
            .iter()
            .flat_map(|p| p.exterior().0.iter())
            .map(|c| bottom(Point::new(c.x, c.y)))
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), h| {
                (a.min(h), b.max(h + thickness))
            });
        if max <= lo || min >= hi {
            return mesh;
        }
        if lo <= min && hi >= max {
            range = None;
        }
    }
    let mut levels = range
        .map(|(lo, hi)| vec![lo - thickness, lo, hi - thickness, hi])
        .unwrap_or_default();
    levels.sort_by(f64::total_cmp);
    levels.dedup_by(|a, b| coincident(*a, *b, thickness));
    let vertex = |c: geo::Coord<f64>| {
        let p = Point::new(c.x, c.y);
        let h = bottom(p);
        let h = levels
            .iter()
            .copied()
            .find(|&v| coincident(h, v, thickness))
            .unwrap_or(h);
        Vertex { p, h }
    };
    let occupied = |cell: &[Vertex]| {
        let h = cell.iter().map(|v| v.h).sum::<f64>() / cell.len() as f64;
        range.is_none_or(|(lo, hi)| h > lo - thickness && h < hi)
    };
    for poly in &region.0 {
        for t in crate::derived_geometry::triangulate_with_boundary(poly) {
            let mut v = t.map(vertex);
            if (v[1].p.x - v[0].p.x) * (v[2].p.y - v[0].p.y)
                - (v[1].p.y - v[0].p.y) * (v[2].p.x - v[0].p.x)
                < 0.
            {
                v.swap(1, 2);
            }
            let mut cells = vec![v.to_vec()];
            for &level in &levels {
                cells = cells.into_iter().flat_map(|p| split(p, level)).collect();
            }
            for cell in cells.into_iter().filter(|p| occupied(p)) {
                if cell.len() == 3 {
                    for top in [false, true] {
                        let mut t = std::array::from_fn(|i| at(cell[i], top, thickness, range));
                        if !top {
                            t.swap(1, 2);
                        }
                        triangle(&mut mesh, t);
                        for i in 0..3 {
                            let a = cell[i];
                            let b = cell[(i + 1) % 3];
                            if levels.iter().any(|&h| a.h == h && b.h == h) {
                                mesh.edges.push([
                                    at(a, top, thickness, range),
                                    at(b, top, thickness, range),
                                ]);
                            }
                        }
                    }
                    continue;
                }
                // A center fan retains every boundary segment, including collinear
                // nodes where adjacent cells meet. Height is affine within each cell.
                let n = cell.len() as f64;
                let center = Vertex {
                    p: Point::new(
                        cell.iter().map(|v| v.p.x).sum::<f64>() / n,
                        cell.iter().map(|v| v.p.y).sum::<f64>() / n,
                    ),
                    h: cell.iter().map(|v| v.h).sum::<f64>() / n,
                };
                for top in [false, true] {
                    for i in 0..cell.len() {
                        let a = cell[i];
                        let b = cell[(i + 1) % cell.len()];
                        let mut t = [
                            at(center, top, thickness, range),
                            at(a, top, thickness, range),
                            at(b, top, thickness, range),
                        ];
                        if !top {
                            t.swap(1, 2);
                        }
                        triangle(&mut mesh, t);
                        if levels.iter().any(|&h| a.h == h && b.h == h) {
                            mesh.edges
                                .push([at(a, top, thickness, range), at(b, top, thickness, range)]);
                        }
                    }
                }
            }
        }
        for ring in std::iter::once(poly.exterior()).chain(poly.interiors()) {
            for pair in ring.0.windows(2) {
                for [a, b] in segments(vertex(pair[0]), vertex(pair[1]), &levels) {
                    if !occupied(&[a, b]) {
                        continue;
                    }
                    let q = [
                        at(a, false, thickness, range),
                        at(b, false, thickness, range),
                        at(b, true, thickness, range),
                        at(a, true, thickness, range),
                    ];
                    triangle(&mut mesh, [q[0], q[1], q[2]]);
                    triangle(&mut mesh, [q[0], q[2], q[3]]);
                    for i in 0..4 {
                        if q[i] != q[(i + 1) % 4] {
                            mesh.edges.push([q[i], q[(i + 1) % 4]]);
                        }
                    }
                }
            }
        }
    }
    mesh
}
