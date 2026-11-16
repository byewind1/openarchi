//! Screen-space selection over canonical model surfaces. No UI or project state.
use crate::{
    derivation_cache::GeometrySource, derived_geometry::GeometryDiagnostic, geometry, model::Point,
};
use glam::{DMat4, DVec3, DVec4};
use std::collections::{BTreeMap, BTreeSet};

fn surfaces(
    source: &GeometrySource,
    filter: geometry::ModelViewFilter<'_>,
) -> Result<Vec<[DVec3; 3]>, GeometryDiagnostic> {
    let area = filter.clip_area();
    let xy = area.map(|a| a.vertices.as_slice());
    let z = area.and_then(|a| a.z_range);
    Ok(match source {
        GeometrySource::Wall(g) => {
            g.geometry
                .clipped(xy, z)
                .map_err(|m| {
                    GeometryDiagnostic::error(
                        g.identity.element_id,
                        "WALL_CLIP_FAILED",
                        "selection",
                        m,
                    )
                })?
                .triangles
        }
        GeometrySource::Opening(g) => {
            g.geometry
                .clipped_proxy(xy, z)
                .map_err(|m| {
                    GeometryDiagnostic::error(
                        g.identity.element_id,
                        "OPENING_CLIP_FAILED",
                        "selection",
                        m,
                    )
                })?
                .triangles
        }
        GeometrySource::ConceptMass(g) | GeometrySource::Zone(g) | GeometrySource::Slab(g) => {
            if let Some(xy) = xy {
                g.clipped(xy, z)?.triangles().collect()
            } else {
                g.triangles().collect()
            }
        }
        GeometrySource::Roof(g) => g.clipped(xy, z)?.mesh.triangles,
        GeometrySource::Beam(g) => g.clipped(xy, z)?.mesh.triangles,
        GeometrySource::Column(g) => g.clipped(xy, z)?.mesh.triangles,
        GeometrySource::Stair(g) => g.clipped(xy, z)?.geometry.mesh.triangles,
        GeometrySource::Polyline(_) => Vec::new(),
    })
}

/// Authored mesh edges, never the edges of a triangulated face. Clipping uses
/// the same canonical solid as rendering and contributes its real cut edges.
fn model_edges(
    source: &GeometrySource,
    filter: geometry::ModelViewFilter<'_>,
) -> Result<Vec<[DVec3; 2]>, GeometryDiagnostic> {
    let area = filter.clip_area();
    let xy = area.map(|a| a.vertices.as_slice());
    let z = area.and_then(|a| a.z_range);
    Ok(match source {
        GeometrySource::Wall(g) => {
            g.geometry
                .clipped(xy, z)
                .map_err(|m| {
                    GeometryDiagnostic::error(
                        g.identity.element_id,
                        "WALL_CLIP_FAILED",
                        "selection",
                        m,
                    )
                })?
                .edges
        }
        GeometrySource::Opening(g) => {
            g.geometry
                .clipped_proxy(xy, z)
                .map_err(|m| {
                    GeometryDiagnostic::error(
                        g.identity.element_id,
                        "OPENING_CLIP_FAILED",
                        "selection",
                        m,
                    )
                })?
                .edges
        }
        GeometrySource::ConceptMass(g) | GeometrySource::Zone(g) | GeometrySource::Slab(g) => {
            if let Some(xy) = xy {
                g.clipped(xy, z)?.edges
            } else {
                g.edges.clone()
            }
        }
        GeometrySource::Roof(g) => g.clipped(xy, z)?.mesh.edges,
        GeometrySource::Beam(g) => g.clipped(xy, z)?.mesh.edges,
        GeometrySource::Column(g) => g.clipped(xy, z)?.mesh.edges,
        GeometrySource::Stair(g) => g.clipped(xy, z)?.geometry.mesh.edges,
        GeometrySource::Polyline(_) => Vec::new(),
    })
}

fn query_matrix(camera: &geometry::Camera, aspect: f64) -> Result<DMat4, GeometryDiagnostic> {
    let matrix = camera.matrix(aspect);
    if !aspect.is_finite() || aspect <= 0. || !matrix.is_finite() || !matrix.inverse().is_finite() {
        return Err(GeometryDiagnostic::error(
            0,
            "INVALID_SCREEN_SELECTION",
            "selection",
            "Selection camera and aspect must be finite and invertible",
        ));
    }
    Ok(matrix)
}

fn clip_point(p: DVec4) -> bool {
    p.is_finite() && p.w > 1e-10 && p.x.abs() <= p.w && p.y.abs() <= p.w && p.z >= 0. && p.z <= p.w
}

/// Clip a true model edge before perspective division. Screen-space Z remains
/// linear along this projected segment and can order hits from different objects.
fn projected_edge(edge: [DVec3; 2], matrix: DMat4) -> Option<[DVec3; 2]> {
    let mut a = matrix * edge[0].extend(1.);
    let mut b = matrix * edge[1].extend(1.);
    if !a.is_finite() || !b.is_finite() {
        return None;
    }
    for plane in [
        DVec4::new(1., 0., 0., 1.),
        DVec4::new(-1., 0., 0., 1.),
        DVec4::new(0., 1., 0., 1.),
        DVec4::new(0., -1., 0., 1.),
        DVec4::new(0., 0., 1., 0.),
        DVec4::new(0., 0., -1., 1.),
    ] {
        let da = plane.dot(a);
        let db = plane.dot(b);
        if da < 0. && db < 0. {
            return None;
        }
        if (da < 0.) != (db < 0.) {
            let cross = a.lerp(b, da / (da - db));
            if da < 0. {
                a = cross;
            } else {
                b = cross;
            }
        }
    }
    (a.w > 1e-10 && b.w > 1e-10).then(|| [a.truncate() / a.w, b.truncate() / b.w])
}

fn edge_hit_depth(
    edges: &[[DVec3; 2]],
    matrix: DMat4,
    point: Point,
    tolerance: f64,
) -> Option<f64> {
    edges
        .iter()
        .filter_map(|&edge| {
            let [a, b] = projected_edge(edge, matrix)?;
            let d = b.truncate() - a.truncate();
            let q = glam::DVec2::new(point.x, point.y);
            let t = if d.length_squared() > 0. {
                ((q - a.truncate()).dot(d) / d.length_squared()).clamp(0., 1.)
            } else {
                0.
            };
            (q.distance(a.truncate() + d * t) <= tolerance).then_some(a.z + (b.z - a.z) * t)
        })
        .min_by(f64::total_cmp)
}

fn source_pick_depth(
    source: &GeometrySource,
    matrix: DMat4,
    ndc: [f64; 2],
    filter: geometry::ModelViewFilter<'_>,
    surface: bool,
    tolerance: f64,
) -> Result<Option<f64>, GeometryDiagnostic> {
    if !surface {
        return Ok(edge_hit_depth(
            &model_edges(source, filter)?,
            matrix,
            Point::new(ndc[0], ndc[1]),
            tolerance,
        ));
    }
    let inv = matrix.inverse();
    let origin = inv.project_point3(DVec3::new(ndc[0], ndc[1], 0.));
    let dir = (inv.project_point3(DVec3::new(ndc[0], ndc[1], 1.)) - origin).normalize();
    let area = filter.clip_area();
    let hit = source.ray_hit(
        origin,
        dir,
        area.map(|a| a.vertices.as_slice()),
        area.and_then(|a| a.z_range),
    )?;
    let surface_depth = hit.and_then(|distance| {
        let point = matrix * (origin + dir * distance).extend(1.);
        clip_point(point).then_some(point.z / point.w)
    });
    if surface_depth.is_some() || tolerance == 0. {
        Ok(surface_depth)
    } else {
        Ok(edge_hit_depth(
            &model_edges(source, filter)?,
            matrix,
            Point::new(ndc[0], ndc[1]),
            tolerance,
        ))
    }
}

fn validate_pick(ndc: [f64; 2], tolerance: f64) -> Result<bool, GeometryDiagnostic> {
    if ndc.iter().any(|v| !v.is_finite()) || !tolerance.is_finite() || tolerance < 0. {
        return Err(GeometryDiagnostic::error(
            0,
            "INVALID_SCREEN_SELECTION",
            "selection",
            "Selection position and tolerance must be finite and tolerance nonnegative",
        ));
    }
    Ok(ndc.iter().all(|v| v.abs() <= 1.))
}

fn ordered_hits(mut hits: Vec<(u64, f64)>) -> Vec<u64> {
    hits.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    let mut seen = BTreeSet::new();
    hits.into_iter()
        .filter_map(|(id, _)| seen.insert(id).then_some(id))
        .collect()
}

// Clip before perspective division, including triangles straddling the eye.
fn projected(triangle: [DVec3; 3], matrix: DMat4) -> Vec<Point> {
    let mut polygon: Vec<_> = triangle
        .into_iter()
        .map(|p| matrix * p.extend(1.))
        .collect();
    for plane in [DVec4::new(0., 0., 1., 0.), DVec4::new(0., 0., -1., 1.)] {
        let mut next = Vec::new();
        if polygon.is_empty() {
            break;
        }
        let mut previous = *polygon.last().unwrap();
        for &current in &polygon {
            let a = plane.dot(previous);
            let b = plane.dot(current);
            if (a >= 0.) != (b >= 0.) {
                next.push(previous.lerp(current, a / (a - b)));
            }
            if b >= 0. {
                next.push(current);
            }
            previous = current;
        }
        polygon = next;
    }
    polygon
        .into_iter()
        .filter(|p| p.is_finite() && p.w > 1e-10)
        .map(|p| Point::new(p.x / p.w, p.y / p.w))
        .collect()
}

pub(crate) fn matches_surface(
    triangles: &[[DVec3; 3]],
    matrix: DMat4,
    rectangle: &[Point],
    entire: bool,
) -> bool {
    let mut seen = false;
    for &triangle in triangles {
        let p = projected(triangle, matrix);
        if p.len() < 3 {
            continue;
        }
        seen = true;
        if entire {
            if p.iter().any(|p| !geometry::contains(*p, rectangle)) {
                return false;
            }
        } else if geometry::closed_path_intersects_polygon(&p, rectangle) {
            return true;
        }
    }
    entire && seen
}

impl crate::editor::Editor {
    /// NDC rectangle, independent of DPI. `entire` tests the displayed surface,
    /// crossing tests actual triangles rather than their bounding box.
    pub fn model_box_selection(
        &self,
        camera: &geometry::Camera,
        aspect: f64,
        a: Point,
        b: Point,
        entire: bool,
        filter: geometry::ModelViewFilter<'_>,
    ) -> Result<Vec<u64>, GeometryDiagnostic> {
        if !aspect.is_finite()
            || aspect <= 0.
            || !a.x.is_finite()
            || !a.y.is_finite()
            || !b.x.is_finite()
            || !b.y.is_finite()
            || !camera.matrix(aspect).is_finite()
        {
            return Err(GeometryDiagnostic::error(
                0,
                "INVALID_SCREEN_SELECTION",
                "selection",
                "Selection rectangle and camera must be finite",
            ));
        }
        let rectangle = geometry::rectangle_polygon(a, b);
        let matrix = camera.matrix(aspect);
        self.with_geometry_read(|read| {
            let mut ids = Vec::new();
            for e in &self.project.elements {
                if !read.model_includes(e.id, filter)? {
                    continue;
                }
                let source = read.source(e.id)?;
                if matches_surface(
                    &surfaces(&source.source, filter)?,
                    matrix,
                    &rectangle,
                    entire,
                ) {
                    ids.push(e.id);
                }
            }
            Ok(ids)
        })?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crossing_hits_edges_without_nodes_and_rejects_empty_bounding_corners() {
        let t = [[
            DVec3::new(-0.9, 0., 0.5),
            DVec3::new(0.9, 0., 0.5),
            DVec3::new(0., 0.9, 0.5),
        ]];
        let r = geometry::rectangle_polygon(Point::new(-0.1, -0.1), Point::new(0.1, 0.1));
        assert!(matches_surface(&t, DMat4::IDENTITY, &r, false));
        assert!(!matches_surface(&t, DMat4::IDENTITY, &r, true));
        let corner = geometry::rectangle_polygon(Point::new(0.7, 0.7), Point::new(0.8, 0.8));
        assert!(!matches_surface(&t, DMat4::IDENTITY, &corner, false));
        let all = geometry::rectangle_polygon(Point::new(-1., -1.), Point::new(1., 1.));
        assert!(matches_surface(&t, DMat4::IDENTITY, &all, true));
        let behind = t.map(|v| v.map(|p| DVec3::new(p.x, p.y, -1.)));
        assert!(!matches_surface(&behind, DMat4::IDENTITY, &all, false));
    }

    #[test]
    fn candidate_paths_deduplicate_objects_and_omit_internal_face_diagonals() {
        let p = |x, y| Point::new(x, y);
        let mut paths = vec![
            SelectionPath {
                id: 1,
                points: vec![p(0., 0.), p(4., 0.), p(4., 4.)],
            },
            SelectionPath {
                id: 1,
                points: vec![p(0., 0.), p(4., 4.), p(0., 4.)],
            },
            SelectionPath {
                id: 2,
                points: vec![p(0., 0.), p(4., 0.), p(4., 4.)],
            },
            SelectionPath {
                id: 2,
                points: vec![p(0., 0.), p(4., 4.), p(0., 4.)],
            },
        ];
        assert_eq!(pick_paths(&paths, p(2., 2.), 0.01, true), vec![2, 1]);
        assert!(pick_paths(&paths, p(2., 2.), 0.01, false).is_empty());
        // Includes each polygon's implicit last-to-first edge.
        assert_eq!(pick_paths(&paths, p(0., 2.), 0.01, false), vec![2, 1]);
        // Equal/reversed duplicated fragments do not erase an outer edge.
        let mut duplicate = paths[0].clone();
        duplicate.points.reverse();
        paths.push(duplicate);
        assert_eq!(pick_paths(&paths, p(2., 0.), 0.01, false), vec![1, 2]);
        assert!(pick_paths(&paths, p(2., 2.), 0.01, false).is_empty());
        paths.push(SelectionPath {
            id: 3,
            points: vec![p(1., 1.), p(3., 1.)],
        });
        assert_eq!(pick_paths(&paths, p(2., 1.), 0.01, false), vec![3]);
        assert!(pick_paths(&paths, p(2., 1.), -0.1, true).is_empty());
    }

    #[test]
    fn boundary_only_selection_preserves_holes_and_avoids_hidden_triangle_seams() {
        use geo::{LineString, Polygon};
        let polygon = Polygon::new(
            LineString::from(vec![(0., 0.), (10., 0.), (10., 10.), (0., 10.), (0., 0.)]),
            vec![LineString::from(vec![
                (3., 3.),
                (7., 3.),
                (7., 7.),
                (3., 7.),
                (3., 3.),
            ])],
        );
        let paths: Vec<_> = crate::derived_geometry::triangulate_with_boundary(&polygon)
            .into_iter()
            .map(|triangle| SelectionPath {
                id: 7,
                points: triangle.into_iter().map(|p| Point::new(p.x, p.y)).collect(),
            })
            .collect();
        assert!(pick_paths(&paths, Point::new(5., 5.), 0.01, true).is_empty());
        assert_eq!(pick_paths(&paths, Point::new(3., 5.), 0.01, false), vec![7]);
        assert_eq!(pick_paths(&paths, Point::new(1., 1.), 0.01, true), vec![7]);
        assert!(pick_paths(&paths, Point::new(1., 1.), 0.01, false).is_empty());
    }

    fn overlapping_masses() -> (crate::editor::Editor, geometry::Camera, u64, u64) {
        use crate::model::{Material, Project, Shape};
        let mut project = Project::default();
        let mut mass = |bottom_offset| {
            project.add(
                0,
                Material::Concrete,
                Shape::ConceptMass {
                    spec: crate::concept_mass::ConceptMassSpec {
                        outline: geometry::rectangle_polygon(
                            Point::new(-2., -2.),
                            Point::new(2., 2.),
                        ),
                        holes: vec![],
                        arcs: vec![],
                        hole_arcs: vec![],
                        height: 0.3,
                        bottom_offset,
                    },
                },
            )
        };
        let back = mass(0.);
        let front = mass(1.);
        let camera = geometry::Camera {
            yaw: 0.,
            pitch: 1.3,
            span: 12.,
            target: DVec3::ZERO,
        };
        (crate::editor::Editor::new(project), camera, back, front)
    }

    #[test]
    fn model_candidates_use_depth_visible_set_and_keep_document_and_history_unchanged() {
        let (mut editor, camera, back, front) = overlapping_masses();
        let before = editor.project.clone();
        let filter = geometry::ModelViewFilter::default();
        assert_eq!(
            editor
                .model_pick_candidates(&camera, [0., 0.], 1., filter, true, 0.01)
                .unwrap(),
            vec![front, back]
        );
        assert!(
            editor
                .model_pick_candidates(&camera, [0., 0.], 1., filter, false, 0.001)
                .unwrap()
                .is_empty()
        );
        assert_eq!(editor.project, before);
        assert!(editor.undo.is_empty() && editor.redo.is_empty());
        assert_eq!(editor.revision, 0);
        assert!(
            editor
                .model_pick_candidates(&camera, [2., 0.], 1., filter, true, 0.01)
                .unwrap()
                .is_empty()
        );
        assert!(
            editor
                .model_pick_candidates(&camera, [0., 0.], f64::NAN, filter, true, 0.01)
                .is_err()
        );
        let layer = editor.project.layers.new_layer("Hidden candidate", None);
        editor
            .project
            .layers
            .layer_mut(layer)
            .unwrap()
            .status
            .visible = false;
        editor
            .project
            .elements
            .iter_mut()
            .find(|e| e.id == front)
            .unwrap()
            .layer
            .0 = layer;
        assert_eq!(
            editor
                .model_pick_candidates(&camera, [0., 0.], 1., filter, true, 0.01)
                .unwrap(),
            vec![back]
        );
        let paths = editor.model_selection_paths(&camera, 1., filter).unwrap();
        assert!(paths.iter().all(|p| p.id == back));
        let selected = [front];
        assert!(
            editor
                .model_pick_candidates(
                    &camera,
                    [0., 0.],
                    1.,
                    geometry::ModelViewFilter {
                        selected: &selected,
                        selection_only: true,
                        ..filter
                    },
                    true,
                    0.01
                )
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn comparison_candidates_preserve_explicit_options_but_ignore_hidden_or_deleted_ids() {
        let (mut editor, camera, back, front) = overlapping_masses();
        let a = editor.project.design_options.create("A".into());
        let b = editor.project.design_options.create("B".into());
        editor
            .project
            .elements
            .iter_mut()
            .find(|e| e.id == back)
            .unwrap()
            .design_option = Some(a);
        editor
            .project
            .elements
            .iter_mut()
            .find(|e| e.id == front)
            .unwrap()
            .design_option = Some(b);
        editor.project.plan_options.active_design_option = Some(a);
        assert_eq!(
            editor
                .model_pick_candidates(
                    &camera,
                    [0., 0.],
                    1.,
                    geometry::ModelViewFilter::default(),
                    true,
                    0.01
                )
                .unwrap(),
            vec![back]
        );
        assert_eq!(
            editor
                .candidate_pick_candidates(
                    &camera,
                    1.,
                    [0., 0.],
                    &[front, back, front, u64::MAX],
                    true,
                    0.01
                )
                .unwrap(),
            vec![front, back]
        );
        editor.project.elements.retain(|e| e.id != front);
        assert_eq!(
            editor
                .candidate_pick_candidates(&camera, 1., [0., 0.], &[front, back], true, 0.01)
                .unwrap(),
            vec![back]
        );
        assert!(
            editor
                .candidate_selection_paths(&camera, 1., &[front])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn canonical_model_edges_exclude_cap_triangulation_and_hit_actual_outline() {
        let (editor, camera, back, _) = overlapping_masses();
        let matrix = camera.matrix(1.);
        let center = matrix.project_point3(DVec3::new(0., 0., 0.3));
        assert!(
            !editor
                .candidate_pick_candidates(&camera, 1., [center.x, center.y], &[back], true, 1e-5)
                .unwrap()
                .is_empty()
        );
        assert!(
            editor
                .candidate_pick_candidates(&camera, 1., [center.x, center.y], &[back], false, 1e-5)
                .unwrap()
                .is_empty()
        );
        let edge = matrix.project_point3(DVec3::new(0., -2., 0.3));
        assert_eq!(
            editor
                .candidate_pick_candidates(&camera, 1., [edge.x, edge.y], &[back], false, 1e-5)
                .unwrap(),
            vec![back]
        );
        assert!(
            edge_hit_depth(
                &[[DVec3::new(0., 0., -1.), DVec3::new(1., 0., -1.)]],
                DMat4::IDENTITY,
                Point::new(0., 0.),
                0.01
            )
            .is_none()
        );
    }

    #[test]
    fn clipped_model_candidates_share_the_actual_marquee_surfaces_and_edges() {
        let (editor, camera, back, front) = overlapping_masses();
        let mut area = crate::editor::MarqueeArea::new(
            geometry::rectangle_polygon(Point::new(-1., -1.), Point::new(1., 1.)),
            crate::editor::MarqueeScope::AllStories,
            0,
        )
        .unwrap();
        area.z_range = Some((0., 0.2));
        let filter = geometry::ModelViewFilter {
            marquee: Some(&area),
            ..Default::default()
        };
        assert_eq!(
            editor
                .model_pick_candidates(&camera, [0., 0.], 1., filter, true, 0.001)
                .unwrap(),
            vec![back]
        );
        let paths = editor.model_selection_paths(&camera, 1., filter).unwrap();
        assert!(paths.iter().any(|p| p.id == back));
        assert!(!paths.iter().any(|p| p.id == front));
        let q = camera.matrix(1.).project_point3(DVec3::new(1., 0., 0.2));
        assert_eq!(
            editor
                .model_pick_candidates(&camera, [q.x, q.y], 1., filter, false, 0.001)
                .unwrap(),
            vec![back]
        );
    }
}

/// One displayed fragment. Solids use triangles (including hole boundaries);
/// open curves use two-point segments. Coordinates belong to the caller's view.
#[derive(Clone, Debug)]
pub struct SelectionPath {
    pub id: u64,
    pub points: Vec<Point>,
}

pub fn path_intersects(points: &[Point], rectangle: &[Point]) -> bool {
    points.iter().any(|p| geometry::contains(*p, rectangle))
        || points.windows(2).any(|edge| {
            (0..rectangle.len()).any(|i| {
                geometry::segments_intersect(
                    edge[0],
                    edge[1],
                    rectangle[i],
                    rectangle[(i + 1) % rectangle.len()],
                )
            })
        })
        || (points.len() >= 3 && geometry::closed_path_intersects_polygon(points, rectangle))
}

pub fn select_paths(paths: &[SelectionPath], rectangle: &[Point], entire: bool) -> Vec<u64> {
    let mut hits = std::collections::BTreeMap::<u64, (bool, bool)>::new();
    for path in paths.iter().filter(|p| !p.points.is_empty()) {
        let hit = hits.entry(path.id).or_insert((false, true));
        hit.0 |= path_intersects(&path.points, rectangle);
        hit.1 &= path
            .points
            .iter()
            .all(|p| geometry::contains(*p, rectangle));
    }
    hits.into_iter()
        .filter_map(|(id, (any, all))| (if entire { all } else { any }).then_some(id))
        .collect()
}

/// All hits in reverse display order, once per element. Closed fragments are
/// a tessellation of the same object: paired internal edges are not pickable.
pub fn pick_paths(
    paths: &[SelectionPath],
    point: Point,
    tolerance: f64,
    surface: bool,
) -> Vec<u64> {
    if !point.finite() || !tolerance.is_finite() || tolerance < 0. {
        return Vec::new();
    }
    let key = |p: Point| {
        [
            if p.x == 0. { 0 } else { p.x.to_bits() },
            if p.y == 0. { 0 } else { p.y.to_bits() },
        ]
    };
    let mut edges = BTreeMap::<(u64, [[u64; 2]; 2]), (u8, [Point; 2])>::new();
    let mut hits = BTreeSet::new();
    for path in paths {
        if surface && path.points.len() >= 3 && geometry::contains(point, &path.points) {
            hits.insert(path.id);
        }
        if path.points.len() == 2 {
            if geometry::segment_distance(point, path.points[0], path.points[1]) <= tolerance {
                hits.insert(path.id);
            }
        } else if path.points.len() >= 3 {
            let area = path
                .points
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let b = path.points[(i + 1) % path.points.len()];
                    a.x * b.y - b.x * a.y
                })
                .sum::<f64>();
            for i in 0..path.points.len() {
                let edge = [path.points[i], path.points[(i + 1) % path.points.len()]];
                let mut pair = edge.map(key);
                let forward = pair[0] <= pair[1];
                pair.sort_unstable();
                let entry = edges.entry((path.id, pair)).or_insert((0, edge));
                // Repeated equal faces are harmless. An edge is internal only
                // when occupied fragments occur on both of its sides.
                entry.0 |= if (area >= 0.) == forward { 1 } else { 2 };
            }
        }
    }
    for ((id, _), (sides, [a, b])) in edges {
        if (surface || sides != 3) && geometry::segment_distance(point, a, b) <= tolerance {
            hits.insert(id);
        }
    }
    let mut seen = BTreeSet::new();
    paths
        .iter()
        .rev()
        .filter_map(|p| (hits.contains(&p.id) && seen.insert(p.id)).then_some(p.id))
        .collect()
}

pub fn pick_path(paths: &[SelectionPath], point: Point, tolerance: f64) -> Option<u64> {
    pick_paths(paths, point, tolerance, true).into_iter().next()
}

impl crate::derivation_cache::GeometryBatch {
    pub fn selection_plan_paths(
        &self,
        ids: &[u64],
    ) -> Result<Vec<SelectionPath>, GeometryDiagnostic> {
        let mut paths = Vec::new();
        for &id in ids {
            let g = &self.source(id)?.source;
            let triangles: Vec<[Point; 3]> = match g.as_ref() {
                GeometrySource::Wall(g) => g.geometry.plan_triangles.clone(),
                GeometrySource::Opening(g) => g.geometry.plan_triangles.clone(),
                GeometrySource::Slab(g)
                | GeometrySource::Zone(g)
                | GeometrySource::ConceptMass(g) => g.plan_triangles.clone(),
                GeometrySource::Roof(g) => g.geometry.plan_triangles.clone(),
                GeometrySource::Beam(g) => g.geometry.plan_triangles.clone(),
                GeometrySource::Stair(g) => g.geometry.plan_triangles.to_vec(),
                GeometrySource::Column(_) => {
                    use crate::column_geometry::ColumnPlanView;
                    match &self.column_plan(id)?.view {
                        ColumnPlanView::SymbolicFootprint(p) => geometry::triangulate_polygon(p),
                        ColumnPlanView::Projected { geometry: g, .. } => {
                            [&g.cut, &g.uncut, &g.overhead]
                                .into_iter()
                                .flat_map(|p| geometry::triangulate_polygon(p))
                                .collect()
                        }
                        ColumnPlanView::NotOnStory => Vec::new(),
                    }
                }
                GeometrySource::Polyline(g) => {
                    paths.extend(g.geometry.plan_path.windows(2).map(|pair| SelectionPath {
                        id,
                        points: pair.to_vec(),
                    }));
                    Vec::new()
                }
            };
            paths.extend(triangles.into_iter().map(|triangle| SelectionPath {
                id,
                points: triangle.to_vec(),
            }));
        }
        Ok(paths)
    }
}

/// Paper-coordinate fragments used by the existing figure painters. Source
/// snapshots remain immutable; this only maps their displayed faces to IDs.
pub fn diagram_selection_paths(
    figure: &crate::diagrams::Diagram,
) -> Result<Vec<SelectionPath>, String> {
    let mut paths = Vec::new();
    if figure.definition.presentation.is_axon() {
        let page = crate::diagrams::axon::Page::new(figure)?;
        for face in &page.faces {
            paths.push(SelectionPath {
                id: face.element_id,
                points: face
                    .points
                    .iter()
                    .map(|q| {
                        let p = page.point(*q);
                        Point::new(p[0], p[1])
                    })
                    .collect(),
            });
        }
    } else {
        let mut polygons = Vec::new();
        if figure.definition.presentation.is_site() {
            let page = crate::diagrams::site::Page::new(figure)?;
            for path in &page.paths {
                if let Some(id) = path.element_id {
                    polygons.push((
                        id,
                        path.rings
                            .iter()
                            .map(|ring| {
                                ring.iter()
                                    .map(|q| {
                                        let p = page.layout.point(*q);
                                        Point::new(p[0], p[1])
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>(),
                    ));
                }
            }
        } else {
            let layout = crate::diagrams::Layout::new(figure);
            for e in &figure.snapshot.elements {
                if let Some((outer, holes)) = e.authored_prism_rings() {
                    polygons.push((
                        e.id,
                        std::iter::once(&outer)
                            .chain(holes.iter())
                            .map(|ring| {
                                ring.iter()
                                    .map(|q| {
                                        let p = layout.point(*q);
                                        Point::new(p[0], p[1])
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>(),
                    ));
                }
            }
        }
        for (id, rings) in polygons {
            if rings.is_empty() {
                continue;
            }
            let ring = |p: &Vec<Point>| {
                geo::LineString::from(p.iter().map(|q| (q.x, q.y)).collect::<Vec<_>>())
            };
            let polygon = geo::Polygon::new(ring(&rings[0]), rings[1..].iter().map(ring).collect());
            paths.extend(
                crate::derived_geometry::triangulate_with_boundary(&polygon)
                    .into_iter()
                    .map(|t| SelectionPath {
                        id,
                        points: t.into_iter().map(|p| Point::new(p.x, p.y)).collect(),
                    }),
            );
        }
    }
    Ok(paths)
}

impl crate::editor::Editor {
    /// Candidate projection for the current model viewport. Uses exactly the
    /// displayed model filter; its fragments are derived and never persisted.
    pub fn model_selection_paths(
        &self,
        camera: &geometry::Camera,
        aspect: f64,
        filter: geometry::ModelViewFilter<'_>,
    ) -> Result<Vec<SelectionPath>, GeometryDiagnostic> {
        let matrix = query_matrix(camera, aspect)?;
        self.with_geometry_read(|read| {
            let mut paths = Vec::new();
            for e in &self.project.elements {
                if !read.model_includes(e.id, filter)? {
                    continue;
                }
                for triangle in surfaces(&read.source(e.id)?.source, filter)? {
                    let points = projected(triangle, matrix);
                    if points.len() >= 3 {
                        paths.push(SelectionPath { id: e.id, points });
                    }
                }
            }
            Ok(paths)
        })?
    }

    pub fn model_pick_candidates(
        &self,
        camera: &geometry::Camera,
        ndc: [f64; 2],
        aspect: f64,
        filter: geometry::ModelViewFilter<'_>,
        surface: bool,
        tolerance_ndc: f64,
    ) -> Result<Vec<u64>, GeometryDiagnostic> {
        let matrix = query_matrix(camera, aspect)?;
        if !validate_pick(ndc, tolerance_ndc)? {
            return Ok(Vec::new());
        }
        self.with_geometry_read(|read| {
            let mut hits = Vec::new();
            for e in &self.project.elements {
                if !read.model_includes(e.id, filter)? {
                    continue;
                }
                if let Some(depth) = source_pick_depth(
                    &read.source(e.id)?.source,
                    matrix,
                    ndc,
                    filter,
                    surface,
                    tolerance_ndc,
                )? {
                    hits.push((e.id, depth));
                }
            }
            Ok(ordered_hits(hits))
        })?
    }

    /// Explicit candidates may span both design options. They still must exist
    /// and be displayed by their layer and host; the main active option is not
    /// the authority for a comparison pane.
    pub fn candidate_pick_candidates(
        &self,
        camera: &geometry::Camera,
        aspect: f64,
        ndc: [f64; 2],
        ids: &[u64],
        surface: bool,
        tolerance_ndc: f64,
    ) -> Result<Vec<u64>, GeometryDiagnostic> {
        let matrix = query_matrix(camera, aspect)?;
        if !validate_pick(ndc, tolerance_ndc)? {
            return Ok(Vec::new());
        }
        self.with_geometry_read(|read| {
            let mut hits = Vec::new();
            let mut seen = BTreeSet::new();
            for &id in ids {
                let Some(e) = self.project.elements.iter().find(|e| e.id == id) else {
                    continue;
                };
                if !seen.insert(id) || !geometry::model_element_layer_visible(&self.project, e) {
                    continue;
                }
                if let Some(depth) = source_pick_depth(
                    &read.source(id)?.source,
                    matrix,
                    ndc,
                    geometry::ModelViewFilter::default(),
                    surface,
                    tolerance_ndc,
                )? {
                    hits.push((id, depth));
                }
            }
            Ok(ordered_hits(hits))
        })?
    }

    /// Explicit displayed candidate set. Comparison viewports intentionally show
    /// both design options, independently of the main canvas's active option.
    pub fn candidate_selection_paths(
        &self,
        camera: &geometry::Camera,
        aspect: f64,
        ids: &[u64],
    ) -> Result<Vec<SelectionPath>, GeometryDiagnostic> {
        let matrix = query_matrix(camera, aspect)?;
        self.with_geometry_read(|read| {
            let mut paths = Vec::new();
            for &id in ids {
                let Some(e) = self.project.elements.iter().find(|e| e.id == id) else {
                    continue;
                };
                if !geometry::model_element_layer_visible(&self.project, e) {
                    continue;
                }
                let g = read.source(id)?;
                for triangle in surfaces(&g.source, geometry::ModelViewFilter::default())? {
                    let points = projected(triangle, matrix);
                    if points.len() >= 3 {
                        paths.push(SelectionPath { id, points });
                    }
                }
            }
            Ok(paths)
        })?
    }
    pub fn pick_candidate(
        &self,
        camera: &geometry::Camera,
        aspect: f64,
        ndc: [f64; 2],
        ids: &[u64],
    ) -> Result<Option<u64>, GeometryDiagnostic> {
        Ok(self
            .candidate_pick_candidates(camera, aspect, ndc, ids, true, 0.)?
            .into_iter()
            .next())
    }
}
