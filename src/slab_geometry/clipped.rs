//! Convex partitioning of the same sampled slab; no second extrusion model.
use super::*;
use crate::column::ColumnMesh;
use glam::DVec3;
use std::collections::{BTreeMap, BTreeSet};
fn key(v: DVec3) -> [u64; 3] {
    v.to_array().map(|x| {
        if x == 0. {
            0.0_f64.to_bits()
        } else {
            x.to_bits()
        }
    })
}
fn face_key(mut face: [usize; 3]) -> [usize; 3] {
    face.sort_unstable();
    face
}
struct Cell {
    ids: [usize; 4],
    faces: Vec<[usize; 3]>,
}

fn partition(
    points: &[DVec3],
    top: Vec<[usize; 3]>,
) -> Option<(Vec<Cell>, BTreeMap<[usize; 3], usize>, f64)> {
    let mut cells = Vec::new();
    let mut volume = 0.;
    let mut face_counts = BTreeMap::<[usize; 3], usize>::new();
    for mut t in top {
        t.sort_unstable();
        let [a, b, c] = t;
        let normal = (points[b * 2] - points[a * 2])
            .cross(points[c * 2] - points[a * 2])
            .z;
        for ids in [
            [a * 2, b * 2, c * 2, c * 2 + 1],
            [a * 2, b * 2, c * 2 + 1, b * 2 + 1],
            [a * 2, a * 2 + 1, b * 2 + 1, c * 2 + 1],
        ] {
            let q = ids.map(|i| points[i]);
            let determinant = (q[1] - q[0]).cross(q[2] - q[0]).dot(q[3] - q[0]);
            if !determinant.is_finite() || determinant * normal >= 0. {
                return None;
            }
            volume += determinant.abs() / 6.;
            let mut faces = Vec::new();
            for excluded in 0..4 {
                let mut f: Vec<_> = (0..4).filter(|&i| i != excluded).map(|i| ids[i]).collect();
                let outward = (points[f[1]] - points[f[0]]).cross(points[f[2]] - points[f[0]]);
                if outward.dot(q[excluded] - points[f[0]]) > 0. {
                    f.swap(1, 2);
                }
                let face = [f[0], f[1], f[2]];
                *face_counts.entry(face_key(face)).or_default() += 1;
                faces.push(face);
            }
            cells.push(Cell { ids, faces });
        }
    }
    if face_counts.values().any(|&n| n > 2) {
        return None;
    }
    Some((cells, face_counts, volume))
}
pub(super) fn build(
    g: &DerivedSlab,
    clip: &[Point],
    range: Option<(f64, f64)>,
) -> Result<ClippedSlabMesh, GeometryDiagnostic> {
    let fail = |code: &str, message: &str| GeometryDiagnostic {
        code: code.into(),
        severity: DiagnosticSeverity::Error,
        location_m: None,
        element_id: g.identity.element_id,
        feature: "clipping".into(),
        message: message.into(),
    };
    crate::geometry_validation::validate_clip_context(Some(clip), range)
        .map_err(|message| fail("INVALID_SLAB_CLIP", &message))?;
    if clip.len() < 3
        || clip.iter().any(|p| !p.finite())
        || range.is_some_and(|(a, b)| !a.is_finite() || !b.is_finite())
    {
        return Err(fail(
            "INVALID_SLAB_CLIP",
            "Invalid slab clipping polygon or height range",
        ));
    }
    let mut result = ClippedSlabMesh {
        identity: Some(g.identity.clone()),
        clipping: Some(SlabClipContext {
            polygon: clip.to_vec(),
            z_range: range.map(|(a, b)| (a.min(b), a.max(b))),
        }),
        ..Default::default()
    };
    if range.is_some_and(|(a, b)| a == b) {
        return Ok(result);
    }
    let points: Vec<_> = g.vertex_pairs.iter().flat_map(|p| *p).collect();
    let lookup: BTreeMap<_, _> = points
        .iter()
        .enumerate()
        .map(|(i, p)| (key(*p), i))
        .collect();
    let mut chosen = None;
    for upper in [true, false] {
        let triangles: Vec<_> = g
            .caps
            .iter()
            .filter(|t| ((t[0].z - g.bounds[1].z).abs() < (t[0].z - g.bounds[0].z).abs()) == upper)
            .map(|t| t.map(|v| lookup[&key(v)] / 2))
            .collect();
        let used: BTreeSet<_> = triangles.iter().flatten().copied().collect();
        if used.len() != g.vertex_pairs.len() {
            continue;
        }
        if let Some((cells, counts, volume)) = partition(&points, triangles) {
            if crate::precision::TolerancePolicy::default()
                .volume_m3
                .accepts(volume, g.sampled_volume_m3)
            {
                chosen = Some((cells, counts));
                break;
            }
        }
    }
    let (cells, face_counts) = chosen.ok_or_else(|| {
        fail(
            "SLAB_PARTITION_INVERTED",
            "No non-inverted boundary-conforming slab partition matched the source volume",
        )
    })?;
    let mut edges: BTreeMap<(usize, usize), [DVec3; 2]> = BTreeMap::new();
    for &edge in &g.edges {
        if let (Some(&a), Some(&b)) = (lookup.get(&key(edge[0])), lookup.get(&key(edge[1]))) {
            edges.insert((a.min(b), a.max(b)), edge);
        }
    }

    for cell in cells {
        let hidden: Vec<_> = cell
            .faces
            .iter()
            .map(|f| face_counts[&face_key(*f)] == 2)
            .collect();
        let seams: Vec<_> = cell
            .faces
            .iter()
            .zip(&hidden)
            .filter(|(_, h)| **h)
            .map(|(f, _)| f.map(|i| points[i]))
            .collect();
        let mut source = ColumnMesh {
            triangles: cell.faces.iter().map(|f| f.map(|i| points[i])).collect(),
            edges: Vec::new(),
        };
        for i in 0..4 {
            for j in i + 1..4 {
                let (a, b) = (cell.ids[i], cell.ids[j]);
                if let Some(edge) = edges.remove(&(a.min(b), a.max(b))) {
                    source.edges.push(edge);
                }
            }
        }
        let mesh = crate::column_clip::clip_convex_mesh(source, Some(clip), range, &hidden, &seams)
            .map_err(|m| fail("SLAB_CLIP_FAILED", &m))?;
        for t in mesh.triangles {
            let n = (t[1] - t[0]).cross(t[2] - t[0]);
            if t.iter().all(|p| (p.z - t[0].z).abs() < 1e-8) {
                if n.z > 0. {
                    result.top.push(t);
                } else {
                    result.bottom.push(t);
                }
            } else {
                result.sides.push(t);
            }
        }
        result.edges.extend(mesh.edges);
    }
    if !edges.is_empty() {
        return Err(fail(
            "SLAB_PARTITION_EDGE_LOST",
            "Slab partition did not retain all source boundary edges",
        ));
    }
    let triangles = std::mem::take(&mut result.top)
        .into_iter()
        .chain(std::mem::take(&mut result.bottom))
        .chain(std::mem::take(&mut result.sides))
        .collect();
    let normalized = crate::column_clip::normalize_clipped_mesh(ColumnMesh {
        triangles,
        edges: std::mem::take(&mut result.edges),
    })
    .map_err(|m| fail("SLAB_CLIP_NORMALIZATION_FAILED", &m))?;
    result.metrics = if normalized.mesh.triangles.is_empty() {
        None
    } else {
        Some(
            crate::derived_geometry::inspect_closed_surface(&normalized.mesh.triangles)
                .map_err(|m| fail("INVALID_SLAB_CLIP_SURFACE", &m))?,
        )
    };
    result.normalization = Some(normalized.normalization);
    result.edges = normalized.mesh.edges;
    for t in normalized.mesh.triangles {
        let n = (t[1] - t[0]).cross(t[2] - t[0]);
        if t.iter().all(|p| (p.z - t[0].z).abs() < 1e-8) {
            if n.z > 0. {
                result.top.push(t);
            } else {
                result.bottom.push(t);
            }
        } else {
            result.sides.push(t);
        }
    }
    Ok(result)
}
