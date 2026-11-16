//! Intersect a convex column solid with an XY polygon prism and horizontal range.
//! Concave clip regions are partitioned; partition faces never become surfaces.
use crate::{column::ColumnMesh, column_inclination::InclinedColumnSolid, model::Point};
use glam::DVec3;
mod conform;
pub const CLIP_ALGORITHM_VERSION: u32 = 3;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipNormalization {
    pub algorithm_version: u32,
    pub merge_distance_m: f64,
    pub input_triangles: usize,
    pub output_triangles: usize,
    pub welded_vertices: usize,
    pub max_displacement_m: f64,
    pub max_edge_deviation_m: f64,
    pub refined_triangles: usize,
    pub removed_degenerate_triangles: usize,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ClippedMesh {
    pub mesh: ColumnMesh,
    pub normalization: ClipNormalization,
}

pub(crate) fn normalize_clipped_mesh(mesh: ColumnMesh) -> Result<ClippedMesh, String> {
    conform::build(mesh)
}
const PLANE_DISTANCE_M: f64 = crate::precision::GeometryContext::TOPOLOGY.clip_plane_distance_m;
const VERTEX_MERGE_M: f64 = crate::precision::GeometryContext::TOPOLOGY.clip_vertex_merge_m;
#[derive(Clone, Copy)]
struct Plane {
    normal: DVec3,
    offset: f64,
    boundary: bool,
}
impl Plane {
    fn distance(self, p: DVec3) -> f64 {
        self.normal.dot(p) - self.offset
    }
}
#[derive(Clone)]
struct Face {
    points: Vec<DVec3>,
    cap: Option<usize>,
    hidden: bool,
}
fn dedup(points: &mut Vec<DVec3>) {
    let mut unique = Vec::new();
    for &p in points.iter() {
        if !unique
            .iter()
            .any(|q: &DVec3| q.distance_squared(p) < VERTEX_MERGE_M * VERTEX_MERGE_M)
        {
            unique.push(p);
        }
    }
    *points = unique;
}
fn vertex_key(p: DVec3) -> [u64; 3] {
    p.to_array().map(|x| {
        if x == 0. {
            0.0_f64.to_bits()
        } else {
            x.to_bits()
        }
    })
}
fn on_plane(p: DVec3, plane: Plane) -> DVec3 {
    let d = plane.distance(p);
    if d.abs() <= PLANE_DISTANCE_M {
        p - plane.normal * d
    } else {
        p
    }
}
fn crossing(mut a: DVec3, mut b: DVec3, plane: Plane) -> DVec3 {
    if vertex_key(a) > vertex_key(b) {
        std::mem::swap(&mut a, &mut b);
    }
    let da = plane.distance(a);
    let db = plane.distance(b);
    if da.abs() <= PLANE_DISTANCE_M {
        return a;
    }
    if db.abs() <= PLANE_DISTANCE_M {
        return b;
    }
    a + (b - a) * (da / (da - db))
}
fn clip_face(points: &[DVec3], plane: Plane, crossings: &mut Vec<DVec3>) -> Vec<DVec3> {
    let mut out = Vec::new();
    for i in 0..points.len() {
        let a = on_plane(points[i], plane);
        let b = on_plane(points[(i + 1) % points.len()], plane);
        let da = plane.distance(a);
        let db = plane.distance(b);
        if da >= -PLANE_DISTANCE_M {
            out.push(a);
        }
        if (da >= -PLANE_DISTANCE_M) != (db >= -PLANE_DISTANCE_M) {
            let p = crossing(a, b, plane);
            out.push(p);
            crossings.push(p);
        }
    }
    dedup(&mut out);
    out
}
fn clipped_segment(mut a: DVec3, mut b: DVec3, planes: &[Plane]) -> Option<[glam::DVec3; 2]> {
    for &plane in planes {
        let da = plane.distance(a);
        let db = plane.distance(b);
        if da < -PLANE_DISTANCE_M && db < -PLANE_DISTANCE_M {
            return None;
        }
        if (da >= -PLANE_DISTANCE_M) != (db >= -PLANE_DISTANCE_M) {
            let p = crossing(a, b, plane);
            if da < -PLANE_DISTANCE_M {
                a = p;
            } else {
                b = p;
            }
        }
    }
    (a.distance_squared(b) > VERTEX_MERGE_M * VERTEX_MERGE_M).then_some([a, b])
}
fn piece(
    source: &ColumnMesh,
    planes: &[Plane],
    hidden: &[bool],
    seams: &[Plane],
    out: &mut ColumnMesh,
) {
    let mut faces: Vec<_> = source
        .triangles
        .iter()
        .enumerate()
        .map(|(i, t)| Face {
            points: t.iter().copied().collect(),
            cap: None,
            hidden: hidden.get(i).copied().unwrap_or(false),
        })
        .collect();
    for (index, &plane) in planes.iter().enumerate() {
        let mut crossings = Vec::new();
        faces = faces
            .into_iter()
            .filter_map(|face| {
                let points = clip_face(&face.points, plane, &mut crossings);
                (points.len() >= 3).then_some(Face {
                    points,
                    cap: face.cap,
                    hidden: face.hidden,
                })
            })
            .collect();
        dedup(&mut crossings);
        if crossings.len() >= 3 {
            let center = crossings.iter().copied().sum::<DVec3>() / crossings.len() as f64;
            let outward = -plane.normal;
            let axis = if outward.x.abs() < 0.9 {
                DVec3::X
            } else {
                DVec3::Y
            };
            let u = outward.cross(axis).normalize();
            let v = outward.cross(u);
            crossings.sort_by(|a, b| {
                let a = *a - center;
                let b = *b - center;
                a.dot(v)
                    .atan2(a.dot(u))
                    .total_cmp(&b.dot(v).atan2(b.dot(u)))
            });
            faces.push(Face {
                points: crossings,
                cap: Some(index),
                hidden: false,
            });
        }
        if faces.is_empty() {
            return;
        }
    }
    // A boundary-only intersection is not a solid. Keep hidden partition faces
    // in this dimensionality check, since they still bound the convex volume.
    if planes.iter().any(|plane| {
        faces
            .iter()
            .flat_map(|face| &face.points)
            .all(|p| plane.distance(*p) <= PLANE_DISTANCE_M)
    }) {
        return;
    }
    for mut face in faces {
        // A source partition interface can become an exterior face when the
        // clipping boundary coincides with it. There is no crossing in this
        // case, so cap construction above intentionally produced no replacement.
        if face.hidden {
            let exposed = planes.iter().enumerate().find(|(_, plane)| {
                plane.boundary
                    && face
                        .points
                        .iter()
                        .all(|p| plane.distance(*p).abs() <= PLANE_DISTANCE_M)
            });
            if let Some((index, _)) = exposed {
                face.hidden = false;
                face.cap = Some(index);
            }
        }
        if face.hidden || face.cap.is_some_and(|i| !planes[i].boundary) {
            continue;
        }
        // Any original face coincident with an internal partition is also internal.
        if planes.iter().any(|p| {
            !p.boundary
                && face
                    .points
                    .iter()
                    .all(|v| p.distance(*v).abs() < PLANE_DISTANCE_M)
        }) {
            continue;
        }
        if face.points.len() == 3 {
            let triangle = [face.points[0], face.points[1], face.points[2]];
            if (triangle[1] - triangle[0])
                .cross(triangle[2] - triangle[0])
                .length_squared()
                > 1e-18
            {
                out.triangles.push(triangle);
            }
        } else {
            let center = face.points.iter().copied().sum::<DVec3>() / face.points.len() as f64;
            for i in 0..face.points.len() {
                let triangle = [
                    center,
                    face.points[i],
                    face.points[(i + 1) % face.points.len()],
                ];
                if (triangle[1] - triangle[0])
                    .cross(triangle[2] - triangle[0])
                    .length_squared()
                    > 1e-18
                {
                    out.triangles.push(triangle);
                }
            }
        }
        if face.cap.is_some() {
            for i in 0..face.points.len() {
                let a = face.points[i];
                let b = face.points[(i + 1) % face.points.len()];
                if planes.iter().any(|p| {
                    !p.boundary
                        && p.distance(a).abs() < PLANE_DISTANCE_M
                        && p.distance(b).abs() < PLANE_DISTANCE_M
                }) {
                    continue;
                }
                if seams
                    .iter()
                    .any(|p| p.distance(a).abs() < 1e-5 && p.distance(b).abs() < 1e-5)
                {
                    continue;
                }
                if a.distance_squared(b) > VERTEX_MERGE_M * VERTEX_MERGE_M {
                    out.edges.push([a, b]);
                }
            }
        }
    }
    for [a, b] in &source.edges {
        if let Some(edge) = clipped_segment(*a, *b, planes) {
            out.edges.push(edge);
        }
    }
}
impl InclinedColumnSolid {
    pub fn clipped_mesh(
        self,
        clip: Option<&[Point]>,
        z_range: Option<(f64, f64)>,
    ) -> Result<ColumnMesh, String> {
        crate::column_geometry::snapshot(crate::column_geometry::ColumnSource::Inclined(self))?
            .clipped(clip, z_range)
    }
}

/// Clip one convex partition; hidden faces still participate in cap construction.
pub(crate) fn clip_convex_mesh(
    source: ColumnMesh,
    clip: Option<&[Point]>,
    z_range: Option<(f64, f64)>,
    hidden: &[bool],
    internal_faces: &[[DVec3; 3]],
) -> Result<ColumnMesh, String> {
    clip_convex_mesh_with_report(source, clip, z_range, hidden, internal_faces).map(|r| r.mesh)
}
pub(crate) fn clip_convex_mesh_with_report(
    source: ColumnMesh,
    clip: Option<&[Point]>,
    z_range: Option<(f64, f64)>,
    hidden: &[bool],
    internal_faces: &[[DVec3; 3]],
) -> Result<ClippedMesh, String> {
    let mesh = clip_convex_mesh_raw(source, clip, z_range, hidden, internal_faces)?;
    if clip.is_none() && z_range.is_none() {
        let count = mesh.triangles.len();
        return Ok(ClippedMesh {
            mesh,
            normalization: ClipNormalization {
                algorithm_version: CLIP_ALGORITHM_VERSION,
                input_triangles: count,
                output_triangles: count,
                ..Default::default()
            },
        });
    }
    normalize_clipped_mesh(mesh)
}
fn clip_convex_mesh_raw(
    source: ColumnMesh,
    clip: Option<&[Point]>,
    z_range: Option<(f64, f64)>,
    hidden: &[bool],
    internal_faces: &[[glam::DVec3; 3]],
) -> Result<ColumnMesh, String> {
    let seams: Vec<_> = internal_faces
        .iter()
        .filter_map(|[a, b, c]| {
            let normal = (b - a).cross(c - a).try_normalize()?;
            Some(Plane {
                normal,
                offset: normal.dot(*a),
                boundary: false,
            })
        })
        .collect();
    if clip.is_none() && z_range.is_none() {
        return Ok(source);
    }
    let mut horizontal = Vec::new();
    if let Some((a, b)) = z_range {
        if !a.is_finite() || !b.is_finite() {
            return Err("Invalid column clipping range".into());
        }
        if a == b {
            return Ok(ColumnMesh::default());
        }
        horizontal.extend([
            Plane {
                normal: DVec3::Z,
                offset: a.min(b).into(),
                boundary: true,
            },
            Plane {
                normal: -DVec3::Z,
                offset: -f64::from(a.max(b)),
                boundary: true,
            },
        ]);
    }
    let mut out = ColumnMesh::default();
    if let Some(clip) = clip {
        if clip.len() < 3 || clip.iter().any(|p| !p.finite()) {
            return Err("Invalid column clipping polygon".into());
        }
        let area = crate::geometry::polygon_area(clip);
        let mut ring = clip.to_vec();
        if area < 0. {
            ring.reverse();
        }
        let convex = area != 0.
            && (0..ring.len()).all(|i| {
                let a = ring[i];
                let b = ring[(i + 1) % ring.len()];
                let c = ring[(i + 2) % ring.len()];
                (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x) >= 0.
            });
        if convex {
            let mut planes = horizontal.clone();
            for i in 0..ring.len() {
                let a = ring[i];
                let b = ring[(i + 1) % ring.len()];
                let normal = DVec3::new(a.y - b.y, b.x - a.x, 0.)
                    .try_normalize()
                    .ok_or("Invalid column clipping polygon")?;
                planes.push(Plane {
                    normal,
                    offset: normal.dot(DVec3::new(a.x, a.y, 0.)),
                    boundary: true,
                });
            }
            piece(&source, &planes, hidden, &seams, &mut out);
            return Ok(out);
        }
        let triangles = crate::geometry::triangulate_polygon(clip);
        if triangles.is_empty() {
            return Err("Invalid column clipping polygon".into());
        }
        for mut tri in triangles {
            let area = (tri[1].x - tri[0].x) * (tri[2].y - tri[0].y)
                - (tri[1].y - tri[0].y) * (tri[2].x - tri[0].x);
            if area < 0. {
                tri.swap(1, 2);
            }
            let mut planes = horizontal.clone();
            for i in 0..3 {
                let a = tri[i];
                let b = tri[(i + 1) % 3];
                let normal =
                    DVec3::new(-f64::from(b.y - a.y), f64::from(b.x - a.x), 0.).normalize();
                let boundary = (0..clip.len()).any(|j| {
                    crate::geometry::point_on_segment(a, clip[j], clip[(j + 1) % clip.len()], 1e-5)
                        && crate::geometry::point_on_segment(
                            b,
                            clip[j],
                            clip[(j + 1) % clip.len()],
                            1e-5,
                        )
                });
                planes.push(Plane {
                    normal,
                    offset: normal.dot(DVec3::new(a.x.into(), a.y.into(), 0.)),
                    boundary,
                });
            }
            piece(&source, &planes, hidden, &seams, &mut out);
        }
    } else {
        piece(&source, &horizontal, hidden, &seams, &mut out);
    }
    Ok(out)
}
