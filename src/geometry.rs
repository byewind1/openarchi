use crate::model::*;
use glam::{DMat4, DVec3};

/// Shared layer eligibility for model rendering and picking. Keep the full
/// project for physical joins; filter emitted surfaces and selectable objects.
pub fn model_element_layer_visible(project: &Project, element: &Element) -> bool {
    let visible = |e: &Element| project.layers.is_visible(e.layer.0);
    visible(element)
        && match element.shape {
            Shape::Opening { host_wall, .. } => project
                .elements
                .iter()
                .find(|e| e.id == host_wall)
                .is_some_and(visible),
            _ => true,
        }
}

/// Current model viewport candidate set. Geometry clipping is a separate step.
#[derive(Clone, Copy, Debug, Default)]
pub struct ModelViewFilter<'a> {
    pub story: Option<u32>,
    pub selected: &'a [u64],
    pub selection_only: bool,
    pub marquee: Option<&'a crate::editor::MarqueeArea>,
    pub outside_marquee: bool,
}
impl<'a> ModelViewFilter<'a> {
    pub fn clip_area(self) -> Option<&'a crate::editor::MarqueeArea> {
        if self.outside_marquee || self.selection_only {
            None
        } else {
            self.marquee
        }
    }
    pub fn contains_anchor(self, point: Point, z: f64) -> bool {
        let Some(area) = self.clip_area() else {
            return true;
        };
        area.z_range
            .is_none_or(|(a, b)| z >= a.min(b) && z <= a.max(b))
            && (point_in_polygon(point, &area.vertices)
                || (0..area.vertices.len()).any(|i| {
                    point_on_segment(
                        point,
                        area.vertices[i],
                        area.vertices[(i + 1) % area.vertices.len()],
                        1e-5,
                    )
                }))
    }
    /// Horizontal reference-line fragments; fragment ends are not model nodes.
    pub fn reference_segments(self, a: Point, b: Point, z: f64) -> Vec<[Point; 2]> {
        use geo::BooleanOps;
        let Some(area) = self.clip_area() else {
            return vec![[a, b]];
        };
        if area
            .z_range
            .is_some_and(|(lo, hi)| z < lo.min(hi) || z > lo.max(hi))
        {
            return Vec::new();
        }
        let polygon = geo::Polygon::new(
            geo::LineString::from(
                area.vertices
                    .iter()
                    .chain(area.vertices.first())
                    .map(|p| (f64::from(p.x), f64::from(p.y)))
                    .collect::<Vec<_>>(),
            ),
            vec![],
        );
        let lines = geo::MultiLineString(vec![geo::LineString::from(vec![
            (f64::from(a.x), f64::from(a.y)),
            (f64::from(b.x), f64::from(b.y)),
        ])]);
        polygon
            .clip(&lines, false)
            .into_iter()
            .flat_map(|line| {
                line.0
                    .windows(2)
                    .map(|pair| {
                        [
                            Point::new(pair[0].x as f64, pair[0].y as f64),
                            Point::new(pair[1].x as f64, pair[1].y as f64),
                        ]
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }
    pub fn includes(self, project: &Project, element: &Element) -> bool {
        if !self.story.is_none_or(|story| story == element.story)
            || !model_element_layer_visible(project, element)
            || !crate::view_settings::DesignOptionScope::active(
                project.plan_options.active_design_option,
            )
            .contains(element.design_option)
            || (self.selection_only && !self.selected.contains(&element.id))
        {
            return false;
        }
        let Some(area) = self.marquee else {
            return true;
        };
        if !area.includes_story(element.story) {
            return false;
        }
        if self.outside_marquee {
            !element_fully_inside_volume(project, element, &area.vertices, area.z_range)
        } else {
            element_intersects_polygon(project, element, &area.vertices)
                && area
                    .z_range
                    .is_none_or(|range| element_overlaps_z_range(project, element, range))
        }
    }
}

/// Reference-line anchors of a wall. `reference_offset` moves the wall body
/// around this line; it never moves the line itself.
pub fn wall_reference_line(e: &Element) -> Option<(Point, Point)> {
    match e.shape {
        Shape::Wall { start, end, .. } => Some((start, end)),
        Shape::ConceptMass { .. }
        | Shape::Zone { .. }
        | Shape::Roof { .. }
        | Shape::Beam { .. }
        | Shape::Column { .. }
        | Shape::Slab { .. }
        | Shape::Stair { .. }
        | Shape::Polyline { .. }
        | Shape::Opening { .. } => None,
    }
}

/// Project a point onto a wall's reference segment, clamped to its ends.
pub fn wall_reference_projection(e: &Element, point: Point) -> Option<Point> {
    if e.wall_contour.is_some() {
        let (station, _) = crate::wall_geometry::project_station(e, point);
        return crate::wall_geometry::point_tangent_at(e, station).map(|(point, _)| point);
    }
    let (start, end) = wall_reference_line(e)?;
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if !length_squared.is_finite() || length_squared < 1e-8 {
        return None;
    }
    let t = (((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared).clamp(0., 1.);
    Some(Point::new(start.x + dx * t, start.y + dy * t))
}

/// A nearby wall reference line that an edited endpoint can snap onto.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WallReferenceSnap {
    pub wall_id: u64,
    pub point: Point,
    pub distance: f64,
}

/// Find the nearest visible wall reference segment to `point`, excluding the
/// wall being edited. The snap point is the perpendicular projection clamped
/// to the target segment, so releasing there stores an exact intersection on
/// the other wall's reference line. Joins and wall-end trims remain derived
/// by `plan_wall_footprints`.
pub fn wall_reference_snap(
    project: &Project,
    story: u32,
    exclude_id: u64,
    point: Point,
    tolerance: f64,
) -> Option<WallReferenceSnap> {
    if !point.finite() || !tolerance.is_finite() || tolerance <= 0. {
        return None;
    }
    let mut best: Option<WallReferenceSnap> = None;
    let mut best_on_body = false;
    // Prefer the topmost wall on an exact distance tie, matching plan hit
    // testing for overlapping reference lines.
    for wall in project.elements.iter().rev().filter(|element| {
        element.id != exclude_id
            && element.story == story
            && matches!(element.shape, Shape::Wall { .. })
            && project
                .plan_options
                .element_visible(element, &project.layers)
    }) {
        let Some((start, end)) = wall_reference_line(wall) else {
            continue;
        };
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let length_squared = dx * dx + dy * dy;
        if !length_squared.is_finite() || length_squared < 1e-8 {
            continue;
        }
        let body_polygon = crate::wall_geometry::outline(wall);
        let on_body = contains(point, &body_polygon);
        let body_distance = polygon_edge_distance(point, &body_polygon);
        let baseline_distance = crate::wall_geometry::project_station(wall, point).1;
        if !on_body && body_distance > tolerance && baseline_distance > tolerance {
            continue;
        }
        let Some(candidate) = wall_reference_projection(wall, point) else {
            continue;
        };
        let distance = candidate.distance(point);
        let better = best.is_none_or(|snap| {
            (on_body && !best_on_body) || (on_body == best_on_body && distance < snap.distance)
        });
        if better {
            best = Some(WallReferenceSnap {
                wall_id: wall.id,
                point: candidate,
                distance,
            });
            best_on_body = on_body;
        }
    }
    best
}

pub fn column_footprint(center: Point, width: f64, depth: f64) -> [Point; 4] {
    rotated_column_footprint(center, width, depth, 0.)
}

pub fn rotated_column_footprint(
    center: Point,
    width: f64,
    depth: f64,
    angle_degrees: f64,
) -> [Point; 4] {
    let (sin, cos) = angle_degrees.rem_euclid(360.).to_radians().sin_cos();
    [
        (-width / 2., -depth / 2.),
        (width / 2., -depth / 2.),
        (width / 2., depth / 2.),
        (-width / 2., depth / 2.),
    ]
    .map(|(x, y)| Point::new(center.x + x * cos - y * sin, center.y + x * sin + y * cos))
}

pub fn footprint(e: &Element) -> [Point; 4] {
    if e.wall_contour.is_some() {
        return contour_bounds(e, 0., crate::wall_geometry::reference_length(e));
    }
    match e.shape {
        Shape::ConceptMass { ref spec } => {
            let (a, b) = crate::slab_geometry::bounds(&spec.paths()[0]);
            [a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)]
        }
        Shape::Zone { ref spec } => {
            let (a, b) = crate::slab_geometry::bounds(&spec.outline);
            [a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)]
        }
        Shape::Roof { ref spec } => spec.outline(),
        Shape::Beam {
            start,
            end,
            width,
            arc,
            ..
        } => {
            if arc.is_none() {
                return wall_footprint(start, end, width, 0.);
            }
            let g = crate::beam::BeamGeometry {
                start,
                end,
                width,
                arc,
                height: 1.,
                start_top: 0.,
                slant_angle: 0.,
            };
            let outline = g.outline().unwrap_or_default();
            let (min, max) = crate::slab_geometry::bounds(&outline);
            [min, Point::new(max.x, min.y), max, Point::new(min.x, max.y)]
        }
        Shape::Column {
            center,
            section,
            axis_rotation,
            ..
        } => match section {
            crate::column::ColumnSection::Rectangular { width, depth } => {
                rotated_column_footprint(center, width, depth, axis_rotation)
            }
            crate::column::ColumnSection::Circular { diameter } => {
                column_footprint(center, diameter, diameter)
            }
        },
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
        Shape::Polyline {
            ref points,
            ref arcs,
        } => crate::polyline_geometry::snapshot(points, arcs, 0.)
            .map(|g| {
                let [a, b] = g.bounds;
                [
                    Point::new(a.x, a.y),
                    Point::new(b.x, a.y),
                    Point::new(b.x, b.y),
                    Point::new(a.x, b.y),
                ]
            })
            .unwrap_or([Point::default(); 4]),
        // An opening has no footprint without its host wall. Use
        // `element_footprint` wherever a project is available.
        Shape::Opening { .. } => [Point::default(); 4],
    }
}

/// Hosted-opening reference segment: the two jamb points on the host wall
/// reference line, ordered from the wall start towards its end.
pub fn opening_span(project: &Project, opening: &Element) -> Option<(Point, Point)> {
    let Shape::Opening {
        host_wall,
        width,
        offset,
        ..
    } = opening.shape
    else {
        return None;
    };
    let host = project
        .elements
        .iter()
        .find(|element| element.id == host_wall)?;
    let half = width * 0.5;
    Some((
        crate::wall_geometry::point_tangent_at(host, offset - half)?.0,
        crate::wall_geometry::point_tangent_at(host, offset + half)?.0,
    ))
}

/// Physical centreline including the host wall body/reference offset.
pub fn opening_frame_span(project: &Project, opening: &Element) -> Option<(Point, Point)> {
    let q = opening_footprint(project, opening)?;
    Some((
        Point::new((q[0].x + q[3].x) / 2., (q[0].y + q[3].y) / 2.),
        Point::new((q[1].x + q[2].x) / 2., (q[1].y + q[2].y) / 2.),
    ))
}

/// Plan rectangle of an opening across its host wall thickness.
pub fn opening_footprint(project: &Project, opening: &Element) -> Option<[Point; 4]> {
    let Shape::Opening { host_wall, .. } = opening.shape else {
        return None;
    };
    let host = project
        .elements
        .iter()
        .find(|element| element.id == host_wall)?;
    let Shape::Wall {
        start,
        end,
        thickness,
        ..
    } = host.shape
    else {
        return None;
    };
    if host.wall_contour.is_some() {
        let Shape::Opening { offset, width, .. } = opening.shape else {
            return None;
        };
        let strips =
            crate::wall_geometry::strips_between(host, offset - width * 0.5, offset + width * 0.5);
        let first = strips.first()?;
        let last = strips.last()?;
        return Some([first[0], last[1], last[2], first[3]]);
    }
    let (a, b) = opening_span(project, opening)?;
    let length = start.distance(end).max(1e-6);
    let direction = Point::new((end.x - start.x) / length, (end.y - start.y) / length);
    let normal = Point::new(-direction.y, direction.x);
    let body = host
        .wall_settings
        .as_ref()
        .map_or(0., |settings| settings.body_offset(thickness));
    let shift = Point::new(normal.x * body, normal.y * body);
    let half = Point::new(normal.x * thickness * 0.5, normal.y * thickness * 0.5);
    let a = Point::new(a.x + shift.x, a.y + shift.y);
    let b = Point::new(b.x + shift.x, b.y + shift.y);
    Some([
        Point::new(a.x + half.x, a.y + half.y),
        Point::new(b.x + half.x, b.y + half.y),
        Point::new(b.x - half.x, b.y - half.y),
        Point::new(a.x - half.x, a.y - half.y),
    ])
}

/// Project a point onto the host wall reference line and return the distance
/// from its start, clamped to the wall length.
pub fn wall_offset_of(project: &Project, host_wall: u64, point: Point) -> Option<f64> {
    let host = project
        .elements
        .iter()
        .find(|element| element.id == host_wall)?;
    matches!(host.shape, Shape::Wall { .. })
        .then(|| crate::wall_geometry::project_station(host, point).0)
}

/// Footprint used by selection, bounds and visibility. Openings resolve through
/// their host wall; every other shape uses its own geometry.
pub fn element_footprint(project: &Project, element: &Element) -> [Point; 4] {
    if let Shape::Opening {
        host_wall,
        offset,
        width,
        ..
    } = element.shape
        && let Some(host) = project.elements.iter().find(|host| host.id == host_wall)
        && host.wall_contour.is_some()
    {
        return contour_bounds(host, offset - width * 0.5, offset + width * 0.5);
    }
    opening_footprint(project, element).unwrap_or_else(|| footprint(element))
}

fn contour_bounds(wall: &Element, from: f64, to: f64) -> [Point; 4] {
    let mut points: Vec<_> = crate::wall_geometry::strips_between(wall, from, to)
        .into_iter()
        .flatten()
        .collect();
    if let Some(WallContour::Arc { arc }) = wall.wall_contour
        && let Some((a, b)) = wall_reference_line(wall)
        && let Some((center, _)) = arc_center_sweep(a, b, arc)
    {
        // Cardinal extrema make the bounding box conservative independently
        // of the rendering tessellation and its chord error.
        for angle in [
            0.,
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::PI,
            std::f64::consts::PI * 1.5,
        ] {
            let p = Point::new(
                center.x + arc.radius * angle.cos(),
                center.y + arc.radius * angle.sin(),
            );
            let station = crate::wall_geometry::project_station(wall, p).0;
            if station >= from
                && station <= to
                && let Some((hi, lo)) = crate::wall_geometry::section_at(wall, station)
            {
                points.extend([hi, lo]);
            }
        }
    }
    let min = points
        .iter()
        .fold(Point::new(f64::INFINITY, f64::INFINITY), |a, p| {
            Point::new(a.x.min(p.x), a.y.min(p.y))
        });
    let max = points
        .iter()
        .fold(Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY), |a, p| {
            Point::new(a.x.max(p.x), a.y.max(p.y))
        });
    [min, Point::new(max.x, min.y), max, Point::new(min.x, max.y)]
}

/// Editable endpoints and explicitly inserted trapezoid nodes; tessellation
/// vertices are display samples, never snap/edit nodes.
pub fn wall_edit_nodes(element: &Element) -> Vec<Point> {
    let Some((start, end)) = wall_reference_line(element) else {
        return Vec::new();
    };
    let mut nodes = vec![start, end];
    if let Some(WallContour::Trapezoid {
        outer_start,
        outer_end,
        ref outer_nodes,
        ..
    }) = element.wall_contour
    {
        nodes.extend([outer_start, outer_end]);
        nodes.extend(outer_nodes.iter().copied());
    }
    nodes
}

pub(crate) fn polygon_edge_distance(point: Point, polygon: &[Point]) -> f64 {
    (0..polygon.len())
        .map(|i| segment_distance(point, polygon[i], polygon[(i + 1) % polygon.len()]))
        .fold(f64::INFINITY, f64::min)
}

/// Ordered editable slab vertices, excluding curve display samples.
pub fn slab_edit_nodes(element: &Element) -> Vec<Point> {
    crate::slab_geometry::rings(element)
        .into_iter()
        .flat_map(|r| r.points)
        .collect()
}

pub fn slab_contains(element: &Element, point: Point) -> bool {
    let paths = crate::slab_geometry::outlines(element);
    paths.first().is_some_and(|outer| contains(point, outer))
        && !paths.iter().skip(1).any(|hole| contains(point, hole))
}

fn slab_snap_points(element: &Element) -> Vec<(Point, SnapKind)> {
    let mut result = Vec::new();
    for ring in crate::slab_geometry::rings(element) {
        for i in 0..ring.points.len() {
            let a = ring.points[i];
            let b = ring.points[(i + 1) % ring.points.len()];
            result.push((a, SnapKind::Endpoint));
            let midpoint = ring
                .arcs
                .get(i)
                .copied()
                .flatten()
                .and_then(|arc| {
                    let (center, sweep) = arc_center_sweep(a, b, arc)?;
                    let angle = (a.y - center.y).atan2(a.x - center.x) + sweep * 0.5;
                    Some(Point::new(
                        center.x + arc.radius * angle.cos(),
                        center.y + arc.radius * angle.sin(),
                    ))
                })
                .unwrap_or(Point::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5));
            result.push((midpoint, SnapKind::Midpoint));
        }
    }
    result
}

/// Actual plan boundary. The four-corner API remains a conservative box for
/// bounds only; picking and marquee geometry use the curved boundary.
pub fn element_plan_outline(project: &Project, element: &Element) -> Vec<Point> {
    if let Some((outline, _holes)) = element.authored_prism_rings() {
        return outline.clone();
    }

    element_plan_outline_on_story(project, element, element.story)
}

pub fn element_on_plan_story(project: &Project, element: &Element, story: u32) -> bool {
    if matches!(
        element.shape,
        Shape::Column {
            inclination: Some(_),
            ..
        }
    ) {
        crate::column_plan::intersects_story(project, element, story)
    } else {
        element.story == story
    }
}

pub fn element_plan_outline_on_story(
    project: &Project,
    element: &Element,
    story: u32,
) -> Vec<Point> {
    if let Some(result) = crate::column_plan::for_element(project, element, story) {
        return result.map(|g| g.visible_outline()).unwrap_or_default();
    }
    if let Some(g) = crate::beam::geometry(project, element) {
        return g.outline().unwrap_or_default();
    }
    if matches!(element.shape, Shape::Column { .. }) {
        return crate::column_geometry::derive_plan(project, element, story)
            .map(|g| g.view.visible_outline())
            .unwrap_or_default();
    }
    if let Shape::Roof { ref spec } = element.shape {
        return spec.footprint_contour().outer;
    }
    if matches!(element.shape, Shape::Slab { .. }) {
        return crate::slab_geometry::outline(element);
    }
    if let Shape::Polyline { points, arcs } = &element.shape {
        return polyline_path(points, arcs);
    }
    if matches!(element.shape, Shape::Stair { .. }) {
        return crate::stair_geometry::derive(project, element)
            .map(|g| g.geometry.source.footprint.to_vec())
            .unwrap_or_default();
    }
    if matches!(element.shape, Shape::Wall { .. }) {
        return crate::wall_solid::derive(project, element)
            .map(|g| g.geometry.plan_path)
            .unwrap_or_default();
    }
    if matches!(element.shape, Shape::Opening { .. }) {
        return crate::opening_geometry::derive(project, element)
            .ok()
            .and_then(|g| g.geometry.plan_paths.into_iter().next())
            .unwrap_or_default();
    }
    element_footprint(project, element).to_vec()
}

/// Independent plan paths for candidate previews, including holes. Filled
/// element boundaries close separately; open polylines stay open.
pub fn element_plan_paths(project: &Project, element: &Element) -> Vec<Vec<Point>> {
    element_plan_paths_on_story(project, element, element.story)
}

/// Candidate outlines at the actual viewing story. Keep cut and projection
/// rings separate; their convex hull would invent edges between the rings.
pub fn element_plan_paths_on_story(
    project: &Project,
    element: &Element,
    story: u32,
) -> Vec<Vec<Point>> {
    if let Shape::Polyline { points, arcs } = &element.shape {
        return vec![polyline_path(points, arcs)];
    }
    let mut paths = if let Some(plan) = crate::column_plan::for_element(project, element, story) {
        match plan {
            Ok(plan) => vec![plan.cut, plan.uncut, plan.overhead]
                .into_iter()
                .filter(|ring| !ring.is_empty())
                .collect(),
            Err(_) => Vec::new(),
        }
    } else {
        match &element.shape {
            Shape::ConceptMass { spec } => spec.paths(),
            Shape::Zone { spec } => std::iter::once(spec.outline.clone())
                .chain(spec.holes.clone())
                .collect(),
            Shape::Roof { spec } => spec
                .footprint_contour()
                .rings()
                .map(<[Point]>::to_vec)
                .collect(),
            Shape::Slab { .. } => crate::slab_geometry::outlines(element),
            _ => vec![element_plan_outline(project, element)],
        }
    };
    for path in &mut paths {
        if let Some(first) = path.first().copied()
            && path.last() != Some(&first)
        {
            path.push(first);
        }
    }
    paths
}

pub fn prism_corners(outline: [Point; 4], bottom: f64, top: f64) -> [DVec3; 8] {
    std::array::from_fn(|index| {
        let p = outline[index % 4];
        DVec3::new(p.x, p.y, if index < 4 { bottom } else { top })
    })
}

/// Exact station boundaries split the tessellated solid around hosted holes.
/// Both 3D rendering and ray picking consume these same blocks.
pub fn wall_solid_prisms(project: &Project, wall: &Element) -> Vec<[DVec3; 8]> {
    let Ok(g) = crate::wall_solid::derive(project, wall) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for band in &g.geometry.bands {
        for &i in &band.cell_indices {
            let c = &g.geometry.cells[i];
            let poly = geo::Polygon::new(
                geo::LineString::from(
                    c.vertices
                        .iter()
                        .chain(c.vertices.first())
                        .map(|p| (p.x, p.y))
                        .collect::<Vec<_>>(),
                ),
                vec![],
            );
            for t in crate::derived_geometry::triangulate_with_boundary(&poly) {
                let [a, b, c] = t.map(|p| Point::new(p.x, p.y));
                out.push(prism_corners([a, b, c, c], band.bottom_m, band.top_m));
            }
        }
    }
    out
}

pub(crate) fn wall_footprint(
    a: Point,
    b: Point,
    thickness: f64,
    reference_offset: f64,
) -> [Point; 4] {
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
pub fn segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy)
        / (dx * dx + dy * dy)
            .max(crate::precision::GeometryContext::SOLVER.segment_denominator_floor_m2))
    .clamp(0., 1.);
    p.distance(Point::new(a.x + t * dx, a.y + t * dy))
}

/// Return a constant-distance offset of an open or explicitly closed path.
/// Positive distances move each directed segment to its left.
pub fn offset_polyline(points: &[Point], distance: f64) -> Option<Vec<Point>> {
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
        if source_delta.x * result_delta.x + source_delta.y * result_delta.y
            <= crate::precision::GeometryContext::SOLVER.offset_forward_dot_m2
        {
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
        let signed_area = |path: &[Point]| polygon_area(path) * 2.;
        if signed_area(&points[..count]) * signed_area(&result)
            <= crate::precision::GeometryContext::SOLVER.offset_area_product_m4
        {
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
    distance: f64,
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
    fn line_circle(a: Point, b: Point, c: Point, r: f64) -> Vec<Point> {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let length2 = dx * dx + dy * dy;
        let t = ((c.x - a.x) * dx + (c.y - a.y) * dy) / length2;
        let foot = Point::new(a.x + t * dx, a.y + t * dy);
        let height2 = r * r - foot.distance(c).powi(2);
        if height2 < -crate::precision::GeometryContext::SOLVER.circle_height_squared_m2 {
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
                    if height2 < -crate::precision::GeometryContext::SOLVER.circle_height_squared_m2
                    {
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
                (a0 - a1).rem_euclid(std::f64::consts::TAU)
            } else {
                (a1 - a0).rem_euclid(std::f64::consts::TAU)
            };
            let (_, old_sweep) = arc_center_sweep(points[i], points[i + 1], arc)?;
            // Do not jump to a complementary loop when an offset consumes a segment.
            if (sweep - old_sweep.abs()).abs() > std::f64::consts::PI {
                return None;
            }
            arc.radius = radius;
            arc.large_arc = sweep > std::f64::consts::PI;
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
    if denominator.abs() < crate::precision::GeometryContext::SOLVER.raw_line_cross_m2 {
        return None;
    }
    let t = ((c.x - a.x) * sy - (c.y - a.y) * sx) / denominator;
    Some(Point::new(a.x + t * rx, a.y + t * ry))
}

/// Fit exact circular-arc metadata through both endpoints and a dragged point.
pub fn arc_segment_through(a: Point, b: Point, through: Point) -> Option<ArcSegment> {
    if ![a.x, a.y, b.x, b.y, through.x, through.y]
        .iter()
        .all(|value| value.is_finite())
    {
        return None;
    }
    // Solve relative to the first endpoint before squaring. Absolute world
    // coordinates cause catastrophic cancellation for small, distant arcs.
    let ux = f64::from(b.x) - f64::from(a.x);
    let uy = f64::from(b.y) - f64::from(a.y);
    let vx = f64::from(through.x) - f64::from(a.x);
    let vy = f64::from(through.y) - f64::from(a.y);
    let determinant = 2. * (ux * vy - uy * vx);
    if determinant.abs() < crate::precision::GeometryContext::SOLVER.arc_center_double_area_m2 {
        return None;
    }
    let uu = ux * ux + uy * uy;
    let vv = vx * vx + vy * vy;
    let cx = (uu * vy - vv * uy) / determinant;
    let cy = (ux * vv - vx * uu) / determinant;
    let radius = cx.hypot(cy) as f64;
    if !radius.is_finite() || radius < 0.005 {
        return None;
    }
    let start = (-cy).atan2(-cx);
    let finish = (uy - cy).atan2(ux - cx);
    let middle = (vy - cy).atan2(vx - cx);
    let positive = (finish - start).rem_euclid(std::f64::consts::TAU);
    let middle_positive = (middle - start).rem_euclid(std::f64::consts::TAU);
    let sweep = if middle_positive
        <= positive + crate::precision::GeometryContext::SOLVER.arc_through_angle_guard_rad
    {
        positive
    } else {
        positive - std::f64::consts::TAU
    };
    if sweep.abs() < crate::precision::GeometryContext::SOLVER.arc_minimum_sweep_rad {
        return None;
    }
    Some(ArcSegment {
        radius,
        clockwise: sweep < 0.,
        // Match the stored geometry and reconstruction precision at a semicircle.
        large_arc: (sweep.abs() as f64) > std::f64::consts::PI,
    })
}

/// Reconstruct the center and signed sweep from endpoints plus exact arc data.
pub fn arc_center_sweep(a: Point, b: Point, arc: ArcSegment) -> Option<(Point, f64)> {
    if ![a.x, a.y, b.x, b.y].iter().all(|value| value.is_finite()) {
        return None;
    }
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let chord = dx.hypot(dy);
    if !chord.is_finite()
        || !arc.radius.is_finite()
        || arc.radius <= 0.
        || chord < 1e-6
        || chord > 2. * arc.radius + crate::precision::GeometryContext::SOLVER.arc_chord_overshoot_m
    {
        return None;
    }
    let half = chord * 0.5;
    let height = (arc.radius * arc.radius - half * half).max(0.).sqrt();
    let midpoint = Point::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
    // A diameter has one center; major/minor flags describe the same half-circle.
    // Avoid letting atan2 rounding reject that center at the PI boundary.
    if height == 0. {
        return Some((
            midpoint,
            if arc.clockwise {
                -std::f64::consts::PI
            } else {
                std::f64::consts::PI
            },
        ));
    }
    let normal = Point::new(-dy / chord, dx / chord);
    for sign in [-1., 1.] {
        let center = Point::new(
            midpoint.x + normal.x * height * sign,
            midpoint.y + normal.y * height * sign,
        );
        let a0 = (a.y - center.y).atan2(a.x - center.x);
        let a1 = (b.y - center.y).atan2(b.x - center.x);
        let ccw = (a1 - a0).rem_euclid(std::f64::consts::TAU);
        let sweep = if arc.clockwise {
            ccw - std::f64::consts::TAU
        } else {
            ccw
        };
        if (sweep.abs() > std::f64::consts::PI) == arc.large_arc {
            return Some((center, sweep));
        }
    }
    None
}

/// View-only tessellation for a single exact arc segment.
pub fn arc_points(a: Point, b: Point, arc: ArcSegment) -> Option<Vec<Point>> {
    let (center, sweep) = arc_center_sweep(a, b, arc)?;
    let start = (a.y - center.y).atan2(a.x - center.x);
    let steps = ((sweep.abs()
        / crate::precision::GeometryContext::SAMPLING.polyline_arc_max_step_rad)
        .ceil() as usize)
        .max(2);
    Some(
        (0..=steps)
            .map(|index| {
                if index == 0 {
                    return a;
                }
                if index == steps {
                    return b;
                }
                let angle = start + sweep * index as f64 / steps as f64;
                Point::new(
                    center.x + arc.radius * angle.cos(),
                    center.y + arc.radius * angle.sin(),
                )
            })
            .collect(),
    )
}

pub fn arc_length(a: Point, b: Point, arc: ArcSegment) -> f64 {
    arc_center_sweep(a, b, arc).map_or_else(|| a.distance(b), |(_, sweep)| arc.radius * sweep.abs())
}

/// Expand segment curves only for rendering and geometric hit calculations.
pub fn polyline_path(points: &[Point], arcs: &[Option<ArcSegment>]) -> Vec<Point> {
    crate::polyline_geometry::snapshot(points, arcs, 0.)
        .map(|g| g.plan_path)
        .unwrap_or_default()
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

fn orientation(a: Point, b: Point, c: Point) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

pub(crate) fn segments_intersect(a: Point, b: Point, c: Point, d: Point) -> bool {
    let ab_c = orientation(a, b, c);
    let ab_d = orientation(a, b, d);
    let cd_a = orientation(c, d, a);
    let cd_b = orientation(c, d, b);
    let area_eps = crate::precision::GeometryContext::TOPOLOGY.segment_collinear_double_area_m2;
    let distance_eps = crate::precision::GeometryContext::TOPOLOGY.segment_touch_distance_m;
    if ab_c.abs() <= area_eps && segment_distance(c, a, b) <= distance_eps {
        return true;
    }
    if ab_d.abs() <= area_eps && segment_distance(d, a, b) <= distance_eps {
        return true;
    }
    if cd_a.abs() <= area_eps && segment_distance(a, c, d) <= distance_eps {
        return true;
    }
    if cd_b.abs() <= area_eps && segment_distance(b, c, d) <= distance_eps {
        return true;
    }
    (ab_c > 0.) != (ab_d > 0.) && (cd_a > 0.) != (cd_b > 0.)
}

/// Editable plan nodes shared by selection, hover and coordinate-reference gestures.
/// These are model edit nodes, rather than CAD snap points or tessellated outlines.
pub fn element_edit_nodes(project: &Project, e: &Element) -> Vec<Point> {
    match e.shape {
        Shape::ConceptMass { ref spec } => spec
            .outline
            .iter()
            .chain(spec.holes.iter().flatten())
            .copied()
            .collect(),
        Shape::Zone { ref spec } => spec
            .outline
            .iter()
            .chain(spec.holes.iter().flatten())
            .copied()
            .collect(),
        Shape::Wall { .. } => wall_edit_nodes(e),
        Shape::Slab { .. } => slab_edit_nodes(e),
        Shape::Beam { start, end, .. } => vec![start, end],
        Shape::Roof { ref spec } => spec
            .footprint_contour()
            .rings()
            .flatten()
            .copied()
            .collect(),
        Shape::Column { .. } => crate::column::edit_nodes(e),
        Shape::Stair { .. } => footprint(e).into_iter().collect(),
        Shape::Polyline { ref points, .. } => points.clone(),
        Shape::Opening { .. } => element_footprint(project, e).into_iter().collect(),
    }
}

/// Archicad-style editing selection: an element is included when at least one
/// of its editable plan nodes lies in the marquee. The whole element is then
/// operated on by group commands.
pub fn element_has_node_in_polygon(project: &Project, e: &Element, polygon: &[Point]) -> bool {
    element_edit_nodes(project, e)
        .into_iter()
        .any(|p| contains(p, polygon))
}

/// Visibility selection for 3D uses geometric overlap, so a wall segment
/// crossing the marquee is included even when neither endpoint is inside.
pub fn element_intersects_polygon(project: &Project, e: &Element, polygon: &[Point]) -> bool {
    if matches!(e.shape, Shape::ConceptMass { .. } | Shape::Zone { .. }) {
        return if matches!(e.shape, Shape::ConceptMass { .. }) {
            crate::concept_mass::derive(e, project.elevation(e.story))
        } else {
            crate::zones::derive(e, project.elevation(e.story))
        }
        .is_ok_and(|g| triangles_intersect_polygon(&g.plan_triangles, polygon));
    }
    if matches!(
        e.shape,
        Shape::Column {
            inclination: Some(_),
            ..
        }
    ) {
        return crate::column::inclined_solid(project, e)
            .and_then(|solid| solid.clipped_mesh(Some(polygon), None).ok())
            .is_some_and(|mesh| !mesh.triangles.is_empty());
    }
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
    if matches!(e.shape, Shape::Slab { .. } | Shape::Roof { .. }) {
        let triangles = if matches!(e.shape, Shape::Roof { .. }) {
            crate::roof_geometry::derive(e, project.elevation(e.story))
                .map(|g| g.geometry.plan_triangles)
                .unwrap_or_default()
        } else {
            crate::slab_geometry::triangles(e)
        };
        return triangles_intersect_polygon(&triangles, polygon);
    }
    let shape = element_plan_outline(project, e);
    closed_path_intersects_polygon(&shape, polygon)
}

pub(crate) fn triangles_intersect_polygon(triangles: &[[Point; 3]], polygon: &[Point]) -> bool {
    triangles.iter().any(|triangle| {
        triangle.iter().any(|p| contains(*p, polygon))
            || polygon.iter().any(|p| contains(*p, triangle))
            || (0..3).any(|i| {
                (0..polygon.len()).any(|j| {
                    segments_intersect(
                        triangle[i],
                        triangle[(i + 1) % 3],
                        polygon[j],
                        polygon[(j + 1) % polygon.len()],
                    )
                })
            })
    })
}
pub(crate) fn closed_path_intersects_polygon(shape: &[Point], polygon: &[Point]) -> bool {
    if shape.iter().any(|p| contains(*p, polygon)) || polygon.iter().any(|p| contains(*p, shape)) {
        return true;
    }
    (0..shape.len()).any(|i| {
        let a = shape[i];
        let b = shape[(i + 1) % shape.len()];
        (0..polygon.len())
            .any(|j| segments_intersect(a, b, polygon[j], polygon[(j + 1) % polygon.len()]))
    })
}

/// Check the entire sampled path, including intervals between boundary crossings.
/// Vertex-only containment is insufficient for a concave marquee.
pub(crate) fn path_fully_inside_polygon(points: &[Point], closed: bool, polygon: &[Point]) -> bool {
    use geo::line_intersection::{LineIntersection, line_intersection};
    if points.is_empty() || polygon.len() < 3 || !points.iter().all(|p| contains(*p, polygon)) {
        return false;
    }
    let coord = |p: Point| geo::Coord {
        x: f64::from(p.x),
        y: f64::from(p.y),
    };
    let count = if closed {
        points.len()
    } else {
        points.len() - 1
    };
    (0..count).all(|i| {
        let a = coord(points[i]);
        let b = coord(points[(i + 1) % points.len()]);
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let length_squared = dx * dx + dy * dy;
        if length_squared == 0. {
            return true;
        }
        let line = geo::Line::new(a, b);
        let mut cuts = vec![0., 1.];
        let mut add = |p: geo::Coord<f64>| {
            cuts.push((((p.x - a.x) * dx + (p.y - a.y) * dy) / length_squared).clamp(0., 1.));
        };
        for j in 0..polygon.len() {
            match line_intersection(
                line,
                geo::Line::new(coord(polygon[j]), coord(polygon[(j + 1) % polygon.len()])),
            ) {
                Some(LineIntersection::SinglePoint { intersection, .. }) => add(intersection),
                Some(LineIntersection::Collinear { intersection }) => {
                    add(intersection.start);
                    add(intersection.end);
                }
                None => {}
            }
        }
        cuts.sort_by(f64::total_cmp);
        cuts.dedup();
        cuts.windows(2).all(|pair| {
            let t = (pair[0] + pair[1]) * 0.5;
            contains(
                Point::new((a.x + t * dx) as f64, (a.y + t * dy) as f64),
                polygon,
            )
        })
    })
}

pub fn element_fully_inside_polygon(project: &Project, e: &Element, polygon: &[Point]) -> bool {
    if matches!(
        e.shape,
        Shape::Column {
            inclination: Some(_),
            ..
        }
    ) {
        return crate::column::inclined_solid(project, e)
            .and_then(|solid| {
                solid
                    .projected_outline(
                        solid.column.bottom,
                        solid.column.bottom + solid.column.height,
                    )
                    .ok()
            })
            .is_some_and(|outline| path_fully_inside_polygon(&outline, true, polygon));
    }
    if let Shape::Polyline {
        ref points,
        ref arcs,
    } = e.shape
    {
        return path_fully_inside_polygon(&polyline_path(points, arcs), false, polygon);
    }
    path_fully_inside_polygon(&element_plan_outline(project, e), true, polygon)
}

/// Actual vertical extent for volume filtering; conservative XY box corners
/// can extrapolate a sloped roof beyond its real contour.
fn element_vertical_extent(project: &Project, element: &Element) -> (f64, f64) {
    if matches!(element.shape, Shape::Roof { .. }) {
        return crate::roof_geometry::derive(element, project.elevation(element.story))
            .map(|g| (g.geometry.bounds[0].z, g.geometry.bounds[1].z))
            .unwrap_or((f64::INFINITY, f64::NEG_INFINITY));
    }
    if matches!(element.shape, Shape::Wall { .. } | Shape::Opening { .. }) {
        let bounds = if matches!(element.shape, Shape::Wall { .. }) {
            crate::wall_solid::derive(project, element)
                .ok()
                .and_then(|g| g.geometry.bounds)
        } else {
            crate::opening_geometry::derive(project, element)
                .ok()
                .and_then(|g| g.geometry.bounds)
        };
        return bounds.map_or((f64::INFINITY, f64::NEG_INFINITY), |[lo, hi]| (lo.z, hi.z));
    }
    if matches!(element.shape, Shape::Stair { .. }) {
        return crate::stair_geometry::derive(project, element)
            .map(|g| {
                (
                    g.geometry.display_bounds[0].z,
                    g.geometry.display_bounds[1].z,
                )
            })
            .unwrap_or((f64::INFINITY, f64::NEG_INFINITY));
    }
    corners(project, element)
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), point| {
            (low.min(point.z), high.max(point.z))
        })
}

pub fn element_overlaps_z_range(project: &Project, e: &Element, range: (f64, f64)) -> bool {
    if range.0.is_nan() || range.1.is_nan() {
        return false;
    }
    let (bottom, top) = (range.0.min(range.1), range.0.max(range.1));
    let (lo, hi) = element_vertical_extent(project, e);
    hi >= bottom && lo <= top
}

pub fn element_fully_inside_volume(
    project: &Project,
    e: &Element,
    polygon: &[Point],
    range: Option<(f64, f64)>,
) -> bool {
    if !element_fully_inside_polygon(project, e, polygon) {
        return false;
    }
    let Some((bottom, top)) = range else {
        return true;
    };
    if bottom.is_nan() || top.is_nan() {
        return false;
    }
    let (bottom, top) = (bottom.min(top), bottom.max(top));
    let (lo, hi) = element_vertical_extent(project, e);
    lo >= bottom && hi <= top
}

/// Triangulate a simple polygon with ear clipping. Marquee polygons may be
/// concave, so clipping each edge as though the whole area were convex is not
/// sufficient.
pub fn triangulate_polygon(poly: &[Point]) -> Vec<[Point; 3]> {
    if poly.len() < 3 {
        return Vec::new();
    }
    let area = polygon_area(poly) * 2.;
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
    let eps = crate::precision::GeometryContext::TOPOLOGY.triangle_double_area_m2;
    (x >= -eps && y >= -eps && z >= -eps) || (x <= eps && y <= eps && z <= eps)
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

/// Signed area with translation removed before multiplication. Accumulating in
/// f64 avoids losing small footprints through cancellation at large coordinates.
pub(crate) fn polygon_area(poly: &[Point]) -> f64 {
    let Some(origin) = poly.first() else {
        return 0.;
    };
    (0..poly.len())
        .map(|i| {
            let a = poly[i];
            let b = poly[(i + 1) % poly.len()];
            let ax = a.x as f64 - origin.x as f64;
            let ay = a.y as f64 - origin.y as f64;
            let bx = b.x as f64 - origin.x as f64;
            let by = b.y as f64 - origin.y as f64;
            ax * by - bx * ay
        })
        .sum::<f64>() as f64
        * 0.5
}

/// Reference-line frame plus the lateral interval occupied by the wall body.
/// `lo`/`hi` are measured left of the start-to-end direction, matching
/// `WallSettings::body_offset`, and the body spans `[lo, hi]`.
#[derive(Clone, Copy)]
struct WallBody {
    start: Point,
    end: Point,
    dir: Point,
    left: Point,
    length: f64,
    lo: f64,
    hi: f64,
}

impl WallBody {
    fn thickness(&self) -> f64 {
        (self.hi - self.lo).max(1e-4)
    }
}

fn dot(a: Point, b: Point) -> f64 {
    a.x * b.x + a.y * b.y
}

fn wall_body(wall: &Element) -> Option<WallBody> {
    if wall.wall_contour.is_some() {
        return None;
    }
    let Shape::Wall {
        start,
        end,
        thickness,
        ..
    } = wall.shape
    else {
        return None;
    };
    let length = start.distance(end);
    if !length.is_finite() || length < 1e-4 {
        return None;
    }
    let dir = Point::new((end.x - start.x) / length, (end.y - start.y) / length);
    let left = Point::new(-dir.y, dir.x);
    let offset = wall
        .wall_settings
        .as_ref()
        .map_or(0., |settings| settings.body_offset(thickness));
    // Archicad stores the offset of one body face and spans [offset, offset +
    // cross-section width]; OpenArchi keeps the same span centred on
    // `body_offset`.
    Some(WallBody {
        start,
        end,
        dir,
        left,
        length,
        lo: offset - thickness * 0.5,
        hi: offset + thickness * 0.5,
    })
}

fn body_polygon(body: &WallBody) -> [Point; 4] {
    let at = |origin: Point, offset: f64| {
        Point::new(
            origin.x + body.left.x * offset,
            origin.y + body.left.y * offset,
        )
    };
    [
        at(body.start, body.hi),
        at(body.end, body.hi),
        at(body.end, body.lo),
        at(body.start, body.lo),
    ]
}

fn intersect_lines(a: Point, a_dir: Point, b: Point, b_dir: Point) -> Option<Point> {
    let denominator = a_dir.x * b_dir.y - a_dir.y * b_dir.x;
    if denominator.abs() < crate::precision::GeometryContext::SOLVER.normalized_parallel_sine {
        return None;
    }
    let delta = Point::new(b.x - a.x, b.y - a.y);
    let t = (delta.x * b_dir.y - delta.y * b_dir.x) / denominator;
    let point = Point::new(a.x + a_dir.x * t, a.y + a_dir.y * t);
    point.finite().then_some(point)
}

/// Lateral offset of the left-hand body side in the frame of a travel
/// direction that points *into* the joint, or *away* from it. A corner join
/// continues the offset curve, so the incoming wall's left side pairs with the
/// outgoing wall's left side.
fn incoming_left_offset(body: &WallBody, at_start: bool) -> f64 {
    if at_start { body.lo } else { body.hi }
}

fn outgoing_left_offset(body: &WallBody, at_start: bool) -> f64 {
    if at_start { body.hi } else { body.lo }
}

fn is_convex_quad(poly: &[Point; 4]) -> bool {
    let mut sign = 0.0_f64;
    for index in 0..4 {
        let a = poly[index];
        let b = poly[(index + 1) % 4];
        let c = poly[(index + 2) % 4];
        let cross = (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x);
        if cross.abs() < 1e-9 {
            continue;
        }
        if sign == 0.0 {
            sign = cross.signum();
        } else if cross.signum() != sign {
            return false;
        }
    }
    true
}

/// Corner points imposed by adjoining walls on one end of `bodies[index]`.
/// Returns `(left_of_travel, right_of_travel)` in whole-body coordinates, or
/// `None` when the end is free or no usable intersection exists.
// Local broad phase only: exact connection predicates remain below.
struct WallJoinIndex {
    root: JoinBoundsNode,
    max_thickness: f64,
}
struct JoinBoundsNode {
    bounds: [f64; 4],
    children: Option<Box<[JoinBoundsNode; 2]>>,
    slots: Vec<usize>,
}
impl JoinBoundsNode {
    fn build(entries: &mut [(usize, [f64; 4])]) -> Self {
        let mut bounds = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        for (_, b) in entries.iter() {
            bounds[0] = bounds[0].min(b[0]);
            bounds[1] = bounds[1].min(b[1]);
            bounds[2] = bounds[2].max(b[2]);
            bounds[3] = bounds[3].max(b[3]);
        }
        if entries.len() <= 8 {
            return Self {
                bounds,
                children: None,
                slots: entries.iter().map(|e| e.0).collect(),
            };
        }
        let axis = usize::from(bounds[3] - bounds[1] > bounds[2] - bounds[0]);
        let middle = entries.len() / 2;
        entries.select_nth_unstable_by(middle, |a, b| {
            (a.1[axis] + a.1[axis + 2]).total_cmp(&(b.1[axis] + b.1[axis + 2]))
        });
        let (left, right) = entries.split_at_mut(middle);
        Self {
            bounds,
            children: Some(Box::new([Self::build(left), Self::build(right)])),
            slots: Vec::new(),
        }
    }
    fn query(&self, q: [f64; 4], result: &mut Vec<usize>) {
        if self.bounds[0] > q[2]
            || self.bounds[2] < q[0]
            || self.bounds[1] > q[3]
            || self.bounds[3] < q[1]
        {
            return;
        }
        if let Some(children) = &self.children {
            children[0].query(q, result);
            children[1].query(q, result);
        } else {
            result.extend_from_slice(&self.slots);
        }
    }
}
impl WallJoinIndex {
    fn new(bodies: &[Option<WallBody>]) -> Option<Self> {
        if bodies.len() <= 32 {
            return None;
        }
        let mut entries = Vec::new();
        let mut max_thickness = 0.0_f64;
        for (slot, body) in bodies.iter().enumerate() {
            let Some(body) = body else {
                continue;
            };
            let end = Point::new(
                body.start.x + body.dir.x * body.length,
                body.start.y + body.dir.y * body.length,
            );
            if [
                body.start.x,
                body.start.y,
                body.end.x,
                body.end.y,
                end.x,
                end.y,
                body.length,
                body.thickness(),
            ]
            .into_iter()
            .any(|v| !v.is_finite() || v.abs() > 100000.)
            {
                return None;
            }
            max_thickness = max_thickness.max(body.thickness());
            entries.push((
                slot,
                [
                    f64::from(body.start.x.min(end.x)),
                    f64::from(body.start.y.min(end.y)),
                    f64::from(body.start.x.max(end.x)),
                    f64::from(body.start.y.max(end.y)),
                ],
            ));
        }
        Some(Self {
            root: JoinBoundsNode::build(&mut entries),
            max_thickness,
        })
    }
    fn candidates(&self, body: &WallBody, at_start: bool) -> Vec<usize> {
        let p = if at_start { body.start } else { body.end };
        // Same monotone f64 tolerance calculation as the narrow phase; the
        // additional metre dominates rounding error within the guarded range.
        let radius = f64::from((body.thickness() + self.max_thickness) * 0.5 + 0.02) + 1.;
        let (x, y) = (f64::from(p.x), f64::from(p.y));
        let mut slots = Vec::new();
        self.root
            .query([x - radius, y - radius, x + radius, y + radius], &mut slots);
        slots.sort_unstable();
        slots
    }
}

fn joined_end_corners(
    bodies: &[Option<WallBody>],
    groups: &[u8],
    stories: &[u32],
    index: usize,
    at_start: bool,
    candidates: impl Iterator<Item = usize>,
    mut dependencies: Option<&mut Vec<usize>>,
) -> Option<(Point, Point)> {
    let body = bodies[index].as_ref()?;
    let point = if at_start { body.start } else { body.end };
    let outward = if at_start {
        Point::new(-body.dir.x, -body.dir.y)
    } else {
        body.dir
    };
    let mut best_left: Option<(f64, Point)> = None;
    let mut best_right: Option<(f64, Point)> = None;
    for other_index in candidates {
        let other = &bodies[other_index];
        if other_index == index || stories[other_index] != stories[index] {
            continue;
        }
        let Some(other) = other else {
            continue;
        };
        // AC29 layer intersection groups: only same non-zero groups join.
        if !groups
            .get(index)
            .zip(groups.get(other_index))
            .is_some_and(|(a, b)| *a != 0 && *a == *b)
        {
            continue;
        }
        let tolerance = (body.thickness() + other.thickness()) * 0.5 + 0.02;
        let along = dot(
            Point::new(point.x - other.start.x, point.y - other.start.y),
            other.dir,
        );
        if along < -tolerance || along > other.length + tolerance {
            continue;
        }
        let nearest = Point::new(
            other.start.x + other.dir.x * along.clamp(0., other.length),
            other.start.y + other.dir.y * along.clamp(0., other.length),
        );
        if point.distance(nearest) > tolerance {
            continue;
        }
        if let Some(dependencies) = dependencies.as_mut() {
            dependencies.push(other_index);
        }
        let limit = 8.0 * (body.thickness() + other.thickness()).max(0.05);

        let consider = |slot: &mut Option<(f64, Point)>, candidate: Point| {
            let reach = dot(
                Point::new(candidate.x - point.x, candidate.y - point.y),
                outward,
            );
            if reach.is_finite()
                && reach.abs() <= limit
                && slot.is_none_or(|(best, _)| reach > best)
            {
                *slot = Some((reach, candidate));
            }
        };

        let own_left = incoming_left_offset(body, at_start);
        let own_right = if at_start { body.hi } else { body.lo };
        let own_left_anchor = Point::new(
            point.x + body.left.x * own_left,
            point.y + body.left.y * own_left,
        );
        let own_right_anchor = Point::new(
            point.x + body.left.x * own_right,
            point.y + body.left.y * own_right,
        );

        let start_distance = point.distance(other.start);
        let end_distance = point.distance(other.end);
        let corner_connection = start_distance.min(end_distance) <= tolerance.max(0.02);
        if corner_connection {
            let other_at_start = start_distance <= end_distance;
            let other_endpoint = if other_at_start {
                other.start
            } else {
                other.end
            };
            let connection_offset = point.distance(other_endpoint);
            let other_left = outgoing_left_offset(other, other_at_start);
            let other_right = if other_at_start { other.lo } else { other.hi };
            let other_left_anchor = Point::new(
                other.start.x + other.left.x * other_left,
                other.start.y + other.left.y * other_left,
            );
            let other_right_anchor = Point::new(
                other.start.x + other.left.x * other_right,
                other.start.y + other.left.y * other_right,
            );
            let same_side = (
                intersect_lines(own_left_anchor, body.dir, other_left_anchor, other.dir),
                intersect_lines(own_right_anchor, body.dir, other_right_anchor, other.dir),
            );
            // A coincident joint uses the ordinary same-side mitre. When the
            // reference endpoints are offset - for example a return wall
            // snapped to the face of the host - the end is trimmed by the far
            // face of the other wall. Both corner points then lie on the same
            // plane, which fills the outside corner without cutting a diagonal
            // bevel across the end.
            let face_offset = if dot(other.left, outward) >= 0.0 {
                other.hi
            } else {
                other.lo
            };
            let face_anchor = Point::new(
                other.start.x + other.left.x * face_offset,
                other.start.y + other.left.y * face_offset,
            );
            let face_pair = (
                intersect_lines(own_left_anchor, body.dir, face_anchor, other.dir),
                intersect_lines(own_right_anchor, body.dir, face_anchor, other.dir),
            );
            let chosen =
                if connection_offset <= 1e-3 && same_side.0.is_some() && same_side.1.is_some() {
                    same_side
                } else {
                    face_pair
                };
            if let Some(candidate) = chosen.0 {
                consider(&mut best_left, candidate);
            }
            if let Some(candidate) = chosen.1 {
                consider(&mut best_right, candidate);
            }
        } else {
            // The other wall passes through: stop at the face on the side
            // from which this wall approaches. Using the nearer face to the
            // body centre picks the far face for a branch that meets a wide
            // wall from the opposite side, leaving hidden geometry inside the
            // host and a cap coincident with its outer face.
            let into_body = if at_start {
                body.dir
            } else {
                Point::new(-body.dir.x, -body.dir.y)
            };
            let lateral = dot(into_body, other.left);
            let face = if lateral > 1e-6 {
                other.hi
            } else if lateral < -1e-6 {
                other.lo
            } else {
                let centre = Point::new(
                    point.x + body.left.x * ((body.lo + body.hi) * 0.5),
                    point.y + body.left.y * ((body.lo + body.hi) * 0.5),
                );
                let centre_lateral = dot(
                    Point::new(centre.x - other.start.x, centre.y - other.start.y),
                    other.left,
                );
                if (centre_lateral - other.hi).abs() <= (centre_lateral - other.lo).abs() {
                    other.hi
                } else {
                    other.lo
                }
            };
            let face_anchor = Point::new(
                other.start.x + other.left.x * face,
                other.start.y + other.left.y * face,
            );
            if let Some(candidate) =
                intersect_lines(own_left_anchor, body.dir, face_anchor, other.dir)
            {
                consider(&mut best_left, candidate);
            }
            if let Some(candidate) =
                intersect_lines(own_right_anchor, body.dir, face_anchor, other.dir)
            {
                consider(&mut best_right, candidate);
            }
        }
    }
    Some((best_left?.1, best_right?.1))
}

/// Returns the display footprints of a wall group with the joins derived from
/// the reference lines and body sides of every connected wall. The same
/// footprints drive the 2D outline and the ordinary 3D view; project elements
/// themselves remain unchanged.
pub fn plan_wall_footprints(walls: &[&Element], groups: &[u8]) -> Vec<[Point; 4]> {
    plan_wall_footprints_collect(walls, groups, None)
}

fn plan_wall_footprints_collect(
    walls: &[&Element],
    groups: &[u8],
    mut dependencies: Option<&mut [Vec<usize>]>,
) -> Vec<[Point; 4]> {
    let bodies: Vec<Option<WallBody>> = walls.iter().map(|wall| wall_body(wall)).collect();
    let stories: Vec<_> = walls.iter().map(|wall| wall.story).collect();
    let join_index = WallJoinIndex::new(&bodies);
    walls
        .iter()
        .enumerate()
        .map(|(index, wall)| {
            let Some(body) = bodies[index].as_ref() else {
                return footprint(wall);
            };
            let mut polygon = body_polygon(body);
            if let Some((left, right)) = joined_end_corners(
                &bodies,
                groups,
                &stories,
                index,
                true,
                join_index
                    .as_ref()
                    .map_or_else(
                        || (0..bodies.len()).collect(),
                        |tree| tree.candidates(body, true),
                    )
                    .into_iter(),
                dependencies.as_deref_mut().map(|d| &mut d[index]),
            ) {
                polygon[0] = right;
                polygon[3] = left;
            }
            if let Some((left, right)) = joined_end_corners(
                &bodies,
                groups,
                &stories,
                index,
                false,
                join_index
                    .as_ref()
                    .map_or_else(
                        || (0..bodies.len()).collect(),
                        |tree| tree.candidates(body, false),
                    )
                    .into_iter(),
                dependencies.as_deref_mut().map(|d| &mut d[index]),
            ) {
                polygon[1] = left;
                polygon[2] = right;
            }
            if polygon_area(&polygon).abs() <= 1e-6 || !is_convex_quad(&polygon) {
                body_polygon(body)
            } else {
                polygon
            }
        })
        .collect()
}

/// Story-wide display polygons. Three or more coincident endpoint rays use
/// angular neighbours, rather than the furthest of unrelated pairwise mitres.
/// The hub belongs to the derived display only; reference lines stay editable.
/// Two-wall and face/through-wall connections retain the established join rules.
pub fn plan_wall_polygons(walls: &[&Element], groups: &[u8]) -> Vec<Vec<Point>> {
    plan_wall_polygons_impl(walls, groups, true)
}

fn plan_wall_polygons_impl(walls: &[&Element], groups: &[u8], indexed: bool) -> Vec<Vec<Point>> {
    plan_wall_polygons_collect(walls, groups, indexed, None)
}

/// Conservative physical neighbours collected by the existing narrow phase.
/// A candidate can be rejected by the later corner/hub construction.
pub(crate) fn plan_wall_polygons_with_dependencies(
    walls: &[&Element],
    groups: &[u8],
) -> (Vec<Vec<Point>>, std::collections::BTreeMap<u64, Vec<u64>>) {
    let mut slots = vec![Vec::new(); walls.len()];
    let polygons = plan_wall_polygons_collect(walls, groups, true, Some(&mut slots));
    let mut dependencies: std::collections::BTreeMap<_, Vec<_>> =
        walls.iter().map(|w| (w.id, Vec::new())).collect();
    for (i, neighbours) in slots.iter().enumerate() {
        for &j in neighbours {
            dependencies
                .get_mut(&walls[i].id)
                .unwrap()
                .push(walls[j].id);
            dependencies
                .get_mut(&walls[j].id)
                .unwrap()
                .push(walls[i].id);
        }
    }
    for neighbours in dependencies.values_mut() {
        neighbours.sort_unstable();
        neighbours.dedup();
    }
    (polygons, dependencies)
}

fn plan_wall_polygons_collect(
    walls: &[&Element],
    groups: &[u8],
    indexed: bool,
    dependencies: Option<&mut [Vec<usize>]>,
) -> Vec<Vec<Point>> {
    let bodies: Vec<_> = walls.iter().map(|wall| wall_body(wall)).collect();
    let old = plan_wall_footprints_collect(walls, groups, dependencies);
    // Caps travel from the right face to the left face when looking away from
    // the node. This convention also handles reversed reference lines.
    let mut caps: Vec<[Vec<Point>; 2]> = old
        .iter()
        .map(|p| [vec![p[3], p[0]], vec![p[1], p[2]]])
        .collect();
    let mut multi_end = vec![[false; 2]; walls.len()];
    let mut endpoints = Vec::new();
    for (i, body) in bodies.iter().enumerate() {
        if let Some(body) = body {
            for (end, p) in [(0, body.start), (1, body.end)] {
                endpoints.push((i, end, p));
            }
        }
    }
    endpoints.sort_by(|a, b| {
        a.2.x
            .total_cmp(&b.2.x)
            .then(a.2.y.total_cmp(&b.2.y))
            .then(walls[a.0].id.cmp(&walls[b.0].id))
            .then(a.1.cmp(&b.1))
    });
    let use_window = indexed
        && endpoints
            .iter()
            .all(|(_, _, p)| p.finite() && p.x.abs() <= 100000. && p.y.abs() <= 100000.);
    let mut assigned = vec![false; endpoints.len()];
    for seed in 0..endpoints.len() {
        if assigned[seed] {
            continue;
        }
        let (seed_wall, _, node) = endpoints[seed];
        let group = groups.get(seed_wall).copied().unwrap_or(0);
        if group == 0 {
            continue;
        }
        let mut incident = Vec::new();
        let candidates = if use_window {
            let x = f64::from(node.x);
            endpoints.partition_point(|e| f64::from(e.2.x) < x - 0.01)
                ..endpoints.partition_point(|e| f64::from(e.2.x) <= x + 0.01)
        } else {
            0..endpoints.len()
        };
        for k in candidates {
            let (i, end, p) = endpoints[k];
            if !assigned[k]
                && walls[i].story == walls[seed_wall].story
                && groups.get(i) == Some(&group)
                && p.distance(node) <= 1e-3
            {
                incident.push((i, end));
                assigned[k] = true;
            }
        }
        if incident.len() < 3 {
            continue;
        }
        for &(i, end) in &incident {
            multi_end[i][end] = true;
        }
        let frame = |i: usize, end: usize| {
            let b = bodies[i].unwrap();
            if end == 0 {
                (b.dir, b.left, b.lo, b.hi, b.length)
            } else {
                (
                    Point::new(-b.dir.x, -b.dir.y),
                    Point::new(-b.left.x, -b.left.y),
                    -b.hi,
                    -b.lo,
                    b.length,
                )
            }
        };
        incident.sort_by(|&(i, e), &(j, f)| {
            let a = frame(i, e).0;
            let b = frame(j, f).0;
            a.y.atan2(a.x)
                .total_cmp(&b.y.atan2(b.x))
                .then(walls[i].id.cmp(&walls[j].id))
        });
        let count = incident.len();
        let anchor =
            |n: Point, offset: f64| Point::new(node.x + n.x * offset, node.y + n.y * offset);
        // Reference lines completely outside the body require a material /
        // priority solver. Do not invent a bridge across the offset gap. The
        // conservative fallback is the original body cap, not the old spike.
        let outside = incident.iter().any(|&(i, e)| {
            let (_, _, lo, hi, _) = frame(i, e);
            lo > 1e-5 || hi < -1e-5
        });
        let duplicate_ray = (0..count).any(|k| {
            let (i, e) = incident[k];
            let (j, f) = incident[(k + 1) % count];
            dot(frame(i, e).0, frame(j, f).0) > 1.0 - 1e-6
        });
        if outside || duplicate_ray {
            for &(i, e) in &incident {
                let b = bodies[i].unwrap();
                let p = body_polygon(&b);
                caps[i][e] = if e == 0 {
                    vec![p[3], p[0]]
                } else {
                    vec![p[1], p[2]]
                };
            }
            continue;
        }
        let mut left = vec![node; count];
        let mut right = vec![node; count];
        let mut left_bevel = vec![None; count];
        let mut right_bevel = vec![None; count];
        for k in 0..count {
            let next = (k + 1) % count;
            let (i, e) = incident[k];
            let (j, f) = incident[next];
            let (d, n, lo, hi, len) = frame(i, e);
            let (od, on, olo, ohi, olen) = frame(j, f);
            let a = anchor(n, hi);
            let b = anchor(on, olo);
            let limit = 8.0 * ((hi - lo) + (ohi - olo)).max(0.05);
            let valid = intersect_lines(a, d, b, od).filter(|p| {
                let delta = Point::new(p.x - node.x, p.y - node.y);
                // Reserve room for the other endpoint's join too. Reject a
                // sector as a pair; dropping only one side opens a crack.
                dot(delta, d).abs() <= (len * 0.45).min(limit)
                    && dot(delta, od).abs() <= (olen * 0.45).min(limit)
            });
            if let Some(p) = valid {
                left[k] = p;
                right[next] = p;
            } else {
                left[k] = a;
                right[next] = b;
                // Shared bevel midpoint splits the sector's hub triangle
                // between its two walls, with no arbitrary ownership order.
                let midpoint = interpolate(a, b, 0.5);
                let delta = Point::new(midpoint.x - node.x, midpoint.y - node.y);
                let scale = 1.0_f64
                    .min((len * 0.45) / dot(delta, d).abs().max(1e-8))
                    .min((olen * 0.45) / dot(delta, od).abs().max(1e-8));
                let mid = interpolate(node, midpoint, scale);
                left_bevel[k] = Some(mid);
                right_bevel[next] = Some(mid);
            }
        }
        for (k, &(i, e)) in incident.iter().enumerate() {
            let mut cap = vec![right[k]];
            if let Some(p) = right_bevel[k] {
                cap.push(p);
            }
            cap.push(node);
            if let Some(p) = left_bevel[k] {
                cap.push(p);
            }
            cap.push(left[k]);
            caps[i][e] = cap;
        }
    }
    // The broad face-join tolerance can otherwise attach the free far end of
    // a very short wall back to the same junction. Bound both legacy caps
    // only on walls participating in a true multi-way node.
    for (i, body) in bodies.iter().enumerate() {
        if let Some(body) = body {
            if !multi_end[i].iter().any(|flag| *flag) {
                continue;
            }
            let raw = body_polygon(body);
            for (end, node) in [(0, body.start), (1, body.end)] {
                if !multi_end[i][end]
                    && caps[i][end].iter().any(|p| {
                        dot(Point::new(p.x - node.x, p.y - node.y), body.dir).abs()
                            > body.length * 0.45
                    })
                {
                    caps[i][end] = if end == 0 {
                        vec![raw[3], raw[0]]
                    } else {
                        vec![raw[1], raw[2]]
                    };
                }
            }
        }
    }
    walls
        .iter()
        .enumerate()
        .map(|(i, wall)| {
            let Some(body) = bodies[i] else {
                return footprint(wall).to_vec();
            };
            let mut p = vec![*caps[i][0].last().unwrap()];
            p.extend(caps[i][1].iter().copied());
            p.extend(caps[i][0][..caps[i][0].len() - 1].iter().copied());
            simplify_join_polygon(&mut p);
            if p.len() < 3 || triangulate_polygon(&p).is_empty() {
                body_polygon(&body).to_vec()
            } else {
                p
            }
        })
        .collect()
}

fn simplify_join_polygon(p: &mut Vec<Point>) {
    loop {
        let removable = (0..p.len()).find(|&i| {
            let a = p[(i + p.len() - 1) % p.len()];
            let b = p[i];
            let c = p[(i + 1) % p.len()];
            a.distance(b) <= 1e-6
                || (orientation(a, b, c).abs() <= 1e-7 && point_on_segment(b, a, c, 1e-6))
        });
        if p.len() <= 3 {
            break;
        }
        if let Some(i) = removable {
            p.remove(i);
        } else {
            break;
        }
    }
}

pub fn plan_wall_polygons_by_id(walls: &[&Element], groups: &[u8]) -> Vec<(u64, Vec<Point>)> {
    walls
        .iter()
        .zip(plan_wall_polygons(walls, groups))
        .map(|(wall, p)| (wall.id, p))
        .collect()
}

pub fn plan_wall_polygon_attached_ends(
    walls: &[&Element],
    polygons: &[Vec<Point>],
    groups: &[u8],
) -> Vec<(u64, bool)> {
    let legacy = plan_wall_footprints(walls, groups);
    let mut attached = Vec::new();
    for (i, wall) in walls.iter().enumerate() {
        let Shape::Wall {
            start, end, height, ..
        } = wall.shape
        else {
            continue;
        };
        for (p, at_start, cap) in [
            (start, true, interpolate(legacy[i][0], legacy[i][3], 0.5)),
            (end, false, interpolate(legacy[i][1], legacy[i][2], 0.5)),
        ] {
            if walls.iter().enumerate().any(|(j, other)| {
                if i == j
                    || wall.story != other.story
                    || !groups
                        .get(i)
                        .zip(groups.get(j))
                        .is_some_and(|(a, b)| *a != 0 && a == b)
                {
                    return false;
                }
                let Shape::Wall {
                    start: os,
                    end: oe,
                    height: oh,
                    ..
                } = other.shape
                else {
                    return false;
                };
                (height - oh).abs() <= 1e-4
                    && (contains(cap, &polygons[j])
                        || ((p.distance(os) <= 1e-3 || p.distance(oe) <= 1e-3)
                            && (contains(p, &polygons[j])
                                || polygons[j].iter().any(|q| p.distance(*q) <= 1e-3))))
            }) {
                attached.push((wall.id, at_start));
            }
        }
    }
    attached
}

pub fn wall_polygon_outline_segments(polygons: &[Vec<Point>]) -> Vec<(Point, Point)> {
    wall_polygon_outline_segments_by_wall(polygons)
        .into_iter()
        .map(|(_, a, b)| (a, b))
        .collect()
}

pub fn wall_polygon_outline_segments_by_wall(
    polygons: &[Vec<Point>],
) -> Vec<(usize, Point, Point)> {
    wall_outline_segments_by_wall_for_polygons(polygons)
}

/// Footprints keyed by element id. All render paths use this one story-wide
/// join pass so wall junctions do not depend on material or renovation
/// grouping used for fills, pens or 3D surfaces.
pub fn plan_wall_footprints_by_id(walls: &[&Element], groups: &[u8]) -> Vec<(u64, [Point; 4])> {
    plan_wall_footprints(walls, groups)
        .into_iter()
        .zip(walls.iter().copied())
        .map(|(footprint, wall)| (wall.id, footprint))
        .collect()
}

/// Returns endpoints whose extended wall footprints reach another wall. The
/// renderer uses these to hide overlapping end faces in the 3D display.
pub fn plan_wall_attached_ends(
    walls: &[&Element],
    footprints: &[[Point; 4]],
    groups: &[u8],
) -> Vec<(u64, bool)> {
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
                if other.id == wall.id || other.story != wall.story {
                    return false;
                }
                // Layers with different intersection groups (or group 0) do
                // not join, so their end faces stay visible.
                if !groups
                    .get(index)
                    .zip(groups.get(other_index))
                    .is_some_and(|(a, b)| *a != 0 && *a == *b)
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
    wall_outline_segments_by_wall(wall_footprints)
        .into_iter()
        .map(|(_, start, end)| (start, end))
        .collect()
}

/// Returns every exposed perimeter fragment together with the wall footprint
/// that supplied it. Callers can preserve per-wall pen styles without letting
/// fill or renovation grouping reintroduce an internal join line.
pub fn wall_outline_segments_by_wall(wall_footprints: &[[Point; 4]]) -> Vec<(usize, Point, Point)> {
    let polygons: Vec<Vec<Point>> = wall_footprints
        .iter()
        .map(|polygon| polygon.to_vec())
        .collect();
    wall_outline_segments_by_wall_for_polygons(&polygons)
}

// Interval bounds retain the original f64 cross-product rejection predicate.
struct ClipCandidateIndex {
    roots: Vec<ClipBoundsNode>,
    count: usize,
}
struct ClipEntry {
    slot: usize,
    fields: [[f64; 4]; 3],
    centre: [f64; 2],
}
struct ClipBoundsNode {
    ranges: [[[f64; 2]; 4]; 3],
    winding: f64,
    children: Option<Box<[ClipBoundsNode; 2]>>,
    slots: Vec<usize>,
}
impl ClipBoundsNode {
    fn build(entries: &mut [ClipEntry], winding: f64) -> Self {
        let mut ranges = [[[f64::INFINITY, f64::NEG_INFINITY]; 4]; 3];
        let mut lo = [f64::INFINITY; 2];
        let mut hi = [f64::NEG_INFINITY; 2];
        for entry in entries.iter() {
            for axis in 0..2 {
                lo[axis] = lo[axis].min(entry.centre[axis]);
                hi[axis] = hi[axis].max(entry.centre[axis]);
            }
            for (edge, fields) in entry.fields.iter().enumerate() {
                for (field, &v) in fields.iter().enumerate() {
                    ranges[edge][field][0] = ranges[edge][field][0].min(v);
                    ranges[edge][field][1] = ranges[edge][field][1].max(v);
                }
            }
        }
        if entries.len() <= 8 {
            return Self {
                ranges,
                winding,
                children: None,
                slots: entries.iter().map(|e| e.slot).collect(),
            };
        }
        let axis = usize::from(hi[1] - lo[1] > hi[0] - lo[0]);
        let mid = entries.len() / 2;
        entries.select_nth_unstable_by(mid, |a, b| a.centre[axis].total_cmp(&b.centre[axis]));
        let (left, right) = entries.split_at_mut(mid);
        Self {
            ranges,
            winding,
            children: Some(Box::new([
                Self::build(left, winding),
                Self::build(right, winding),
            ])),
            slots: Vec::new(),
        }
    }
    fn query(&self, x: [f64; 2], y: [f64; 2], slots: &mut Vec<usize>) {
        fn product(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
            let values = [a[0] * b[0], a[0] * b[1], a[1] * b[0], a[1] * b[1]];
            [
                values.into_iter().fold(f64::INFINITY, f64::min),
                values.into_iter().fold(f64::NEG_INFINITY, f64::max),
            ]
        }
        for edge in self.ranges {
            let cx = edge[0];
            let cy = edge[1];
            let dx = edge[2];
            let dy = edge[3];
            let left = product(dx, [y[0] - cy[1], y[1] - cy[0]]);
            let right = product(dy, [x[0] - cx[1], x[1] - cx[0]]);
            let upper = if self.winding > 0. {
                left[1] - right[0]
            } else {
                -(left[0] - right[1])
            };
            if upper < -1e-5 {
                return;
            }
        }
        if let Some(children) = &self.children {
            children[0].query(x, y, slots);
            children[1].query(x, y, slots);
        } else {
            slots.extend_from_slice(&self.slots);
        }
    }
}
impl ClipCandidateIndex {
    fn new(triangles: &[(usize, &[Point; 3], f64)]) -> Option<Self> {
        if triangles.len() <= 32 {
            return None;
        }
        let mut groups = std::collections::BTreeMap::<[i8; 7], Vec<ClipEntry>>::new();
        for (slot, (_, triangle, winding)) in triangles.iter().enumerate() {
            if triangle
                .iter()
                .any(|p| !p.finite() || p.x.abs() > 100000. || p.y.abs() > 100000.)
            {
                return None;
            }
            let mut fields = [[0.; 4]; 3];
            let mut key = [0; 7];
            key[6] = *winding as i8;
            for edge in 0..3 {
                let c = triangle[edge];
                let d = triangle[(edge + 1) % 3];
                fields[edge] = [c.x, c.y, d.x - c.x, d.y - c.y];
                for axis in 0..2 {
                    let v = fields[edge][2 + axis];
                    key[edge * 2 + axis] = if v > 0. {
                        1
                    } else if v < 0. {
                        -1
                    } else {
                        0
                    };
                }
            }
            groups.entry(key).or_default().push(ClipEntry {
                slot,
                fields,
                centre: [triangle[0].x, triangle[0].y],
            });
        }
        Some(Self {
            roots: groups
                .into_iter()
                .map(|(key, mut entries)| ClipBoundsNode::build(&mut entries, f64::from(key[6])))
                .collect(),
            count: triangles.len(),
        })
    }
    fn candidates(&self, a: Point, b: Point) -> Vec<usize> {
        if [a, b]
            .into_iter()
            .any(|p| !p.finite() || p.x.abs() > 100000. || p.y.abs() > 100000.)
        {
            return (0..self.count).collect();
        }
        let mut slots = Vec::new();
        for root in &self.roots {
            root.query(
                [a.x.min(b.x), a.x.max(b.x)],
                [a.y.min(b.y), a.y.max(b.y)],
                &mut slots,
            );
        }
        slots.sort_unstable();
        slots
    }
}

fn wall_outline_segments_by_wall_for_polygons(
    wall_footprints: &[Vec<Point>],
) -> Vec<(usize, Point, Point)> {
    let triangles: Vec<_> = wall_footprints
        .iter()
        .map(|p| {
            triangulate_polygon(p)
                .into_iter()
                .map(|triangle| {
                    let winding = if polygon_area(&triangle) >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    };
                    (triangle, winding)
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let flat: Vec<_> = triangles
        .iter()
        .enumerate()
        .flat_map(|(wall, ts)| ts.iter().map(move |(t, w)| (wall, t, *w)))
        .collect();
    let index = ClipCandidateIndex::new(&flat);
    let mut outline = Vec::new();
    for (wall_index, polygon) in wall_footprints.iter().enumerate() {
        for edge_index in 0..polygon.len() {
            let a = polygon[edge_index];
            let b = polygon[(edge_index + 1) % polygon.len()];
            let mut covered = Vec::new();
            let candidates = index
                .as_ref()
                .map_or_else(|| (0..flat.len()).collect(), |index| index.candidates(a, b));
            for candidate in candidates {
                let (other_index, triangle, winding) = flat[candidate];
                if wall_index == other_index {
                    continue;
                }
                let other = &wall_footprints[other_index];
                if let Some(interval) = clip_segment_with_winding(a, b, triangle, winding) {
                    if interval.1 - interval.0 <= 1e-5 {
                        continue;
                    }
                    let midpoint = interpolate(a, b, (interval.0 + interval.1) * 0.5);
                    let boundary_edge = (0..other.len()).find(|&edge| {
                        segment_distance(midpoint, other[edge], other[(edge + 1) % other.len()])
                            <= 1e-5
                    });
                    let lies_on_other_boundary = boundary_edge.is_some();
                    // A concave polygon's centroid can be outside the polygon.
                    // Its boundary winding, rather than its centroid, gives
                    // the interior side of a coincident edge reliably.
                    let same_interior_side = boundary_edge.is_some_and(|edge| {
                        let c = other[edge];
                        let d = other[(edge + 1) % other.len()];
                        let aligned = dot(
                            Point::new(b.x - a.x, b.y - a.y),
                            Point::new(d.x - c.x, d.y - c.y),
                        )
                        .signum();
                        polygon_area(other).signum() * aligned == polygon_area(polygon).signum()
                    });
                    // A boundary shared from opposite sides is internal and
                    // disappears. Coincident outer edges with interiors on
                    // the same side are retained once.
                    if !lies_on_other_boundary || !same_interior_side || other_index < wall_index {
                        covered.push(interval);
                    }
                }
            }
            covered.sort_by(|left, right| left.0.total_cmp(&right.0));
            let mut cursor = 0.0_f64;
            for (start, end) in covered {
                let start = start.clamp(0.0, 1.0);
                let end = end.clamp(0.0, 1.0);
                if start > cursor + 1e-5 {
                    outline.push((
                        wall_index,
                        interpolate(a, b, cursor),
                        interpolate(a, b, start),
                    ));
                }
                cursor = cursor.max(end);
                if cursor >= 1.0 {
                    break;
                }
            }
            if cursor < 1.0 - 1e-5 {
                outline.push((wall_index, interpolate(a, b, cursor), b));
            }
        }
    }
    outline
}

#[cfg(test)]
fn wall_outline_segments_by_wall_scan(
    wall_footprints: &[Vec<Point>],
) -> Vec<(usize, Point, Point)> {
    let triangles: Vec<_> = wall_footprints
        .iter()
        .map(|p| {
            triangulate_polygon(p)
                .into_iter()
                .map(|triangle| {
                    let winding = if polygon_area(&triangle) >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    };
                    (triangle, winding)
                })
                .collect::<Vec<_>>()
        })
        .collect();
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
                for (triangle, winding) in &triangles[other_index] {
                    if let Some(interval) = clip_segment_with_winding(a, b, triangle, *winding) {
                        if interval.1 - interval.0 <= 1e-5 {
                            continue;
                        }
                        let midpoint = interpolate(a, b, (interval.0 + interval.1) * 0.5);
                        let boundary_edge = (0..other.len()).find(|&edge| {
                            segment_distance(midpoint, other[edge], other[(edge + 1) % other.len()])
                                <= 1e-5
                        });
                        let lies_on_other_boundary = boundary_edge.is_some();
                        // A concave polygon's centroid can be outside the polygon.
                        // Its boundary winding, rather than its centroid, gives
                        // the interior side of a coincident edge reliably.
                        let same_interior_side = boundary_edge.is_some_and(|edge| {
                            let c = other[edge];
                            let d = other[(edge + 1) % other.len()];
                            let aligned = dot(
                                Point::new(b.x - a.x, b.y - a.y),
                                Point::new(d.x - c.x, d.y - c.y),
                            )
                            .signum();
                            polygon_area(other).signum() * aligned == polygon_area(polygon).signum()
                        });
                        // A boundary shared from opposite sides is internal and
                        // disappears. Coincident outer edges with interiors on
                        // the same side are retained once.
                        if !lies_on_other_boundary
                            || !same_interior_side
                            || other_index < wall_index
                        {
                            covered.push(interval);
                        }
                    }
                }
            }
            covered.sort_by(|left, right| left.0.total_cmp(&right.0));
            let mut cursor = 0.0_f64;
            for (start, end) in covered {
                let start = start.clamp(0.0, 1.0);
                let end = end.clamp(0.0, 1.0);
                if start > cursor + 1e-5 {
                    outline.push((
                        wall_index,
                        interpolate(a, b, cursor),
                        interpolate(a, b, start),
                    ));
                }
                cursor = cursor.max(end);
                if cursor >= 1.0 {
                    break;
                }
            }
            if cursor < 1.0 - 1e-5 {
                outline.push((wall_index, interpolate(a, b, cursor), b));
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

/// Joins exposed outline fragments into continuous paths. Plan uses this after
/// bucketing fragments by pen so equal-styled walls still stroke one path.
pub fn wall_outline_paths_from_segments(mut remaining: Vec<(Point, Point)>) -> Vec<Vec<Point>> {
    if remaining.len() <= 32
        || remaining.iter().any(|(a, b)| {
            [a, b]
                .into_iter()
                .any(|p| !p.finite() || p.x.abs() > 100000. || p.y.abs() > 100000.)
        })
    {
        return wall_outline_paths_scan(remaining);
    }
    let mut index = OutlineEndpointIndex::default();
    for (slot, &segment) in remaining.iter().enumerate() {
        index.insert(slot, segment);
    }
    let mut paths = Vec::new();
    while let Some((start, end)) = remaining.pop() {
        index.remove(remaining.len(), (start, end));
        let mut path = vec![start, end];
        loop {
            let tail = *path.last().unwrap();
            let head = path[0];
            let next = index.candidates(head, tail).into_iter().find(|&slot| {
                let (a, b) = remaining[slot];
                a.distance(tail) <= 1e-3
                    || b.distance(tail) <= 1e-3
                    || a.distance(head) <= 1e-3
                    || b.distance(head) <= 1e-3
            });
            let Some(slot) = next else { break };
            let last = remaining.len() - 1;
            let segment = remaining[slot];
            index.remove(slot, segment);
            if slot != last {
                index.remove(last, remaining[last]);
                index.insert(slot, remaining[last]);
            }
            let (a, b) = remaining.swap_remove(slot);
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
        paths.push(path);
    }
    paths
}

#[derive(Default)]
struct OutlineEndpointIndex {
    cells: std::collections::HashMap<(i64, i64), std::collections::BTreeSet<usize>>,
}
impl OutlineEndpointIndex {
    fn cell(p: Point) -> (i64, i64) {
        (
            (f64::from(p.x) / 0.01).floor() as i64,
            (f64::from(p.y) / 0.01).floor() as i64,
        )
    }
    fn insert(&mut self, slot: usize, (a, b): (Point, Point)) {
        for p in [a, b] {
            self.cells.entry(Self::cell(p)).or_default().insert(slot);
        }
    }
    fn remove(&mut self, slot: usize, (a, b): (Point, Point)) {
        for p in [a, b] {
            let cell = Self::cell(p);
            if let Some(slots) = self.cells.get_mut(&cell) {
                slots.remove(&slot);
                if slots.is_empty() {
                    self.cells.remove(&cell);
                }
            }
        }
    }
    fn candidates(&self, a: Point, b: Point) -> std::collections::BTreeSet<usize> {
        let mut slots = std::collections::BTreeSet::new();
        for p in [a, b] {
            let (x, y) = Self::cell(p);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if let Some(found) = self.cells.get(&(x + dx, y + dy)) {
                        slots.extend(found);
                    }
                }
            }
        }
        slots
    }
}

fn wall_outline_paths_scan(mut remaining: Vec<(Point, Point)>) -> Vec<Vec<Point>> {
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

fn interpolate(a: Point, b: Point, t: f64) -> Point {
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

/// Clip a segment against a convex polygon and return the covered parameter
/// interval. Polygon winding may be clockwise or counter-clockwise.
#[cfg(test)]
fn segment_inside_convex_polygon(a: Point, b: Point, polygon: &[Point]) -> Option<(f64, f64)> {
    if polygon.len() < 3 {
        return None;
    }
    let winding = if polygon_area(polygon) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let mut lo = 0.0_f64;
    let mut hi = 1.0_f64;
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

fn clip_segment_with_winding(
    a: Point,
    b: Point,
    polygon: &[Point],
    winding: f64,
) -> Option<(f64, f64)> {
    if polygon.len() < 3 {
        return None;
    }
    let mut lo = 0.0_f64;
    let mut hi = 1.0_f64;
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

pub fn point_on_segment(p: Point, a: Point, b: Point, tolerance: f64) -> bool {
    segment_distance(p, a, b) <= tolerance
        && p.x >= a.x.min(b.x) - tolerance
        && p.x <= a.x.max(b.x) + tolerance
        && p.y >= a.y.min(b.y) - tolerance
        && p.y <= a.y.max(b.y) + tolerance
}
// These optional read handles are the same immutable snapshots already used by
// the current frame. The kernels and picking precedence remain shared below.
fn plan_hit_outline(
    project: &Project,
    e: &Element,
    story: u32,
    batch: Option<&crate::derivation_cache::GeometryBatch>,
) -> Vec<Point> {
    if matches!(e.shape, Shape::Wall { .. } | Shape::Opening { .. }) {
        if let Some(Ok(outline)) = batch.map(|b| b.plan_outline(e.id)) {
            return outline;
        }
    }
    if story == e.story {
        element_plan_outline(project, e)
    } else {
        element_plan_outline_on_story(project, e, story)
    }
}
fn opening_symbol_hit(
    project: &Project,
    e: &Element,
    p: Point,
    tolerance: f64,
    batch: Option<&crate::derivation_cache::GeometryBatch>,
) -> bool {
    let hit = |snapshot: &crate::opening_geometry::OpeningSnapshot| {
        snapshot.symbol.iter().any(|path| {
            (path.closed && contains(p, &path.points))
                || path
                    .points
                    .windows(2)
                    .any(|w| segment_distance(p, w[0], w[1]) <= tolerance)
                || (path.closed
                    && path
                        .points
                        .first()
                        .zip(path.points.last())
                        .is_some_and(|(a, b)| segment_distance(p, *a, *b) <= tolerance))
        })
    };
    if let Some(Ok(g)) = batch.map(|b| b.opening(e.id)) {
        return hit(g.geometry);
    }
    crate::opening_geometry::derive(project, e).is_ok_and(|g| hit(&g.geometry))
}
/// Use only with the current frame's immutable geometry batch from this project.
pub fn hit_plan_with_batch(
    project: &Project,
    story: u32,
    p: Point,
    tolerance: f64,
    batch: &crate::derivation_cache::GeometryBatch,
) -> Option<u64> {
    hit_plan_impl(project, story, p, tolerance, Some(batch))
}
pub fn hit_plan(project: &Project, story: u32, p: Point, tolerance: f64) -> Option<u64> {
    hit_plan_impl(project, story, p, tolerance, None)
}
fn hit_plan_impl(
    project: &Project,
    story: u32,
    p: Point,
    tolerance: f64,
    batch: Option<&crate::derivation_cache::GeometryBatch>,
) -> Option<u64> {
    project
        .elements
        .iter()
        .rev()
        .filter(|e| project.plan_options.element_visible(e, &project.layers))
        .find(|e| {
            e.story == story
                && e.worksheet == project.plan_options.active_worksheet
                && matches!(e.shape, Shape::Opening { .. })
                && if let Some((outline, holes)) = e.authored_prism_rings() {
                    (contains(p, &outline) && !holes.iter().any(|h| contains(p, h)))
                        || std::iter::once(&outline)
                            .chain(holes.iter())
                            .any(|ring| polygon_edge_distance(p, ring) <= tolerance)
                } else if matches!(e.shape, Shape::Slab { .. }) {
                    slab_contains(e, p)
                        || crate::slab_geometry::outlines(e)
                            .iter()
                            .any(|outline| polygon_edge_distance(p, outline) <= tolerance)
                } else {
                    contains(p, &plan_hit_outline(project, e, e.story, batch))
                        || opening_symbol_hit(project, e, p, tolerance, batch)
                }
        })
        .map(|e| e.id)
        .or_else(|| {
            project
                .elements
                .iter()
                .rev()
                .filter(|e| project.plan_options.element_visible(e, &project.layers))
                .filter(|e| {
                    e.story == story
                        && e.worksheet == project.plan_options.active_worksheet
                        && matches!(e.shape, Shape::Wall { .. })
                })
                .find(|e| {
                    let outline = plan_hit_outline(project, e, e.story, batch);
                    contains(p, &outline)
                        || polygon_edge_distance(p, &outline) <= tolerance
                        || crate::wall_geometry::project_station(e, p).1 <= tolerance
                })
                .map(|e| e.id)
        })
        .or_else(|| {
            project
                .elements
                .iter()
                .rev()
                .filter(|e| project.plan_options.element_visible(e, &project.layers))
                .find(|e| {
                    if e.story != story || e.worksheet != project.plan_options.active_worksheet {
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
                .map(|e| e.id)
        })
        .or_else(|| {
            project
                .elements
                .iter()
                .rev()
                .filter(|e| project.plan_options.element_visible(e, &project.layers))
                .find(|e| {
                    element_on_plan_story(project, e, story)
                        && e.worksheet == project.plan_options.active_worksheet
                        && !matches!(e.shape, Shape::Polyline { .. })
                        && !matches!(e.shape, Shape::Opening { .. } | Shape::Wall { .. })
                        && if let Some((outline, holes)) = e.authored_prism_rings() {
                            (contains(p, &outline) && !holes.iter().any(|h| contains(p, h)))
                                || std::iter::once(&outline)
                                    .chain(holes.iter())
                                    .any(|ring| polygon_edge_distance(p, ring) <= tolerance)
                        } else if matches!(e.shape, Shape::Slab { .. }) {
                            slab_contains(e, p)
                                || crate::slab_geometry::outlines(e)
                                    .iter()
                                    .any(|outline| polygon_edge_distance(p, outline) <= tolerance)
                        } else if matches!(
                            e.shape,
                            Shape::Column {
                                inclination: Some(_),
                                ..
                            }
                        ) {
                            let outline = element_plan_outline_on_story(project, e, story);
                            contains(p, &outline) || polygon_edge_distance(p, &outline) <= tolerance
                        } else if let Shape::Column {
                            center,
                            section: crate::column::ColumnSection::Circular { diameter },
                            ..
                        } = e.shape
                        {
                            center.distance(p) <= diameter / 2.
                        } else if let Shape::Roof { ref spec } = e.shape {
                            let c = spec.footprint_contour();
                            c.contains(p)
                                || c.rings()
                                    .any(|ring| polygon_edge_distance(p, ring) <= tolerance)
                        } else {
                            contains(p, &element_plan_outline(project, e))
                        }
                })
                .map(|e| e.id)
        })
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
    node_tolerance: f64,
    edge_tolerance: f64,
) -> Option<PlanHoverKind> {
    plan_hover_kind_impl(project, story, p, node_tolerance, edge_tolerance, None)
}
/// Same hover semantics using the current frame's canonical wall/opening snapshots.
pub fn plan_hover_kind_with_batch(
    project: &Project,
    story: u32,
    p: Point,
    node_tolerance: f64,
    edge_tolerance: f64,
    batch: &crate::derivation_cache::GeometryBatch,
) -> Option<PlanHoverKind> {
    plan_hover_kind_impl(
        project,
        story,
        p,
        node_tolerance,
        edge_tolerance,
        Some(batch),
    )
}
fn plan_hover_kind_impl(
    project: &Project,
    story: u32,
    p: Point,
    node_tolerance: f64,
    edge_tolerance: f64,
    batch: Option<&crate::derivation_cache::GeometryBatch>,
) -> Option<PlanHoverKind> {
    let elements: Vec<_> = project
        .elements
        .iter()
        .filter(|element| {
            project
                .plan_options
                .element_visible(element, &project.layers)
                && element_on_plan_story(project, element, story)
                && element.worksheet == project.plan_options.active_worksheet
        })
        .collect();

    let mut nearest_node = f64::INFINITY;
    for element in &elements {
        let nodes = element_edit_nodes(project, element);
        for node in nodes {
            nearest_node = nearest_node.min(p.distance(node));
        }
    }
    if nearest_node <= node_tolerance {
        return Some(PlanHoverKind::Node);
    }

    let mut nearest_edge = f64::INFINITY;
    for element in &elements {
        match element.shape {
            Shape::ConceptMass { ref spec } => {
                for ring in &spec.paths() {
                    nearest_edge = nearest_edge.min(polygon_edge_distance(p, ring));
                }
            }
            Shape::Zone { ref spec } => {
                for ring in std::iter::once(&spec.outline).chain(spec.holes.iter()) {
                    nearest_edge = nearest_edge.min(polygon_edge_distance(p, ring));
                }
            }
            Shape::Wall { start, end, .. } => {
                let _ = (start, end);
                nearest_edge =
                    nearest_edge.min(crate::wall_geometry::project_station(element, p).1);
                let corners = plan_hit_outline(project, element, element.story, batch);
                for index in 0..corners.len() {
                    nearest_edge = nearest_edge.min(segment_distance(
                        p,
                        corners[index],
                        corners[(index + 1) % corners.len()],
                    ));
                }
            }
            Shape::Slab { .. } => {
                for outline in crate::slab_geometry::outlines(element) {
                    nearest_edge = nearest_edge.min(polygon_edge_distance(p, &outline));
                }
            }
            Shape::Roof { ref spec } => {
                for ring in spec.footprint_contour().rings() {
                    nearest_edge = nearest_edge.min(polygon_edge_distance(p, ring));
                }
            }
            Shape::Beam { .. } | Shape::Column { .. } | Shape::Stair { .. } => {
                let corners = element_plan_outline_on_story(project, element, story);
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
            Shape::Opening { .. } => {
                let corners = plan_hit_outline(project, element, story, batch);
                for index in 0..corners.len() {
                    nearest_edge = nearest_edge.min(segment_distance(
                        p,
                        corners[index],
                        corners[(index + 1) % corners.len()],
                    ));
                }
            }
        }
    }
    if nearest_edge <= edge_tolerance {
        return Some(PlanHoverKind::Edge);
    }

    hit_plan_impl(project, story, p, edge_tolerance, batch).map(|_| PlanHoverKind::Body)
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
    tolerance: f64,
    enabled: bool,
) -> (Point, SnapKind) {
    snap_filtered(project, story, p, tolerance, enabled, |_, _, _| true)
}

/// Apply an operation's candidate policy before choosing the nearest point.
/// The default snap path retains its existing candidates through the wrapper.
pub fn snap_filtered(
    project: &Project,
    story: u32,
    p: Point,
    tolerance: f64,
    enabled: bool,
    mut allow_candidate: impl FnMut(&Element, Point, SnapKind) -> bool,
) -> (Point, SnapKind) {
    if !enabled {
        return (p, SnapKind::Free);
    }
    let mut best = (p, SnapKind::Free, tolerance);
    for e in project.elements.iter().filter(|e| {
        project.plan_options.element_visible(e, &project.layers)
            && element_on_plan_story(project, e, story)
            && e.worksheet == project.plan_options.active_worksheet
    }) {
        if matches!(e.shape, Shape::Opening { .. }) {
            continue;
        }
        let pts = element_snap_points(project, e, story);
        for (q, k) in pts {
            if !allow_candidate(e, q, k) {
                continue;
            }
            let d = p.distance(q);
            if d < best.2 {
                best = (q, k, d)
            }
        }
    }
    if let Some(q) = crate::cad_reference::snap(
        &project.drawings,
        project.plan_options.active_worksheet,
        p,
        best.2,
    ) {
        best = (q, SnapKind::Endpoint, p.distance(q));
    }
    if best.1 != SnapKind::Free {
        return (best.0, best.1);
    }
    (
        Point::new((p.x * 10.).round() / 10., (p.y * 10.).round() / 10.),
        SnapKind::Grid,
    )
}

/// Shared model snap candidates for the active view and transformed references.
/// Reference callers supply their displayed story and scope without mutating a Project.
pub fn element_snap_points(project: &Project, e: &Element, story: u32) -> Vec<(Point, SnapKind)> {
    let mut pts: Vec<(Point, SnapKind)> = match &e.shape {
        Shape::Slab { .. } => slab_snap_points(e),
        Shape::Column {
            inclination: Some(_),
            ..
        } => {
            if !project.plan_options.element_visible(e, &project.layers) {
                return vec![];
            }
            crate::column_plan::reference_points(project, e, story)
                .into_iter()
                .map(|p| (p, SnapKind::SnapPoint))
                .collect()
        }
        Shape::Column { .. } => crate::column::edit_nodes(e)
            .into_iter()
            .map(|p| (p, SnapKind::Endpoint))
            .collect(),
        Shape::Polyline { points, .. } => points
            .iter()
            .copied()
            .map(|point| (point, SnapKind::Endpoint))
            .collect(),
        Shape::Wall { .. } if e.wall_contour.is_some() => wall_edit_nodes(e)
            .into_iter()
            .map(|point| (point, SnapKind::Endpoint))
            .collect(),
        _ => element_plan_outline(project, e)
            .into_iter()
            .map(|point| (point, SnapKind::Endpoint))
            .collect(),
    };
    if let Shape::Wall { start, end, .. } = e.shape {
        pts.extend([
            (start, SnapKind::Endpoint),
            (end, SnapKind::Endpoint),
            (
                crate::wall_geometry::point_tangent_at(
                    e,
                    crate::wall_geometry::reference_length(e) * 0.5,
                )
                .map_or(start, |(point, _)| point),
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
    pts
}

/// Resolve a polyline's live next point against its existing chain and nearby
/// geometry. Constraint candidates are accepted only within the screen-derived
/// tolerance; ordinary endpoint/midpoint snapping keeps priority.
pub fn polyline_snap_target(
    project: &Project,
    story: u32,
    chain: &[Point],
    p: Point,
    tolerance: f64,
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
    if let Some((distance, point)) = chain_snap
        && (!matches!(
            base_kind,
            SnapKind::SnapPoint | SnapKind::Endpoint | SnapKind::Midpoint
        ) || p.distance(base) > distance)
    {
        return (point, SnapKind::Endpoint);
    }
    if matches!(
        base_kind,
        SnapKind::SnapPoint | SnapKind::Endpoint | SnapKind::Midpoint
    ) {
        return (base, base_kind);
    }

    let mut best: Option<(f64, Point, SnapKind)> = None;
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
    for element in project.elements.iter().filter(|element| {
        element.story == story && element.worksheet == project.plan_options.active_worksheet
    }) {
        let (nodes, segments): (Vec<Point>, Vec<(Point, Point)>) = match &element.shape {
            Shape::ConceptMass { spec } => {
                let nodes = spec
                    .outline
                    .iter()
                    .chain(spec.holes.iter().flatten())
                    .copied()
                    .collect();
                let segments = spec
                    .paths()
                    .iter()
                    .flat_map(|r| (0..r.len()).map(|i| (r[i], r[(i + 1) % r.len()])))
                    .collect();
                (nodes, segments)
            }
            Shape::Zone { spec } => {
                let nodes = spec.outline.clone();
                let segments = std::iter::once(&spec.outline)
                    .chain(spec.holes.iter())
                    .flat_map(|r| (0..r.len()).map(|i| (r[i], r[(i + 1) % r.len()])))
                    .collect();
                (nodes, segments)
            }
            Shape::Wall { start, end, .. } => {
                let outline = element_plan_outline(project, element);
                let path = crate::wall_geometry::reference_path(element);
                let mut segments: Vec<_> = path.windows(2).map(|pair| (pair[0], pair[1])).collect();
                segments.extend(
                    (0..outline.len()).map(|i| (outline[i], outline[(i + 1) % outline.len()])),
                );
                (
                    vec![
                        *start,
                        *end,
                        crate::wall_geometry::point_tangent_at(
                            element,
                            crate::wall_geometry::reference_length(element) * 0.5,
                        )
                        .map_or(*start, |(point, _)| point),
                    ],
                    segments,
                )
            }
            Shape::Polyline { points, .. } => (
                points.clone(),
                points.windows(2).map(|pair| (pair[0], pair[1])).collect(),
            ),
            Shape::Slab { .. } => {
                let nodes = slab_edit_nodes(element);
                let segments = crate::slab_geometry::rings(element)
                    .into_iter()
                    .flat_map(|ring| {
                        (0..ring.points.len())
                            .filter(|i| ring.arcs.get(*i).copied().flatten().is_none())
                            .map(|i| (ring.points[i], ring.points[(i + 1) % ring.points.len()]))
                            .collect::<Vec<_>>()
                    })
                    .collect();
                (nodes, segments)
            }
            Shape::Roof { spec } => {
                let c = spec.footprint_contour();
                let nodes = c.rings().flatten().copied().collect();
                let segments = c
                    .rings()
                    .flat_map(|r| (0..r.len()).map(|i| (r[i], r[(i + 1) % r.len()])))
                    .collect();
                (nodes, segments)
            }
            Shape::Column {
                section: crate::column::ColumnSection::Circular { .. },
                ..
            } => (Vec::new(), Vec::new()),
            Shape::Beam { .. } | Shape::Column { .. } | Shape::Stair { .. } => {
                let outline = element_plan_outline(project, element);
                (
                    outline.to_vec(),
                    (0..outline.len())
                        .map(|i| (outline[i], outline[(i + 1) % outline.len()]))
                        .collect(),
                )
            }
            Shape::Opening { .. } => (Vec::new(), Vec::new()),
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
        let angle = step as f64 * std::f64::consts::PI / 6.0;
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
        let angle = (end.y - start.y).atan2(end.x - start.x) + std::f64::consts::FRAC_PI_2;
        for perpendicular in [angle, angle + std::f64::consts::PI] {
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
pub fn corners(project: &Project, e: &Element) -> [DVec3; 8] {
    if matches!(e.shape, Shape::Wall { .. } | Shape::Opening { .. }) {
        let bounds = if matches!(e.shape, Shape::Wall { .. }) {
            crate::wall_solid::derive(project, e).ok().and_then(|g| {
                g.geometry.bounds.or_else(|| {
                    // A fixed eight-point compatibility reference frame for a legal
                    // all-void wall. Physical geometry queries retain bounds=None.
                    let (lo, hi) = crate::wall_solid::vertical_range(project, e)?;
                    let path = &g.geometry.plan_path;
                    Some([
                        DVec3::new(
                            path.iter().map(|p| p.x).fold(f64::INFINITY, f64::min),
                            path.iter().map(|p| p.y).fold(f64::INFINITY, f64::min),
                            lo,
                        ),
                        DVec3::new(
                            path.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max),
                            path.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max),
                            hi,
                        ),
                    ])
                })
            })
        } else {
            crate::opening_geometry::derive(project, e)
                .ok()
                .and_then(|g| g.geometry.bounds)
        };
        return bounds.map_or([DVec3::ZERO; 8], |[lo, hi]| {
            std::array::from_fn(|i| {
                DVec3::new(
                    if matches!(i % 4, 1 | 2) { hi.x } else { lo.x },
                    if matches!(i % 4, 2 | 3) { hi.y } else { lo.y },
                    if i < 4 { lo.z } else { hi.z },
                )
            })
        });
    }
    if matches!(e.shape, Shape::Slab { .. })
        && let Ok(derived) = crate::slab_geometry::derive(e, project.elevation(e.story))
    {
        let [lo, hi] = derived.bounds;
        return std::array::from_fn(|i| {
            DVec3::new(
                if matches!(i % 4, 1 | 2) { hi.x } else { lo.x },
                if matches!(i % 4, 2 | 3) { hi.y } else { lo.y },
                if i < 4 { lo.z } else { hi.z },
            )
        });
    }
    if matches!(e.shape, Shape::Stair { .. }) {
        return crate::stair_geometry::derive(project, e)
            .map(|g| {
                let [lo, hi] = g.geometry.display_bounds;
                std::array::from_fn(|i| {
                    let p = g.geometry.source.footprint[i % 4];
                    DVec3::new(p.x, p.y, if i < 4 { lo.z } else { hi.z })
                })
            })
            .unwrap_or([DVec3::ZERO; 8]);
    }

    if matches!(e.shape, Shape::Column { .. }) {
        return crate::column_geometry::derive(project, e)
            .and_then(|g| {
                g.geometry.compatibility_corners().map_err(|m| {
                    crate::derived_geometry::GeometryDiagnostic::error(
                        e.id,
                        "COLUMN_CORNERS_FAILED",
                        "bounds",
                        m,
                    )
                })
            })
            .unwrap_or([DVec3::ZERO; 8]);
    }
    if matches!(e.shape, Shape::Beam { .. }) {
        return crate::beam_geometry::derive(project, e)
            .map(|g| g.geometry.solid.corners)
            .unwrap_or([DVec3::ZERO; 8]);
    }

    let base = project.elevation(e.story);
    if matches!(e.shape, Shape::Roof { .. }) {
        return crate::roof_geometry::derive(e, base)
            .map(|g| g.geometry.compatibility_corners())
            .unwrap_or([DVec3::ZERO; 8]);
    }
    let (bottom, top) = match e.shape {
        Shape::ConceptMass { ref spec } => (
            base + spec.bottom_offset,
            base + spec.bottom_offset + spec.height,
        ),
        Shape::Zone { ref spec } => (
            base + spec.bottom_offset,
            base + spec.bottom_offset + spec.height,
        ),
        Shape::Roof { .. } => unreachable!("roof corners handled above"),
        Shape::Beam {
            height, top_offset, ..
        } => (base + top_offset - height, base + top_offset),
        Shape::Column {
            height,
            bottom_offset,
            ..
        } => (base + bottom_offset, base + bottom_offset + height),
        Shape::Wall { height, .. } => {
            let (bottom_offset, top_offset) = e
                .wall_settings
                .as_ref()
                .map_or((0., 0.), |settings| settings.vertical_offsets());
            (base + bottom_offset, base + height + top_offset)
        }
        Shape::Slab { thickness, .. } => {
            let top = base + e.slab_contour.as_ref().map_or(0., |c| c.elevation);
            (top - thickness, top)
        }
        Shape::Stair { ref spec } => (base, base + spec.total_rise),
        Shape::Polyline { .. } => (base, base),
        Shape::Opening { sill, height, .. } => (base + sill, base + sill + height),
    };
    let p = element_footprint(project, e);
    std::array::from_fn(|i| DVec3::new(p[i % 4].x, p[i % 4].y, if i < 4 { bottom } else { top }))
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
    pub yaw: f64,
    pub pitch: f64,
    pub span: f64,
    pub target: DVec3,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: -1.05,
            pitch: 0.83,
            span: 21.,
            target: DVec3::new(6., 3., 0.8),
        }
    }
}
/// The same clipped solids used by model display, including newly exposed caps.
fn clipped_pick_triangles(
    project: &Project,
    e: &Element,
    area: &crate::editor::MarqueeArea,
) -> Result<Vec<[DVec3; 3]>, crate::derived_geometry::GeometryDiagnostic> {
    use crate::derived_geometry::GeometryDiagnostic;
    let clip = Some(area.vertices.as_slice());
    let z = area.z_range;
    crate::geometry_validation::validate_clip_context(clip, z)
        .map_err(|m| GeometryDiagnostic::error(e.id, "INVALID_VIEW_CLIP", "clipping", m))?;
    let clip_error = |code, m| GeometryDiagnostic::error(e.id, code, "clipping", m);
    Ok(match &e.shape {
        Shape::ConceptMass { .. } => crate::concept_mass::derive(e, project.elevation(e.story))?
            .clipped(&area.vertices, z)?
            .triangles()
            .collect(),
        Shape::Zone { .. } => crate::zones::derive(e, project.elevation(e.story))?
            .clipped(&area.vertices, z)?
            .triangles()
            .collect(),
        Shape::Wall { .. } => {
            crate::wall_solid::derive(project, e)?
                .geometry
                .clipped(clip, z)
                .map_err(|m| clip_error("WALL_CLIP_FAILED", m))?
                .triangles
        }
        Shape::Opening { .. } => {
            crate::opening_geometry::derive(project, e)?
                .geometry
                .clipped_proxy(clip, z)
                .map_err(|m| clip_error("OPENING_CLIP_FAILED", m))?
                .triangles
        }
        Shape::Slab { .. } => crate::slab_geometry::derive(e, project.elevation(e.story))?
            .clipped(&area.vertices, z)?
            .triangles()
            .collect(),
        Shape::Roof { .. } => {
            crate::roof_geometry::derive(e, project.elevation(e.story))?
                .clipped(clip, z)?
                .mesh
                .triangles
        }
        Shape::Beam { .. } => {
            crate::beam_geometry::derive(project, e)?
                .clipped(clip, z)?
                .mesh
                .triangles
        }
        Shape::Column { .. } => {
            crate::column_geometry::derive(project, e)?
                .clipped(clip, z)?
                .mesh
                .triangles
        }
        Shape::Stair { .. } => {
            crate::stair_geometry::derive(project, e)?
                .clipped(clip, z)?
                .geometry
                .mesh
                .triangles
        }
        Shape::Polyline { .. } => Vec::new(),
    })
}

impl Camera {
    fn direction(&self) -> DVec3 {
        DVec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
        )
    }
    /// Frame the supplied visible geometry in this viewport without changing
    /// direction. Fit the perspective side planes, rather than a world-space
    /// bounding sphere, leaving eight percent of the viewport on each side.
    /// Returns false, with no camera change, when no finite fit is available.
    pub fn fit_points(&mut self, points: &[DVec3], aspect: f64) -> bool {
        if !aspect.is_finite() || aspect <= 0. || !self.yaw.is_finite() || !self.pitch.is_finite() {
            return false;
        }
        let mut finite = points.iter().copied().filter(|p| p.is_finite());
        let Some(first) = finite.next() else {
            return false;
        };
        let (mut lo, mut hi) = (first, first);
        for point in finite {
            lo = lo.min(point);
            hi = hi.max(point);
        }
        // Half before adding, and subtract a local origin before projections:
        // legal survey coordinates must not inflate the framing distance.
        let origin = lo * 0.5 + hi * 0.5;
        let toward_eye = self.direction();
        let right = (-toward_eye).cross(DVec3::Z).normalize_or_zero();
        let up = right.cross(-toward_eye);
        if right == DVec3::ZERO {
            return false;
        }
        let aspect = aspect.max(0.01);
        let tangent = 22.5_f64.to_radians().tan();
        let kx = tangent * aspect * 0.84;
        let ky = tangent * 0.84;
        let (mut left, mut right_limit) = (f64::NEG_INFINITY, f64::INFINITY);
        let (mut bottom, mut top) = (f64::NEG_INFINITY, f64::INFINITY);
        let mut nearest = f64::NEG_INFINITY;
        for point in points.iter().copied().filter(|p| p.is_finite()) {
            let relative = point - origin;
            let x = relative.dot(right);
            let y = relative.dot(up);
            let z = relative.dot(toward_eye);
            left = left.max(x + kx * z);
            right_limit = right_limit.min(x - kx * z);
            bottom = bottom.max(y + ky * z);
            top = top.min(y - ky * z);
            nearest = nearest.max(z);
        }
        // Each pair of perspective side planes gives an interval for the
        // lateral camera center. The smallest feasible distance is where the
        // two interval ends meet. Its midpoint also centers the limiting axis.
        let distance = ((left - right_limit) / (2. * kx))
            .max((bottom - top) / (2. * ky))
            .max(0.1)
            .max(nearest + 0.1)
            // matrix() uses near=max(0.05, distance/5000). Keep even a
            // depth-aligned thin model strictly beyond that changing plane.
            .max((nearest + 0.1) / (1. - 1. / 5000.));
        let target =
            origin + right * (left * 0.5 + right_limit * 0.5) + up * (bottom * 0.5 + top * 0.5);
        let span = 2. * tangent * distance * aspect.min(1.);
        if !target.is_finite() || !span.is_finite() || span <= 0. {
            return false;
        }
        self.target = target;
        self.span = span;
        true
    }
    pub fn eye(&self, aspect: f64) -> DVec3 {
        self.target
            + self.direction() * (self.vertical_span(aspect) / (2. * 22.5_f64.to_radians().tan()))
    }
    /// Looking rotates about the eye, unlike orbiting about the target.
    pub fn look_pixels(&mut self, dx: f64, dy: f64, aspect: f64) {
        let eye = self.eye(aspect);
        let distance = (eye - self.target).length();
        // Looking has its own gentle sensitivity. Orbit's drag gain is far too
        // strong for unpressed mouse motion (100px previously turned ~52 degrees).
        let gain = 45_f64.to_radians() / 600.;
        self.yaw -= dx * gain;
        self.pitch = (self.pitch + dy * gain).clamp(-1.53, 1.53);
        self.target = eye - self.direction() * distance;
    }
    pub fn explore_move(&mut self, x: f64, y: f64, z: f64, seconds: f64, fly: bool) {
        let forward = if fly {
            -self.direction()
        } else {
            DVec3::new(-self.yaw.cos(), -self.yaw.sin(), 0.)
        };
        let right = DVec3::new(-self.yaw.sin(), self.yaw.cos(), 0.);
        let delta = right * x + forward * y + DVec3::Z * z;
        self.target += delta.normalize_or_zero() * seconds;
    }
    pub fn orbit_pixels(&mut self, dx: f64, dy: f64) {
        self.yaw -= dx * 0.009;
        self.pitch = (self.pitch + dy * 0.007).clamp(-1.53, 1.53);
    }
    pub fn pan_pixels(&mut self, dx: f64, dy: f64, width: f64, height: f64) {
        let right = DVec3::new(-self.yaw.sin(), self.yaw.cos(), 0.);
        let up = DVec3::new(
            -self.yaw.cos() * self.pitch.sin(),
            -self.yaw.sin() * self.pitch.sin(),
            self.pitch.cos(),
        );
        self.target += (-right * dx + up * dy) * self.vertical_span(width / height) / height;
    }
    pub fn zoom_scroll(&mut self, scroll: f64) {
        if scroll != 0. {
            self.span = (self.span * (-scroll * 0.002).exp()).clamp(0.001, 1e12);
        }
    }
    /// Keep the shorter viewport dimension stable when resizing split panels.
    pub fn vertical_span(&self, aspect: f64) -> f64 {
        self.span / aspect.clamp(0.01, 1.0)
    }
    pub fn matrix(&self, aspect: f64) -> DMat4 {
        let dir = DVec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
        );
        let span = self.vertical_span(aspect);
        let fov = 45_f64.to_radians();
        let distance = span / (2. * (fov * 0.5).tan());
        // Fit may place the eye many kilometres from a valid local model.
        // Keep that target inside the depth interval and avoid an ill-conditioned
        // inverse projection when picking at large scales. Ordinary views retain
        // the original clipping planes.
        let far = (distance * 4.).max(1000.);
        let near = (far / 20000.).max(0.05);
        // Right-handed perspective, depth 0..1, computed entirely in model precision.
        let f = 1. / (fov * 0.5).tan();
        let z = far / (near - far);
        let projection = DMat4::from_cols(
            glam::DVec4::new(f / aspect.max(0.01), 0., 0., 0.),
            glam::DVec4::new(0., f, 0., 0.),
            glam::DVec4::new(0., 0., z, -1.),
            glam::DVec4::new(0., 0., z * near, 0.),
        );
        let eye = self.target + dir * distance;
        let forward = (self.target - eye).normalize();
        let right = forward.cross(DVec3::Z).normalize();
        let up = right.cross(forward);
        let view = DMat4::from_cols(
            glam::DVec4::new(right.x, up.x, -forward.x, 0.),
            glam::DVec4::new(right.y, up.y, -forward.y, 0.),
            glam::DVec4::new(right.z, up.z, -forward.z, 0.),
            glam::DVec4::new(-right.dot(eye), -up.dot(eye), forward.dot(eye), 1.),
        );
        projection * view
    }
    pub fn pick(
        &self,
        project: &Project,
        ndc: [f64; 2],
        aspect: f64,
        story: Option<u32>,
    ) -> Option<u64> {
        self.pick_in_view(
            project,
            ndc,
            aspect,
            ModelViewFilter {
                story,
                ..Default::default()
            },
        )
    }
    /// Same hit roles and nearest ordering, using one current document read.
    pub fn pick_in_view_cached(
        &self,
        read: &mut crate::derivation_cache::GeometryRead<'_>,
        ndc: [f64; 2],
        aspect: f64,
        filter: ModelViewFilter<'_>,
    ) -> Result<Option<u64>, crate::derived_geometry::GeometryDiagnostic> {
        crate::geometry_validation::validate_clip_context(
            filter.marquee.map(|a| a.vertices.as_slice()),
            filter.marquee.and_then(|a| a.z_range),
        )
        .map_err(|m| {
            crate::derived_geometry::GeometryDiagnostic::error(
                0,
                "INVALID_VIEW_CLIP",
                "clipping",
                m,
            )
        })?;
        let inv = self.matrix(aspect).inverse();
        let origin = inv.project_point3(DVec3::new(ndc[0], ndc[1], 0.));
        let dir = (inv.project_point3(DVec3::new(ndc[0], ndc[1], 1.)) - origin).normalize();
        let project = read.project();
        let mut hit = None;
        let mut nearest = f64::INFINITY;
        let area = filter.clip_area();
        for e in &project.elements {
            if matches!(e.shape, Shape::Polyline { .. })
                || !read.model_includes_valid_clip(e.id, filter)?
            {
                continue;
            }
            let handle = read.source(e.id)?;
            if let Some(t) = handle.source.ray_hit(
                origin,
                dir,
                area.map(|a| a.vertices.as_slice()),
                area.and_then(|a| a.z_range),
            )? {
                if t < nearest {
                    nearest = t;
                    hit = Some(e.id);
                }
            }
        }
        Ok(hit)
    }
    pub fn pick_in_view(
        &self,
        project: &Project,
        ndc: [f64; 2],
        aspect: f64,
        filter: ModelViewFilter<'_>,
    ) -> Option<u64> {
        if crate::geometry_validation::validate_clip_context(
            filter.marquee.map(|a| a.vertices.as_slice()),
            filter.marquee.and_then(|a| a.z_range),
        )
        .is_err()
        {
            return None;
        }
        let inv = self.matrix(aspect).inverse();
        let origin = inv.project_point3(DVec3::new(ndc[0], ndc[1], 0.));
        let dir = (inv.project_point3(DVec3::new(ndc[0], ndc[1], 1.)) - origin).normalize();
        let mut hit = None;
        let mut nearest = f64::INFINITY;
        let mut wall_queries = crate::wall_solid::WallQuery::new(project);
        for e in project
            .elements
            .iter()
            .filter(|e| !matches!(e.shape, Shape::Polyline { .. }) && filter.includes(project, e))
        {
            if matches!(e.shape, Shape::Wall { .. } | Shape::Opening { .. }) {
                let clip = if !filter.outside_marquee && !filter.selection_only {
                    filter.marquee
                } else {
                    None
                };
                let xy = clip.map(|a| a.vertices.as_slice());
                let z = clip.and_then(|a| a.z_range);
                let mesh = if matches!(e.shape, Shape::Wall { .. }) {
                    wall_queries
                        .wall(e.id)
                        .ok()
                        .and_then(|g| g.geometry.clipped(xy, z).ok())
                } else {
                    wall_queries
                        .opening(e.id)
                        .ok()
                        .and_then(|g| g.geometry.clipped_proxy(xy, z).ok())
                };
                if let Some(mesh) = mesh {
                    for [a, b, c] in mesh.triangles {
                        if let Some(t) = ray_triangle(origin, dir, a, b, c)
                            && t < nearest
                        {
                            nearest = t;
                            hit = Some(e.id);
                        }
                    }
                }
                continue;
            }
            if !filter.outside_marquee
                && !filter.selection_only
                && let Some(area) = filter.marquee
            {
                for [a, b, c] in clipped_pick_triangles(project, e, area)
                    .into_iter()
                    .flatten()
                {
                    if let Some(t) = ray_triangle(origin, dir, a, b, c)
                        && t < nearest
                    {
                        nearest = t;
                        hit = Some(e.id);
                    }
                }
                continue;
            }
            if matches!(e.shape, Shape::Stair { .. }) {
                if let Ok(g) = crate::stair_geometry::derive(project, e) {
                    for [a, b, c] in g.geometry.source.triangles {
                        if let Some(t) = ray_triangle(origin, dir, a, b, c)
                            && t < nearest
                        {
                            nearest = t;
                            hit = Some(e.id);
                        }
                    }
                }
                continue;
            }
            if matches!(e.shape, Shape::Roof { .. }) {
                for [a, b, c] in crate::roof_geometry::derive(e, project.elevation(e.story))
                    .into_iter()
                    .flat_map(|g| g.geometry.mesh.triangles)
                {
                    if let Some(t) = ray_triangle(origin, dir, a, b, c)
                        && t < nearest
                    {
                        nearest = t;
                        hit = Some(e.id);
                    }
                }
                continue;
            }
            if matches!(e.shape, Shape::Slab { .. }) {
                if let Ok(triangles) =
                    crate::slab_geometry::solid_triangles(e, project.elevation(e.story))
                {
                    for [a, b, c] in triangles {
                        if let Some(t) = ray_triangle(origin, dir, a, b, c)
                            && t < nearest
                        {
                            nearest = t;
                            hit = Some(e.id);
                        }
                    }
                }
                continue;
            }
            if matches!(e.shape, Shape::Beam { .. }) {
                if let Ok(g) = crate::beam_geometry::derive(project, e) {
                    for [a, b, c] in g.geometry.mesh.triangles {
                        if let Some(t) = ray_triangle(origin, dir, a, b, c)
                            && t < nearest
                        {
                            nearest = t;
                            hit = Some(e.id);
                        }
                    }
                }
                continue;
            }
            if matches!(e.shape, Shape::Column { .. }) {
                if let Ok(g) = crate::column_geometry::derive(project, e)
                    && let Some(t) = g.geometry.ray_hit(origin, dir)
                    && t < nearest
                {
                    nearest = t;
                    hit = Some(e.id);
                }
                continue;
            }
        }
        hit
    }
    pub fn point_on_horizontal_plane(&self, ndc: [f64; 2], aspect: f64, z: f64) -> Option<Point> {
        let inverse = self.matrix(aspect).inverse();
        let near = inverse.project_point3(DVec3::new(ndc[0], ndc[1], 0.));
        // Unprojecting exactly at the far plane loses precision in
        // f64. Any point along the same perspective ray is sufficient; use
        // an interior depth to keep the ground-plane intersection stable.
        let far = inverse.project_point3(DVec3::new(ndc[0], ndc[1], 0.99));
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
pub(crate) fn ray_triangle(o: DVec3, d: DVec3, a: DVec3, b: DVec3, c: DVec3) -> Option<f64> {
    let e1 = b - a;
    let e2 = c - a;
    let h = d.cross(e2);
    let det = e1.dot(h);
    if det.abs() < crate::precision::GeometryContext::TOPOLOGY.pick_ray_determinant_m2 {
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
        assert!((centered.iter().map(|point| point.y).sum::<f64>() / 4. - 2.).abs() < 1e-6);

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
        assert!((shifted.iter().map(|point| point.y).sum::<f64>() / 4. - 2.25).abs() < 1e-6);
    }

    #[test]
    fn wall_reference_snap_projects_onto_another_baseline_and_skips_the_edited_wall() {
        let mut project = Project::default();
        let edited = project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(4., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        let target = project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(2., 2.),
                end: Point::new(2., -1.),
                thickness: 0.3,
                height: 3.,
            },
        );
        let snap = wall_reference_snap(&project, 0, edited, Point::new(2.04, 0.12), 0.1)
            .expect("nearby target baseline");
        assert_eq!(snap.wall_id, target);
        assert!(snap.point.distance(Point::new(2., 0.12)) < 1e-6);
        assert!((snap.distance - 0.04).abs() < 1e-6);
        assert!(
            wall_reference_snap(&project, 0, edited, Point::new(5., 3.), 0.1).is_none(),
            "a point outside the snap radius must stay free"
        );
        assert!(
            wall_reference_snap(&project, 1, edited, Point::new(2.04, 0.12), 0.1).is_none(),
            "walls on another story are not snap targets"
        );
        assert!(
            wall_reference_snap(&project, 0, target, Point::new(2.04, 2.5), 0.1).is_none(),
            "the edited wall is excluded from its own snap targets"
        );
    }

    #[test]
    fn wall_reference_snap_accepts_a_pointer_inside_a_thick_foreign_material_wall() {
        let mut project = Project::default();
        let edited = project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(4., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        let target = project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(2., 2.),
                end: Point::new(2., -2.),
                thickness: 1.,
                height: 3.,
            },
        );
        // 0.35 m is 17.5 px at 50 px/m, well outside a 12 px snap radius, but
        // the pointer is still inside the target wall body. Dragging onto the
        // visible wall, not only onto its reference line, must connect.
        let snap = wall_reference_snap(&project, 0, edited, Point::new(2.35, 0.12), 0.24)
            .expect("pointer inside the target wall body");
        assert_eq!(snap.wall_id, target);
        assert!(snap.point.distance(Point::new(2., 0.12)) < 1e-6);
        assert!(
            snap.distance > 0.24,
            "the accepted candidate came from body containment, not reference-line proximity"
        );
    }

    #[test]
    fn wall_reference_snap_accepts_the_baseline_when_the_body_is_offset_away() {
        let mut project = Project::default();
        let edited = project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(4., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        let target = project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(2., 2.),
                end: Point::new(2., -2.),
                thickness: 1.,
                height: 3.,
            },
        );
        project
            .elements
            .iter_mut()
            .find(|element| element.id == target)
            .and_then(|element| element.wall_settings.as_mut())
            .expect("target wall settings")
            .geometry
            .reference_offset = 1.;

        let snap = wall_reference_snap(&project, 0, edited, Point::new(2.04, 0.12), 0.1)
            .expect("reference-line proximity must not depend on the shifted body");
        assert_eq!(snap.wall_id, target);
        assert!(snap.point.distance(Point::new(2., 0.12)) < 1e-6);
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
            |wall: &Element| footprint(wall).iter().map(|point| point.y).sum::<f64>() / 4.;
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
        assert!((sweep.abs() - std::f64::consts::PI).abs() < 1e-5);
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
    fn shared_edit_nodes_exclude_wall_body_corners_and_arc_samples() {
        let mut project = Project::default();
        project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(4., 0.),
                thickness: 1.,
                height: 3.,
            },
        );
        let wall = &project.elements[0];
        assert_eq!(
            element_edit_nodes(&project, wall),
            vec![Point::new(0., 0.), Point::new(4., 0.)]
        );
        assert!(!element_edit_nodes(&project, wall).contains(&Point::new(4., -0.5)));
        project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(10., 0.), Point::new(12., 0.)],
                arcs: vec![Some(
                    arc_segment_through(
                        Point::new(10., 0.),
                        Point::new(12., 0.),
                        Point::new(11., 1.),
                    )
                    .unwrap(),
                )],
            },
        );
        assert_eq!(
            element_edit_nodes(&project, &project.elements[1]),
            vec![Point::new(10., 0.), Point::new(12., 0.)]
        );
        assert_eq!(
            plan_hover_kind(&project, 0, Point::new(11., 1.), 0.01, 0.01),
            Some(PlanHoverKind::Edge)
        );
    }

    #[test]
    fn shared_edit_nodes_keep_stair_selection_and_hover_on_the_same_footprint() {
        let mut project = Project::default();
        project.add(
            0,
            Material::Concrete,
            Shape::Stair {
                spec: crate::stairs::StairSpec::default(),
            },
        );
        let element = &project.elements[0];
        let nodes = element_edit_nodes(&project, element);
        assert_eq!(nodes, footprint(element).to_vec());
        let corner = nodes[0];
        let polygon = [
            Point::new(corner.x - 0.02, corner.y - 0.02),
            Point::new(corner.x + 0.02, corner.y - 0.02),
            Point::new(corner.x + 0.02, corner.y + 0.02),
            Point::new(corner.x - 0.02, corner.y + 0.02),
        ];
        assert!(element_has_node_in_polygon(&project, element, &polygon));
        assert_eq!(
            plan_hover_kind(&project, 0, corner, 0.01, 0.01),
            Some(PlanHoverKind::Node)
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
        assert!(!element_has_node_in_polygon(&project, e, &area));
        assert!(element_intersects_polygon(&project, e, &area));
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
        let groups = vec![1u8; project.elements.len()];
        let walls: Vec<_> = project.elements.iter().collect();
        let footprints = plan_wall_footprints(&walls, &groups);
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
        let groups = vec![1u8; project.elements.len()];
        let walls: Vec<_> = project.elements.iter().collect();
        let footprints = plan_wall_footprints(&walls, &groups);
        let outline = wall_outline_segments(&footprints);
        assert!(
            plan_wall_attached_ends(&walls, &footprints, &groups).contains(&(walls[1].id, true)),
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
    fn cross_material_and_renovation_t_junction_trims_caps_and_outline() {
        let mut project = Project::default();
        let main = project.add_with_status(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(4., 0.),
                thickness: 0.2,
                height: 3.,
            },
            crate::view_settings::RenovationStatus::Existing,
        );
        let branch = project.add_with_status(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(2., 0.),
                end: Point::new(2., 2.),
                thickness: 0.2,
                height: 3.,
            },
            crate::view_settings::RenovationStatus::New,
        );
        let groups = vec![1u8; project.elements.len()];
        let walls: Vec<_> = project.elements.iter().collect();
        let footprints = plan_wall_footprints(&walls, &groups);
        let outline = wall_outline_segments(&footprints);
        assert_eq!(walls[0].id, main);
        assert_eq!(walls[1].id, branch);
        assert!(
            (footprints[1][0].y - 0.1).abs() < 1e-4 && (footprints[1][3].y - 0.1).abs() < 1e-4,
            "the branch must stop at the host's near face across materials: {:?}",
            footprints[1]
        );
        assert!(
            plan_wall_attached_ends(&walls, &footprints, &groups).contains(&(branch, true)),
            "3D must hide a buried end cap even when materials and renovation differ"
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
        let groups = vec![1u8; project.elements.len()];
        let walls: Vec<_> = project.elements.iter().collect();
        let footprints = plan_wall_footprints(&walls, &groups);
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
        let groups = vec![1u8; project.elements.len()];
        let walls: Vec<_> = project.elements.iter().collect();
        let footprints = plan_wall_footprints(&walls, &groups);
        let outline = wall_outline_segments(&footprints);
        let paths = wall_outline_paths(&footprints);
        assert!(contains(Point::new(7.55, 4.05), &footprints[0]));
        assert!(contains(Point::new(7.55, 4.05), &footprints[1]));
        assert!(
            contains(Point::new(7.45, 3.95), &footprints[0])
                || contains(Point::new(7.45, 3.95), &footprints[1]),
            "extended rectangular wall bodies should fill the outside corner without a bevel"
        );
        assert!(
            plan_wall_attached_ends(&walls, &footprints, &groups).contains(&(walls[0].id, true))
        );
        assert!(
            plan_wall_attached_ends(&walls, &footprints, &groups).contains(&(walls[1].id, true))
        );
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
    fn angled_l_corner_uses_the_true_mitre_instead_of_a_half_thickness_spike() {
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
                start: Point::new(4., 0.),
                end: Point::new(6., 2.),
                thickness: 0.2,
                height: 3.,
            },
        );
        let groups = vec![1u8; project.elements.len()];
        let walls: Vec<_> = project.elements.iter().collect();
        let footprints = plan_wall_footprints(&walls, &groups);
        let left = footprints[0][1];
        let right = footprints[0][2];
        assert!(
            left.distance(Point::new(3.958_579, 0.1)) <= 1e-4,
            "{left:?}"
        );
        assert!(
            right.distance(Point::new(4.041_421, -0.1)) <= 1e-4,
            "{right:?}"
        );
        assert!(
            footprints[0]
                .iter()
                .all(|point| (point.x - 4.1).abs() > 1e-4),
            "a 45-degree mitre must not extend by one full half-thickness: {:?}",
            footprints[0]
        );
        let outline = wall_outline_segments(&footprints);
        assert!(
            !outline
                .iter()
                .any(|(a, b)| a.distance(left) <= 1e-4 && b.distance(right) <= 1e-4),
            "the end cap must be replaced by the mitre: {outline:?}"
        );
    }

    /// AC29 layer intersection groups: only same non-zero groups join, so a
    /// wall pair on different groups keeps its own end caps.
    #[test]
    fn collinear_walls_on_different_intersection_groups_keep_end_caps() {
        fn corner_pair() -> Project {
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
                    start: Point::new(4., 0.),
                    end: Point::new(4., 3.),
                    thickness: 0.2,
                    height: 3.,
                },
            );
            project
        }

        /// True when the first wall's end corners reach the second wall's
        /// faces (x 3.9 / 4.1) — the sign of a processed corner join.
        fn first_wall_joins(footprints: &[[Point; 4]]) -> bool {
            footprints[0]
                .iter()
                .any(|p| (p.x - 3.9).abs() <= 1e-4 || (p.x - 4.1).abs() <= 1e-4)
        }

        let mut project = corner_pair();
        let walls: Vec<_> = project.elements.iter().collect();
        let joined_groups = vec![1u8; walls.len()];
        let joined = plan_wall_footprints(&walls, &joined_groups);
        assert!(
            first_wall_joins(&joined),
            "same group processes the corner join: {joined:?}"
        );

        let project = corner_pair();
        let walls: Vec<_> = project.elements.iter().collect();
        let split_groups = vec![1u8, 2u8];
        let split = plan_wall_footprints(&walls, &split_groups);
        assert!(
            !first_wall_joins(&split),
            "different groups keep the square end caps: {split:?}"
        );

        let zero_groups = vec![0u8, 0u8];
        let zero = plan_wall_footprints(&walls, &zero_groups);
        assert!(
            !first_wall_joins(&zero),
            "group 0 never intersects: {zero:?}"
        );
    }

    #[test]
    fn l_corner_with_different_thickness_meets_on_matching_faces() {
        let mut project = Project::default();
        project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(4., 0.),
                thickness: 0.3,
                height: 3.,
            },
        );
        project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(4., 0.),
                end: Point::new(4., 3.),
                thickness: 0.2,
                height: 3.,
            },
        );
        let groups = vec![1u8; project.elements.len()];
        let walls: Vec<_> = project.elements.iter().collect();
        let footprints = plan_wall_footprints(&walls, &groups);
        let expected = [Point::new(3.9, 0.15), Point::new(4.1, -0.15)];
        for point in expected {
            assert!(footprints[0].iter().any(|p| p.distance(point) <= 1e-4));
            assert!(footprints[1].iter().any(|p| p.distance(point) <= 1e-4));
        }
        assert!(
            footprints[0]
                .iter()
                .all(|point| (point.y - 0.15).abs() <= 1e-4 || (point.y + 0.15).abs() <= 1e-4),
            "the thick wall keeps its own faces: {:?}",
            footprints[0]
        );
    }

    #[test]
    fn t_junction_branch_stops_on_the_face_it_approaches() {
        let mut project = Project::default();
        project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(4., 0.),
                thickness: 0.4,
                height: 3.,
            },
        );
        project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(2., -2.),
                end: Point::new(2., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        project.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::new(2., 2.),
                end: Point::new(2., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        let groups = vec![1u8; project.elements.len()];
        let walls: Vec<_> = project.elements.iter().collect();
        let footprints = plan_wall_footprints(&walls, &groups);
        assert!(
            (footprints[1][1].y + 0.2).abs() <= 1e-4,
            "{:?}",
            footprints[1]
        );
        assert!(
            (footprints[1][2].y + 0.2).abs() <= 1e-4,
            "{:?}",
            footprints[1]
        );
        assert!(
            (footprints[2][1].y - 0.2).abs() <= 1e-4,
            "{:?}",
            footprints[2]
        );
        assert!(
            (footprints[2][2].y - 0.2).abs() <= 1e-4,
            "{:?}",
            footprints[2]
        );
    }

    #[test]
    fn four_way_crossing_closes_the_centre_without_cutting_arm_ends_at_bevels() {
        let mut project = Project::default();
        for (start, end) in [
            (Point::new(-2., 0.), Point::new(0., 0.)),
            (Point::new(2., 0.), Point::new(0., 0.)),
            (Point::new(0., 2.), Point::new(0., 0.)),
            (Point::new(0., -2.), Point::new(0., 0.)),
        ] {
            project.add(
                0,
                Material::Concrete,
                Shape::Wall {
                    start,
                    end,
                    thickness: 0.2,
                    height: 3.,
                },
            );
        }
        let groups = vec![1u8; project.elements.len()];
        let walls: Vec<_> = project.elements.iter().collect();
        let footprints = plan_wall_footprints(&walls, &groups);
        for x in [-0.05, 0., 0.05] {
            for y in [-0.05, 0., 0.05] {
                assert!(
                    footprints
                        .iter()
                        .any(|polygon| contains(Point::new(x, y), polygon)),
                    "the centre square must be filled at ({x}, {y})"
                );
            }
        }
        let outline = wall_outline_segments(&footprints);
        assert!(
            outline
                .iter()
                .all(|(a, b)| (a.x - b.x).abs() <= 1e-4 || (a.y - b.y).abs() <= 1e-4),
            "the cross perimeter must not contain bevel edges: {outline:?}"
        );
        let paths = wall_outline_paths(&footprints);
        assert_eq!(paths.len(), 1, "a symmetric cross has one outer perimeter");
        assert!(paths[0][0].distance(*paths[0].last().unwrap()) <= 1e-3);
    }

    #[test]
    fn reference_line_modes_and_numeric_offset_join_on_the_shifted_faces() {
        use crate::wall::WallReferenceLine;

        for (line, offset, expected) in [
            (
                WallReferenceLine::Center,
                0.,
                [Point::new(-0.12, -0.12), Point::new(0.12, 0.12)],
            ),
            (
                WallReferenceLine::Inside,
                0.,
                [Point::new(0., 0.), Point::new(0.24, 0.24)],
            ),
            (
                WallReferenceLine::Outside,
                0.,
                [Point::new(-0.24, -0.24), Point::new(0., 0.)],
            ),
            (
                WallReferenceLine::Center,
                0.2,
                [Point::new(-0.32, -0.32), Point::new(-0.08, -0.08)],
            ),
        ] {
            let mut project = Project::default();
            let a = project.add(
                0,
                Material::Concrete,
                Shape::Wall {
                    start: Point::new(4., 0.),
                    end: Point::new(0., 0.),
                    thickness: 0.24,
                    height: 3.,
                },
            );
            let b = project.add(
                0,
                Material::Concrete,
                Shape::Wall {
                    start: Point::new(0., 0.),
                    end: Point::new(0., 4.),
                    thickness: 0.24,
                    height: 3.,
                },
            );
            for id in [a, b] {
                let settings = project
                    .elements
                    .iter_mut()
                    .find(|element| element.id == id)
                    .and_then(|element| element.wall_settings.as_mut())
                    .unwrap();
                settings.geometry.reference_line = line;
                settings.geometry.reference_offset = offset;
            }
            let groups = vec![1u8; project.elements.len()];
            let walls: Vec<_> = project.elements.iter().collect();
            let footprints = plan_wall_footprints(&walls, &groups);
            for point in expected {
                assert!(
                    footprints[0].iter().any(|p| p.distance(point) <= 1e-4),
                    "{line:?} {offset}: {point:?} missing from {:?}",
                    footprints[0]
                );
                assert!(
                    footprints[1].iter().any(|p| p.distance(point) <= 1e-4),
                    "{line:?} {offset}: {point:?} missing from {:?}",
                    footprints[1]
                );
            }
            assert_eq!(
                wall_reference_line(walls[0]),
                Some((Point::new(4., 0.), Point::new(0., 0.)))
            );
            assert_eq!(
                wall_reference_line(walls[1]),
                Some((Point::new(0., 0.), Point::new(0., 4.)))
            );
        }
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
            .project_point3(DVec3::new(world.x, world.y, 0.));
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
        let area: f64 = fragments.iter().map(|p| polygon_area(p).abs()).sum();
        assert!((area - 3.).abs() < 1e-4, "clipped area was {area}");
        assert!(fragments.iter().flatten().all(|p| contains(*p, &concave)));
    }

    #[test]
    fn camera_projection_is_perspective() {
        let camera = Camera::default();
        let matrix = camera.matrix(1.5);
        let dir = DVec3::new(
            camera.yaw.cos() * camera.pitch.cos(),
            camera.yaw.sin() * camera.pitch.cos(),
            camera.pitch.sin(),
        );
        let camera_distance = camera.vertical_span(1.5) / (2. * (45_f64.to_radians() * 0.5).tan());
        let distant = camera.target + dir * (camera_distance * 0.2);
        let close = camera.target + dir * (camera_distance * 0.7);
        let side = DVec3::new(-camera.yaw.sin(), camera.yaw.cos(), 0.) * 2.;
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

#[cfg(test)]
mod contour_geometry_tests {
    use super::*;

    fn curved_project() -> (Project, u64) {
        let mut project = Project::default();
        let id = project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(4., 0.),
                thickness: 0.3,
                height: 3.,
            },
        );
        project.elements[0].wall_contour = Some(WallContour::Arc {
            arc: arc_segment_through(Point::new(0., 0.), Point::new(4., 0.), Point::new(2., 2.))
                .unwrap(),
        });
        (project, id)
    }

    #[test]
    fn contour_plan_hit_and_hover_follow_arc_not_chord_or_bounds() {
        let (project, id) = curved_project();
        assert_eq!(hit_plan(&project, 0, Point::new(2., 2.), 0.01), Some(id));
        assert_eq!(hit_plan(&project, 0, Point::new(2., 0.), 0.01), None);
        assert_eq!(hit_plan(&project, 0, Point::new(2., 1.), 0.01), None);
        assert_eq!(
            plan_hover_kind(&project, 0, Point::new(2., 2.), 0.02, 0.02),
            Some(PlanHoverKind::Edge)
        );
        assert_eq!(
            plan_hover_kind(&project, 0, Point::new(2., 0.), 0.02, 0.02),
            None
        );
        let (point, kind) = snap(&project, 0, Point::new(2.001, 2.001), 0.02, true);
        assert_eq!(kind, SnapKind::Midpoint);
        assert!(point.distance(Point::new(2., 2.)) < 1e-5);
        let projected =
            wall_reference_projection(&project.elements[0], Point::new(2., 3.)).unwrap();
        assert!(projected.distance(Point::new(2., 2.)) < 1e-5);
    }

    #[test]
    fn contour_opening_stations_and_solid_hole_use_arc_length() {
        let (mut project, host) = curved_project();
        let length = crate::wall_geometry::reference_length(&project.elements[0]);
        assert!((length - std::f64::consts::TAU).abs() < 1e-4);
        let opening = project.add(
            0,
            Material::Concrete,
            Shape::Opening {
                host_wall: host,
                kind: OpeningKind::Window,
                offset: length * 0.5,
                width: 1.,
                sill: 1.,
                height: 1.,
            },
        );
        let wall = &project.elements[0];
        let open = project.elements.iter().find(|e| e.id == opening).unwrap();
        let (a, b) = opening_span(&project, open).unwrap();
        assert!((wall_offset_of(&project, host, a).unwrap() - (length * 0.5 - 0.5)).abs() < 1e-4);
        assert!((wall_offset_of(&project, host, b).unwrap() - (length * 0.5 + 0.5)).abs() < 1e-4);
        let point = Point::new(2., 2.);
        assert_eq!(hit_plan(&project, 0, point, 0.01), Some(opening));
        let solids = wall_solid_prisms(&project, wall);
        let contains_at = |z: f64| {
            solids.iter().any(|s| {
                z > s[0].z + 1e-4
                    && z < s[4].z - 1e-4
                    && contains(
                        point,
                        &s[..4]
                            .iter()
                            .map(|v| Point::new(v.x, v.y))
                            .collect::<Vec<_>>(),
                    )
            })
        };
        assert!(contains_at(0.5));
        assert!(
            !contains_at(1.5),
            "window hole must remain empty across the curved strip"
        );
        assert!(contains_at(2.5));
    }

    #[test]
    fn contour_camera_picks_visible_arc_and_rejects_empty_box_interior() {
        let (project, id) = curved_project();
        let camera = Camera {
            target: DVec3::new(2., 1., 0.),
            yaw: 0.,
            pitch: 1.5,
            span: 10.,
        };
        let project_point = |p: DVec3| {
            let n = camera.matrix(1.).project_point3(p);
            [n.x, n.y]
        };
        assert_eq!(
            camera.pick(&project, project_point(DVec3::new(2., 2., 3.)), 1., Some(0)),
            Some(id)
        );
        assert_eq!(
            camera.pick(&project, project_point(DVec3::new(2., 1., 3.)), 1., Some(0)),
            None
        );
    }

    #[test]
    fn contour_trapezoid_hit_snap_and_marquee_use_outer_nodes() {
        let (mut project, id) = curved_project();
        project.elements[0].wall_contour = Some(WallContour::Trapezoid {
            outer_start: Point::new(0., 0.5),
            outer_end: Point::new(4., 1.),
            outer_nodes: vec![Point::new(2., 1.5)],
            outer_positive: true,
        });
        let wall = &project.elements[0];
        assert_eq!(hit_plan(&project, 0, Point::new(2., 1.2), 0.01), Some(id));
        assert_eq!(
            plan_hover_kind(&project, 0, Point::new(2., 1.5), 0.02, 0.02),
            Some(PlanHoverKind::Node)
        );
        let polygon = [
            Point::new(1.9, 1.4),
            Point::new(2.1, 1.4),
            Point::new(2.1, 1.6),
            Point::new(1.9, 1.6),
        ];
        assert!(element_has_node_in_polygon(&project, wall, &polygon));
        assert!(element_intersects_polygon(&project, wall, &polygon));
        let empty = [
            Point::new(0.1, 1.2),
            Point::new(0.3, 1.2),
            Point::new(0.3, 1.4),
            Point::new(0.1, 1.4),
        ];
        assert!(!element_intersects_polygon(&project, wall, &empty));
    }
    fn junction_project(angles: &[f64], lengths: &[f64], widths: &[f64]) -> Project {
        let mut p = Project::default();
        for (i, angle) in angles.iter().enumerate() {
            let r = angle.to_radians();
            p.add(
                0,
                Material::Concrete,
                Shape::Wall {
                    start: Point::default(),
                    end: Point::new(r.cos() * lengths[i], r.sin() * lengths[i]),
                    thickness: widths[i],
                    height: 3.,
                },
            );
        }
        p
    }
    fn junction_polygons(p: &Project) -> Vec<Vec<Point>> {
        plan_wall_polygons(
            &p.elements.iter().collect::<Vec<_>>(),
            &vec![1; p.elements.len()],
        )
    }
    fn junction_union_area(polys: &[Vec<Point>]) -> f64 {
        wall_polygon_outline_segments(polys)
            .iter()
            .map(|(a, b)| a.x * b.y - b.x * a.y)
            .sum::<f64>()
            .abs()
            * 0.5
    }
    fn assert_valid_junction(polys: &[Vec<Point>]) {
        for poly in polys {
            assert!(poly.iter().all(|p| p.finite()), "{poly:?}");
            for i in 0..poly.len() {
                for j in i + 1..poly.len() {
                    if j == i + 1 || (i == 0 && j == poly.len() - 1) {
                        continue;
                    }
                    let (a, b, c, d) = (
                        poly[i],
                        poly[(i + 1) % poly.len()],
                        poly[j],
                        poly[(j + 1) % poly.len()],
                    );
                    let proper_cross = orientation(a, b, c) * orientation(a, b, d) < -1e-10
                        && orientation(c, d, a) * orientation(c, d, b) < -1e-10;
                    assert!(!proper_cross, "self crossing {poly:?}");
                }
            }
            let triangles = triangulate_polygon(poly);
            assert!(!triangles.is_empty(), "{poly:?}");
            let area: f64 = triangles.iter().map(|t| polygon_area(t).abs()).sum();
            assert!((area - polygon_area(poly).abs()).abs() < 1e-4, "{poly:?}");
        }
    }
    #[test]
    fn junction_polygons_y_t_x_match_observed_union_and_have_no_hub_cracks() {
        for (angles, area) in [
            (vec![0., 120., 240.], 3.5610289),
            (vec![0., 90., 180.], 3.555),
            (vec![0., 90., 180., 270.], 4.71),
        ] {
            let count = angles.len();
            let project = junction_project(&angles, &vec![4.; count], &vec![0.3; count]);
            let polygons = junction_polygons(&project);
            assert_valid_junction(&polygons);
            assert!(
                (junction_union_area(&polygons) - area).abs() < 1e-4,
                "{angles:?}: {}",
                junction_union_area(&polygons)
            );
            for x in -4..=4 {
                for y in -4..=4 {
                    let p = Point::new(x as f64 * 0.01 + 0.00013, y as f64 * 0.01 + 0.00017);
                    assert!(
                        polygons.iter().any(|poly| contains(p, poly)),
                        "hub gap {p:?}"
                    );
                }
            }
            assert!(
                wall_polygon_outline_segments(&polygons)
                    .iter()
                    .all(|(a, b)| segment_distance(Point::default(), *a, *b) > 0.09)
            );
            let walls: Vec<_> = project.elements.iter().collect();
            let attached = plan_wall_polygon_attached_ends(&walls, &polygons, &vec![1; count]);
            assert!(walls.iter().all(|w| attached.contains(&(w.id, true))));
        }
    }
    #[test]
    fn junction_polygons_fan_keeps_neighbour_mitre_and_removes_backwards_spike() {
        let project = junction_project(&[0., 20., 150., 250.], &[4.; 4], &[0.3; 4]);
        let polys = junction_polygons(&project);
        assert_valid_junction(&polys);
        assert!(
            polys
                .iter()
                .flatten()
                .any(|p| p.distance(Point::new(0.8506923, 0.15)) < 1e-5)
        );
        assert!(!polys.iter().any(|p| contains(Point::new(-0.75, -0.1), p)));
        assert!((junction_union_area(&polys) - 4.6272697).abs() < 1e-4);
    }
    #[test]
    fn junction_polygons_moved_t_matches_adjacent_sector_boundary() {
        let mut project = junction_project(&[0.; 3], &[4.; 3], &[0.3; 3]);
        for (wall, end) in project.elements.iter_mut().zip([
            Point::new(-2.5714285, -2.3154762),
            Point::new(5.4285715, -2.3154762),
            Point::new(1.4285715, 1.6845238),
        ]) {
            if let Shape::Wall { end: e, .. } = &mut wall.shape {
                *e = end;
            }
        }
        let polys = junction_polygons(&project);
        assert_valid_junction(&polys);
        assert!((junction_union_area(&polys) - 3.4248404).abs() < 1e-4);
        assert!(
            polys
                .iter()
                .flatten()
                .any(|p| p.distance(Point::new(-0.107873, 0.104715)) < 1e-5)
        );
    }
    #[test]
    fn junction_polygons_are_order_and_reference_direction_independent() {
        let project = junction_project(&[0., 20., 150., 250.], &[4.; 4], &[0.2, 0.3, 0.4, 0.25]);
        let expected = junction_polygons(&project);
        for a in 0..4 {
            for b in 0..4 {
                for c in 0..4 {
                    for d in 0..4 {
                        let order = [a, b, c, d];
                        if (0..4).any(|i| (i + 1..4).any(|j| order[i] == order[j])) {
                            continue;
                        }
                        let walls: Vec<_> = order.iter().map(|&i| &project.elements[i]).collect();
                        let actual = plan_wall_polygons(&walls, &[1; 4]);
                        for (k, &i) in order.iter().enumerate() {
                            assert_eq!(actual[k], expected[i]);
                        }
                    }
                }
            }
        }
        let mut reversed = project.clone();
        for wall in &mut reversed.elements {
            if let Shape::Wall { start, end, .. } = &mut wall.shape {
                std::mem::swap(start, end);
            }
        }
        let actual = junction_polygons(&reversed);
        assert_valid_junction(&actual);
        assert!((junction_union_area(&actual) - junction_union_area(&expected)).abs() < 1e-5);
        for (a, b) in actual.iter().zip(&expected) {
            assert!(a.iter().all(|p| b.iter().any(|q| p.distance(*q) < 1e-5)));
        }
    }
    #[test]
    fn junction_polygons_width_reference_offsets_and_short_bevels_stay_valid() {
        use crate::wall::WallReferenceLine;
        for mode in [
            WallReferenceLine::Center,
            WallReferenceLine::Inside,
            WallReferenceLine::Outside,
        ] {
            let mut project = junction_project(&[0., 120., 240.], &[4.; 3], &[0.2, 0.4, 0.6]);
            for wall in &mut project.elements {
                wall.wall_settings.as_mut().unwrap().geometry.reference_line = mode;
            }
            assert_valid_junction(&junction_polygons(&project));
        }
        let mut project = junction_project(&[0., 120., 240.], &[4.; 3], &[0.2, 0.4, 0.6]);
        for wall in &mut project.elements {
            wall.wall_settings
                .as_mut()
                .unwrap()
                .geometry
                .reference_offset = 0.04;
        }
        let expected = junction_polygons(&project);
        assert_valid_junction(&expected);
        for wall in &mut project.elements {
            if let Shape::Wall { start, end, .. } = &mut wall.shape {
                std::mem::swap(start, end);
            }
            wall.wall_settings
                .as_mut()
                .unwrap()
                .geometry
                .reference_offset = -0.04;
        }
        assert!(
            (junction_union_area(&junction_polygons(&project)) - junction_union_area(&expected))
                .abs()
                < 1e-4
        );
        // Entirely external reference lines conservatively retain body caps.
        for wall in &mut project.elements {
            wall.wall_settings
                .as_mut()
                .unwrap()
                .geometry
                .reference_offset = 0.8;
        }
        let raw = junction_polygons(&project);
        for (poly, wall) in raw.iter().zip(&project.elements) {
            assert_eq!(*poly, body_polygon(&wall_body(wall).unwrap()).to_vec());
        }
        let short = junction_project(&[0., 1., 120., 240.], &[0.1, 0.1, 4., 4.], &[0.3; 4]);
        let polys = junction_polygons(&short);
        assert_valid_junction(&polys);
        for (poly, wall) in polys.iter().zip(&short.elements) {
            let b = wall_body(wall).unwrap();
            assert!(poly.iter().all(
                |p| dot(Point::new(p.x - b.start.x, p.y - b.start.y), b.dir) <= b.length + 1e-5
            ));
        }
    }
    #[test]
    fn junction_polygons_require_matching_story_group_and_strict_node() {
        let mut project = junction_project(&[0., 120., 240.], &[4.; 3], &[0.3; 3]);
        let walls: Vec<_> = project.elements.iter().collect();
        for groups in [[0, 0, 0], [1, 2, 3]] {
            let polys = plan_wall_polygons(&walls, &groups);
            for (p, w) in polys.iter().zip(&walls) {
                assert_eq!(*p, body_polygon(&wall_body(w).unwrap()).to_vec());
            }
        }
        for (i, w) in project.elements.iter_mut().enumerate() {
            w.story = i as u32;
        }
        let polys = junction_polygons(&project);
        for (p, w) in polys.iter().zip(&project.elements) {
            assert_eq!(*p, body_polygon(&wall_body(w).unwrap()).to_vec());
        }
        for (i, w) in project.elements.iter_mut().enumerate() {
            w.story = 0;
            if let Shape::Wall { start, .. } = &mut w.shape {
                start.x += i as f64 * 0.01;
            }
        }
        // Nearby face joins retain their established pairwise workflow.
        assert!(junction_polygons(&project).iter().all(|p| p.len() == 4));
    }
    #[test]
    fn junction_concave_outline_clips_against_actual_interior() {
        let concave = vec![
            Point::new(0., 0.),
            Point::new(3., 0.),
            Point::new(3., 1.),
            Point::new(1., 1.),
            Point::new(1., 3.),
            Point::new(0., 3.),
        ];
        let square = vec![
            Point::new(0.5, 0.5),
            Point::new(2., 0.5),
            Point::new(2., 2.),
            Point::new(0.5, 2.),
        ];
        let polys = vec![concave, square];
        assert!((junction_union_area(&polys) - 6.).abs() < 1e-4);
        assert!(
            !wall_polygon_outline_segments(&polys)
                .iter()
                .any(|(a, b)| point_on_segment(Point::new(0.75, 0.5), *a, *b, 1e-5))
        );
    }
    #[test]
    fn junction_polygon_pipeline_preserves_ordinary_l_t_and_shifted_faces() {
        use crate::wall::WallReferenceLine;
        for (mode, offset) in [
            (WallReferenceLine::Center, 0.),
            (WallReferenceLine::Inside, 0.),
            (WallReferenceLine::Outside, 0.),
            (WallReferenceLine::Center, 0.2),
        ] {
            for fixture in 0..4 {
                let mut project = junction_project(&[0., 90.], &[4.; 2], &[0.3, 0.2]);
                if let Shape::Wall { start, end, .. } = &mut project.elements[0].shape {
                    if fixture == 1 {
                        *start = Point::new(-4., 0.);
                        *end = Point::new(4., 0.);
                    }
                    if fixture == 2 {
                        *start = Point::new(-4., 0.);
                        *end = Point::default();
                    }
                }
                if let Shape::Wall { start, end, .. } = &mut project.elements[1].shape {
                    if fixture == 2 {
                        start.y = 0.15;
                        end.y = 4.15;
                    }
                    if fixture == 3 {
                        *end = Point::new(3., 2.);
                    }
                }
                for wall in &mut project.elements {
                    let g = &mut wall.wall_settings.as_mut().unwrap().geometry;
                    g.reference_line = mode;
                    g.reference_offset = offset;
                }
                let walls: Vec<_> = project.elements.iter().collect();
                let old = plan_wall_footprints(&walls, &[1, 1]);
                let new = plan_wall_polygons(&walls, &[1, 1]);
                for (a, b) in old.iter().zip(&new) {
                    assert_eq!(
                        a.as_slice(),
                        b.as_slice(),
                        "fixture {fixture}, offset {offset}"
                    );
                }
            }
        }
    }
    #[test]
    fn junction_polygons_support_both_ends_without_crossing_caps() {
        for length in [0.1, 1.] {
            let mut project = junction_project(&[0., 120., 240.], &[length, 4., 4.], &[0.3; 3]);
            for angle in [60_f64, 300.] {
                let r = angle.to_radians();
                project.add(
                    0,
                    Material::Concrete,
                    Shape::Wall {
                        start: Point::new(length, 0.),
                        end: Point::new(length + 4. * r.cos(), 4. * r.sin()),
                        thickness: 0.3,
                        height: 3.,
                    },
                );
            }
            let polygons = junction_polygons(&project);
            assert_valid_junction(&polygons);
            assert!(polygons[0].len() >= 6);
            assert!(
                polygons[0]
                    .iter()
                    .all(|p| p.x >= -1e-5 && p.x <= length + 1e-5)
            );
            for x in 0..100 {
                assert!(
                    polygons.iter().any(|p| contains(
                        Point::new(x as f64 * length / 100. + 0.0001, 0.0002),
                        p
                    ))
                );
            }
            let angle = 42.3_f64.to_radians();
            let rotate = |p: Point| {
                Point::new(
                    p.x * angle.cos() - p.y * angle.sin(),
                    p.x * angle.sin() + p.y * angle.cos(),
                )
            };
            for wall in &mut project.elements {
                if let Shape::Wall { start, end, .. } = &mut wall.shape {
                    *start = rotate(*start);
                    *end = rotate(*end);
                }
            }
            let rotated = junction_polygons(&project);
            assert_valid_junction(&rotated);
            for (a, b) in polygons.iter().zip(&rotated) {
                assert_eq!(a.len(), b.len());
                assert!(
                    a.iter()
                        .all(|p| b.iter().any(|q| rotate(*p).distance(*q) < 1e-5)),
                    "{a:?} vs {b:?}"
                );
            }
        }
    }
}

#[cfg(test)]
mod slab_perimeter_regressions {
    use super::*;
    use crate::slab_geometry::{self, SlabChange};

    fn rectangle() -> (Project, u64) {
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
        (p, id)
    }
    #[test]
    fn slab_arc_samples_are_edges_not_editable_nodes_and_midpoint_is_exact() {
        let (mut p, _) = rectangle();
        let c = slab_geometry::changed(
            &p.elements[0],
            0,
            &SlabChange::Curve,
            Point::new(2., 0.),
            Point::new(2., -1.),
        )
        .unwrap()
        .remove(0);
        slab_geometry::sync(&mut p.elements[0], c).unwrap();
        let sample = slab_geometry::outline(&p.elements[0])[4];
        assert_eq!(
            plan_hover_kind(&p, 0, sample, 0.0001, 0.0001),
            Some(PlanHoverKind::Edge)
        );
        let box_at_sample = rectangle_polygon(
            Point::new(sample.x - 0.0001, sample.y - 0.0001),
            Point::new(sample.x + 0.0001, sample.y + 0.0001),
        );
        assert!(!element_has_node_in_polygon(
            &p,
            &p.elements[0],
            &box_at_sample
        ));
        let (mid, kind) = snap(&p, 0, Point::new(2., -1.0001), 0.001, true);
        assert_eq!(kind, SnapKind::Midpoint);
        assert!(mid.distance(Point::new(2., -1.)) < 1e-5);
        let (_, sample_kind) = snap(&p, 0, sample, 0.0001, true);
        assert_ne!(sample_kind, SnapKind::Endpoint);
    }
    #[test]
    fn slab_hole_is_empty_for_body_marquee_and_3d_pick_and_elevation_is_shared() {
        let (mut p, id) = rectangle();
        let cutter = vec![
            Point::new(1., 1.),
            Point::new(3., 1.),
            Point::new(3., 2.),
            Point::new(1., 2.),
        ];
        let mut c = slab_geometry::changed(
            &p.elements[0],
            0,
            &SlabChange::SubtractPolygon(cutter),
            Point::default(),
            Point::default(),
        )
        .unwrap()
        .remove(0);
        c.elevation = 2.5;
        slab_geometry::sync(&mut p.elements[0], c).unwrap();
        let inside_hole = Point::new(2., 1.5);
        assert_eq!(hit_plan(&p, 0, inside_hole, 0.01), None);
        assert_eq!(plan_hover_kind(&p, 0, inside_hole, 0.01, 0.01), None);
        let hole_box = rectangle_polygon(Point::new(1.5, 1.25), Point::new(2.5, 1.75));
        assert!(!element_intersects_polygon(&p, &p.elements[0], &hole_box));
        assert_eq!(hit_plan(&p, 0, Point::new(0.5, 1.5), 0.01), Some(id));
        assert_eq!(
            plan_hover_kind(&p, 0, Point::new(1., 1.5), 0.01, 0.01),
            Some(PlanHoverKind::Edge)
        );
        let corners = corners(&p, &p.elements[0]);
        assert!((corners[0].z - 2.2).abs() < 1e-6);
        assert!((corners[4].z - 2.5).abs() < 1e-6);
        let camera = Camera {
            yaw: 0.,
            pitch: std::f64::consts::FRAC_PI_2 - 0.001,
            span: 10.,
            target: DVec3::new(2., 1.5, 2.5),
        };
        let projected = camera.matrix(1.).project_point3(DVec3::new(2., 1.5, 2.5));
        assert_eq!(
            camera.pick(&p, [projected.x, projected.y], 1., Some(0)),
            None
        );
    }
}

#[cfg(test)]
mod outline_index_regressions {
    use super::*;

    #[test]
    fn indexed_outline_paths_match_scan_order_at_junctions_and_tolerance_boundaries() {
        let p = Point::new;
        for seed in 0..24_u64 {
            let mut state = seed + 1;
            let mut next = || {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                state
            };
            let mut segments = Vec::new();
            for i in 0..120 {
                let x = (i % 12) as f64 - 6.;
                let y = (i / 12) as f64 - 5.;
                let a = p(x, y);
                let b = match next() % 4 {
                    0 => p(x + 1., y),
                    1 => p(x, y + 1.),
                    2 => p(x + 1., y + 1.),
                    _ => a,
                };
                segments.push(if next() % 2 == 0 { (a, b) } else { (b, a) });
            }
            for delta in [0., 0.0009999, 0.001, 0.0010001, -0.0009999] {
                segments.extend([
                    (p(-0.01, 0.), p(0.01, 0.)),
                    (p(0.01 + delta, 0.), p(0.02, 1.)),
                    (p(-0.01, 0.), p(-0.01, 1.)),
                ]);
            }
            for i in (1..segments.len()).rev() {
                let j = next() as usize % (i + 1);
                segments.swap(i, j);
            }
            assert_eq!(
                wall_outline_paths_from_segments(segments.clone()),
                wall_outline_paths_scan(segments),
                "seed={seed}"
            );
        }
    }

    #[test]
    fn indexed_outline_paths_preserve_disconnected_closed_loops_and_large_offsets() {
        for shift in [-99900., -100., 0., 99900.] {
            let mut segments = Vec::new();
            for i in 0..40 {
                let x = shift + (i % 10) as f64 * 2.;
                let y = (i / 10) as f64 * 2.;
                let q = [
                    Point::new(x, y),
                    Point::new(x + 1., y),
                    Point::new(x + 1., y + 1.),
                    Point::new(x, y + 1.),
                ];
                for j in 0..4 {
                    segments.push((q[j], q[(j + 1) % 4]));
                }
            }
            assert_eq!(
                wall_outline_paths_from_segments(segments.clone()),
                wall_outline_paths_scan(segments)
            );
        }
        let mut index = OutlineEndpointIndex::default();
        for i in 0..10000 {
            let x = i as f64;
            index.insert(i, (Point::new(x, 0.), Point::new(x, 1.)));
        }
        assert_eq!(
            index
                .candidates(Point::new(5000., 0.), Point::new(5000., 1.))
                .into_iter()
                .collect::<Vec<_>>(),
            vec![5000]
        );
    }
}

#[cfg(test)]
mod prepared_clipping_regressions {
    use super::*;

    #[test]
    fn prepared_clipping_matches_original_intervals_exactly() {
        let p = Point::new;
        for shift in [-99000., -0.001, 0., 99000.] {
            for scale in [0., 0.000001, 0.001, 0.2, 10.] {
                let base = [p(shift, 0.), p(shift + scale, 0.), p(shift, scale)];
                for triangle in [base, [base[2], base[1], base[0]]] {
                    let winding = if polygon_area(&triangle) >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    };
                    for i in -8..=8 {
                        for j in -8..=8 {
                            for delta in [-0.00001001, -0.00001, 0., 0.00001, 0.00001001] {
                                let a = p(shift + i as f64 * scale / 4., delta);
                                let b = p(shift + j as f64 * scale / 4., scale - delta);
                                assert_eq!(
                                    clip_segment_with_winding(a, b, &triangle, winding),
                                    segment_inside_convex_polygon(a, b, &triangle),
                                    "shift={shift}, scale={scale}, i={i}, j={j}, delta={delta}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod wall_join_index_regressions {
    use super::*;

    #[test]
    fn indexed_wall_join_corners_match_full_scan_exactly() {
        for shift in [-99000., -100., 0., 99000.] {
            let mut project = Project::default();
            for i in 0..160 {
                let x = shift + (i % 16) as f64 * 0.5;
                let y = (i / 16) as f64 * 0.5;
                let (mut a, mut b) = match i % 5 {
                    0 => (Point::new(x, y), Point::new(x + 4., y)),
                    1 => (Point::new(x, y), Point::new(x, y + 4.)),
                    2 => (Point::new(x, y), Point::new(x + 1., y + 2.)),
                    3 => (Point::new(x - 20., y), Point::new(x + 20., y + 0.001)),
                    _ => (Point::new(x, y), Point::new(x + 0.001, y + 0.001)),
                };
                if i % 3 == 0 {
                    std::mem::swap(&mut a, &mut b);
                }
                project.add(
                    (i % 3) as u32,
                    Material::Plaster,
                    Shape::Wall {
                        start: a,
                        end: b,
                        thickness: [0.001, 0.2, 1., 4.][i % 4],
                        height: 3.,
                    },
                );
            }
            let bodies: Vec<_> = project.elements.iter().map(wall_body).collect();
            let index = WallJoinIndex::new(&bodies).unwrap();
            let stories: Vec<_> = project.elements.iter().map(|e| e.story).collect();
            for groups in [
                vec![1; bodies.len()],
                (0..bodies.len()).map(|i| (i % 3) as u8).collect(),
            ] {
                for (i, body) in bodies.iter().enumerate() {
                    let Some(body) = body else {
                        continue;
                    };
                    for end in [true, false] {
                        assert_eq!(
                            joined_end_corners(
                                &bodies,
                                &groups,
                                &stories,
                                i,
                                end,
                                index.candidates(body, end).into_iter(),
                                None
                            ),
                            joined_end_corners(
                                &bodies,
                                &groups,
                                &stories,
                                i,
                                end,
                                0..bodies.len(),
                                None
                            ),
                            "shift={shift} index={i} start={end}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn wall_join_index_limits_sparse_candidates_and_falls_back_outside_guard() {
        let mut project = Project::default();
        for i in 0..10000 {
            let x = (i % 100) as f64 * 10.;
            let y = (i / 100) as f64 * 10.;
            project.add(
                0,
                Material::Plaster,
                Shape::Wall {
                    start: Point::new(x, y),
                    end: Point::new(x + 1., y),
                    thickness: 0.2,
                    height: 3.,
                },
            );
        }
        let mut bodies: Vec<_> = project.elements.iter().map(wall_body).collect();
        let index = WallJoinIndex::new(&bodies).unwrap();
        for i in [0, 5030, 9999] {
            let slots = index.candidates(bodies[i].as_ref().unwrap(), true);
            assert!(slots.contains(&i));
            assert!(slots.len() <= 32, "{} candidates", slots.len());
        }
        bodies[0].as_mut().unwrap().start.x = 100001.;
        assert!(WallJoinIndex::new(&bodies).is_none());
        bodies[0].as_mut().unwrap().start.x = f64::NAN;
        assert!(WallJoinIndex::new(&bodies).is_none());
    }
}

#[cfg(test)]
mod clip_index_regressions {
    use super::*;

    #[test]
    fn indexed_coverage_matches_original_at_boundaries_and_concave_overlaps() {
        for shift in [-99000., -1., 0., 99000.] {
            let mut polygons = Vec::new();
            for i in 0..60 {
                let x = shift + (i % 10) as f64 * 0.37;
                let y = (i / 10) as f64 * 0.23;
                let width = [0.00001, 0.001, 0.2, 1.5][i % 4];
                let mut polygon = match i % 3 {
                    0 => vec![
                        Point::new(x, y),
                        Point::new(x + 1., y),
                        Point::new(x + 1., y + width),
                        Point::new(x, y + width),
                    ],
                    1 => vec![
                        Point::new(x, y),
                        Point::new(x + 1., y + 0.7),
                        Point::new(x + 1. - width, y + 0.7 + width),
                        Point::new(x - width, y + width),
                    ],
                    _ => vec![
                        Point::new(x, y),
                        Point::new(x + 1., y),
                        Point::new(x + 1., y + width),
                        Point::new(x + width, y + width),
                        Point::new(x + width, y + 1.),
                        Point::new(x, y + 1.),
                    ],
                };
                if i % 2 == 0 {
                    polygon.reverse();
                }
                polygons.push(polygon);
            }
            polygons.push(polygons[2].clone());
            assert_eq!(
                wall_outline_segments_by_wall_for_polygons(&polygons),
                wall_outline_segments_by_wall_scan(&polygons),
                "shift={shift}"
            );
        }
    }

    #[test]
    fn interval_index_retains_every_original_clip_and_prunes_sparse_sets() {
        let mut triangles = Vec::new();
        for i in 0..120 {
            let x = (i % 12) as f64 * 2. - 10.;
            let y = (i / 12) as f64 * 2. - 10.;
            let w = [0., 0.000001, 0.001, 0.2, 1.][i % 5];
            let t = [
                Point::new(x, y),
                Point::new(x + 1., y + w),
                Point::new(x + w, y + 1.),
            ];
            triangles.push(t);
            triangles.push([t[2], t[1], t[0]]);
        }
        let flat: Vec<_> = triangles
            .iter()
            .enumerate()
            .map(|(i, t)| (i, t, if polygon_area(t) >= 0. { 1. } else { -1. }))
            .collect();
        let index = ClipCandidateIndex::new(&flat).unwrap();
        for i in -25..25 {
            for delta in [-0.00001001, -0.00001, 0., 0.00001, 0.00001001] {
                let a = Point::new(i as f64 * 0.5, delta);
                let b = Point::new(i as f64 * 0.5 + 0.9, 1. + delta);
                let candidates = index.candidates(a, b);
                for (slot, (_, t, w)) in flat.iter().enumerate() {
                    if clip_segment_with_winding(a, b, *t, *w).is_some() {
                        assert!(
                            candidates.binary_search(&slot).is_ok(),
                            "lost slot={slot} i={i} delta={delta}"
                        );
                    }
                }
            }
        }
        // Degenerate triangles can accept a collinear segment far beyond
        // their vertices under the original half-plane clipping predicate.
        let a = Point::new(-100., -100.);
        let b = Point::new(-99., -99.);
        let candidates = index.candidates(a, b);
        for (slot, (_, triangle, winding)) in flat.iter().enumerate() {
            if clip_segment_with_winding(a, b, *triangle, *winding).is_some() {
                assert!(candidates.binary_search(&slot).is_ok());
            }
        }
        let sparse: Vec<_> = (0..10000)
            .map(|i| {
                let x = (i % 100) as f64 * 10.;
                let y = (i / 100) as f64 * 10.;
                [
                    Point::new(x, y),
                    Point::new(x + 1., y),
                    Point::new(x, y + 1.),
                ]
            })
            .collect();
        let flat: Vec<_> = sparse.iter().enumerate().map(|(i, t)| (i, t, 1.)).collect();
        let index = ClipCandidateIndex::new(&flat).unwrap();
        assert!(
            index
                .candidates(Point::new(500., 500.), Point::new(501., 500.))
                .len()
                <= 32
        );
        assert_eq!(
            index
                .candidates(Point::new(100001., 0.), Point::new(100002., 0.))
                .len(),
            flat.len()
        );
    }
}

#[cfg(test)]
mod camera_framing_tests {
    use super::*;

    fn box_points(lo: DVec3, hi: DVec3) -> Vec<DVec3> {
        (0..8)
            .map(|i| {
                DVec3::new(
                    if i & 1 == 0 { lo.x } else { hi.x },
                    if i & 2 == 0 { lo.y } else { hi.y },
                    if i & 4 == 0 { lo.z } else { hi.z },
                )
            })
            .collect()
    }

    #[test]
    fn camera_fit_points_fills_actual_aspect_and_preserves_direction() {
        let shapes = [
            (DVec3::ZERO, DVec3::new(100., 74., 20.)),
            (DVec3::new(-0.1, -0.1, 0.), DVec3::new(0.1, 0.1, 95.)),
            (DVec3::new(-25., -2., 0.), DVec3::new(25., 2., 0.)),
        ];
        for (lo, hi) in shapes {
            for offset in [DVec3::ZERO, DVec3::new(54530., 50382., 1200.)] {
                let points = box_points(lo + offset, hi + offset);
                for aspect in [0.25, 0.5, 1., 2., 4.] {
                    for yaw in [-1.05, 0., 2.] {
                        for pitch in [-1.3, 0., 0.83, 1.3] {
                            let mut camera = Camera {
                                yaw,
                                pitch,
                                ..Camera::default()
                            };
                            assert!(camera.fit_points(&points, aspect));
                            assert_eq!(camera.yaw, yaw);
                            assert_eq!(camera.pitch, pitch);
                            let mut extent = 0_f64;
                            let matrix = camera.matrix(aspect);
                            for point in &points {
                                let ndc = matrix.project_point3(*point);
                                assert!(ndc.is_finite());
                                assert!(
                                    ndc.x.abs() <= 0.840000001
                                        && ndc.y.abs() <= 0.840000001
                                        && ndc.z > 0.
                                        && ndc.z < 1.,
                                    "{ndc:?}; aspect={aspect}, yaw={yaw}, pitch={pitch}"
                                );
                                extent = extent.max(ndc.x.abs()).max(ndc.y.abs());
                            }
                            assert!(
                                (extent - 0.84).abs() < 1e-8,
                                "fit should fill one viewport dimension: {extent}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn camera_fit_points_is_translation_invariant_and_keeps_degenerate_points_visible() {
        let points = box_points(DVec3::ZERO, DVec3::new(120., 3., 5.));
        let shift = DVec3::new(1e8, -1e8, 500.);
        let shifted: Vec<_> = points.iter().map(|p| *p + shift).collect();
        let mut local = Camera::default();
        let mut survey = local.clone();
        assert!(local.fit_points(&points, 2.));
        assert!(survey.fit_points(&shifted, 2.));
        assert!((local.span - survey.span).abs() < 1e-6);
        assert!((local.target + shift - survey.target).length() < 1e-6);

        for points in [
            vec![shift],
            vec![shift, shift],
            vec![shift, shift + DVec3::new(0., 0., 0.001)],
            vec![shift, DVec3::splat(f64::NAN)],
        ] {
            let mut camera = Camera::default();
            assert!(camera.fit_points(&points, 0.5));
            assert!(camera.target.is_finite() && camera.span.is_finite());
            for point in points.into_iter().filter(|p| p.is_finite()) {
                let ndc = camera.matrix(0.5).project_point3(point);
                assert!(ndc.is_finite() && ndc.z > 0. && ndc.z < 1.);
            }
        }
    }

    #[test]
    fn camera_fit_points_rejects_invalid_input_without_changing_camera() {
        let mut camera = Camera::default();
        let before = camera.clone();
        for (points, aspect) in [
            (vec![], 1.),
            (vec![DVec3::splat(f64::NAN)], 1.),
            (vec![DVec3::ZERO], 0.),
            (vec![DVec3::ZERO], -1.),
            (vec![DVec3::ZERO], f64::NAN),
            (vec![DVec3::ZERO], f64::INFINITY),
        ] {
            assert!(!camera.fit_points(&points, aspect));
            assert_eq!(camera.target, before.target);
            assert_eq!(camera.span, before.span);
            assert_eq!(camera.yaw, before.yaw);
            assert_eq!(camera.pitch, before.pitch);
        }
    }
}

#[cfg(test)]
mod hub_window_regressions {
    use super::*;
    #[test]
    fn hub_window_preserves_full_scan_polygons_and_assignment_order() {
        for shift in [-99000., -0.01, 0., 99000.] {
            let mut project = Project::default();
            for i in 0..120 {
                let x = shift + (i / 6) as f64 * 3.;
                let y = (i / 30) as f64 * 3.;
                let angle = (i % 6) as f64 * std::f64::consts::PI / 3.;
                let delta = [0., 0.0009999, 0.0010001][i % 3];
                let mut a = Point::new(x + delta, y);
                let mut b = Point::new(x + angle.cos() * 2., y + angle.sin() * 2.);
                if i % 2 == 0 {
                    std::mem::swap(&mut a, &mut b);
                }
                project.add(
                    (i % 2) as u32,
                    Material::Plaster,
                    Shape::Wall {
                        start: a,
                        end: b,
                        thickness: 0.2,
                        height: 3.,
                    },
                );
            }
            let walls: Vec<_> = project.elements.iter().collect();
            for groups in [
                vec![1; walls.len()],
                (0..walls.len()).map(|i| (i % 3) as u8).collect(),
            ] {
                assert_eq!(
                    plan_wall_polygons_impl(&walls, &groups, true),
                    plan_wall_polygons_impl(&walls, &groups, false),
                    "shift={shift}"
                );
            }
        }
    }
}
