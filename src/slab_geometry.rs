//! Typed slab perimeter edits. Exact editable arcs remain separate from display samples.
use crate::{
    geometry,
    model::{ArcSegment, Element, Point, Shape, SlabContour, SlabEdgeSettings},
};
use geo::{BooleanOps, Coord, LineString, Polygon, TriangulateEarcut};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlabEdgeScope {
    Clicked,
    Ring,
    All,
}
#[derive(Clone, Debug, PartialEq)]
pub enum SlabChange {
    InsertNode,
    InsertArcNode,
    MoveNode,
    Curve,
    Tangent,
    OffsetEdge,
    OffsetAll,
    AddPolygon(Vec<Point>),
    SubtractPolygon(Vec<Point>),
    SetEdge {
        angle_degrees: f32,
        surface: Option<crate::model::Material>,
        scope: SlabEdgeScope,
    },
    Elevate(f32),
    Fillet(f32),
    Chamfer(f32),
}
fn invalid<T>() -> Result<T, String> {
    Err("Invalid edit parameters".into())
}
pub fn contour(e: &Element) -> SlabContour {
    e.slab_contour.clone().unwrap_or_else(|| SlabContour {
        points: geometry::footprint(e).to_vec(),
        ..Default::default()
    })
}
pub fn rings(e: &Element) -> Vec<SlabContour> {
    let c = contour(e);
    std::iter::once(c.clone())
        .chain(c.holes)
        .map(|mut r| {
            r.holes.clear();
            r
        })
        .collect()
}
fn arcs(r: &SlabContour) -> Vec<Option<ArcSegment>> {
    if r.arcs.is_empty() {
        vec![None; r.points.len()]
    } else {
        r.arcs.clone()
    }
}
fn settings(r: &SlabContour) -> Vec<SlabEdgeSettings> {
    if r.edge_settings.is_empty() {
        vec![SlabEdgeSettings::default(); r.points.len()]
    } else {
        r.edge_settings.clone()
    }
}
pub fn ring_path(r: &SlabContour) -> Vec<Point> {
    if r.points.len() < 2 {
        return r.points.clone();
    }
    let mut points = r.points.clone();
    points.push(points[0]);
    let mut path = geometry::polyline_path(&points, &arcs(r));
    path.pop();
    path
}
pub fn outline(e: &Element) -> Vec<Point> {
    if !matches!(e.shape, Shape::Slab { .. }) {
        return Vec::new();
    }
    ring_path(&contour(e))
}
pub fn outlines(e: &Element) -> Vec<Vec<Point>> {
    rings(e).iter().map(ring_path).collect()
}
pub fn closed_path(e: &Element) -> Vec<Point> {
    let mut p = outline(e);
    if let Some(a) = p.first().copied() {
        p.push(a);
    }
    p
}
pub fn bounds(points: &[Point]) -> (Point, Point) {
    points.iter().fold(
        (
            Point::new(f32::INFINITY, f32::INFINITY),
            Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY),
        ),
        |(a, b), p| {
            (
                Point::new(a.x.min(p.x), a.y.min(p.y)),
                Point::new(b.x.max(p.x), b.y.max(p.y)),
            )
        },
    )
}
fn signed_area(p: &[Point]) -> f32 {
    if p.len() < 3 {
        return 0.;
    }
    (0..p.len())
        .map(|i| {
            let a = p[i];
            let b = p[(i + 1) % p.len()];
            a.x * b.y - a.y * b.x
        })
        .sum::<f32>()
        * 0.5
}
pub fn area(p: &[Point]) -> f32 {
    signed_area(p).abs()
}
fn signed_ring_area(r: &SlabContour) -> f32 {
    let mut result = signed_area(&r.points);
    for (i, arc) in arcs(r).iter().enumerate() {
        if let Some(a) = arc
            && let Some((_, sweep)) =
                geometry::arc_center_sweep(r.points[i], r.points[(i + 1) % r.points.len()], *a)
        {
            result += a.radius * a.radius * (sweep - sweep.sin()) * 0.5;
        }
    }
    result
}
fn ring_area(r: &SlabContour) -> f32 {
    signed_ring_area(r).abs()
}
pub fn actual_area(e: &Element) -> f32 {
    let c = contour(e);
    ring_area(&c) - c.holes.iter().map(ring_area).sum::<f32>()
}
fn on_arc(q: Point, a: Point, b: Point, arc: ArcSegment, tolerance: f32) -> bool {
    let Some((center, sweep)) = geometry::arc_center_sweep(a, b, arc) else {
        return false;
    };
    if (q.distance(center) - arc.radius).abs() > tolerance {
        return false;
    }
    let start = (a.y - center.y).atan2(a.x - center.x);
    let angle = (q.y - center.y).atan2(q.x - center.x);
    let t = if sweep > 0. {
        (angle - start).rem_euclid(std::f32::consts::TAU)
    } else {
        (start - angle).rem_euclid(std::f32::consts::TAU)
    };
    q.distance(a) <= tolerance || q.distance(b) <= tolerance || t <= sweep.abs() + 1e-5
}
pub fn contour_bounds(r: &SlabContour) -> (Point, Point) {
    let mut points = r.points.clone();
    for (i, arc) in arcs(r).into_iter().enumerate() {
        if let Some(arc) = arc
            && let Some((c, _)) =
                geometry::arc_center_sweep(r.points[i], r.points[(i + 1) % r.points.len()], arc)
        {
            for (x, y) in [(1., 0.), (-1., 0.), (0., 1.), (0., -1.)] {
                let q = Point::new(c.x + x * arc.radius, c.y + y * arc.radius);
                if on_arc(
                    q,
                    r.points[i],
                    r.points[(i + 1) % r.points.len()],
                    arc,
                    1e-4,
                ) {
                    points.push(q);
                }
            }
        }
    }
    bounds(&points)
}
fn line_string(points: &[Point]) -> LineString<f64> {
    let mut v: Vec<_> = points
        .iter()
        .map(|p| Coord {
            x: p.x as f64,
            y: p.y as f64,
        })
        .collect();
    if v.first() != v.last()
        && let Some(first) = v.first().copied()
    {
        v.push(first);
    }
    LineString::new(v)
}
pub fn polygon(e: &Element) -> Polygon<f64> {
    polygon_contour(&contour(e))
}
fn polygon_contour(c: &SlabContour) -> Polygon<f64> {
    Polygon::new(
        line_string(&ring_path(c)),
        c.holes.iter().map(|h| line_string(&ring_path(h))).collect(),
    )
}
pub fn triangles(e: &Element) -> Vec<[Point; 3]> {
    polygon(e)
        .earcut_triangles()
        .into_iter()
        .map(|t| [t.v1(), t.v2(), t.v3()].map(|p| Point::new(p.x as f32, p.y as f32)))
        .collect()
}
fn validate_path(points: &[Point], minimum: f32) -> Result<(), String> {
    let n = points.len();
    if !(3..=100_000).contains(&n)
        || points
            .iter()
            .any(|p| !p.finite() || p.x.abs() >= 100000. || p.y.abs() >= 100000.)
        || area(points) < 1e-6
    {
        return invalid();
    }
    for i in 0..n {
        if points[i].distance(points[(i + 1) % n]) < minimum {
            return invalid();
        }
        for j in i + 1..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            if geometry::segments_intersect(
                points[i],
                points[(i + 1) % n],
                points[j],
                points[(j + 1) % n],
            ) {
                return invalid();
            }
        }
    }
    Ok(())
}
pub fn validate(points: &[Point]) -> Result<(), String> {
    validate_path(points, 0.01)
}
fn validate_ring(r: &SlabContour) -> Result<(), String> {
    let n = r.points.len();
    if !(3..=10_000).contains(&n)
        || (!r.arcs.is_empty() && r.arcs.len() != n)
        || (!r.edge_settings.is_empty() && r.edge_settings.len() != n)
        || !r.elevation.is_finite()
    {
        return invalid();
    }
    for i in 0..n {
        if !r.points[i].finite() || r.points[i].distance(r.points[(i + 1) % n]) < 0.01 {
            return invalid();
        }
    }
    for (i, a) in arcs(r).iter().enumerate() {
        if let Some(a) = a
            && geometry::arc_center_sweep(r.points[i], r.points[(i + 1) % n], *a).is_none()
        {
            return invalid();
        }
    }
    if r.edge_settings.iter().any(|s| {
        !s.angle_degrees.is_finite() || s.angle_degrees <= 0.01 || s.angle_degrees >= 179.99
    }) {
        return invalid();
    }
    validate_path(&ring_path(r), 1e-6)
}
pub fn validate_contour(c: &SlabContour) -> Result<(), String> {
    validate_ring(c)?;
    let outer = ring_path(c);
    let mut holes = Vec::new();
    for hole in &c.holes {
        if !hole.holes.is_empty() {
            return invalid();
        }
        validate_ring(hole)?;
        let path = ring_path(hole);
        if path.iter().any(|p| !geometry::point_in_polygon(*p, &outer)) {
            return invalid();
        }
        for ring in std::iter::once(&outer).chain(holes.iter()) {
            for i in 0..path.len() {
                for j in 0..ring.len() {
                    if geometry::segments_intersect(
                        path[i],
                        path[(i + 1) % path.len()],
                        ring[j],
                        ring[(j + 1) % ring.len()],
                    ) {
                        return invalid();
                    }
                }
            }
        }
        if holes.iter().any(|other: &Vec<Point>| {
            geometry::point_in_polygon(path[0], other)
                || geometry::point_in_polygon(other[0], &path)
        }) {
            return invalid();
        }
        holes.push(path);
    }
    if ring_area(c) - c.holes.iter().map(ring_area).sum::<f32>() < 0.0001 {
        return invalid();
    }
    Ok(())
}
pub fn sync(e: &mut Element, c: SlabContour) -> Result<(), String> {
    if !matches!(e.shape, Shape::Slab { .. }) {
        return invalid();
    }
    validate_contour(&c)?;
    let (a, b) = contour_bounds(&c);
    if let Shape::Slab { min, max, .. } = &mut e.shape {
        *min = a;
        *max = b;
    }
    e.slab_contour = Some(c);
    Ok(())
}
pub fn transform(e: &mut Element, f: impl Fn(Point) -> Point, scale: f32, mirror: bool) {
    fn ring(r: &mut SlabContour, f: &impl Fn(Point) -> Point, s: f32, m: bool) {
        for p in &mut r.points {
            *p = f(*p);
        }
        for a in r.arcs.iter_mut().flatten() {
            a.radius *= s.abs();
            if m {
                a.clockwise = !a.clockwise;
            }
        }
        for h in &mut r.holes {
            ring(h, f, s, m);
        }
    }
    if let Some(c) = &mut e.slab_contour {
        ring(c, &f, scale, mirror);
        let (a, b) = contour_bounds(c);
        if let Shape::Slab { min, max, .. } = &mut e.shape {
            *min = a;
            *max = b;
        }
    }
}
fn locate(c: &SlabContour, index: usize) -> Option<(usize, usize)> {
    let mut base = 0;
    for (ring, r) in std::iter::once(c).chain(c.holes.iter()).enumerate() {
        if index < base + r.points.len() {
            return Some((ring, index - base));
        }
        base += r.points.len();
    }
    None
}
fn ring_mut(c: &mut SlabContour, index: usize) -> &mut SlabContour {
    if index == 0 {
        c
    } else {
        &mut c.holes[index - 1]
    }
}
fn normal(r: &SlabContour, i: usize, p: Point) -> Result<Point, String> {
    let a = r.points[i];
    let b = r.points[(i + 1) % r.points.len()];
    if let Some(arc) = arcs(r)[i] {
        let (center, _) = geometry::arc_center_sweep(a, b, arc).ok_or("Invalid edit parameters")?;
        let d = p.distance(center);
        if d < 1e-5 {
            return invalid();
        }
        let sign = if arc.clockwise { 1. } else { -1. };
        Ok(Point::new(
            sign * (p.x - center.x) / d,
            sign * (p.y - center.y) / d,
        ))
    } else {
        let d = a.distance(b);
        Ok(Point::new(-(b.y - a.y) / d, (b.x - a.x) / d))
    }
}
fn sub(a: Point, b: Point) -> Point {
    Point::new(a.x - b.x, a.y - b.y)
}
fn cross(a: Point, b: Point) -> f32 {
    a.x * b.y - a.y * b.x
}
#[derive(Clone, Copy)]
struct Support {
    a: Point,
    b: Point,
    circle: Option<(Point, f32)>,
}
fn intersections(a: Support, b: Support) -> Vec<Point> {
    match (a.circle, b.circle) {
        (None, None) => {
            let v = sub(a.b, a.a);
            let w = sub(b.b, b.a);
            let den = cross(v, w);
            if den.abs() < 1e-6 {
                return vec![];
            }
            let t = cross(sub(b.a, a.a), w) / den;
            vec![Point::new(a.a.x + t * v.x, a.a.y + t * v.y)]
        }
        (None, Some((center, radius))) | (Some((center, radius)), None) => {
            let line = if a.circle.is_none() { a } else { b };
            let d = sub(line.b, line.a);
            let v = sub(line.a, center);
            let aa = d.x * d.x + d.y * d.y;
            let bb = 2. * (v.x * d.x + v.y * d.y);
            let cc = v.x * v.x + v.y * v.y - radius * radius;
            let det = bb * bb - 4. * aa * cc;
            if det < 0. || aa < 1e-10 {
                return vec![];
            }
            [
                (-bb - det.sqrt()) / (2. * aa),
                (-bb + det.sqrt()) / (2. * aa),
            ]
            .map(|t| Point::new(line.a.x + t * d.x, line.a.y + t * d.y))
            .to_vec()
        }
        (Some((ca, ra)), Some((cb, rb))) => {
            let d = ca.distance(cb);
            if d < 1e-6 || d > ra + rb || d < (ra - rb).abs() {
                return vec![];
            }
            let x = (ra * ra - rb * rb + d * d) / (2. * d);
            let h = (ra * ra - x * x).max(0.).sqrt();
            let u = Point::new((cb.x - ca.x) / d, (cb.y - ca.y) / d);
            let mid = Point::new(ca.x + x * u.x, ca.y + x * u.y);
            vec![
                Point::new(mid.x - h * u.y, mid.y + h * u.x),
                Point::new(mid.x + h * u.y, mid.y - h * u.x),
            ]
        }
    }
}
fn offset_edge(r: &mut SlabContour, index: usize, distance: f32) -> Result<(), String> {
    let mut distances = vec![0.; r.points.len()];
    distances[index] = distance;
    offset_distances(r, &distances)
}
fn offset_distances(r: &mut SlabContour, distances: &[f32]) -> Result<(), String> {
    let n = r.points.len();
    let source = r.points.clone();
    let mut aa = arcs(r);
    let mut supports = Vec::new();
    for i in 0..n {
        let a = source[i];
        let b = source[(i + 1) % n];
        let delta = distances[i];
        if let Some(mut arc) = aa[i] {
            let (c, _) = geometry::arc_center_sweep(a, b, arc).ok_or("Invalid edit parameters")?;
            let radius = arc.radius + if arc.clockwise { delta } else { -delta };
            if radius < 0.01 {
                return invalid();
            }
            let ratio = radius / arc.radius;
            arc.radius = radius;
            aa[i] = Some(arc);
            supports.push(Support {
                a: Point::new(c.x + (a.x - c.x) * ratio, c.y + (a.y - c.y) * ratio),
                b: Point::new(c.x + (b.x - c.x) * ratio, c.y + (b.y - c.y) * ratio),
                circle: Some((c, radius)),
            });
        } else {
            let l = a.distance(b);
            let shift = Point::new(-(b.y - a.y) * delta / l, (b.x - a.x) * delta / l);
            supports.push(Support {
                a: Point::new(a.x + shift.x, a.y + shift.y),
                b: Point::new(b.x + shift.x, b.y + shift.y),
                circle: None,
            });
        }
    }
    for i in 0..n {
        let prev = supports[(i + n - 1) % n];
        let current = supports[i];
        if prev.b.distance(current.a) < 1e-5 {
            r.points[i] = current.a;
            continue;
        }
        r.points[i] = intersections(prev, current)
            .into_iter()
            .min_by(|a, b| a.distance(source[i]).total_cmp(&b.distance(source[i])))
            .ok_or("Offset cannot preserve a connected valid path at this distance")?;
    }
    r.arcs = aa;
    Ok(())
}
/// Derive the lower outline from the fixed upper outline and each edge's custom angle.
/// Offsets follow material inward: hole boundaries therefore use the opposite ring sign.
pub fn bottom_contour(e: &Element) -> Result<SlabContour, String> {
    let mut c = contour(e);
    validate_contour(&c)?;
    fn lower(r: &mut SlabContour, thickness: f32, hole: bool) -> Result<(), String> {
        let sign = signed_ring_area(r).signum() * if hole { -1. } else { 1. };
        let distances: Vec<_> = settings(r)
            .iter()
            .map(|s| {
                if (s.angle_degrees - 90.).abs() < 1e-5 {
                    0.
                } else {
                    thickness / s.angle_degrees.to_radians().tan() * sign
                }
            })
            .collect();
        if distances.iter().any(|d| d.abs() > 1e-6) {
            offset_distances(r, &distances)?;
        }
        for h in &mut r.holes {
            lower(h, thickness, true)?;
        }
        Ok(())
    }
    lower(&mut c, e.thickness(), false)?;
    validate_contour(&c)?;
    Ok(c)
}
fn edge_sample(r: &SlabContour, i: usize, t: f32) -> Point {
    let a = r.points[i];
    let b = r.points[(i + 1) % r.points.len()];
    if let Some(arc) = arcs(r)[i]
        && let Some((center, sweep)) = geometry::arc_center_sweep(a, b, arc)
    {
        let angle = (a.y - center.y).atan2(a.x - center.x) + sweep * t;
        Point::new(
            center.x + arc.radius * angle.cos(),
            center.y + arc.radius * angle.sin(),
        )
    } else {
        Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
    }
}
fn edge_samples(a: &SlabContour, b: &SlabContour, i: usize) -> usize {
    [a, b]
        .iter()
        .filter_map(|r| {
            arcs(r)[i].and_then(|arc| {
                geometry::arc_center_sweep(r.points[i], r.points[(i + 1) % r.points.len()], arc)
            })
        })
        .map(|(_, s)| (s.abs() / 5_f32.to_radians()).ceil() as usize)
        .max()
        .unwrap_or(1)
        .max(1)
}
/// Hole-aware top and bottom caps use the exact same boundary samples as side_faces.
pub fn cap_triangles(e: &Element, base: f32) -> Result<Vec<[glam::Vec3; 3]>, String> {
    let top = contour(e);
    let bottom = bottom_contour(e)?;
    let mut paths = vec![Vec::new(), Vec::new()];
    for (a, b) in std::iter::once(&top)
        .chain(top.holes.iter())
        .zip(std::iter::once(&bottom).chain(bottom.holes.iter()))
    {
        let mut pair = [Vec::new(), Vec::new()];
        for i in 0..a.points.len() {
            let n = edge_samples(a, b, i);
            for j in 0..n {
                let t = j as f32 / n as f32;
                pair[0].push(edge_sample(a, i, t));
                pair[1].push(edge_sample(b, i, t));
            }
        }
        for (k, p) in pair.iter().enumerate() {
            paths[k].push(line_string(p));
        }
    }
    let mut result = Vec::new();
    for (k, mut rings) in paths.into_iter().enumerate() {
        let poly = Polygon::new(rings.remove(0), rings);
        let z = base + top.elevation - if k == 0 { 0. } else { e.thickness() };
        for triangle in poly.earcut_triangles() {
            let mut q = [triangle.v1(), triangle.v2(), triangle.v3()]
                .map(|p| glam::Vec3::new(p.x as f32, p.y as f32, z));
            if k == 1 {
                q.swap(0, 2);
            }
            result.push(q);
        }
    }
    Ok(result)
}
/// Side quads retain the source edge's surface settings for shared picking/rendering.
pub fn side_faces(
    e: &Element,
    base: f32,
) -> Result<Vec<(SlabEdgeSettings, [glam::Vec3; 4])>, String> {
    let top = contour(e);
    let bottom = bottom_contour(e)?;
    let z = base + top.elevation;
    let mut result = Vec::new();
    for (a, b) in std::iter::once(&top)
        .chain(top.holes.iter())
        .zip(std::iter::once(&bottom).chain(bottom.holes.iter()))
    {
        for i in 0..a.points.len() {
            let samples = edge_samples(a, b, i);
            let setting = settings(a)[i].clone();
            for j in 0..samples {
                let t0 = j as f32 / samples as f32;
                let t1 = (j + 1) as f32 / samples as f32;
                let points = [
                    edge_sample(a, i, t0),
                    edge_sample(a, i, t1),
                    edge_sample(b, i, t1),
                    edge_sample(b, i, t0),
                ];
                result.push((
                    setting.clone(),
                    std::array::from_fn(|k| {
                        glam::Vec3::new(
                            points[k].x,
                            points[k].y,
                            if k < 2 { z } else { z - e.thickness() },
                        )
                    }),
                ));
            }
        }
    }
    Ok(result)
}
pub fn solid_triangles(e: &Element, base: f32) -> Result<Vec<[glam::Vec3; 3]>, String> {
    let mut result = cap_triangles(e, base)?;
    for (_, q) in side_faces(e, base)? {
        result.extend([[q[0], q[1], q[2]], [q[0], q[2], q[3]]]);
    }
    Ok(result)
}
fn offset_all(r: &mut SlabContour, d: f32) -> Result<(), String> {
    let mut p = r.points.clone();
    p.push(p[0]);
    let (p, arcs) = geometry::offset_polyline_with_arcs(&p, &arcs(r), d)
        .ok_or("Offset cannot preserve a connected valid path at this distance")?;
    r.points = p[..p.len() - 1].to_vec();
    r.arcs = arcs;
    Ok(())
}
fn adapt_corner(
    e: &Element,
    r: &mut SlabContour,
    index: usize,
    change: crate::polyline_edit::PolylineChange,
    anchor: Point,
    point: Point,
) -> Result<(), String> {
    let mut fake = e.clone();
    let mut p = r.points.clone();
    p.push(p[0]);
    fake.shape = Shape::Polyline {
        points: p,
        arcs: arcs(r),
    };
    let Shape::Polyline { mut points, arcs } =
        crate::polyline_edit::changed_shape(&fake, Some(index), None, change, anchor, point)?
    else {
        return invalid();
    };
    points.pop();
    r.points = points;
    r.arcs = arcs;
    r.edge_settings.clear();
    Ok(())
}
pub fn changed(
    e: &Element,
    index: usize,
    change: &SlabChange,
    anchor: Point,
    point: Point,
) -> Result<Vec<SlabContour>, String> {
    if !matches!(e.shape, Shape::Slab { .. }) || !anchor.finite() || !point.finite() {
        return invalid();
    }
    let mut c = contour(e);
    validate_contour(&c)?;
    let before = c.clone();
    match change {
        SlabChange::AddPolygon(points) | SlabChange::SubtractPolygon(points) => {
            return boolean(&c, points, matches!(change, SlabChange::SubtractPolygon(_)));
        }
        SlabChange::Elevate(dz) => {
            if !dz.is_finite() {
                return invalid();
            }
            c.elevation += dz;
        }
        _ => {
            let (ring, index) = locate(&c, index).ok_or("Invalid edit parameters")?;
            if let SlabChange::SetEdge {
                angle_degrees,
                surface,
                scope,
            } = change
            {
                if !angle_degrees.is_finite() || *angle_degrees <= 0.01 || *angle_degrees >= 179.99
                {
                    return invalid();
                }
                let value = SlabEdgeSettings {
                    angle_degrees: *angle_degrees,
                    surface: *surface,
                };
                let apply = |r: &mut SlabContour, only: Option<usize>| {
                    r.edge_settings = settings(r);
                    if let Some(i) = only {
                        r.edge_settings[i] = value.clone();
                    } else {
                        r.edge_settings.fill(value.clone());
                    }
                };
                match scope {
                    SlabEdgeScope::Clicked => apply(ring_mut(&mut c, ring), Some(index)),
                    SlabEdgeScope::Ring => apply(ring_mut(&mut c, ring), None),
                    SlabEdgeScope::All => {
                        apply(&mut c, None);
                        for h in &mut c.holes {
                            apply(h, None);
                        }
                    }
                }
            } else {
                let r = ring_mut(&mut c, ring);
                let n = r.points.len();
                r.arcs = arcs(r);
                match change {
                    SlabChange::InsertNode => {
                        if r.arcs[index].is_some() {
                            r.arcs[index] = None;
                        } else {
                            r.points.insert(index + 1, point);
                            r.arcs.insert(index + 1, None);
                            if !r.edge_settings.is_empty() {
                                let v = r.edge_settings[index].clone();
                                r.edge_settings.insert(index + 1, v);
                            }
                        }
                    }
                    SlabChange::InsertArcNode => {
                        let arc = r.arcs[index].ok_or("Invalid edit parameters")?;
                        let a = r.points[index];
                        let b = r.points[(index + 1) % n];
                        let (center, sweep) = geometry::arc_center_sweep(a, b, arc)
                            .ok_or("Invalid edit parameters")?;
                        let d = center.distance(anchor);
                        if d < 1e-6 {
                            return invalid();
                        }
                        let q = Point::new(
                            center.x + (anchor.x - center.x) * arc.radius / d,
                            center.y + (anchor.y - center.y) * arc.radius / d,
                        );
                        if !on_arc(q, a, b, arc, 1e-4)
                            || q.distance(a) < 0.01
                            || q.distance(b) < 0.01
                        {
                            return invalid();
                        }
                        let start = (a.y - center.y).atan2(a.x - center.x);
                        let split = (q.y - center.y).atan2(q.x - center.x);
                        let first = if sweep > 0. {
                            (split - start).rem_euclid(std::f32::consts::TAU)
                        } else {
                            -(start - split).rem_euclid(std::f32::consts::TAU)
                        };
                        let mut left = arc;
                        left.large_arc = first.abs() > std::f32::consts::PI;
                        let mut right = arc;
                        right.large_arc = (sweep - first).abs() > std::f32::consts::PI;
                        r.points.insert(index + 1, q);
                        r.arcs[index] = Some(left);
                        r.arcs.insert(index + 1, Some(right));
                        if !r.edge_settings.is_empty() {
                            let value = r.edge_settings[index].clone();
                            r.edge_settings.insert(index + 1, value);
                        }
                    }
                    SlabChange::MoveNode => {
                        let prev = (index + n - 1) % n;
                        let next = (index + 1) % n;
                        if point.distance(r.points[prev]) < 0.01
                            || point.distance(r.points[next]) < 0.01
                        {
                            if n <= 3 {
                                return invalid();
                            }
                            r.points.remove(index);
                            r.arcs.remove(index);
                            if !r.edge_settings.is_empty() {
                                r.edge_settings.remove(index);
                            }
                            let previous = (index + r.points.len() - 1) % r.points.len();
                            r.arcs[previous] = None;
                        } else {
                            r.points[index] = point;
                        }
                    }
                    SlabChange::Curve => {
                        r.arcs[index] = geometry::arc_segment_through(
                            r.points[index],
                            r.points[(index + 1) % n],
                            point,
                        )
                    }
                    SlabChange::Tangent => {
                        r.arcs[index] = crate::polyline_edit::tangent_arc(
                            r.points[index],
                            r.points[(index + 1) % n],
                            point,
                            anchor.distance(r.points[index])
                                > anchor.distance(r.points[(index + 1) % n]),
                        )?
                    }
                    SlabChange::OffsetEdge | SlabChange::OffsetAll => {
                        let normal = normal(r, index, anchor)?;
                        let v = sub(point, anchor);
                        let d = v.x * normal.x + v.y * normal.y;
                        if matches!(change, SlabChange::OffsetEdge) {
                            offset_edge(r, index, d)?;
                        } else {
                            offset_all(r, d)?;
                        }
                    }
                    SlabChange::Fillet(v) => adapt_corner(
                        e,
                        r,
                        index,
                        crate::polyline_edit::PolylineChange::Fillet(*v),
                        anchor,
                        point,
                    )?,
                    SlabChange::Chamfer(v) => adapt_corner(
                        e,
                        r,
                        index,
                        crate::polyline_edit::PolylineChange::Chamfer(*v),
                        anchor,
                        point,
                    )?,
                    _ => unreachable!(),
                }
            }
        }
    }
    validate_contour(&c)?;
    let mut candidate = e.clone();
    candidate.slab_contour = Some(c.clone());
    bottom_contour(&candidate)?;
    if equivalent(&c, &before) {
        return Err("No elements changed".into());
    }
    Ok(vec![c])
}
fn equivalent(a: &SlabContour, b: &SlabContour) -> bool {
    fn normalized(c: &SlabContour) -> SlabContour {
        let mut result = c.clone();
        result.arcs = arcs(c);
        result.edge_settings = settings(c);
        result.holes = c.holes.iter().map(normalized).collect();
        result
    }
    normalized(a) == normalized(b)
}
pub fn insert_node(e: &mut Element, index: usize, point: Point) -> Result<(), String> {
    let c = changed(e, index, &SlabChange::InsertNode, point, point)?.remove(0);
    sync(e, c)
}

#[derive(Clone)]
struct SourceEdge {
    a: Point,
    b: Point,
    arc: Option<ArcSegment>,
    settings: SlabEdgeSettings,
}
fn source_edges(c: &SlabContour) -> Vec<SourceEdge> {
    std::iter::once(c)
        .chain(c.holes.iter())
        .flat_map(|r| {
            let arcs = arcs(r);
            let settings = settings(r);
            (0..r.points.len()).map(move |i| SourceEdge {
                a: r.points[i],
                b: r.points[(i + 1) % r.points.len()],
                arc: arcs[i],
                settings: settings[i].clone(),
            })
        })
        .collect()
}
fn restore_ring(line: &LineString<f64>, sources: &[SourceEdge]) -> SlabContour {
    let mut points: Vec<_> = line
        .0
        .iter()
        .map(|p| Point::new(p.x as f32, p.y as f32))
        .collect();
    if points.len() > 1 && points[0].distance(*points.last().unwrap()) < 1e-5 {
        points.pop();
    }
    points.dedup_by(|a, b| a.distance(*b) < 1e-5);
    let n = points.len();
    if n < 3 {
        return SlabContour {
            points,
            ..Default::default()
        };
    }
    let mut classes: Vec<_> = (0..n)
        .map(|i| {
            sources.iter().position(|s| {
                let a = points[i];
                let b = points[(i + 1) % n];
                if let Some(arc) = s.arc {
                    let tol = (arc.radius * 1e-5).max(1e-4);
                    (on_arc(a, s.a, s.b, arc, tol) && on_arc(b, s.a, s.b, arc, tol))
                        || geometry::arc_points(s.a, s.b, arc).is_some_and(|path| {
                            path.windows(2).any(|edge| {
                                geometry::segment_distance(a, edge[0], edge[1]) < tol
                                    && geometry::segment_distance(b, edge[0], edge[1]) < tol
                            })
                        })
                } else {
                    geometry::segment_distance(a, s.a, s.b) < 1e-4
                        && geometry::segment_distance(b, s.a, s.b) < 1e-4
                }
            })
        })
        .collect();
    // Boolean topology is found on display chords, then crossings of recovered
    // line/circle supports are refined before exact arcs are stored again.
    for i in 0..n {
        if let (Some(a), Some(b)) = (classes[(i + n - 1) % n], classes[i]) {
            if a == b || (sources[a].arc.is_none() && sources[b].arc.is_none()) {
                continue;
            }
            let support = |s: &SourceEdge| Support {
                a: s.a,
                b: s.b,
                circle: s.arc.and_then(|arc| {
                    geometry::arc_center_sweep(s.a, s.b, arc).map(|(c, _)| (c, arc.radius))
                }),
            };
            let admissible = |q: Point, s: &SourceEdge| {
                s.arc.map_or_else(
                    || geometry::segment_distance(q, s.a, s.b) < 1e-3,
                    |arc| on_arc(q, s.a, s.b, arc, (arc.radius * 1e-5).max(1e-4)),
                )
            };
            if let Some(q) = intersections(support(&sources[a]), support(&sources[b]))
                .into_iter()
                .filter(|q| admissible(*q, &sources[a]) && admissible(*q, &sources[b]))
                .min_by(|a, b| a.distance(points[i]).total_cmp(&b.distance(points[i])))
            {
                let error = sources[a]
                    .arc
                    .into_iter()
                    .chain(sources[b].arc)
                    .map(|a| a.radius * (1. - (std::f32::consts::PI / 72.).cos()))
                    .fold(0.001, f32::max);
                if q.distance(points[i]) <= error * 2. + 0.001 {
                    points[i] = q;
                }
            }
        }
    }
    // Start at a support change, so retained circular runs never wrap the seam.
    if let Some(start) = (0..n).find(|i| classes[*i] != classes[(*i + n - 1) % n]) {
        points.rotate_left(start);
        classes.rotate_left(start);
    }
    let mut result = SlabContour::default();
    let mut i = 0;
    while i < n {
        let mut end = i + 1;
        if classes[i].is_some() {
            while end < n && classes[end] == classes[i] {
                end += 1;
            }
        }
        result.points.push(points[i]);
        let mut restored = None;
        let mut setting = SlabEdgeSettings::default();
        if let Some(source) = classes[i].map(|j| &sources[j]) {
            setting = source.settings.clone();
            if let Some(mut arc) = source.arc
                && let Some((center, _)) = geometry::arc_center_sweep(source.a, source.b, arc)
            {
                let mut sweep = 0.;
                for k in i..end {
                    let a = sub(points[k], center);
                    let b = sub(points[(k + 1) % n], center);
                    sweep += cross(a, b).atan2(a.x * b.x + a.y * b.y);
                }
                arc.clockwise = sweep < 0.;
                arc.large_arc = sweep.abs() > std::f32::consts::PI;
                restored = Some(arc);
            }
        }
        result.arcs.push(restored);
        result.edge_settings.push(setting);
        i = end;
    }
    // A complete circular ring may have one support; retain at least three editable anchors.
    if result.points.len() < 3 {
        return SlabContour {
            arcs: (0..n)
                .map(|i| {
                    classes[i].and_then(|j| {
                        let s = &sources[j];
                        s.arc.map(|mut arc| {
                            let (c, _) = geometry::arc_center_sweep(s.a, s.b, arc).unwrap();
                            arc.clockwise =
                                cross(sub(points[i], c), sub(points[(i + 1) % n], c)) < 0.;
                            arc.large_arc = false;
                            arc
                        })
                    })
                })
                .collect(),
            points,
            ..Default::default()
        };
    }
    result
}
fn boolean(c: &SlabContour, points: &[Point], subtract: bool) -> Result<Vec<SlabContour>, String> {
    use geo::{Area, Intersects};
    validate(points)?;
    let original = polygon_contour(c);
    let operand = Polygon::new(line_string(points), vec![]);
    if !original.intersects(&operand) {
        return Err(if subtract {
            "No elements changed"
        } else {
            "Invalid edit parameters"
        }
        .into());
    }
    let changed = if subtract {
        original.difference(&operand)
    } else {
        original.union(&operand)
    };
    if (changed.unsigned_area() - original.unsigned_area()).abs() < 1e-8 {
        return Err("No elements changed".into());
    }
    if !subtract && changed.0.len() > 1 {
        return invalid();
    }
    let mut sources = source_edges(c);
    sources.extend((0..points.len()).map(|i| SourceEdge {
        a: points[i],
        b: points[(i + 1) % points.len()],
        arc: None,
        settings: SlabEdgeSettings::default(),
    }));
    let mut result = Vec::new();
    for poly in changed.0 {
        let mut contour = restore_ring(poly.exterior(), &sources);
        contour.holes = poly
            .interiors()
            .iter()
            .map(|line| restore_ring(line, &sources))
            .collect();
        contour.elevation = c.elevation;
        validate_contour(&contour)?;
        result.push(contour);
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        editor::Editor,
        geometry,
        model::{Material, Project},
        reshape::ReshapeCommand,
    };
    fn fixture() -> (Editor, u64) {
        let mut p = Project::default();
        let id = p.add(
            0,
            Material::Concrete,
            Shape::Slab {
                min: Point::new(0., 0.),
                max: Point::new(4., 3.),
                thickness: 0.3,
            },
        );
        (Editor::new(p), id)
    }
    #[test]
    fn slab_insert_concave_candidate_undo_redo_and_roundtrip() {
        let (mut editor, id) = fixture();
        let before = editor.project.clone();
        let command = ReshapeCommand::InsertSlabNode {
            id,
            edge: 2,
            point: Point::new(2., 2.),
        };
        let (candidate, _) = crate::reshape::candidate(&before, &command).unwrap();
        assert_eq!(editor.project, before);
        assert_eq!(candidate.elements[0].area(), 10.);
        let poly = outline(&candidate.elements[0]);
        assert_eq!(poly.len(), 5);
        let triangles = geometry::triangulate_polygon(&poly);
        assert_eq!(triangles.len(), 3);
        assert!((triangles.iter().map(|t| area(t)).sum::<f32>() - 10.).abs() < 1e-5);
        assert_eq!(
            geometry::hit_plan(&candidate, 0, Point::new(2., 2.8), 0.01),
            None
        );
        editor.reshape(command).unwrap();
        let edited = editor.project.clone();
        assert_eq!(edited.elements[0].id, id);
        assert_eq!(edited.elements[0].thickness(), 0.3);
        editor.undo();
        assert_eq!(editor.project, before);
        editor.redo();
        assert_eq!(editor.project, edited);
        let serialized = serde_json::to_string(&edited).unwrap();
        let loaded: Project = serde_json::from_str(&serialized).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded, edited);
    }
    #[test]
    fn slab_all_four_edges_including_closure_and_invalid_atomicity() {
        for (edge, point) in [
            (0, Point::new(2., 1.)),
            (1, Point::new(3., 1.5)),
            (2, Point::new(2., 2.)),
            (3, Point::new(1., 1.5)),
        ] {
            let (mut e, id) = fixture();
            e.reshape(ReshapeCommand::InsertSlabNode { id, edge, point })
                .unwrap();
            assert_eq!(outline(&e.project.elements[0]).len(), 5);
            e.project.validate().unwrap();
        }
        let (mut e, id) = fixture();
        let original = e.project.clone();
        for (edge, point) in [
            (0, Point::new(0., 0.)),
            (0, Point::new(2., 4.)),
            (5, Point::new(2., 1.)),
            (0, Point::new(f32::NAN, 0.)),
        ] {
            assert!(
                e.reshape(ReshapeCommand::InsertSlabNode { id, edge, point })
                    .is_err()
            );
            assert_eq!(e.project, original);
        }
        e.project.edit_state.locked.insert(id);
        assert!(
            e.reshape(ReshapeCommand::InsertSlabNode {
                id,
                edge: 0,
                point: Point::new(2., 1.)
            })
            .is_err()
        );
    }
    #[test]
    fn legacy_rectangle_and_translation_keep_contour_consistent() {
        let (mut e, id) = fixture();
        let mut old = serde_json::to_value(&e.project).unwrap();
        old["elements"][0]
            .as_object_mut()
            .unwrap()
            .remove("slab_contour");
        let legacy: Project = serde_json::from_value(old).unwrap();
        assert_eq!(outline(&legacy.elements[0]).len(), 4);
        assert_eq!(legacy.elements[0].area(), 12.);
        e.reshape(ReshapeCommand::InsertSlabNode {
            id,
            edge: 0,
            point: Point::new(2., 1.),
        })
        .unwrap();
        let a = e.project.elements[0].area();
        e.project.elements[0].translate(Point::new(10., -4.));
        e.project.validate().unwrap();
        assert_eq!(e.project.elements[0].area(), a);
    }
}

#[cfg(test)]
mod transform_tests {
    use super::*;
    use crate::{
        editing::EditCommand,
        editor::Editor,
        geometry,
        model::{Material, Project},
    };
    #[test]
    fn slab_contour_transform_and_3d_pick_use_actual_polygon() {
        let mut p = Project::default();
        let id = p.add(
            0,
            Material::Concrete,
            Shape::Slab {
                min: Point::new(0., 0.),
                max: Point::new(4., 3.),
                thickness: 0.3,
            },
        );
        insert_node(&mut p.elements[0], 2, Point::new(2., 2.)).unwrap();
        let camera = geometry::Camera {
            yaw: 0.,
            pitch: std::f32::consts::FRAC_PI_2 - 0.001,
            span: 10.,
            target: glam::Vec3::new(2., 1.5, 0.),
        };
        for (point, expected) in [(Point::new(2., 2.8), None), (Point::new(1., 1.), Some(id))] {
            let projected = camera
                .matrix(1.)
                .project_point3(glam::Vec3::new(point.x, point.y, 0.));
            assert_eq!(
                camera.pick(&p, [projected.x, projected.y], 1., Some(0)),
                expected
            );
        }
        let mut e = Editor::new(p);
        e.set_selection(vec![id]);
        let original = e.project.clone();
        e.edit(EditCommand::Transform {
            offset: Point::new(5., -2.),
            origin: Point::default(),
            angle: 37.,
            scale: 2.,
            mirror: true,
            copies: 0,
        })
        .unwrap();
        e.project.validate().unwrap();
        assert!((e.project.elements[0].area() - 40.).abs() < 1e-4);
        assert_eq!(outline(&e.project.elements[0]).len(), 5);
        e.undo();
        assert_eq!(e.project, original);
    }
}

#[cfg(test)]
mod full_kernel_tests {
    use super::*;
    use crate::model::{Material, Project};
    fn slab() -> Element {
        let mut p = Project::default();
        p.add(
            0,
            Material::Concrete,
            Shape::Slab {
                min: Point::new(0., 0.),
                max: Point::new(4., 3.),
                thickness: 0.3,
            },
        );
        p.elements.remove(0)
    }
    fn apply(e: &mut Element, index: usize, change: SlabChange, a: Point, p: Point) {
        let result = changed(e, index, &change, a, p).unwrap();
        assert_eq!(result.len(), 1);
        sync(e, result[0].clone()).unwrap();
    }
    fn rectangle(a: Point, b: Point) -> Vec<Point> {
        vec![a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)]
    }
    #[test]
    fn slab_kernel_exact_curve_tangent_straighten_and_roundtrip() {
        let mut e = slab();
        let original = e.clone();
        apply(
            &mut e,
            1,
            SlabChange::Curve,
            Point::new(4., 1.5),
            Point::new(5., 1.5),
        );
        let c = contour(&e);
        assert_eq!(c.points.len(), 4);
        assert!(c.arcs[1].is_some());
        assert!(actual_area(&e) > 12.);
        assert!((contour_bounds(&c).1.x - 5.).abs() < 1e-4);
        assert!(outline(&e).len() > 4);
        let json = serde_json::to_string(&e).unwrap();
        let round: Element = serde_json::from_str(&json).unwrap();
        assert_eq!(round, e);
        validate_contour(&contour(&round)).unwrap();
        apply(
            &mut e,
            1,
            SlabChange::Tangent,
            Point::new(4., 0.1),
            Point::new(5., 0.),
        );
        assert_eq!(contour(&e).points, c.points);
        assert!(contour(&e).arcs[1].is_some());
        apply(
            &mut e,
            1,
            SlabChange::InsertNode,
            Point::new(4., 1.),
            Point::new(9., 9.),
        );
        assert_eq!(contour(&e).points, contour(&original).points);
        assert_eq!(contour(&e).arcs[1], None);
        assert!((actual_area(&e) - 12.).abs() < 1e-5);
    }
    #[test]
    fn slab_kernel_single_edge_and_all_offsets_preserve_closed_ring() {
        let mut e = slab();
        apply(
            &mut e,
            1,
            SlabChange::OffsetEdge,
            Point::new(4., 1.5),
            Point::new(5., 1.5),
        );
        assert!((actual_area(&e) - 15.).abs() < 1e-5);
        assert_eq!(contour(&e).points[1], Point::new(5., 0.));
        assert_eq!(contour(&e).points[2], Point::new(5., 3.));
        let mut e = slab();
        apply(
            &mut e,
            1,
            SlabChange::OffsetAll,
            Point::new(4., 1.5),
            Point::new(5., 1.5),
        );
        assert!((actual_area(&e) - 30.).abs() < 1e-5);
        assert_eq!(
            contour_bounds(&contour(&e)),
            (Point::new(-1., -1.), Point::new(5., 4.))
        );
    }
    #[test]
    fn slab_kernel_boolean_hole_split_union_and_caps() {
        let mut e = slab();
        apply(
            &mut e,
            0,
            SlabChange::SubtractPolygon(rectangle(Point::new(1., 1.), Point::new(2., 2.))),
            Point::default(),
            Point::default(),
        );
        assert_eq!(contour(&e).holes.len(), 1);
        assert!((actual_area(&e) - 11.).abs() < 1e-5);
        let cap = triangles(&e);
        assert!((cap.iter().map(|t| area(t)).sum::<f32>() - 11.).abs() < 1e-4);
        assert!(
            cap.iter()
                .all(|t| !geometry::point_in_polygon(Point::new(1.5, 1.5), t))
        );
        let parts = changed(
            &slab(),
            0,
            &SlabChange::SubtractPolygon(rectangle(Point::new(1.5, -1.), Point::new(2.5, 4.))),
            Point::default(),
            Point::default(),
        )
        .unwrap();
        assert_eq!(parts.len(), 2);
        assert!((parts.iter().map(ring_area).sum::<f32>() - 9.).abs() < 1e-4);
        let mut e = slab();
        apply(
            &mut e,
            0,
            SlabChange::AddPolygon(rectangle(Point::new(3., 1.), Point::new(5., 2.))),
            Point::default(),
            Point::default(),
        );
        assert!((actual_area(&e) - 13.).abs() < 1e-4);
        assert!(
            changed(
                &e,
                0,
                &SlabChange::AddPolygon(rectangle(Point::new(10., 10.), Point::new(11., 11.))),
                Point::default(),
                Point::default()
            )
            .is_err()
        );
    }
    #[test]
    fn slab_kernel_boolean_preserves_unaffected_exact_arc() {
        let mut e = slab();
        apply(
            &mut e,
            1,
            SlabChange::Curve,
            Point::new(4., 1.5),
            Point::new(5., 1.5),
        );
        let arc = contour(&e).arcs[1].unwrap();
        apply(
            &mut e,
            0,
            SlabChange::SubtractPolygon(rectangle(Point::new(0.5, 1.), Point::new(1.5, 2.))),
            Point::default(),
            Point::default(),
        );
        assert_eq!(contour(&e).points.len(), 4);
        assert!(
            contour(&e)
                .arcs
                .iter()
                .flatten()
                .any(|a| (a.radius - arc.radius).abs() < 1e-5)
        );
        assert_eq!(contour(&e).holes.len(), 1);
    }
    #[test]
    fn slab_kernel_insert_arc_node_preserves_exact_circle_and_area() {
        let mut e = slab();
        apply(
            &mut e,
            1,
            SlabChange::Curve,
            Point::new(4., 1.5),
            Point::new(5., 1.5),
        );
        let before = actual_area(&e);
        let radius = contour(&e).arcs[1].unwrap().radius;
        apply(
            &mut e,
            1,
            SlabChange::InsertArcNode,
            Point::new(5.02, 1.5),
            Point::new(5.02, 1.5),
        );
        let c = contour(&e);
        assert_eq!(c.points.len(), 5);
        assert!((actual_area(&e) - before).abs() < 1e-4);
        assert!((c.points[2].x - 5.).abs() < 1e-4);
        assert_eq!(c.arcs[1].unwrap().radius, radius);
        assert_eq!(c.arcs[2].unwrap().radius, radius);
    }
    #[test]
    fn slab_kernel_sloped_bottom_preserves_top_and_angles() {
        for (angle, low, high) in [(45., 0.3, 3.7), (135., -0.3, 4.3)] {
            let mut e = slab();
            apply(
                &mut e,
                0,
                SlabChange::SetEdge {
                    angle_degrees: angle,
                    surface: None,
                    scope: SlabEdgeScope::All,
                },
                Point::default(),
                Point::default(),
            );
            let top = contour(&e);
            let bottom = bottom_contour(&e).unwrap();
            let (a, b) = contour_bounds(&bottom);
            assert!((a.x - low).abs() < 1e-4 && (b.x - high).abs() < 1e-4);
            assert_eq!(contour(&e), top);
            let solid = solid_triangles(&e, 2.).unwrap();
            assert!(
                solid
                    .iter()
                    .flatten()
                    .all(|v| (v.z - 2.).abs() < 1e-5 || (v.z - 1.7).abs() < 1e-5)
            );
        }
        let mut e = slab();
        apply(
            &mut e,
            1,
            SlabChange::SetEdge {
                angle_degrees: 45.,
                surface: None,
                scope: SlabEdgeScope::Clicked,
            },
            Point::default(),
            Point::default(),
        );
        let (a, b) = contour_bounds(&bottom_contour(&e).unwrap());
        assert_eq!(a, Point::new(0., 0.));
        assert!((b.x - 3.7).abs() < 1e-4 && (b.y - 3.).abs() < 1e-4);
        apply(
            &mut e,
            1,
            SlabChange::Curve,
            Point::new(4., 1.5),
            Point::new(4.5, 1.5),
        );
        let top = contour(&e);
        let bottom = bottom_contour(&e).unwrap();
        assert!(
            (bottom.arcs[1].unwrap().radius - (top.arcs[1].unwrap().radius - 0.3)).abs() < 1e-4
        );
        let mut hole = slab();
        apply(
            &mut hole,
            0,
            SlabChange::SubtractPolygon(rectangle(Point::new(1., 1.), Point::new(2., 2.))),
            Point::default(),
            Point::default(),
        );
        apply(
            &mut hole,
            4,
            SlabChange::SetEdge {
                angle_degrees: 45.,
                surface: None,
                scope: SlabEdgeScope::Ring,
            },
            Point::default(),
            Point::default(),
        );
        let bottom = bottom_contour(&hole).unwrap();
        let (a, b) = contour_bounds(&bottom.holes[0]);
        assert!((a.x - 0.7).abs() < 1e-4 && (b.x - 2.3).abs() < 1e-4);
    }
    #[test]
    fn slab_kernel_hole_indices_scopes_and_transform_preserve_metadata() {
        let mut e = slab();
        apply(
            &mut e,
            0,
            SlabChange::SubtractPolygon(rectangle(Point::new(1., 1.), Point::new(2., 2.))),
            Point::default(),
            Point::default(),
        );
        apply(
            &mut e,
            4,
            SlabChange::SetEdge {
                angle_degrees: 45.,
                surface: Some(Material::Timber),
                scope: SlabEdgeScope::Ring,
            },
            Point::default(),
            Point::default(),
        );
        let c = contour(&e);
        assert!(
            settings(&c)
                .iter()
                .all(|s| *s == SlabEdgeSettings::default())
        );
        assert!(
            c.holes[0]
                .edge_settings
                .iter()
                .all(|s| s.angle_degrees == 45. && s.surface == Some(Material::Timber))
        );
        apply(
            &mut e,
            0,
            SlabChange::Elevate(0.5),
            Point::default(),
            Point::default(),
        );
        let area = actual_area(&e);
        transform(&mut e, |p| Point::new(-p.x * 2. + 10., p.y * 2.), 2., true);
        validate_contour(&contour(&e)).unwrap();
        assert!((actual_area(&e) - area * 4.).abs() < 1e-4);
        assert_eq!(contour(&e).elevation, 0.5);
        assert_eq!(contour(&e).holes[0].edge_settings, c.holes[0].edge_settings);
    }
}
