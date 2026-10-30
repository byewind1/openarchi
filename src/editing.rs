//! Atomic editing operations shared by menus and future pointer tools.
use crate::{
    editor::Editor,
    geometry,
    model::{Element, Point, Project, Shape, WallContour},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditState {
    /// Outer groups precede inner groups; each membership is an element id.
    pub groups: Vec<Vec<u64>>,
    pub locked: BTreeSet<u64>,
    pub suspend_groups: bool,
    pub autogroup: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EditCommand {
    Intersect,
    MergeWalls,
    Corner {
        node: usize,
        radius: f32,
        chamfer: bool,
    },
    Group,
    Ungroup,
    SuspendGroups,
    Autogroup,
    Lock,
    Unlock,
    UnlockAll,
    Order(i8), // -2 back, -1 backward, 0 default, 1 forward, 2 front
    Transform {
        offset: Point,
        origin: Point,
        angle: f32,
        scale: f32,
        mirror: bool,
        copies: usize,
    },
    Align {
        axis: usize,
        edge: i8,
    },
    Distribute {
        x: bool,
        y: bool,
    },
    Story {
        target: u32,
        preserve_elevation: bool,
    },
    Elevate(f32),
    InvertWall,
    WallReference(f32),
    Explode,
    Consolidate,
    Split {
        a: Point,
        b: Point,
    },
    Adjust {
        a: Point,
        b: Point,
    },
    Offset(f32),
    Unify,
}

pub fn bounds(project: &Project, element: &Element) -> Option<(Point, Point)> {
    let points = geometry::element_plan_outline(project, element);
    if points.is_empty() {
        return None;
    }
    let mut lo = points[0];
    let mut hi = lo;
    for p in points {
        lo.x = lo.x.min(p.x);
        lo.y = lo.y.min(p.y);
        hi.x = hi.x.max(p.x);
        hi.y = hi.y.max(p.y);
    }
    Some((lo, hi))
}
fn axis(p: Point, a: usize) -> f32 {
    if a == 0 { p.x } else { p.y }
}
fn shift(a: usize, v: f32) -> Point {
    if a == 0 {
        Point::new(v, 0.)
    } else {
        Point::new(0., v)
    }
}
fn cross(a: Point, b: Point) -> f32 {
    a.x * b.y - a.y * b.x
}
fn sub(a: Point, b: Point) -> Point {
    Point::new(a.x - b.x, a.y - b.y)
}
fn lerp(a: Point, b: Point, t: f32) -> Point {
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}
fn intersection(a: Point, b: Point, c: Point, d: Point) -> Option<f32> {
    let v = sub(b, a);
    let w = sub(d, c);
    let den = cross(v, w);
    (den.abs() > 1e-6).then(|| cross(sub(c, a), w) / den)
}
fn segment(e: &Element) -> Option<(Point, Point)> {
    match &e.shape {
        Shape::Wall { start, end, .. } if e.wall_contour.is_none() => Some((*start, *end)),
        Shape::Polyline { points, arcs }
            if points.len() == 2 && arcs.iter().all(Option::is_none) =>
        {
            Some((points[0], points[1]))
        }
        _ => None,
    }
}
fn set_segment(e: &mut Element, a: Point, b: Point) {
    match &mut e.shape {
        Shape::Wall { start, end, .. } => {
            *start = a;
            *end = b;
        }
        Shape::Polyline { points, .. } => {
            *points = vec![a, b];
        }
        _ => {}
    }
}
fn transform(
    e: &mut Element,
    origin: Point,
    angle: f32,
    scale: f32,
    mirror: bool,
    offset: Point,
) -> Result<(), String> {
    let rad = angle.to_radians();
    let (sin, cos) = rad.sin_cos();
    if matches!(e.shape, Shape::Slab { .. })
        && e.slab_contour.is_none()
        && sin.abs() > 1e-5
        && cos.abs() > 1e-5
    {
        e.slab_contour = Some(crate::slab_geometry::contour(e));
    }
    let f = |p: Point| {
        let x = (p.x - origin.x) * scale * if mirror { -1. } else { 1. };
        let y = (p.y - origin.y) * scale;
        Point::new(
            origin.x + x * cos - y * sin + offset.x,
            origin.y + x * sin + y * cos + offset.y,
        )
    };
    crate::wall_geometry::transform(e, origin, scale, angle, mirror, offset);
    if e.slab_contour.is_some() {
        crate::slab_geometry::transform(e, f, scale, mirror);
        return Ok(());
    }
    match &mut e.shape {
        Shape::Wall {
            start,
            end,
            thickness,
            ..
        } => {
            *start = f(*start);
            *end = f(*end);
            if mirror {
                let settings = e.wall_settings.get_or_insert_with(Default::default);
                settings.geometry.reference_offset = -settings.body_offset(*thickness);
                settings.geometry.reference_line = crate::wall::WallReferenceLine::Center;
            }
        }
        Shape::Slab { min, max, .. } => {
            let a = f(*min);
            let b = f(*max);
            *min = Point::new(a.x.min(b.x), a.y.min(b.y));
            *max = Point::new(a.x.max(b.x), a.y.max(b.y));
        }
        Shape::Stair { spec } => {
            spec.start = f(spec.start);
            spec.end = f(spec.end);
            spec.width *= scale;
        }
        Shape::Polyline { points, arcs } => {
            for p in points {
                *p = f(*p);
            }
            for arc in arcs.iter_mut().flatten() {
                arc.radius *= scale;
                if mirror {
                    arc.clockwise = !arc.clockwise;
                }
            }
        }
        Shape::Opening {
            offset: along,
            width,
            ..
        } => {
            *along *= scale;
            *width *= scale;
        }
    }
    Ok(())
}

impl Editor {
    pub fn expanded_selection(&self, mut ids: Vec<u64>) -> Vec<u64> {
        if !self.project.edit_state.suspend_groups {
            loop {
                let n = ids.len();
                for group in &self.project.edit_state.groups {
                    if group.iter().any(|id| ids.contains(id)) {
                        ids.extend(group);
                    }
                }
                ids.sort_unstable();
                ids.dedup();
                if n == ids.len() {
                    break;
                }
            }
        }
        ids.retain(|id| self.project.elements.iter().any(|e| e.id == *id));
        ids
    }
    pub fn edit(&mut self, command: EditCommand) -> Result<usize, String> {
        let ids = self.selection_ids();
        if ids.is_empty()
            && !matches!(
                command,
                EditCommand::UnlockAll | EditCommand::SuspendGroups | EditCommand::Autogroup
            )
        {
            return Err("Select elements first".into());
        }
        let mut affected = ids.clone();
        // Hosted elements follow their selected wall in every geometry operation.
        for e in &self.project.elements {
            if let Shape::Opening { host_wall, .. } = e.shape {
                if ids.contains(&host_wall) {
                    affected.push(e.id);
                }
            }
        }
        affected.sort_unstable();
        affected.dedup();
        if !matches!(
            command,
            EditCommand::Unlock
                | EditCommand::UnlockAll
                | EditCommand::SuspendGroups
                | EditCommand::Autogroup
        ) && affected.iter().any(|id| self.locked_element(*id))
        {
            return Err("Selection contains locked elements".into());
        }
        let mut candidate = self.project.clone();
        let result = apply_edit(&mut candidate, &affected, &command)?;
        candidate.validate()?;
        let before = std::mem::replace(&mut self.project, candidate);
        self.commit(before);
        if let Some(ids) = result {
            self.set_selection(ids);
        }
        self.last_edit = Some(command);
        self.status = "Edit applied".into();
        Ok(affected.len())
    }
}
pub(crate) fn apply_edit(
    p: &mut Project,
    ids: &[u64],
    command: &EditCommand,
) -> Result<Option<Vec<u64>>, String> {
    let selected = |e: &&Element| ids.contains(&e.id);
    match *command {
        EditCommand::Intersect => {
            if ids.len() != 2 {
                return Err("Select exactly two straight elements".into());
            }
            let i = p
                .elements
                .iter()
                .position(|e| e.id == ids[0])
                .ok_or("Select elements first")?;
            let j = p
                .elements
                .iter()
                .position(|e| e.id == ids[1])
                .ok_or("Select elements first")?;
            let (a, b) =
                segment(&p.elements[i]).ok_or("Select straight walls or two-node lines")?;
            let (c, d) =
                segment(&p.elements[j]).ok_or("Select straight walls or two-node lines")?;
            let t = intersection(a, b, c, d).ok_or("Parallel elements cannot intersect")?;
            let u = intersection(c, d, a, b).ok_or("Parallel elements cannot intersect")?;
            if p.elements
                .iter()
                .any(|e| matches!(e.shape,Shape::Opening{host_wall,..} if ids.contains(&host_wall)))
            {
                return Err("Reshape walls without openings first".into());
            }
            let hit = lerp(a, b, t);
            if t < 0.5 {
                set_segment(&mut p.elements[i], hit, b);
            } else {
                set_segment(&mut p.elements[i], a, hit);
            }
            if u < 0.5 {
                set_segment(&mut p.elements[j], hit, d);
            } else {
                set_segment(&mut p.elements[j], c, hit);
            }
        }
        EditCommand::MergeWalls => {
            if ids.len() != 2 {
                return Err("Select exactly two straight elements".into());
            }
            let mut walls: Vec<_> = p.elements.iter().filter(selected).cloned().collect();
            let mut first = walls.remove(0);
            let second = walls.remove(0);
            if !matches!(first.shape, Shape::Wall { .. })
                || !matches!(second.shape, Shape::Wall { .. })
            {
                return Err("Select walls first".into());
            }
            let (a, b) = segment(&first).unwrap();
            let (c, d) = segment(&second).unwrap();
            let mut comparable = second.clone();
            comparable.id = first.id;
            comparable.name = first.name.clone();
            set_segment(&mut comparable, a, b);
            if comparable != first {
                return Err("Selected elements must share attributes".into());
            }
            if cross(sub(b, a), sub(d, c)).abs() > 1e-5 || cross(sub(b, a), sub(c, a)).abs() > 1e-5
            {
                return Err("Walls must be collinear and touching".into());
            }
            let (start, end) = if b.distance(c) < 0.001 {
                (a, d)
            } else if a.distance(d) < 0.001 {
                (c, b)
            } else {
                return Err("Walls must be collinear and touching".into());
            };
            if sub(b, a).x * sub(d, c).x + sub(b, a).y * sub(d, c).y <= 0. {
                return Err("Walls must share their reference direction".into());
            }
            if p.elements
                .iter()
                .any(|e| matches!(e.shape,Shape::Opening{host_wall,..} if ids.contains(&host_wall)))
            {
                return Err("Reshape walls without openings first".into());
            }
            set_segment(&mut first, start, end);
            let id = first.id;
            p.elements.retain(|e| !ids.contains(&e.id));
            p.elements.push(first);
            return Ok(Some(vec![id]));
        }
        EditCommand::Corner {
            node,
            radius,
            chamfer,
        } => {
            if !radius.is_finite() || radius <= 0. {
                return Err("Invalid edit parameters".into());
            }
            for e in p.elements.iter_mut().filter(|e| ids.contains(&e.id)) {
                let Shape::Polyline { points, arcs } = &mut e.shape else {
                    return Err("Select straight polylines".into());
                };
                if node == 0 || node + 1 >= points.len() {
                    return Err("Choose an interior node index".into());
                }
                if arcs.get(node - 1).is_some_and(Option::is_some)
                    || arcs.get(node).is_some_and(Option::is_some)
                {
                    return Err("Select straight polylines".into());
                }
                let b = points[node];
                let u = sub(points[node - 1], b);
                let v = sub(points[node + 1], b);
                let la = u.x.hypot(u.y);
                let lb = v.x.hypot(v.y);
                let angle = ((u.x * v.x + u.y * v.y) / (la * lb)).clamp(-1., 1.).acos();
                if angle < 0.01 || (std::f32::consts::PI - angle) < 0.01 {
                    return Err("Parallel elements cannot intersect".into());
                }
                let distance = if chamfer {
                    radius
                } else {
                    radius / (angle * 0.5).tan()
                };
                if distance >= la - 0.01 || distance >= lb - 0.01 {
                    return Err("Corner radius exceeds adjacent segments".into());
                }
                let p1 = Point::new(b.x + u.x / la * distance, b.y + u.y / la * distance);
                let p2 = Point::new(b.x + v.x / lb * distance, b.y + v.y / lb * distance);
                if arcs.is_empty() {
                    *arcs = vec![None; points.len() - 1];
                }
                points.splice(node..=node, [p1, p2]);
                let arc = if chamfer {
                    None
                } else {
                    Some(crate::model::ArcSegment {
                        radius,
                        clockwise: cross(u, v) > 0.,
                        large_arc: false,
                    })
                };
                arcs.splice(node - 1..=node, [None, arc, None]);
            }
        }
        EditCommand::Group => {
            if ids.len() < 2 {
                return Err("Select at least two elements".into());
            }
            if p.elements.iter().filter(selected).any(
                |e| matches!(e.shape,Shape::Opening{host_wall,..} if !ids.contains(&host_wall)),
            ) {
                return Err("Select the host wall with its openings".into());
            }
            let mut group = ids.to_vec();
            group.sort_unstable();
            if !p.edit_state.groups.contains(&group) {
                p.edit_state.groups.insert(0, group);
            }
        }
        EditCommand::Ungroup => {
            if p.edit_state.suspend_groups {
                for g in &mut p.edit_state.groups {
                    g.retain(|id| !ids.contains(id));
                }
                p.edit_state.groups.retain(|g| g.len() > 1);
            } else {
                let outer: Vec<_> = p
                    .edit_state
                    .groups
                    .iter()
                    .enumerate()
                    .filter(|(_, g)| g.iter().any(|id| ids.contains(id)))
                    .filter(|(i, g)| {
                        !p.edit_state.groups[..*i]
                            .iter()
                            .any(|parent| g.iter().all(|id| parent.contains(id)))
                    })
                    .map(|(i, _)| i)
                    .collect();
                p.edit_state.groups = std::mem::take(&mut p.edit_state.groups)
                    .into_iter()
                    .enumerate()
                    .filter(|(i, _)| !outer.contains(i))
                    .map(|(_, g)| g)
                    .collect();
            }
        }
        EditCommand::SuspendGroups => p.edit_state.suspend_groups = !p.edit_state.suspend_groups,
        EditCommand::Autogroup => p.edit_state.autogroup = !p.edit_state.autogroup,
        EditCommand::Lock => p.edit_state.locked.extend(ids),
        EditCommand::Unlock => p.edit_state.locked.retain(|id| !ids.contains(id)),
        EditCommand::UnlockAll => p.edit_state.locked.clear(),
        EditCommand::Order(order) => match order {
            0 => p.elements.sort_by_key(|e| e.id),
            2 => p.elements.sort_by_key(|e| ids.contains(&e.id)),
            -2 => p.elements.sort_by_key(|e| !ids.contains(&e.id)),
            1 => {
                for i in (0..p.elements.len().saturating_sub(1)).rev() {
                    if ids.contains(&p.elements[i].id) && !ids.contains(&p.elements[i + 1].id) {
                        p.elements.swap(i, i + 1);
                    }
                }
            }
            -1 => {
                for i in 1..p.elements.len() {
                    if ids.contains(&p.elements[i].id) && !ids.contains(&p.elements[i - 1].id) {
                        p.elements.swap(i, i - 1);
                    }
                }
            }
            _ => return Err("Invalid edit parameters".into()),
        },
        EditCommand::Transform {
            offset,
            origin,
            angle,
            scale,
            mirror,
            copies,
        } => {
            if !offset.finite()
                || !origin.finite()
                || !angle.is_finite()
                || !scale.is_finite()
                || scale <= 0.
                || copies > 1000
            {
                return Err("Invalid edit parameters".into());
            }
            let originals: Vec<_> = p.elements.iter().filter(selected).cloned().collect();
            if originals.iter().any(
                |e| matches!(e.shape,Shape::Opening{host_wall,..} if !ids.contains(&host_wall)),
            ) {
                return Err("Select the host wall with its openings".into());
            }
            if p.elements
                .len()
                .saturating_add(originals.len().saturating_mul(copies))
                > 100_000
            {
                return Err("Invalid project size".into());
            }
            if copies == 0 {
                for e in p.elements.iter_mut().filter(|e| ids.contains(&e.id)) {
                    transform(e, origin, angle, scale, mirror, offset)?;
                }
            } else {
                let mut added = Vec::new();
                let groups = p.edit_state.groups.clone();
                for n in 1..=copies {
                    let mut map = BTreeMap::new();
                    for e in &originals {
                        map.insert(e.id, p.next_id);
                        p.next_id += 1;
                    }
                    for e in &originals {
                        let mut copy = e.clone();
                        copy.id = map[&e.id];
                        if let Shape::Opening { host_wall, .. } = &mut copy.shape {
                            *host_wall = map[host_wall];
                        }
                        transform(
                            &mut copy,
                            origin,
                            angle * n as f32,
                            scale.powi(n as i32),
                            mirror && n % 2 == 1,
                            Point::new(offset.x * n as f32, offset.y * n as f32),
                        )?;
                        added.push(copy.id);
                        p.elements.push(copy);
                    }
                    for g in &groups {
                        let remapped: Vec<_> =
                            g.iter().filter_map(|id| map.get(id).copied()).collect();
                        if remapped.len() > 1 {
                            p.edit_state.groups.push(remapped);
                        }
                    }
                }
                return Ok(Some(added));
            }
        }
        EditCommand::Align { axis: a, edge } => {
            if a > 1 || !(-1..=1).contains(&edge) {
                return Err("Invalid edit parameters".into());
            }
            let items: Vec<_> = p
                .elements
                .iter()
                .filter(selected)
                .filter(|e| !matches!(e.shape, Shape::Opening { .. }))
                .filter_map(|e| bounds(p, e).map(|b| (e.id, b)))
                .collect();
            if items.len() < 2 {
                return Err("Select at least two elements".into());
            }
            let low = items
                .iter()
                .map(|(_, b)| axis(b.0, a))
                .fold(f32::INFINITY, f32::min);
            let high = items
                .iter()
                .map(|(_, b)| axis(b.1, a))
                .fold(f32::NEG_INFINITY, f32::max);
            let target = if edge < 0 {
                low
            } else if edge > 0 {
                high
            } else {
                (low + high) / 2.
            };
            for (id, (lo, hi)) in items {
                let at = if edge < 0 {
                    axis(lo, a)
                } else if edge > 0 {
                    axis(hi, a)
                } else {
                    (axis(lo, a) + axis(hi, a)) / 2.
                };
                p.translate_many(&[id], shift(a, target - at));
            }
        }
        EditCommand::Distribute { x, y } => {
            for a in 0..2 {
                if !(if a == 0 { x } else { y }) {
                    continue;
                }
                let mut items: Vec<_> = p
                    .elements
                    .iter()
                    .filter(selected)
                    .filter(|e| !matches!(e.shape, Shape::Opening { .. }))
                    .filter_map(|e| {
                        bounds(p, e).map(|(lo, hi)| (e.id, (axis(lo, a) + axis(hi, a)) / 2.))
                    })
                    .collect();
                if items.len() < 3 {
                    return Err("Select at least three elements".into());
                }
                items.sort_by(|a, b| a.1.total_cmp(&b.1));
                let first = items[0].1;
                let step = (items.last().unwrap().1 - first) / (items.len() - 1) as f32;
                for (i, (id, at)) in items.into_iter().enumerate() {
                    p.translate_many(&[id], shift(a, first + i as f32 * step - at));
                }
            }
        }
        EditCommand::Story {
            target,
            preserve_elevation,
        } => {
            if !p.stories.iter().any(|s| s.id == target) {
                return Err("Invalid story".into());
            }
            let elevations: BTreeMap<_, _> =
                p.stories.iter().map(|s| (s.id, s.elevation)).collect();
            for e in p.elements.iter_mut().filter(|e| ids.contains(&e.id)) {
                if preserve_elevation {
                    let dz = elevations[&e.story] - elevations[&target];
                    if dz != 0. {
                        elevate(e, dz)?;
                    }
                }
                if matches!(e.shape,Shape::Opening{host_wall,..} if !ids.contains(&host_wall)) {
                    return Err("Select the host wall with its openings".into());
                }
                e.story = target;
            }
        }
        EditCommand::Elevate(dz) => {
            if !dz.is_finite() {
                return Err("Invalid edit parameters".into());
            }
            for e in p.elements.iter_mut().filter(|e| ids.contains(&e.id)) {
                elevate(e, dz)?;
            }
        }
        EditCommand::InvertWall => {
            let lengths: BTreeMap<_, _> = p
                .elements
                .iter()
                .filter(selected)
                .filter_map(|e| {
                    if let Shape::Wall { .. } = e.shape {
                        Some((e.id, crate::wall_geometry::reference_length(e)))
                    } else {
                        None
                    }
                })
                .collect();
            if lengths.is_empty() {
                return Err("Select walls first".into());
            }
            for e in &mut p.elements {
                if lengths.contains_key(&e.id) {
                    let thickness = e.thickness();
                    let settings = e.wall_settings.get_or_insert_with(Default::default);
                    settings.geometry.reference_offset = -settings.body_offset(thickness);
                    settings.geometry.reference_line = crate::wall::WallReferenceLine::Center;
                    if let Shape::Wall { start, end, .. } = &mut e.shape {
                        std::mem::swap(start, end);
                    }
                    match e.wall_contour.as_mut() {
                        Some(WallContour::Arc { arc }) => arc.clockwise = !arc.clockwise,
                        Some(WallContour::Trapezoid {
                            outer_start,
                            outer_end,
                            outer_nodes,
                            outer_positive,
                        }) => {
                            std::mem::swap(outer_start, outer_end);
                            outer_nodes.reverse();
                            *outer_positive = !*outer_positive;
                        }
                        None => {}
                    }
                }
                if let Shape::Opening {
                    host_wall, offset, ..
                } = &mut e.shape
                {
                    if let Some(len) = lengths.get(host_wall) {
                        *offset = *len - *offset;
                    }
                }
            }
        }
        EditCommand::WallReference(value) => {
            if !value.is_finite() {
                return Err("Invalid edit parameters".into());
            }
            for e in p.elements.iter_mut().filter(|e| ids.contains(&e.id)) {
                if e.wall_contour.is_some() {
                    return Err("Reference line changes require a straight wall".into());
                }
                let Shape::Wall {
                    start,
                    end,
                    thickness,
                    ..
                } = &mut e.shape
                else {
                    return Err("Select walls first".into());
                };
                let s = e.wall_settings.get_or_insert_with(Default::default);
                let old = s.body_offset(*thickness);
                let len = start.distance(*end);
                let d = Point::new(
                    -(end.y - start.y) / len * (old - value),
                    (end.x - start.x) / len * (old - value),
                );
                start.x += d.x;
                start.y += d.y;
                end.x += d.x;
                end.y += d.y;
                s.geometry.reference_line = crate::wall::WallReferenceLine::Center;
                s.geometry.reference_offset = value;
            }
        }
        EditCommand::Explode => {
            let originals: Vec<_> = p.elements.iter().filter(selected).cloned().collect();
            let mut added = Vec::new();
            for e in originals {
                if e.wall_contour.is_some() {
                    return Err("Explode requires a straight wall".into());
                }
                let (points, arcs) = match &e.shape {
                    Shape::Polyline { points, arcs } => (points.clone(), arcs.clone()),
                    Shape::Opening { .. } => return Err("Select independent elements".into()),
                    _ => {
                        let mut pts = geometry::element_footprint(p, &e).to_vec();
                        if let Some(first) = pts.first().copied() {
                            pts.push(first);
                        }
                        (pts, vec![])
                    }
                };
                for (i, pair) in points.windows(2).enumerate() {
                    let mut line = e.clone();
                    line.id = p.next_id;
                    p.next_id += 1;
                    line.wall_settings = None;
                    line.shape = Shape::Polyline {
                        points: pair.to_vec(),
                        arcs: vec![arcs.get(i).copied().flatten()],
                    };
                    added.push(line.id);
                    p.elements.push(line);
                }
            }
            p.elements.retain(|e| !ids.contains(&e.id));
            if p.edit_state.autogroup && added.len() > 1 {
                p.edit_state.groups.insert(0, added.clone());
            }
            return Ok(Some(added));
        }
        EditCommand::Consolidate => {
            let mut seen: Vec<Element> = Vec::new();
            let mut remove = Vec::new();
            for e in p.elements.iter().filter(selected) {
                if !matches!(e.shape, Shape::Polyline { .. }) {
                    return Err("Select straight polylines".into());
                }
                if seen.iter().any(|other| {
                    e.shape == other.shape
                        && e.story == other.story
                        && e.layer == other.layer
                        && e.material == other.material
                }) {
                    remove.push(e.id);
                } else {
                    seen.push(e.clone());
                }
            }
            p.elements.retain(|e| !remove.contains(&e.id));
        }
        EditCommand::Split { a, b } | EditCommand::Adjust { a, b } => {
            if !a.finite() || !b.finite() || a.distance(b) < 0.001 {
                return Err("Invalid edit parameters".into());
            }
            let split = matches!(command, EditCommand::Split { .. });
            let mut added = Vec::new();
            for id in ids {
                let Some(index) = p.elements.iter().position(|e| e.id == *id) else {
                    continue;
                };
                let (start, end) =
                    segment(&p.elements[index]).ok_or("Select straight walls or two-node lines")?;
                if p.elements
                    .iter()
                    .any(|e| matches!(e.shape,Shape::Opening{host_wall,..} if host_wall==*id))
                {
                    return Err("Reshape walls without openings first".into());
                }
                let Some(t) = intersection(start, end, a, b) else {
                    continue;
                };
                let hit = lerp(start, end, t);
                if split {
                    if !(0.0001..0.9999).contains(&t) {
                        continue;
                    }
                    let mut other = p.elements[index].clone();
                    other.id = p.next_id;
                    p.next_id += 1;
                    set_segment(&mut other, hit, end);
                    set_segment(&mut p.elements[index], start, hit);
                    added.push(other.id);
                    p.elements.push(other);
                } else if t < 0.5 {
                    set_segment(&mut p.elements[index], hit, end);
                } else {
                    set_segment(&mut p.elements[index], start, hit);
                }
            }
            if split {
                added.extend_from_slice(ids);
                return Ok(Some(added));
            }
        }
        EditCommand::Offset(distance) => {
            if !distance.is_finite() {
                return Err("Invalid edit parameters".into());
            }
            for e in p.elements.iter_mut().filter(|e| ids.contains(&e.id)) {
                if let Shape::Polyline { points, arcs } = &mut e.shape {
                    if distance.abs() < 1e-6 {
                        continue;
                    }
                    let (new_points, new_arcs) =
                        geometry::offset_polyline_with_arcs(points, arcs, distance)
                            .ok_or("Invalid edit parameters")?;
                    *points = new_points;
                    *arcs = new_arcs;
                    continue;
                }
                let (a, b) = segment(e).ok_or("Select straight walls or two-node lines")?;
                let len = a.distance(b);
                e.translate(Point::new(
                    -(b.y - a.y) / len * distance,
                    (b.x - a.x) / len * distance,
                ));
            }
        }
        EditCommand::Unify => {
            if ids.len() < 2 {
                return Err("Select at least two elements".into());
            }
            let mut originals: Vec<_> = p.elements.iter().filter(selected).cloned().collect();
            let mut first = originals.remove(0);
            let mut points = match &first.shape {
                Shape::Polyline { points, arcs } if arcs.iter().all(Option::is_none) => {
                    points.clone()
                }
                _ => return Err("Select straight polylines".into()),
            };
            while !originals.is_empty() {
                let mut found = None;
                for (i, e) in originals.iter().enumerate() {
                    if e.story != first.story
                        || e.layer != first.layer
                        || e.material != first.material
                    {
                        return Err("Selected elements must share attributes".into());
                    }
                    let Shape::Polyline { points: ps, arcs } = &e.shape else {
                        return Err("Select straight polylines".into());
                    };
                    if arcs.iter().any(Option::is_some) {
                        return Err("Select straight polylines".into());
                    }
                    let end = *points.last().unwrap();
                    let start = points[0];
                    if end.distance(ps[0]) < 0.001 {
                        points.extend_from_slice(&ps[1..]);
                        found = Some(i);
                        break;
                    }
                    if end.distance(*ps.last().unwrap()) < 0.001 {
                        points.extend(ps.iter().rev().skip(1));
                        found = Some(i);
                        break;
                    }
                    if start.distance(*ps.last().unwrap()) < 0.001 {
                        let mut joined = ps[..ps.len() - 1].to_vec();
                        joined.extend(points);
                        points = joined;
                        found = Some(i);
                        break;
                    }
                    if start.distance(ps[0]) < 0.001 {
                        let mut joined: Vec<_> = ps.iter().rev().copied().collect();
                        joined.pop();
                        joined.extend(points);
                        points = joined;
                        found = Some(i);
                        break;
                    }
                }
                if let Some(i) = found {
                    originals.remove(i);
                } else {
                    return Err("Selected lines must form a connected chain".into());
                }
            }
            first.shape = Shape::Polyline {
                points,
                arcs: vec![],
            };
            p.elements.retain(|e| !ids.contains(&e.id));
            let id = first.id;
            p.elements.push(first);
            return Ok(Some(vec![id]));
        }
    }
    Ok(None)
}
fn elevate(e: &mut Element, dz: f32) -> Result<(), String> {
    if matches!(e.shape, Shape::Slab { .. }) {
        let mut c = crate::slab_geometry::contour(e);
        c.elevation += dz;
        return crate::slab_geometry::sync(e, c);
    }
    match &mut e.shape {
        Shape::Wall { .. } => {
            let geometry = &mut e
                .wall_settings
                .get_or_insert_with(Default::default)
                .geometry;
            geometry.bottom_offset += dz;
            geometry.top_offset += dz;
        }
        Shape::Opening { sill, .. } => *sill += dz,
        _ => return Err("Elevation currently supports walls and openings".into()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ArcSegment, Material};
    fn scene() -> Editor {
        let mut p = Project::default();
        for x in [0., 3., 10.] {
            p.add(
                0,
                Material::Plaster,
                Shape::Wall {
                    start: Point::new(x, 0.),
                    end: Point::new(x + 2., 0.),
                    thickness: 0.2,
                    height: 3.,
                },
            );
        }
        let mut e = Editor::new(p);
        e.set_selection(vec![1, 2, 3]);
        e
    }
    fn move_by(x: f32, copies: usize) -> EditCommand {
        EditCommand::Transform {
            offset: Point::new(x, 0.),
            origin: Point::default(),
            angle: 0.,
            scale: 1.,
            mirror: false,
            copies,
        }
    }
    #[test]
    fn editing_group_nested_suspend_and_ungroup_roundtrip() {
        let mut e = scene();
        e.set_selection(vec![1, 2]);
        e.edit(EditCommand::Group).unwrap();
        e.set_selection(vec![1]);
        assert_eq!(e.selection_ids(), vec![1, 2]);
        e.set_selection(vec![1, 3]);
        e.edit(EditCommand::Group).unwrap();
        e.set_selection(vec![3]);
        assert_eq!(e.selection_ids(), vec![1, 2, 3]);
        e.edit(EditCommand::Ungroup).unwrap();
        e.set_selection(vec![1]);
        assert_eq!(e.selection_ids(), vec![1, 2]);
        e.edit(EditCommand::SuspendGroups).unwrap();
        e.set_selection(vec![1]);
        assert_eq!(e.selection_ids(), vec![1]);
        let after = e.project.clone();
        e.undo();
        e.redo();
        assert_eq!(e.project, after);
        let json = serde_json::to_string(&after).unwrap();
        assert_eq!(serde_json::from_str::<Project>(&json).unwrap(), after);
    }
    #[test]
    fn editing_locked_batch_rejected_atomically() {
        let mut e = scene();
        e.set_selection(vec![2]);
        e.edit(EditCommand::Lock).unwrap();
        e.set_selection(vec![1, 2]);
        let before = e.project.clone();
        let n = e.undo.len();
        assert!(e.edit(move_by(5., 0)).is_err());
        assert_eq!(e.project, before);
        assert_eq!(e.undo.len(), n);
        e.delete();
        assert_eq!(e.project, before);
        e.edit(EditCommand::Unlock).unwrap();
        e.edit(move_by(5., 0)).unwrap();
    }
    #[test]
    fn editing_transform_is_one_undo_and_invalid_preserves_history() {
        let mut e = scene();
        let before = e.project.clone();
        e.edit(move_by(4., 3)).unwrap();
        assert_eq!(e.project.elements.len(), 12);
        assert_eq!(e.undo.len(), 1);
        let after = e.project.clone();
        e.undo();
        assert_eq!(e.project, before);
        e.redo();
        assert_eq!(e.project, after);
        e.set_selection(vec![1]);
        assert!(e.edit(move_by(f32::NAN, 0)).is_err());
        assert_eq!(e.project, after);
        assert_eq!(e.undo.len(), 1);
    }
    #[test]
    fn editing_copy_preserves_metadata_and_remaps_host() {
        let mut e = scene();
        e.project.elements[0].name = "Named wall".into();
        e.project.elements[0]
            .wall_settings
            .as_mut()
            .unwrap()
            .geometry
            .bottom_offset = 0.2;
        let host = e.project.add(
            0,
            Material::Plaster,
            Shape::Opening {
                host_wall: 1,
                kind: crate::model::OpeningKind::Door,
                width: 0.8,
                height: 2.,
                sill: 0.2,
                offset: 1.,
            },
        );
        e.set_selection(vec![1]);
        e.copy_selection();
        assert_eq!(e.clipboard.len(), 2);
        assert_eq!(e.paste(Point::new(4., 0.)), 2, "{}", e.status);
        let wall = &e.project.elements[4];
        assert_eq!(wall.name, "Named wall");
        assert_eq!(wall.wall_settings, e.project.elements[0].wall_settings);
        assert!(
            matches!(e.project.elements[5].shape,Shape::Opening{host_wall,..} if host_wall==wall.id)
        );
        assert!(e.project.elements.iter().any(|e| e.id == host));
        e.project.validate().unwrap();
    }
    #[test]
    fn editing_arc_radius_and_handedness_transform() {
        let mut p = Project::default();
        let id = p.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(0., 0.), Point::new(2., 0.)],
                arcs: vec![Some(ArcSegment {
                    radius: 2.,
                    clockwise: true,
                    large_arc: false,
                })],
            },
        );
        let mut e = Editor::new(p);
        e.set_selection(vec![id]);
        e.edit(EditCommand::Transform {
            offset: Point::default(),
            origin: Point::default(),
            angle: 90.,
            scale: 2.,
            mirror: true,
            copies: 0,
        })
        .unwrap();
        let Shape::Polyline { points, arcs } = &e.project.elements[0].shape else {
            panic!()
        };
        assert!((points[1].y + 4.).abs() < 1e-5);
        assert_eq!(arcs[0].unwrap().radius, 4.);
        assert!(!arcs[0].unwrap().clockwise);
    }
    #[test]
    fn editing_slab_rotation_does_not_inflate_geometry() {
        let mut p = Project::default();
        p.add(
            0,
            Material::Plaster,
            Shape::Slab {
                min: Point::default(),
                max: Point::new(4., 2.),
                thickness: 0.2,
            },
        );
        let mut e = Editor::new(p);
        e.set_selection(vec![1]);
        let before = e.project.clone();
        e.edit(EditCommand::Transform {
            offset: Point::default(),
            origin: Point::default(),
            angle: 45.,
            scale: 1.,
            mirror: false,
            copies: 0,
        })
        .unwrap();
        assert!((e.project.elements[0].area() - 8.).abs() < 1e-4);
        assert!(e.project.elements[0].slab_contour.is_some());
        assert_eq!(e.undo.len(), 1);
        e.undo();
        assert_eq!(e.project, before);
    }
    #[test]
    fn editing_distribute_and_align_have_measured_results() {
        let mut e = scene();
        e.edit(EditCommand::Distribute { x: true, y: false })
            .unwrap();
        let Shape::Wall { start, .. } = e.project.elements[1].shape else {
            panic!()
        };
        assert!((start.x - 5.).abs() < 1e-5);
        e.edit(EditCommand::Align { axis: 0, edge: -1 }).unwrap();
        for wall in &e.project.elements {
            let Shape::Wall { start, .. } = wall.shape else {
                panic!()
            };
            assert!(start.x.abs() < 1e-5);
        }
    }
    #[test]
    fn editing_split_adjust_and_unify_are_atomic() {
        let mut p = Project::default();
        p.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::default(), Point::new(4., 0.)],
                arcs: vec![],
            },
        );
        let mut e = Editor::new(p);
        e.set_selection(vec![1]);
        e.edit(EditCommand::Split {
            a: Point::new(2., -1.),
            b: Point::new(2., 1.),
        })
        .unwrap();
        assert_eq!(e.project.elements.len(), 2);
        e.edit(EditCommand::Unify).unwrap();
        assert_eq!(e.project.elements.len(), 1);
        e.edit(EditCommand::Adjust {
            a: Point::new(6., -1.),
            b: Point::new(6., 1.),
        })
        .unwrap_err();
        e.undo();
        assert_eq!(e.project.elements.len(), 2);
    }
    #[test]
    fn editing_order_undo_restores_exact_sequence() {
        let mut e = scene();
        e.set_selection(vec![1]);
        e.edit(EditCommand::Order(2)).unwrap();
        assert_eq!(
            e.project.elements.iter().map(|e| e.id).collect::<Vec<_>>(),
            vec![2, 3, 1]
        );
        e.undo();
        assert_eq!(
            e.project.elements.iter().map(|e| e.id).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }
    #[test]
    fn editing_invert_preserves_wall_body() {
        let mut e = scene();
        e.project.elements[0]
            .wall_settings
            .as_mut()
            .unwrap()
            .geometry
            .reference_offset = 0.1;
        let before = geometry::element_footprint(&e.project, &e.project.elements[0]);
        e.set_selection(vec![1]);
        e.edit(EditCommand::InvertWall).unwrap();
        let after = geometry::element_footprint(&e.project, &e.project.elements[0]);
        for p in before {
            assert!(after.iter().any(|q| p.distance(*q) < 1e-5));
        }
    }
    #[test]
    fn custom_wall_invert_preserves_exact_body_and_arc_opening_station() {
        for contour in [
            WallContour::Arc {
                arc: ArcSegment {
                    radius: 1.,
                    clockwise: true,
                    large_arc: false,
                },
            },
            WallContour::Trapezoid {
                outer_start: Point::new(0., 0.3),
                outer_end: Point::new(2., 0.6),
                outer_nodes: vec![Point::new(0.7, 0.8)],
                outer_positive: true,
            },
        ] {
            let mut editor = scene();
            editor.project.elements[0].wall_contour = Some(contour.clone());
            let length = crate::wall_geometry::reference_length(&editor.project.elements[0]);
            let offset = length - 0.5;
            let opening = editor.project.add(
                0,
                Material::Timber,
                Shape::Opening {
                    host_wall: 1,
                    kind: crate::model::OpeningKind::Window,
                    width: 0.5,
                    height: 1.,
                    sill: 1.,
                    offset,
                },
            );
            let before = editor.project.clone();
            let outline = crate::wall_geometry::outline(&editor.project.elements[0]);
            let opening_point =
                crate::wall_geometry::point_tangent_at(&editor.project.elements[0], offset)
                    .unwrap()
                    .0;
            editor.set_selection(vec![1]);
            editor.edit(EditCommand::InvertWall).unwrap();
            let wall = &editor.project.elements[0];
            let after = crate::wall_geometry::outline(wall);
            for p in outline {
                assert!(after.iter().any(|q| p.distance(*q) < 1e-4));
            }
            let Shape::Opening {
                offset: new_offset, ..
            } = editor
                .project
                .elements
                .iter()
                .find(|e| e.id == opening)
                .unwrap()
                .shape
            else {
                panic!()
            };
            assert!((new_offset - 0.5).abs() < 1e-5);
            assert!(
                crate::wall_geometry::point_tangent_at(wall, new_offset)
                    .unwrap()
                    .0
                    .distance(opening_point)
                    < 1e-5
            );
            match (&contour, wall.wall_contour.as_ref().unwrap()) {
                (WallContour::Arc { arc: a }, WallContour::Arc { arc: b }) => {
                    assert_ne!(a.clockwise, b.clockwise)
                }
                (
                    WallContour::Trapezoid {
                        outer_start: a,
                        outer_end: b,
                        outer_positive: c,
                        ..
                    },
                    WallContour::Trapezoid {
                        outer_start: x,
                        outer_end: y,
                        outer_positive: z,
                        ..
                    },
                ) => {
                    assert_eq!(a, y);
                    assert_eq!(b, x);
                    assert_ne!(c, z);
                }
                _ => panic!(),
            }
            let changed = editor.project.clone();
            let json = serde_json::to_string(&changed).unwrap();
            assert_eq!(changed, serde_json::from_str(&json).unwrap());
            editor.undo();
            assert_eq!(editor.project, before);
            editor.redo();
            assert_eq!(editor.project, changed);
        }
    }
    #[test]
    fn custom_wall_legacy_reference_and_explode_reject_without_mutation() {
        let mut editor = scene();
        editor.project.elements[0].wall_contour = Some(WallContour::Arc {
            arc: ArcSegment {
                radius: 1.,
                clockwise: true,
                large_arc: false,
            },
        });
        editor.set_selection(vec![1]);
        let before = editor.project.clone();
        for command in [EditCommand::WallReference(0.1), EditCommand::Explode] {
            assert!(editor.edit(command).is_err());
            assert_eq!(editor.project, before);
            assert!(editor.undo.is_empty());
        }
    }
}

#[cfg(test)]
mod reshape_tests {
    use super::*;
    use crate::model::Material;
    #[test]
    fn intersect_meets_both_reference_lines_and_undo_restores() {
        let mut p = Project::default();
        for (a, b) in [
            (Point::new(0., 0.), Point::new(1., 0.)),
            (Point::new(2., 1.), Point::new(2., 2.)),
        ] {
            p.add(
                0,
                Material::Plaster,
                Shape::Polyline {
                    points: vec![a, b],
                    arcs: vec![],
                },
            );
        }
        let before = p.clone();
        let mut e = Editor::new(p);
        e.set_selection(vec![1, 2]);
        e.edit(EditCommand::Intersect).unwrap();
        assert_eq!(
            segment(&e.project.elements[0]).unwrap().1,
            Point::new(2., 0.)
        );
        assert_eq!(
            segment(&e.project.elements[1]).unwrap().0,
            Point::new(2., 0.)
        );
        e.undo();
        assert_eq!(e.project, before);
    }
    #[test]
    fn merge_walls_requires_equal_properties_and_restores_both_ids() {
        let mut p = Project::default();
        for x in [0., 2.] {
            p.add(
                0,
                Material::Plaster,
                Shape::Wall {
                    start: Point::new(x, 0.),
                    end: Point::new(x + 2., 0.),
                    thickness: 0.2,
                    height: 3.,
                },
            );
        }
        let before = p.clone();
        let mut e = Editor::new(p);
        e.set_selection(vec![1, 2]);
        e.edit(EditCommand::MergeWalls).unwrap();
        assert_eq!(e.project.elements.len(), 1);
        assert_eq!(
            segment(&e.project.elements[0]).unwrap().1,
            Point::new(4., 0.)
        );
        e.undo();
        assert_eq!(e.project, before);
        e.set_selection(vec![1, 2]);
        e.project.elements[1].material = Material::Concrete;
        let before = e.project.clone();
        assert!(e.edit(EditCommand::MergeWalls).is_err());
        assert_eq!(e.project, before);
    }
    #[test]
    fn fillet_non_right_angle_is_tangent_and_invalid_radius_is_atomic() {
        let mut p = Project::default();
        p.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![
                    Point::new(3., 0.),
                    Point::default(),
                    Point::new(1.5, 2.598076),
                ],
                arcs: vec![],
            },
        );
        let mut e = Editor::new(p);
        e.set_selection(vec![1]);
        let before = e.project.clone();
        assert!(
            e.edit(EditCommand::Corner {
                node: 1,
                radius: 100.,
                chamfer: false
            })
            .is_err()
        );
        assert_eq!(e.project, before);
        e.edit(EditCommand::Corner {
            node: 1,
            radius: 0.5,
            chamfer: false,
        })
        .unwrap();
        let Shape::Polyline { points, arcs } = &e.project.elements[0].shape else {
            panic!()
        };
        assert!((points[1].x - 0.8660254).abs() < 1e-5);
        assert_eq!(arcs[1].unwrap().radius, 0.5);
        assert!(arcs[1].unwrap().clockwise);
        e.project.validate().unwrap();
    }
    #[test]
    fn elevate_moves_wall_and_opening_without_changing_wall_height() {
        let mut p = Project::default();
        p.add(
            0,
            Material::Plaster,
            Shape::Wall {
                start: Point::default(),
                end: Point::new(4., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        p.add(
            0,
            Material::Plaster,
            Shape::Opening {
                host_wall: 1,
                kind: crate::model::OpeningKind::Door,
                width: 0.8,
                height: 2.,
                sill: 0.,
                offset: 1.,
            },
        );
        let mut e = Editor::new(p);
        e.set_selection(vec![1]);
        e.edit(EditCommand::Elevate(0.5)).unwrap();
        let g = &e.project.elements[0]
            .wall_settings
            .as_ref()
            .unwrap()
            .geometry;
        assert_eq!(g.bottom_offset, 0.5);
        assert_eq!(g.top_offset, 0.5);
        assert!(matches!(e.project.elements[1].shape,Shape::Opening{sill,..} if sill==0.5));
        e.project.validate().unwrap();
    }
}
