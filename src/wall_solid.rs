//! Material cells derived once from wall parameters and hosted void station ranges.
//! No Project is stored here, and no float/integer Boolean round trip changes cells.
use crate::{
    derived_geometry::{self, GeometryDiagnostic, GeometryIdentity, SurfaceMesh, VolumeSource},
    model::{Element, Point, Project, Shape, WallContour},
    planar_partition::{Vertex, coincident, split},
};
use geo::{Area, Orient, Validation, orient::Direction};
use glam::DVec3;
use std::collections::BTreeMap;
pub const WALL_DERIVATION_VERSION: u32 = 2;
const STATION_MERGE_M: f64 = crate::precision::GeometryContext::SOLVER.wall_station_merge_m; // Existing wall_solid_prisms station partition policy.
fn key(p: Point) -> [u64; 2] {
    [p.x, p.y].map(|x| {
        if x == 0. {
            0.0_f64.to_bits()
        } else {
            x.to_bits()
        }
    })
}
fn polygon(path: &[Point]) -> geo::Polygon<f64> {
    geo::Polygon::new(
        geo::LineString::from(
            path.iter()
                .chain(path.first())
                .map(|p| (p.x, p.y))
                .collect::<Vec<_>>(),
        ),
        vec![],
    )
    .orient(Direction::Default)
}
#[derive(Clone, Debug, PartialEq)]
pub struct WallContext {
    pub story: u32,
    pub joined_footprints: BTreeMap<u64, Vec<Point>>,
    /// Conservative neighbours, not a claim that their final solids intersect.
    pub join_candidates: BTreeMap<u64, Vec<u64>>,
    pub input_fingerprint: [u8; 32],
}
pub fn context(
    project: &Project,
    story: u32,
    candidate: Option<&Element>,
) -> Result<WallContext, String> {
    if !project.stories.iter().any(|s| s.id == story) {
        return Err("Wall story does not exist".into());
    }
    let mut walls: Vec<_> = project
        .elements
        .iter()
        .map(|e| candidate.filter(|c| c.id == e.id).unwrap_or(e))
        .filter(|e| {
            e.story == story && matches!(e.shape, Shape::Wall { .. }) && e.wall_contour.is_none()
        })
        .collect();
    if let Some(c) = candidate
        && !project.elements.iter().any(|e| e.id == c.id)
        && c.story == story
        && matches!(c.shape, Shape::Wall { .. })
        && c.wall_contour.is_none()
    {
        walls.push(c);
    }
    let groups = project.layers.wall_groups(&walls);
    let inputs: Vec<_> = walls
        .iter()
        .zip(&groups)
        .map(|(e, g)| {
            (
                e.id,
                &e.shape,
                e.wall_settings
                    .as_ref()
                    .map(|s| (s.body_offset(e.thickness()), s.vertical_offsets())),
                g,
            )
        })
        .collect();
    let (polygons, join_candidates) =
        crate::geometry::plan_wall_polygons_with_dependencies(&walls, &groups);
    Ok(WallContext {
        story,
        joined_footprints: walls.iter().map(|w| w.id).zip(polygons).collect(),
        join_candidates,
        input_fingerprint: derived_geometry::input_fingerprint(&inputs, project.elevation(story))?,
    })
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WallSection {
    pub station_m: f64,
    pub positive: Point,
    pub negative: Point,
}
#[derive(Clone, Debug, PartialEq)]
pub struct WallCell {
    pub vertices: Vec<Point>,
    pub triangles: Vec<[Point; 3]>,
    pub station_m: f64,
    pub area_m2: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct WallBand {
    pub bottom_m: f64,
    pub top_m: f64,
    pub cell_indices: Vec<usize>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct HostedVoid {
    pub id: u64,
    pub host_wall: u64,
    pub kind: crate::model::OpeningKind,
    pub input_fingerprint: [u8; 32],
    pub from_m: f64,
    pub to_m: f64,
    pub bottom_m: f64,
    pub top_m: f64,
    pub cell_indices: Vec<usize>,
    pub reference_span: [Point; 2],
    pub nominal_area_m2: f64,
    pub sampled_cut_volume_m3: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct WallSnapshot {
    pub gross_footprint: geo::MultiPolygon<f64>,
    pub plan_path: Vec<Point>,
    pub plan_triangles: Vec<[Point; 3]>,
    pub cells: Vec<WallCell>,
    pub cross_sections: Vec<WallSection>,
    pub bands: Vec<WallBand>,
    pub voids: Vec<HostedVoid>,
    pub mesh: SurfaceMesh,
    pub bounds: Option<[DVec3; 2]>,
    pub gross_volume_m3: f64,
    pub material_volume_m3: f64,
    pub sampled_volume_m3: f64,
    pub nominal_reference_area_m2: f64,
    pub empty_by_openings: bool,
    pub volume_source: VolumeSource,
    pub max_chord_error_m: f64,
    pub station_coalesced_count: usize,
    pub station_label_max_adjustment_m: f64,
    pub normalization: crate::column_clip::ClipNormalization,
    neighbors: Vec<Vec<Option<usize>>>,
    smooth_anchors: Option<Vec<Point>>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DerivedWall {
    pub identity: GeometryIdentity,
    pub geometry: WallSnapshot,
}
fn neighbors(cells: &[WallCell]) -> Result<Vec<Vec<Option<usize>>>, String> {
    let mut edges = BTreeMap::<([u64; 2], [u64; 2]), Vec<(usize, usize)>>::new();
    for (i, c) in cells.iter().enumerate() {
        for j in 0..c.vertices.len() {
            let (a, b) = (
                key(c.vertices[j]),
                key(c.vertices[(j + 1) % c.vertices.len()]),
            );
            edges
                .entry(if a < b { (a, b) } else { (b, a) })
                .or_default()
                .push((i, j));
        }
    }
    let mut result: Vec<_> = cells.iter().map(|c| vec![None; c.vertices.len()]).collect();
    for e in edges.values() {
        match e.as_slice() {
            [(a, b), (c, d)] => {
                result[*a][*b] = Some(*c);
                result[*c][*d] = Some(*a);
            }
            [_] => {}
            _ => return Err("Wall planar partition has more than two neighboring cells".into()),
        }
    }
    Ok(result)
}
fn cell_surface(
    c: &WallCell,
    lo: f64,
    hi: f64,
    below: bool,
    above: bool,
    sides: &[bool],
) -> (SurfaceMesh, Vec<bool>) {
    let mut mesh = SurfaceMesh::default();
    let mut hidden = Vec::new();
    for top in [false, true] {
        for &t in &c.triangles {
            let mut t = t.map(|p| DVec3::new(p.x, p.y, if top { hi } else { lo }));
            if !top {
                t.swap(1, 2);
            }
            mesh.triangles.push(t);
            hidden.push(if top { above } else { below });
        }
    }
    for i in 0..c.vertices.len() {
        let a = c.vertices[i];
        let b = c.vertices[(i + 1) % c.vertices.len()];
        let q = [
            DVec3::new(a.x, a.y, lo),
            DVec3::new(b.x, b.y, lo),
            DVec3::new(b.x, b.y, hi),
            DVec3::new(a.x, a.y, hi),
        ];
        mesh.triangles
            .extend([[q[0], q[1], q[2]], [q[0], q[2], q[3]]]);
        hidden.extend([sides[i]; 2]);
    }
    (mesh, hidden)
}
fn creases(mesh: &mut SurfaceMesh) {
    let key = |p: DVec3| {
        p.to_array().map(|x| {
            if x == 0. {
                0.0_f64.to_bits()
            } else {
                x.to_bits()
            }
        })
    };
    let mut edges = BTreeMap::<([u64; 3], [u64; 3]), Vec<DVec3>>::new();
    for &[a, b, c] in &mesh.triangles {
        let n = (b - a).cross(c - a).normalize();
        for (a, b) in [(a, b), (b, c), (c, a)] {
            let (a, b) = (key(a), key(b));
            edges
                .entry(if a < b { (a, b) } else { (b, a) })
                .or_default()
                .push(n);
        }
    }
    mesh.edges.clear();
    for ((a, b), n) in edges {
        if n.len() != 2 || n[0].cross(n[1]).length() > 1e-7 || n[0].dot(n[1]) < 0. {
            mesh.edges.push([
                DVec3::from_array(a.map(f64::from_bits)),
                DVec3::from_array(b.map(f64::from_bits)),
            ]);
        }
    }
}
fn material_surface(
    cells: &[WallCell],
    bands: &[WallBand],
    neighbors: &[Vec<Option<usize>>],
    clip: Option<&[Point]>,
    range: Option<(f64, f64)>,
) -> Result<crate::column_clip::ClippedMesh, String> {
    let active: Vec<Vec<bool>> = bands
        .iter()
        .map(|b| {
            (0..cells.len())
                .map(|i| b.cell_indices.contains(&i))
                .collect()
        })
        .collect();
    let mut mesh = SurfaceMesh::default();
    for (bi, b) in bands.iter().enumerate() {
        for &ci in &b.cell_indices {
            let below = bi > 0 && bands[bi - 1].top_m == b.bottom_m && active[bi - 1][ci];
            let above =
                bi + 1 < bands.len() && bands[bi + 1].bottom_m == b.top_m && active[bi + 1][ci];
            let sides: Vec<_> = neighbors[ci]
                .iter()
                .map(|n| n.is_some_and(|n| active[bi][n]))
                .collect();
            let (source, hidden) =
                cell_surface(&cells[ci], b.bottom_m, b.top_m, below, above, &sides);
            if clip.is_none() && range.is_none() {
                mesh.triangles.extend(
                    source
                        .triangles
                        .into_iter()
                        .zip(hidden)
                        .filter_map(|(t, h)| (!h).then_some(t)),
                );
            } else {
                let seams: Vec<_> = source
                    .triangles
                    .iter()
                    .zip(&hidden)
                    .filter_map(|(t, h)| h.then_some(*t))
                    .collect();
                mesh.triangles.extend(
                    crate::column_clip::clip_convex_mesh(source, clip, range, &hidden, &seams)?
                        .triangles,
                );
            }
        }
    }
    let mut result = crate::column_clip::normalize_clipped_mesh(mesh)?;
    creases(&mut result.mesh);
    if !result.mesh.triangles.is_empty() {
        derived_geometry::inspect_closed_surface(&result.mesh.triangles)?;
    }
    Ok(result)
}
pub(crate) fn boundary_paths(
    cells: &[WallCell],
    indices: &[usize],
) -> Result<Vec<Vec<Point>>, String> {
    let mut edges = BTreeMap::<([u64; 2], [u64; 2]), Vec<(Point, Point)>>::new();
    for &i in indices {
        let c = &cells[i];
        for j in 0..c.vertices.len() {
            let (a, b) = (c.vertices[j], c.vertices[(j + 1) % c.vertices.len()]);
            let (ka, kb) = (key(a), key(b));
            edges
                .entry(if ka < kb { (ka, kb) } else { (kb, ka) })
                .or_default()
                .push((a, b));
        }
    }
    let mut remaining: Vec<_> = edges
        .into_values()
        .filter_map(|e| (e.len() == 1).then(|| e[0]))
        .collect();
    let mut paths = Vec::new();
    while let Some((a, b)) = remaining.pop() {
        let mut path = vec![a];
        let start = key(a);
        let (mut previous, mut current) = (a, b);
        let mut limit = remaining.len() + 2;
        while key(current) != start {
            path.push(current);
            if limit == 0 {
                return Err("Wall boundary did not close".into());
            }
            limit -= 1;
            let dx = current.x - previous.x;
            let dy = current.y - previous.y;
            let index = remaining
                .iter()
                .enumerate()
                .filter(|(_, e)| key(e.0) == key(current))
                .max_by(|(_, a), (_, b)| {
                    let angle = |e: &(Point, Point)| {
                        let x = e.1.x - current.x;
                        let y = e.1.y - current.y;
                        (dx * y - dy * x).atan2(dx * x + dy * y)
                    };
                    angle(a).total_cmp(&angle(b))
                })
                .map(|(i, _)| i)
                .ok_or("Wall boundary has an unpaired vertex")?;
            let (_, next) = remaining.remove(index);
            previous = current;
            current = next;
        }
        paths.push(path);
    }
    Ok(paths)
}
pub(crate) fn polygon_surface(path: &[Point], lo: f64, hi: f64) -> Result<SurfaceMesh, String> {
    let mut vertices = path.to_vec();
    if vertices.first() == vertices.last() {
        vertices.pop();
    }
    let p = polygon(&vertices);
    if !p.is_valid() || p.unsigned_area() <= 0. || !lo.is_finite() || !hi.is_finite() || hi <= lo {
        return Err("Invalid wall display polygon".into());
    }
    let area = crate::geometry::polygon_area(&vertices);
    if area < 0. {
        vertices.reverse();
    }
    let triangles = derived_geometry::triangulate_with_boundary(&p)
        .into_iter()
        .map(|t| t.map(|p| Point::new(p.x, p.y)))
        .collect();
    let c = WallCell {
        vertices,
        triangles,
        station_m: 0.,
        area_m2: area.abs(),
    };
    selection_surface(&[c], lo, hi, None, None).map(|r| r.mesh)
}
pub(crate) fn selection_surface(
    cells: &[WallCell],
    lo: f64,
    hi: f64,
    clip: Option<&[Point]>,
    range: Option<(f64, f64)>,
) -> Result<crate::column_clip::ClippedMesh, String> {
    let neighbors = neighbors(cells)?;
    material_surface(
        cells,
        &[WallBand {
            bottom_m: lo,
            top_m: hi,
            cell_indices: (0..cells.len()).collect(),
        }],
        &neighbors,
        clip,
        range,
    )
}
fn smooth_edges(mesh: &mut SurfaceMesh, anchors: Option<&[Point]>, clip: Option<&[Point]>) {
    if let Some(anchors) = anchors {
        mesh.edges.retain(|[a, b]| {
            let p = Point::new(a.x, a.y);
            p.distance(Point::new(b.x, b.y)) > 1e-8
                || anchors.iter().any(|a| a.distance(p) < 1e-5)
                || clip.is_some_and(|c| {
                    (0..c.len()).any(|i| {
                        crate::geometry::point_on_segment(p, c[i], c[(i + 1) % c.len()], 1e-5)
                    })
                })
        });
    }
}
impl WallSnapshot {
    pub fn contains_at(&self, p: geo::Point<f64>, z: f64) -> bool {
        use geo::Contains;
        self.bands
            .iter()
            .filter(|b| z > b.bottom_m && z < b.top_m)
            .any(|b| {
                b.cell_indices
                    .iter()
                    .any(|&i| polygon(&self.cells[i].vertices).contains(&p))
            })
    }
    pub fn clipped(
        &self,
        clip: Option<&[Point]>,
        range: Option<(f64, f64)>,
    ) -> Result<SurfaceMesh, String> {
        self.clipped_with_report(clip, range).map(|r| r.mesh)
    }
    pub fn clipped_with_report(
        &self,
        clip: Option<&[Point]>,
        range: Option<(f64, f64)>,
    ) -> Result<crate::column_clip::ClippedMesh, String> {
        crate::geometry_validation::validate_clip_context(clip, range)?;
        if clip.is_some_and(|p| p.len() < 3 || p.iter().any(|p| !p.finite()))
            || range.is_some_and(|(a, b)| !a.is_finite() || !b.is_finite())
        {
            return Err("Invalid wall clipping context".into());
        }
        if clip.is_none() && range.is_none() {
            return Ok(crate::column_clip::ClippedMesh {
                mesh: self.mesh.clone(),
                normalization: self.normalization.clone(),
            });
        }
        let mut result = material_surface(&self.cells, &self.bands, &self.neighbors, clip, range)?;
        smooth_edges(&mut result.mesh, self.smooth_anchors.as_deref(), clip);
        Ok(result)
    }
}
pub fn vertical_range(project: &Project, e: &Element) -> Option<(f64, f64)> {
    let Shape::Wall { height, .. } = e.shape else {
        return None;
    };
    let base = project.stories.iter().find(|s| s.id == e.story)?.elevation;
    let (lo, hi) = e
        .wall_settings
        .as_ref()
        .map_or((0., 0.), |s| s.vertical_offsets());
    Some((base + lo, base + height + hi))
}
/// A borrowed, per-read index of inputs and disposable results, never an owned model.
/// Reusing it for a frame avoids a full element scan per wall or hosted opening.
pub(crate) struct WallQuery<'a> {
    project: &'a Project,
    elements: BTreeMap<u64, &'a Element>,
    openings: BTreeMap<u64, Vec<&'a Element>>,
    contexts: BTreeMap<u32, WallContext>,
    walls: BTreeMap<u64, DerivedWall>,
}
impl<'a> WallQuery<'a> {
    pub fn new(project: &'a Project) -> Self {
        let mut openings = BTreeMap::<_, Vec<_>>::new();
        for e in &project.elements {
            if let Shape::Opening { host_wall, .. } = e.shape {
                openings.entry(host_wall).or_default().push(e);
            }
        }
        Self {
            project,
            elements: project.elements.iter().map(|e| (e.id, e)).collect(),
            openings,
            contexts: BTreeMap::new(),
            walls: BTreeMap::new(),
        }
    }
    pub fn context(&mut self, story: u32) -> Result<&WallContext, String> {
        if !self.contexts.contains_key(&story) {
            self.contexts
                .insert(story, context(self.project, story, None)?);
        }
        Ok(&self.contexts[&story])
    }
    pub fn wall(&mut self, id: u64) -> Result<&DerivedWall, GeometryDiagnostic> {
        if !self.walls.contains_key(&id) {
            let e = *self.elements.get(&id).ok_or_else(|| {
                GeometryDiagnostic::error(id, "ELEMENT_NOT_FOUND", "element", "Wall does not exist")
            })?;
            self.context(e.story).map_err(|m| {
                GeometryDiagnostic::error(id, "WALL_CONTEXT_FAILED", "connection", m)
            })?;
            let g = derive_from_inputs(
                self.project,
                e,
                &self.contexts[&e.story],
                self.openings.get(&id).map_or(&[], Vec::as_slice),
            )?;
            self.walls.insert(id, g);
        }
        Ok(&self.walls[&id])
    }
    pub fn opening(
        &mut self,
        id: u64,
    ) -> Result<crate::opening_geometry::DerivedOpening, GeometryDiagnostic> {
        let e = *self.elements.get(&id).ok_or_else(|| {
            GeometryDiagnostic::error(id, "ELEMENT_NOT_FOUND", "element", "Opening does not exist")
        })?;
        let Shape::Opening { host_wall, .. } = e.shape else {
            return Err(GeometryDiagnostic::error(
                id,
                "NOT_OPENING",
                "element",
                "Element is not an opening",
            ));
        };
        let host = *self.elements.get(&host_wall).ok_or_else(|| {
            GeometryDiagnostic::error(
                id,
                "OPENING_HOST_MISSING",
                "host",
                "Opening references a missing host wall",
            )
        })?;
        let base = self.project.elevation(host.story);
        let g = self.wall(host_wall)?;
        crate::opening_geometry::derive_with_host(host, e, g, base)
    }
}
pub fn derive(project: &Project, wall: &Element) -> Result<DerivedWall, GeometryDiagnostic> {
    let c = context(project, wall.story, Some(wall))
        .map_err(|m| GeometryDiagnostic::error(wall.id, "WALL_CONTEXT_FAILED", "connection", m))?;
    derive_with_context(project, wall, &c)
}
pub fn derive_with_context(
    project: &Project,
    wall: &Element,
    ctx: &WallContext,
) -> Result<DerivedWall, GeometryDiagnostic> {
    let openings: Vec<_> = project
        .elements
        .iter()
        .filter(|e| matches!(e.shape,Shape::Opening{host_wall,..}if host_wall==wall.id))
        .collect();
    derive_from_inputs(project, wall, ctx, &openings)
}
pub(crate) fn derive_from_inputs(
    project: &Project,
    wall: &Element,
    ctx: &WallContext,
    openings: &[&Element],
) -> Result<DerivedWall, GeometryDiagnostic> {
    let fail = |m| GeometryDiagnostic::error(wall.id, "INVALID_WALL_SOLID", "solid", m);
    let Shape::Wall {
        start,
        end,
        thickness,
        height: wall_height,
    } = wall.shape
    else {
        return Err(fail("Element is not a wall".into()));
    };
    if ctx.story != wall.story
        || ![start, end]
            .into_iter()
            .all(crate::precision::GeometryContext::point_in_range)
        || !crate::precision::segment_meets_minimum(start, end, 0.01)
        || !thickness.is_finite()
        || !(0.01..=5.).contains(&thickness)
        || !wall_height.is_finite()
        || !(0.01..=100.).contains(&wall_height)
    {
        return Err(fail("Invalid wall parameters".into()));
    }
    crate::wall_geometry::validate(wall).map_err(fail)?;
    if let Some(s) = &wall.wall_settings {
        s.validate().map_err(fail)?;
    }
    let base = project.elevation(wall.story);
    let (bo, to) = wall
        .wall_settings
        .as_ref()
        .map_or((0., 0.), |s| s.vertical_offsets());
    let (bottom, top) = (base + bo, base + wall_height + to);
    let length = crate::wall_geometry::reference_length(wall);
    if !bottom.is_finite() || !top.is_finite() || top <= bottom {
        return Err(fail("Invalid wall elevations".into()));
    }
    let mut cuts = vec![0., length];
    let mut holes = Vec::new();
    let mut levels = vec![bottom, top];
    for &e in openings {
        let Shape::Opening {
            host_wall,
            offset,
            width,
            sill,
            height,
            ..
        } = e.shape
        else {
            unreachable!()
        };
        if e.story != wall.story
            || ![offset, width, sill, height]
                .into_iter()
                .all(f64::is_finite)
            || !(0.05..=100.).contains(&width)
            || !(0.05..=100.).contains(&height)
            || !(0. ..=100.).contains(&sill)
            || offset.abs() >= 100000.
            || offset - width / 2. < -1e-4
            || offset + width / 2. > length + 1e-4
            || sill < bo - 1e-4
            || sill + height > wall_height + to + 1e-4
        {
            return Err(fail(format!("Invalid hosted opening {}", e.id)));
        }
        let (from, to) = (
            (offset - width / 2.).clamp(0., length),
            (offset + width / 2.).clamp(0., length),
        );
        let (lo, hi) = (
            (base + sill).clamp(bottom, top),
            (base + sill + height).clamp(bottom, top),
        );
        cuts.extend([from, to]);
        levels.extend([lo, hi]);
        holes.push((e, host_wall, from, to, lo, hi, width * height));
    }
    let cuts_before = cuts.len();
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < STATION_MERGE_M);
    if let Some(last) = cuts.last_mut()
        && length - *last < STATION_MERGE_M
    {
        *last = length;
    }
    let station_coalesced_count = cuts_before - cuts.len();
    let canonical = |s: f64| {
        cuts.iter()
            .copied()
            .find(|v| (*v - s).abs() < STATION_MERGE_M)
            .unwrap_or(s)
    };
    let mut stations: Vec<_> = cuts
        .windows(2)
        .flat_map(|w| crate::wall_geometry::stations(wall, w[0], w[1]))
        .collect();
    if let Some(WallContour::Arc { arc }) = wall.wall_contour {
        let (center, sweep) = crate::geometry::arc_center_sweep(start, end, arc)
            .ok_or_else(|| fail("Invalid wall arc".into()))?;
        let a = (start.y - center.y).atan2(start.x - center.x);
        for i in 0..4 {
            let angle = i as f64 * std::f64::consts::FRAC_PI_2;
            let travel = if sweep > 0. {
                (angle - a).rem_euclid(std::f64::consts::TAU)
            } else {
                (a - angle).rem_euclid(std::f64::consts::TAU)
            };
            if travel < sweep.abs() {
                stations.push(travel * arc.radius);
            }
        }
    }
    stations.sort_by(f64::total_cmp);
    stations.dedup_by(|a, b| (*a - *b).abs() < STATION_MERGE_M);
    let sections: Vec<_> = stations
        .iter()
        .map(|&s| crate::wall_geometry::section_at(wall, s).map(|p| (s, p)))
        .collect::<Option<_>>()
        .ok_or_else(|| fail("Wall section failed".into()))?;
    let mut sampled_path: Vec<_> = sections.iter().map(|(_, p)| p.0).collect();
    sampled_path.extend(sections.iter().rev().map(|(_, p)| p.1));
    let plan_path = if wall.wall_contour.is_some() {
        sampled_path
    } else {
        ctx.joined_footprints
            .get(&wall.id)
            .cloned()
            .unwrap_or(sampled_path)
    };
    let gross = polygon(&plan_path);
    if !gross.is_valid() || gross.unsigned_area() <= 0. {
        return Err(fail("Invalid gross wall footprint".into()));
    }
    let mut station_label_max_adjustment_m: f64 = 0.;
    let mut cells = Vec::new();
    let mut add = |mut vertices: Vec<Point>, station_m: f64| -> Result<(), GeometryDiagnostic> {
        let area = crate::geometry::polygon_area(&vertices);
        if area < 0. {
            vertices.reverse();
        }
        if area == 0. || !area.is_finite() {
            return Err(fail("Wall partition has a degenerate cell".into()));
        }
        let triangles = derived_geometry::triangulate_with_boundary(&polygon(&vertices))
            .into_iter()
            .map(|t| t.map(|p| Point::new(p.x, p.y)))
            .collect();
        cells.push(WallCell {
            vertices,
            triangles,
            station_m,
            area_m2: area.abs(),
        });
        Ok(())
    };
    if wall.wall_contour.is_some() {
        for w in sections.windows(2) {
            let (a, b) = (w[0].1, w[1].1);
            add(vec![a.0, b.0, b.1, a.1], (w[0].0 + w[1].0) / 2.)?;
        }
    } else {
        let (dx, dy) = (end.x - start.x, end.y - start.y);
        let len = dx.hypot(dy);
        let mut vertex = |p: Point| {
            let raw_h = ((p.x - start.x) * dx + (p.y - start.y) * dy) / len;
            let roundoff = crate::precision::arithmetic_length_roundoff_m(
                p.x.abs()
                    .max(p.y.abs())
                    .max(start.x.abs())
                    .max(start.y.abs())
                    .max(len)
                    .max(thickness),
            );
            let h = cuts
                .iter()
                .copied()
                .find(|&v| coincident(raw_h, v, thickness) || (raw_h - v).abs() <= roundoff)
                .unwrap_or(raw_h);
            station_label_max_adjustment_m = station_label_max_adjustment_m.max((h - raw_h).abs());
            Vertex { p, h }
        };
        for t in derived_geometry::triangulate_with_boundary(&gross) {
            let mut parts = vec![t.map(|p| vertex(Point::new(p.x, p.y))).to_vec()];
            for &s in &cuts {
                parts = parts.into_iter().flat_map(|p| split(p, s)).collect();
            }
            for p in parts {
                let station_m = p.iter().map(|v| v.h).sum::<f64>() / p.len() as f64;
                add(p.into_iter().map(|v| v.p).collect(), station_m)?;
            }
        }
    }
    let neighbor = neighbors(&cells).map_err(fail)?;
    let mut voids = Vec::new();
    for (e, host_wall, from, to, lo, hi, nominal_area) in holes {
        let (from, to) = (canonical(from), canonical(to));
        let indices: Vec<_> = cells
            .iter()
            .enumerate()
            .filter_map(|(i, c)| (c.station_m > from && c.station_m < to).then_some(i))
            .collect();
        let area = indices.iter().map(|&i| cells[i].area_m2).sum::<f64>();
        let a = crate::wall_geometry::point_tangent_at(wall, from)
            .ok_or_else(|| fail("Opening start section failed".into()))?
            .0;
        let b = crate::wall_geometry::point_tangent_at(wall, to)
            .ok_or_else(|| fail("Opening end section failed".into()))?
            .0;
        voids.push(HostedVoid {
            id: e.id,
            host_wall,
            kind: if let Shape::Opening { kind, .. } = e.shape {
                kind
            } else {
                unreachable!()
            },
            input_fingerprint: derived_geometry::input_fingerprint(&e.shape, base).map_err(fail)?,
            from_m: from,
            to_m: to,
            bottom_m: lo,
            top_m: hi,
            cell_indices: indices,
            reference_span: [a, b],
            nominal_area_m2: nominal_area,
            sampled_cut_volume_m3: area * (hi - lo),
        });
    }
    levels.sort_by(f64::total_cmp);
    levels.dedup();
    let mut bands = Vec::new();
    for w in levels.windows(2) {
        if w[1] <= w[0] {
            continue;
        }
        let z = (w[0] + w[1]) / 2.;
        let indices = (0..cells.len())
            .filter(|&i| {
                !voids
                    .iter()
                    .any(|h| z > h.bottom_m && z < h.top_m && h.cell_indices.contains(&i))
            })
            .collect();
        bands.push(WallBand {
            bottom_m: w[0],
            top_m: w[1],
            cell_indices: indices,
        });
    }
    let material_volume = bands
        .iter()
        .map(|b| {
            b.cell_indices
                .iter()
                .map(|&i| cells[i].area_m2)
                .sum::<f64>()
                * (b.top_m - b.bottom_m)
        })
        .sum::<f64>();
    let smooth_anchors = if matches!(wall.wall_contour, Some(WallContour::Arc { .. })) {
        let mut stations = vec![0., length];
        for h in &voids {
            stations.extend([h.from_m, h.to_m]);
        }
        Some(
            sections
                .iter()
                .filter(|(s, _)| stations.iter().any(|v| (*v - *s).abs() < STATION_MERGE_M))
                .flat_map(|(_, p)| [p.0, p.1])
                .collect::<Vec<_>>(),
        )
    } else {
        None
    };
    let mut result = material_surface(&cells, &bands, &neighbor, None, None).map_err(fail)?;
    smooth_edges(&mut result.mesh, smooth_anchors.as_deref(), None);
    if result.mesh.triangles.is_empty() && material_volume > 0. {
        return Err(fail(
            "Wall region has material but produced no surface".into(),
        ));
    }
    let (bounds, sampled_volume) = if result.mesh.triangles.is_empty() {
        (None, 0.)
    } else {
        let m = derived_geometry::inspect_closed_surface(&result.mesh.triangles).map_err(fail)?;
        (Some(m.bounds), m.signed_volume_m3)
    };
    if !crate::precision::TolerancePolicy::default()
        .volume_m3
        .accepts(sampled_volume, material_volume)
    {
        return Err(fail(format!(
            "Wall surface volume {sampled_volume} differs from cells {material_volume}"
        )));
    }
    let max_chord_error_m = if let Some(WallContour::Arc { arc }) = wall.wall_contour {
        let offset = wall
            .wall_settings
            .as_ref()
            .map_or(0., |s| s.body_offset(thickness));
        let step = stations
            .windows(2)
            .map(|w| (w[1] - w[0]) / arc.radius)
            .fold(0., f64::max);
        (arc.radius + offset.abs() + thickness / 2.) * (1. - (step / 2.).cos())
    } else {
        0.
    };
    let hole_inputs: Vec<_> = openings.iter().map(|e| (e.id, &e.shape)).collect();
    let identity = derived_geometry::identity(
        wall.id,
        &(
            &wall.shape,
            &wall.wall_contour,
            wall.wall_settings
                .as_ref()
                .map(|s| (s.body_offset(thickness), s.vertical_offsets())),
            ctx.joined_footprints.get(&wall.id),
            hole_inputs,
        ),
        base,
        WALL_DERIVATION_VERSION,
    )
    .map_err(fail)?;
    let empty_by_openings = material_volume == 0. && !voids.is_empty();
    Ok(DerivedWall {
        identity,
        geometry: WallSnapshot {
            gross_volume_m3: gross.unsigned_area() * (top - bottom),
            gross_footprint: geo::MultiPolygon(vec![gross]),
            plan_path,
            plan_triangles: cells
                .iter()
                .flat_map(|c| c.triangles.iter().copied())
                .collect(),
            cells,
            cross_sections: sections
                .iter()
                .map(|(station, p)| WallSection {
                    station_m: *station,
                    positive: p.0,
                    negative: p.1,
                })
                .collect(),
            bands,
            voids,
            mesh: result.mesh,
            bounds,
            material_volume_m3: material_volume,
            sampled_volume_m3: sampled_volume,
            nominal_reference_area_m2: length * wall_height,
            empty_by_openings,
            volume_source: if matches!(wall.wall_contour, Some(WallContour::Arc { .. })) {
                VolumeSource::SampledSurface
            } else {
                VolumeSource::AnalyticPrism
            },
            max_chord_error_m,
            station_coalesced_count,
            station_label_max_adjustment_m,
            normalization: result.normalization,
            neighbors: neighbor,
            smooth_anchors,
        },
    })
}
