//! Source-backed open 2.5D surface. Unknown coverage never becomes a ground plane.
use super::{HorizontalReference, LengthUnit, SiteContext, VerticalReference};
use crate::{derived_geometry::SurfaceMesh, model::Point};
use geo::{Intersects, Relate};
use glam::DVec3;
use rstar::{AABB, RTree, RTreeObject};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "TerrainGeometry")]
pub struct Geometry {
    pub vertices: Vec<[f64; 3]>,
    pub faces: Vec<[u32; 3]>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(rename = "TerrainPlacement")]
pub enum Placement {
    ModelXyz,
    SiteReference {
        horizontal: HorizontalReference,
        vertical: VerticalReference,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "TerrainOrigin")]
pub struct Origin {
    pub source_sha256: String,
    pub unit: LengthUnit,
    pub placement: Placement,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "TerrainSurface")]
pub struct Terrain {
    pub name: String,
    pub origin: Origin,
    pub geometry: Geometry,
    pub imported: Geometry,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "TerrainImportProposal")]
pub struct Proposal {
    pub terrain: Terrain,
    pub expected_previous: Option<Terrain>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(rename = "TerrainEdit")]
pub enum Edit {
    Import {
        proposal: Proposal,
    },
    UpdateVertex {
        index: u32,
        expected: [f64; 3],
        position: [f64; 3],
    },
    Remove {
        expected: Terrain,
    },
}
#[derive(Clone, Serialize, Debug)]
pub struct Metrics {
    pub vertices: usize,
    pub triangles: usize,
    pub projected_area_m2: f64,
    pub surface_area_m2: f64,
    pub bounds_m: [[f64; 3]; 2],
    pub boundary_edges: usize,
}
#[derive(Clone)]
struct Indexed {
    index: usize,
    envelope: AABB<[f64; 2]>,
}
impl RTreeObject for Indexed {
    type Envelope = AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}
fn bits(v: f64) -> u64 {
    if v == 0. { 0 } else { v.to_bits() }
}
fn polygon(v: [[f64; 3]; 3]) -> geo::Polygon<f64> {
    geo::Polygon::new(
        geo::LineString::from(vec![
            (v[0][0], v[0][1]),
            (v[1][0], v[1][1]),
            (v[2][0], v[2][1]),
            (v[0][0], v[0][1]),
        ]),
        vec![],
    )
}
fn twice_area(p: [[f64; 3]; 3]) -> f64 {
    (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0])
}
impl Geometry {
    pub fn triangle(&self, f: [u32; 3]) -> Result<[[f64; 3]; 3], String> {
        let mut v = [[0.; 3]; 3];
        for (out, id) in v.iter_mut().zip(f) {
            *out = *self
                .vertices
                .get(id as usize)
                .ok_or("Terrain face index is out of range")?;
        }
        Ok(v)
    }
    pub fn validate(&self) -> Result<(), String> {
        if !(3..=10000).contains(&self.vertices.len())
            || self.faces.is_empty()
            || self.faces.len() > 20000
        {
            return Err("Terrain needs 3–10000 vertices and 1–20000 triangles".into());
        }
        let mut xy = BTreeSet::new();
        for p in &self.vertices {
            if p.iter().any(|v| !v.is_finite() || v.abs() > 100000.)
                || !xy.insert((bits(p[0]), bits(p[1])))
            {
                return Err(
                    "Terrain vertices must be finite, within model bounds and unique in XY".into(),
                );
            }
        }
        let mut used = BTreeSet::new();
        let mut faces = BTreeSet::new();
        let mut edges = BTreeMap::new();
        let mut index = vec![];
        let mut polygons = vec![];
        for (i, &f) in self.faces.iter().enumerate() {
            let v = self.triangle(f)?;
            if twice_area(v) <= 2e-8 {
                return Err("Terrain needs nondegenerate CCW triangles in XY".into());
            }
            let mut key = f;
            key.sort();
            if !faces.insert(key) {
                return Err("Duplicate terrain face".into());
            }
            for id in f {
                used.insert(id);
            }
            for [a, b] in [[f[0], f[1]], [f[1], f[2]], [f[2], f[0]]] {
                let n = edges.entry((a.min(b), a.max(b))).or_insert(0);
                *n += 1;
                if *n > 2 {
                    return Err("Terrain edge has more than two incident faces".into());
                }
            }
            let min = [
                v.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min),
                v.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min),
            ];
            let max = [
                v.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max),
                v.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max),
            ];
            index.push(Indexed {
                index: i,
                envelope: AABB::from_corners(min, max),
            });
            polygons.push(polygon(v));
        }
        if used.len() != self.vertices.len() {
            return Err(
                "Terrain has unused vertices; no source points are silently dropped".into(),
            );
        }
        let tree = RTree::bulk_load(index.clone());
        let mut candidate_pairs = 0usize;
        for item in &index {
            for other in tree.locate_in_envelope_intersecting(&item.envelope) {
                if other.index <= item.index {
                    continue;
                }
                candidate_pairs += 1;
                if candidate_pairs > 250000 {
                    return Err("Terrain topology exceeds supported candidate complexity".into());
                }
                if polygons[item.index]
                    .relate(&polygons[other.index])
                    .matches("T********")
                    .map_err(|e| e.to_string())?
                {
                    return Err("Terrain triangles overlap in XY".into());
                }
                for (a, b) in [
                    (self.faces[item.index], self.faces[other.index]),
                    (self.faces[other.index], self.faces[item.index]),
                ] {
                    for v in a {
                        for [u, w] in [[b[0], b[1]], [b[1], b[2]], [b[2], b[0]]] {
                            if v == u || v == w {
                                continue;
                            }
                            let point = self.vertices[v as usize];
                            let e1 = self.vertices[u as usize];
                            let e2 = self.vertices[w as usize];
                            let line = geo::Line::new(
                                geo::Coord { x: e1[0], y: e1[1] },
                                geo::Coord { x: e2[0], y: e2[1] },
                            );
                            if line.intersects(&geo::Point::new(point[0], point[1])) {
                                return Err("Terrain has a T-junction or partial shared edge; split and weld the source explicitly".into());
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
    pub fn edges(&self) -> Result<Vec<[[f64; 3]; 2]>, String> {
        if self
            .vertices
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || v.abs() > 100000.)
        {
            return Err("Invalid terrain edge coordinates".into());
        }
        let mut edges = BTreeSet::new();
        for f in &self.faces {
            self.triangle(*f)?;
            for [a, b] in [[f[0], f[1]], [f[1], f[2]], [f[2], f[0]]] {
                edges.insert((a.min(b), a.max(b)));
            }
        }
        Ok(edges
            .into_iter()
            .map(|(a, b)| [self.vertices[a as usize], self.vertices[b as usize]])
            .collect())
    }
    pub fn height_at(&self, xy: [f64; 2]) -> Result<Option<(f64, usize)>, String> {
        self.validate()?;
        if xy.iter().any(|v| !v.is_finite() || v.abs() > 100000.) {
            return Err("Invalid terrain query coordinates".into());
        }
        for (i, &face) in self.faces.iter().enumerate() {
            let t = self.triangle(face)?;
            if polygon(t).intersects(&geo::Point::new(xy[0], xy[1])) {
                return Ok(Some((height(t, xy)?, i)));
            }
        }
        Ok(None)
    }
    pub fn metrics(&self) -> Result<Metrics, String> {
        self.validate()?;
        let mut projected = 0.;
        let mut surface = 0.;
        let mut bounds = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
        let mut edges = BTreeMap::new();
        for v in &self.vertices {
            for (k, x) in v.iter().enumerate() {
                bounds[0][k] = bounds[0][k].min(*x);
                bounds[1][k] = bounds[1][k].max(*x);
            }
        }
        for &f in &self.faces {
            let t = self.triangle(f)?;
            projected += twice_area(t) / 2.;
            let [a, b, c] = t.map(DVec3::from);
            surface += (b - a).cross(c - a).length() / 2.;
            for [a, b] in [[f[0], f[1]], [f[1], f[2]], [f[2], f[0]]] {
                *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            }
        }
        Ok(Metrics {
            vertices: self.vertices.len(),
            triangles: self.faces.len(),
            projected_area_m2: projected,
            surface_area_m2: surface,
            bounds_m: bounds,
            boundary_edges: edges.values().filter(|n| **n == 1).count(),
        })
    }
    pub fn mesh(
        &self,
        clip: Option<&[Point]>,
        z: Option<(f64, f64)>,
    ) -> Result<SurfaceMesh, String> {
        self.validate()?;
        crate::geometry_validation::validate_clip_context(clip, z)?;
        let mut mesh = SurfaceMesh::default();
        for &f in &self.faces {
            let t = self.triangle(f)?;
            let xy = t.map(|p| Point::new(p[0], p[1]));
            let paths = if let Some(clip) = clip {
                crate::geometry::intersect_footprint_with_polygon(&xy, clip)
            } else {
                vec![xy.to_vec()]
            };
            for path in paths {
                let mut cells = vec![
                    path.into_iter()
                        .map(|p| {
                            Ok(crate::planar_partition::Vertex {
                                p,
                                h: height(t, [p.x, p.y])?,
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                ];
                if let Some((lo, hi)) = z {
                    for level in [lo, hi] {
                        cells = cells
                            .into_iter()
                            .flat_map(|p| crate::planar_partition::split(p, level))
                            .collect();
                    }
                    cells.retain(|p| p.iter().all(|v| v.h >= lo - 1e-9 && v.h <= hi + 1e-9));
                }
                for p in cells {
                    if p.len() < 3 {
                        continue;
                    }
                    let points = p.iter().map(|v| v.p).collect::<Vec<_>>();
                    for triangle in crate::geometry::triangulate_polygon(&points) {
                        let mut out = [DVec3::ZERO; 3];
                        for (i, v) in triangle.into_iter().enumerate() {
                            out[i] = DVec3::new(v.x, v.y, height(t, [v.x, v.y])?);
                        }
                        mesh.triangles.push(out);
                    }
                    for i in 0..p.len() {
                        let a = p[i];
                        let b = p[(i + 1) % p.len()];
                        mesh.edges
                            .push([DVec3::new(a.p.x, a.p.y, a.h), DVec3::new(b.p.x, b.p.y, b.h)]);
                    }
                }
            }
        }
        Ok(mesh)
    }
}
fn height(t: [[f64; 3]; 3], p: [f64; 2]) -> Result<f64, String> {
    let d = twice_area(t);
    if !d.is_finite() || d.abs() <= 2e-8 {
        return Err("Degenerate terrain height interpolation".into());
    }
    let x = p[0] - t[0][0];
    let y = p[1] - t[0][1];
    let b = (x * (t[2][1] - t[0][1]) - y * (t[2][0] - t[0][0])) / d;
    let c = ((t[1][0] - t[0][0]) * y - (t[1][1] - t[0][1]) * x) / d;
    let z = t[0][2] + b * (t[1][2] - t[0][2]) + c * (t[2][2] - t[0][2]);
    if z.is_finite() {
        Ok(z)
    } else {
        Err("Invalid terrain interpolation".into())
    }
}
impl Terrain {
    pub fn validate(&self) -> Result<(), String> {
        if self.geometry.vertices.len() != self.imported.vertices.len()
            || self.geometry.faces != self.imported.faces
        {
            return Err("Terrain edits must keep the imported vertex count and topology".into());
        }
        if self.name.trim().is_empty()
            || self.name.len() > 256
            || !crate::storage::valid_document_identity(&self.origin.source_sha256)
        {
            return Err("Invalid terrain source identity or name".into());
        }
        if let Placement::SiteReference {
            horizontal,
            vertical,
        } = &self.origin.placement
        {
            SiteContext {
                horizontal: Some(horizontal.clone()),
                vertical: Some(vertical.clone()),
                ..Default::default()
            }
            .validate()?;
        }
        self.geometry.validate()?;
        self.imported.validate()
    }
    pub fn user_modified(&self) -> bool {
        self.geometry != self.imported
    }
    pub fn reference_current(&self, site: &SiteContext) -> bool {
        match &self.origin.placement {
            Placement::ModelXyz => true,
            Placement::SiteReference {
                horizontal,
                vertical,
            } => {
                site.horizontal.as_ref() == Some(horizontal)
                    && site.vertical.as_ref() == Some(vertical)
            }
        }
    }
}
pub fn apply(site: &SiteContext, edit: &Edit) -> Result<Option<Terrain>, String> {
    let mut terrain = site.terrain.clone();
    match edit {
        Edit::Import { proposal } => {
            proposal.terrain.validate()?;
            if proposal.terrain.user_modified() {
                return Err("Terrain import must retain exact source geometry".into());
            }
            if terrain != proposal.expected_previous {
                return Err("Terrain changed after preview".into());
            }
            if terrain.as_ref().is_some_and(Terrain::user_modified) {
                return Err("Reimport refused: terrain contains user edits".into());
            }
            if !proposal.terrain.reference_current(site) {
                return Err("Terrain reference changed after preview".into());
            }
            if matches!(
                proposal.terrain.origin.placement,
                Placement::SiteReference { .. }
            ) && !matches!(
                site.control_check()?,
                super::ControlCheck::Measured {
                    consistent: true,
                    ..
                }
            ) {
                return Err(
                    "Projected terrain requires two consistent controls and a stated tolerance"
                        .into(),
                );
            }
            terrain = Some(proposal.terrain.clone());
        }
        Edit::UpdateVertex {
            index,
            expected,
            position,
        } => {
            let t = terrain.as_mut().ok_or("No terrain surface")?;
            let v = t
                .geometry
                .vertices
                .get_mut(*index as usize)
                .ok_or("Terrain vertex no longer exists")?;
            if v != expected {
                return Err("Terrain vertex changed after editing began".into());
            }
            *v = *position;
            t.validate()?;
        }
        Edit::Remove { expected } => {
            if terrain.as_ref() != Some(expected) {
                return Err("Terrain changed after preview".into());
            }
            terrain = None;
        }
    }
    Ok(terrain)
}
pub fn summary(site: &SiteContext) -> serde_json::Value {
    match &site.terrain {
        None => {
            serde_json::json!({"status":"unknown","reason":"No terrain surface; flat ground is not inferred","surroundings":"Unverified neighboring elevations and windows"})
        }
        Some(t) => {
            serde_json::json!({"status":"declared_unverified","name":t.name,"source_sha256":t.origin.source_sha256,"geometry_user_modified":t.user_modified(),"reference_current":t.reference_current(site),"metrics":t.geometry.metrics().map(|m|serde_json::json!(m)).unwrap_or_else(|reason|serde_json::json!({"status":"invalid","reason":reason})),"surroundings":"Unverified neighboring elevations and windows"})
        }
    }
}
