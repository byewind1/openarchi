use crate::model::*;
use glam::{Mat4, Vec3};

/// Reference-line anchors of a wall. `reference_offset` moves the wall body
/// around this line; it never moves the line itself.
pub fn wall_reference_line(e: &Element) -> Option<(Point, Point)> {
    match e.shape {
        Shape::Wall { start, end, .. } => Some((start, end)),
        Shape::Slab { .. } | Shape::Stair { .. } | Shape::Polyline { .. } => None,
    }
}

pub fn footprint(e: &Element) -> [Point; 4] {
    match e.shape {
        Shape::Wall {
            start: a,
            end: b,
            thickness: t,
            ..
        } => wall_footprint(
            a,
            b,
            t,
            e.wall_settings
                .as_ref()
                .map_or(0., |settings| settings.body_offset(t)),
        ),
        Shape::Slab { min: a, max: b, .. } => [a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)],
        Shape::Stair { ref spec } => crate::stairs::generate(spec, 0.0)
            .map(|geometry| geometry.footprint)
            .unwrap_or([spec.start; 4]),
        Shape::Polyline { ref points, .. } => {
            let min = points
                .iter()
                .fold(Point::new(f32::INFINITY, f32::INFINITY), |a, p| {
                    Point::new(a.x.min(p.x), a.y.min(p.y))
                });
            let max = points
                .iter()
                .fold(Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY), |a, p| {
                    Point::new(a.x.max(p.x), a.y.max(p.y))
                });
            [min, Point::new(max.x, min.y), max, Point::new(min.x, max.y)]
        }
    }
}

fn wall_footprint(a: Point, b: Point, thickness: f32, reference_offset: f32) -> [Point; 4] {
    let length = a.distance(b).max(0.001);
    let unit = Point::new(-(b.y - a.y) / length, (b.x - a.x) / length);
    let shift = Point::new(unit.x * reference_offset, unit.y * reference_offset);
    let a = Point::new(a.x + shift.x, a.y + shift.y);
    let b = Point::new(b.x + shift.x, b.y + shift.y);
    let normal = Point::new(unit.x * thickness / 2., unit.y * thickness / 2.);
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

/// Return a constant-distance offset of an open or explicitly closed path.
/// Positive distances move each directed segment to its left.
pub fn offset_polyline(points: &[Point], distance: f32) -> Option<Vec<Point>> {
    if points.len() < 2 || !distance.is_finite() || distance.abs() < 1e-6 {
        return None;
    }
    let closed = points.len() > 3 && points[0].distance(*points.last()?) < 0.01;
    let count = if closed {
        points.len() - 1
    } else {
        points.len()
    };
    let edge_count = if closed { count } else { count - 1 };
    let mut normals = Vec::with_capacity(edge_count);
    for index in 0..edge_count {
        let a = points[index];
        let b = points[(index + 1) % count];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let length = dx.hypot(dy);
        if length < 1e-6 {
            return None;
        }
        normals.push(Point::new(-dy / length, dx / length));
    }
    let shifted_line = |edge: usize| {
        let normal = normals[edge];
        let a = points[edge];
        let b = points[(edge + 1) % count];
        (
            Point::new(a.x + normal.x * distance, a.y + normal.y * distance),
            Point::new(b.x + normal.x * distance, b.y + normal.y * distance),
        )
    };
    let mut result = Vec::with_capacity(points.len());
    for vertex in 0..count {
        if !closed && vertex == 0 {
            let normal = normals[0];
            result.push(Point::new(
                points[0].x + normal.x * distance,
                points[0].y + normal.y * distance,
            ));
            continue;
        }
        if !closed && vertex + 1 == count {
            let normal = *normals.last()?;
            result.push(Point::new(
                points[vertex].x + normal.x * distance,
                points[vertex].y + normal.y * distance,
            ));
            continue;
        }
        let previous = if vertex == 0 {
            edge_count - 1
        } else {
            vertex - 1
        };
        let next = vertex % edge_count;
        let (pa, pb) = shifted_line(previous);
        let (na, nb) = shifted_line(next);
        let intersection = line_intersection(pa, pb, na, nb);
        let joined = if let Some(point) = intersection {
            point
        } else {
            let n0 = normals[previous];
            let n1 = normals[next];
            let sum = Point::new(n0.x + n1.x, n0.y + n1.y);
            let length = sum.x.hypot(sum.y);
            if length < 1e-6 {
                return None;
            }
            Point::new(
                points[vertex].x + sum.x / length * distance,
                points[vertex].y + sum.y / length * distance,
            )
        };
        result.push(joined);
    }
    let result_edges = if closed { count } else { count - 1 };
    for edge in 0..result_edges {
        let source_delta = Point::new(
            points[(edge + 1) % count].x - points[edge].x,
            points[(edge + 1) % count].y - points[edge].y,
        );
        let result_delta = Point::new(
            result[(edge + 1) % count].x - result[edge].x,
            result[(edge + 1) % count].y - result[edge].y,
        );
        if source_delta.x * result_delta.x + source_delta.y * result_delta.y <= 1e-6 {
            return None;
        }
    }
    for first in 0..result_edges {
        for second in first + 1..result_edges {
            if second == first + 1 || (closed && first == 0 && second + 1 == result_edges) {
                continue;
            }
            if segments_intersect(
                result[first],
                result[(first + 1) % count],
                result[second],
                result[(second + 1) % count],
            ) {
                return None;
            }
        }
    }
    if closed {
        let signed_area = |path: &[Point]| {
            (0..path.len())
                .map(|i| {
                    let a = path[i];
                    let b = path[(i + 1) % path.len()];
                    a.x * b.y - b.x * a.y
                })
                .sum::<f32>()
        };
        if signed_area(&points[..count]) * signed_area(&result) <= 1e-6 {
            return None;
        }
        result.push(result[0]);
    }
    Some(result)
}

/// Offset exact line/arc segments from their current geometry, without replacing
/// arcs by chords. Adjacent supports are trimmed at their nearest intersection.
pub fn offset_polyline_with_arcs(
    points: &[Point],
    arcs: &[Option<ArcSegment>],
    distance: f32,
) -> Option<(Vec<Point>, Vec<Option<ArcSegment>>)> {
    if !arcs.iter().any(Option::is_some) {
        return Some((offset_polyline(points, distance)?, arcs.to_vec()));
    }
    if points.len() < 2 || !distance.is_finite() {
        return None;
    }
    if distance.abs() < 1e-6 {
        return Some((points.to_vec(), arcs.to_vec()));
    }
    let count = points.len() - 1;
    if arcs.len() != count {
        return None;
    }
    let closed = points[0].distance(points[count]) < 1e-5;
    // Each support stores its shifted endpoints and optional exact circle.
    let mut supports = Vec::with_capacity(count);
    for i in 0..count {
        let a = points[i];
        let b = points[i + 1];
        let support = if let Some(arc) = arcs[i] {
            let (center, _) = arc_center_sweep(a, b, arc)?;
            let radius = arc.radius + if arc.clockwise { distance } else { -distance };
            if radius < 0.01 {
                return None;
            }
            let shift = |p: Point| {
                Point::new(
                    center.x + (p.x - center.x) * radius / arc.radius,
                    center.y + (p.y - center.y) * radius / arc.radius,
                )
            };
            (shift(a), shift(b), Some((center, radius)))
        } else {
            let length = a.distance(b);
            if length < 0.01 {
                return None;
            }
            let nx = -(b.y - a.y) / length * distance;
            let ny = (b.x - a.x) / length * distance;
            (
                Point::new(a.x + nx, a.y + ny),
                Point::new(b.x + nx, b.y + ny),
                None,
            )
        };
        supports.push(support);
    }
    fn line_circle(a: Point, b: Point, c: Point, r: f32) -> Vec<Point> {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let length2 = dx * dx + dy * dy;
        let t = ((c.x - a.x) * dx + (c.y - a.y) * dy) / length2;
        let foot = Point::new(a.x + t * dx, a.y + t * dy);
        let height2 = r * r - foot.distance(c).powi(2);
        if height2 < -1e-4 {
            return vec![];
        }
        let dt = (height2.max(0.) / length2).sqrt();
        vec![
            Point::new(foot.x + dt * dx, foot.y + dt * dy),
            Point::new(foot.x - dt * dx, foot.y - dt * dy),
        ]
    }
    let mut result = Vec::with_capacity(points.len());
    for vertex in 0..=count {
        if vertex == count && closed {
            result.push(result[0]);
            break;
        }
        if vertex == 0 && !closed {
            result.push(supports[0].0);
            continue;
        }
        if vertex == count {
            result.push(supports[count - 1].1);
            continue;
        }
        let prev = supports[if vertex == 0 { count - 1 } else { vertex - 1 }];
        let next = supports[vertex];
        let expected = Point::new((prev.1.x + next.0.x) * 0.5, (prev.1.y + next.0.y) * 0.5);
        let candidates = if prev.1.distance(next.0) < 1e-4 {
            vec![expected]
        } else {
            match (prev.2, next.2) {
                (None, None) => vec![line_intersection(prev.0, prev.1, next.0, next.1)?],
                (None, Some((c, r))) => line_circle(prev.0, prev.1, c, r),
                (Some((c, r)), None) => line_circle(next.0, next.1, c, r),
                (Some((c, r)), Some((d, s))) => {
                    let length = c.distance(d);
                    if length < 1e-6 || length > r + s + 1e-4 || length < (r - s).abs() - 1e-4 {
                        return None;
                    }
                    let along = (r * r - s * s + length * length) / (2. * length);
                    let height2 = r * r - along * along;
                    if height2 < -1e-4 {
                        return None;
                    }
                    let nx = (d.x - c.x) / length;
                    let ny = (d.y - c.y) / length;
                    let foot = Point::new(c.x + along * nx, c.y + along * ny);
                    let h = height2.max(0.).sqrt();
                    vec![
                        Point::new(foot.x - h * ny, foot.y + h * nx),
                        Point::new(foot.x + h * ny, foot.y - h * nx),
                    ]
                }
            }
        };
        result.push(
            candidates
                .into_iter()
                .filter(|p| p.x.is_finite() && p.y.is_finite())
                .min_by(|a, b| a.distance(expected).total_cmp(&b.distance(expected)))?,
        );
    }
    let mut result_arcs = arcs.to_vec();
    for i in 0..count {
        let a = result[i];
        let b = result[i + 1];
        if a.distance(b) < 0.01 {
            return None;
        }
        if let Some(mut arc) = arcs[i] {
            let (center, radius) = supports[i].2?;
            let a0 = (a.y - center.y).atan2(a.x - center.x);
            let a1 = (b.y - center.y).atan2(b.x - center.x);
            let sweep = if arc.clockwise {
                (a0 - a1).rem_euclid(std::f32::consts::TAU)
            } else {
                (a1 - a0).rem_euclid(std::f32::consts::TAU)
            };
            let (_, old_sweep) = arc_center_sweep(points[i], points[i + 1], arc)?;
            // Do not jump to a complementary loop when an offset consumes a segment.
            if (sweep - old_sweep.abs()).abs() > std::f32::consts::PI {
                return None;
            }
            arc.radius = radius;
            arc.large_arc = sweep > std::f32::consts::PI;
            let (reconstructed, _) = arc_center_sweep(a, b, arc)?;
            if reconstructed.distance(center) > 1e-3 {
                return None;
            }
            result_arcs[i] = Some(arc);
        } else if (b.x - a.x) * (points[i + 1].x - points[i].x)
            + (b.y - a.y) * (points[i + 1].y - points[i].y)
            <= 1e-6
        {
            return None;
        }
    }
    // Reject self-crossings without storing tessellation as editable nodes.
    let path = polyline_path(&result, &result_arcs);
    for i in 0..path.len() - 1 {
        for j in i + 2..path.len() - 1 {
            if closed && i == 0 && j == path.len() - 2 {
                continue;
            }
            if segments_intersect(path[i], path[i + 1], path[j], path[j + 1]) {
                return None;
            }
        }
    }
    Some((result, result_arcs))
}

fn line_intersection(a: Point, b: Point, c: Point, d: Point) -> Option<Point> {
    let (rx, ry) = (b.x - a.x, b.y - a.y);
    let (sx, sy) = (d.x - c.x, d.y - c.y);
    let denominator = rx * sy - ry * sx;
    if denominator.abs() < 1e-6 {
        return None;
    }
    let t = ((c.x - a.x) * sy - (c.y - a.y) * sx) / denominator;
    Some(Point::new(a.x + t * rx, a.y + t * ry))
}

/// Fit exact circular-arc metadata through both endpoints and a dragged point.
pub fn arc_segment_through(a: Point, b: Point, through: Point) -> Option<ArcSegment> {
    let determinant =
        2. * (a.x * (b.y - through.y) + b.x * (through.y - a.y) + through.x * (a.y - b.y));
    if determinant.abs() < 1e-5 {
        return None;
    }
    let aa = a.x * a.x + a.y * a.y;
    let bb = b.x * b.x + b.y * b.y;
    let cc = through.x * through.x + through.y * through.y;
    let center = Point::new(
        (aa * (b.y - through.y) + bb * (through.y - a.y) + cc * (a.y - b.y)) / determinant,
        (aa * (through.x - b.x) + bb * (a.x - through.x) + cc * (b.x - a.x)) / determinant,
    );
    let radius = center.distance(a);
    if !radius.is_finite() || radius < 0.005 {
        return None;
    }
    let start = (a.y - center.y).atan2(a.x - center.x);
    let finish = (b.y - center.y).atan2(b.x - center.x);
    let middle = (through.y - center.y).atan2(through.x - center.x);
    let positive = (finish - start).rem_euclid(std::f32::consts::TAU);
    let middle_positive = (middle - start).rem_euclid(std::f32::consts::TAU);
    let sweep = if middle_positive <= positive + 1e-5 {
        positive
    } else {
        positive - std::f32::consts::TAU
    };
    if sweep.abs() < 1e-4 {
        return None;
    }
    Some(ArcSegment {
        radius,
        clockwise: sweep < 0.,
        large_arc: sweep.abs() > std::f32::consts::PI,
    })
}

/// Reconstruct the center and signed sweep from endpoints plus exact arc data.
pub fn arc_center_sweep(a: Point, b: Point, arc: ArcSegment) -> Option<(Point, f32)> {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let chord = dx.hypot(dy);
    if !arc.radius.is_finite() || arc.radius <= 0. || chord < 1e-6 || chord > 2. * arc.radius + 1e-4
    {
        return None;
    }
    let half = chord * 0.5;
    let height = (arc.radius * arc.radius - half * half).max(0.).sqrt();
    let midpoint = Point::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
    let normal = Point::new(-dy / chord, dx / chord);
    for sign in [-1., 1.] {
        let center = Point::new(
            midpoint.x + normal.x * height * sign,
            midpoint.y + normal.y * height * sign,
        );
        let a0 = (a.y - center.y).atan2(a.x - center.x);
        let a1 = (b.y - center.y).atan2(b.x - center.x);
        let ccw = (a1 - a0).rem_euclid(std::f32::consts::TAU);
        let sweep = if arc.clockwise {
            ccw - std::f32::consts::TAU
        } else {
            ccw
        };
        if (sweep.abs() > std::f32::consts::PI) == arc.large_arc {
            return Some((center, sweep));
        }
    }
    None
}

/// View-only tessellation for a single exact arc segment.
pub fn arc_points(a: Point, b: Point, arc: ArcSegment) -> Option<Vec<Point>> {
    let (center, sweep) = arc_center_sweep(a, b, arc)?;
    let start = (a.y - center.y).atan2(a.x - center.x);
    let steps = ((sweep.abs() / (std::f32::consts::PI / 36.)).ceil() as usize).max(2);
    Some(
        (0..=steps)
            .map(|index| {
                if index == 0 {
                    return a;
                }
                if index == steps {
                    return b;
                }
                let angle = start + sweep * index as f32 / steps as f32;
                Point::new(
                    center.x + arc.radius * angle.cos(),
                    center.y + arc.radius * angle.sin(),
                )
            })
            .collect(),
    )
}

pub fn arc_length(a: Point, b: Point, arc: ArcSegment) -> f32 {
    arc_center_sweep(a, b, arc).map_or_else(|| a.distance(b), |(_, sweep)| arc.radius * sweep.abs())
}

/// Expand segment curves only for rendering and geometric hit calculations.
pub fn polyline_path(points: &[Point], arcs: &[Option<ArcSegment>]) -> Vec<Point> {
    let mut path = Vec::new();
    for (index, pair) in points.windows(2).enumerate() {
        let segment = arcs
            .get(index)
            .and_then(|arc| *arc)
            .and_then(|arc| arc_points(pair[0], pair[1], arc))
            .unwrap_or_else(|| vec![pair[0], pair[1]]);
        if path.is_empty() {
            path.extend(segment);
        } else {
            path.extend(segment.into_iter().skip(1));
        }
    }
    path
}

/// Compatibility helper for older callers; returned points are display-only.
pub fn circular_arc_through(a: Point, b: Point, through: Point) -> Option<Vec<Point>> {
    arc_points(a, b, arc_segment_through(a, b, through)?)
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
    if len < 1e-4 {
        return None;
    }
    let n = Point::new(-dy / len, dx / len);
    let depth = (side.x - b.x) * n.x + (side.y - b.y) * n.y;
    if depth.abs() < 1e-4 {
        return None;
    }
    let offset = Point::new(n.x * depth, n.y * depth);
    Some(vec![
        a,
        b,
        Point::new(b.x + offset.x, b.y + offset.y),
        Point::new(a.x + offset.x, a.y + offset.y),
    ])
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
    if ab_c.abs() <= eps && segment_distance(c, a, b) <= eps {
        return true;
    }
    if ab_d.abs() <= eps && segment_distance(d, a, b) <= eps {
        return true;
    }
    if cd_a.abs() <= eps && segment_distance(a, c, d) <= eps {
        return true;
    }
    if cd_b.abs() <= eps && segment_distance(b, c, d) <= eps {
        return true;
    }
    (ab_c > 0.) != (ab_d > 0.) && (cd_a > 0.) != (cd_b > 0.)
}

/// Archicad-style editing selection: an element is included when at least one
/// of its editable plan nodes lies in the marquee. The whole element is then
/// operated on by group commands.
pub fn element_has_node_in_polygon(e: &Element, polygon: &[Point]) -> bool {
    let nodes: Vec<Point> = match e.shape {
        Shape::Wall { start, end, .. } => vec![start, end],
        Shape::Slab { .. } => footprint(e).into_iter().collect(),
        Shape::Stair { .. } => footprint(e).into_iter().collect(),
        Shape::Polyline { ref points, .. } => points.clone(),
    };
    nodes.into_iter().any(|p| contains(p, polygon))
}

/// Visibility selection for 3D uses geometric overlap, so a wall segment
/// crossing the marquee is included even when neither endpoint is inside.
pub fn element_intersects_polygon(e: &Element, polygon: &[Point]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    if let Shape::Polyline {
        ref points,
        ref arcs,
    } = e.shape
    {
        return points.iter().any(|point| contains(*point, polygon))
            || polyline_path(points, arcs).windows(2).any(|pair| {
                (0..polygon.len()).any(|index| {
                    segments_intersect(
                        pair[0],
                        pair[1],
                        polygon[index],
                        polygon[(index + 1) % polygon.len()],
                    )
                })
            });
    }
    let shape = footprint(e);
    if shape.iter().any(|p| contains(*p, polygon)) || polygon.iter().any(|p| contains(*p, &shape)) {
        return true;
    }
    (0..shape.len()).any(|i| {
        let a = shape[i];
        let b = shape[(i + 1) % shape.len()];
        (0..polygon.len())
            .any(|j| segments_intersect(a, b, polygon[j], polygon[(j + 1) % polygon.len()]))
    })
}

pub fn element_fully_inside_polygon(e: &Element, polygon: &[Point]) -> bool {
    if let Shape::Polyline { ref points, .. } = e.shape {
        return points.iter().all(|point| contains(*point, polygon));
    }
    footprint(e).iter().all(|p| contains(*p, polygon))
}

pub fn element_overlaps_z_range(project: &Project, e: &Element, range: (f32, f32)) -> bool {
    let corners = corners(project, e);
    let lo = corners.iter().map(|p| p.z).fold(f32::INFINITY, f32::min);
    let hi = corners
        .iter()
        .map(|p| p.z)
        .fold(f32::NEG_INFINITY, f32::max);
    hi >= range.0 && lo <= range.1
}

pub fn element_fully_inside_volume(
    project: &Project,
    e: &Element,
    polygon: &[Point],
    range: Option<(f32, f32)>,
) -> bool {
    if !element_fully_inside_polygon(e, polygon) {
        return false;
    }
    let Some((bottom, top)) = range else {
        return true;
    };
    let corners = corners(project, e);
    corners.iter().all(|p| p.z >= bottom && p.z <= top)
}

/// Triangulate a simple polygon with ear clipping. Marquee polygons may be
/// concave, so clipping each edge as though the whole area were convex is not
/// sufficient.
pub fn triangulate_polygon(poly: &[Point]) -> Vec<[Point; 3]> {
    if poly.len() < 3 {
        return Vec::new();
    }
    let area: f32 = (0..poly.len())
        .map(|i| {
            let a = poly[i];
            let b = poly[(i + 1) % poly.len()];
            a.x * b.y - b.x * a.y
        })
        .sum();
    if area.abs() < 1e-6 {
        return Vec::new();
    }
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
            if if ccw { turn <= 1e-6 } else { turn >= -1e-6 } {
                continue;
            }
            let inside = indices
                .iter()
                .copied()
                .filter(|&j| j != ia && j != ib && j != ic)
                .any(|j| point_in_triangle(poly[j], poly[ia], poly[ib], poly[ic]));
            if !inside {
                ear = Some((i, [poly[ia], poly[ib], poly[ic]]));
                break;
            }
        }
        let Some((i, triangle)) = ear else {
            return Vec::new();
        };
        out.push(triangle);
        indices.remove(i);
    }
    if indices.len() == 3 {
        out.push([poly[indices[0]], poly[indices[1]], poly[indices[2]]]);
    }
    out
}

fn point_in_triangle(p: Point, a: Point, b: Point, c: Point) -> bool {
    let x = orientation(a, b, p);
    let y = orientation(b, c, p);
    let z = orientation(c, a, p);
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
            let a = tri[edge];
            let b = tri[(edge + 1) % 3];
            let input = std::mem::take(&mut output);
            if input.is_empty() {
                break;
            }
            let signed = |p: Point| orientation(a, b, p) * area.signum();
            let mut prev = *input.last().unwrap();
            let mut prev_in = signed(prev) >= -1e-5;
            for curr in input {
                let curr_in = signed(curr) >= -1e-5;
                if curr_in != prev_in {
                    let d = signed(prev) - signed(curr);
                    if d.abs() > 1e-8 {
                        let t = (signed(prev) / d).clamp(0., 1.);
                        output.push(Point::new(
                            prev.x + (curr.x - prev.x) * t,
                            prev.y + (curr.y - prev.y) * t,
                        ));
                    }
                }
                if curr_in {
                    output.push(curr);
                }
                prev = curr;
                prev_in = curr_in;
            }
        }
        if output.len() >= 3 && polygon_area(&output).abs() > 1e-6 {
            fragments.push(output);
        }
    }
    fragments
}

fn polygon_area(poly: &[Point]) -> f32 {
    (0..poly.len())
        .map(|i| {
            let a = poly[i];
            let b = poly[(i + 1) % poly.len()];
            a.x * b.y - b.x * a.y
        })
        .sum::<f32>()
        * 0.5
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
            Shape::Slab { .. } | Shape::Stair { .. } | Shape::Polyline { .. } => {
                unreachable!("plan wall join groups contain walls only")
            }
        })
        .collect();
    let mut endpoint_extensions = vec![[0.0_f32; 2]; walls.len()];

    for (index, wall) in walls.iter().enumerate() {
        let own_thickness = wall.thickness();
        for endpoint_index in 0..2 {
            let endpoint = if endpoint_index == 0 {
                centerlines[index].0
            } else {
                centerlines[index].1
            };
            let mut nearest: Option<(f32, Point, f32)> = None;
            for (other_index, other) in walls.iter().enumerate() {
                if index == other_index {
                    continue;
                }
                let (other_start, other_end, other_thickness) = match other.shape {
                    Shape::Wall {
                        start,
                        end,
                        thickness,
                        ..
                    } => (start, end, thickness),
                    Shape::Slab { .. } | Shape::Stair { .. } | Shape::Polyline { .. } => continue,
                };
                let own_start = match wall.shape {
                    Shape::Wall { start, .. } => start,
                    _ => unreachable!(),
                };
                let own_end = match wall.shape {
                    Shape::Wall { end, .. } => end,
                    _ => unreachable!(),
                };
                let own_direction = Point::new(own_end.x - own_start.x, own_end.y - own_start.y);
                let other_direction =
                    Point::new(other_end.x - other_start.x, other_end.y - other_start.y);
                let denominator =
                    own_direction.x * other_direction.y - own_direction.y * other_direction.x;
                if denominator.abs() < 1e-5 {
                    continue;
                }
                let delta = Point::new(other_start.x - own_start.x, other_start.y - own_start.y);
                let own_t =
                    (delta.x * other_direction.y - delta.y * other_direction.x) / denominator;
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
                if other_t < -endpoint_extension || other_t > 1.0 + endpoint_extension {
                    continue;
                }
                let distance = endpoint.distance(candidate);
                if distance <= tolerance && nearest.is_none_or(|(best, _, _)| distance < best) {
                    nearest = Some((distance, candidate, other_thickness));
                }
            }
            if let Some((_, joined, other_thickness)) = nearest {
                if endpoint_index == 0 {
                    centerlines[index].0 = joined;
                } else {
                    centerlines[index].1 = joined;
                }
                endpoint_extensions[index][endpoint_index] = other_thickness * 0.5;
            }
        }
    }

    walls
        .iter()
        .enumerate()
        .map(|(index, wall)| {
            let (mut start, mut end) = centerlines[index];
            let length = start.distance(end).max(0.001);
            let direction = Point::new((end.x - start.x) / length, (end.y - start.y) / length);
            start.x -= direction.x * endpoint_extensions[index][0];
            start.y -= direction.y * endpoint_extensions[index][0];
            end.x += direction.x * endpoint_extensions[index][1];
            end.y += direction.y * endpoint_extensions[index][1];
            wall_footprint(
                start,
                end,
                wall.thickness(),
                wall.wall_settings
                    .as_ref()
                    .map_or(0., |settings| settings.body_offset(wall.thickness())),
            )
        })
        .collect()
}

/// Returns endpoints whose extended wall footprints reach another wall. The
/// renderer uses these to hide overlapping end faces in the 3D display.
pub fn plan_wall_attached_ends(walls: &[&Element], footprints: &[[Point; 4]]) -> Vec<(u64, bool)> {
    let mut attached = Vec::new();
    for (index, wall) in walls.iter().enumerate() {
        for (endpoint, at_start) in [
            (
                Point::new(
                    (footprints[index][0].x + footprints[index][3].x) * 0.5,
                    (footprints[index][0].y + footprints[index][3].y) * 0.5,
                ),
                true,
            ),
            (
                Point::new(
                    (footprints[index][1].x + footprints[index][2].x) * 0.5,
                    (footprints[index][1].y + footprints[index][2].y) * 0.5,
                ),
                false,
            ),
        ] {
            let joined = walls.iter().enumerate().any(|(other_index, other)| {
                if other.id == wall.id
                    || other.material != wall.material
                    || other.story != wall.story
                {
                    return false;
                }
                let Shape::Wall { height, .. } = wall.shape else {
                    return false;
                };
                let Shape::Wall {
                    height: other_height,
                    ..
                } = other.shape
                else {
                    return false;
                };
                (height - other_height).abs() <= 1e-4
                    && contains(endpoint, &footprints[other_index])
            });
            if joined {
                attached.push((wall.id, at_start));
            }
        }
    }
    attached
}

/// Returns only the exposed perimeter fragments of already-joined wall
/// footprints. This removes internal cap and side strokes in plan.
pub fn wall_outline_segments(wall_footprints: &[[Point; 4]]) -> Vec<(Point, Point)> {
    let polygons: Vec<Vec<Point>> = wall_footprints
        .iter()
        .map(|polygon| polygon.to_vec())
        .collect();
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
                if wall_index == other_index {
                    continue;
                }
                if let Some(interval) = segment_inside_convex_polygon(a, b, other) {
                    if interval.1 - interval.0 <= 1e-5 {
                        continue;
                    }
                    let midpoint = interpolate(a, b, (interval.0 + interval.1) * 0.5);
                    let lies_on_other_boundary = (0..other.len()).any(|edge| {
                        segment_distance(midpoint, other[edge], other[(edge + 1) % other.len()])
                            <= 1e-5
                    });
                    let other_center = other.iter().fold(Point::default(), |sum, point| {
                        Point::new(
                            sum.x + point.x / other.len() as f32,
                            sum.y + point.y / other.len() as f32,
                        )
                    });
                    let same_interior_side =
                        orientation(a, b, other_center).signum() == polygon_area(polygon).signum();
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
                if cursor >= 1.0 {
                    break;
                }
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
                a.distance(tail) <= 1e-3
                    || b.distance(tail) <= 1e-3
                    || a.distance(head) <= 1e-3
                    || b.distance(head) <= 1e-3
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
    if polygon.len() < 3 {
        return None;
    }
    let winding = if polygon_area(polygon) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let mut lo = 0.0_f32;
    let mut hi = 1.0_f32;
    for index in 0..polygon.len() {
        let c = polygon[index];
        let d = polygon[(index + 1) % polygon.len()];
        let at_start = winding * orientation(c, d, a);
        let at_end = winding * orientation(c, d, b);
        let start_inside = at_start >= -1e-5;
        let end_inside = at_end >= -1e-5;
        if !start_inside && !end_inside {
            return None;
        }
        if start_inside != end_inside {
            let crossing = (at_start / (at_start - at_end)).clamp(0.0, 1.0);
            if !start_inside {
                lo = lo.max(crossing);
            } else {
                hi = hi.min(crossing);
            }
        }
        if lo >= hi {
            return None;
        }
    }
    Some((lo, hi))
}

pub fn point_on_segment(p: Point, a: Point, b: Point, tolerance: f32) -> bool {
    segment_distance(p, a, b) <= tolerance
        && p.x >= a.x.min(b.x) - tolerance
        && p.x <= a.x.max(b.x) + tolerance
        && p.y >= a.y.min(b.y) - tolerance
        && p.y <= a.y.max(b.y) + tolerance
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
            project.elements.iter().rev().find(|e| {
                if e.story != story {
                    return false;
                }
                if let Shape::Polyline {
                    ref points,
                    ref arcs,
                } = e.shape
                {
                    polyline_path(points, arcs)
                        .windows(2)
                        .any(|pair| segment_distance(p, pair[0], pair[1]) <= tolerance)
                } else {
                    false
                }
            })
        })
        .or_else(|| {
            project.elements.iter().rev().find(|e| {
                e.story == story
                    && !matches!(e.shape, Shape::Polyline { .. })
                    && contains(p, &footprint(e))
            })
        })
        .map(|e| e.id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanHoverKind {
    Body,
    Node,
    Edge,
}

/// Classify the pointer target for Select-tool cursor feedback.
/// Editable nodes take priority over edges, which take priority over element bodies.
pub fn plan_hover_kind(
    project: &Project,
    story: u32,
    p: Point,
    node_tolerance: f32,
    edge_tolerance: f32,
) -> Option<PlanHoverKind> {
    let elements: Vec<_> = project
        .elements
        .iter()
        .filter(|element| element.story == story)
        .collect();

    let mut nearest_node = f32::INFINITY;
    for element in &elements {
        let nodes: Vec<Point> = match element.shape {
            Shape::Wall { start, end, .. } => vec![start, end],
            Shape::Slab { .. } => footprint(element).into_iter().collect(),
            Shape::Stair { .. } => Vec::new(),
            Shape::Polyline { ref points, .. } => points.clone(),
        };
        for node in nodes {
            nearest_node = nearest_node.min(p.distance(node));
        }
    }
    if nearest_node <= node_tolerance {
        return Some(PlanHoverKind::Node);
    }

    let mut nearest_edge = f32::INFINITY;
    for element in &elements {
        match element.shape {
            Shape::Wall { start, end, .. } => {
                nearest_edge = nearest_edge.min(segment_distance(p, start, end));
                let corners = footprint(element);
                for index in 0..corners.len() {
                    nearest_edge = nearest_edge.min(segment_distance(
                        p,
                        corners[index],
                        corners[(index + 1) % corners.len()],
                    ));
                }
            }
            Shape::Slab { .. } | Shape::Stair { .. } => {
                let corners = footprint(element);
                for index in 0..corners.len() {
                    nearest_edge = nearest_edge.min(segment_distance(
                        p,
                        corners[index],
                        corners[(index + 1) % corners.len()],
                    ));
                }
            }
            Shape::Polyline {
                ref points,
                ref arcs,
            } => {
                for pair in polyline_path(points, arcs).windows(2) {
                    nearest_edge = nearest_edge.min(segment_distance(p, pair[0], pair[1]));
                }
            }
        }
    }
    if nearest_edge <= edge_tolerance {
        return Some(PlanHoverKind::Edge);
    }

    hit_plan(project, story, p, edge_tolerance).map(|_| PlanHoverKind::Body)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SnapKind {
    SnapPoint,
    Guide,
    Endpoint,
    Midpoint,
    Grid,
    Angle,
    Perpendicular,
    Alignment,
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
        let mut pts: Vec<(Point, SnapKind)> = match &e.shape {
            Shape::Polyline { points, .. } => points
                .iter()
                .copied()
                .map(|point| (point, SnapKind::Endpoint))
                .collect(),
            _ => footprint(e)
                .into_iter()
                .map(|point| (point, SnapKind::Endpoint))
                .collect(),
        };
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
        if let Shape::Polyline { ref points, .. } = e.shape {
            pts.extend(
                points
                    .iter()
                    .copied()
                    .map(|point| (point, SnapKind::Endpoint)),
            );
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

/// Resolve a polyline's live next point against its existing chain and nearby
/// geometry. Constraint candidates are accepted only within the screen-derived
/// tolerance; ordinary endpoint/midpoint snapping keeps priority.
pub fn polyline_snap_target(
    project: &Project,
    story: u32,
    chain: &[Point],
    p: Point,
    tolerance: f32,
    enabled: bool,
) -> (Point, SnapKind) {
    let (base, base_kind) = snap(project, story, p, tolerance, enabled);
    let Some(anchor) = chain.last().copied() else {
        return (base, base_kind);
    };
    if !enabled {
        return (base, base_kind);
    }

    let chain_snap = chain
        .iter()
        .copied()
        .filter(|point| point.distance(anchor) >= 0.01)
        .map(|point| (p.distance(point), point))
        .filter(|(distance, _)| *distance <= tolerance)
        .min_by(|left, right| left.0.total_cmp(&right.0));
    if let Some((distance, point)) = chain_snap {
        if !matches!(base_kind, SnapKind::Endpoint | SnapKind::Midpoint)
            || p.distance(base) > distance
        {
            return (point, SnapKind::Endpoint);
        }
    }
    if matches!(base_kind, SnapKind::Endpoint | SnapKind::Midpoint) {
        return (base, base_kind);
    }

    let mut best: Option<(f32, Point, SnapKind)> = None;
    let mut consider = |candidate: Point, kind: SnapKind| {
        let error = p.distance(candidate);
        if candidate.distance(anchor) >= 0.01
            && error <= tolerance
            && best
                .as_ref()
                .is_none_or(|(best_error, _, _)| error < *best_error)
        {
            best = Some((error, candidate, kind));
        }
    };

    // Horizontal/vertical alignments also use nodes in the active chain.
    for point in chain.iter().copied() {
        consider(Point::new(p.x, point.y), SnapKind::Alignment);
        consider(Point::new(point.x, p.y), SnapKind::Alignment);
    }

    let mut nearby_nodes = Vec::new();
    let mut directions = Vec::new();
    for element in project
        .elements
        .iter()
        .filter(|element| element.story == story)
    {
        let (nodes, segments): (Vec<Point>, Vec<(Point, Point)>) = match &element.shape {
            Shape::Wall { start, end, .. } => {
                let outline = footprint(element);
                let mut segments = vec![(*start, *end)];
                segments.extend(
                    (0..outline.len()).map(|i| (outline[i], outline[(i + 1) % outline.len()])),
                );
                (
                    vec![
                        *start,
                        *end,
                        Point::new((start.x + end.x) * 0.5, (start.y + end.y) * 0.5),
                    ],
                    segments,
                )
            }
            Shape::Polyline { points, .. } => (
                points.clone(),
                points.windows(2).map(|pair| (pair[0], pair[1])).collect(),
            ),
            Shape::Slab { .. } | Shape::Stair { .. } => {
                let outline = footprint(element);
                (
                    outline.to_vec(),
                    (0..outline.len())
                        .map(|i| (outline[i], outline[(i + 1) % outline.len()]))
                        .collect(),
                )
            }
        };
        nearby_nodes.extend(nodes);
        directions.extend(segments);
    }

    // Align the new endpoint horizontally or vertically with a nearby node.
    for point in nearby_nodes.iter().copied() {
        consider(Point::new(p.x, point.y), SnapKind::Alignment);
        consider(Point::new(point.x, p.y), SnapKind::Alignment);
    }

    // Architectural angle inference uses 30-degree increments, including X/Y.
    for step in 0..12 {
        let angle = step as f32 * std::f32::consts::PI / 6.0;
        let direction = Point::new(angle.cos(), angle.sin());
        let distance = (p.x - anchor.x) * direction.x + (p.y - anchor.y) * direction.y;
        if distance > 0.01 {
            consider(
                Point::new(
                    anchor.x + direction.x * distance,
                    anchor.y + direction.y * distance,
                ),
                SnapKind::Angle,
            );
        }
    }

    // A direction perpendicular to an existing segment is another candidate.
    for (start, end) in directions
        .into_iter()
        .chain(chain.windows(2).map(|pair| (pair[0], pair[1])))
    {
        let angle = (end.y - start.y).atan2(end.x - start.x) + std::f32::consts::FRAC_PI_2;
        for perpendicular in [angle, angle + std::f32::consts::PI] {
            let direction = Point::new(perpendicular.cos(), perpendicular.sin());
            let distance = (p.x - anchor.x) * direction.x + (p.y - anchor.y) * direction.y;
            if distance > 0.01 {
                consider(
                    Point::new(
                        anchor.x + direction.x * distance,
                        anchor.y + direction.y * distance,
                    ),
                    SnapKind::Perpendicular,
                );
            }
        }
    }

    best.map_or((base, base_kind), |(_, point, kind)| (point, kind))
}
pub fn corners(project: &Project, e: &Element) -> [Vec3; 8] {
    let base = project.elevation(e.story);
    let (bottom, top) = match e.shape {
        Shape::Wall { height, .. } => {
            let (bottom_offset, top_offset) = e
                .wall_settings
                .as_ref()
                .map_or((0., 0.), |settings| settings.vertical_offsets());
            (base + bottom_offset, base + height + top_offset)
        }
        Shape::Slab { thickness, .. } => (base - thickness, base),
        Shape::Stair { ref spec } => (base, base + spec.total_rise),
        Shape::Polyline { .. } => (base, base),
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
            * glam::camera::rh::view::look_at_mat4(
                self.target + dir * distance,
                self.target,
                Vec3::Z,
            )
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
        if direction.z.abs() < 1e-6 {
            return None;
        }
        let t = (z - near.z) / direction.z;
        if t < 0. {
            return None;
        }
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
    fn wall_reference_line_stays_put_when_the_body_offset_changes() {
        let mut project = Project::default();
        let id = project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(1., 2.),
                end: Point::new(5., 2.),
                thickness: 0.3,
                height: 3.,
            },
        );
        let wall = project
            .elements
            .iter()
            .find(|element| element.id == id)
            .unwrap();
        assert_eq!(
            wall_reference_line(wall),
            Some((Point::new(1., 2.), Point::new(5., 2.)))
        );
        let centered = footprint(wall);
        assert!((centered.iter().map(|point| point.y).sum::<f32>() / 4. - 2.).abs() < 1e-6);

        project.elements[0]
            .wall_settings
            .as_mut()
            .unwrap()
            .geometry
            .reference_offset = 0.25;
        let wall = &project.elements[0];
        assert_eq!(
            wall_reference_line(wall),
            Some((Point::new(1., 2.), Point::new(5., 2.))),
            "the reference line is the wall's defining axis"
        );
        let shifted = footprint(wall);
        assert!((shifted.iter().map(|point| point.y).sum::<f32>() / 4. - 2.25).abs() < 1e-6);
    }

    #[test]
    fn wall_inside_and_outside_locations_move_the_body_not_the_line() {
        let mut project = Project::default();
        let id = project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(4., 0.),
                thickness: 0.4,
                height: 3.,
            },
        );
        let center_y =
            |wall: &Element| footprint(wall).iter().map(|point| point.y).sum::<f32>() / 4.;
        let wall = project
            .elements
            .iter()
            .find(|element| element.id == id)
            .unwrap();
        assert!(center_y(wall).abs() < 1e-6);

        let settings = project.elements[0].wall_settings.as_mut().unwrap();
        settings.geometry.reference_line = crate::wall::WallReferenceLine::Inside;
        assert!((center_y(&project.elements[0]) + 0.2).abs() < 1e-6);
        assert_eq!(
            wall_reference_line(&project.elements[0]),
            Some((Point::new(0., 0.), Point::new(4., 0.)))
        );

        let settings = project.elements[0].wall_settings.as_mut().unwrap();
        settings.geometry.reference_line = crate::wall::WallReferenceLine::Outside;
        settings.geometry.reference_offset = 0.1;
        assert!((center_y(&project.elements[0]) - 0.3).abs() < 1e-6);
        assert_eq!(
            wall_reference_line(&project.elements[0]),
            Some((Point::new(0., 0.), Point::new(4., 0.)))
        );
    }

    #[test]
    fn curved_offsets_preserve_current_circle_and_compose() {
        let points = vec![Point::new(-1., 0.), Point::new(1., 0.)];
        let arc = arc_segment_through(points[0], points[1], Point::new(0., 1.)).unwrap();
        let (first, first_arcs) = offset_polyline_with_arcs(&points, &[Some(arc)], 0.25).unwrap();
        let a = first_arcs[0].unwrap();
        assert!((a.radius - 1.25).abs() < 1e-4);
        assert_eq!(first.len(), 2);
        assert!(
            arc_center_sweep(first[0], first[1], a)
                .unwrap()
                .0
                .distance(Point::default())
                < 1e-4
        );
        let (second, second_arcs) = offset_polyline_with_arcs(&first, &first_arcs, 0.25).unwrap();
        assert!((second_arcs[0].unwrap().radius - 1.5).abs() < 1e-4);
        assert!((second[0].x + 1.5).abs() < 1e-4);
        assert!(offset_polyline_with_arcs(&points, &[Some(arc)], -1.).is_none());
    }

    #[test]
    fn mixed_and_closed_arc_offsets_keep_exact_joins() {
        let points = vec![
            Point::new(-1., -2.),
            Point::new(-1., 0.),
            Point::new(1., 0.),
            Point::new(1., -2.),
        ];
        let arc = arc_segment_through(points[1], points[2], Point::new(0., 1.)).unwrap();
        let (p, a) = offset_polyline_with_arcs(&points, &[None, Some(arc), None], 0.25).unwrap();
        assert!((p[1].x + 1.25).abs() < 1e-4 && p[1].y.abs() < 1e-3);
        assert!((p[2].x - 1.25).abs() < 1e-4 && p[2].y.abs() < 1e-3);
        assert!((a[1].unwrap().radius - 1.25).abs() < 1e-4);
        let closed = vec![Point::new(-1., 0.), Point::new(1., 0.), Point::new(-1., 0.)];
        let lower = arc_segment_through(closed[1], closed[2], Point::new(0., -1.)).unwrap();
        let (p, a) = offset_polyline_with_arcs(&closed, &[Some(arc), Some(lower)], 0.25).unwrap();
        assert_eq!(p[0], p[2]);
        assert!(
            a.iter()
                .all(|arc| (arc.unwrap().radius - 1.25).abs() < 1e-4)
        );
        let reversed = vec![points[3], points[2], points[1], points[0]];
        let mut reverse_arc = arc;
        reverse_arc.clockwise = !arc.clockwise;
        let (_, a) =
            offset_polyline_with_arcs(&reversed, &[None, Some(reverse_arc), None], -0.25).unwrap();
        assert!((a[1].unwrap().radius - 1.25).abs() < 1e-4);
    }

    #[test]
    fn polyline_offset_moves_every_edge_and_preserves_closed_loop() {
        let open = [Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 2.)];
        let shifted = offset_polyline(&open, 0.25).unwrap();
        assert_eq!(
            shifted,
            vec![
                Point::new(0., 0.25),
                Point::new(1.75, 0.25),
                Point::new(1.75, 2.)
            ]
        );
        let square = [
            Point::new(0., 0.),
            Point::new(2., 0.),
            Point::new(2., 2.),
            Point::new(0., 2.),
            Point::new(0., 0.),
        ];
        let inward = offset_polyline(&square, 0.25).unwrap();
        assert_eq!(
            inward,
            vec![
                Point::new(0.25, 0.25),
                Point::new(1.75, 0.25),
                Point::new(1.75, 1.75),
                Point::new(0.25, 1.75),
                Point::new(0.25, 0.25)
            ]
        );
        let outward = offset_polyline(&square, -0.25).unwrap();
        assert_eq!(
            outward,
            vec![
                Point::new(-0.25, -0.25),
                Point::new(2.25, -0.25),
                Point::new(2.25, 2.25),
                Point::new(-0.25, 2.25),
                Point::new(-0.25, -0.25)
            ]
        );
        assert!(
            offset_polyline(&square, 1.1).is_none(),
            "an inward offset past collapse is rejected"
        );
    }

    #[test]
    fn circular_curve_passes_through_drag_point_and_segment_endpoints() {
        let start = Point::new(0., 0.);
        let end = Point::new(2., 0.);
        let through = Point::new(1., 1.);
        let definition = arc_segment_through(start, end, through).unwrap();
        assert!((definition.radius - 1.).abs() < 1e-5);
        let (center, sweep) = arc_center_sweep(start, end, definition).unwrap();
        assert!((center.x - 1.).abs() < 1e-5 && center.y.abs() < 1e-5);
        assert!((sweep.abs() - std::f32::consts::PI).abs() < 1e-5);
        let arc = arc_points(start, end, definition).unwrap();
        assert!(arc.len() >= 3);
        assert!(arc[0].distance(start) < 1e-5);
        assert!(arc.last().unwrap().distance(end) < 1e-5);
        assert!(
            arc.iter()
                .all(|point| (point.distance(center) - definition.radius).abs() < 1e-4)
        );
        assert!(arc.iter().any(|point| point.distance(through) < 1e-4));
        let flatter = arc_segment_through(start, end, Point::new(1., 0.5)).unwrap();
        assert!((flatter.radius - 1.25).abs() < 1e-4 && flatter.radius > definition.radius);
        assert!(
            circular_arc_through(Point::new(0., 0.), Point::new(2., 0.), Point::new(1., 0.))
                .is_none()
        );
    }

    #[test]
    fn polyline_arc_metadata_roundtrips_and_old_json_defaults_to_straight_edges() {
        let old: Shape = serde_json::from_str(
            r#"{"kind":"Polyline","points":[{"x":0.0,"y":0.0},{"x":2.0,"y":0.0}]}"#,
        )
        .unwrap();
        assert!(matches!(old,Shape::Polyline{arcs, ..} if arcs.is_empty()));
        let shape = Shape::Polyline {
            points: vec![Point::new(0., 0.), Point::new(2., 0.)],
            arcs: vec![Some(
                arc_segment_through(Point::new(0., 0.), Point::new(2., 0.), Point::new(1., 0.5))
                    .unwrap(),
            )],
        };
        let encoded = serde_json::to_string(&shape).unwrap();
        assert_eq!(serde_json::from_str::<Shape>(&encoded).unwrap(), shape);
    }

    #[test]
    fn select_cursor_target_prefers_nodes_then_edges_then_object_bodies() {
        let mut project = Project::default();
        project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 2.)],
                arcs: vec![],
            },
        );
        project.add(
            0,
            Material::Concrete,
            Shape::Slab {
                min: Point::new(4., 0.),
                max: Point::new(6., 2.),
                thickness: 0.2,
            },
        );

        assert_eq!(
            plan_hover_kind(&project, 0, Point::new(0., 0.05), 0.1, 0.1),
            Some(PlanHoverKind::Node)
        );
        assert_eq!(
            plan_hover_kind(&project, 0, Point::new(1., 0.05), 0.1, 0.1),
            Some(PlanHoverKind::Edge)
        );
        assert_eq!(
            plan_hover_kind(&project, 0, Point::new(5., 1.), 0.1, 0.1),
            Some(PlanHoverKind::Body)
        );
        assert_eq!(
            plan_hover_kind(&project, 0, Point::new(8., 4.), 0.1, 0.1),
            None
        );
    }

    #[test]
    fn polygon_selection_uses_editable_nodes_but_visibility_uses_overlap() {
        let mut project = Project::default();
        let wall = project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(10., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        let area = [
            Point::new(4., -1.),
            Point::new(6., -1.),
            Point::new(6., 1.),
            Point::new(4., 1.),
        ];
        let e = project.elements.iter().find(|e| e.id == wall).unwrap();
        assert!(!element_has_node_in_polygon(e, &area));
        assert!(element_intersects_polygon(e, &area));
    }

    #[test]
    fn same_material_collinear_walls_have_no_internal_cap_line() {
        let mut project = Project::default();
        project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(2., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(2., 0.),
                end: Point::new(4., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        let walls: Vec<_> = project.elements.iter().collect();
        let footprints = plan_wall_footprints(&walls);
        let outline = wall_outline_segments(&footprints);
        assert!(
            !outline.iter().any(|(a, b)| (a.x - 2.).abs() < 1e-4
                && (b.x - 2.).abs() < 1e-4
                && (a.y - b.y).abs() > 0.1),
            "a shared wall end must not be outlined: {outline:?}"
        );
        assert!(
            outline
                .iter()
                .any(|(a, b)| (a.y - b.y).abs() < 1e-4 && (a.y.abs() - 0.1).abs() < 1e-4)
        );
    }

    #[test]
    fn same_material_t_junction_trims_covered_caps_and_side_edges() {
        let mut project = Project::default();
        project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(4., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(2., 0.),
                end: Point::new(2., 2.),
                thickness: 0.2,
                height: 3.,
            },
        );
        let walls: Vec<_> = project.elements.iter().collect();
        let footprints = plan_wall_footprints(&walls);
        let outline = wall_outline_segments(&footprints);
        assert!(
            plan_wall_attached_ends(&walls, &footprints).contains(&(walls[1].id, true)),
            "the T branch's buried 3D end cap must be hidden"
        );
        assert!(
            !outline
                .iter()
                .any(|(a, b)| a.y.abs() < 1e-4 && b.y.abs() < 1e-4 && (a.x - b.x).abs() > 0.1),
            "the branch end cap should be hidden inside the main wall"
        );
        assert!(
            !outline.iter().any(|(a, b)| (a.y - 0.1).abs() < 1e-4
                && (b.y - 0.1).abs() < 1e-4
                && a.x.min(b.x) < 2.0
                && a.x.max(b.x) > 2.0),
            "the main wall's exposed side should break around the branch"
        );
    }

    #[test]
    fn same_material_l_corner_auto_trims_to_one_continuous_outline() {
        let mut project = Project::default();
        project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(4., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                // Simulate drawing the second wall from the first wall's outside face.
                start: Point::new(4.08, 0.08),
                end: Point::new(4.08, 4.),
                thickness: 0.2,
                height: 3.,
            },
        );
        let walls: Vec<_> = project.elements.iter().collect();
        let footprints = plan_wall_footprints(&walls);
        let outline = wall_outline_segments(&footprints);
        let paths = wall_outline_paths(&footprints);
        let Shape::Wall {
            end: joined_end, ..
        } = walls[0].shape
        else {
            unreachable!()
        };
        let Shape::Wall {
            start: joined_start,
            ..
        } = walls[1].shape
        else {
            unreachable!()
        };
        assert!(footprints.iter().all(|poly| {
            poly.iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
        }));
        assert_eq!(
            joined_end,
            Point::new(4., 0.),
            "the stored project geometry remains untouched"
        );
        assert_eq!(
            joined_start,
            Point::new(4.08, 0.08),
            "the stored project geometry remains untouched"
        );
        assert!(contains(Point::new(4.03, 0.04), &footprints[0]));
        assert!(contains(Point::new(4.03, 0.04), &footprints[1]));
        assert!(
            !outline.iter().any(|(a, b)| (a.y.abs() < 1e-4
                && b.y.abs() < 1e-4
                && (a.x - b.x).abs() > 0.1)
                || ((a.x - 4.08).abs() < 1e-4
                    && (b.x - 4.08).abs() < 1e-4
                    && a.y.min(b.y) < 0.0
                    && a.y.max(b.y) > 0.0)),
            "there should be no cap or seam at the auto-joined corner: {outline:?}"
        );
        assert_eq!(
            paths.len(),
            1,
            "an L corner must render as one continuous outline path: {paths:?}"
        );
        assert!(
            paths[0][0].distance(*paths[0].last().unwrap()) <= 1e-3,
            "the joined outline must close at its final corner: {paths:?}"
        );
    }

    #[test]
    fn face_aligned_l_corner_snaps_through_full_wall_depth() {
        let mut project = Project::default();
        project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(7.5, 4.0),
                end: Point::new(12.0, 4.0),
                thickness: 0.22,
                height: 3.0,
            },
        );
        project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                // Typical face-aligned drawing: the return wall starts 0.20 m
                // into the host wall's depth, not exactly on its centerline.
                start: Point::new(7.5, 4.2),
                end: Point::new(7.5, 8.0),
                thickness: 0.22,
                height: 3.0,
            },
        );
        let walls: Vec<_> = project.elements.iter().collect();
        let footprints = plan_wall_footprints(&walls);
        let outline = wall_outline_segments(&footprints);
        let paths = wall_outline_paths(&footprints);
        assert!(contains(Point::new(7.55, 4.05), &footprints[0]));
        assert!(contains(Point::new(7.55, 4.05), &footprints[1]));
        assert!(
            contains(Point::new(7.45, 3.95), &footprints[0])
                || contains(Point::new(7.45, 3.95), &footprints[1]),
            "extended rectangular wall bodies should fill the outside corner without a bevel"
        );
        assert!(plan_wall_attached_ends(&walls, &footprints).contains(&(walls[0].id, true)));
        assert!(plan_wall_attached_ends(&walls, &footprints).contains(&(walls[1].id, true)));
        assert!(
            !outline.iter().any(|(a, b)| (a.x - 7.5).abs() < 1e-4
                && (b.x - 7.5).abs() < 1e-4
                && (a.y - b.y).abs() > 0.21),
            "the face-aligned return must not leave an exposed end cap: {outline:?}"
        );
        assert!(
            outline
                .iter()
                .all(|(a, b)| (a.x - b.x).abs() < 1e-4 || (a.y - b.y).abs() < 1e-4),
            "joined wall outlines must meet at orthogonal edge intersections, never diagonal bevels: {outline:?}"
        );
        assert_eq!(
            paths.len(),
            1,
            "the connected L perimeter should be one path: {paths:?}"
        );
        assert!(paths[0][0].distance(*paths[0].last().unwrap()) <= 1e-3);
    }

    #[test]
    fn rotated_rectangle_tracks_three_points() {
        let polygon =
            rotated_rectangle_polygon(Point::new(0., 0.), Point::new(4., 0.), Point::new(4., 2.))
                .unwrap();
        assert_eq!(
            polygon,
            vec![
                Point::new(0., 0.),
                Point::new(4., 0.),
                Point::new(4., 2.),
                Point::new(0., 2.)
            ]
        );
        assert!(
            rotated_rectangle_polygon(Point::default(), Point::default(), Point::new(0., 1.))
                .is_none()
        );
    }

    #[test]
    fn perspective_plane_projection_round_trips_and_rejects_above_horizon() {
        let camera = Camera::default();
        let world = Point::new(6., 3.);
        let clip = camera
            .matrix(4. / 3.)
            .project_point3(Vec3::new(world.x, world.y, 0.));
        let round_trip = camera
            .point_on_horizontal_plane([clip.x, clip.y], 4. / 3., 0.)
            .unwrap();
        assert!(round_trip.distance(world) < 1e-3);
        let above_horizon = camera.point_on_horizontal_plane([0., 3.], 4. / 3., 0.);
        assert!(
            above_horizon.is_none(),
            "above-horizon ray returned {above_horizon:?}"
        );
    }

    #[test]
    fn concave_marquee_clipping_covers_exact_intersection_without_spill() {
        let concave = [
            Point::new(0., 0.),
            Point::new(2., 0.),
            Point::new(2., 1.),
            Point::new(1., 1.),
            Point::new(1., 2.),
            Point::new(0., 2.),
        ];
        let subject = [
            Point::new(-1., -1.),
            Point::new(3., -1.),
            Point::new(3., 3.),
            Point::new(-1., 3.),
        ];
        let fragments = intersect_footprint_with_polygon(&subject, &concave);
        let area: f32 = fragments.iter().map(|p| polygon_area(p).abs()).sum();
        assert!((area - 3.).abs() < 1e-4, "clipped area was {area}");
        assert!(fragments.iter().flatten().all(|p| contains(*p, &concave)));
    }

    #[test]
    fn camera_projection_is_perspective() {
        let camera = Camera::default();
        let matrix = camera.matrix(1.5);
        let dir = Vec3::new(
            camera.yaw.cos() * camera.pitch.cos(),
            camera.yaw.sin() * camera.pitch.cos(),
            camera.pitch.sin(),
        );
        let camera_distance = camera.vertical_span(1.5) / (2. * (45_f32.to_radians() * 0.5).tan());
        let distant = camera.target + dir * (camera_distance * 0.2);
        let close = camera.target + dir * (camera_distance * 0.7);
        let side = Vec3::new(-camera.yaw.sin(), camera.yaw.cos(), 0.) * 2.;
        let distant_width = matrix
            .project_point3(distant + side)
            .distance(matrix.project_point3(distant - side));
        let close_width = matrix
            .project_point3(close + side)
            .distance(matrix.project_point3(close - side));
        assert!(
            close_width > distant_width * 1.8,
            "close={close_width}, distant={distant_width}"
        );
    }
}
