//! Typed polyline geometry adapter for the shared editing session.
use crate::{geometry, model::*};
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PolylineChange {
    InsertNode,
    MoveNode,
    OffsetPolyline,
    CurveEdge,
    Tangent,
    Chamfer(f32),
    Fillet(f32),
}
pub fn changed_shape(
    original: &Element,
    node: Option<usize>,
    edge: Option<usize>,
    change: PolylineChange,
    start: Point,
    current: Point,
) -> Result<Shape, String> {
    if !start.finite() || !current.finite() {
        return Err("Invalid edit parameters".into());
    }
    let Shape::Polyline {
        points: source,
        arcs: source_arcs,
    } = &original.shape
    else {
        return Err("Selected element is not a Polyline".into());
    };
    let mut points = source.clone();
    let mut arcs = if source_arcs.is_empty() {
        vec![None; source.len().saturating_sub(1)]
    } else {
        source_arcs.clone()
    };
    let delta = Point::new(current.x - start.x, current.y - start.y);
    match change {
        PolylineChange::InsertNode => {
            if let Some(index) = edge.filter(|index| *index + 1 < source.len()) {
                let a = source[index];
                let b = source[index + 1];
                let (node, split) = if let Some(arc) = arcs[index] {
                    let (center, sweep) =
                        geometry::arc_center_sweep(a, b, arc).ok_or("Invalid edit parameters")?;
                    if (current.distance(center) - arc.radius).abs() < 0.0001 {
                        let angle = (current.y - center.y).atan2(current.x - center.x);
                        let start_angle = (a.y - center.y).atan2(a.x - center.x);
                        let traveled = if sweep > 0. {
                            (angle - start_angle).rem_euclid(std::f32::consts::TAU)
                        } else {
                            (start_angle - angle).rem_euclid(std::f32::consts::TAU)
                        };
                        if traveled >= sweep.abs() {
                            return Err("Invalid edit parameters".into());
                        }
                        let mut first = arc;
                        first.large_arc = traveled > std::f32::consts::PI;
                        let mut second = arc;
                        second.large_arc = sweep.abs() - traveled > std::f32::consts::PI;
                        (current, (Some(first), Some(second)))
                    } else {
                        (current, (None, None))
                    }
                } else {
                    (current, (None, None))
                };
                if node.distance(a) < 0.01 || node.distance(b) < 0.01 {
                    return Err("Invalid edit parameters".into());
                }
                points.insert(index + 1, node);
                arcs[index] = split.0;
                arcs.insert(index + 1, split.1);
            }
        }
        PolylineChange::MoveNode => {
            if let Some(node) = node.filter(|index| *index < points.len()) {
                points[node] = current;
                if source.len() > 2
                    && source
                        .first()
                        .zip(source.last())
                        .is_some_and(|(a, b)| a.distance(*b) < 0.01)
                {
                    if node == 0 {
                        let last = points.len() - 1;
                        points[last] = current;
                    }
                    if node + 1 == points.len() {
                        points[0] = current;
                    }
                }
            }
        }
        PolylineChange::OffsetPolyline => {
            let edge = edge.or_else(|| node.map(|i| i.saturating_sub(1)));
            if let Some(index) = edge.filter(|index| *index + 1 < source.len()) {
                let a = source[index];
                let b = source[index + 1];
                let length = a.distance(b);
                if length > 1e-5 {
                    let normal = source_arcs
                        .get(index)
                        .and_then(|arc| *arc)
                        .and_then(|arc| {
                            let (center, _) = geometry::arc_center_sweep(a, b, arc)?;
                            let radial = Point::new(start.x - center.x, start.y - center.y);
                            let len = radial.x.hypot(radial.y);
                            (len > 1e-5).then(|| {
                                let sign = if arc.clockwise { 1. } else { -1. };
                                Point::new(sign * radial.x / len, sign * radial.y / len)
                            })
                        })
                        .unwrap_or(Point::new(-(b.y - a.y) / length, (b.x - a.x) / length));
                    let distance = delta.x * normal.x + delta.y * normal.y;
                    if let Some((offset, offset_arcs)) =
                        geometry::offset_polyline_with_arcs(source, source_arcs, distance)
                    {
                        points = offset;
                        arcs = offset_arcs;
                    } else if distance.abs() > 1e-6 {
                        return Err(
                            "Offset cannot preserve a connected valid path at this distance".into(),
                        );
                    }
                }
            }
        }
        PolylineChange::CurveEdge | PolylineChange::Tangent => {
            if let Some(index) = edge
                .or_else(|| node.map(|i| i.saturating_sub(1)))
                .filter(|index| *index + 1 < source.len())
            {
                arcs[index] = if change == PolylineChange::Tangent {
                    tangent_arc(
                        source[index],
                        source[index + 1],
                        current,
                        start.distance(source[index]) > start.distance(source[index + 1]),
                    )?
                } else {
                    geometry::arc_segment_through(source[index], source[index + 1], current)
                };
            }
        }
        PolylineChange::Chamfer(value) | PolylineChange::Fillet(value) => {
            let node = node.ok_or("Choose a corner node first")?;
            let closed = source.first() == source.last();
            let node = if closed && node == source.len() - 1 {
                0
            } else {
                node
            };
            if (!closed && node == 0) || node + 1 >= source.len() {
                return Err("Chamfer and Fillet need an interior node".into());
            }
            let prev = if node == 0 {
                source.len() - 2
            } else {
                node - 1
            };
            if arcs[prev].is_some() || arcs[node].is_some() {
                return Err("Select straight polylines".into());
            }
            let corner = source[node];
            let a = source[prev];
            let b = source[node + 1];
            let la = corner.distance(a);
            let lb = corner.distance(b);
            let u = Point::new((a.x - corner.x) / la, (a.y - corner.y) / la);
            let v = Point::new((b.x - corner.x) / lb, (b.y - corner.y) / lb);
            let angle = (u.x * v.x + u.y * v.y).clamp(-1., 1.).acos();
            if angle < 0.01 || (std::f32::consts::PI - angle).abs() < 0.01 {
                return Err("Corner edit needs two non-collinear segments".into());
            }
            let fillet = matches!(change, PolylineChange::Fillet(_));
            let distance = if fillet {
                value / (angle * 0.5).tan()
            } else {
                value
            };
            if !value.is_finite()
                || distance <= 0.
                || distance >= la - 0.01
                || distance >= lb - 0.01
            {
                return Err(
                    "Value must be positive and leave 10 mm on both adjacent segments".into(),
                );
            }
            let p1 = Point::new(corner.x + u.x * distance, corner.y + u.y * distance);
            let p2 = Point::new(corner.x + v.x * distance, corner.y + v.y * distance);
            let arc = fillet.then_some(ArcSegment {
                radius: value,
                clockwise: u.x * v.y - u.y * v.x > 0.,
                large_arc: false,
            });
            if node == 0 {
                points[0] = p2;
                let last = points.len() - 1;
                points[last] = p1;
                points.push(p2);
                arcs.push(arc);
            } else {
                points.splice(node..=node, [p1, p2]);
                arcs.splice(node - 1..=node, [None, arc, None]);
            }
        }
    }
    Ok(Shape::Polyline { points, arcs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{editor::Editor, reshape::ReshapeCommand};
    fn editor(points: Vec<Point>, arcs: Vec<Option<ArcSegment>>) -> (Editor, u64) {
        let mut p = Project::default();
        let id = p.add(0, Material::Plaster, Shape::Polyline { points, arcs });
        (Editor::new(p), id)
    }
    fn apply(
        e: &mut Editor,
        id: u64,
        node: Option<usize>,
        edge: Option<usize>,
        change: PolylineChange,
        anchor: Point,
        point: Point,
    ) -> Result<(), String> {
        e.reshape(ReshapeCommand::Polyline {
            id,
            node,
            edge,
            change,
            anchor,
            point,
        })
    }
    #[test]
    fn arc_insertion_keeps_circle_and_invalid_insert_is_atomic() {
        let a = Point::new(0., 0.);
        let b = Point::new(2., 0.);
        let q = Point::new(1., 1.);
        let arc = geometry::arc_segment_through(a, b, q).unwrap();
        let (mut e, id) = editor(vec![a, b], vec![Some(arc)]);
        let before = e.project.clone();
        assert!(apply(&mut e, id, None, Some(0), PolylineChange::InsertNode, q, a).is_err());
        assert_eq!(e.project, before);
        assert!(e.undo.is_empty());
        apply(&mut e, id, None, Some(0), PolylineChange::InsertNode, q, q).unwrap();
        let Shape::Polyline { points, arcs } = &e.project.elements[0].shape else {
            panic!()
        };
        assert_eq!(points, &vec![a, q, b]);
        for piece in arcs {
            let piece = piece.unwrap();
            assert_eq!(piece.radius, arc.radius);
            assert_eq!(piece.clockwise, arc.clockwise);
            assert!(!piece.large_arc);
        }
        let after = e.project.clone();
        e.undo();
        assert_eq!(e.project, before);
        e.redo();
        assert_eq!(e.project, after);
    }
    #[test]
    fn closed_endpoint_move_and_seam_corner_keep_closure() {
        let ps = vec![
            Point::new(0., 0.),
            Point::new(4., 0.),
            Point::new(4., 4.),
            Point::new(0., 4.),
            Point::new(0., 0.),
        ];
        let (mut e, id) = editor(ps, vec![]);
        let before = e.project.clone();
        apply(
            &mut e,
            id,
            Some(4),
            None,
            PolylineChange::MoveNode,
            Point::default(),
            Point::new(-1., -1.),
        )
        .unwrap();
        let Shape::Polyline { points, .. } = &e.project.elements[0].shape else {
            panic!()
        };
        assert_eq!(points[0], *points.last().unwrap());
        e.undo();
        assert_eq!(e.project, before);
        for change in [PolylineChange::Chamfer(0.25), PolylineChange::Fillet(0.25)] {
            apply(
                &mut e,
                id,
                Some(0),
                None,
                change,
                Point::default(),
                Point::default(),
            )
            .unwrap();
            let Shape::Polyline { points, arcs } = &e.project.elements[0].shape else {
                panic!()
            };
            assert_eq!(points.len(), 6);
            assert_eq!(points[0], *points.last().unwrap());
            assert_eq!(arcs.len(), 5);
            e.undo();
            assert_eq!(e.project, before);
        }
    }
    #[test]
    fn fillet_non_right_angle_is_tangent_and_rejects_invalid_corners() {
        let (mut e, id) = editor(
            vec![
                Point::new(0., 0.),
                Point::new(4., 0.),
                Point::new(6., 3.4641016),
            ],
            vec![],
        );
        let before = e.project.clone();
        for value in [0., -1., f32::NAN, 100.] {
            assert!(
                apply(
                    &mut e,
                    id,
                    Some(1),
                    None,
                    PolylineChange::Fillet(value),
                    Point::default(),
                    Point::default()
                )
                .is_err()
            );
            assert_eq!(e.project, before);
            assert!(e.undo.is_empty());
        }
        apply(
            &mut e,
            id,
            Some(1),
            None,
            PolylineChange::Fillet(0.5),
            Point::default(),
            Point::default(),
        )
        .unwrap();
        let Shape::Polyline { points, arcs } = &e.project.elements[0].shape else {
            panic!()
        };
        let arc = arcs[1].unwrap();
        let (center, _) = geometry::arc_center_sweep(points[1], points[2], arc).unwrap();
        let dot = |a: Point, b: Point, c: Point, d: Point| {
            (b.x - a.x) * (d.x - c.x) + (b.y - a.y) * (d.y - c.y)
        };
        assert!(dot(points[0], points[1], center, points[1]).abs() < 1e-4);
        assert!(dot(points[2], points[3], center, points[2]).abs() < 1e-4);
        assert!((arc.radius - 0.5).abs() < 1e-6);
    }
    #[test]
    fn curve_straighten_roundtrip_and_locked_layer_rejection() {
        let (mut e, id) = editor(vec![Point::default(), Point::new(2., 0.)], vec![]);
        apply(
            &mut e,
            id,
            None,
            Some(0),
            PolylineChange::CurveEdge,
            Point::new(1., 0.),
            Point::new(1., 1.),
        )
        .unwrap();
        let curved = e.project.clone();
        apply(
            &mut e,
            id,
            None,
            Some(0),
            PolylineChange::CurveEdge,
            Point::new(1., 1.),
            Point::new(1., 0.),
        )
        .unwrap();
        assert!(
            matches!(&e.project.elements[0].shape,Shape::Polyline{arcs,..} if arcs[0].is_none())
        );
        e.undo();
        assert_eq!(e.project, curved);
        e.project.edit_state.locked.insert(id);
        let locked = e.project.clone();
        let n = e.undo.len();
        assert!(
            apply(
                &mut e,
                id,
                Some(0),
                None,
                PolylineChange::MoveNode,
                Point::default(),
                Point::new(-1., 0.)
            )
            .is_err()
        );
        assert_eq!(e.project, locked);
        assert_eq!(e.undo.len(), n);
        let loaded: Project =
            serde_json::from_slice(&serde_json::to_vec(&curved).unwrap()).unwrap();
        assert_eq!(loaded, curved);
    }
}

pub fn tangent_arc(
    a: Point,
    b: Point,
    point: Point,
    reverse: bool,
) -> Result<Option<ArcSegment>, String> {
    let (start, end) = if reverse { (b, a) } else { (a, b) };
    let len = start.distance(point);
    if len < 0.001 {
        return Err("Define a nonzero tangent".into());
    }
    let t = Point::new((point.x - start.x) / len, (point.y - start.y) / len);
    let d = Point::new(end.x - start.x, end.y - start.y);
    let den = 2. * (-d.x * t.y + d.y * t.x);
    let contour = if den.abs() < 0.00001 {
        None
    } else {
        let h = (d.x * d.x + d.y * d.y) / den;
        let center = Point::new(start.x - t.y * h, start.y + t.x * h);
        let aa = (start.y - center.y).atan2(start.x - center.x);
        let bb = (end.y - center.y).atan2(end.x - center.x);
        let clockwise = h < 0.;
        let sweep = if clockwise {
            (aa - bb).rem_euclid(std::f32::consts::TAU)
        } else {
            (bb - aa).rem_euclid(std::f32::consts::TAU)
        };
        Some(ArcSegment {
            radius: h.abs(),
            clockwise: clockwise ^ reverse,
            large_arc: sweep > std::f32::consts::PI,
        })
    };
    Ok(contour)
}
