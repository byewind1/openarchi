use crate::model::*;
use glam::{Mat4, Vec3};
pub fn footprint(e: &Element) -> [Point; 4] {
    match e.shape {
        Shape::Wall {
            start: a,
            end: b,
            thickness: t,
            ..
        } => wall_footprint(a, b, t),
        Shape::Slab { min: a, max: b, .. } => [a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)],
    }
}

fn wall_footprint(a: Point, b: Point, thickness: f32) -> [Point; 4] {
    let length = a.distance(b).max(0.001);
    let normal = Point::new(
        -(b.y - a.y) * thickness / length / 2.,
        (b.x - a.x) * thickness / length / 2.,
    );
    [
        Point::new(a.x + normal.x, a.y + normal.y),
        Point::new(b.x + normal.x, b.y + normal.y),
        Point::new(b.x - normal.x, b.y - normal.y),
        Point::new(a.x - normal.x, a.y - normal.y),
    ]
}
pub fn segment_distance(p: Point, a: Point, b: Point) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0., 1.);
    p.distance(Point::new(a.x + t * dx, a.y + t * dy))
}
pub fn contains(p: Point, poly: &[Point]) -> bool {
    if poly.len() < 3 {
        return false;
    }
    if (0..poly.len()).any(|i| segment_distance(p, poly[i], poly[(i + 1) % poly.len()]) <= 1e-5) {
        return true;
    }
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[j];
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside
        }
        j = i;
    }
    inside
}

pub fn point_in_polygon(p: Point, poly: &[Point]) -> bool {
    contains(p, poly)
}

pub fn rectangle_polygon(a: Point, b: Point) -> Vec<Point> {
    vec![a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)]
}

pub fn rotated_rectangle_polygon(a: Point, b: Point, side: Point) -> Option<Vec<Point>> {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len = dx.hypot(dy);
    if len < 1e-4 { return None; }
    let n = Point::new(-dy / len, dx / len);
    let depth = (side.x - b.x) * n.x + (side.y - b.y) * n.y;
    if depth.abs() < 1e-4 { return None; }
    let offset = Point::new(n.x * depth, n.y * depth);
    Some(vec![a, b, Point::new(b.x + offset.x, b.y + offset.y), Point::new(a.x + offset.x, a.y + offset.y)])
}

fn orientation(a: Point, b: Point, c: Point) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn segments_intersect(a: Point, b: Point, c: Point, d: Point) -> bool {
    let ab_c = orientation(a, b, c);
    let ab_d = orientation(a, b, d);
    let cd_a = orientation(c, d, a);
    let cd_b = orientation(c, d, b);
    let eps = 1e-5;
    if ab_c.abs() <= eps && segment_distance(c, a, b) <= eps { return true; }
    if ab_d.abs() <= eps && segment_distance(d, a, b) <= eps { return true; }
    if cd_a.abs() <= eps && segment_distance(a, c, d) <= eps { return true; }
    if cd_b.abs() <= eps && segment_distance(b, c, d) <= eps { return true; }
    (ab_c > 0.) != (ab_d > 0.) && (cd_a > 0.) != (cd_b > 0.)
}

/// Archicad-style editing selection: an element is included when at least one
/// of its editable plan nodes lies in the marquee. The whole element is then
/// operated on by group commands.
pub fn element_has_node_in_polygon(e: &Element, polygon: &[Point]) -> bool {
    let nodes: Vec<Point> = match e.shape {
        Shape::Wall { start, end, .. } => vec![start, end],
        Shape::Slab { .. } => footprint(e).into_iter().collect(),
    };
    nodes.into_iter().any(|p| contains(p, polygon))
}

/// Visibility selection for 3D uses geometric overlap, so a wall segment
/// crossing the marquee is included even when neither endpoint is inside.
pub fn element_intersects_polygon(e: &Element, polygon: &[Point]) -> bool {
    if polygon.len() < 3 { return false; }
    let shape = footprint(e);
    if shape.iter().any(|p| contains(*p, polygon))
        || polygon.iter().any(|p| contains(*p, &shape))
    {
        return true;
    }
    (0..shape.len()).any(|i| {
        let a = shape[i];
        let b = shape[(i + 1) % shape.len()];
        (0..polygon.len()).any(|j| segments_intersect(a, b, polygon[j], polygon[(j + 1) % polygon.len()]))
    })
}

pub fn element_fully_inside_polygon(e: &Element, polygon: &[Point]) -> bool {
    footprint(e).iter().all(|p| contains(*p, polygon))
}

pub fn element_overlaps_z_range(project: &Project, e: &Element, range: (f32, f32)) -> bool {
    let corners = corners(project, e);
    let lo = corners.iter().map(|p| p.z).fold(f32::INFINITY, f32::min);
    let hi = corners.iter().map(|p| p.z).fold(f32::NEG_INFINITY, f32::max);
    hi >= range.0 && lo <= range.1
}

pub fn element_fully_inside_volume(project: &Project, e: &Element, polygon: &[Point], range: Option<(f32, f32)>) -> bool {
    if !element_fully_inside_polygon(e, polygon) { return false; }
    let Some((bottom, top)) = range else { return true };
    let corners = corners(project, e);
    corners.iter().all(|p| p.z >= bottom && p.z <= top)
}

/// Triangulate a simple polygon with ear clipping. Marquee polygons may be
/// concave, so clipping each edge as though the whole area were convex is not
/// sufficient.
pub fn triangulate_polygon(poly: &[Point]) -> Vec<[Point; 3]> {
    if poly.len() < 3 { return Vec::new(); }
    let area: f32 = (0..poly.len()).map(|i| {
        let a = poly[i]; let b = poly[(i + 1) % poly.len()]; a.x * b.y - b.x * a.y
    }).sum();
    if area.abs() < 1e-6 { return Vec::new(); }
    let ccw = area > 0.;
    let mut indices: Vec<usize> = (0..poly.len()).collect();
    let mut out = Vec::with_capacity(poly.len() - 2);
    let mut guard = poly.len() * poly.len();
    while indices.len() > 3 && guard > 0 {
        guard -= 1;
        let mut ear = None;
        for i in 0..indices.len() {
            let ia = indices[(i + indices.len() - 1) % indices.len()];
            let ib = indices[i];
            let ic = indices[(i + 1) % indices.len()];
            let turn = orientation(poly[ia], poly[ib], poly[ic]);
            if if ccw { turn <= 1e-6 } else { turn >= -1e-6 } { continue; }
            let inside = indices.iter().copied().filter(|&j| j != ia && j != ib && j != ic)
                .any(|j| point_in_triangle(poly[j], poly[ia], poly[ib], poly[ic]));
            if !inside { ear = Some((i, [poly[ia], poly[ib], poly[ic]])); break; }
        }
        let Some((i, triangle)) = ear else { return Vec::new(); };
        out.push(triangle);
        indices.remove(i);
    }
    if indices.len() == 3 { out.push([poly[indices[0]], poly[indices[1]], poly[indices[2]]]); }
    out
}

fn point_in_triangle(p: Point, a: Point, b: Point, c: Point) -> bool {
    let x = orientation(a, b, p); let y = orientation(b, c, p); let z = orientation(c, a, p);
    (x >= -1e-6 && y >= -1e-6 && z >= -1e-6) || (x <= 1e-6 && y <= 1e-6 && z <= 1e-6)
}

/// Intersect a convex element footprint with an arbitrary simple Marquee.
/// Result fragments are convex polygons and their union is the exact XY cut.
pub fn intersect_footprint_with_polygon(subject: &[Point], clip: &[Point]) -> Vec<Vec<Point>> {
    let mut fragments = Vec::new();
    for tri in triangulate_polygon(clip) {
        let mut output = subject.to_vec();
        let area = orientation(tri[0], tri[1], tri[2]);
        for edge in 0..3 {
            let a = tri[edge]; let b = tri[(edge + 1) % 3];
            let input = std::mem::take(&mut output);
            if input.is_empty() { break; }
            let signed = |p: Point| orientation(a, b, p) * area.signum();
            let mut prev = *input.last().unwrap();
            let mut prev_in = signed(prev) >= -1e-5;
            for curr in input {
                let curr_in = signed(curr) >= -1e-5;
                if curr_in != prev_in {
                    let d = signed(prev) - signed(curr);
                    if d.abs() > 1e-8 {
                        let t = (signed(prev) / d).clamp(0., 1.);
                        output.push(Point::new(prev.x + (curr.x-prev.x)*t, prev.y + (curr.y-prev.y)*t));
                    }
                }
                if curr_in { output.push(curr); }
                prev = curr; prev_in = curr_in;
            }
        }
        if output.len() >= 3 && polygon_area(&output).abs() > 1e-6 { fragments.push(output); }
    }
    fragments
}

fn polygon_area(poly: &[Point]) -> f32 {
    (0..poly.len()).map(|i| { let a=poly[i]; let b=poly[(i+1)%poly.len()]; a.x*b.y-b.x*a.y }).sum::<f32>() * 0.5
}

/// Returns joined display footprints with near endpoints snapped to their
/// same-group centerline intersection and extended through the adjoining wall.
/// The same footprints drive the 2D outline and the ordinary 3D view; project
/// elements themselves remain unchanged.
pub fn plan_wall_footprints(walls: &[&Element]) -> Vec<[Point; 4]> {
    let mut centerlines: Vec<_> = walls
        .iter()
        .map(|wall| match wall.shape {
            Shape::Wall { start, end, .. } => (start, end),
            Shape::Slab { .. } => unreachable!("plan wall join groups contain walls only"),
        })
        .collect();
    let mut endpoint_extensions = vec![[0.0_f32; 2]; walls.len()];

    for (index, wall) in walls.iter().enumerate() {
        let own_thickness = wall.thickness();
        for endpoint_index in 0..2 {
            let endpoint = if endpoint_index == 0 { centerlines[index].0 } else { centerlines[index].1 };
            let mut nearest: Option<(f32, Point, f32)> = None;
            for (other_index, other) in walls.iter().enumerate() {
                if index == other_index { continue; }
                let (other_start, other_end, other_thickness) = match other.shape {
                    Shape::Wall { start, end, thickness, .. } => (start, end, thickness),
                    Shape::Slab { .. } => continue,
                };
                let own_start = match wall.shape { Shape::Wall { start, .. } => start, _ => unreachable!() };
                let own_end = match wall.shape { Shape::Wall { end, .. } => end, _ => unreachable!() };
                let own_direction = Point::new(own_end.x - own_start.x, own_end.y - own_start.y);
                let other_direction = Point::new(other_end.x - other_start.x, other_end.y - other_start.y);
                let denominator = own_direction.x * other_direction.y - own_direction.y * other_direction.x;
                if denominator.abs() < 1e-5 { continue; }
                let delta = Point::new(other_start.x - own_start.x, other_start.y - own_start.y);
                let own_t = (delta.x * other_direction.y - delta.y * other_direction.x) / denominator;
                let other_t = (delta.x * own_direction.y - delta.y * own_direction.x) / denominator;
                let candidate = Point::new(
                    own_start.x + own_direction.x * own_t,
                    own_start.y + own_direction.y * own_t,
                );
                // A wall drawn to the face of another wall can leave the
                // centerline endpoint up to one wall thickness from the
                // reference-line intersection. Cover that ordinary drafting
                // offset so the display join is not reduced to a barely
                // touching sliver (while leaving stored geometry editable).
                let tolerance = own_thickness.max(other_thickness)
                    + own_thickness.min(other_thickness) * 0.5
                    + 0.02;
                let other_length = other_start.distance(other_end).max(0.001);
                let endpoint_extension = tolerance / other_length;
                if other_t < -endpoint_extension || other_t > 1.0 + endpoint_extension { continue; }
                let distance = endpoint.distance(candidate);
                if distance <= tolerance && nearest.is_none_or(|(best, _, _)| distance < best) {
                    nearest = Some((distance, candidate, other_thickness));
                }
            }
            if let Some((_, joined, other_thickness)) = nearest {
                if endpoint_index == 0 { centerlines[index].0 = joined; }
                else { centerlines[index].1 = joined; }
                endpoint_extensions[index][endpoint_index] = other_thickness * 0.5;
            }
        }
    }

    walls.iter().enumerate().map(|(index, wall)| {
        let (mut start, mut end) = centerlines[index];
        let length = start.distance(end).max(0.001);
        let direction = Point::new((end.x - start.x) / length, (end.y - start.y) / length);
        start.x -= direction.x * endpoint_extensions[index][0];
        start.y -= direction.y * endpoint_extensions[index][0];
        end.x += direction.x * endpoint_extensions[index][1];
        end.y += direction.y * endpoint_extensions[index][1];
        wall_footprint(start, end, wall.thickness())
    }).collect()
}

/// Returns endpoints whose extended wall footprints reach another wall. The
/// renderer uses these to hide overlapping end faces in the 3D display.
pub fn plan_wall_attached_ends(walls: &[&Element], footprints: &[[Point; 4]]) -> Vec<(u64, bool)> {
    let mut attached = Vec::new();
    for (index, wall) in walls.iter().enumerate() {
        for (endpoint, at_start) in [
            (Point::new((footprints[index][0].x + footprints[index][3].x) * 0.5, (footprints[index][0].y + footprints[index][3].y) * 0.5), true),
            (Point::new((footprints[index][1].x + footprints[index][2].x) * 0.5, (footprints[index][1].y + footprints[index][2].y) * 0.5), false),
        ] {
            let joined = walls.iter().enumerate().any(|(other_index, other)| {
                if other.id == wall.id || other.material != wall.material || other.story != wall.story {
                    return false;
                }
                let Shape::Wall { height, .. } = wall.shape else { return false };
                let Shape::Wall { height: other_height, .. } = other.shape else { return false };
                (height - other_height).abs() <= 1e-4 && contains(endpoint, &footprints[other_index])
            });
            if joined { attached.push((wall.id, at_start)); }
        }
    }
    attached
}

/// Returns only the exposed perimeter fragments of already-joined wall
/// footprints. This removes internal cap and side strokes in plan.
pub fn wall_outline_segments(wall_footprints: &[[Point; 4]]) -> Vec<(Point, Point)> {
    let polygons: Vec<Vec<Point>> = wall_footprints.iter().map(|polygon| polygon.to_vec()).collect();
    wall_outline_segments_for_polygons(&polygons)
}

fn wall_outline_segments_for_polygons(wall_footprints: &[Vec<Point>]) -> Vec<(Point, Point)> {
    let mut outline = Vec::new();
    for (wall_index, polygon) in wall_footprints.iter().enumerate() {
        for edge_index in 0..polygon.len() {
            let a = polygon[edge_index];
            let b = polygon[(edge_index + 1) % polygon.len()];
            let mut covered = Vec::new();
            for (other_index, other) in wall_footprints.iter().enumerate() {
                if wall_index == other_index { continue; }
                if let Some(interval) = segment_inside_convex_polygon(a, b, other) {
                    if interval.1 - interval.0 <= 1e-5 { continue; }
                    let midpoint = interpolate(a, b, (interval.0 + interval.1) * 0.5);
                    let lies_on_other_boundary = (0..other.len()).any(|edge| {
                        segment_distance(midpoint, other[edge], other[(edge + 1) % other.len()]) <= 1e-5
                    });
                    let other_center = other.iter().fold(Point::default(), |sum, point| {
                        Point::new(sum.x + point.x / other.len() as f32, sum.y + point.y / other.len() as f32)
                    });
                    let same_interior_side = orientation(a, b, other_center).signum() == polygon_area(polygon).signum();
                    // A boundary shared from opposite sides is internal and
                    // disappears. Coincident outer edges with interiors on
                    // the same side are retained once.
                    if !lies_on_other_boundary || !same_interior_side || other_index < wall_index {
                        covered.push(interval);
                    }
                }
            }
            covered.sort_by(|left, right| left.0.total_cmp(&right.0));
            let mut cursor = 0.0_f32;
            for (start, end) in covered {
                let start = start.clamp(0.0, 1.0);
                let end = end.clamp(0.0, 1.0);
                if start > cursor + 1e-5 {
                    outline.push((interpolate(a, b, cursor), interpolate(a, b, start)));
                }
                cursor = cursor.max(end);
                if cursor >= 1.0 { break; }
            }
            if cursor < 1.0 - 1e-5 {
                outline.push((interpolate(a, b, cursor), b));
            }
        }
    }
    outline
}

/// Join the exposed perimeter fragments into continuous paths so the renderer
/// can stroke a corner as one path instead of leaving antialiasing cracks
/// between independently rasterized line segments.
pub fn wall_outline_paths(wall_footprints: &[[Point; 4]]) -> Vec<Vec<Point>> {
    wall_outline_paths_from_segments(wall_outline_segments(wall_footprints))
}

fn wall_outline_paths_from_segments(mut remaining: Vec<(Point, Point)>) -> Vec<Vec<Point>> {
    let mut paths = Vec::new();
    while let Some((start, end)) = remaining.pop() {
        let mut path = vec![start, end];
        loop {
            let tail = *path.last().unwrap();
            let head = path[0];
            let next = remaining.iter().position(|(a, b)| {
                a.distance(tail) <= 1e-3 || b.distance(tail) <= 1e-3
                    || a.distance(head) <= 1e-3 || b.distance(head) <= 1e-3
            });
            let Some(index) = next else { break };
            let (a, b) = remaining.swap_remove(index);
            if a.distance(tail) <= 1e-3 {
                path.push(b);
            } else if b.distance(tail) <= 1e-3 {
                path.push(a);
            } else if b.distance(head) <= 1e-3 {
                path.insert(0, a);
            } else if a.distance(head) <= 1e-3 {
                path.insert(0, b);
            }
        }
        if path.len() >= 2 {
            paths.push(path);
        }
    }
    paths
}

fn interpolate(a: Point, b: Point, t: f32) -> Point {
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

/// Clip a segment against a convex polygon and return the covered parameter
/// interval. Polygon winding may be clockwise or counter-clockwise.
fn segment_inside_convex_polygon(a: Point, b: Point, polygon: &[Point]) -> Option<(f32, f32)> {
    if polygon.len() < 3 { return None; }
    let winding = if polygon_area(polygon) >= 0.0 { 1.0 } else { -1.0 };
    let mut lo = 0.0_f32;
    let mut hi = 1.0_f32;
    for index in 0..polygon.len() {
        let c = polygon[index];
        let d = polygon[(index + 1) % polygon.len()];
        let at_start = winding * orientation(c, d, a);
        let at_end = winding * orientation(c, d, b);
        let start_inside = at_start >= -1e-5;
        let end_inside = at_end >= -1e-5;
        if !start_inside && !end_inside { return None; }
        if start_inside != end_inside {
            let crossing = (at_start / (at_start - at_end)).clamp(0.0, 1.0);
            if !start_inside { lo = lo.max(crossing); } else { hi = hi.min(crossing); }
        }
        if lo >= hi { return None; }
    }
    Some((lo, hi))
}

pub fn point_on_segment(p: Point, a: Point, b: Point, tolerance: f32) -> bool {
    segment_distance(p, a, b) <= tolerance
        && p.x >= a.x.min(b.x)-tolerance && p.x <= a.x.max(b.x)+tolerance
        && p.y >= a.y.min(b.y)-tolerance && p.y <= a.y.max(b.y)+tolerance
}
pub fn hit_plan(project: &Project, story: u32, p: Point, tolerance: f32) -> Option<u64> {
    project
        .elements
        .iter()
        .rev()
        .filter(|e| e.story == story && matches!(e.shape, Shape::Wall { .. }))
        .find(|e| {
            if let Shape::Wall {
                start,
                end,
                thickness,
                ..
            } = e.shape
            {
                segment_distance(p, start, end) <= thickness / 2. + tolerance
            } else {
                false
            }
        })
        .or_else(|| {
            project
                .elements
                .iter()
                .rev()
                .find(|e| e.story == story && contains(p, &footprint(e)))
        })
        .map(|e| e.id)
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SnapKind {
    Endpoint,
    Midpoint,
    Grid,
    Free,
}
pub fn snap(
    project: &Project,
    story: u32,
    p: Point,
    tolerance: f32,
    enabled: bool,
) -> (Point, SnapKind) {
    if !enabled {
        return (p, SnapKind::Free);
    }
    let mut best = (p, SnapKind::Free, tolerance);
    for e in project.elements.iter().filter(|e| e.story == story) {
        let mut pts: Vec<(Point, SnapKind)> = footprint(e)
            .into_iter()
            .map(|p| (p, SnapKind::Endpoint))
            .collect();
        if let Shape::Wall { start, end, .. } = e.shape {
            pts.extend([
                (start, SnapKind::Endpoint),
                (end, SnapKind::Endpoint),
                (
                    Point::new((start.x + end.x) / 2., (start.y + end.y) / 2.),
                    SnapKind::Midpoint,
                ),
            ]);
        }
        for (q, k) in pts {
            let d = p.distance(q);
            if d < best.2 {
                best = (q, k, d)
            }
        }
    }
    if best.1 != SnapKind::Free {
        return (best.0, best.1);
    }
    (
        Point::new((p.x * 10.).round() / 10., (p.y * 10.).round() / 10.),
        SnapKind::Grid,
    )
}
pub fn corners(project: &Project, e: &Element) -> [Vec3; 8] {
    let base = project.elevation(e.story);
    let (bottom, top) = match e.shape {
        Shape::Wall { height, .. } => (base, base + height),
        Shape::Slab { thickness, .. } => (base - thickness, base),
    };
    let p = footprint(e);
    std::array::from_fn(|i| Vec3::new(p[i % 4].x, p[i % 4].y, if i < 4 { bottom } else { top }))
}
pub const FACES: [[usize; 4]; 6] = [
    [0, 3, 2, 1],
    [4, 5, 6, 7],
    [0, 1, 5, 4],
    [1, 2, 6, 5],
    [2, 3, 7, 6],
    [3, 0, 4, 7],
];
#[derive(Clone, Debug)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub span: f32,
    pub target: Vec3,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: -1.05,
            pitch: 0.83,
            span: 21.,
            target: Vec3::new(6., 3., 0.8),
        }
    }
}
impl Camera {
    /// Keep the shorter viewport dimension stable when resizing split panels.
    pub fn vertical_span(&self, aspect: f32) -> f32 {
        self.span / aspect.clamp(0.01, 1.0)
    }
    pub fn matrix(&self, aspect: f32) -> Mat4 {
        let dir = Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
        );
        let span = self.vertical_span(aspect);
        let fov = 45_f32.to_radians();
        let distance = span / (2. * (fov * 0.5).tan());
        glam::camera::rh::proj::directx::perspective(fov, aspect.max(0.01), 0.05, 1000.)
            * glam::camera::rh::view::look_at_mat4(self.target + dir * distance, self.target, Vec3::Z)
    }
    pub fn pick(
        &self,
        project: &Project,
        ndc: [f32; 2],
        aspect: f32,
        story: Option<u32>,
    ) -> Option<u64> {
        let inv = self.matrix(aspect).inverse();
        let origin = inv.project_point3(Vec3::new(ndc[0], ndc[1], 0.));
        let dir = (inv.project_point3(Vec3::new(ndc[0], ndc[1], 1.)) - origin).normalize();
        let mut hit = None;
        let mut nearest = f32::INFINITY;
        for e in project
            .elements
            .iter()
            .filter(|e| story.is_none_or(|s| s == e.story))
        {
            let c = corners(project, e);
            for f in FACES {
                for tri in [[f[0], f[1], f[2]], [f[0], f[2], f[3]]] {
                    if let Some(t) = ray_triangle(origin, dir, c[tri[0]], c[tri[1]], c[tri[2]]) {
                        if t < nearest {
                            nearest = t;
                            hit = Some(e.id)
                        }
                    }
                }
            }
        }
        hit
    }
    pub fn point_on_horizontal_plane(&self, ndc: [f32; 2], aspect: f32, z: f32) -> Option<Point> {
        let inverse = self.matrix(aspect).inverse();
        let near = inverse.project_point3(Vec3::new(ndc[0], ndc[1], 0.));
        // Unprojecting exactly at the far plane (1000 m) loses precision in
        // f32. Any point along the same perspective ray is sufficient; use
        // an interior depth to keep the ground-plane intersection stable.
        let far = inverse.project_point3(Vec3::new(ndc[0], ndc[1], 0.99));
        let direction = far - near;
        if direction.z.abs() < 1e-6 { return None; }
        let t = (z - near.z) / direction.z;
        if t < 0. { return None; }
        let point = near + direction * t;
        point.is_finite().then_some(Point::new(point.x, point.y))
    }
}
fn ray_triangle(o: Vec3, d: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let e1 = b - a;
    let e2 = c - a;
    let h = d.cross(e2);
    let det = e1.dot(h);
    if det.abs() < 1e-7 {
        return None;
    }
    let s = o - a;
    let u = s.dot(h) / det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) / det;
    if v < 0. || u + v > 1. {
        return None;
    }
    let t = e2.dot(q) / det;
    (t >= 0.).then_some(t)
}

#[cfg(test)]
mod marquee_tests {
    use super::*;

    #[test]
    fn polygon_selection_uses_editable_nodes_but_visibility_uses_overlap() {
        let mut project = Project::default();
        let wall = project.add(0, Material::Concrete, Shape::Wall {
            start: Point::new(0., 0.), end: Point::new(10., 0.), thickness: 0.2, height: 3.,
        });
        let area = [Point::new(4., -1.), Point::new(6., -1.), Point::new(6., 1.), Point::new(4., 1.)];
        let e = project.elements.iter().find(|e| e.id == wall).unwrap();
        assert!(!element_has_node_in_polygon(e, &area));
        assert!(element_intersects_polygon(e, &area));
    }

    #[test]
    fn same_material_collinear_walls_have_no_internal_cap_line() {
        let mut project = Project::default();
        project.add(0, Material::Concrete, Shape::Wall {
            start: Point::new(0., 0.), end: Point::new(2., 0.), thickness: 0.2, height: 3.,
        });
        project.add(0, Material::Concrete, Shape::Wall {
            start: Point::new(2., 0.), end: Point::new(4., 0.), thickness: 0.2, height: 3.,
        });
        let walls: Vec<_> = project.elements.iter().collect();
        let footprints = plan_wall_footprints(&walls);
        let outline = wall_outline_segments(&footprints);
        assert!(!outline.iter().any(|(a, b)|
            (a.x - 2.).abs() < 1e-4 && (b.x - 2.).abs() < 1e-4 && (a.y - b.y).abs() > 0.1
        ), "a shared wall end must not be outlined: {outline:?}");
        assert!(outline.iter().any(|(a, b)|
            (a.y - b.y).abs() < 1e-4 && (a.y.abs() - 0.1).abs() < 1e-4
        ));
    }

    #[test]
    fn same_material_t_junction_trims_covered_caps_and_side_edges() {
        let mut project = Project::default();
        project.add(0, Material::Plaster, Shape::Wall {
            start: Point::new(0., 0.), end: Point::new(4., 0.), thickness: 0.2, height: 3.,
        });
        project.add(0, Material::Plaster, Shape::Wall {
            start: Point::new(2., 0.), end: Point::new(2., 2.), thickness: 0.2, height: 3.,
        });
        let walls: Vec<_> = project.elements.iter().collect();
        let footprints = plan_wall_footprints(&walls);
        let outline = wall_outline_segments(&footprints);
        assert!(plan_wall_attached_ends(&walls, &footprints).contains(&(walls[1].id, true)), "the T branch's buried 3D end cap must be hidden");
        assert!(!outline.iter().any(|(a, b)|
            a.y.abs() < 1e-4 && b.y.abs() < 1e-4 && (a.x - b.x).abs() > 0.1
        ), "the branch end cap should be hidden inside the main wall");
        assert!(!outline.iter().any(|(a, b)|
            (a.y - 0.1).abs() < 1e-4 && (b.y - 0.1).abs() < 1e-4
                && a.x.min(b.x) < 2.0 && a.x.max(b.x) > 2.0
        ), "the main wall's exposed side should break around the branch");
    }

    #[test]
    fn same_material_l_corner_auto_trims_to_one_continuous_outline() {
        let mut project = Project::default();
        project.add(0, Material::Plaster, Shape::Wall {
            start: Point::new(0., 0.), end: Point::new(4., 0.), thickness: 0.2, height: 3.,
        });
        project.add(0, Material::Plaster, Shape::Wall {
            // Simulate drawing the second wall from the first wall's outside face.
            start: Point::new(4.08, 0.08), end: Point::new(4.08, 4.), thickness: 0.2, height: 3.,
        });
        let walls: Vec<_> = project.elements.iter().collect();
        let footprints = plan_wall_footprints(&walls);
        let outline = wall_outline_segments(&footprints);
        let paths = wall_outline_paths(&footprints);
        let Shape::Wall { end: joined_end, .. } = walls[0].shape else { unreachable!() };
        let Shape::Wall { start: joined_start, .. } = walls[1].shape else { unreachable!() };
        assert!(footprints.iter().all(|poly| poly.iter().all(|point| point.x.is_finite() && point.y.is_finite())));
        assert_eq!(joined_end, Point::new(4., 0.), "the stored project geometry remains untouched");
        assert_eq!(joined_start, Point::new(4.08, 0.08), "the stored project geometry remains untouched");
        assert!(contains(Point::new(4.03, 0.04), &footprints[0]));
        assert!(contains(Point::new(4.03, 0.04), &footprints[1]));
        assert!(!outline.iter().any(|(a, b)|
            (a.y.abs() < 1e-4 && b.y.abs() < 1e-4 && (a.x - b.x).abs() > 0.1)
                || ((a.x - 4.08).abs() < 1e-4 && (b.x - 4.08).abs() < 1e-4
                    && a.y.min(b.y) < 0.0 && a.y.max(b.y) > 0.0)
        ), "there should be no cap or seam at the auto-joined corner: {outline:?}");
        assert_eq!(paths.len(), 1, "an L corner must render as one continuous outline path: {paths:?}");
        assert!(paths[0][0].distance(*paths[0].last().unwrap()) <= 1e-3,
            "the joined outline must close at its final corner: {paths:?}");
    }

    #[test]
    fn face_aligned_l_corner_snaps_through_full_wall_depth() {
        let mut project = Project::default();
        project.add(0, Material::Plaster, Shape::Wall {
            start: Point::new(7.5, 4.0), end: Point::new(12.0, 4.0), thickness: 0.22, height: 3.0,
        });
        project.add(0, Material::Plaster, Shape::Wall {
            // Typical face-aligned drawing: the return wall starts 0.20 m
            // into the host wall's depth, not exactly on its centerline.
            start: Point::new(7.5, 4.2), end: Point::new(7.5, 8.0), thickness: 0.22, height: 3.0,
        });
        let walls: Vec<_> = project.elements.iter().collect();
        let footprints = plan_wall_footprints(&walls);
        let outline = wall_outline_segments(&footprints);
        let paths = wall_outline_paths(&footprints);
        assert!(contains(Point::new(7.55, 4.05), &footprints[0]));
        assert!(contains(Point::new(7.55, 4.05), &footprints[1]));
        assert!(contains(Point::new(7.45, 3.95), &footprints[0]) || contains(Point::new(7.45, 3.95), &footprints[1]),
            "extended rectangular wall bodies should fill the outside corner without a bevel");
        assert!(plan_wall_attached_ends(&walls, &footprints).contains(&(walls[0].id, true)));
        assert!(plan_wall_attached_ends(&walls, &footprints).contains(&(walls[1].id, true)));
        assert!(!outline.iter().any(|(a, b)|
            (a.x - 7.5).abs() < 1e-4 && (b.x - 7.5).abs() < 1e-4
                && (a.y - b.y).abs() > 0.21
        ), "the face-aligned return must not leave an exposed end cap: {outline:?}");
        assert!(outline.iter().all(|(a, b)|
            (a.x - b.x).abs() < 1e-4 || (a.y - b.y).abs() < 1e-4
        ), "joined wall outlines must meet at orthogonal edge intersections, never diagonal bevels: {outline:?}");
        assert_eq!(paths.len(), 1, "the connected L perimeter should be one path: {paths:?}");
        assert!(paths[0][0].distance(*paths[0].last().unwrap()) <= 1e-3);
    }

    #[test]
    fn rotated_rectangle_tracks_three_points() {
        let polygon = rotated_rectangle_polygon(
            Point::new(0., 0.), Point::new(4., 0.), Point::new(4., 2.),
        ).unwrap();
        assert_eq!(polygon, vec![Point::new(0., 0.), Point::new(4., 0.), Point::new(4., 2.), Point::new(0., 2.)]);
        assert!(rotated_rectangle_polygon(Point::default(), Point::default(), Point::new(0., 1.)).is_none());
    }

    #[test]
    fn perspective_plane_projection_round_trips_and_rejects_above_horizon() {
        let camera = Camera::default();
        let world = Point::new(6., 3.);
        let clip = camera.matrix(4. / 3.).project_point3(Vec3::new(world.x, world.y, 0.));
        let round_trip = camera.point_on_horizontal_plane([clip.x, clip.y], 4. / 3., 0.).unwrap();
        assert!(round_trip.distance(world) < 1e-3);
        let above_horizon = camera.point_on_horizontal_plane([0., 3.], 4. / 3., 0.);
        assert!(above_horizon.is_none(), "above-horizon ray returned {above_horizon:?}");
    }

    #[test]
    fn concave_marquee_clipping_covers_exact_intersection_without_spill() {
        let concave = [
            Point::new(0.,0.), Point::new(2.,0.), Point::new(2.,1.),
            Point::new(1.,1.), Point::new(1.,2.), Point::new(0.,2.),
        ];
        let subject = [Point::new(-1.,-1.), Point::new(3.,-1.), Point::new(3.,3.), Point::new(-1.,3.)];
        let fragments = intersect_footprint_with_polygon(&subject,&concave);
        let area: f32 = fragments.iter().map(|p| polygon_area(p).abs()).sum();
        assert!((area-3.).abs()<1e-4, "clipped area was {area}");
        assert!(fragments.iter().flatten().all(|p| contains(*p,&concave)));
    }

    #[test]
    fn camera_projection_is_perspective() {
        let camera=Camera::default();
        let matrix=camera.matrix(1.5);
        let dir=Vec3::new(camera.yaw.cos()*camera.pitch.cos(),camera.yaw.sin()*camera.pitch.cos(),camera.pitch.sin());
        let camera_distance=camera.vertical_span(1.5)/(2.*(45_f32.to_radians()*0.5).tan());
        let distant=camera.target+dir*(camera_distance*0.2);
        let close=camera.target+dir*(camera_distance*0.7);
        let side=Vec3::new(-camera.yaw.sin(),camera.yaw.cos(),0.)*2.;
        let distant_width=matrix.project_point3(distant+side).distance(matrix.project_point3(distant-side));
        let close_width=matrix.project_point3(close+side).distance(matrix.project_point3(close-side));
        assert!(close_width>distant_width*1.8, "close={close_width}, distant={distant_width}");
    }
}
