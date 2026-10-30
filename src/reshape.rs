//! Plan reshape geometry. All edits validate a private project and commit once.
use crate::{
    editing::{self, EditCommand},
    editor::{Editor, Tool},
    geometry,
    model::*,
};

#[derive(Clone, Debug, PartialEq)]
pub enum ReshapeCommand {
    SlabEdit {
        id: u64,
        index: usize,
        change: crate::slab_geometry::SlabChange,
        anchor: Point,
        point: Point,
    },
    InsertSlabNode {
        id: u64,
        edge: usize,
        point: Point,
    },
    Polyline {
        id: u64,
        node: Option<usize>,
        edge: Option<usize>,
        change: crate::polyline_edit::PolylineChange,
        anchor: Point,
        point: Point,
    },
    MoveWallJunction {
        ids: Vec<u64>,
        origin: Point,
        point: Point,
    },
    WallContour {
        id: u64,
        contour: Option<WallContour>,
    },
    SelectionEdit {
        ids: Vec<u64>,
        command: EditCommand,
    },
    WallTrapezoidNode {
        id: u64,
        point: Point,
    },
    InsertWallNode {
        id: u64,
        point: Point,
        split: f32,
    },
    Trim {
        id: u64,
        point: Point,
        story: u32,
    },
    Adjust {
        ids: Vec<u64>,
        a: Point,
        b: Point,
    },
    Intersect {
        ids: Vec<u64>,
    },
    Corner {
        ids: Vec<u64>,
        radius: f32,
        chamfer: bool,
    },
    Resize {
        ids: Vec<u64>,
        origin: Point,
        scale: f32,
        thickness: bool,
    },
}
fn sub(a: Point, b: Point) -> Point {
    Point::new(a.x - b.x, a.y - b.y)
}
fn cross(a: Point, b: Point) -> f32 {
    a.x * b.y - a.y * b.x
}
fn lerp(a: Point, b: Point, t: f32) -> Point {
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}
pub fn intersect(a: Point, b: Point, c: Point, d: Point) -> Option<(f32, f32)> {
    let v = sub(b, a);
    let w = sub(d, c);
    let den = cross(v, w);
    (den.abs() > 1e-6).then(|| (cross(sub(c, a), w) / den, cross(sub(c, a), v) / den))
}
pub fn straight_path(e: &Element) -> Option<Vec<Point>> {
    match &e.shape {
        Shape::Wall { start, end, .. } if e.wall_contour.is_none() => Some(vec![*start, *end]),
        Shape::Polyline { points, arcs } if arcs.iter().all(Option::is_none) => {
            Some(points.clone())
        }
        _ => None,
    }
}
pub fn source_path(e: &Element) -> Option<Vec<Point>> {
    straight_path(e).or_else(|| match e.shape {
        Shape::Slab { .. } => Some(crate::slab_geometry::closed_path(e)),
        _ => None,
    })
}
pub fn visible(p: &Project, e: &Element, story: u32) -> bool {
    e.story == story && p.plan_options.element_visible(e, &p.layers)
}
pub fn distance_segment(q: Point, a: Point, b: Point) -> (f32, f32) {
    let v = sub(b, a);
    let d = v.x * v.x + v.y * v.y;
    let t = if d > 1e-10 {
        ((q.x - a.x) * v.x + (q.y - a.y) * v.y) / d
    } else {
        0.
    };
    (q.distance(lerp(a, b, t.clamp(0., 1.))), t.clamp(0., 1.))
}
pub fn edge_at(
    p: &Project,
    story: u32,
    q: Point,
    tolerance: f32,
    exclude: &[u64],
) -> Option<(u64, Point, Point)> {
    p.elements
        .iter()
        .filter(|e| visible(p, e, story) && !exclude.contains(&e.id))
        .filter_map(|e| source_path(e).map(|ps| (e.id, ps)))
        .flat_map(|(id, ps)| ps.windows(2).map(|w| (id, w[0], w[1])).collect::<Vec<_>>())
        .filter_map(|(id, a, b)| {
            let (d, _) = distance_segment(q, a, b);
            (d <= tolerance).then_some((d, id, a, b))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id, a, b)| (id, a, b))
}
fn set_path(e: &mut Element, ps: Vec<Point>) {
    match &mut e.shape {
        Shape::Wall { start, end, .. } => {
            *start = ps[0];
            *end = *ps.last().unwrap();
        }
        Shape::Polyline { points, arcs } => {
            *arcs = vec![None; ps.len() - 1];
            *points = ps;
        }
        _ => {}
    }
}
fn editable(p: &Project, ids: &[u64], groups: bool) -> Result<(), String> {
    if ids.is_empty() {
        return Err("Select elements first".into());
    }
    for id in ids {
        let e = p
            .elements
            .iter()
            .find(|e| e.id == *id)
            .ok_or("Select elements first")?;
        if p.edit_state.locked.contains(id) || p.layers.is_locked(e.layer.0) {
            return Err("Selection contains locked elements".into());
        }
        if groups
            && !p.edit_state.suspend_groups
            && p.edit_state.groups.iter().any(|g| g.contains(id))
        {
            return Err("Suspend Groups before reshaping grouped elements".into());
        }
    }
    Ok(())
}
fn no_openings(p: &Project, ids: &[u64]) -> Result<(), String> {
    if p.elements
        .iter()
        .any(|e| matches!(e.shape,Shape::Opening{host_wall,..} if ids.contains(&host_wall)))
    {
        Err("Reshape walls without openings first".into())
    } else {
        Ok(())
    }
}
fn slice_path(ps: &[Point], from: f32, to: f32) -> Vec<Point> {
    let at = |v: f32| {
        let i = (v.floor() as usize).min(ps.len() - 2);
        lerp(ps[i], ps[i + 1], v - i as f32)
    };
    let mut out = vec![at(from)];
    for (i, p) in ps.iter().enumerate().skip(1).take(ps.len() - 2) {
        if (i as f32) > from + 1e-5 && (i as f32) < to - 1e-5 {
            out.push(*p);
        }
    }
    out.push(at(to));
    out
}
fn trim(p: &mut Project, id: u64, q: Point, story: u32) -> Result<Vec<u64>, String> {
    editable(p, &[id], true)?;
    no_openings(p, &[id])?;
    let index = p
        .elements
        .iter()
        .position(|e| e.id == id)
        .ok_or("Select elements first")?;
    let original = p.elements[index].clone();
    let ps = straight_path(&original).ok_or("Use straight walls or straight polylines")?;
    let (seg, t) = ps
        .windows(2)
        .enumerate()
        .map(|(i, w)| {
            let (d, t) = distance_segment(q, w[0], w[1]);
            (d, i, t)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, i, t)| (i, t))
        .unwrap();
    let position = seg as f32 + t;
    let end = (ps.len() - 1) as f32;
    let mut cuts = vec![0., end];
    let mut has_boundary = false;
    for e in p
        .elements
        .iter()
        .filter(|e| e.id != id && visible(p, e, story))
    {
        if let Some(boundary) = source_path(e) {
            for (i, w) in ps.windows(2).enumerate() {
                for b in boundary.windows(2) {
                    if let Some((t, u)) = intersect(w[0], w[1], b[0], b[1])
                        && (-1e-5..=1.00001).contains(&t)
                        && (-1e-5..=1.00001).contains(&u)
                    {
                        has_boundary = true;
                        cuts.push(i as f32 + t.clamp(0., 1.));
                    }
                }
            }
        }
    }
    cuts.sort_by(f32::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
    if !has_boundary {
        return Err("No intersecting boundary to trim against".into());
    }
    let interval = cuts
        .windows(2)
        .find(|w| position >= w[0] - 1e-5 && position <= w[1] + 1e-5)
        .unwrap();
    let (lo, hi) = (interval[0], interval[1]);
    let mut paths = vec![];
    if lo > 1e-5 {
        paths.push(slice_path(&ps, 0., lo));
    }
    if hi < end - 1e-5 {
        paths.push(slice_path(&ps, hi, end));
    }
    // Closed paths retain a single connected path around the untrimmed side.
    if ps[0].distance(*ps.last().unwrap()) < 1e-5 && paths.len() == 2 {
        let mut tail = paths.pop().unwrap();
        tail.extend(paths.remove(0).into_iter().skip(1));
        paths.push(tail);
    }
    p.elements.remove(index);
    let mut ids = vec![];
    for (i, path) in paths.into_iter().enumerate() {
        let mut e = original.clone();
        if i > 0 {
            e.id = p.next_id;
            p.next_id += 1;
        }
        set_path(&mut e, path);
        ids.push(e.id);
        p.elements.push(e);
    }
    Ok(ids)
}
fn corner_path(
    ps: &[Point],
    radius: f32,
    chamfer: bool,
) -> Result<(Vec<Point>, Vec<Option<ArcSegment>>), String> {
    let closed = ps[0].distance(*ps.last().unwrap()) < 1e-5;
    let n = ps.len() - usize::from(closed);
    if n < 3 {
        return Err("Select two straight elements or a polyline corner".into());
    }
    let mut pairs = vec![(Point::default(), Point::default(), None); n];
    for i in 0..n {
        let b = ps[i];
        if !closed && (i == 0 || i == n - 1) {
            pairs[i] = (b, b, None);
            continue;
        }
        let a = ps[(i + n - 1) % n];
        let c = ps[(i + 1) % n];
        let u = sub(a, b);
        let v = sub(c, b);
        let la = a.distance(b);
        let lb = c.distance(b);
        let angle = ((u.x * v.x + u.y * v.y) / (la * lb)).clamp(-1., 1.).acos();
        if angle < 0.01 || std::f32::consts::PI - angle < 0.01 {
            pairs[i] = (b, b, None);
            continue;
        }
        // All-corner operation shares each adjacent edge between its two corners.
        let left_share = if closed || i > 1 { 0.5 } else { 1. };
        let right_share = if closed || i + 2 < n { 0.5 } else { 1. };
        let distance = (radius / (angle * 0.5).tan())
            .min((la * left_share - 0.011).max(0.))
            .min((lb * right_share - 0.011).max(0.));
        let actual_radius = distance * (angle * 0.5).tan();
        let first = Point::new(b.x + u.x / la * distance, b.y + u.y / la * distance);
        let second = Point::new(b.x + v.x / lb * distance, b.y + v.y / lb * distance);
        pairs[i] = (
            first,
            second,
            (!chamfer).then_some(ArcSegment {
                radius: actual_radius,
                clockwise: cross(u, v) > 0.,
                large_arc: false,
            }),
        );
    }
    let mut out = vec![];
    let mut arcs = vec![];
    for (a, b, arc) in pairs {
        if !out.is_empty() {
            arcs.push(None);
        }
        out.push(a);
        if a.distance(b) > 1e-5 {
            out.push(b);
            arcs.push(arc);
        }
    }
    if closed {
        out.push(out[0]);
        arcs.push(None);
    }
    Ok((out, arcs))
}
/// Exact finite reference-line junctions; pointer hit tolerance belongs to the view.
pub fn wall_junctions(project: &Project, selected: &[u64], story: u32) -> Vec<(Point, Vec<u64>)> {
    const EPS: f32 = 0.0001;
    let walls: Vec<_> = project
        .elements
        .iter()
        .filter(|e| selected.contains(&e.id) && visible(project, e, story))
        .filter_map(|e| match e.shape {
            Shape::Wall { start, end, .. } if e.wall_contour.is_none() => Some((e.id, start, end)),
            _ => None,
        })
        .collect();
    let mut points: Vec<Point> = vec![];
    for (i, (_, a, b)) in walls.iter().enumerate() {
        for (_, c, d) in &walls[i + 1..] {
            let mut hits = vec![];
            if let Some((t, u)) = intersect(*a, *b, *c, *d) {
                let q = lerp(*a, *b, t);
                if (-EPS..=1. + EPS).contains(&t)
                    && (-EPS..=1. + EPS).contains(&u)
                    && distance_segment(q, *a, *b).0 <= EPS
                    && distance_segment(q, *c, *d).0 <= EPS
                {
                    hits.push(q);
                }
            }
            for q in [*a, *b] {
                if q.distance(*c) <= EPS || q.distance(*d) <= EPS {
                    hits.push(q);
                }
            }
            for q in hits {
                if !points.iter().any(|p| p.distance(q) <= EPS) {
                    points.push(q);
                }
            }
        }
    }
    points
        .into_iter()
        .filter_map(|q| {
            let ids: Vec<_> = walls
                .iter()
                .filter(|(_, a, b)| distance_segment(q, *a, *b).0 <= EPS)
                .map(|(id, _, _)| *id)
                .collect();
            (ids.len() > 1).then_some((q, ids))
        })
        .collect()
}

pub fn candidate(
    project: &Project,
    command: &ReshapeCommand,
) -> Result<(Project, Vec<u64>), String> {
    let mut p = project.clone();
    let ids = match command {
        ReshapeCommand::SlabEdit {
            id,
            index,
            change,
            anchor,
            point,
        } => {
            editable(&p, &[*id], true)?;
            let position = p
                .elements
                .iter()
                .position(|e| e.id == *id)
                .ok_or("Element not found")?;
            let original = p.elements[position].clone();
            let contours =
                crate::slab_geometry::changed(&original, *index, change, *anchor, *point)?;
            if contours.is_empty() {
                return Err("Invalid edit parameters".into());
            }
            let mut ids = Vec::new();
            for (n, contour) in contours.into_iter().enumerate() {
                let mut element = original.clone();
                crate::slab_geometry::sync(&mut element, contour)?;
                if n == 0 {
                    p.elements[position] = element;
                    ids.push(*id);
                } else {
                    element.id = p.next_id;
                    p.next_id += 1;
                    ids.push(element.id);
                    p.elements.push(element);
                }
            }
            ids
        }
        ReshapeCommand::InsertSlabNode { id, edge, point } => {
            editable(&p, &[*id], true)?;
            let element = p
                .elements
                .iter_mut()
                .find(|e| e.id == *id)
                .ok_or("Element not found")?;
            crate::slab_geometry::insert_node(element, *edge, *point)?;
            vec![*id]
        }
        ReshapeCommand::Polyline {
            id,
            node,
            edge,
            change,
            anchor,
            point,
        } => {
            editable(&p, &[*id], true)?;
            let e = p
                .elements
                .iter_mut()
                .find(|e| e.id == *id)
                .ok_or("Element not found")?;
            e.shape =
                crate::polyline_edit::changed_shape(e, *node, *edge, *change, *anchor, *point)?;
            vec![*id]
        }
        ReshapeCommand::MoveWallJunction { ids, origin, point } => {
            if !origin.finite() || !point.finite() {
                return Err("Invalid edit parameters".into());
            }
            if origin.distance(*point) < 0.00001 {
                return Err("No elements changed".into());
            }
            let mut unique = ids.clone();
            unique.sort_unstable();
            unique.dedup();
            if unique.len() < 2 {
                return Err("Select elements first".into());
            }
            editable(&p, &unique, true)?;
            let affected:Vec<_>=p.elements.iter().filter(|e|matches!(e.shape,Shape::Opening{host_wall,..} if unique.contains(&host_wall))).map(|e|e.id).collect();
            if !affected.is_empty() {
                editable(&p, &affected, false)?;
            }
            let mut result = unique.clone();
            for id in &unique {
                let index = p
                    .elements
                    .iter()
                    .position(|e| e.id == *id)
                    .ok_or("Missing wall")?;
                let original = &p.elements[index];
                let Shape::Wall { start, end, .. } = original.shape else {
                    return Err("Element is not a wall".into());
                };
                if original.wall_contour.is_some() {
                    return Err("Reference line changes require a straight wall".into());
                }
                let (distance, t) = distance_segment(*origin, start, end);
                if distance > 0.0001 {
                    return Err("Invalid edit parameters".into());
                }
                if origin.distance(start) <= 0.0001 || origin.distance(end) <= 0.0001 {
                    let (a, b) = if origin.distance(start) <= 0.0001 {
                        (*point, end)
                    } else {
                        (start, *point)
                    };
                    p.validate_wall_reference_line(*id, a, b)?;
                    set_path(&mut p.elements[index], vec![a, b]);
                } else {
                    let (next, split_ids) = candidate(
                        &p,
                        &ReshapeCommand::InsertWallNode {
                            id: *id,
                            point: *point,
                            split: t,
                        },
                    )?;
                    p = next;
                    result.extend(
                        split_ids
                            .into_iter()
                            .filter(|new_id| !unique.contains(new_id)),
                    );
                }
            }
            result
        }
        ReshapeCommand::WallContour { id, contour } => {
            let affected: Vec<_> = p
                .elements
                .iter()
                .filter(|e| {
                    e.id == *id || matches!(e.shape,Shape::Opening{host_wall,..} if host_wall==*id)
                })
                .map(|e| e.id)
                .collect();
            editable(&p, &affected, true)?;
            let e = p
                .elements
                .iter_mut()
                .find(|e| e.id == *id)
                .ok_or("Missing wall")?;
            if !matches!(e.shape, Shape::Wall { .. }) {
                return Err("Element is not a wall".into());
            }
            e.wall_contour = contour.clone();
            vec![*id]
        }
        ReshapeCommand::SelectionEdit { ids, command } => {
            let mut editor = Editor::new(p);
            editor.set_selection(ids.clone());
            editor.edit(command.clone())?;
            let ids = editor.selection_ids();
            p = editor.project;
            ids
        }
        ReshapeCommand::WallTrapezoidNode { id, point } => {
            if !point.finite() {
                return Err("Invalid edit parameters".into());
            }
            let original = p
                .elements
                .iter()
                .find(|e| e.id == *id)
                .ok_or("Missing wall")?;
            let Shape::Wall { start, end, .. } = original.shape else {
                return Err("Element is not a wall".into());
            };
            let (outer_start, outer_end, outer_positive) = match &original.wall_contour {
                None => {
                    let first = crate::wall_geometry::section_at(original, 0.)
                        .ok_or("Invalid wall geometry")?;
                    let last = crate::wall_geometry::section_at(original, start.distance(end))
                        .ok_or("Invalid wall geometry")?;
                    (first.0, last.0, true)
                }
                Some(WallContour::Trapezoid {
                    outer_start,
                    outer_end,
                    outer_nodes,
                    outer_positive,
                }) if outer_nodes.is_empty() => (*outer_start, *outer_end, *outer_positive),
                _ => return Err("Use a straight or trapezoid wall".into()),
            };
            let delta = sub(end, start);
            let length_squared = delta.x * delta.x + delta.y * delta.y;
            let split =
                ((point.x - start.x) * delta.x + (point.y - start.y) * delta.y) / length_squared;
            // Reuse node insertion's opening-span checks, host reassignment, group
            // membership and id allocation. Both reference segments remain collinear.
            let (next, ids) = candidate(
                &p,
                &ReshapeCommand::InsertWallNode {
                    id: *id,
                    point: lerp(start, end, split),
                    split,
                },
            )?;
            p = next;
            for (wall, a, b) in [(ids[0], outer_start, *point), (ids[1], *point, outer_end)] {
                p.elements
                    .iter_mut()
                    .find(|e| e.id == wall)
                    .unwrap()
                    .wall_contour = Some(WallContour::Trapezoid {
                    outer_start: a,
                    outer_end: b,
                    outer_nodes: vec![],
                    outer_positive,
                });
            }
            ids
        }
        ReshapeCommand::Trim { id, point, story } => trim(&mut p, *id, *point, *story)?,
        ReshapeCommand::InsertWallNode { id, point, split } => {
            editable(&p, &[*id], true)?;
            if !point.finite() || !split.is_finite() || *split <= 0. || *split >= 1. {
                return Err("Choose a point inside the wall segment".into());
            }
            let index = p
                .elements
                .iter()
                .position(|e| e.id == *id)
                .ok_or("Missing wall")?;
            let original = p.elements[index].clone();
            let Shape::Wall { start, end, .. } = original.shape else {
                return Err("Element is not a wall".into());
            };
            if start.distance(*point) < 0.01 || end.distance(*point) < 0.01 {
                return Err("Wall must stay at least 0.01 m long".into());
            }
            let station = start.distance(end) * split;
            let new_id = p.next_id;
            let mut affected = vec![*id];
            for e in &p.elements {
                if let Shape::Opening { host_wall, .. } = e.shape
                    && host_wall == *id
                {
                    affected.push(e.id);
                }
            }
            editable(&p, &affected, false)?;
            for e in &mut p.elements {
                if let Shape::Opening {
                    host_wall,
                    offset,
                    width,
                    ..
                } = &mut e.shape
                    && *host_wall == *id
                {
                    if *offset - *width * 0.5 < station - 1e-5
                        && *offset + *width * 0.5 > station + 1e-5
                    {
                        return Err("Cannot insert a wall node through an opening".into());
                    }
                    if *offset >= station {
                        *host_wall = new_id;
                        *offset -= station;
                    }
                }
            }
            p.elements[index].wall_contour = None;
            set_path(&mut p.elements[index], vec![start, *point]);
            let mut second = original;
            second.id = new_id;
            second.wall_contour = None;
            set_path(&mut second, vec![*point, end]);
            p.next_id += 1;
            p.elements.push(second);
            for group in &mut p.edit_state.groups {
                if group.contains(id) {
                    group.push(new_id);
                }
            }
            vec![*id, new_id]
        }
        ReshapeCommand::Adjust { ids, a, b } => {
            editable(&p, ids, true)?;
            no_openings(&p, ids)?;
            if !a.finite() || !b.finite() || a.distance(*b) < 0.001 {
                return Err("Invalid edit parameters".into());
            }
            for e in p.elements.iter_mut().filter(|e| ids.contains(&e.id)) {
                let mut ps = straight_path(e).ok_or("Use straight walls or straight polylines")?;
                let last = ps.len() - 1;
                let mut choices = vec![];
                for (i, j) in [(0, 1), (last, last - 1)] {
                    if let Some((t, _)) = intersect(ps[i], ps[j], *a, *b) {
                        let hit = lerp(ps[i], ps[j], t);
                        if hit.distance(ps[j]) > 0.01 {
                            choices.push((ps[i].distance(hit), i, hit));
                        }
                    }
                }
                if let Some((_, i, hit)) = choices.into_iter().min_by(|a, b| a.0.total_cmp(&b.0)) {
                    ps[i] = hit;
                    set_path(e, ps);
                }
            }
            ids.clone()
        }
        ReshapeCommand::Intersect { ids } => {
            editable(&p, ids, true)?;
            no_openings(&p, ids)?;
            editing::apply_edit(&mut p, ids, &EditCommand::Intersect)?;
            ids.clone()
        }
        ReshapeCommand::Corner {
            ids,
            radius,
            chamfer,
        } => {
            editable(&p, ids, true)?;
            no_openings(&p, ids)?;
            if !radius.is_finite() || *radius <= 0. {
                return Err("Invalid edit parameters".into());
            }
            if ids.len() == 2 {
                let indices: Vec<_> = p
                    .elements
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| ids.contains(&e.id))
                    .map(|(i, _)| i)
                    .collect();
                let first = p.elements[indices[0]].clone();
                let second = p.elements[indices[1]].clone();
                let a = straight_path(&first).ok_or("Use straight walls or straight polylines")?;
                let b = straight_path(&second).ok_or("Use straight walls or straight polylines")?;
                if a.len() != 2 || b.len() != 2 {
                    return Err("Select two straight elements or a polyline corner".into());
                }
                let wall = matches!(first.shape, Shape::Wall { .. });
                if wall != matches!(second.shape, Shape::Wall { .. }) {
                    return Err("Selected elements must share attributes".into());
                }
                if wall && !chamfer {
                    return Err("Curved walls are not supported; choose Chamfer".into());
                }
                let (t, u) = intersect(a[0], a[1], b[0], b[1])
                    .ok_or("Parallel elements cannot intersect")?;
                let hit = lerp(a[0], a[1], t);
                let outer_a = a[usize::from(t < 0.5)];
                let outer_b = b[usize::from(u < 0.5)];
                let (points, arcs) = corner_path(&[outer_a, hit, outer_b], *radius, *chamfer)?;
                if points.len() != 4 {
                    return Err("Parallel elements cannot intersect".into());
                }
                set_path(
                    &mut p.elements[indices[0]],
                    if t < 0.5 {
                        vec![points[1], outer_a]
                    } else {
                        vec![outer_a, points[1]]
                    },
                );
                set_path(
                    &mut p.elements[indices[1]],
                    if u < 0.5 {
                        vec![points[2], outer_b]
                    } else {
                        vec![outer_b, points[2]]
                    },
                );
                let mut bridge = first;
                bridge.id = p.next_id;
                p.next_id += 1;
                if wall {
                    set_path(&mut bridge, vec![points[1], points[2]]);
                } else {
                    bridge.shape = Shape::Polyline {
                        points: vec![points[1], points[2]],
                        arcs: vec![arcs[1]],
                    };
                }
                let mut result = ids.clone();
                result.push(bridge.id);
                p.elements.push(bridge);
                result
            } else if ids.len() == 1 {
                let e = p.elements.iter_mut().find(|e| e.id == ids[0]).unwrap();
                if !matches!(e.shape, Shape::Polyline { .. }) {
                    return Err("Select two straight elements or a polyline corner".into());
                }
                let ps = straight_path(e).ok_or("Use straight walls or straight polylines")?;
                let (points, arcs) = corner_path(&ps, *radius, *chamfer)?;
                e.shape = Shape::Polyline { points, arcs };
                ids.clone()
            } else {
                return Err("Select two straight elements or a polyline corner".into());
            }
        }
        ReshapeCommand::Resize {
            ids,
            origin,
            scale,
            thickness,
        } => {
            editable(&p, ids, true)?;
            let mut affected = ids.clone();
            for e in &p.elements {
                if let Shape::Opening { host_wall, .. } = e.shape
                    && ids.contains(&host_wall)
                {
                    affected.push(e.id);
                }
            }
            editable(&p, &affected, false)?;
            editing::apply_edit(
                &mut p,
                &affected,
                &EditCommand::Transform {
                    offset: Point::default(),
                    origin: *origin,
                    angle: 0.,
                    scale: *scale,
                    mirror: false,
                    copies: 0,
                },
            )?;
            if *thickness {
                for e in &mut p.elements {
                    if ids.contains(&e.id)
                        && let Shape::Wall {
                            thickness, height, ..
                        } = &mut e.shape
                    {
                        *thickness *= *scale;
                        *height *= *scale;
                    }
                }
            }
            ids.clone()
        }
    };
    p.validate()?;
    if p == *project {
        return Err("No elements changed".into());
    }
    Ok((p, ids))
}
impl Editor {
    pub fn reshape(&mut self, command: ReshapeCommand) -> Result<(), String> {
        let (candidate, ids) = candidate(&self.project, &command)?;
        let before = std::mem::replace(&mut self.project, candidate);
        self.commit(before);
        self.set_selection(ids);
        self.last_edit = None;
        self.status = "Edit applied".into();
        Ok(())
    }
    pub fn magic_wand(&mut self, point: Point, tolerance: f32) -> Result<(), String> {
        let path = trace(
            &self.project,
            self.story,
            &self.selection_ids(),
            point,
            tolerance,
        )?;
        let mut p = self.project.clone();
        let mut ids = vec![];
        match self.tool {
            Tool::Polyline => {
                ids.push(p.add_with_status(
                    self.story,
                    self.material,
                    Shape::Polyline {
                        points: path,
                        arcs: vec![],
                    },
                    self.wall_renovation,
                ));
            }
            Tool::Wall => {
                if p.layers.is_locked(self.wall_layer.0) {
                    return Err("Selection contains locked elements".into());
                }
                for w in path.windows(2) {
                    let id = p.add_with_status(
                        self.story,
                        self.material,
                        Shape::Wall {
                            start: w[0],
                            end: w[1],
                            thickness: self.thickness,
                            height: self.height,
                        },
                        self.wall_renovation,
                    );
                    let e = p.elements.iter_mut().find(|e| e.id == id).unwrap();
                    e.wall_settings = Some(self.wall_settings.clone());
                    e.layer = self.wall_layer;
                    ids.push(id);
                }
            }
            Tool::Slab => {
                if path.len() != 5
                    || path[0].distance(path[4]) > 1e-5
                    || path
                        .windows(2)
                        .any(|w| (w[0].x - w[1].x).abs() > 1e-5 && (w[0].y - w[1].y).abs() > 1e-5)
                {
                    return Err("Slabs require a closed axis-aligned rectangle".into());
                }
                let min = path
                    .iter()
                    .fold(Point::new(f32::INFINITY, f32::INFINITY), |a, b| {
                        Point::new(a.x.min(b.x), a.y.min(b.y))
                    });
                let max = path
                    .iter()
                    .fold(Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY), |a, b| {
                        Point::new(a.x.max(b.x), a.y.max(b.y))
                    });
                ids.push(p.add_with_status(
                    self.story,
                    self.material,
                    Shape::Slab {
                        min,
                        max,
                        thickness: self.thickness,
                    },
                    self.wall_renovation,
                ));
            }
            _ => return Err("Choose Wall, Slab or Polyline before using Magic Wand".into()),
        }
        p.validate()?;
        let before = std::mem::replace(&mut self.project, p);
        self.commit(before);
        self.set_selection(ids);
        self.last_edit = None;
        self.status = "Magic Wand created independent elements".into();
        Ok(())
    }
}
/// Trace a whole source path at an edge, or the smallest bounded face under the pointer.
/// Intersections split the planar graph; a left-hand walk enumerates bounded faces.
pub fn trace(
    p: &Project,
    story: u32,
    selection: &[u64],
    q: Point,
    tolerance: f32,
) -> Result<Vec<Point>, String> {
    let paths: Vec<_> = p
        .elements
        .iter()
        .filter(|e| visible(p, e, story) && (selection.is_empty() || selection.contains(&e.id)))
        .filter_map(source_path)
        .collect();
    if let Some((_, path)) = paths
        .iter()
        .filter_map(|ps| {
            ps.windows(2)
                .map(|w| distance_segment(q, w[0], w[1]).0)
                .min_by(f32::total_cmp)
                .filter(|d| *d <= tolerance)
                .map(|d| (d, ps))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
    {
        let mut chain = path.clone();
        let mut used = vec![paths.iter().position(|p| std::ptr::eq(p, path)).unwrap()];
        // Extend only unambiguous endpoint connections. A branch needs an interior
        // face click; never choose an arbitrary branch from iteration order.
        for reverse in [false, true] {
            if reverse {
                chain.reverse();
            }
            loop {
                if chain[0].distance(*chain.last().unwrap()) < 1e-5 {
                    break;
                }
                let end = *chain.last().unwrap();
                let joins: Vec<_> = paths
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !used.contains(i))
                    .filter_map(|(i, p)| {
                        if end.distance(p[0]) < 1e-5 {
                            Some((i, false))
                        } else if end.distance(*p.last().unwrap()) < 1e-5 {
                            Some((i, true))
                        } else {
                            None
                        }
                    })
                    .collect();
                if joins.len() != 1 {
                    break;
                }
                let (i, backwards) = joins[0];
                if backwards {
                    chain.extend(paths[i].iter().rev().skip(1).copied());
                } else {
                    chain.extend(paths[i].iter().skip(1).copied());
                }
                used.push(i);
            }
            if reverse {
                chain.reverse();
            }
        }
        return Ok(chain);
    }
    let segments: Vec<_> = paths
        .iter()
        .flat_map(|ps| ps.windows(2).map(|w| (w[0], w[1])))
        .collect();
    if segments.len() > 512 {
        return Err("Magic Wand source is too complex".into());
    }
    let mut nodes: Vec<Point> = vec![];
    let mut edges = vec![];
    for (i, (a, b)) in segments.iter().copied().enumerate() {
        let mut ts = vec![0., 1.];
        for (j, (c, d)) in segments.iter().copied().enumerate() {
            if i != j
                && let Some((t, u)) = intersect(a, b, c, d)
                && t > 1e-5
                && t < 0.99999
                && (-1e-5..=1.00001).contains(&u)
            {
                ts.push(t);
            }
        }
        ts.sort_by(f32::total_cmp);
        ts.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
        for t in ts.windows(2) {
            let mut ns = vec![];
            for v in t {
                let point = lerp(a, b, *v);
                let n = nodes
                    .iter()
                    .position(|p| p.distance(point) < 1e-4)
                    .unwrap_or_else(|| {
                        nodes.push(point);
                        nodes.len() - 1
                    });
                ns.push(n);
            }
            if ns[0] != ns[1]
                && !edges.contains(&(ns[0], ns[1]))
                && !edges.contains(&(ns[1], ns[0]))
            {
                edges.push((ns[0], ns[1]));
            }
        }
    }
    let mut adjacent = vec![vec![]; nodes.len()];
    for &(a, b) in &edges {
        adjacent[a].push(b);
        adjacent[b].push(a);
    }
    for (i, neighbors) in adjacent.iter_mut().enumerate() {
        neighbors.sort_by(|a, b| {
            (nodes[*a].y - nodes[i].y)
                .atan2(nodes[*a].x - nodes[i].x)
                .total_cmp(&(nodes[*b].y - nodes[i].y).atan2(nodes[*b].x - nodes[i].x))
        });
    }
    let mut visited = std::collections::BTreeSet::new();
    let mut faces = vec![];
    for &(a, b) in &edges {
        for start in [(a, b), (b, a)] {
            if visited.contains(&start) {
                continue;
            }
            let mut edge = start;
            let mut face = vec![];
            for _ in 0..edges.len() * 2 + 1 {
                if !visited.insert(edge) {
                    break;
                }
                face.push(nodes[edge.0]);
                let neighbors = &adjacent[edge.1];
                let at = neighbors.iter().position(|n| *n == edge.0).unwrap();
                edge = (
                    edge.1,
                    neighbors[(at + neighbors.len() - 1) % neighbors.len()],
                );
                if edge == start {
                    break;
                }
            }
            if edge != start || face.len() < 3 {
                continue;
            }
            let area: f32 = (0..face.len())
                .map(|i| cross(face[i], face[(i + 1) % face.len()]))
                .sum::<f32>()
                / 2.;
            if area > 1e-5 && geometry::point_in_polygon(q, &face) {
                face.push(face[0]);
                faces.push((area, face));
            }
        }
    }
    faces
        .into_iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, ps)| ps)
        .ok_or_else(|| "No closed boundary here; click an edge or enclosed region".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn line(p: &mut Project, a: (f32, f32), b: (f32, f32)) -> u64 {
        p.add(
            0,
            Material::Concrete,
            Shape::Polyline {
                points: vec![Point::new(a.0, a.1), Point::new(b.0, b.1)],
                arcs: vec![],
            },
        )
    }
    fn assert_roundtrip(p: &Project) {
        let s = serde_json::to_string(p).unwrap();
        assert_eq!(*p, serde_json::from_str::<Project>(&s).unwrap());
    }
    fn node_wall(p: &mut Project) -> u64 {
        p.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(1., 2.),
                end: Point::new(7., 10.),
                thickness: 0.3,
                height: 3.,
            },
        )
    }
    fn node_opening(p: &mut Project, host_wall: u64, offset: f32) -> u64 {
        p.add(
            0,
            Material::Timber,
            Shape::Opening {
                host_wall,
                kind: OpeningKind::Window,
                width: 1.,
                height: 1.,
                sill: 1.,
                offset,
            },
        )
    }
    #[test]
    fn trapezoid_node_splits_rotated_reference_shares_width_and_rehosts_openings() {
        let mut p = Project::default();
        let id = node_wall(&mut p);
        let opening = node_opening(&mut p, id, 8.);
        // Midpoint (4,6), positive normal (-.8,.6), outer width .9.
        let point = Point::new(3.28, 6.54);
        let old_outer = crate::wall_geometry::section_at(&p.elements[0], 0.)
            .unwrap()
            .0;
        let before = p.clone();
        let mut editor = Editor::new(p);
        let command = ReshapeCommand::WallTrapezoidNode { id, point };
        let (preview, ids) = candidate(&editor.project, &command).unwrap();
        assert_eq!(editor.project, before);
        editor.reshape(command).unwrap();
        assert_eq!(editor.project, preview);
        let a = editor
            .project
            .elements
            .iter()
            .find(|e| e.id == ids[0])
            .unwrap();
        let b = editor
            .project
            .elements
            .iter()
            .find(|e| e.id == ids[1])
            .unwrap();
        assert!(
            matches!(a.shape, Shape::Wall { end, .. } if end.distance(Point::new(4.,6.)) < 1e-5)
        );
        assert!(
            matches!(b.shape, Shape::Wall { start, .. } if start.distance(Point::new(4.,6.)) < 1e-5)
        );
        let sa = crate::wall_geometry::section_at(a, 5.).unwrap();
        let sb = crate::wall_geometry::section_at(b, 0.).unwrap();
        assert!(sa.0.distance(point) < 1e-5);
        assert!(sa.0.distance(sb.0) < 1e-5 && sa.1.distance(sb.1) < 1e-5);
        assert_eq!(
            crate::wall_geometry::section_at(a, 0.).unwrap().0,
            old_outer
        );
        assert_eq!(a.wall_settings, b.wall_settings);
        assert!(
            matches!(editor.project.elements.iter().find(|e| e.id == opening).unwrap().shape,
            Shape::Opening { host_wall, offset, .. } if host_wall == ids[1] && (offset-3.).abs()<1e-5)
        );
        assert_eq!(editor.undo.len(), 1);
        assert_roundtrip(&editor.project);
        let after = editor.project.clone();
        editor.undo();
        assert_eq!(editor.project, before);
        editor.redo();
        assert_eq!(editor.project, after);
    }
    #[test]
    fn trapezoid_node_preserves_existing_outer_ends_and_rejects_invalid_atomically() {
        let mut p = Project::default();
        let id = node_wall(&mut p);
        let first = Point::new(0.6, 2.3);
        let last = Point::new(6.2, 10.6);
        p.elements[0].wall_contour = Some(WallContour::Trapezoid {
            outer_start: first,
            outer_end: last,
            outer_nodes: vec![],
            outer_positive: true,
        });
        let (next, ids) = candidate(
            &p,
            &ReshapeCommand::WallTrapezoidNode {
                id,
                point: Point::new(3.04, 6.72),
            },
        )
        .unwrap();
        assert!(
            matches!(next.elements.iter().find(|e|e.id==ids[0]).unwrap().wall_contour,
            Some(WallContour::Trapezoid {outer_start,..}) if outer_start==first)
        );
        assert!(
            matches!(next.elements.iter().find(|e|e.id==ids[1]).unwrap().wall_contour,
            Some(WallContour::Trapezoid {outer_end,..}) if outer_end==last)
        );
        let opening = node_opening(&mut p, id, 5.);
        let mut editor = Editor::new(p.clone());
        for point in [
            Point::new(3.04, 6.72),
            Point::new(1., 2.),
            Point::new(4.8, 5.4),
            Point::new(f32::NAN, 0.),
        ] {
            assert!(
                editor
                    .reshape(ReshapeCommand::WallTrapezoidNode { id, point })
                    .is_err()
            );
            assert_eq!(editor.project, p);
            assert!(editor.undo.is_empty());
        }
        p.elements.retain(|e| e.id != opening);
        p.elements[0].wall_contour = Some(WallContour::Arc {
            arc: ArcSegment {
                radius: 5.,
                clockwise: false,
                large_arc: false,
            },
        });
        assert!(
            candidate(
                &p,
                &ReshapeCommand::WallTrapezoidNode {
                    id,
                    point: Point::new(3.04, 6.72)
                }
            )
            .is_err()
        );
    }
    #[test]
    fn wall_contour_edit_keeps_one_id_preview_undo_openings_and_persistence() {
        let mut p = Project::default();
        let id = node_wall(&mut p);
        let opening = node_opening(&mut p, id, 8.);
        let before = p.clone();
        let mut editor = Editor::new(p);
        let contour = Some(WallContour::Arc {
            arc: ArcSegment {
                radius: 5.,
                clockwise: true,
                large_arc: false,
            },
        });
        let cmd = ReshapeCommand::WallContour {
            id,
            contour: contour.clone(),
        };
        let (preview, ids) = candidate(&editor.project, &cmd).unwrap();
        assert_eq!(ids, vec![id]);
        assert_eq!(editor.project, before);
        editor.reshape(cmd).unwrap();
        assert_eq!(editor.project, preview);
        assert_eq!(editor.project.elements.len(), 2);
        assert_eq!(editor.project.elements[0].wall_contour, contour);
        assert!(
            matches!(editor.project.elements.iter().find(|e|e.id==opening).unwrap().shape,
            Shape::Opening{host_wall,offset,..} if host_wall==id && offset==8.)
        );
        assert_roundtrip(&editor.project);
        let after = editor.project.clone();
        editor.undo();
        assert_eq!(editor.project, before);
        editor.redo();
        assert_eq!(editor.project, after);
        editor.project.edit_state.locked.insert(opening);
        let locked = editor.project.clone();
        assert!(
            editor
                .reshape(ReshapeCommand::WallContour { id, contour: None })
                .is_err()
        );
        assert_eq!(editor.project, locked);
    }
    #[test]
    fn selection_edit_copies_exact_arc_and_opening_atomically_with_mirror_rotation() {
        let mut p = Project::default();
        let id = node_wall(&mut p);
        node_opening(&mut p, id, 8.);
        p.elements[0].wall_contour = Some(WallContour::Arc {
            arc: ArcSegment {
                radius: 5.,
                clockwise: true,
                large_arc: false,
            },
        });
        let before = p.clone();
        let mut editor = Editor::new(p);
        editor
            .reshape(ReshapeCommand::SelectionEdit {
                ids: vec![id],
                command: EditCommand::Transform {
                    offset: Point::new(20., 0.),
                    origin: Point::default(),
                    angle: 90.,
                    scale: 2.,
                    mirror: true,
                    copies: 1,
                },
            })
            .unwrap();
        assert_eq!(editor.project.elements.len(), 4);
        assert_eq!(editor.undo.len(), 1);
        let copy = editor
            .project
            .elements
            .iter()
            .find(|e| e.id != id && matches!(e.shape, Shape::Wall { .. }))
            .unwrap();
        assert!(
            matches!(copy.wall_contour,Some(WallContour::Arc {arc}) if arc.radius==10. && !arc.clockwise)
        );
        assert!(
            matches!(copy.shape,Shape::Wall{start,end,..} if start.distance(Point::new(16.,-2.))<1e-4 && end.distance(Point::new(0.,-14.))<1e-4)
        );
        assert!(editor.project.elements.iter().any(|e|matches!(e.shape,Shape::Opening{host_wall,offset,width,..} if host_wall==copy.id && offset==16. && width==2.)));
        assert_roundtrip(&editor.project);
        let after = editor.project.clone();
        editor.undo();
        assert_eq!(editor.project, before);
        editor.redo();
        assert_eq!(editor.project, after);
    }
    #[test]
    fn wall_node_split_preserves_endpoints_metadata_openings_and_roundtrip() {
        let mut p = Project::default();
        let id = node_wall(&mut p);
        let first = p.add(
            0,
            Material::Timber,
            Shape::Opening {
                host_wall: id,
                kind: OpeningKind::Window,
                width: 1.,
                height: 1.,
                sill: 1.,
                offset: 2.,
            },
        );
        let second = p.add(
            0,
            Material::Timber,
            Shape::Opening {
                host_wall: id,
                kind: OpeningKind::Window,
                width: 1.,
                height: 1.,
                sill: 1.,
                offset: 8.,
            },
        );
        let before = p.clone();
        let mut e = Editor::new(p);
        let point = Point::new(6., 4.);
        let command = ReshapeCommand::InsertWallNode {
            id,
            point,
            split: 0.5,
        };
        let (preview, ids) = candidate(&e.project, &command).unwrap();
        assert_eq!(e.project, before);
        e.reshape(command).unwrap();
        assert_eq!(e.project, preview);
        let a = e.project.elements.iter().find(|e| e.id == ids[0]).unwrap();
        let b = e.project.elements.iter().find(|e| e.id == ids[1]).unwrap();
        assert_eq!(straight_path(a).unwrap(), vec![Point::new(1., 2.), point]);
        assert_eq!(straight_path(b).unwrap(), vec![point, Point::new(7., 10.)]);
        assert_eq!(a.material, b.material);
        assert_eq!(a.layer, b.layer);
        assert_eq!(a.wall_settings, b.wall_settings);
        for (opening, host, offset) in [(first, id, 2.), (second, ids[1], 3.)] {
            assert!(
                matches!(e.project.elements.iter().find(|e| e.id==opening).unwrap().shape,Shape::Opening{host_wall,offset:o,..} if host_wall==host && (o-offset).abs()<1e-5)
            );
        }
        assert_eq!(e.undo.len(), 1);
        assert_roundtrip(&e.project);
        let after = e.project.clone();
        e.undo();
        assert_eq!(e.project, before);
        e.redo();
        assert_eq!(e.project, after);
    }
    #[test]
    fn wall_node_rejects_opening_crossing_locks_groups_and_invalid_atomically() {
        let mut p = Project::default();
        let id = node_wall(&mut p);
        let cmd = ReshapeCommand::InsertWallNode {
            id,
            point: Point::new(5., 5.),
            split: 0.5,
        };
        let opening = p.add(
            0,
            Material::Timber,
            Shape::Opening {
                host_wall: id,
                kind: OpeningKind::Window,
                width: 2.,
                height: 1.,
                sill: 1.,
                offset: 5.,
            },
        );
        assert!(
            candidate(&p, &cmd)
                .unwrap_err()
                .contains("through an opening")
        );
        p.elements.retain(|e| e.id != opening);
        p.edit_state.locked.insert(id);
        assert!(candidate(&p, &cmd).is_err());
        p.edit_state.locked.clear();
        p.edit_state.groups.push(vec![id]);
        assert!(candidate(&p, &cmd).is_err());
        p.edit_state.suspend_groups = true;
        let (split, ids) = candidate(&p, &cmd).unwrap();
        assert!(split.edit_state.groups[0].contains(&ids[1]));
        let before = p.clone();
        let mut e = Editor::new(p);
        for point in [Point::new(1., 2.), Point::new(f32::NAN, 0.)] {
            assert!(
                e.reshape(ReshapeCommand::InsertWallNode {
                    id,
                    point,
                    split: 0.5
                })
                .is_err()
            );
            assert_eq!(e.project, before);
            assert!(e.undo.is_empty());
        }
    }
    #[test]
    fn reshape_trim_middle_preserves_metadata_and_undo() {
        let mut p = Project::default();
        let id = line(&mut p, (0., 0.), (10., 0.));
        line(&mut p, (3., -1.), (3., 1.));
        line(&mut p, (7., -1.), (7., 1.));
        let original = p.clone();
        let mut e = Editor::new(p);
        e.reshape(ReshapeCommand::Trim {
            id,
            point: Point::new(5., 0.),
            story: 0,
        })
        .unwrap();
        let paths: Vec<_> = e
            .project
            .elements
            .iter()
            .filter(|e| e.name == original.elements[0].name)
            .map(|e| straight_path(e).unwrap())
            .collect();
        assert_eq!(
            paths,
            vec![
                vec![Point::new(0., 0.), Point::new(3., 0.)],
                vec![Point::new(7., 0.), Point::new(10., 0.)]
            ]
        );
        assert_eq!(e.undo.len(), 1);
        let after = e.project.clone();
        assert_roundtrip(&after);
        e.undo();
        assert_eq!(e.project, original);
        e.redo();
        assert_eq!(e.project, after);
    }
    #[test]
    fn reshape_trim_repeated_ends_use_nearest_wall_baseline_and_slab_edges() {
        for from_left in [true, false] {
            let mut p = Project::default();
            let id = p.add(
                0,
                Material::Concrete,
                Shape::Wall {
                    start: Point::new(0., 0.),
                    end: Point::new(10., 0.),
                    thickness: 0.2,
                    height: 3.,
                },
            );
            // Wall reference at x=2, not its x=1.8/2.2 footprint.
            p.add(
                0,
                Material::Concrete,
                Shape::Wall {
                    start: Point::new(2., -2.),
                    end: Point::new(2., 2.),
                    thickness: 0.4,
                    height: 3.,
                },
            );
            p.add(
                0,
                Material::Concrete,
                Shape::Slab {
                    min: Point::new(4., -2.),
                    max: Point::new(6., 2.),
                    thickness: 0.2,
                },
            );
            line(&mut p, (8., -2.), (8., 2.));
            // A disjoint finite line must not be extrapolated as a cutting boundary.
            line(&mut p, (1., 3.), (1., 4.));
            let before = p.clone();
            let mut e = Editor::new(p);
            let clicks = if from_left {
                [1., 3., 5., 7., 9.]
            } else {
                [9., 7., 5., 3., 1.]
            };
            let mut history = vec![before];
            for (step, x) in clicks.into_iter().enumerate() {
                let cmd = ReshapeCommand::Trim {
                    id,
                    point: Point::new(x, 0.),
                    story: 0,
                };
                let preview_before = e.project.clone();
                let (preview, _) = candidate(&e.project, &cmd).unwrap();
                assert_eq!(e.project, preview_before);
                e.reshape(cmd).unwrap();
                assert_eq!(e.project, preview);
                assert_eq!(e.undo.len(), step + 1);
                if step < 4 {
                    let wall = e.project.elements.iter().find(|e| e.id == id).unwrap();
                    let expected = if from_left {
                        vec![Point::new((step + 1) as f32 * 2., 0.), Point::new(10., 0.)]
                    } else {
                        vec![Point::new(0., 0.), Point::new(8. - step as f32 * 2., 0.)]
                    };
                    assert_eq!(straight_path(wall).unwrap(), expected);
                } else {
                    assert!(!e.project.elements.iter().any(|e| e.id == id));
                }
                assert_roundtrip(&e.project);
                history.push(e.project.clone());
            }
            for expected in history[..5].iter().rev() {
                e.undo();
                assert_eq!(&e.project, expected);
            }
            for expected in &history[1..] {
                e.redo();
                assert_eq!(&e.project, expected);
            }
        }
    }
    #[test]
    fn reshape_trim_endpoint_bounded_middle_remainder_and_no_boundary() {
        let mut p = Project::default();
        let id = line(&mut p, (0., 0.), (10., 0.));
        for x in [0., 3., 7., 10.] {
            line(&mut p, (x, -2.), (x, 2.));
        }
        let mut e = Editor::new(p);
        e.reshape(ReshapeCommand::Trim {
            id,
            point: Point::new(5., 0.),
            story: 0,
        })
        .unwrap();
        let remainder = e.selection_ids();
        assert_eq!(remainder.len(), 2);
        for (id, x) in remainder.into_iter().zip([1., 9.]) {
            e.reshape(ReshapeCommand::Trim {
                id,
                point: Point::new(x, 0.),
                story: 0,
            })
            .unwrap();
            assert!(!e.project.elements.iter().any(|e| e.id == id));
        }
        assert_eq!(e.project.elements.len(), 4);
        let isolated = line(&mut e.project, (20., 0.), (22., 0.));
        let before = e.project.clone();
        assert!(
            e.reshape(ReshapeCommand::Trim {
                id: isolated,
                point: Point::new(21., 0.),
                story: 0
            })
            .is_err()
        );
        assert_eq!(e.project, before);
        assert_eq!(e.undo.len(), 3);
    }
    #[test]
    fn reshape_adjust_preview_cancel_parallel_and_locked_are_atomic() {
        let mut p = Project::default();
        let id = line(&mut p, (0., 0.), (2., 0.));
        let before = p.clone();
        let cmd = ReshapeCommand::Adjust {
            ids: vec![id],
            a: Point::new(4., -1.),
            b: Point::new(4., 1.),
        };
        let (preview, _) = candidate(&p, &cmd).unwrap();
        assert_eq!(p, before);
        assert_eq!(
            straight_path(&preview.elements[0]).unwrap()[1],
            Point::new(4., 0.)
        );
        let mut e = Editor::new(p);
        e.project.edit_state.locked.insert(id);
        let locked = e.project.clone();
        assert!(e.reshape(cmd).is_err());
        assert_eq!(e.project, locked);
        assert!(e.undo.is_empty());
        e.project.edit_state.locked.clear();
        assert!(
            e.reshape(ReshapeCommand::Adjust {
                ids: vec![id],
                a: Point::new(0., 2.),
                b: Point::new(4., 2.)
            })
            .is_err()
        );
        assert!(e.undo.is_empty());
    }
    #[test]
    fn reshape_fillet_and_chamfer_use_same_tangency_points() {
        let mut p = Project::default();
        let a = line(&mut p, (0., 0.), (4., 0.));
        let b = line(&mut p, (4., 0.), (4., 4.));
        for chamfer in [false, true] {
            let (result, ids) = candidate(
                &p,
                &ReshapeCommand::Corner {
                    ids: vec![a, b],
                    radius: 1.,
                    chamfer,
                },
            )
            .unwrap();
            assert_eq!(ids.len(), 3);
            let Shape::Polyline { points, arcs } = &result.elements[2].shape else {
                panic!()
            };
            assert!((points[0].x - 3.).abs() < 1e-5);
            assert!((points[1].y - 1.).abs() < 1e-5);
            assert_eq!(arcs[0].is_none(), chamfer);
            assert_roundtrip(&result);
        }
    }
    #[test]
    fn reshape_all_polygon_corners_and_large_radius_are_valid() {
        let mut p = Project::default();
        let id = p.add(
            0,
            Material::Concrete,
            Shape::Polyline {
                points: vec![
                    Point::new(0., 0.),
                    Point::new(4., 0.),
                    Point::new(4., 3.),
                    Point::new(0., 3.),
                    Point::new(0., 0.),
                ],
                arcs: vec![],
            },
        );
        let (result, _) = candidate(
            &p,
            &ReshapeCommand::Corner {
                ids: vec![id],
                radius: 100.,
                chamfer: false,
            },
        )
        .unwrap();
        let Shape::Polyline { points, arcs } = &result.elements[0].shape else {
            panic!()
        };
        assert_eq!(points.len(), 9);
        assert_eq!(arcs.iter().flatten().count(), 4);
        assert_eq!(points.first(), points.last());
        assert_roundtrip(&result);
    }
    #[test]
    fn reshape_magic_wand_finds_smallest_face_and_keeps_source() {
        let mut p = Project::default();
        for (a, b) in [
            ((0., 0.), (4., 0.)),
            ((4., 0.), (4., 4.)),
            ((4., 4.), (0., 4.)),
            ((0., 4.), (0., 0.)),
            ((2., 0.), (2., 4.)),
        ] {
            line(&mut p, a, b);
        }
        let path = trace(&p, 0, &[], Point::new(1., 2.), 0.05).unwrap();
        assert_eq!(path.len(), 5);
        assert!(path.iter().all(|p| p.x <= 2.));
        let source = p.clone();
        let mut e = Editor::new(p);
        e.tool = Tool::Slab;
        e.magic_wand(Point::new(1., 2.), 0.05).unwrap();
        assert_eq!(&e.project.elements[..5], source.elements);
        assert_eq!(e.undo.len(), 1);
        let Shape::Slab { min, max, .. } = e.project.elements[5].shape else {
            panic!()
        };
        assert_eq!(min, Point::new(0., 0.));
        assert_eq!(max, Point::new(2., 4.));
        let after = e.project.clone();
        assert_roundtrip(&after);
        e.undo();
        assert_eq!(e.project, source);
        e.redo();
        assert_eq!(e.project, after);
    }
    #[test]
    fn reshape_magic_wand_selection_and_nonrectangle_rejection() {
        let mut p = Project::default();
        let id = p.add(
            0,
            Material::Concrete,
            Shape::Polyline {
                points: vec![
                    Point::new(0., 0.),
                    Point::new(4., 0.),
                    Point::new(2., 3.),
                    Point::new(0., 0.),
                ],
                arcs: vec![],
            },
        );
        assert!(trace(&p, 0, &[999], Point::new(2., 1.), 0.05).is_err());
        let mut e = Editor::new(p.clone());
        e.tool = Tool::Slab;
        assert!(e.magic_wand(Point::new(2., 1.), 0.05).is_err());
        assert_eq!(e.project, p);
        assert!(e.undo.is_empty());
        e.tool = Tool::Wall;
        e.set_selection(vec![id]);
        e.magic_wand(Point::new(2., 1.), 0.05).unwrap();
        assert_eq!(e.project.elements.len(), 4);
        assert_eq!(e.undo.len(), 1);
    }
    #[test]
    fn reshape_resize_fixed_point_and_optional_thickness() {
        let mut p = Project::default();
        let id = p.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(1., 1.),
                end: Point::new(3., 1.),
                thickness: 0.2,
                height: 3.,
            },
        );
        for thickness in [false, true] {
            let (r, _) = candidate(
                &p,
                &ReshapeCommand::Resize {
                    ids: vec![id],
                    origin: Point::new(1., 1.),
                    scale: 2.,
                    thickness,
                },
            )
            .unwrap();
            let Shape::Wall {
                start,
                end,
                thickness: t,
                height,
            } = r.elements[0].shape
            else {
                panic!()
            };
            assert_eq!(start, Point::new(1., 1.));
            assert_eq!(end, Point::new(5., 1.));
            assert_eq!(t, if thickness { 0.4 } else { 0.2 });
            assert_eq!(height, if thickness { 6. } else { 3. });
            assert_roundtrip(&r);
        }
        assert!(
            candidate(
                &p,
                &ReshapeCommand::Resize {
                    ids: vec![id],
                    origin: Point::default(),
                    scale: f32::NAN,
                    thickness: true
                }
            )
            .is_err()
        );
    }
    #[test]
    fn reshape_group_and_hosted_wall_guards() {
        let mut p = Project::default();
        let a = line(&mut p, (0., 0.), (2., 0.));
        let b = line(&mut p, (3., 1.), (3., 3.));
        p.edit_state.groups.push(vec![a, b]);
        assert!(candidate(&p, &ReshapeCommand::Intersect { ids: vec![a, b] }).is_err());
        p.edit_state.suspend_groups = true;
        let (p, _) = candidate(&p, &ReshapeCommand::Intersect { ids: vec![a, b] }).unwrap();
        assert_eq!(
            straight_path(&p.elements[0]).unwrap()[1],
            Point::new(3., 0.)
        );
    }
    fn joint_wall(p: &mut Project, a: Point, b: Point) -> u64 {
        p.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: a,
                end: b,
                thickness: 0.2,
                height: 3.,
            },
        )
    }
    #[test]
    fn wall_junction_all_endpoints_move_once_and_roundtrip() {
        let mut p = Project::default();
        let origin = Point::new(1., 1.);
        let ends = [
            Point::new(-3., 1.),
            Point::new(5., 1.),
            Point::new(1., 5.),
            Point::new(1., -3.),
        ];
        let ids: Vec<_> = ends
            .iter()
            .enumerate()
            .map(|(i, end)| {
                if i % 2 == 0 {
                    joint_wall(&mut p, origin, *end)
                } else {
                    joint_wall(&mut p, *end, origin)
                }
            })
            .collect();
        let unrelated = joint_wall(&mut p, Point::new(7., 0.), Point::new(8., 0.));
        let q = Point::new(2., 2.);
        let before = p.clone();
        let mut editor = Editor::new(p);
        editor
            .reshape(ReshapeCommand::MoveWallJunction {
                ids: ids.clone(),
                origin,
                point: q,
            })
            .unwrap();
        assert_eq!(editor.project.elements.len(), 5);
        assert_eq!(editor.undo.len(), 1);
        for (i, id) in ids.iter().enumerate() {
            let e = editor
                .project
                .elements
                .iter()
                .find(|e| e.id == *id)
                .unwrap();
            let path = straight_path(e).unwrap();
            assert_eq!(
                path,
                if i % 2 == 0 {
                    vec![q, ends[i]]
                } else {
                    vec![ends[i], q]
                }
            );
        }
        assert_eq!(
            editor.project.elements.iter().find(|e| e.id == unrelated),
            before.elements.iter().find(|e| e.id == unrelated)
        );
        let after = editor.project.clone();
        assert_roundtrip(&after);
        editor.undo();
        assert_eq!(editor.project, before);
        editor.redo();
        assert_eq!(editor.project, after);
    }
    #[test]
    fn wall_junction_t_and_x_split_only_through_walls() {
        for cross in [false, true] {
            let mut p = Project::default();
            let origin = Point::default();
            let a = joint_wall(&mut p, Point::new(-4., 0.), Point::new(4., 0.));
            let b = joint_wall(
                &mut p,
                if cross { Point::new(0., -4.) } else { origin },
                Point::new(0., 4.),
            );
            let q = Point::new(1., 1.);
            let before = p.clone();
            let (after, ids) = candidate(
                &p,
                &ReshapeCommand::MoveWallJunction {
                    ids: vec![a, b],
                    origin,
                    point: q,
                },
            )
            .unwrap();
            assert_eq!(p, before);
            assert_eq!(after.elements.len(), if cross { 4 } else { 3 });
            assert_eq!(ids.len(), after.elements.len());
            let paths: Vec<_> = after
                .elements
                .iter()
                .map(|e| straight_path(e).unwrap())
                .collect();
            assert!(paths.iter().all(|ps| ps[0] == q || ps[1] == q));
            for fixed in [Point::new(-4., 0.), Point::new(4., 0.), Point::new(0., 4.)] {
                assert!(paths.iter().any(|ps| ps.contains(&fixed)));
            }
            if cross {
                assert!(paths.iter().any(|ps| ps.contains(&Point::new(0., -4.))));
            }
            assert_roundtrip(&after);
        }
    }
    #[test]
    fn wall_junction_rejects_locked_invalid_or_nonincident_atomically() {
        let mut p = Project::default();
        let origin = Point::default();
        let a = joint_wall(&mut p, origin, Point::new(4., 0.));
        let b = joint_wall(&mut p, origin, Point::new(0., 4.));
        for case in 0..4 {
            let mut p = p.clone();
            if case == 0 {
                p.edit_state.locked.insert(b);
            }
            if case == 1 {
                p.edit_state.groups.push(vec![a, b]);
            }
            let point = if case == 2 {
                Point::new(0., 4.)
            } else {
                Point::new(1., 1.)
            };
            let origin = if case == 3 {
                Point::new(1., 0.)
            } else {
                origin
            };
            let before = p.clone();
            let mut editor = Editor::new(p);
            assert!(
                editor
                    .reshape(ReshapeCommand::MoveWallJunction {
                        ids: vec![a, b],
                        origin,
                        point
                    })
                    .is_err()
            );
            assert_eq!(editor.project, before);
            assert!(editor.undo.is_empty());
        }
    }
    #[test]
    fn wall_junction_openings_rehost_and_failed_split_rolls_back_all() {
        let mut p = Project::default();
        let a = joint_wall(&mut p, Point::new(-5., 0.), Point::new(5., 0.));
        let b = joint_wall(&mut p, Point::default(), Point::new(0., 5.));
        let opening = node_opening(&mut p, a, 8.);
        let cmd = ReshapeCommand::MoveWallJunction {
            ids: vec![a, b],
            origin: Point::default(),
            point: Point::new(1., 1.),
        };
        let (after, _) = candidate(&p, &cmd).unwrap();
        assert!(after.elements.iter().any(|e|e.id==opening&&matches!(e.shape,Shape::Opening{host_wall,offset,..} if host_wall!=a&&(offset-3.).abs()<1e-5)));
        if let Shape::Opening { offset, .. } = &mut p
            .elements
            .iter_mut()
            .find(|e| e.id == opening)
            .unwrap()
            .shape
        {
            *offset = 5.;
        }
        let before = p.clone();
        let mut editor = Editor::new(p);
        assert!(editor.reshape(cmd).is_err());
        assert_eq!(editor.project, before);
        assert!(editor.undo.is_empty());
    }
    #[test]
    fn wall_junction_hit_uses_finite_geometry_not_pointer_tolerance() {
        let mut p = Project::default();
        let a = joint_wall(&mut p, Point::new(-4., 0.), Point::new(4., 0.));
        let b = joint_wall(&mut p, Point::default(), Point::new(0., 4.));
        let nearby = joint_wall(&mut p, Point::new(0.02, 0.02), Point::new(4., 3.));
        let joint = wall_junctions(&p, &[a, b, nearby], 0);
        assert_eq!(joint, vec![(Point::default(), vec![a, b])]);
        assert!(wall_junctions(&p, &[a], 0).is_empty());
        assert!(wall_junctions(&p, &[a, b], 1).is_empty());
    }
}

#[cfg(test)]
mod chain_tests {
    use super::*;
    #[test]
    fn reshape_magic_wand_edge_traces_connected_chain() {
        let mut p = Project::default();
        for (a, b) in [
            (Point::new(0., 0.), Point::new(2., 0.)),
            (Point::new(2., 0.), Point::new(2., 2.)),
            (Point::new(0., 2.), Point::new(2., 2.)),
            (Point::new(0., 0.), Point::new(0., 2.)),
        ] {
            p.add(
                0,
                Material::Concrete,
                Shape::Polyline {
                    points: vec![a, b],
                    arcs: vec![],
                },
            );
        }
        let chain = trace(&p, 0, &[], Point::new(1., 0.), 0.05).unwrap();
        assert_eq!(chain.len(), 5);
        assert_eq!(chain.first(), chain.last());
    }
}
