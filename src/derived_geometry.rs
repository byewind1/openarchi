//! Shared identity and diagnostics for derived snapshots, never a second model.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfaceMesh {
    pub triangles: Vec<[glam::DVec3; 3]>,
    pub edges: Vec<[glam::DVec3; 2]>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GeometryDiagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub location_m: Option<[f64; 3]>,
    pub element_id: u64,
    pub feature: String,
    pub message: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryIdentity {
    pub element_id: u64,
    pub input_revision: Option<u64>,
    pub document_session: Option<u64>,
    pub policy_version: u32,
    pub algorithm_version: u32,
    pub input_fingerprint: [u8; 32],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolumeSource {
    AnalyticPrism,
    SampledSurface,
}

impl GeometryDiagnostic {
    pub fn error(
        element_id: u64,
        code: impl Into<String>,
        feature: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Error,
            location_m: None,
            element_id,
            feature: feature.into(),
            message: message.into(),
        }
    }
}
pub(crate) fn identity<T: serde::Serialize>(
    id: u64,
    input: &T,
    base: f64,
    algorithm_version: u32,
) -> Result<GeometryIdentity, String> {
    let fingerprint = input_fingerprint(input, base)?;
    Ok(GeometryIdentity {
        element_id: id,
        input_revision: None,
        document_session: None,
        policy_version: crate::precision::GEOMETRY_POLICY_VERSION,
        algorithm_version,
        input_fingerprint: fingerprint,
    })
}

pub(crate) fn input_fingerprint<T: serde::Serialize>(
    input: &T,
    base: f64,
) -> Result<[u8; 32], String> {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(serde_json::to_vec(input).map_err(|e| e.to_string())?);
    hash.update(base.to_bits().to_le_bytes());
    Ok(hash.finalize().into())
}

#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceMetrics {
    pub bounds: [glam::DVec3; 2],
    pub signed_volume_m3: f64,
    pub triangle_count: usize,
    pub component_count: usize,
}
/// Inspect the supplied surface without changing it. Exact boundary identity is
/// deliberate; a repair, welding or sampling policy must never be hidden here.
pub fn inspect_closed_surface(triangles: &[[glam::DVec3; 3]]) -> Result<SurfaceMetrics, String> {
    use glam::DVec3;
    use std::collections::BTreeMap;
    if triangles.is_empty() {
        return Err("Closed surface has no triangles".into());
    }
    fn root(parents: &mut [usize], mut i: usize) -> usize {
        while parents[i] != i {
            parents[i] = parents[parents[i]];
            i = parents[i];
        }
        i
    }
    let mut parents: Vec<_> = (0..triangles.len()).collect();
    let mut lo = DVec3::splat(f64::INFINITY);
    let mut hi = DVec3::splat(f64::NEG_INFINITY);
    let mut edges = BTreeMap::<([u64; 3], [u64; 3]), (usize, i32, usize)>::new();
    let key = |p: DVec3| {
        p.to_array().map(|x| {
            if x == 0. {
                0.0_f64.to_bits()
            } else {
                x.to_bits()
            }
        })
    };
    for (ti, &[a, b, c]) in triangles.iter().enumerate() {
        if !a.is_finite()
            || !b.is_finite()
            || !c.is_finite()
            || (b - a).cross(c - a).length_squared() == 0.
        {
            return Err("Surface has non-finite or degenerate triangles".into());
        }
        for p in [a, b, c] {
            lo = lo.min(p);
            hi = hi.max(p);
        }
        for (a, b) in [(a, b), (b, c), (c, a)] {
            let (a, b) = (key(a), key(b));
            let (key, sign) = if a < b { ((a, b), 1) } else { ((b, a), -1) };
            let edge = edges.entry(key).or_insert((0, 0, ti));
            if edge.0 > 0 {
                let a = root(&mut parents, edge.2);
                let b = root(&mut parents, ti);
                if a != b {
                    parents[b] = a;
                }
            }
            edge.0 += 1;
            edge.1 += sign;
        }
    }
    if let Some(((a, b), (n, d, _))) = edges.iter().find(|(_, v)| v.0 != 2 || v.1 != 0) {
        let point = |p: &[u64; 3]| p.map(f64::from_bits);
        return Err(format!(
            "Surface edges do not have two opposite neighbouring faces: {:?} -> {:?}, neighbours {n}, orientation balance {d}",
            point(a),
            point(b)
        ));
    }
    // The supported prism/through-hole representations have one positive
    // material shell per connected component, not nested enclosed void shells.
    // Integrate around each component's own first vertex to avoid far-island
    // cancellation, then sum. No welding, repair or face reorientation here.
    let mut components = BTreeMap::<usize, (DVec3, f64)>::new();
    for (ti, &[a, b, c]) in triangles.iter().enumerate() {
        let r = root(&mut parents, ti);
        let (origin, volume) = components.entry(r).or_insert((a, 0.));
        *volume += (a - *origin).dot((b - *origin).cross(c - *origin)) / 6.;
    }
    for (component, (origin, volume)) in &components {
        if !volume.is_finite() || *volume <= 0. {
            return Err(format!(
                "Material surface component {component} at {:?} does not enclose a positive oriented volume",
                origin.to_array()
            ));
        }
    }
    let volume = components.values().map(|(_, v)| v).sum::<f64>();
    if !volume.is_finite() {
        return Err("Surface volume overflow".into());
    }
    Ok(SurfaceMetrics {
        bounds: [lo, hi],
        signed_volume_m3: volume,
        triangle_count: triangles.len(),
        component_count: components.len(),
    })
}

/// Retain collinear boundary nodes omitted by Earcut so caps and side faces share edges.
pub(crate) fn triangulate_with_boundary(poly: &geo::Polygon<f64>) -> Vec<[geo::Coord<f64>; 3]> {
    use geo::TriangulateEarcut;
    use std::collections::BTreeSet;
    let key = |p: geo::Coord<f64>| {
        [
            if p.x == 0. {
                0.0_f64.to_bits()
            } else {
                p.x.to_bits()
            },
            if p.y == 0. {
                0.0_f64.to_bits()
            } else {
                p.y.to_bits()
            },
        ]
    };
    let mut triangles: Vec<_> = poly
        .earcut_triangles()
        .into_iter()
        .map(|t| t.to_array())
        .collect();
    let mut used: BTreeSet<_> = triangles.iter().flatten().map(|&p| key(p)).collect();
    for &p in std::iter::once(poly.exterior())
        .chain(poly.interiors())
        .flat_map(|r| r.0.iter())
    {
        if used.contains(&key(p)) {
            continue;
        }
        let mut refined = Vec::new();
        let mut inserted = false;
        for t in triangles {
            let mut split = false;
            for i in 0..3 {
                let (a, b, c) = (t[i], t[(i + 1) % 3], t[(i + 2) % 3]);
                let dx = b.x - a.x;
                let dy = b.y - a.y;
                let u = ((p.x - a.x) * dx + (p.y - a.y) * dy) / (dx * dx + dy * dy);
                if u > 0. && u < 1. && (p.x - a.x - u * dx).hypot(p.y - a.y - u * dy) < 1e-8 {
                    refined.extend([[a, p, c], [p, b, c]]);
                    inserted = true;
                    split = true;
                    break;
                }
            }
            if !split {
                refined.push(t);
            }
        }
        triangles = refined;
        if inserted {
            used.insert(key(p));
        }
    }
    triangles
}
