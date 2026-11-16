//! Canonical scalar-plane splitting of convex planar cells, shared by prisms and walls.
use crate::model::Point;
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Vertex {
    pub p: Point,
    pub h: f64,
}
pub(crate) fn coincident(a: f64, b: f64, thickness: f64) -> bool {
    (a - b).abs() <= 8. * f64::EPSILON * a.abs().max(b.abs()).max(thickness.abs()).max(1.)
}
fn key(v: Vertex) -> [u64; 2] {
    [v.p.x, v.p.y].map(|x| {
        if x == 0. {
            0.0_f64.to_bits()
        } else {
            x.to_bits()
        }
    })
}
/// The same segment in reverse must produce the same bit-identical crossing.
pub(crate) fn crossing(mut a: Vertex, mut b: Vertex, level: f64) -> Vertex {
    if a.h == level {
        return a;
    }
    if b.h == level {
        return b;
    }
    if key(a) > key(b) {
        std::mem::swap(&mut a, &mut b);
    }
    let t = (level - a.h) / (b.h - a.h);
    Vertex {
        p: Point::new(a.p.x + (b.p.x - a.p.x) * t, a.p.y + (b.p.y - a.p.y) * t),
        h: level,
    }
}
pub(crate) fn split(poly: Vec<Vertex>, level: f64) -> Vec<Vec<Vertex>> {
    if poly.iter().all(|v| v.h >= level) || poly.iter().all(|v| v.h <= level) {
        return vec![poly];
    }
    let mut out = [Vec::new(), Vec::new()];
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        if a.h <= level {
            out[0].push(a);
        }
        if a.h >= level {
            out[1].push(a);
        }
        if (a.h < level && b.h > level) || (a.h > level && b.h < level) {
            let v = crossing(a, b, level);
            out[0].push(v);
            out[1].push(v);
        }
    }
    out.into_iter().filter(|p| p.len() >= 3).collect()
}
