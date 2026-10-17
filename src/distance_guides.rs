//! Pure, current-story wall distance queries and validated distance edits.
use crate::model::{Point, Project, Shape};

const EPS: f32 = 1e-4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DistanceMeasurement {
    pub source: Point,
    pub target: Point,
    pub context_id: u64,
    pub reference_start: Point,
    pub reference_end: Point,
    pub distance: f32,
    /// Sign relative to the reference segment's left-hand normal.
    pub signed_distance: f32,
    pub edges: bool,
}

fn add(a: Point, b: Point) -> Point {
    Point::new(a.x + b.x, a.y + b.y)
}
fn sub(a: Point, b: Point) -> Point {
    Point::new(a.x - b.x, a.y - b.y)
}
fn scale(a: Point, amount: f32) -> Point {
    Point::new(a.x * amount, a.y * amount)
}
fn dot(a: Point, b: Point) -> f32 {
    a.x * b.x + a.y * b.y
}
fn midpoint(a: Point, b: Point) -> Point {
    scale(add(a, b), 0.5)
}

/// Return at most one nearest perpendicular measurement per cardinal bucket.
/// `exclude_id` also selects a wall whose facing axis/edge replaces the point
/// source. Without a selected wall, `source` is the point being placed.
pub fn measurements(
    project: &Project,
    story: u32,
    source: Point,
    exclude_id: Option<u64>,
    edges: bool,
) -> Vec<DistanceMeasurement> {
    if !source.finite() {
        return vec![];
    }
    let selected = exclude_id.and_then(|id| {
        project.elements.iter().find_map(|element| {
            if element.id == id && element.story == story {
                if let Shape::Wall {
                    start,
                    end,
                    thickness,
                    ..
                } = element.shape
                {
                    return Some((start, end, thickness));
                }
            }
            None
        })
    });
    let origin = selected.map_or(source, |(a, b, _)| midpoint(a, b));
    let mut nearest: [Option<DistanceMeasurement>; 4] = [None; 4];
    for element in &project.elements {
        if element.story != story || Some(element.id) == exclude_id {
            continue;
        }
        let Shape::Wall {
            start,
            end,
            thickness,
            ..
        } = element.shape
        else {
            continue;
        };
        let length = start.distance(end);
        if !length.is_finite() || length < 0.01 {
            continue;
        }
        let tangent = scale(sub(end, start), length.recip());
        let normal = Point::new(-tangent.y, tangent.x);
        let origin_signed = dot(sub(origin, start), normal);
        if origin_signed.abs() < EPS {
            continue;
        }
        let facing = scale(normal, -origin_signed.signum());
        let mut measurement_source = origin;
        if let Some((a, b, selected_thickness)) = selected {
            let selected_length = a.distance(b);
            if selected_length < 0.01 || !selected_length.is_finite() {
                continue;
            }
            let selected_direction = scale(sub(b, a), selected_length.recip());
            let axial = dot(selected_direction, facing);
            // Parallel walls measure from the center; otherwise use the facing
            // endpoint so a perpendicular context measures the end clearance.
            if axial.abs() > EPS {
                measurement_source = if axial > 0. { b } else { a };
            }
            if edges {
                let side = Point::new(-selected_direction.y, selected_direction.x);
                let side_alignment = dot(side, facing);
                if side_alignment.abs() > EPS {
                    measurement_source = add(
                        measurement_source,
                        scale(side, selected_thickness * 0.5 * side_alignment.signum()),
                    );
                }
            }
        }
        let edge_offset = if edges {
            thickness * 0.5 * origin_signed.signum()
        } else {
            0.
        };
        let reference_start = add(start, scale(normal, edge_offset));
        let reference_end = add(end, scale(normal, edge_offset));
        let relative = sub(measurement_source, reference_start);
        let along = dot(relative, tangent);
        let signed_distance = dot(relative, normal);
        if along < -EPS || along > length + EPS || signed_distance * origin_signed.signum() <= EPS {
            continue;
        }
        let target = add(reference_start, scale(tangent, along.clamp(0., length)));
        let distance = measurement_source.distance(target);
        if !distance.is_finite() || distance <= EPS {
            continue;
        }
        let delta = sub(target, measurement_source);
        let bucket = if delta.x.abs() >= delta.y.abs() {
            usize::from(delta.x >= 0.)
        } else {
            2 + usize::from(delta.y >= 0.)
        };
        let measurement = DistanceMeasurement {
            source: measurement_source,
            target,
            context_id: element.id,
            reference_start,
            reference_end,
            distance,
            signed_distance,
            edges,
        };
        if nearest[bucket].is_none_or(|old| distance < old.distance) {
            nearest[bucket] = Some(measurement);
        }
    }
    nearest.into_iter().flatten().collect()
}

/// Position a point at a positive distance on its current side of a reference.
pub fn target_at_distance(measurement: &DistanceMeasurement, value: f32) -> Result<Point, String> {
    if !value.is_finite() || value <= 0. || value >= 100_000. {
        return Err("Distance must be finite, positive and below 100000 m".into());
    }
    let distance = measurement.source.distance(measurement.target);
    if !measurement.source.finite()
        || !measurement.target.finite()
        || !distance.is_finite()
        || distance <= EPS
    {
        return Err("Distance reference is invalid".into());
    }
    let reference_length = measurement
        .reference_start
        .distance(measurement.reference_end);
    if !measurement.reference_start.finite()
        || !measurement.reference_end.finite()
        || !reference_length.is_finite()
        || reference_length < 0.01
        || !measurement.distance.is_finite()
        || !measurement.signed_distance.is_finite()
    {
        return Err("Distance reference is invalid".into());
    }
    let reference_direction = scale(
        sub(measurement.reference_end, measurement.reference_start),
        reference_length.recip(),
    );
    let normal = Point::new(-reference_direction.y, reference_direction.x);
    let along = dot(
        sub(measurement.target, measurement.reference_start),
        reference_direction,
    );
    let tolerance = EPS * distance.max(1.);
    if (measurement.distance - distance).abs() > tolerance
        || (dot(sub(measurement.source, measurement.target), normal) - measurement.signed_distance)
            .abs()
            > tolerance
        || dot(sub(measurement.target, measurement.reference_start), normal).abs() > tolerance
        || dot(
            sub(measurement.source, measurement.target),
            reference_direction,
        )
        .abs()
            > tolerance
        || along < -EPS
        || along > reference_length + EPS
    {
        return Err("Distance reference is inconsistent".into());
    }
    let result = add(
        measurement.target,
        scale(
            sub(measurement.source, measurement.target),
            value / distance,
        ),
    );
    if !result.finite() || result.x.abs() >= 100_000. || result.y.abs() >= 100_000. {
        return Err("Distance places the point outside project limits".into());
    }
    Ok(result)
}

/// Return a validated edited project. Callers commit it through Editor history.
/// An axial edit stretches the facing endpoint; other edits translate the wall.
pub fn apply_distance(
    project: &Project,
    measurement: &DistanceMeasurement,
    selected_id: u64,
    value: f32,
) -> Result<Project, String> {
    let selected = project
        .elements
        .iter()
        .find(|element| element.id == selected_id)
        .ok_or("Selected wall no longer exists")?;
    let Shape::Wall { start, end, .. } = selected.shape else {
        return Err("Distance editing requires one selected wall".into());
    };
    let current = measurements(
        project,
        selected.story,
        midpoint(start, end),
        Some(selected_id),
        measurement.edges,
    );
    if !current.iter().any(|candidate| {
        candidate.context_id == measurement.context_id
            && candidate.source.distance(measurement.source) < EPS
            && candidate.target.distance(measurement.target) < EPS
            && candidate
                .reference_start
                .distance(measurement.reference_start)
                < EPS
            && candidate.reference_end.distance(measurement.reference_end) < EPS
    }) {
        return Err("Distance reference changed; select a current guide".into());
    }
    let new_source = target_at_distance(measurement, value)?;
    let delta = sub(new_source, measurement.source);
    let direction = scale(sub(end, start), start.distance(end).recip());
    let measurement_direction = scale(
        sub(measurement.target, measurement.source),
        measurement.distance.recip(),
    );
    let mut result = project.clone();
    let element = result
        .elements
        .iter_mut()
        .find(|element| element.id == selected_id)
        .unwrap();
    if dot(direction, measurement_direction).abs() > 0.9999 {
        let Shape::Wall {
            start: new_start,
            end: new_end,
            ..
        } = &mut element.shape
        else {
            unreachable!()
        };
        if dot(direction, measurement_direction) > 0. {
            *new_end = add(*new_end, delta);
        } else {
            *new_start = add(*new_start, delta);
        }
        if dot(sub(*new_end, *new_start), direction) < 0.01 {
            return Err("Distance would collapse or reverse the selected wall".into());
        }
    } else {
        element.translate(delta);
    }
    result.validate()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Material;

    fn wall(project: &mut Project, a: (f32, f32), b: (f32, f32)) -> u64 {
        project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(a.0, a.1),
                end: Point::new(b.0, b.1),
                thickness: 0.2,
                height: 3.,
            },
        )
    }
    fn near(a: f32, b: f32) {
        assert!((a - b).abs() < EPS, "{a} != {b}");
    }

    #[test]
    fn room_point_distances_and_physical_edges() {
        let mut p = Project::default();
        wall(&mut p, (0., 0.), (8., 0.));
        wall(&mut p, (8., 0.), (8., 6.));
        wall(&mut p, (8., 6.), (0., 6.));
        wall(&mut p, (0., 6.), (0., 0.));
        let axes = measurements(&p, 0, Point::new(4., 3.), None, false);
        let edges = measurements(&p, 0, Point::new(4., 3.), None, true);
        assert_eq!(axes.len(), 4);
        for (axis, edge) in axes.iter().zip(edges) {
            near(axis.distance - edge.distance, 0.1);
        }
        assert!(measurements(&p, 1, Point::new(4., 3.), None, false).is_empty());
        assert!(measurements(&p, 0, Point::new(20., 20.), None, false).is_empty());
    }

    #[test]
    fn selected_parallel_wall_moves_and_roundtrips() {
        let mut p = Project::default();
        wall(&mut p, (0., 0.), (8., 0.));
        let id = wall(&mut p, (2., 2.), (6., 2.));
        let m = measurements(&p, 0, Point::new(4., 2.), Some(id), true)[0];
        near(m.distance, 1.8);
        let changed = apply_distance(&p, &m, id, 2.8).unwrap();
        let Shape::Wall { start, end, .. } = changed.elements[1].shape else {
            panic!()
        };
        near(start.y, 3.);
        near(end.y, 3.);
        assert_eq!(
            serde_json::from_str::<Project>(&serde_json::to_string(&changed).unwrap()).unwrap(),
            changed
        );
        assert!(apply_distance(&changed, &m, id, 2.).is_err());
        near(m.distance, 1.8); // original remains unchanged
    }

    #[test]
    fn axial_edit_stretches_facing_endpoint_and_rejects_reversal() {
        let mut p = Project::default();
        wall(&mut p, (8., -3.), (8., 3.));
        let id = wall(&mut p, (0., 0.), (6., 0.));
        let m = measurements(&p, 0, Point::new(3., 0.), Some(id), false)[0];
        near(m.distance, 2.);
        let changed = apply_distance(&p, &m, id, 3.).unwrap();
        let Shape::Wall { start, end, .. } = changed.elements[1].shape else {
            panic!()
        };
        assert_eq!(start, Point::new(0., 0.));
        assert_eq!(end, Point::new(5., 0.));
        assert!(apply_distance(&p, &m, id, 8.).is_err());
        assert!(apply_distance(&p, &m, id, 9.).is_err());
    }

    #[test]
    fn rotated_projection_and_numeric_validation() {
        let mut p = Project::default();
        wall(&mut p, (0., 0.), (4., 4.));
        let m = measurements(&p, 0, Point::new(1., 3.), None, false)[0];
        near(m.target.x, 2.);
        near(m.target.y, 2.);
        let point = target_at_distance(&m, 2_f32.sqrt() * 2.).unwrap();
        near(point.x, 0.);
        near(point.y, 4.);
        for value in [f32::NAN, f32::INFINITY, -1., 0., 100_000.] {
            assert!(target_at_distance(&m, value).is_err());
        }
        assert!(measurements(&p, 0, Point::new(f32::NAN, 0.), None, false).is_empty());
    }

    #[test]
    fn nearest_reference_wins_and_overlaps_are_excluded() {
        let mut p = Project::default();
        wall(&mut p, (0., 0.), (5., 0.));
        let near_id = wall(&mut p, (0., 1.), (5., 1.));
        let found = measurements(&p, 0, Point::new(2., 3.), None, false);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].context_id, near_id);
        let overlapping = measurements(&p, 0, Point::new(2., 1.05), None, true);
        assert!(overlapping.iter().all(|m| m.context_id != near_id));
    }

    #[test]
    fn rotated_parallel_edit_moves_both_endpoints_perpendicular() {
        let mut p = Project::default();
        wall(&mut p, (0., 0.), (10., 10.));
        let id = wall(&mut p, (1., 3.), (5., 7.));
        let m = measurements(&p, 0, Point::new(3., 5.), Some(id), false)[0];
        let changed = apply_distance(&p, &m, id, m.distance * 2.).unwrap();
        let Shape::Wall { start, end, .. } = changed.elements[1].shape else {
            panic!()
        };
        near(start.x, 0.);
        near(start.y, 4.);
        near(end.x, 4.);
        near(end.y, 8.);
        assert_eq!(p.elements[0], changed.elements[0]);
    }

    #[test]
    fn axial_edit_preserves_opposite_endpoint_and_connected_wall() {
        let mut p = Project::default();
        wall(&mut p, (8., -3.), (8., 3.));
        let id = wall(&mut p, (0., 0.), (6., 0.));
        let connected = wall(&mut p, (0., 0.), (0., 4.));
        let m = measurements(&p, 0, Point::new(3., 0.), Some(id), false)[0];
        let changed = apply_distance(&p, &m, id, 1.).unwrap();
        let Shape::Wall { start, end, .. } = changed.elements[1].shape else {
            panic!()
        };
        assert_eq!(start, Point::new(0., 0.));
        assert_eq!(end, Point::new(7., 0.));
        assert_eq!(
            p.elements.iter().find(|e| e.id == connected),
            changed.elements.iter().find(|e| e.id == connected),
        );
    }

    #[test]
    fn tampered_and_removed_references_are_rejected() {
        let mut p = Project::default();
        wall(&mut p, (0., 0.), (8., 0.));
        let id = wall(&mut p, (2., 2.), (6., 2.));
        let m = measurements(&p, 0, Point::new(4., 2.), Some(id), false)[0];
        let mut invalid = m;
        invalid.distance = f32::NAN;
        assert!(target_at_distance(&invalid, 3.).is_err());
        assert!(apply_distance(&p, &invalid, id, 3.).is_err());
        invalid = m;
        invalid.signed_distance *= -1.;
        assert!(target_at_distance(&invalid, 3.).is_err());
        invalid = m;
        invalid.reference_end = invalid.reference_start;
        assert!(target_at_distance(&invalid, 3.).is_err());
        p.elements.remove(0);
        assert!(apply_distance(&p, &m, id, 3.).is_err());
    }
}
