//! Exact wall reference geometry with derived, view-only ribbon tessellation.
//! A curved wall remains one element. Arc radius/handedness and trapezoid
//! controls are persisted; triangles and sampled stations are never persisted.
use crate::model::{Element, Point, Shape, WallContour};

fn frame(e: &Element) -> Option<(Point, Point, f32, f32)> {
    let Shape::Wall {
        start,
        end,
        thickness,
        ..
    } = e.shape
    else {
        return None;
    };
    let offset = e
        .wall_settings
        .as_ref()
        .map_or(0., |s| s.body_offset(thickness));
    Some((start, end, thickness, offset))
}
fn add(a: Point, b: Point) -> Point {
    Point::new(a.x + b.x, a.y + b.y)
}
fn sub(a: Point, b: Point) -> Point {
    Point::new(a.x - b.x, a.y - b.y)
}
fn mul(a: Point, s: f32) -> Point {
    Point::new(a.x * s, a.y * s)
}
fn dot(a: Point, b: Point) -> f32 {
    a.x * b.x + a.y * b.y
}
fn lerp(a: Point, b: Point, t: f32) -> Point {
    add(a, mul(sub(b, a), t))
}
fn left(t: Point) -> Point {
    Point::new(-t.y, t.x)
}

pub fn reference_length(e: &Element) -> f32 {
    let Some((a, b, _, _)) = frame(e) else {
        return 0.;
    };
    match e.wall_contour {
        Some(WallContour::Arc { arc }) => crate::geometry::arc_length(a, b, arc),
        _ => a.distance(b),
    }
}
/// Reference point and unit forward tangent at an arc-length station.
pub fn point_tangent_at(e: &Element, station: f32) -> Option<(Point, Point)> {
    let (a, b, _, _) = frame(e)?;
    if !station.is_finite() {
        return None;
    }
    if let Some(WallContour::Arc { arc }) = e.wall_contour {
        let (center, sweep) = crate::geometry::arc_center_sweep(a, b, arc)?;
        let length = arc.radius * sweep.abs();
        let t = (station / length).clamp(0., 1.);
        let theta = (a.y - center.y).atan2(a.x - center.x) + sweep * t;
        let sign = sweep.signum();
        let point = if t == 0. {
            a
        } else if t == 1. {
            b
        } else {
            Point::new(
                center.x + arc.radius * theta.cos(),
                center.y + arc.radius * theta.sin(),
            )
        };
        Some((point, Point::new(-theta.sin() * sign, theta.cos() * sign)))
    } else {
        let length = a.distance(b);
        if length < 1e-6 {
            return None;
        }
        let tangent = mul(sub(b, a), 1. / length);
        Some((add(a, mul(tangent, station.clamp(0., length))), tangent))
    }
}
/// Closest station and Euclidean distance to the finite reference path.
pub fn project_station(e: &Element, q: Point) -> (f32, f32) {
    let Some((a, b, _, _)) = frame(e) else {
        return (0., f32::INFINITY);
    };
    if let Some(WallContour::Arc { arc }) = e.wall_contour {
        let Some((center, sweep)) = crate::geometry::arc_center_sweep(a, b, arc) else {
            return (0., f32::INFINITY);
        };
        let theta = (q.y - center.y).atan2(q.x - center.x);
        let start = (a.y - center.y).atan2(a.x - center.x);
        let travel = if sweep > 0. {
            (theta - start).rem_euclid(std::f32::consts::TAU)
        } else {
            (start - theta).rem_euclid(std::f32::consts::TAU)
        };
        if travel <= sweep.abs() + 1e-6 {
            let station = (travel * arc.radius).clamp(0., reference_length(e));
            let point = point_tangent_at(e, station).unwrap().0;
            (station, q.distance(point))
        } else if q.distance(a) <= q.distance(b) {
            (0., q.distance(a))
        } else {
            (reference_length(e), q.distance(b))
        }
    } else {
        let length = a.distance(b);
        if length < 1e-6 {
            return (0., q.distance(a));
        }
        let station = dot(sub(q, a), sub(b, a)) / length;
        let station = station.clamp(0., length);
        (station, q.distance(lerp(a, b, station / length)))
    }
}
fn stations(e: &Element, from: f32, to: f32) -> Vec<f32> {
    let length = reference_length(e);
    let from = from.clamp(0., length);
    let to = to.clamp(0., length);
    if to <= from + 1e-6 {
        return vec![];
    }
    let mut out = vec![from, to];
    if let Some(WallContour::Arc { arc }) = e.wall_contour {
        // Max 5 degrees AND about 2 mm sagitta, bounded for pathological inputs.
        let max_angle = (2. * (1. - 0.002 / arc.radius).clamp(-1., 1.).acos())
            .min(std::f32::consts::PI / 36.)
            .max(0.001);
        let count = ((to - from) / (arc.radius * max_angle))
            .ceil()
            .clamp(1., 4096.) as usize;
        for i in 1..count {
            out.push(from + (to - from) * i as f32 / count as f32);
        }
    }
    if let Some(WallContour::Trapezoid {
        ref outer_nodes, ..
    }) = e.wall_contour
    {
        for p in outer_nodes {
            let s = project_station(e, *p).0;
            if s > from + 1e-5 && s < to - 1e-5 {
                out.push(s);
            }
        }
    }
    out.sort_by(f32::total_cmp);
    out.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
    out
}
pub fn reference_path(e: &Element) -> Vec<Point> {
    let length = reference_length(e);
    stations(e, 0., length)
        .into_iter()
        .filter_map(|s| point_tangent_at(e, s).map(|v| v.0))
        .collect()
}
/// Positive-left and negative-left body boundary at the given reference station.
pub fn section_at(e: &Element, station: f32) -> Option<(Point, Point)> {
    let (a, b, thickness, offset) = frame(e)?;
    let (point, tangent) = point_tangent_at(e, station)?;
    let normal = left(tangent);
    let mut hi = add(point, mul(normal, offset + thickness * 0.5));
    let mut lo = add(point, mul(normal, offset - thickness * 0.5));
    if let Some(WallContour::Trapezoid {
        outer_start,
        outer_end,
        ref outer_nodes,
        outer_positive,
    }) = e.wall_contour
    {
        let length = a.distance(b);
        let s = station.clamp(0., length);
        let mut controls = vec![(0., outer_start)];
        controls.extend(outer_nodes.iter().map(|p| (project_station(e, *p).0, *p)));
        controls.push((length, outer_end));
        let pair = controls
            .windows(2)
            .find(|w| s >= w[0].0 - 1e-5 && s <= w[1].0 + 1e-5)?;
        let t = ((s - pair[0].0) / (pair[1].0 - pair[0].0)).clamp(0., 1.);
        let outer = lerp(pair[0].1, pair[1].1, t);
        if outer_positive {
            hi = outer;
        } else {
            lo = outer;
        }
    }
    Some((hi, lo))
}
pub fn strips_between(e: &Element, from: f32, to: f32) -> Vec<[Point; 4]> {
    stations(e, from, to)
        .windows(2)
        .filter_map(|s| {
            let (a_hi, a_lo) = section_at(e, s[0])?;
            let (b_hi, b_lo) = section_at(e, s[1])?;
            Some([a_hi, b_hi, b_lo, a_lo])
        })
        .collect()
}
pub fn strips(e: &Element) -> Vec<[Point; 4]> {
    strips_between(e, 0., reference_length(e))
}
/// One closed ribbon contour, without repeating its first point.
pub fn outline(e: &Element) -> Vec<Point> {
    let cross: Vec<_> = stations(e, 0., reference_length(e))
        .into_iter()
        .filter_map(|s| section_at(e, s))
        .collect();
    let mut points: Vec<_> = cross.iter().map(|p| p.0).collect();
    points.extend(cross.iter().rev().map(|p| p.1));
    points
}
pub fn validate(e: &Element) -> Result<(), String> {
    let Some(contour) = &e.wall_contour else {
        return Ok(());
    };
    let (a, b, thickness, offset) = frame(e).ok_or("Wall contour requires a wall")?;
    let finite = |p: Point| p.finite() && p.x.abs() < 100_000. && p.y.abs() < 100_000.;
    match contour {
        WallContour::Arc { arc } => {
            if !arc.radius.is_finite() || arc.radius > 100_000. {
                return Err("Invalid wall arc".into());
            }
            let (_, sweep) =
                crate::geometry::arc_center_sweep(a, b, *arc).ok_or("Invalid wall arc")?;
            let inner = arc.radius - (offset + thickness * 0.5) * sweep.signum();
            let outer = arc.radius - (offset - thickness * 0.5) * sweep.signum();
            if inner.min(outer) < 0.005 || reference_length(e) < 0.01 {
                return Err("Wall arc radius is too small for its thickness".into());
            }
            if outline(e).into_iter().any(|p| !finite(p)) {
                return Err("Invalid wall contour point".into());
            }
        }
        WallContour::Trapezoid {
            outer_start,
            outer_end,
            outer_nodes,
            outer_positive,
        } => {
            if outer_nodes.len() > 256 {
                return Err("Too many wall contour nodes".into());
            }
            let length = a.distance(b);
            let tangent = mul(sub(b, a), 1. / length);
            let normal = left(tangent);
            let controls: Vec<_> = std::iter::once(*outer_start)
                .chain(outer_nodes.iter().copied())
                .chain(std::iter::once(*outer_end))
                .collect();
            let mut previous = -1.;
            for (i, p) in controls.iter().copied().enumerate() {
                if !finite(p) {
                    return Err("Invalid wall contour point".into());
                }
                let station = dot(sub(p, a), tangent);
                if (i == 0 && station.abs() > 1e-3)
                    || (i + 1 == controls.len() && (station - length).abs() > 1e-3)
                    || station < -1e-4
                    || station > length + 1e-4
                    || station <= previous + 1e-5
                {
                    return Err("Wall contour nodes must follow the reference segment".into());
                }
                let lateral = dot(sub(p, a), normal);
                let width = if *outer_positive {
                    lateral - (offset - thickness * 0.5)
                } else {
                    offset + thickness * 0.5 - lateral
                };
                if !(0.01..=100.).contains(&width) {
                    return Err("Wall contour thickness must stay positive".into());
                }
                previous = station;
            }
        }
    }
    Ok(())
}
/// Transform only persisted contour metadata. Caller transforms Shape anchors.
/// Angle is in degrees, matching editing::transform.
pub fn transform(
    e: &mut Element,
    origin: Point,
    scale: f32,
    angle: f32,
    mirror: bool,
    offset: Point,
) {
    let (sin, cos) = angle.to_radians().sin_cos();
    let f = |p: Point| {
        let x = (p.x - origin.x) * scale * if mirror { -1. } else { 1. };
        let y = (p.y - origin.y) * scale;
        Point::new(
            origin.x + x * cos - y * sin + offset.x,
            origin.y + x * sin + y * cos + offset.y,
        )
    };
    match e.wall_contour.as_mut() {
        Some(WallContour::Arc { arc }) => {
            arc.radius *= scale.abs();
            if mirror {
                arc.clockwise = !arc.clockwise;
            }
        }
        Some(WallContour::Trapezoid {
            outer_start,
            outer_end,
            outer_nodes,
            outer_positive,
        }) => {
            *outer_start = f(*outer_start);
            *outer_end = f(*outer_end);
            for p in outer_nodes {
                *p = f(*p);
            }
            if mirror {
                *outer_positive = !*outer_positive;
            }
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ArcSegment, Material, Project};
    fn wall() -> Element {
        let mut p = Project::default();
        p.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(-2., 0.),
                end: Point::new(2., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        p.elements.remove(0)
    }
    #[test]
    fn exact_arc_station_projection_and_body_share_geometry() {
        let mut e = wall();
        e.wall_contour = Some(WallContour::Arc {
            arc: ArcSegment {
                radius: 2.,
                clockwise: true,
                large_arc: false,
            },
        });
        validate(&e).unwrap();
        assert!((reference_length(&e) - 2. * std::f32::consts::PI).abs() < 1e-4);
        let (mid, tangent) = point_tangent_at(&e, reference_length(&e) * 0.5).unwrap();
        assert!(mid.distance(Point::new(0., 2.)) < 1e-4);
        assert!(tangent.distance(Point::new(1., 0.)) < 1e-4);
        let (s, d) = project_station(&e, Point::new(0., 2.5));
        assert!((s - std::f32::consts::PI).abs() < 1e-4);
        assert!((d - 0.5).abs() < 1e-4);
        let pieces = strips_between(&e, 1., 2.);
        assert!(!pieces.is_empty());
        let expected = section_at(&e, 1.).unwrap();
        assert_eq!(pieces[0][0], expected.0);
        let expected = section_at(&e, 2.).unwrap();
        assert_eq!(pieces.last().unwrap()[1], expected.0);
        assert!(outline(&e).len() > 4);
    }
    #[test]
    fn trapezoid_extra_node_and_reflection_are_persistent() {
        let mut e = wall();
        e.wall_contour = Some(WallContour::Trapezoid {
            outer_start: Point::new(-2., 0.3),
            outer_end: Point::new(2., 0.7),
            outer_nodes: vec![Point::new(0., 1.)],
            outer_positive: true,
        });
        validate(&e).unwrap();
        assert_eq!(strips(&e).len(), 2);
        assert_eq!(section_at(&e, 2.).unwrap().0, Point::new(0., 1.));
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(e, serde_json::from_str(&json).unwrap());
        transform(&mut e, Point::default(), 1., 0., true, Point::default());
        if let Shape::Wall { start, end, .. } = &mut e.shape {
            start.x = -start.x;
            end.x = -end.x;
        }
        validate(&e).unwrap();
        assert!(matches!(
            e.wall_contour,
            Some(WallContour::Trapezoid {
                outer_positive: false,
                ..
            })
        ));
        assert_eq!(section_at(&e, 2.).unwrap().1, Point::new(0., 1.));
    }
    #[test]
    fn one_arc_wall_with_opening_uses_arc_length_and_roundtrips() {
        let mut p = Project::default();
        let mut e = wall();
        e.id = 1;
        e.wall_contour = Some(WallContour::Arc {
            arc: ArcSegment {
                radius: 2.,
                clockwise: true,
                large_arc: false,
            },
        });
        p.next_id = 2;
        p.elements.push(e);
        p.add(
            0,
            Material::Timber,
            Shape::Opening {
                host_wall: 1,
                kind: crate::model::OpeningKind::Window,
                width: 0.5,
                height: 1.,
                sill: 1.,
                offset: 5.,
            },
        );
        // Offset 5 is beyond the chord length 4 but inside the true arc length 2π.
        p.validate().unwrap();
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(p, serde_json::from_str(&json).unwrap());
        assert!((p.elements[0].area() - 6. * std::f32::consts::PI).abs() < 1e-4);
        let original = p.elements[0].clone();
        p.elements[0].translate(Point::new(3., -2.));
        assert_eq!(p.elements[0].wall_contour, original.wall_contour);
        let outline = outline(&p.elements[0]);
        let before = super::outline(&original);
        for (a, b) in outline.iter().zip(before) {
            assert!(a.distance(Point::new(b.x + 3., b.y - 2.)) < 1e-4);
        }
    }
    #[test]
    fn invalid_inner_radius_and_crossed_outer_controls_rejected() {
        let mut e = wall();
        e.wall_contour = Some(WallContour::Arc {
            arc: ArcSegment {
                radius: 1.,
                clockwise: false,
                large_arc: false,
            },
        });
        assert!(validate(&e).is_err());
        e.wall_contour = Some(WallContour::Trapezoid {
            outer_start: Point::new(-2., -0.2),
            outer_end: Point::new(2., 0.3),
            outer_nodes: vec![],
            outer_positive: true,
        });
        assert!(validate(&e).is_err());
    }
}
