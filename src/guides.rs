//! Typed drafting aids. Distances are metres; paths retain exact circular arcs.
use crate::geometry::{arc_center_sweep, footprint};
use crate::model::{ArcSegment, Element, Point, Project, Shape};
use serde::{Deserialize, Serialize};

const EPS: f32 = 1e-5;
const MAX_MARKERS: usize = 2048;
const WORLD_LIMIT: f32 = 100_000.;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapMethod {
    #[default]
    Half,
    Divisions,
    Percent,
    Distance,
    BestDivision,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapScope {
    #[default]
    BetweenNodes,
    BetweenIntersections,
    EntireElement,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapOrigin {
    #[default]
    Endpoint,
    Midpoint,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceReference {
    #[default]
    Edges,
    ReferenceLines,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DoorReference {
    #[default]
    Center,
    Side,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GuideSettings {
    pub snap_enabled: bool,
    pub guide_lines_enabled: bool,
    pub distance_guides_enabled: bool,
    pub method: SnapMethod,
    pub divisions: u16,
    pub percent: f32,
    pub distance: f32,
    pub scope: SnapScope,
    pub origin: SnapOrigin,
    pub distance_reference: DistanceReference,
    pub door_reference: DoorReference,
}
impl Default for GuideSettings {
    fn default() -> Self {
        Self {
            snap_enabled: true,
            guide_lines_enabled: true,
            distance_guides_enabled: true,
            method: SnapMethod::Half,
            divisions: 3,
            percent: 20.,
            distance: 1.,
            scope: SnapScope::BetweenNodes,
            origin: SnapOrigin::Endpoint,
            distance_reference: DistanceReference::Edges,
            door_reference: DoorReference::Center,
        }
    }
}
impl GuideSettings {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !(3..=100).contains(&self.divisions) {
            return Err("Snap divisions must be between 3 and 100");
        }
        if !self.percent.is_finite() || self.percent <= 0. || self.percent >= 100. {
            return Err("Snap percentage must be greater than 0 and less than 100");
        }
        if !self.distance.is_finite() || self.distance <= 0. {
            return Err("Snap distance must be positive and finite");
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum GuidePrimitive {
    Segment {
        start: Point,
        end: Point,
    },
    Line {
        origin: Point,
        direction: Point,
    },
    Circle {
        center: Point,
        radius: f32,
    },
    Arc {
        start: Point,
        end: Point,
        arc: ArcSegment,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PersistentGuide {
    pub story: u32,
    pub primitive: GuidePrimitive,
}
impl GuidePrimitive {
    pub fn validate(&self) -> Result<(), &'static str> {
        match *self {
            Self::Segment { start, end }
                if world_point(start) && world_point(end) && start.distance(end) > EPS =>
            {
                Ok(())
            }
            Self::Line { origin, direction }
                if world_point(origin)
                    && direction.finite()
                    && norm(direction).is_finite()
                    && norm(direction) > EPS =>
            {
                Ok(())
            }
            Self::Circle { center, radius }
                if world_point(center)
                    && radius.is_finite()
                    && radius > EPS
                    && radius <= WORLD_LIMIT =>
            {
                Ok(())
            }
            Self::Arc { start, end, arc }
                if world_point(start)
                    && world_point(end)
                    && start.distance(end) > EPS
                    && arc.radius.is_finite()
                    && arc.radius > EPS
                    && arc.radius <= WORLD_LIMIT
                    && arc_center_sweep(start, end, arc).is_some_and(|(center, sweep)| {
                        world_point(center) && sweep.is_finite() && sweep.abs() > EPS
                    }) =>
            {
                Ok(())
            }
            _ => Err("Guide geometry must be finite, nondegenerate and within 100000 metres"),
        }
    }

    /// Signed left offset of lines/segments, outward radial offset of circles/arcs.
    /// Returns a validated value; callers decide preview/commit history.
    pub fn offset(&self, distance: f32) -> Result<Self, &'static str> {
        self.validate()?;
        if !distance.is_finite() {
            return Err("Guide offset must be finite");
        }
        let result = match *self {
            Self::Line { origin, direction } => {
                let length = norm(direction);
                let delta = Point::new(
                    -direction.y / length * distance,
                    direction.x / length * distance,
                );
                Self::Line {
                    origin: add(origin, delta),
                    direction,
                }
            }
            Self::Segment { start, end } => {
                let direction = sub(end, start);
                let length = norm(direction);
                let delta = Point::new(
                    -direction.y / length * distance,
                    direction.x / length * distance,
                );
                Self::Segment {
                    start: add(start, delta),
                    end: add(end, delta),
                }
            }
            Self::Circle { center, radius } => Self::Circle {
                center,
                radius: radius + distance,
            },
            Self::Arc {
                start,
                end,
                mut arc,
            } => {
                let (center, _) = arc_center_sweep(start, end, arc).ok_or("Invalid guide arc")?;
                let ratio = (arc.radius + distance) / arc.radius;
                arc.radius += distance;
                Self::Arc {
                    start: add(center, scale(sub(start, center), ratio)),
                    end: add(center, scale(sub(end, center), ratio)),
                    arc,
                }
            }
        };
        result.validate()?;
        Ok(result)
    }

    /// Closest point on a guide. The centre of a circle chooses its positive X pole.
    pub fn project(&self, cursor: Point) -> Option<Point> {
        if !cursor.finite() || self.validate().is_err() {
            return None;
        }
        match *self {
            Self::Segment { start, end } => {
                Some(project_line(cursor, start, sub(end, start), true))
            }
            Self::Line { origin, direction } => {
                Some(project_line(cursor, origin, direction, false))
            }
            Self::Circle { center, radius } => {
                let d = sub(cursor, center);
                let length = norm(d);
                Some(add(
                    center,
                    if length > EPS {
                        scale(d, radius / length)
                    } else {
                        Point::new(radius, 0.)
                    },
                ))
            }
            Self::Arc { start, end, arc } => Some(
                GuideEdge {
                    start,
                    end,
                    arc: Some(arc),
                }
                .project(cursor)
                .0,
            ),
        }
    }

    pub fn intersections(&self, other: &Self) -> Vec<Point> {
        if self.validate().is_err() || other.validate().is_err() {
            return vec![];
        }
        primitive_intersections(self.supporting_primitive(), other.supporting_primitive())
            .into_iter()
            .filter(|point| self.contains_on_arc(*point) && other.contains_on_arc(*point))
            .collect()
    }

    fn supporting_primitive(&self) -> Self {
        match *self {
            Self::Arc { start, end, arc } => {
                // Called only after validation, so the circular support exists.
                let (center, _) = arc_center_sweep(start, end, arc).expect("validated guide arc");
                Self::Circle {
                    center,
                    radius: arc.radius,
                }
            }
            primitive => primitive,
        }
    }

    fn contains_on_arc(&self, point: Point) -> bool {
        match *self {
            Self::Arc { start, end, arc } => {
                GuideEdge {
                    start,
                    end,
                    arc: Some(arc),
                }
                .project(point)
                .0
                .distance(point)
                    <= EPS * 8.
            }
            _ => true,
        }
    }
}

/// Prefer nearby isolated intersections, then the nearest projection.
pub fn nearest_guide(
    guides: &[PersistentGuide],
    story: u32,
    cursor: Point,
    tolerance: f32,
) -> Option<Point> {
    if !cursor.finite() || !tolerance.is_finite() || tolerance < 0. {
        return None;
    }
    let nearby: Vec<_> = guides
        .iter()
        .filter(|g| g.story == story)
        .filter_map(|g| g.primitive.project(cursor).map(|p| (g.primitive, p)))
        .filter(|(_, p)| p.distance(cursor) <= tolerance)
        .collect();
    let mut intersections = vec![];
    for (index, (a, _)) in nearby.iter().enumerate() {
        for (b, _) in nearby.iter().skip(index + 1) {
            intersections.extend(
                a.intersections(b)
                    .into_iter()
                    .filter(|p| p.distance(cursor) <= tolerance),
            );
        }
    }
    intersections
        .into_iter()
        .min_by(|a, b| a.distance(cursor).total_cmp(&b.distance(cursor)))
        .or_else(|| {
            nearby
                .into_iter()
                .map(|(_, p)| p)
                .min_by(|a, b| a.distance(cursor).total_cmp(&b.distance(cursor)))
        })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuideEdge {
    pub start: Point,
    pub end: Point,
    pub arc: Option<ArcSegment>,
}
impl GuideEdge {
    pub fn line(start: Point, end: Point) -> Self {
        Self {
            start,
            end,
            arc: None,
        }
    }
    pub fn length(&self) -> f32 {
        self.circle().map_or_else(
            || self.start.distance(self.end),
            |(_, radius, sweep)| radius * sweep.abs(),
        )
    }
    fn circle(&self) -> Option<(Point, f32, f32)> {
        let arc = self.arc?;
        let (center, sweep) = arc_center_sweep(self.start, self.end, arc)?;
        Some((center, arc.radius, sweep))
    }
    pub fn point_at(&self, distance: f32) -> Point {
        let t = (distance / self.length().max(EPS)).clamp(0., 1.);
        if t == 0. {
            return self.start;
        }
        if t == 1. {
            return self.end;
        }
        if let Some((center, radius, sweep)) = self.circle() {
            let angle = (self.start.y - center.y).atan2(self.start.x - center.x) + sweep * t;
            add(
                center,
                Point::new(radius * angle.cos(), radius * angle.sin()),
            )
        } else {
            add(self.start, scale(sub(self.end, self.start), t))
        }
    }
    /// Return closest point and distance along this edge.
    pub fn project(&self, cursor: Point) -> (Point, f32) {
        if let Some((center, radius, sweep)) = self.circle() {
            let angle = (cursor.y - center.y).atan2(cursor.x - center.x);
            let start_angle = (self.start.y - center.y).atan2(self.start.x - center.x);
            let progress = if sweep > 0. {
                (angle - start_angle).rem_euclid(std::f32::consts::TAU)
            } else {
                (start_angle - angle).rem_euclid(std::f32::consts::TAU)
            };
            if progress <= sweep.abs() + EPS {
                let along = (progress * radius).min(self.length());
                return (self.point_at(along), along);
            }
            if cursor.distance(self.start) <= cursor.distance(self.end) {
                (self.start, 0.)
            } else {
                (self.end, self.length())
            }
        } else {
            let p = project_line(cursor, self.start, sub(self.end, self.start), true);
            (p, self.start.distance(p))
        }
    }
    fn supporting_primitive(&self) -> GuidePrimitive {
        self.circle().map_or(
            GuidePrimitive::Segment {
                start: self.start,
                end: self.end,
            },
            |(center, radius, _)| GuidePrimitive::Circle { center, radius },
        )
    }
    pub fn intersections(&self, other: &Self) -> Vec<Point> {
        self.supporting_primitive()
            .intersections(&other.supporting_primitive())
            .into_iter()
            .filter(|p| {
                self.project(*p).0.distance(*p) <= EPS * 8.
                    && other.project(*p).0.distance(*p) <= EPS * 8.
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GuidePath {
    pub edges: Vec<GuideEdge>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathProjection {
    pub point: Point,
    pub distance: f32,
    pub along: f32,
    pub edge_index: usize,
}
impl GuidePath {
    pub fn length(&self) -> f32 {
        self.edges.iter().map(GuideEdge::length).sum()
    }
    pub fn point_at(&self, distance: f32) -> Option<Point> {
        if !distance.is_finite() {
            return None;
        }
        let mut along = distance.max(0.);
        for edge in &self.edges {
            let length = edge.length();
            if along <= length {
                return Some(edge.point_at(along));
            }
            along -= length;
        }
        self.edges.last().map(|edge| edge.end)
    }
    pub fn project(&self, cursor: Point) -> Option<PathProjection> {
        if !cursor.finite() {
            return None;
        }
        let mut offset = 0.;
        let mut best: Option<PathProjection> = None;
        for (edge_index, edge) in self.edges.iter().enumerate() {
            let (point, along) = edge.project(cursor);
            let distance = point.distance(cursor);
            if distance.is_finite() && best.is_none_or(|p| distance < p.distance) {
                best = Some(PathProjection {
                    point,
                    distance,
                    along: offset + along,
                    edge_index,
                });
            }
            offset += edge.length();
        }
        best
    }
}

fn perimeter(points: &[Point]) -> GuidePath {
    GuidePath {
        edges: (0..points.len())
            .map(|i| GuideEdge::line(points[i], points[(i + 1) % points.len()]))
            .collect(),
    }
}
pub fn element_paths(element: &Element, reference: DistanceReference) -> Vec<GuidePath> {
    let path = match &element.shape {
        Shape::Wall { start, end, .. } if reference == DistanceReference::ReferenceLines => {
            GuidePath {
                edges: vec![GuideEdge::line(*start, *end)],
            }
        }
        Shape::Polyline { points, arcs } => GuidePath {
            edges: points
                .windows(2)
                .enumerate()
                .map(|(i, pair)| GuideEdge {
                    start: pair[0],
                    end: pair[1],
                    arc: arcs.get(i).and_then(|arc| *arc),
                })
                .collect(),
        },
        _ => perimeter(&footprint(element)),
    };
    if path.edges.is_empty() || !path.length().is_finite() || path.length() <= EPS {
        vec![]
    } else {
        vec![path]
    }
}

/// Snap markers for the nearest hovered element path on one story.
/// The caller may retain these briefly after hover to provide UI snap-point lifetime.
pub fn snap_candidates(
    project: &Project,
    story: u32,
    cursor: Point,
    tolerance: f32,
    settings: &GuideSettings,
) -> Vec<Point> {
    if !settings.snap_enabled
        || settings.validate().is_err()
        || !cursor.finite()
        || !tolerance.is_finite()
        || tolerance < 0.
    {
        return vec![];
    }
    let paths: Vec<(u64, GuidePath)> = project
        .elements
        .iter()
        .filter(|e| e.story == story)
        .flat_map(|e| {
            element_paths(e, DistanceReference::ReferenceLines)
                .into_iter()
                .map(move |p| (e.id, p))
        })
        .collect();
    let Some((index, projection)) = paths
        .iter()
        .enumerate()
        .filter_map(|(i, (_, p))| p.project(cursor).map(|v| (i, v)))
        .filter(|(_, p)| p.distance <= tolerance)
        .min_by(|a, b| a.1.distance.total_cmp(&b.1.distance))
    else {
        return vec![];
    };
    let (element_id, path) = &paths[index];
    let (start, end) = match settings.scope {
        SnapScope::EntireElement => (0., path.length()),
        SnapScope::BetweenNodes => {
            let start: f32 = path.edges[..projection.edge_index]
                .iter()
                .map(GuideEdge::length)
                .sum();
            (start, start + path.edges[projection.edge_index].length())
        }
        SnapScope::BetweenIntersections => {
            let mut breaks = vec![0., path.length()];
            let mut offset = 0.;
            for edge in &path.edges {
                for (other_id, other) in &paths {
                    if other_id == element_id {
                        continue;
                    }
                    for other_edge in &other.edges {
                        for point in edge.intersections(other_edge) {
                            breaks.push(offset + edge.project(point).1);
                        }
                    }
                }
                offset += edge.length();
            }
            breaks.sort_by(f32::total_cmp);
            breaks.dedup_by(|a, b| (*a - *b).abs() < EPS);
            let interval = breaks
                .windows(2)
                .find(|pair| projection.along < pair[1] - EPS)
                .or_else(|| breaks.windows(2).last());
            interval.map_or((0., path.length()), |p| (p[0], p[1]))
        }
    };
    let nearer_end = match (path.point_at(start), path.point_at(end)) {
        (Some(a), Some(b)) => cursor.distance(b) < cursor.distance(a),
        _ => false,
    };
    interval_from_origin(end - start, projection.along - start, nearer_end, settings)
        .into_iter()
        .filter_map(|along| path.point_at(start + along))
        .collect()
}

/// Interval markers as distances from a path interval's start.
pub fn interval_distances(length: f32, cursor_along: f32, settings: &GuideSettings) -> Vec<f32> {
    interval_from_origin(length, cursor_along, cursor_along > length * 0.5, settings)
}

fn interval_from_origin(
    length: f32,
    cursor_along: f32,
    nearer_end: bool,
    settings: &GuideSettings,
) -> Vec<f32> {
    if !length.is_finite()
        || length <= EPS
        || !cursor_along.is_finite()
        || settings.validate().is_err()
    {
        return vec![];
    }
    let cursor = cursor_along.clamp(0., length);
    let evenly = |parts: usize| {
        (1..parts.min(MAX_MARKERS + 1))
            .map(|n| length * n as f32 / parts as f32)
            .collect::<Vec<_>>()
    };
    match settings.method {
        SnapMethod::Half => vec![length * 0.5],
        SnapMethod::Divisions => evenly(settings.divisions as usize),
        SnapMethod::BestDivision => {
            let ratio = length as f64 / settings.distance as f64;
            let low = ratio.floor().max(1.).min(usize::MAX as f64 - 1.) as usize;
            let high = low.saturating_add(1);
            let error =
                |parts: usize| (length as f64 / parts as f64 - settings.distance as f64).abs();
            let parts = if error(high) < error(low) { high } else { low };
            marker_window(length, cursor, length / parts as f32, 0.)
        }
        SnapMethod::Percent | SnapMethod::Distance => {
            let origin = if settings.origin == SnapOrigin::Midpoint {
                length * 0.5
            } else if nearer_end {
                length
            } else {
                0.
            };
            let direction = if settings.origin == SnapOrigin::Midpoint {
                if cursor < origin { -1. } else { 1. }
            } else if origin > 0. {
                -1.
            } else {
                1.
            };
            if settings.method == SnapMethod::Percent {
                let marker = origin + direction * length * settings.percent / 100.;
                if marker > EPS && marker < length - EPS {
                    vec![marker]
                } else {
                    vec![]
                }
            } else {
                let mut markers = marker_window(length, cursor, settings.distance, origin);
                if settings.origin == SnapOrigin::Endpoint {
                    markers.retain(|value| (*value - origin) * direction > EPS);
                }
                markers
            }
        }
    }
}

fn marker_window(length: f32, cursor: f32, spacing: f32, origin: f32) -> Vec<f32> {
    if !spacing.is_finite() || spacing <= 0. {
        return vec![];
    }
    let lower = ((-origin as f64 + EPS as f64) / spacing as f64).ceil();
    let upper = ((length as f64 - origin as f64 - EPS as f64) / spacing as f64).floor();
    let near = ((cursor as f64 - origin as f64) / spacing as f64).round();
    let first = if upper - lower + 1. > MAX_MARKERS as f64 {
        (near - (MAX_MARKERS / 2) as f64).clamp(lower, upper - MAX_MARKERS as f64 + 1.)
    } else {
        lower
    };
    let count = ((upper - first + 1.).max(0.) as usize).min(MAX_MARKERS);
    (0..count)
        .map(|i| (origin as f64 + (first + i as f64) * spacing as f64) as f32)
        .filter(|v| *v > EPS && *v < length - EPS)
        .collect()
}

fn add(a: Point, b: Point) -> Point {
    Point::new(a.x + b.x, a.y + b.y)
}
fn sub(a: Point, b: Point) -> Point {
    Point::new(a.x - b.x, a.y - b.y)
}
fn scale(a: Point, factor: f32) -> Point {
    Point::new(a.x * factor, a.y * factor)
}
fn dot(a: Point, b: Point) -> f32 {
    a.x * b.x + a.y * b.y
}
fn cross(a: Point, b: Point) -> f32 {
    a.x * b.y - a.y * b.x
}
fn norm(a: Point) -> f32 {
    a.x.hypot(a.y)
}
fn world_point(point: Point) -> bool {
    point.finite() && point.x.abs() < WORLD_LIMIT && point.y.abs() < WORLD_LIMIT
}
fn project_line(p: Point, origin: Point, direction: Point, bounded: bool) -> Point {
    // Compute in f64: finite but very large direction magnitudes must not overflow
    // the squared norm in an otherwise valid infinite line.
    let dx = direction.x as f64;
    let dy = direction.y as f64;
    let denominator = dx * dx + dy * dy;
    if denominator <= (EPS * EPS) as f64 {
        return origin;
    }
    let t =
        ((p.x as f64 - origin.x as f64) * dx + (p.y as f64 - origin.y as f64) * dy) / denominator;
    let t = if bounded { t.clamp(0., 1.) } else { t };
    Point::new(
        (origin.x as f64 + dx * t) as f32,
        (origin.y as f64 + dy * t) as f32,
    )
}
fn line_data(primitive: GuidePrimitive) -> Option<(Point, Point, bool)> {
    match primitive {
        GuidePrimitive::Segment { start, end } => Some((start, sub(end, start), true)),
        GuidePrimitive::Line { origin, direction } => {
            Some((origin, scale(direction, 1. / norm(direction)), false))
        }
        _ => None,
    }
}
fn primitive_intersections(a: GuidePrimitive, b: GuidePrimitive) -> Vec<Point> {
    if let (Some((p, r, bounded_a)), Some((q, s, bounded_b))) = (line_data(a), line_data(b)) {
        let divisor = cross(r, s);
        if divisor.abs() <= EPS * norm(r) * norm(s) {
            return vec![];
        }
        let t = cross(sub(q, p), s) / divisor;
        let u = cross(sub(q, p), r) / divisor;
        if (!bounded_a || (-EPS..=1. + EPS).contains(&t))
            && (!bounded_b || (-EPS..=1. + EPS).contains(&u))
        {
            return vec![add(p, scale(r, t))];
        }
        return vec![];
    }
    if let (
        GuidePrimitive::Circle {
            center: c0,
            radius: r0,
        },
        GuidePrimitive::Circle {
            center: c1,
            radius: r1,
        },
    ) = (a, b)
    {
        let delta = sub(c1, c0);
        let distance = norm(delta);
        if distance <= EPS || distance > r0 + r1 + EPS || distance < (r0 - r1).abs() - EPS {
            return vec![];
        }
        let along = (r0 * r0 - r1 * r1 + distance * distance) / (2. * distance);
        let height = (r0 * r0 - along * along).max(0.).sqrt();
        let middle = add(c0, scale(delta, along / distance));
        if height <= EPS {
            return vec![middle];
        }
        let offset = scale(Point::new(-delta.y, delta.x), height / distance);
        return vec![add(middle, offset), sub(middle, offset)];
    }
    let (line, circle) = if line_data(a).is_some() {
        (a, b)
    } else {
        (b, a)
    };
    let Some((origin, direction, bounded)) = line_data(line) else {
        return vec![];
    };
    let GuidePrimitive::Circle { center, radius } = circle else {
        return vec![];
    };
    let d = sub(origin, center);
    let aa = dot(direction, direction);
    let bb = 2. * dot(d, direction);
    let cc = dot(d, d) - radius * radius;
    let discriminant = bb * bb - 4. * aa * cc;
    if discriminant < -EPS {
        return vec![];
    }
    let root = discriminant.max(0.).sqrt();
    let mut points = vec![];
    for t in [(-bb - root) / (2. * aa), (-bb + root) / (2. * aa)] {
        if !bounded || (-EPS..=1. + EPS).contains(&t) {
            let point = add(origin, scale(direction, t));
            if points.iter().all(|p: &Point| p.distance(point) > EPS) {
                points.push(point);
            }
        }
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Material;
    fn p(x: f32, y: f32) -> Point {
        Point::new(x, y)
    }
    fn close(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-4, "{a} != {b}");
    }
    fn points(a: &[f32], b: &[f32]) {
        assert_eq!(a.len(), b.len(), "{a:?} != {b:?}");
        for (a, b) in a.iter().zip(b) {
            close(*a, *b);
        }
    }
    fn wall(project: &mut Project, a: Point, b: Point, story: u32) {
        project.add(
            story,
            Material::Concrete,
            Shape::Wall {
                start: a,
                end: b,
                thickness: 0.2,
                height: 3.,
            },
        );
    }
    #[test]
    fn settings_defaults_validation_and_roundtrip() {
        let mut s = GuideSettings::default();
        assert_eq!(s.validate(), Ok(()));
        assert_eq!(s.method, SnapMethod::Half);
        assert_eq!(s.divisions, 3);
        assert_eq!(s.percent, 20.);
        assert_eq!(s.distance, 1.);
        assert_eq!(serde_json::from_str::<GuideSettings>("{}").unwrap(), s);
        assert_eq!(
            serde_json::from_str::<GuideSettings>(&serde_json::to_string(&s).unwrap()).unwrap(),
            s
        );
        s.divisions = 2;
        assert!(s.validate().is_err());
        s.divisions = 101;
        assert!(s.validate().is_err());
        s.divisions = 3;
        s.percent = f32::NAN;
        assert!(s.validate().is_err());
        s.percent = 100.;
        assert!(s.validate().is_err());
        s.percent = 20.;
        s.distance = 0.;
        assert!(s.validate().is_err());
    }
    #[test]
    fn half_and_divisions_have_exact_expected_positions() {
        let mut s = GuideSettings::default();
        points(&interval_distances(12., 2., &s), &[6.]);
        s.method = SnapMethod::Divisions;
        s.divisions = 4;
        points(&interval_distances(12., 2., &s), &[3., 6., 9.]);
        assert!(interval_distances(0., 0., &s).is_empty());
    }
    #[test]
    fn percent_follows_nearest_endpoint_and_midpoint_side() {
        let mut s = GuideSettings {
            method: SnapMethod::Percent,
            ..Default::default()
        };
        points(&interval_distances(10., 1., &s), &[2.]);
        points(&interval_distances(10., 9., &s), &[8.]);
        s.origin = SnapOrigin::Midpoint;
        points(&interval_distances(10., 1., &s), &[3.]);
        points(&interval_distances(10., 9., &s), &[7.]);
        s.percent = 60.;
        assert!(interval_distances(10., 1., &s).is_empty());
    }
    #[test]
    fn distance_reverses_remainder_and_midpoint_generates_both_sides() {
        let mut s = GuideSettings {
            method: SnapMethod::Distance,
            distance: 3.,
            ..Default::default()
        };
        points(&interval_distances(10., 1., &s), &[3., 6., 9.]);
        points(&interval_distances(10., 9., &s), &[1., 4., 7.]);
        s.origin = SnapOrigin::Midpoint;
        points(&interval_distances(10., 1., &s), &[2., 5., 8.]);
    }
    #[test]
    fn best_division_minimizes_segment_length_error() {
        let s = GuideSettings {
            method: SnapMethod::BestDivision,
            ..Default::default()
        };
        points(
            &interval_distances(2.45, 1., &s),
            &[2.45 / 3., 2.45 * 2. / 3.],
        );
        assert!(interval_distances(0.4, 0.2, &s).is_empty());
    }
    #[test]
    fn marker_window_is_bounded_and_tracks_hover() {
        let s = GuideSettings {
            method: SnapMethod::Distance,
            distance: 0.001,
            ..Default::default()
        };
        let markers = interval_distances(100., 75., &s);
        assert_eq!(markers.len(), MAX_MARKERS);
        assert!(markers.iter().any(|v| (*v - 75.).abs() < 0.001));
        assert!(markers.iter().all(|v| *v > 0. && *v < 100.));
    }
    #[test]
    fn exact_arc_length_projection_and_interpolation() {
        let edge = GuideEdge {
            start: p(1., 0.),
            end: p(0., 1.),
            arc: Some(ArcSegment {
                radius: 1.,
                clockwise: false,
                large_arc: false,
            }),
        };
        close(edge.length(), std::f32::consts::FRAC_PI_2);
        let midpoint = edge.point_at(edge.length() / 2.);
        close(midpoint.x, 0.5_f32.sqrt());
        close(midpoint.y, 0.5_f32.sqrt());
        let (projected, along) = edge.project(p(2., 2.));
        close(projected.distance(midpoint), 0.);
        close(along, edge.length() / 2.);
        close(edge.project(p(-1., 0.)).0.distance(edge.end), 0.);
    }
    #[test]
    fn exact_arc_intersection_rejects_other_circle_branch() {
        let edge = GuideEdge {
            start: p(1., 0.),
            end: p(0., 1.),
            arc: Some(ArcSegment {
                radius: 1.,
                clockwise: false,
                large_arc: false,
            }),
        };
        let intersections = edge.intersections(&GuideEdge::line(p(-2., 0.5), p(2., 0.5)));
        assert_eq!(intersections.len(), 1);
        close(intersections[0].x, 0.75_f32.sqrt());
        close(intersections[0].y, 0.5);
    }
    #[test]
    fn entire_path_accumulates_exact_edges() {
        let path = GuidePath {
            edges: vec![
                GuideEdge::line(p(0., 0.), p(6., 0.)),
                GuideEdge::line(p(6., 0.), p(6., 2.)),
            ],
        };
        close(path.length(), 8.);
        assert_eq!(path.point_at(7.), Some(p(6., 1.)));
        close(path.project(p(6.1, 1.)).unwrap().along, 7.);
    }
    #[test]
    fn scopes_distinguish_segment_entire_path_and_intersections() {
        let mut project = Project::default();
        project.add(
            0,
            Material::Concrete,
            Shape::Polyline {
                points: vec![p(0., 0.), p(6., 0.), p(6., 2.)],
                arcs: vec![],
            },
        );
        let mut s = GuideSettings::default();
        assert_eq!(
            snap_candidates(&project, 0, p(5., 0.01), 0.2, &s),
            vec![p(3., 0.)]
        );
        s.scope = SnapScope::EntireElement;
        assert_eq!(
            snap_candidates(&project, 0, p(5., 0.01), 0.2, &s),
            vec![p(4., 0.)]
        );
        wall(&mut project, p(2., -2.), p(2., 2.), 0);
        wall(&mut project, p(4., -2.), p(4., 2.), 0);
        s.scope = SnapScope::BetweenIntersections;
        assert_eq!(
            snap_candidates(&project, 0, p(3.2, 0.01), 0.2, &s),
            vec![p(3., 0.)]
        );
    }
    #[test]
    fn snapping_respects_story_hover_and_enabled_state() {
        let mut project = Project::default();
        wall(&mut project, p(0., 0.), p(10., 0.), 1);
        let mut s = GuideSettings::default();
        assert!(snap_candidates(&project, 0, p(1., 0.), 0.2, &s).is_empty());
        assert!(snap_candidates(&project, 1, p(1., 3.), 0.2, &s).is_empty());
        assert_eq!(
            snap_candidates(&project, 1, p(1., 0.), 0.2, &s),
            vec![p(5., 0.)]
        );
        s.snap_enabled = false;
        assert!(snap_candidates(&project, 1, p(1., 0.), 0.2, &s).is_empty());
    }
    #[test]
    fn guide_projection_validation_intersections_and_serde() {
        let line = GuidePrimitive::Line {
            origin: p(0., 0.),
            direction: p(1., 0.),
        };
        let circle = GuidePrimitive::Circle {
            center: p(0., 0.),
            radius: 2.,
        };
        assert_eq!(line.project(p(100., 4.)), Some(p(100., 0.)));
        assert_eq!(circle.project(p(0., 0.)), Some(p(2., 0.)));
        assert_eq!(line.intersections(&circle), vec![p(-2., 0.), p(2., 0.)]);
        let invalid = GuidePrimitive::Line {
            origin: p(0., 0.),
            direction: p(0., 0.),
        };
        assert!(invalid.validate().is_err());
        assert!(invalid.project(p(0., 0.)).is_none());
        let guide = PersistentGuide {
            story: 2,
            primitive: circle,
        };
        assert_eq!(
            serde_json::from_str::<PersistentGuide>(&serde_json::to_string(&guide).unwrap())
                .unwrap(),
            guide
        );
        assert!(nearest_guide(&[guide], 0, p(2., 0.), 1.).is_none());
        assert_eq!(nearest_guide(&[guide], 2, p(2.1, 0.), 0.2), Some(p(2., 0.)));
    }
    #[test]
    fn nearest_guide_prefers_crossing_and_segment_bounds() {
        let a = PersistentGuide {
            story: 0,
            primitive: GuidePrimitive::Line {
                origin: p(0., 0.),
                direction: p(1., 0.),
            },
        };
        let b = PersistentGuide {
            story: 0,
            primitive: GuidePrimitive::Line {
                origin: p(0., 0.),
                direction: p(0., 1.),
            },
        };
        assert_eq!(nearest_guide(&[a, b], 0, p(0.1, 0.1), 0.2), Some(p(0., 0.)));
        let segment = GuidePrimitive::Segment {
            start: p(1., 1.),
            end: p(2., 1.),
        };
        assert_eq!(segment.project(p(3., 2.)), Some(p(2., 1.)));
        assert!(segment.intersections(&b.primitive).is_empty());
    }
    #[test]
    fn nearest_endpoint_uses_spatial_distance_on_folded_path() {
        let mut project = Project::default();
        project.add(
            0,
            Material::Concrete,
            Shape::Polyline {
                points: vec![p(0., 0.), p(10., 0.), p(10., 10.), p(1., 10.), p(1., 1.)],
                arcs: vec![],
            },
        );
        let settings = GuideSettings {
            method: SnapMethod::Percent,
            scope: SnapScope::EntireElement,
            ..Default::default()
        };
        let candidates = snap_candidates(&project, 0, p(2., 0.), 0.1, &settings);
        assert_eq!(candidates.len(), 1);
        close(candidates[0].distance(p(1., 8.6)), 0.);
    }
    #[test]
    fn wall_edges_and_slab_perimeters_have_explicit_lengths() {
        let mut project = Project::default();
        wall(&mut project, p(0., 0.), p(10., 0.), 0);
        let wall = &project.elements[0];
        close(
            element_paths(wall, DistanceReference::ReferenceLines)[0].length(),
            10.,
        );
        close(
            element_paths(wall, DistanceReference::Edges)[0].length(),
            20.4,
        );
        let id = project.add(
            0,
            Material::Concrete,
            Shape::Slab {
                min: p(0., 0.),
                max: p(6., 4.),
                thickness: 0.2,
            },
        );
        let slab = project.elements.iter().find(|e| e.id == id).unwrap();
        let path = &element_paths(slab, DistanceReference::Edges)[0];
        close(path.length(), 20.);
        assert_eq!(path.point_at(10.), Some(p(6., 4.)));
    }
    #[test]
    fn circle_intersections_cover_tangent_disjoint_and_coincident() {
        let circle = GuidePrimitive::Circle {
            center: p(0., 0.),
            radius: 1.,
        };
        let tangent = GuidePrimitive::Circle {
            center: p(2., 0.),
            radius: 1.,
        };
        assert_eq!(circle.intersections(&tangent), vec![p(1., 0.)]);
        let crossing = GuidePrimitive::Circle {
            center: p(1., 0.),
            radius: 1.,
        };
        let points = circle.intersections(&crossing);
        assert_eq!(points.len(), 2);
        for point in points {
            close(point.x, 0.5);
            close(point.y.abs(), 0.75_f32.sqrt());
        }
        assert!(circle.intersections(&circle).is_empty());
        assert!(
            circle
                .intersections(&GuidePrimitive::Circle {
                    center: p(4., 0.),
                    radius: 1.
                })
                .is_empty()
        );
    }
    #[test]
    fn persistent_arc_projects_exactly_and_roundtrips() {
        let arc = GuidePrimitive::Arc {
            start: p(1., 0.),
            end: p(0., 1.),
            arc: ArcSegment {
                radius: 1.,
                clockwise: false,
                large_arc: false,
            },
        };
        assert!(arc.validate().is_ok());
        let projected = arc.project(p(2., 2.)).unwrap();
        close(projected.x, 0.5_f32.sqrt());
        close(projected.y, 0.5_f32.sqrt());
        assert_eq!(arc.project(p(0., -2.)), Some(p(1., 0.)));
        let guide = PersistentGuide {
            story: 1,
            primitive: arc,
        };
        assert_eq!(
            serde_json::from_str::<PersistentGuide>(&serde_json::to_string(&guide).unwrap())
                .unwrap(),
            guide
        );
        let major = GuidePrimitive::Arc {
            start: p(1., 0.),
            end: p(0., 1.),
            arc: ArcSegment {
                radius: 1.,
                clockwise: true,
                large_arc: true,
            },
        };
        assert!(major.validate().is_ok());
        close(major.project(p(-2., 0.)).unwrap().distance(p(-1., 0.)), 0.);
    }
    #[test]
    fn persistent_arc_intersections_filter_both_sweeps() {
        let arc = GuidePrimitive::Arc {
            start: p(1., 0.),
            end: p(0., 1.),
            arc: ArcSegment {
                radius: 1.,
                clockwise: false,
                large_arc: false,
            },
        };
        let horizontal = GuidePrimitive::Line {
            origin: p(0., 0.5),
            direction: p(1., 0.),
        };
        let crossing = arc.intersections(&horizontal);
        assert_eq!(crossing.len(), 1);
        close(crossing[0].distance(p(0.75_f32.sqrt(), 0.5)), 0.);
        let circle = GuidePrimitive::Circle {
            center: p(1., 0.),
            radius: 1.,
        };
        let crossing = arc.intersections(&circle);
        assert_eq!(crossing.len(), 1);
        close(crossing[0].distance(p(0.5, 0.75_f32.sqrt())), 0.);
        let other_arc = GuidePrimitive::Arc {
            start: p(0., 0.),
            end: p(1., 1.),
            arc: ArcSegment {
                radius: 1.,
                clockwise: true,
                large_arc: false,
            },
        };
        assert!(other_arc.validate().is_ok());
        let crossing = arc.intersections(&other_arc);
        assert_eq!(crossing.len(), 1);
        close(crossing[0].distance(p(0.5, 0.75_f32.sqrt())), 0.);
        assert_eq!(arc.intersections(&other_arc), other_arc.intersections(&arc));
        let beyond = GuidePrimitive::Segment {
            start: p(-2., 0.5),
            end: p(0., 0.5),
        };
        assert!(arc.intersections(&beyond).is_empty());
    }
    #[test]
    fn guide_bounds_reject_invalid_positions_radii_and_arcs() {
        for point in [
            p(100_000., 0.),
            p(-100_000., 0.),
            p(0., f32::INFINITY),
            p(f32::NAN, 0.),
        ] {
            assert!(
                GuidePrimitive::Segment {
                    start: point,
                    end: p(1., 1.)
                }
                .validate()
                .is_err()
            );
            assert!(
                GuidePrimitive::Line {
                    origin: point,
                    direction: p(1., 0.)
                }
                .validate()
                .is_err()
            );
            assert!(
                GuidePrimitive::Circle {
                    center: point,
                    radius: 1.
                }
                .validate()
                .is_err()
            );
        }
        for radius in [0., -1., 100_001., f32::NAN, f32::INFINITY] {
            assert!(
                GuidePrimitive::Circle {
                    center: p(0., 0.),
                    radius
                }
                .validate()
                .is_err()
            );
        }
        assert!(
            GuidePrimitive::Circle {
                center: p(0., 0.),
                radius: 100_000.
            }
            .validate()
            .is_ok()
        );
        let invalid_arc = GuidePrimitive::Arc {
            start: p(0., 0.),
            end: p(10., 0.),
            arc: ArcSegment {
                radius: 1.,
                clockwise: false,
                large_arc: false,
            },
        };
        assert!(invalid_arc.validate().is_err());
        assert!(invalid_arc.project(p(2., 2.)).is_none());
        let huge_direction = GuidePrimitive::Line {
            origin: p(0., 0.),
            direction: p(1e30, 0.),
        };
        assert!(huge_direction.validate().is_ok());
        assert_eq!(huge_direction.project(p(3., 4.)), Some(p(3., 0.)));
        assert_eq!(
            huge_direction.intersections(&GuidePrimitive::Line {
                origin: p(3., 4.),
                direction: p(0., 1e30)
            }),
            vec![p(3., 0.)]
        );
        assert!(
            GuidePrimitive::Line {
                origin: p(0., 0.),
                direction: p(f32::MAX, f32::MAX)
            }
            .validate()
            .is_err()
        );
    }
}
