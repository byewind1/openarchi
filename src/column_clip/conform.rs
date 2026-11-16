//! Explicit normalization of clipped samples under the existing clipping policy.
//! It never edits source parameters or adds missing volume/boundary faces.
use super::{ClipNormalization, ClippedMesh, VERTEX_MERGE_M, vertex_key};
use crate::column::ColumnMesh;
use glam::DVec3;
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub(super) fn build(mesh: ColumnMesh) -> Result<ClippedMesh, String> {
    let eps = VERTEX_MERGE_M;
    let eps2 = eps * eps;
    let mut report = ClipNormalization {
        algorithm_version: super::CLIP_ALGORITHM_VERSION,
        merge_distance_m: eps,
        input_triangles: mesh.triangles.len(),
        ..Default::default()
    };
    let mut unique = BTreeMap::new();
    for &p in mesh
        .triangles
        .iter()
        .flatten()
        .chain(mesh.edges.iter().flatten())
    {
        if !p.is_finite() {
            return Err("Clipped mesh contains non-finite vertices".into());
        }
        unique.insert(vertex_key(p), p);
    }
    let mut points = Vec::<DVec3>::new();
    let mut mapping = BTreeMap::new();
    let mut grid = HashMap::<[i64; 3], Vec<usize>>::new();
    let bucket = |p: DVec3| p.to_array().map(|x| (x / eps).floor() as i64);
    for (key, p) in unique {
        let b = bucket(p);
        let mut representative = None;
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    if let Some(ids) = grid.get(&[b[0] + x, b[1] + y, b[2] + z]) {
                        for &id in ids {
                            if points[id].distance_squared(p) <= eps2
                                && representative.is_none_or(|i| id < i)
                            {
                                representative = Some(id);
                            }
                        }
                    }
                }
            }
        }
        let id = if let Some(id) = representative {
            report.welded_vertices += 1;
            report.max_displacement_m = report.max_displacement_m.max(points[id].distance(p));
            id
        } else {
            let id = points.len();
            points.push(p);
            grid.entry(b).or_default().push(id);
            id
        };
        mapping.insert(key, id);
    }
    let mut by_x: Vec<_> = (0..points.len()).collect();
    by_x.sort_by(|&a, &b| points[a].x.total_cmp(&points[b].x).then(a.cmp(&b)));
    let mut splits = BTreeMap::<(usize, usize), Vec<usize>>::new();
    let mut out = ColumnMesh::default();
    for t in mesh.triangles {
        let ids = t.map(|p| mapping[&vertex_key(p)]);
        let [a, b, c] = ids.map(|i| points[i]);
        if ids[0] == ids[1]
            || ids[0] == ids[2]
            || ids[1] == ids[2]
            || (b - a).cross(c - a).length_squared() == 0.
        {
            report.removed_degenerate_triangles += 1;
            continue;
        }
        let mut boundary = Vec::new();
        for i in 0..3 {
            let a = ids[i];
            let b = ids[(i + 1) % 3];
            let k = (a.min(b), a.max(b));
            let nodes = splits.entry(k).or_insert_with(|| {
                let a = points[k.0];
                let b = points[k.1];
                let d = b - a;
                let length = d.length_squared();
                let begin = by_x.partition_point(|&i| points[i].x < a.x.min(b.x) - eps);
                let end = by_x.partition_point(|&i| points[i].x <= a.x.max(b.x) + eps);
                let mut nodes: Vec<_> = by_x[begin..end]
                    .iter()
                    .filter_map(|&id| {
                        if id == k.0 || id == k.1 {
                            return None;
                        }
                        let u = (points[id] - a).dot(d) / length;
                        let deviation = points[id].distance_squared(a + d * u);
                        if u > 0. && u < 1. && deviation <= eps2 {
                            report.max_edge_deviation_m =
                                report.max_edge_deviation_m.max(deviation.sqrt());
                            Some((u, id))
                        } else {
                            None
                        }
                    })
                    .collect();
                nodes.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
                nodes.into_iter().map(|(_, id)| id).collect()
            });
            boundary.push(points[a]);
            if a == k.0 {
                boundary.extend(nodes.iter().map(|&i| points[i]));
            } else {
                boundary.extend(nodes.iter().rev().map(|&i| points[i]));
            }
        }
        if boundary.len() == 3 {
            out.triangles.push([a, b, c]);
            continue;
        }
        report.refined_triangles += 1;
        let center = (a + b + c) / 3.;
        for i in 0..boundary.len() {
            let t = [center, boundary[i], boundary[(i + 1) % boundary.len()]];
            if (t[1] - t[0]).cross(t[2] - t[0]).length_squared() > 0. {
                out.triangles.push(t);
            } else {
                report.removed_degenerate_triangles += 1;
            }
        }
    }
    let mut edges = BTreeSet::new();
    for [a, b] in mesh.edges {
        let a = mapping[&vertex_key(a)];
        let b = mapping[&vertex_key(b)];
        if a != b && edges.insert((a.min(b), a.max(b))) {
            out.edges.push([points[a], points[b]]);
        }
    }
    report.output_triangles = out.triangles.len();
    Ok(ClippedMesh {
        mesh: out,
        normalization: report,
    })
}
