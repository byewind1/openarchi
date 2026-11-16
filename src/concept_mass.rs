//! Authored concept envelopes. These solids are neither habitable Zones nor construction material.
use crate::model::{ArcSegment, Element, Point, Shape, SlabContour};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConceptMassSpec {
    pub outline: Vec<Point>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arcs: Vec<Option<ArcSegment>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hole_arcs: Vec<Vec<Option<ArcSegment>>>,
    #[serde(default)]
    pub holes: Vec<Vec<Point>>,
    pub height: f64,
    #[serde(default)]
    pub bottom_offset: f64,
}
impl ConceptMassSpec {
    pub fn contour(&self) -> SlabContour {
        SlabContour {
            points: self.outline.clone(),
            arcs: self.arcs.clone(),
            holes: self
                .holes
                .iter()
                .enumerate()
                .map(|(i, p)| SlabContour {
                    points: p.clone(),
                    arcs: self.hole_arcs.get(i).cloned().unwrap_or_default(),
                    ..Default::default()
                })
                .collect(),
            elevation: self.bottom_offset + self.height,
            ..Default::default()
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if !self.height.is_finite()
            || !(0.01..=100.).contains(&self.height)
            || !self.bottom_offset.is_finite()
            || !(-100. ..=100.).contains(&self.bottom_offset)
        {
            return Err("Invalid concept mass parameters".into());
        }
        if !self.hole_arcs.is_empty() && self.hole_arcs.len() != self.holes.len() {
            return Err("Invalid concept mass parameters".into());
        }
        crate::slab_geometry::validate_contour(&self.contour())
    }
    pub fn area(&self) -> f64 {
        crate::slab_geometry::contour_area(&self.contour())
    }
    /// Display/analysis samples of the exact authored arcs, never editable vertices.
    pub fn paths(&self) -> Vec<Vec<Point>> {
        let c = self.contour();
        std::iter::once(&c)
            .chain(c.holes.iter())
            .map(crate::slab_geometry::ring_path)
            .collect()
    }

    pub fn volume(&self) -> f64 {
        self.area() * self.height
    }
    pub fn transform(&mut self, f: impl Fn(Point) -> Point) {
        let o = f(Point::new(0., 0.));
        let x = f(Point::new(1., 0.));
        let y = f(Point::new(0., 1.));
        let scale = o.distance(x);
        let mirror = (x.x - o.x) * (y.y - o.y) - (x.y - o.y) * (y.x - o.x) < 0.;
        for arc in self
            .arcs
            .iter_mut()
            .chain(self.hole_arcs.iter_mut().flatten())
            .flatten()
        {
            arc.radius *= scale;
            if mirror {
                arc.clockwise = !arc.clockwise;
            }
        }
        for p in self
            .outline
            .iter_mut()
            .chain(self.holes.iter_mut().flatten())
        {
            *p = f(*p);
        }
    }
}
pub fn derive(
    e: &Element,
    base: f64,
) -> Result<crate::slab_geometry::DerivedSlab, crate::derived_geometry::GeometryDiagnostic> {
    let error = |message| {
        crate::derived_geometry::GeometryDiagnostic::error(
            e.id,
            "INVALID_CONCEPT_MASS",
            "concept_mass",
            message,
        )
    };
    let Shape::ConceptMass { spec } = &e.shape else {
        return Err(error("Element is not a concept mass".into()));
    };
    spec.validate().map_err(error)?;
    // Reuse the checked prism kernel with a disposable input adapter, never change live shape identity.
    let mut input = e.clone();
    let (min, max) = crate::slab_geometry::contour_bounds(&spec.contour());
    input.shape = Shape::Slab {
        min,
        max,
        thickness: spec.height,
    };
    input.slab_contour = Some(spec.contour());
    let mut g = crate::slab_geometry::derive_space_prism(&input, base)?;
    g.identity = crate::derived_geometry::identity(e.id, spec, base, 1).map_err(error)?;
    Ok(g)
}
/// A single-region exact contour edit. The temporary slab is only an
/// adapter to the existing contour kernel, never a second live model object.
pub fn changed_footprint(
    element: &Element,
    index: usize,
    change: &crate::slab_geometry::SlabChange,
    anchor: Point,
    point: Point,
) -> Result<ConceptMassSpec, String> {
    use crate::slab_geometry::SlabChange as C;
    let Shape::ConceptMass { spec } = &element.shape else {
        return Err("Element is not a concept mass".into());
    };
    spec.validate()?;
    if matches!(change, C::SetEdge { .. }) {
        return Err("Concept masses have vertical envelope edges".into());
    }
    let mut adapter = element.clone();
    let (min, max) = crate::slab_geometry::contour_bounds(&spec.contour());
    adapter.shape = Shape::Slab {
        min,
        max,
        thickness: spec.height,
    };
    adapter.slab_contour = Some(spec.contour());
    let mut regions = crate::slab_geometry::changed(&adapter, index, change, anchor, point)?;
    if regions.len() != 1 {
        return Err("Concept mass footprint must remain one connected region".into());
    }
    let contour = regions.remove(0);
    let compact = |a: Vec<Option<ArcSegment>>| {
        if a.iter().any(Option::is_some) {
            a
        } else {
            vec![]
        }
    };
    let mut hole_arcs: Vec<_> = contour
        .holes
        .iter()
        .map(|h| compact(h.arcs.clone()))
        .collect();
    if hole_arcs.iter().all(Vec::is_empty) {
        hole_arcs.clear();
    }
    let result = ConceptMassSpec {
        outline: contour.points,
        arcs: compact(contour.arcs),
        hole_arcs,
        holes: contour.holes.into_iter().map(|h| h.points).collect(),
        height: spec.height,
        bottom_offset: contour.elevation - spec.height,
    };
    result.validate()?;
    Ok(result)
}

/// An authored envelope face; side indices refer to exact edges, never samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MassFace {
    Top,
    Bottom,
    Side(usize),
}

pub fn face_normal(
    element: &Element,
    face: MassFace,
    at: glam::DVec3,
) -> Result<glam::DVec3, String> {
    match face {
        MassFace::Top => Ok(glam::DVec3::Z),
        MassFace::Bottom => Ok(-glam::DVec3::Z),
        MassFace::Side(index) => {
            let mut offset = 0;
            for (ring_index, ring) in crate::slab_geometry::rings(element).iter().enumerate() {
                if index < offset + ring.points.len() {
                    let n =
                        crate::slab_geometry::normal(ring, index - offset, Point::new(at.x, at.y))?;
                    let path = crate::slab_geometry::ring_path(ring);
                    let area: f64 = path
                        .iter()
                        .zip(path.iter().cycle().skip(1))
                        .take(path.len())
                        .map(|(a, b)| a.x * b.y - b.x * a.y)
                        .sum();
                    let sign = if ring_index == 0 {
                        -area.signum()
                    } else {
                        area.signum()
                    };
                    return Ok(glam::DVec3::new(n.x * sign, n.y * sign, 0.));
                }
                offset += ring.points.len();
            }
            Err("Invalid edit parameters".into())
        }
    }
}

/// Pure candidate builder used by pointer/numeric input and other callers.
pub fn pushed_face(
    element: &Element,
    face: MassFace,
    anchor: glam::DVec3,
    distance: f64,
) -> Result<Element, String> {
    let Shape::ConceptMass { spec } = &element.shape else {
        return Err("Element is not a concept mass".into());
    };
    if !distance.is_finite() {
        return Err("Invalid edit parameters".into());
    }
    if distance == 0. {
        spec.validate()?;
        face_normal(element, face, anchor)?;
        return Ok(element.clone());
    }
    let mut next = spec.clone();
    match face {
        MassFace::Top => next.height += distance,
        MassFace::Bottom => {
            next.bottom_offset -= distance;
            next.height += distance;
        }
        MassFace::Side(index) => {
            let n = face_normal(element, face, anchor)?;
            let a = Point::new(anchor.x, anchor.y);
            next = changed_footprint(
                element,
                index,
                &crate::slab_geometry::SlabChange::OffsetEdge,
                a,
                Point::new(a.x + n.x * distance, a.y + n.y * distance),
            )?;
        }
    }
    next.validate()?;
    let mut result = element.clone();
    result.shape = Shape::ConceptMass { spec: next };
    if matches!(face, MassFace::Side(_)) {
        let before = crate::slab_geometry::rings(element);
        let after = crate::slab_geometry::rings(&result);
        let winding = |r: &SlabContour| {
            let p = crate::slab_geometry::ring_path(r);
            p.iter()
                .zip(p.iter().cycle().skip(1))
                .take(p.len())
                .map(|(a, b)| a.x * b.y - b.x * a.y)
                .sum::<f64>()
                .signum()
        };
        if before.len() != after.len()
            || before
                .iter()
                .zip(&after)
                .any(|(a, b)| winding(a) != winding(b))
        {
            return Err("Offset cannot preserve a connected valid path at this distance".into());
        }
    }
    Ok(result)
}

/// Face triangles for picking/highlight from the same authored envelope as rendering.
pub fn face_triangles(
    element: &Element,
    base: f64,
) -> Result<Vec<(MassFace, [glam::DVec3; 3])>, String> {
    use glam::DVec3 as V;
    let Shape::ConceptMass { spec } = &element.shape else {
        return Err("Element is not a concept mass".into());
    };
    let mesh = derive(element, base).map_err(|e| e.message)?;
    let lo = base + spec.bottom_offset;
    let hi = lo + spec.height;
    let mut result: Vec<_> = mesh
        .caps
        .into_iter()
        .map(|t| {
            (
                if (t[0].z - hi).abs() < 1e-6 {
                    MassFace::Top
                } else {
                    MassFace::Bottom
                },
                t,
            )
        })
        .collect();
    let mut offset = 0;
    for ring in crate::slab_geometry::rings(element) {
        for (i, &a) in ring.points.iter().enumerate() {
            let b = ring.points[(i + 1) % ring.points.len()];
            let path = ring
                .arcs
                .get(i)
                .and_then(|a| *a)
                .and_then(|arc| crate::geometry::arc_points(a, b, arc))
                .unwrap_or_else(|| vec![a, b]);
            for pair in path.windows(2) {
                let [a, b] = [pair[0], pair[1]];
                let q = [
                    V::new(a.x, a.y, lo),
                    V::new(b.x, b.y, lo),
                    V::new(b.x, b.y, hi),
                    V::new(a.x, a.y, hi),
                ];
                result.push((MassFace::Side(offset + i), [q[0], q[1], q[2]]));
                result.push((MassFace::Side(offset + i), [q[0], q[2], q[3]]));
            }
        }
        offset += ring.points.len();
    }
    Ok(result)
}

impl crate::editor::Editor {
    pub fn place_concept_mass(&mut self, spec: ConceptMassSpec) -> Result<u64, String> {
        spec.validate()?;
        let e =
            self.creation_candidate(self.material, self.wall_layer, Shape::ConceptMass { spec });
        let id = e.id;
        self.apply(crate::command::ModelCommand::AddElement { element: e })?;
        self.set_selection(vec![id]);
        Ok(id)
    }
    pub fn finish_concept_mass(&mut self) -> Result<u64, String> {
        let id = self.place_concept_mass(ConceptMassSpec {
            arcs: vec![],
            hole_arcs: vec![],
            outline: self.polyline_points.clone(),
            holes: vec![],
            height: self.concept_mass_height,
            bottom_offset: self.concept_mass_bottom_offset,
        })?;
        self.polyline_points.clear();
        self.anchor = None;
        self.status = "Concept mass created".into();
        Ok(id)
    }
    pub(crate) fn click_concept_mass(&mut self, p: Point, tolerance: f64) {
        if self.zone_polygon {
            if self.polyline_points.len() >= 3 && self.polyline_points[0].distance(p) <= tolerance {
                if let Err(e) = self.finish_concept_mass() {
                    self.status = e;
                }
            } else {
                self.polyline_points.push(p);
            }
        } else if self.anchor.is_some() {
            let spec = ConceptMassSpec {
                arcs: vec![],
                hole_arcs: vec![],
                outline: self.zone_preview(p),
                holes: vec![],
                height: self.concept_mass_height,
                bottom_offset: self.concept_mass_bottom_offset,
            };
            match self.place_concept_mass(spec) {
                Ok(_) => {
                    self.anchor = None;
                    self.status = "Concept mass created".into();
                }
                Err(e) => self.status = e,
            }
        } else {
            self.anchor = Some(p);
        }
    }
}

#[cfg(test)]
mod footprint_tests {
    use super::*;
    use crate::{
        command::ModelCommand,
        editor::Editor,
        model::{Material, Project},
        reshape::ReshapeCommand,
        slab_geometry::SlabChange as C,
    };
    fn rectangle(x: f64, y: f64, w: f64, h: f64) -> Vec<Point> {
        vec![
            Point::new(x, y),
            Point::new(x + w, y),
            Point::new(x + w, y + h),
            Point::new(x, y + h),
        ]
    }
    fn fixture() -> (Editor, u64) {
        let mut p = Project::default();
        let id = p.add(
            0,
            Material::Plaster,
            Shape::ConceptMass {
                spec: ConceptMassSpec {
                    arcs: vec![],
                    hole_arcs: vec![],
                    outline: rectangle(0., 0., 8., 6.),
                    holes: vec![],
                    height: 6.,
                    bottom_offset: 1.25,
                },
            },
        );
        (Editor::new(p), id)
    }
    fn operation(id: u64, index: usize, change: C, anchor: Point, point: Point) -> ReshapeCommand {
        ReshapeCommand::MassFootprint {
            id,
            index,
            change,
            anchor,
            point,
        }
    }
    #[test]
    fn straight_mass_courtyard_typed_history_prism_and_inner_ring_edits() {
        let (mut e, id) = fixture();
        let before = e.project.clone();
        e.reshape(operation(
            id,
            0,
            C::SubtractPolygon(rectangle(2., 2., 2., 2.)),
            Point::new(0., 0.),
            Point::new(0., 0.),
        ))
        .unwrap();
        let changed = e.project.clone();
        let mass = &changed.elements[0];
        let Shape::ConceptMass { spec } = &mass.shape else {
            panic!()
        };
        assert_eq!(spec.holes.len(), 1);
        assert_eq!(spec.area(), 44.);
        assert_eq!(spec.volume(), 264.);
        assert_eq!(spec.bottom_offset, 1.25);
        assert!(mass.slab_contour.is_none());
        assert_eq!(mass.material, before.elements[0].material);
        assert!(matches!(
            e.undo.last().unwrap().command,
            ModelCommand::UpdateElement { .. }
        ));
        let g = derive(mass, 0.).unwrap();
        assert_eq!(g.plan_paths.len(), 2);
        assert!((g.volume_m3 - 264.).abs() < 1e-8);
        assert_eq!(g.bounds[0].z, 1.25);
        assert_eq!(g.bounds[1].z, 7.25);
        assert!(
            !g.plan_triangles
                .iter()
                .any(|tri| crate::geometry::point_in_polygon(Point::new(3., 3.), tri))
        );
        let outer = spec.outline.len();
        let node = spec.holes[0][0];
        let next = spec.holes[0][1];
        e.reshape(operation(
            id,
            outer,
            C::InsertNode,
            node,
            Point::new((node.x + next.x) * 0.5, (node.y + next.y) * 0.5),
        ))
        .unwrap();
        let Shape::ConceptMass { spec } = &e.project.elements[0].shape else {
            panic!()
        };
        assert_eq!(spec.holes[0].len(), 5);
        e.undo();
        assert_eq!(e.project, changed);
        e.undo();
        assert_eq!(e.project, before);
        e.redo();
        assert_eq!(e.project, changed);
        let encoded = serde_json::to_string(&e.project).unwrap();
        let restored: Project = serde_json::from_str(&encoded).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored, changed);
    }
    #[test]
    fn straight_mass_rejects_split_empty_unsupported_locked_and_invalid_without_history() {
        let (mut e, id) = fixture();
        let before = e.project.clone();
        for change in [
            C::SubtractPolygon(rectangle(3., -1., 2., 8.)),
            C::SubtractPolygon(rectangle(-1., -1., 10., 8.)),
            C::AddPolygon(rectangle(12., 0., 1., 1.)),
            C::SetEdge {
                angle_degrees: 80.,
                surface: None,
                scope: crate::slab_geometry::SlabEdgeScope::All,
            },
        ] {
            assert!(
                e.reshape(operation(
                    id,
                    0,
                    change,
                    Point::new(0., 0.),
                    Point::new(2., 1.)
                ))
                .is_err()
            );
            assert_eq!(e.project, before);
            assert!(e.undo.is_empty());
        }
        assert!(
            e.reshape(operation(
                id,
                100,
                C::MoveNode,
                Point::new(0., 0.),
                Point::new(2., 1.)
            ))
            .is_err()
        );
        assert_eq!(e.project, before);
        e.project.edit_state.locked.insert(id);
        let locked = e.project.clone();
        assert!(
            e.reshape(operation(
                id,
                0,
                C::SubtractPolygon(rectangle(2., 2., 2., 2.)),
                Point::new(0., 0.),
                Point::new(0., 0.)
            ))
            .is_err()
        );
        assert_eq!(e.project, locked);
        assert!(e.undo.is_empty());
    }
}

#[cfg(test)]
mod push_pull_tests {
    use super::*;
    use glam::DVec3 as V;
    #[test]
    fn mass_push_pull_faces_signed_candidates_exact_history_and_persistence() {
        let mut editor = crate::editor::Editor::new(crate::model::Project::default());
        let id = editor
            .place_concept_mass(ConceptMassSpec {
                outline: vec![
                    Point::new(0., 0.),
                    Point::new(8., 0.),
                    Point::new(8., 6.),
                    Point::new(0., 6.),
                ],
                holes: vec![vec![
                    Point::new(2., 2.),
                    Point::new(4., 2.),
                    Point::new(4., 4.),
                    Point::new(2., 4.),
                ]],
                height: 6.,
                bottom_offset: 1.25,
                ..Default::default()
            })
            .unwrap();
        let source = editor.project.elements[0].clone();
        let spec_of = |e: Element| {
            let Shape::ConceptMass { spec } = e.shape else {
                panic!()
            };
            spec
        };
        let top = spec_of(pushed_face(&source, MassFace::Top, V::ZERO, 2.).unwrap());
        assert_eq!((top.height, top.bottom_offset), (8., 1.25));
        let bottom = spec_of(pushed_face(&source, MassFace::Bottom, V::ZERO, -1.).unwrap());
        assert_eq!((bottom.height, bottom.bottom_offset), (5., 2.25));
        let side = pushed_face(&source, MassFace::Side(0), V::new(4., 0., 4.), 1.).unwrap();
        let side_spec = spec_of(side.clone());
        assert!(side_spec.outline.iter().any(|p| p.y == -1.));
        assert_eq!(side_spec.holes, top.holes);
        let hole =
            spec_of(pushed_face(&source, MassFace::Side(4), V::new(3., 2., 4.), 0.25).unwrap());
        assert!(hole.area() > top.area()); // Outward from solid fills part of the courtyard.
        assert!(pushed_face(&source, MassFace::Top, V::ZERO, -6.).is_err());
        assert!(pushed_face(&source, MassFace::Top, V::ZERO, f64::NAN).is_err());
        assert!(pushed_face(&source, MassFace::Side(99), V::ZERO, 1.).is_err());
        assert!(pushed_face(&source, MassFace::Side(0), V::new(4., 0., 3.), -7.).is_err());
        let triangles = face_triangles(&source, 0.).unwrap();
        assert!(triangles.iter().any(|(f, _)| *f == MassFace::Bottom));
        assert!(triangles.iter().any(|(f, _)| *f == MassFace::Side(4)));
        editor.undo.clear();
        editor
            .apply(crate::command::ModelCommand::UpdateElement {
                element: side.clone(),
            })
            .unwrap();
        assert_eq!(editor.undo.len(), 1);
        let saved: crate::model::Project =
            serde_json::from_str(&serde_json::to_string(&editor.project).unwrap()).unwrap();
        assert_eq!(saved.elements[0], side);
        saved.validate().unwrap();
        editor.undo();
        assert_eq!(editor.project.elements[0], source);
        editor.redo();
        assert_eq!(editor.project.elements[0], side);
        editor.project.edit_state.locked.insert(id);
        assert!(
            editor
                .apply(crate::command::ModelCommand::UpdateElement { element: source })
                .is_err()
        );
    }
    #[test]
    fn mass_push_pull_curved_side_preserves_exact_arc_and_other_faces() {
        let mut p = crate::model::Project::default();
        p.add(
            0,
            crate::model::Material::Plaster,
            Shape::ConceptMass {
                spec: ConceptMassSpec {
                    outline: vec![
                        Point::new(0., 0.),
                        Point::new(8., 0.),
                        Point::new(8., 6.),
                        Point::new(0., 6.),
                    ],
                    arcs: vec![
                        Some(ArcSegment {
                            radius: 5.,
                            clockwise: false,
                            large_arc: false,
                        }),
                        None,
                        None,
                        None,
                    ],
                    height: 6.,
                    ..Default::default()
                },
            },
        );
        let source = &p.elements[0];
        let next = pushed_face(source, MassFace::Side(0), V::new(4., -2., 3.), 0.25).unwrap();
        let Shape::ConceptMass { spec } = next.shape else {
            panic!()
        };
        assert_eq!(spec.arcs[0].unwrap().radius, 5.25);
        assert_eq!(spec.height, 6.);
        assert!(spec.validate().is_ok());
    }
}
