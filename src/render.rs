//! Native wgpu offscreen scene. The depth-tested texture is composited by egui.
use crate::{
    derived_geometry::GeometryDiagnostic, editor::MarqueeArea, geometry::Camera, model::*,
};
use eframe::{
    egui,
    egui_wgpu::{RenderState, wgpu},
};
use wgpu::util::DeviceExt;
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuVertex {
    position: [f32; 3],
    color: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Vertex {
    position: [f64; 3],
    color: [f32; 3],
}

fn gpu_vertices(vertices: &[Vertex], origin: glam::DVec3) -> Vec<GpuVertex> {
    vertices
        .iter()
        .map(|v| GpuVertex {
            position: crate::precision::RenderOrigin(origin)
                .position(glam::DVec3::from_array(v.position)),
            color: v.color,
        })
        .collect()
}

fn vertex(p: glam::DVec3, color: [f32; 3]) -> Vertex {
    Vertex {
        position: p.to_array(),
        color,
    }
}

fn push_triangle(
    out: &mut Vec<Vertex>,
    a: glam::DVec3,
    b: glam::DVec3,
    c: glam::DVec3,
    color: [f32; 3],
) {
    out.extend([vertex(a, color), vertex(b, color), vertex(c, color)]);
}

/// Extrude the same arbitrary wall display polygon used by the plan. A junction
/// can contribute two cap edges through its hub, so a four-corner prism loses
/// geometry. Keep all individual cap faces: at unequal elevations they may be
/// exposed, while the same-level merged path removes internal faces beforehand.
fn append_wall_polygon_prism(
    outline: &[Point],
    bottom: f64,
    top: f64,
    base: [f32; 3],
    edge: [f32; 3],
    vertices: &mut Vec<Vertex>,
    lines: &mut Vec<Vertex>,
) -> bool {
    let Ok(mesh) = crate::wall_solid::polygon_surface(outline, bottom, top) else {
        return false;
    };
    for [a, b, c] in mesh.triangles {
        let n = (b - a).cross(c - a).normalize();
        let shade = if n.z > 0.999 {
            1.
        } else if n.z < -0.999 {
            0.48
        } else {
            0.73
        };
        push_triangle(vertices, a, b, c, base.map(|v| v * shade));
    }
    for [a, b] in mesh.edges {
        lines.extend([vertex(a, edge), vertex(b, edge)]);
    }
    true
}

#[cfg(test)]
fn append_slab_prism(
    e: &Element,
    bottom: f64,
    top: f64,
    base: [f32; 3],
    vertices: &mut Vec<Vertex>,
    lines: &mut Vec<Vertex>,
) -> Result<(), GeometryDiagnostic> {
    let story_base = top - crate::slab_geometry::contour(e).elevation;
    let geometry = crate::slab_geometry::derive(e, story_base)?;
    append_slab_snapshot(&geometry, top, base, vertices, lines);
    let _ = bottom;
    Ok(())
}
fn append_slab_snapshot(
    geometry: &crate::slab_geometry::DerivedSlab,
    top: f64,
    base: [f32; 3],
    vertices: &mut Vec<Vertex>,
    lines: &mut Vec<Vertex>,
) {
    for [a, b, c] in &geometry.caps {
        let color = if (a.z - top).abs() < 1e-5 {
            base
        } else {
            base.map(|x| x * 0.48)
        };
        push_triangle(vertices, *a, *b, *c, color);
    }
    for (setting, q) in &geometry.sides {
        let side = setting
            .surface
            .map_or(base, |m| m.color())
            .map(|x| x * 0.73);
        push_triangle(vertices, q[0], q[1], q[2], side);
        push_triangle(vertices, q[0], q[2], q[3], side);
    }
    let edge = base.map(|x| x * 0.48);
    for [a, b] in &geometry.edges {
        lines.extend([vertex(*a, edge), vertex(*b, edge)]);
    }
}

fn wall_level_groups(project: &Project, walls: &[&Element]) -> Vec<(f64, f64, Vec<usize>)> {
    let mut groups: Vec<(f64, f64, Vec<usize>)> = Vec::new();
    for (index, wall) in walls.iter().enumerate() {
        let Some((bottom, top)) = crate::wall_solid::vertical_range(project, wall) else {
            continue;
        };
        if let Some((_, _, indices)) = groups
            .iter_mut()
            .find(|(low, high, _)| (bottom - *low).abs() <= 1e-4 && (top - *high).abs() <= 1e-4)
        {
            indices.push(index);
        } else {
            groups.push((bottom, top, vec![index]));
        }
    }
    groups
}

/// Draws connected, coplanar walls as one extruded perimeter. Actual bottom/top
/// levels (including wall offsets) determine whether walls can share this mesh.
fn append_joined_wall_prism(
    bottom: f64,
    top: f64,
    material: Material,
    outline: Vec<Point>,
    vertices: &mut Vec<Vertex>,
    lines: &mut Vec<Vertex>,
) -> bool {
    let base = material.color();
    append_wall_polygon_prism(
        &outline,
        bottom,
        top,
        base,
        base.map(|x| x * 0.42),
        vertices,
        lines,
    )
}

/// CPU consumer of the same checked source/clip results used by queries and
/// picking. Exhaustive matching prevents a future shape falling back to a box.
fn append_element_checked(
    project: &Project,
    element: &Element,
    clip: Option<&MarqueeArea>,
    base: [f32; 3],
    vertices: &mut Vec<Vertex>,
    lines: &mut Vec<Vertex>,
    walls: &mut crate::wall_solid::WallQuery<'_>,
) -> Result<(), GeometryDiagnostic> {
    use crate::derivation_cache::GeometrySource as S;
    let source = match &element.shape {
        Shape::ConceptMass { .. } => S::ConceptMass(crate::concept_mass::derive(
            element,
            project.elevation(element.story),
        )?),
        Shape::Zone { .. } => S::Zone(crate::zones::derive(
            element,
            project.elevation(element.story),
        )?),
        Shape::Wall { .. } => S::Wall(walls.wall(element.id)?.clone()),
        Shape::Opening { .. } => S::Opening(walls.opening(element.id)?),
        Shape::Slab { .. } => S::Slab(crate::slab_geometry::derive(
            element,
            project.elevation(element.story),
        )?),
        Shape::Roof { .. } => S::Roof(crate::roof_geometry::derive(
            element,
            project.elevation(element.story),
        )?),
        Shape::Column { .. } => S::Column(crate::column_geometry::derive(project, element)?),
        Shape::Beam { .. } => S::Beam(crate::beam_geometry::derive(project, element)?),
        Shape::Stair { .. } => S::Stair(crate::stair_geometry::derive(project, element)?),
        Shape::Polyline { .. } => S::Polyline(crate::polyline_geometry::derive(project, element)?),
    };
    append_source_checked(&source, clip, base, vertices, lines)
}
fn append_source_checked(
    source: &crate::derivation_cache::GeometrySource,
    clip: Option<&MarqueeArea>,
    base: [f32; 3],
    vertices: &mut Vec<Vertex>,
    lines: &mut Vec<Vertex>,
) -> Result<(), GeometryDiagnostic> {
    use crate::derivation_cache::GeometrySource as S;
    let id = source.identity().element_id;
    let xy = clip.map(|a| a.vertices.as_slice());
    let z = clip.and_then(|a| a.z_range);
    crate::geometry_validation::validate_clip_context(xy, z)
        .map_err(|m| GeometryDiagnostic::error(id, "INVALID_VIEW_CLIP", "clipping", m))?;
    match source {
        S::Polyline(_) => {}
        S::Opening(g) => {
            for (role, part) in &g.geometry.joinery {
                let mesh = part.clipped(xy, z).map_err(|m| {
                    GeometryDiagnostic::error(id, "OPENING_CLIP_FAILED", "joinery", m)
                })?;
                let color = match role {
                    crate::opening_joinery::PartRole::Frame => [0.72, 0.70, 0.64],
                    crate::opening_joinery::PartRole::Leaf => [0.62, 0.43, 0.27],
                };
                append_lit_mesh(&mesh, color, MeshStyle::Wall, vertices, lines);
            }
            if let Some(pane) = &g.geometry.pane {
                let clipped;
                let mesh = if clip.is_none() {
                    &pane.mesh
                } else {
                    clipped = pane.clipped(xy, z).map_err(|m| {
                        GeometryDiagnostic::error(id, "OPENING_CLIP_FAILED", "clipping", m)
                    })?;
                    &clipped
                };
                for &[a, b, c] in &mesh.triangles {
                    push_triangle(vertices, a, b, c, [0.55, 0.74, 0.82]);
                }
            }
        }
        S::Wall(g) => {
            let clipped;
            let mesh = if clip.is_none() {
                &g.geometry.mesh
            } else {
                clipped = g.geometry.clipped(xy, z).map_err(|m| {
                    GeometryDiagnostic::error(id, "WALL_CLIP_FAILED", "clipping", m)
                })?;
                &clipped
            };
            append_lit_mesh(mesh, base, MeshStyle::Wall, vertices, lines);
        }
        S::ConceptMass(g) | S::Slab(g) | S::Zone(g) => {
            if let Some(area) = clip {
                let mesh = g.clipped(&area.vertices, z)?;
                for (triangles, shade) in [(mesh.top, 1.), (mesh.bottom, 0.48), (mesh.sides, 0.73)]
                {
                    for [a, b, c] in triangles {
                        push_triangle(vertices, a, b, c, base.map(|v| v * shade));
                    }
                }
                for [a, b] in mesh.edges {
                    lines.extend([
                        vertex(a, base.map(|v| v * 0.48)),
                        vertex(b, base.map(|v| v * 0.48)),
                    ]);
                }
            } else {
                append_slab_snapshot(g, g.bounds[1].z, base, vertices, lines);
            }
        }
        S::Roof(g) => {
            let clipped;
            let mesh = if clip.is_none() {
                &g.geometry.mesh
            } else {
                clipped = g.clipped(xy, z)?.mesh;
                &clipped
            };
            append_lit_mesh(mesh, base, MeshStyle::Roof, vertices, lines);
        }
        S::Beam(g) => {
            let clipped;
            let mesh = if clip.is_none() {
                &g.geometry.mesh
            } else {
                clipped = g.clipped(xy, z)?.mesh;
                &clipped
            };
            append_lit_mesh(mesh, base, MeshStyle::Section, vertices, lines);
        }
        S::Column(g) => {
            let clipped;
            let mesh = if clip.is_none() {
                &g.geometry.mesh
            } else {
                clipped = g.clipped(xy, z)?.mesh;
                &clipped
            };
            append_lit_mesh(mesh, base, MeshStyle::Section, vertices, lines);
        }
        S::Stair(g) => {
            let clipped;
            let (triangles, edges, handrails): (
                &[[glam::DVec3; 3]],
                &[[glam::DVec3; 2]],
                &[[glam::DVec3; 2]],
            ) = if clip.is_none() {
                (
                    &g.geometry.source.triangles,
                    &g.geometry.source.edges,
                    &g.geometry.source.handrails,
                )
            } else {
                clipped = g.clipped(xy, z)?.geometry;
                (
                    &clipped.mesh.triangles,
                    &clipped.mesh.edges,
                    &clipped.handrails,
                )
            };
            append_lit_surfaces(triangles, edges, base, MeshStyle::Stair, vertices, lines);
            for &[a, b] in handrails {
                lines.extend([vertex(a, [0.16, 0.22, 0.22]), vertex(b, [0.16, 0.22, 0.22])]);
            }
        }
    }
    Ok(())
}
#[derive(Clone, Copy)]
enum MeshStyle {
    Wall,
    Roof,
    Section,
    Stair,
}
fn append_lit_mesh(
    mesh: &crate::derived_geometry::SurfaceMesh,
    base: [f32; 3],
    style: MeshStyle,
    vertices: &mut Vec<Vertex>,
    lines: &mut Vec<Vertex>,
) {
    append_lit_surfaces(&mesh.triangles, &mesh.edges, base, style, vertices, lines);
}
fn append_lit_surfaces(
    triangles: &[[glam::DVec3; 3]],
    edges: &[[glam::DVec3; 2]],
    base: [f32; 3],
    style: MeshStyle,
    vertices: &mut Vec<Vertex>,
    lines: &mut Vec<Vertex>,
) {
    for &[a, b, c] in triangles {
        let n = (b - a).cross(c - a).normalize();
        let shade = match style {
            MeshStyle::Wall => {
                if n.z > 0.999 {
                    1.
                } else if n.z < -0.999 {
                    0.55
                } else {
                    0.65 + 0.2 * n.dot(glam::DVec3::new(0.6, -0.8, 0.)).abs() as f32
                }
            }
            MeshStyle::Roof => 1.,
            MeshStyle::Stair => 0.82,
            MeshStyle::Section => {
                (0.55 + 0.45 * n.dot(glam::DVec3::new(0.3, -0.4, 0.866)).max(0.)) as f32
            }
        };
        push_triangle(vertices, a, b, c, base.map(|v| v * shade));
    }
    let shade = match style {
        MeshStyle::Wall => 0.42,
        MeshStyle::Stair => 0.45,
        _ => 0.48,
    };
    for &[a, b] in edges {
        lines.extend([
            vertex(a, base.map(|v| v * shade)),
            vertex(b, base.map(|v| v * shade)),
        ]);
    }
}

// Legacy test entry points delegate to the actual checked consumer. Panes retain
// the host selection convention used by these older render fixtures.
#[cfg(test)]
fn append_wall_with_openings(
    project: &Project,
    wall: &Element,
    _openings: &[&Element],
    clip: Option<&MarqueeArea>,
    base: [f32; 3],
    vertices: &mut Vec<Vertex>,
    lines: &mut Vec<Vertex>,
) {
    let mut query = crate::wall_solid::WallQuery::new(project);
    append_element_checked(project, wall, clip, base, vertices, lines, &mut query).unwrap();
    for e in &project.elements {
        if matches!(e.shape,Shape::Opening{host_wall,..} if host_wall==wall.id) {
            append_element_checked(project, e, clip, base, vertices, lines, &mut query).unwrap();
        }
    }
}
#[cfg(test)]
fn append_contour_wall(
    project: &Project,
    wall: &Element,
    base: [f32; 3],
    clip: Option<&MarqueeArea>,
    vertices: &mut Vec<Vertex>,
    lines: &mut Vec<Vertex>,
) {
    append_wall_with_openings(project, wall, &[], clip, base, vertices, lines);
}
#[cfg(test)]
fn append_clipped_prism(
    project: &Project,
    element: &Element,
    marquee: &MarqueeArea,
    base: [f32; 3],
    vertices: &mut Vec<Vertex>,
    lines: &mut Vec<Vertex>,
) {
    let mut query = crate::wall_solid::WallQuery::new(project);
    append_element_checked(
        project,
        element,
        Some(marquee),
        base,
        vertices,
        lines,
        &mut query,
    )
    .unwrap();
}
#[cfg(test)]
fn append_column_snapshot(
    solid: &crate::column_geometry::ColumnSnapshot,
    clip: Option<&MarqueeArea>,
    base: [f32; 3],
    vertices: &mut Vec<Vertex>,
    lines: &mut Vec<Vertex>,
) {
    let mesh = solid
        .clipped(
            clip.map(|a| a.vertices.as_slice()),
            clip.and_then(|a| a.z_range),
        )
        .unwrap();
    append_lit_mesh(&mesh, base, MeshStyle::Section, vertices, lines);
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct CpuScene {
    vertices: Vec<Vertex>,
    lines: Vec<Vertex>,
    grid_vertices: usize,
    pub diagnostics: Vec<GeometryDiagnostic>,
}
impl CpuScene {
    /// Actual displayed model geometry; construction-grid padding is never a fit target.
    pub fn framing_points(&self) -> impl Iterator<Item = glam::DVec3> + '_ {
        self.vertices
            .iter()
            .chain(self.lines.iter().skip(self.grid_vertices))
            .map(|v| glam::DVec3::from_array(v.position))
            .filter(|p| p.is_finite())
    }
    /// Deterministic evidence for headless comparisons of the actual CPU output.
    /// This does not measure GPU rendering or participate in cache identity.
    pub fn content_fingerprint(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        for list in [&self.vertices, &self.lines] {
            hash.update((list.len() as u64).to_le_bytes());
            for vertex in list {
                for value in vertex.position {
                    hash.update(value.to_le_bytes());
                }
                for value in vertex.color {
                    hash.update(value.to_le_bytes());
                }
            }
        }
        hash.update(format!("{:?}", self.diagnostics).as_bytes());
        hash.finalize().into()
    }
    pub fn triangle_count(&self) -> usize {
        self.vertices.len() / 3
    }
    pub fn line_count(&self) -> usize {
        self.lines.len() / 2
    }
}
#[derive(Clone, Copy)]
pub struct SceneView<'a> {
    pub selected: &'a [u64],
    pub hovered: Option<u64>,
    pub story: Option<u32>,
    pub marquee: Option<&'a MarqueeArea>,
    pub outside_marquee: bool,
    pub selection_only: bool,
}
pub fn prepare_scene(
    p: &Project,
    view: SceneView<'_>,
    mut cached: Option<&mut crate::derivation_cache::GeometryRead<'_>>,
) -> CpuScene {
    if cached
        .as_deref()
        .is_some_and(|read| !read.matches_project(p))
    {
        return CpuScene {
            diagnostics: vec![GeometryDiagnostic::error(
                0,
                "GEOMETRY_READ_PROJECT_MISMATCH",
                "geometry",
                "Geometry read belongs to a different Project",
            )],
            ..Default::default()
        };
    }
    let SceneView {
        selected,
        hovered,
        story,
        marquee,
        outside_marquee,
        selection_only,
    } = view;
    let mut diagnostics = Vec::new();
    let valid_view = match crate::geometry_validation::validate_clip_context(
        marquee.map(|a| a.vertices.as_slice()),
        marquee.and_then(|a| a.z_range),
    ) {
        Ok(()) => true,
        Err(message) => {
            diagnostics.push(GeometryDiagnostic::error(
                0,
                "INVALID_VIEW_CLIP",
                "clipping",
                message,
            ));
            false
        }
    };
    let mut vertices = vec![];
    let mut lines = vec![];
    // Construction grid on the ground plane, derived from the same
    // settings as the floor plan (rotation, cycle and colors).
    let grid = &p.grid_settings;
    if grid.show_construction_grid {
        let mut min_u = f64::INFINITY;
        let mut max_u = f64::NEG_INFINITY;
        let mut min_v = f64::INFINITY;
        let mut max_v = f64::NEG_INFINITY;
        let mut base_z = f64::INFINITY;
        for element in p
            .elements
            .iter()
            .filter(|element| story.is_none_or(|story_id| element.story == story_id))
        {
            for corner in crate::geometry::element_footprint(p, element) {
                let local = grid.local_point(corner);
                min_u = min_u.min(local.x);
                max_u = max_u.max(local.x);
                min_v = min_v.min(local.y);
                max_v = max_v.max(local.y);
            }
        }
        for value in p.stories.iter().filter_map(|item| {
            story
                .is_none_or(|story_id| item.id == story_id)
                .then_some(item.elevation)
        }) {
            base_z = base_z.min(value);
        }
        if !min_u.is_finite() {
            (min_u, max_u, min_v, max_v) = (-10.0, 25.0, -10.0, 25.0);
        }
        if !base_z.is_finite() {
            base_z = 0.0;
        }
        let pad = ((max_u - min_u).max(max_v - min_v) * 0.25).clamp(5.0, 40.0);
        let z = base_z - 0.02;
        let line_rgb = |main: bool| {
            let color = grid.line_color(main);
            let mut rgb = [
                color[0] as f32 / 255.0,
                color[1] as f32 / 255.0,
                color[2] as f32 / 255.0,
            ];
            if !main {
                for channel in &mut rgb {
                    *channel += (0.97 - *channel) * 0.55;
                }
            }
            rgb
        };
        for line in grid.lines_in_local(min_u - pad, max_u + pad, min_v - pad, max_v + pad) {
            let color = line_rgb(line.main);
            lines.push(Vertex {
                position: [line.start.x, line.start.y, z],
                color,
            });
            lines.push(Vertex {
                position: [line.end.x, line.end.y, z],
                color,
            });
        }
    }
    let grid_line_count = lines.len();
    let mut wall_queries = crate::wall_solid::WallQuery::new(p);
    let mut merged_wall_ids = Vec::new();
    let walls_with_openings: std::collections::HashSet<u64> = p
        .elements
        .iter()
        .filter_map(|element| match element.shape {
            Shape::Opening { host_wall, .. } => Some(host_wall),
            _ => None,
        })
        .collect();
    let join_3d_display = marquee.is_none() && !outside_marquee && !selection_only;
    let stories: Vec<u32> = story.map_or_else(
        || p.stories.iter().map(|item| item.id).collect(),
        |id| vec![id],
    );
    for story_id in stories {
        if !valid_view {
            continue;
        }
        // Wall joins are physical geometry and must not depend on the
        // material batches used for colors and surfaces. Compute one
        // footprint set for every wall on the story first.
        let story_walls: Vec<_> = p
            .elements
            .iter()
            .filter(|element| {
                element.story == story_id
                    && matches!(element.shape, Shape::Wall { .. })
                    && element.wall_contour.is_none()
            })
            .collect();
        let wall_context = if let Some(read) = cached.as_deref() {
            read.wall_context(story_id)
        } else {
            wall_queries.context(story_id).ok()
        };
        let Some(wall_context) = wall_context else {
            continue;
        };
        let story_polygons_by_id = &wall_context.joined_footprints;
        for material in Material::ALL {
            let walls: Vec<_> = story_walls
                .iter()
                .copied()
                .filter(|element| element.material == material)
                .filter(|element| crate::geometry::model_element_layer_visible(p, element))
                .collect();
            if walls.is_empty() {
                continue;
            }
            if join_3d_display
                && !walls.iter().any(|wall| {
                    selected.contains(&wall.id)
                        || hovered == Some(wall.id)
                        || walls_with_openings.contains(&wall.id)
                })
            {
                for (bottom, top, indices) in wall_level_groups(p, &walls) {
                    if indices.len() < 2 {
                        continue;
                    }
                    let polygons: Vec<_> = indices
                        .iter()
                        .map(|index| story_polygons_by_id[&walls[*index].id].clone())
                        .collect();
                    let outlines = crate::geometry::wall_outline_paths_from_segments(
                        crate::geometry::wall_polygon_outline_segments(&polygons),
                    );
                    // A single closed loop is the common connected L/T
                    // junction case. Groups with separate loops or holes
                    // retain per-element geometry.
                    if outlines.len() == 1
                        && outlines[0].len() > 3
                        && outlines[0][0].distance(*outlines[0].last().unwrap()) <= 1e-3
                    {
                        if append_joined_wall_prism(
                            bottom,
                            top,
                            material,
                            outlines.into_iter().next().unwrap(),
                            &mut vertices,
                            &mut lines,
                        ) {
                            merged_wall_ids.extend(indices.iter().map(|index| walls[*index].id));
                        }
                    }
                }
            }
        }
    }
    let filter = crate::geometry::ModelViewFilter {
        story,
        selected,
        selection_only,
        marquee,
        outside_marquee,
    };
    for e in &p.elements {
        if !valid_view {
            continue;
        }
        let included = if let Some(read) = cached.as_deref_mut() {
            let opening_host = if let Shape::Opening { host_wall, .. } = e.shape {
                Some(host_wall)
            } else {
                None
            };
            let eligible_filter = if opening_host.is_some() {
                crate::geometry::ModelViewFilter {
                    selection_only: false,
                    ..filter
                }
            } else {
                filter
            };
            match read.model_includes_valid_clip(e.id, eligible_filter) {
                Ok(included) => {
                    included
                        && (opening_host.is_none()
                            || !selection_only
                            || selected.contains(&e.id)
                            || opening_host.is_some_and(|host| selected.contains(&host)))
                }
                Err(d) => {
                    diagnostics.push(d);
                    false
                }
            }
        } else {
            if let Shape::Opening { host_wall, .. } = e.shape {
                crate::geometry::ModelViewFilter {
                    selection_only: false,
                    ..filter
                }
                .includes(p, e)
                    && (!selection_only
                        || selected.contains(&e.id)
                        || selected.contains(&host_wall))
            } else {
                filter.includes(p, e)
            }
        };
        if !included {
            continue;
        }
        if join_3d_display && merged_wall_ids.contains(&e.id) {
            continue;
        }
        let base = if matches!(e.shape, Shape::Zone { .. }) {
            let color = crate::zones::surfaces::element_color(e);
            if hovered == Some(e.id) {
                color.map(|v| v * 0.8 + 0.2)
            } else if selected.contains(&e.id) {
                color.map(|v| v * 0.9 + 0.1)
            } else {
                color
            }
        } else if hovered == Some(e.id) {
            [0.55, 0.8, 1.]
        } else if selected.contains(&e.id) {
            [0.31, 0.70, 0.70]
        } else {
            crate::zones::surfaces::element_color(e)
        };
        let clip = marquee.filter(|_| !outside_marquee && !selection_only);
        let result = if let Some(read) = cached.as_deref_mut() {
            read.source(e.id).and_then(|handle| {
                append_source_checked(&handle.source, clip, base, &mut vertices, &mut lines)
            })
        } else {
            append_element_checked(
                p,
                e,
                clip,
                base,
                &mut vertices,
                &mut lines,
                &mut wall_queries,
            )
        };
        if let Err(diagnostic) = result {
            diagnostics.push(diagnostic);
        }
    }
    if valid_view && !selection_only && p.plan_options.active_worksheet.is_none() {
        if let Some(terrain) = &p.site.terrain {
            let result = (|| {
                terrain.geometry.validate()?;
                if outside_marquee && let Some(area) = marquee {
                    let inside = terrain.geometry.faces.iter().all(|&f| {
                        terrain.geometry.triangle(f).is_ok_and(|v| {
                            let xy = v.map(|p| Point::new(p[0], p[1]));
                            crate::geometry::path_fully_inside_polygon(&xy, true, &area.vertices)
                                && area.z_range.is_none_or(|(lo, hi)| {
                                    v.iter().all(|p| p[2] >= lo && p[2] <= hi)
                                })
                        })
                    });
                    if inside {
                        return Ok::<(), String>(());
                    }
                }
                let clip = marquee.filter(|_| !outside_marquee);
                let mesh = terrain.geometry.mesh(
                    clip.map(|a| a.vertices.as_slice()),
                    clip.and_then(|a| a.z_range),
                )?;
                append_lit_mesh(
                    &mesh,
                    [0.49, 0.57, 0.38],
                    MeshStyle::Section,
                    &mut vertices,
                    &mut lines,
                );
                Ok(())
            })();
            if let Err(message) = result {
                diagnostics.push(GeometryDiagnostic::error(
                    0,
                    "TERRAIN_DISPLAY_INVALID",
                    "terrain",
                    message,
                ));
            }
        }
    }
    apply_model_style(
        p.plan_options.model_3d_style,
        &mut vertices,
        &mut lines,
        grid_line_count,
    );
    CpuScene {
        vertices,
        lines,
        grid_vertices: grid_line_count,
        diagnostics,
    }
}
fn apply_model_style(
    style: crate::view_settings::Model3dStyle,
    vertices: &mut Vec<Vertex>,
    lines: &mut Vec<Vertex>,
    grid_lines: usize,
) {
    match style {
        crate::view_settings::Model3dStyle::Wireframe => vertices.clear(),
        crate::view_settings::Model3dStyle::Shaded => lines.truncate(grid_lines),
        crate::view_settings::Model3dStyle::ShadedWithEdges => {}
    }
}

#[derive(Clone, Copy, Default, Debug, serde::Serialize)]
pub struct SceneCacheStats {
    pub hits: usize,
    pub builds: usize,
}
/// One bounded last-view result. Source geometry and view/style identity remain
/// separate; changing a presentation setting never invalidates source solids.
pub struct CpuSceneCache {
    enabled: bool,
    key: Option<[u8; 32]>,
    scene: CpuScene,
    stats: SceneCacheStats,
}
impl Default for CpuSceneCache {
    fn default() -> Self {
        Self {
            enabled: true,
            key: None,
            scene: CpuScene::default(),
            stats: SceneCacheStats::default(),
        }
    }
}
impl CpuSceneCache {
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.key = None;
    }
    pub fn stats(&self) -> SceneCacheStats {
        self.stats
    }
    /// Bounded comparison of authored prisms from the authoritative project.
    /// Ignores the active-option filter: both complete candidate versions are
    /// visible. The default call has no selection tint. No second Project.
    pub fn prepare_comparison(
        &mut self,
        p: &Project,
        ids: &[u64],
        read: &mut crate::derivation_cache::GeometryRead<'_>,
    ) -> &CpuScene {
        self.prepare_comparison_selected(p, ids, &[], read)
    }
    pub fn prepare_comparison_selected(
        &mut self,
        p: &Project,
        ids: &[u64],
        selected: &[u64],
        read: &mut crate::derivation_cache::GeometryRead<'_>,
    ) -> &CpuScene {
        let invalid = |id, message: String| {
            GeometryDiagnostic::error(id, "COMPARISON_SOURCE_INVALID", "comparison", message)
        };
        let key = (|| {
            if !read.matches_project(p) {
                return Err(invalid(
                    0,
                    "Geometry read belongs to a different Project".into(),
                ));
            }
            if ids.is_empty() || ids.len() > 32 || ids.windows(2).any(|w| w[0] >= w[1]) {
                return Err(invalid(
                    0,
                    "Candidate 3D preview needs 1–32 unique ascending object IDs".into(),
                ));
            }
            let mut inputs = Vec::with_capacity(ids.len());
            for id in ids {
                let e = p
                    .elements
                    .iter()
                    .find(|e| e.id == *id)
                    .ok_or_else(|| invalid(*id, "Comparison object is missing".into()))?;
                if !matches!(e.shape, Shape::ConceptMass { .. } | Shape::Zone { .. }) {
                    return Err(invalid(
                        *id,
                        "Candidate 3D preview supports concept masses and Zones only".into(),
                    ));
                }
                inputs.push((
                    *id,
                    read.source(*id)?.identity.input_fingerprint,
                    e.material,
                ));
            }
            crate::derived_geometry::input_fingerprint(
                &(
                    "candidate_comparison_selection_v2",
                    selected,
                    inputs,
                    p.plan_options.model_3d_style,
                ),
                0.,
            )
            .map_err(|m| invalid(0, m))
        })();
        let key = match key {
            Ok(key) => key,
            Err(d) => {
                self.key = None;
                self.scene = CpuScene {
                    diagnostics: vec![d],
                    ..Default::default()
                };
                return &self.scene;
            }
        };
        if self.enabled && self.key == Some(key) {
            self.stats.hits += 1;
            return &self.scene;
        }
        let mut scene = CpuScene::default();
        for id in ids {
            let e = p
                .elements
                .iter()
                .find(|e| e.id == *id)
                .expect("validated comparison object");
            let result = read.source(*id).and_then(|g| {
                append_source_checked(
                    &g.source,
                    None,
                    if selected.contains(id) {
                        [0.18, 0.68, 0.40]
                    } else {
                        crate::zones::surfaces::element_color(e)
                    },
                    &mut scene.vertices,
                    &mut scene.lines,
                )
            });
            if let Err(d) = result {
                scene.diagnostics.push(d);
            }
        }
        apply_model_style(
            p.plan_options.model_3d_style,
            &mut scene.vertices,
            &mut scene.lines,
            0,
        );
        if !scene.diagnostics.is_empty() {
            scene.vertices.clear();
            scene.lines.clear();
        }
        self.scene = scene;
        self.key = Some(key);
        self.stats.builds += 1;
        &self.scene
    }
    pub fn prepare(
        &mut self,
        p: &Project,
        view: SceneView<'_>,
        read: &mut crate::derivation_cache::GeometryRead<'_>,
    ) -> &CpuScene {
        if !read.matches_project(p) {
            self.key = None;
            self.scene = CpuScene {
                diagnostics: vec![GeometryDiagnostic::error(
                    0,
                    "GEOMETRY_READ_PROJECT_MISMATCH",
                    "geometry",
                    "Geometry read belongs to a different Project",
                )],
                ..Default::default()
            };
            return &self.scene;
        }
        let styles: Vec<_> = p
            .elements
            .iter()
            .map(|e| {
                (
                    &e.layer,
                    e.material,
                    e.worksheet,
                    e.renovation,
                    e.design_option,
                )
            })
            .collect();
        let stories: Vec<_> = p.stories.iter().map(|s| (s.id, s.elevation)).collect();
        let clip = view.marquee.map(|m| {
            (
                &m.vertices,
                m.z_range,
                m.story,
                m.scope == crate::editor::MarqueeScope::AllStories,
            )
        });
        let key = crate::derived_geometry::input_fingerprint(
            &(
                read.input_keys(),
                crate::column_clip::CLIP_ALGORITHM_VERSION,
                styles,
                stories,
                &p.layers,
                &p.grid_settings,
                &p.site.terrain.as_ref().map(|t| &t.geometry),
                &p.plan_options,
                &p.design_options,
                view.selected,
                view.hovered,
                view.story,
                clip,
                view.outside_marquee,
                view.selection_only,
            ),
            0.,
        );
        let key = match key {
            Ok(key) => key,
            Err(message) => {
                self.key = None;
                self.scene = CpuScene {
                    diagnostics: vec![GeometryDiagnostic::error(
                        0,
                        "SCENE_INPUT_IDENTITY",
                        "display",
                        message,
                    )],
                    ..Default::default()
                };
                return &self.scene;
            }
        };
        if self.enabled && self.key == Some(key) {
            self.stats.hits += 1;
            return &self.scene;
        }
        self.scene = prepare_scene(p, view, Some(read));
        self.key = Some(key);
        self.stats.builds += 1;
        &self.scene
    }
}
pub struct SceneRenderer {
    scene: CpuSceneCache,
    cad: CadGpuCache,
    state: RenderState,
    pipeline: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    cad_xray: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
    multisample: wgpu::TextureView,
    pub texture: egui::TextureId,
    pub diagnostics: Vec<GeometryDiagnostic>,
    size: [u32; 2],
}
impl SceneRenderer {
    pub fn new(state: RenderState) -> Self {
        let device = &state.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("OpenArchi scene"),
            source: wgpu::ShaderSource::Wgsl(include_str!("scene.wgsl").into()),
        });
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera"),
            contents: bytemuck::cast_slice(&glam::Mat4::IDENTITY.to_cols_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let cad_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("CAD camera"),
            contents: bytemuck::cast_slice(&glam::Mat4::IDENTITY.to_cols_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let cad_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("CAD camera"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: cad_uniform.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let make = |topology, write, bias, compare| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Architectural mesh"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: 24,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3],
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(write),
                    depth_compare: Some(compare),
                    stencil: Default::default(),
                    bias: wgpu::DepthBiasState {
                        constant: bias,
                        slope_scale: 0.,
                        clamp: 0.,
                    },
                }),
                multisample: wgpu::MultisampleState {
                    count: 4,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipeline = make(
            wgpu::PrimitiveTopology::TriangleList,
            true,
            1,
            wgpu::CompareFunction::LessEqual,
        );
        let lines = make(
            wgpu::PrimitiveTopology::LineList,
            false,
            0,
            wgpu::CompareFunction::LessEqual,
        );
        let cad_xray = make(
            wgpu::PrimitiveTopology::LineList,
            false,
            0,
            wgpu::CompareFunction::Always,
        );
        let (color, depth, multisample) = Self::targets(device, [2, 2]);
        let texture = state.renderer.write().register_native_texture(
            device,
            &color,
            wgpu::FilterMode::Linear,
        );
        Self {
            scene: CpuSceneCache::default(),
            cad: CadGpuCache {
                key: None,
                lines: [None, None],
                count: [0, 0],
                origin: glam::DVec3::ZERO,
                uniform: cad_uniform,
                bind: cad_bind,
            },
            state,
            pipeline,
            lines,
            cad_xray,
            uniform,
            bind,
            color,
            depth,
            multisample,
            texture,
            diagnostics: Vec::new(),
            size: [2, 2],
        }
    }
    fn targets(
        d: &wgpu::Device,
        size: [u32; 2],
    ) -> (wgpu::TextureView, wgpu::TextureView, wgpu::TextureView) {
        let make = |format, usage, sample_count| {
            d.create_texture(&wgpu::TextureDescriptor {
                label: Some("Scene target"),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
            .create_view(&Default::default())
        };
        (
            make(
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                1,
            ),
            make(
                wgpu::TextureFormat::Depth32Float,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
                4,
            ),
            make(
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
                4,
            ),
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        p: &Project,
        selected: &[u64],
        hovered: Option<u64>,
        camera: &Camera,
        size: [u32; 2],
        story: Option<u32>,
        marquee: Option<&MarqueeArea>,
        outside_marquee: bool,
        selection_only: bool,
    ) {
        self.render_geometry(
            p,
            selected,
            hovered,
            camera,
            size,
            story,
            marquee,
            outside_marquee,
            selection_only,
            None,
            None,
        );
    }
    #[allow(clippy::too_many_arguments)]
    pub fn render_cached(
        &mut self,
        p: &Project,
        selected: &[u64],
        hovered: Option<u64>,
        camera: &Camera,
        size: [u32; 2],
        story: Option<u32>,
        marquee: Option<&MarqueeArea>,
        outside_marquee: bool,
        selection_only: bool,
        read: &mut crate::derivation_cache::GeometryRead<'_>,
    ) {
        self.render_geometry(
            p,
            selected,
            hovered,
            camera,
            size,
            story,
            marquee,
            outside_marquee,
            selection_only,
            Some(read),
            None,
        );
    }
    /// An independent texture on the same device and the same rendering code.
    pub fn sibling(&self) -> Self {
        Self::new(self.state.clone())
    }
    pub fn render_comparison(
        &mut self,
        p: &Project,
        ids: &[u64],
        selected: &[u64],
        camera: &Camera,
        size: [u32; 2],
        read: &mut crate::derivation_cache::GeometryRead<'_>,
    ) {
        self.render_geometry(
            p,
            selected,
            None,
            camera,
            size,
            None,
            None,
            false,
            false,
            Some(read),
            Some(ids),
        );
    }
    #[allow(clippy::too_many_arguments)]
    fn render_geometry(
        &mut self,
        p: &Project,
        selected: &[u64],
        hovered: Option<u64>,
        camera: &Camera,
        size: [u32; 2],
        story: Option<u32>,
        marquee: Option<&MarqueeArea>,
        outside_marquee: bool,
        selection_only: bool,
        cached: Option<&mut crate::derivation_cache::GeometryRead<'_>>,
        comparison_ids: Option<&[u64]>,
    ) {
        let size = [size[0].clamp(2, 4096), size[1].clamp(2, 4096)];
        let d = &self.state.device;
        if size != self.size {
            (self.color, self.depth, self.multisample) = Self::targets(d, size);
            self.state
                .renderer
                .write()
                .update_egui_texture_from_wgpu_texture(
                    d,
                    &self.color,
                    wgpu::FilterMode::Linear,
                    self.texture,
                );
            self.size = size;
        }
        let origin = camera.target;
        let mut local_camera = camera.clone();
        local_camera.target = glam::DVec3::ZERO;
        self.state.queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::cast_slice(
                &local_camera
                    .matrix(size[0] as f64 / size[1] as f64)
                    .as_mat4()
                    .to_cols_array(),
            ),
        );
        let view = SceneView {
            selected,
            hovered,
            story,
            marquee,
            outside_marquee,
            selection_only,
        };
        let uncached;
        let scene = if let Some(read) = cached {
            if let Some(ids) = comparison_ids {
                self.scene
                    .prepare_comparison_selected(p, ids, selected, read)
            } else {
                self.scene.prepare(p, view, read)
            }
        } else {
            uncached = prepare_scene(p, view, None);
            &uncached
        };
        self.diagnostics = scene.diagnostics.clone();
        let vertices = &scene.vertices;
        let lines = &scene.lines;
        let gpu_triangles = gpu_vertices(vertices, origin);
        let gpu_lines = gpu_vertices(lines, origin);
        let vb = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Scene triangles"),
            contents: bytemuck::cast_slice(&gpu_triangles),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let lb = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Scene edges"),
            contents: bytemuck::cast_slice(&gpu_lines),
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.cad
            .prepare(&self.state, p, camera, size, comparison_ids.is_none());
        let mut encoder = d.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Model viewport"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.multisample,
                    depth_slice: None,
                    resolve_target: Some(&self.color),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.89,
                            g: 0.90,
                            b: 0.88,
                            a: 1.,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.bind, &[]);
            if !vertices.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_vertex_buffer(0, vb.slice(..));
                pass.draw(0..vertices.len() as u32, 0..1);
            }
            if !lines.is_empty() {
                pass.set_pipeline(&self.lines);
                pass.set_vertex_buffer(0, lb.slice(..));
                pass.draw(0..lines.len() as u32, 0..1);
            }
            for group in 0..2 {
                if let Some(buffer) = &self.cad.lines[group] {
                    pass.set_bind_group(0, &self.cad.bind, &[]);
                    pass.set_pipeline(if group == 0 {
                        &self.lines
                    } else {
                        &self.cad_xray
                    });
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(0..self.cad.count[group], 0..1);
                }
            }
        }
        self.state.queue.submit([encoder.finish()]);
    }
}

impl Drop for SceneRenderer {
    fn drop(&mut self) {
        self.state.renderer.write().free_texture(&self.texture);
    }
}

#[cfg(test)]
mod clipping_tests {
    use super::*;
    use crate::{
        editor::MarqueeScope,
        model::{Material, Point, Shape},
    };

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires native Metal GPU access; run explicitly with --ignored"]
    fn metal_scene_survives_empty_project_story_and_selection_then_draws_again() {
        use std::{
            future::Future,
            sync::Arc,
            task::{Context, Poll, Wake, Waker},
            time::{Duration, Instant},
        };
        struct ThreadWake(std::thread::Thread);
        impl Wake for ThreadWake {
            fn wake(self: Arc<Self>) {
                self.0.unpark();
            }
        }
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let config = eframe::egui_wgpu::WgpuConfiguration::default();
        let mut future = std::pin::pin!(RenderState::create(
            &config,
            &instance,
            None,
            Default::default()
        ));
        let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
        let mut context = Context::from_waker(&waker);
        let deadline = Instant::now() + Duration::from_secs(20);
        let state = loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(result) => break result.expect("Metal scene test needs a working GPU"),
                Poll::Pending => {
                    assert!(Instant::now() < deadline, "Metal device creation timed out");
                    std::thread::park_timeout(Duration::from_millis(10));
                }
            }
        };
        let mut renderer = SceneRenderer::new(state);
        let mut populated = Project::default();
        populated.add(
            0,
            Material::Concrete,
            Shape::Column {
                center: Point::default(),
                section: crate::column::ColumnSection::Rectangular {
                    width: 0.4,
                    depth: 0.4,
                },
                height: 3.,
                bottom_offset: 0.,
                axis_rotation: 0.,
                inclination: None,
                plan: Default::default(),
            },
        );
        let empty = Project::default();
        let mut slab_editor = crate::editor::Editor::new(Project::default());
        slab_editor
            .place_slab_contour(crate::model::SlabContour {
                points: vec![
                    Point::new(0., 0.),
                    Point::new(3., 4.),
                    Point::new(7., 1.),
                    Point::new(4., -3.),
                ],
                ..Default::default()
            })
            .unwrap();
        let mut grid_only = Project::default();
        grid_only.grid_settings.show_construction_grid = true;
        for (project, story, selection_only) in [
            (&populated, None, false),
            (&slab_editor.project, None, false),
            (&empty, None, false),
            (&populated, Some(1), false),
            (&populated, None, true),
            (&grid_only, None, false),
            (&populated, None, false),
        ] {
            renderer.render(
                project,
                &[],
                None,
                &Camera::default(),
                [64, 64],
                story,
                None,
                false,
                selection_only,
            );
            renderer
                .state
                .device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(Duration::from_secs(10)),
                })
                .expect("Scene submission must complete without GPU validation errors");
        }
    }

    #[test]
    fn clipped_opening_wall_preserves_holes_glazing_and_cut_cap_area() {
        for kind in [OpeningKind::Door, OpeningKind::Window] {
            let mut e = crate::editor::Editor::new(Project::default());
            e.thickness = 0.2;
            e.place_wall_segments(vec![(Point::new(0., 0.), Point::new(6., 0.))])
                .unwrap();
            let host = e.project.elements[0].id;
            e.place_opening_on_wall(
                kind,
                host,
                3.,
                crate::model::OpeningDefaults {
                    width: 2.,
                    height: 2.1,
                    sill: 0.,
                },
            )
            .unwrap();
            let wall = &e.project.elements[0];
            let openings = vec![&e.project.elements[1]];
            for shape in 0..3 {
                let polygon = match shape {
                    0 => vec![
                        Point::new(1., -1.),
                        Point::new(5., -1.),
                        Point::new(5., 1.),
                        Point::new(1., 1.),
                    ],
                    1 => vec![
                        Point::new(1., -1.),
                        Point::new(5., -1.),
                        Point::new(5., 1.),
                        Point::new(4.5, 1.),
                        Point::new(4.5, 0.),
                        Point::new(1., 0.),
                    ],
                    _ => vec![
                        Point::new(2.5, 0.05),
                        Point::new(3.5, 0.05),
                        Point::new(3.5, 0.09),
                        Point::new(2.5, 0.09),
                    ],
                };
                let area = MarqueeArea {
                    vertices: polygon,
                    story: 0,
                    scope: MarqueeScope::AllStories,
                    z_range: Some((0.5, 1.5)),
                };
                let mut vertices = Vec::new();
                let mut lines = Vec::new();
                append_wall_with_openings(
                    &e.project,
                    wall,
                    &openings,
                    Some(&area),
                    [1.; 3],
                    &mut vertices,
                    &mut lines,
                );
                if shape == 2 {
                    assert!(
                        vertices.is_empty(),
                        "clip wholly within the hole, away from glazing"
                    );
                    assert!(lines.is_empty());
                    continue;
                }
                let mut cap_area = 0.;
                for tri in vertices.chunks_exact(3) {
                    let points: Vec<_> = tri
                        .iter()
                        .map(|v| glam::DVec3::from_array(v.position))
                        .collect();
                    assert!(points.iter().all(|p| p.z >= 0.5 - 1e-5
                        && p.z <= 1.5 + 1e-5
                        && p.x >= 1. - 1e-5
                        && p.x <= 5. + 1e-5));
                    let center = (points[0] + points[1] + points[2]) / 3.;
                    if center.x > 2. + 1e-5 && center.x < 4. - 1e-5 {
                        let jamb = (center.x <= 2.05001 || center.x >= 3.94999)
                            && center.y.abs() <= 0.04001;
                        let fitting = if kind == OpeningKind::Window {
                            center.y.abs() <= 0.00801
                        } else {
                            (2.02999..=2.07001).contains(&center.x) && center.y >= 0.03999
                        };
                        assert!(
                            jamb || fitting,
                            "only native joinery or glazing may occupy the wall void: {center:?}"
                        );
                    }
                    if points.iter().all(|p| (p.z - 1.5).abs() < 1e-5) {
                        cap_area += (points[1] - points[0])
                            .cross(points[2] - points[0])
                            .length()
                            * 0.5;
                    }
                }
                let expected = if shape == 0 { 0.4 } else { 0.25 }
                    + if kind == OpeningKind::Window {
                        if shape == 0 { 0.0384 } else { 0.0192 }
                    } else {
                        if shape == 0 { 0.0464 } else { 0.004 }
                    };
                assert!(
                    (cap_area - expected).abs() < 1e-5,
                    "cap {cap_area}, expected {expected}"
                );
            }
        }
    }

    #[test]
    fn circular_column_marquee_clips_real_disk_and_height() {
        let mut editor = crate::editor::Editor::new(Project::default());
        editor
            .place_column_section(
                Point::default(),
                crate::column::ColumnSection::Circular { diameter: 2. },
                3.,
            )
            .unwrap();
        let area = MarqueeArea {
            vertices: vec![
                Point::new(0., -2.),
                Point::new(2., -2.),
                Point::new(2., 2.),
                Point::new(0., 2.),
            ],
            story: 0,
            scope: MarqueeScope::AllStories,
            z_range: Some((0.5, 2.5)),
        };
        let mut vertices = Vec::new();
        let mut lines = Vec::new();
        append_clipped_prism(
            &editor.project,
            &editor.project.elements[0],
            &area,
            [1.; 3],
            &mut vertices,
            &mut lines,
        );
        assert!(!vertices.is_empty());
        let mut cap_area = 0.;
        for tri in vertices.chunks_exact(3) {
            let p: Vec<_> = tri
                .iter()
                .map(|v| glam::DVec3::from_array(v.position))
                .collect();
            assert!(p.iter().all(|v| v.x >= -1e-5
                && v.x * v.x + v.y * v.y <= 1.00001
                && v.z >= 0.5
                && v.z <= 2.5));
            if p.iter().all(|v| (v.z - 2.5).abs() < 1e-5) {
                cap_area += (p[1] - p[0]).cross(p[2] - p[0]).length() / 2.;
            }
        }
        assert!((cap_area - std::f64::consts::PI / 2.).abs() < 0.003);
    }

    #[test]
    fn slab_concave_prism_cap_area_matches_model_without_bounding_box_fill() {
        let mut project = Project::default();
        project.add(
            0,
            Material::Concrete,
            Shape::Slab {
                min: Point::new(0., 0.),
                max: Point::new(4., 3.),
                thickness: 0.3,
            },
        );
        crate::slab_geometry::insert_node(&mut project.elements[0], 2, Point::new(2., 2.)).unwrap();
        let mut vertices = Vec::new();
        let mut lines = Vec::new();
        append_slab_prism(
            &project.elements[0],
            -0.3,
            0.,
            [1.; 3],
            &mut vertices,
            &mut lines,
        )
        .unwrap();
        let top_area: f64 = vertices
            .chunks_exact(3)
            .filter(|t| t.iter().all(|v| v.position[2] == 0.))
            .map(|t| {
                let a = glam::DVec3::from_array(t[0].position);
                let b = glam::DVec3::from_array(t[1].position);
                let c = glam::DVec3::from_array(t[2].position);
                (b - a).cross(c - a).length() * 0.5
            })
            .sum();
        assert!((top_area - project.elements[0].area()).abs() < 1e-5);
        assert_eq!(lines.len(), 30);
    }

    #[test]
    fn joined_wall_prism_emits_only_the_exposed_l_outline() {
        let mut project = Project::default();
        project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(0., 0.),
                end: Point::new(3., 0.),
                thickness: 0.2,
                height: 3.,
            },
        );
        project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(3., 0.),
                end: Point::new(3., 2.),
                thickness: 0.2,
                height: 3.,
            },
        );
        let walls: Vec<_> = project.elements.iter().collect();
        let layer_groups = project.layers.wall_groups(&walls);
        let polygons = crate::geometry::plan_wall_polygons(&walls, &layer_groups);
        let outlines = crate::geometry::wall_outline_paths_from_segments(
            crate::geometry::wall_polygon_outline_segments(&polygons),
        );
        assert_eq!(outlines.len(), 1, "an L junction has one exposed perimeter");
        let outline = &outlines[0];
        let unique_count = outline.len() - 1;
        assert!(outline[0].distance(*outline.last().unwrap()) <= 1e-3);
        for index in 0..unique_count {
            let a = outline[index];
            let b = outline[index + 1];
            assert!(
                (a.x - b.x).abs() <= 1e-4 || (a.y - b.y).abs() <= 1e-4,
                "L junction outline has no diagonal bevel edge: {a:?} -> {b:?}"
            );
        }

        let mut triangles = Vec::new();
        let mut edges = Vec::new();
        append_joined_wall_prism(
            0.,
            3.,
            Material::Concrete,
            outline.clone(),
            &mut triangles,
            &mut edges,
        );
        assert_eq!(
            edges.len(),
            unique_count * 6,
            "only exposed perimeter edges are emitted"
        );
        assert_eq!(triangles.len() % 3, 0);
        assert!(!triangles.is_empty());
    }

    #[test]
    fn slanted_beam_render_clipping_preserves_affine_faces() {
        let mut editor = crate::editor::Editor::new(Project::default());
        editor.beam_size = [2., std::f64::consts::FRAC_1_SQRT_2, 0.];
        editor
            .place_slanted_beam(Point::default(), Point::new(4., 0.), 45.)
            .unwrap();
        let marquee = MarqueeArea {
            vertices: vec![
                Point::new(2., -0.5),
                Point::new(3., -0.5),
                Point::new(3., 0.5),
                Point::new(2., 0.5),
            ],
            scope: MarqueeScope::CurrentStory,
            story: 0,
            z_range: Some((1., 2.)),
        };
        let mut triangles = vec![];
        let mut edges = vec![];
        append_clipped_prism(
            &editor.project,
            &editor.project.elements[0],
            &marquee,
            [0.5; 3],
            &mut triangles,
            &mut edges,
        );
        assert!(!triangles.is_empty());
        let volume = triangles
            .chunks_exact(3)
            .map(|t| {
                let [a, b, c] = [t[0].position, t[1].position, t[2].position]
                    .map(|p| glam::DVec3::from_array(p));
                a.dot(b.cross(c)) / 6.
            })
            .sum::<f64>()
            .abs();
        assert!((volume - 0.5).abs() < 1e-5, "{volume}");
        for v in triangles.iter().chain(edges.iter()) {
            let [x, _, z] = v.position;
            assert!(z >= x - 1. - 1e-5 && z <= x + 1e-5 && (1. - 1e-5..=2. + 1e-5).contains(&z));
        }
    }

    #[test]
    fn clipped_prism_is_cut_in_xy_and_z() {
        let mut project = Project::default();
        project.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(-1., 0.),
                end: Point::new(3., 0.),
                thickness: 1.,
                height: 3.,
            },
        );
        let element = &project.elements[0];
        let marquee = MarqueeArea {
            vertices: vec![
                Point::new(0., -1.),
                Point::new(2., -1.),
                Point::new(2., 1.),
                Point::new(0., 1.),
            ],
            scope: MarqueeScope::CurrentStory,
            story: 0,
            z_range: Some((1., 2.)),
        };
        let mut triangles = Vec::new();
        let mut edges = Vec::new();
        append_clipped_prism(
            &project,
            element,
            &marquee,
            [0.7, 0.7, 0.7],
            &mut triangles,
            &mut edges,
        );
        assert!(!triangles.is_empty());
        for v in triangles.iter().chain(edges.iter()) {
            let [x, y, z] = v.position;
            assert!((-1e-4..=2.0001).contains(&x), "x={x}");
            assert!((-1.0001..=1.0001).contains(&y), "y={y}");
            assert!((0.9999..=2.0001).contains(&z), "z={z}");
        }
        assert!(
            triangles.iter().any(|v| v.color != [0.7, 0.7, 0.7]),
            "cut caps should be shaded"
        );
    }
}

#[cfg(test)]
mod contour_render_tests {
    use super::*;

    #[test]
    fn contour_window_glazing_survives_xy_and_z_clipping() {
        let mut e = crate::editor::Editor::new(Project::default());
        e.place_wall_segments(vec![(Point::new(0., 0.), Point::new(4., 0.))])
            .unwrap();
        let host = e.project.elements[0].id;
        e.project.elements[0].wall_contour = Some(WallContour::Arc {
            arc: crate::geometry::arc_segment_through(
                Point::new(0., 0.),
                Point::new(4., 0.),
                Point::new(2., 2.),
            )
            .unwrap(),
        });
        let station = crate::wall_geometry::reference_length(&e.project.elements[0]) * 0.5;
        e.place_opening_on_wall(
            OpeningKind::Window,
            host,
            station,
            crate::model::OpeningDefaults {
                width: 2.,
                height: 2.,
                sill: 0.5,
            },
        )
        .unwrap();
        let wall = &e.project.elements[0];
        let render = |area: Option<&MarqueeArea>| {
            let mut v = Vec::new();
            append_contour_wall(&e.project, wall, [0.7; 3], area, &mut v, &mut Vec::new());
            v.into_iter()
                .filter(|v| v.color == [0.55, 0.74, 0.82])
                .collect::<Vec<_>>()
        };
        let surface_area = |v: &[Vertex]| {
            v.chunks_exact(3)
                .map(|t| {
                    let a = glam::DVec3::from_array(t[0].position);
                    let b = glam::DVec3::from_array(t[1].position);
                    let c = glam::DVec3::from_array(t[2].position);
                    (b - a).cross(c - a).length() * 0.5
                })
                .sum::<f64>()
        };
        let full = render(None);
        assert!(!full.is_empty());
        let mut area = MarqueeArea {
            vertices: vec![
                Point::new(-1., -1.),
                Point::new(5., -1.),
                Point::new(5., 3.),
                Point::new(-1., 3.),
            ],
            story: 0,
            scope: crate::editor::MarqueeScope::AllStories,
            z_range: None,
        };
        let whole = render(Some(&area));
        assert!((surface_area(&whole) - surface_area(&full)).abs() < 1e-5);
        area.z_range = Some((1.8, 0.8));
        let height_clipped = render(Some(&area));
        assert!(!height_clipped.is_empty());
        assert!((surface_area(&height_clipped) / surface_area(&full) - 1. / 1.94).abs() < 1e-5);
        area.vertices = vec![
            Point::new(1.5, -1.),
            Point::new(2.5, -1.),
            Point::new(2.5, 3.),
            Point::new(1.5, 3.),
        ];
        let narrow = render(Some(&area));
        assert!(!narrow.is_empty());
        assert!(surface_area(&narrow) < surface_area(&height_clipped));
        assert!(
            narrow
                .iter()
                .all(|v| (1.49999..=2.50001).contains(&v.position[0])
                    && (0.79999..=1.80001).contains(&v.position[2]))
        );
        area.z_range = Some((4., 5.));
        assert!(render(Some(&area)).is_empty());
        area.z_range = None;
        for p in &mut area.vertices {
            p.x += 20.;
        }
        assert!(render(Some(&area)).is_empty());
    }

    #[test]
    fn contour_mesh_preserves_curvature_and_has_no_tessellation_cap_lines() {
        let mut project = Project::default();
        project.add(
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
            arc: crate::geometry::arc_segment_through(
                Point::new(0., 0.),
                Point::new(4., 0.),
                Point::new(2., 2.),
            )
            .unwrap(),
        });
        let mut vertices = Vec::new();
        let mut lines = Vec::new();
        append_contour_wall(
            &project,
            &project.elements[0],
            [0.7; 3],
            None,
            &mut vertices,
            &mut lines,
        );
        assert!(!vertices.is_empty());
        assert!(vertices.iter().any(|v| v.position[1] > 1.9));
        let vertical_lines = lines
            .chunks_exact(2)
            .filter(|pair| {
                let a = pair[0].position;
                let b = pair[1].position;
                (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5 && (a[2] - b[2]).abs() > 1.
            })
            .count();
        assert_eq!(
            vertical_lines, 4,
            "only the two ends have vertical cap edges"
        );
        assert!(
            vertices
                .iter()
                .all(|v| v.position.iter().all(|x| x.is_finite()))
        );
    }

    #[test]
    fn contour_mesh_clips_to_marquee_xy_and_z() {
        let mut project = Project::default();
        project.add(
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
            arc: crate::geometry::arc_segment_through(
                Point::new(0., 0.),
                Point::new(4., 0.),
                Point::new(2., 2.),
            )
            .unwrap(),
        });
        let area = MarqueeArea {
            vertices: vec![
                Point::new(1., -1.),
                Point::new(3., -1.),
                Point::new(3., 3.),
                Point::new(1., 3.),
            ],
            scope: crate::editor::MarqueeScope::CurrentStory,
            story: 0,
            z_range: Some((1., 2.)),
        };
        let mut vertices = Vec::new();
        let mut lines = Vec::new();
        append_contour_wall(
            &project,
            &project.elements[0],
            [0.7; 3],
            Some(&area),
            &mut vertices,
            &mut lines,
        );
        assert!(!vertices.is_empty());
        assert!(
            vertices
                .iter()
                .chain(&lines)
                .all(|v| (0.9999..=3.0001).contains(&v.position[0])
                    && (0.9999..=2.0001).contains(&v.position[2]))
        );
    }
}

#[cfg(test)]
mod wall_polygon_render_tests {
    use super::*;

    #[test]
    fn polygon_prism_preserves_concave_hub_and_actual_vertical_levels() {
        let polygon = vec![
            Point::new(0., 0.),
            Point::new(4., 0.),
            Point::new(4., 0.4),
            Point::new(0.4, 0.4),
            Point::new(0.2, 0.2),
            Point::new(0., 0.4),
        ];
        let mut vertices = vec![];
        let mut lines = vec![];
        let selected = [0.31, 0.70, 0.70];
        let edge = [0.07, 0.40, 0.42];
        append_wall_polygon_prism(
            &polygon,
            1.25,
            5.5,
            selected,
            edge,
            &mut vertices,
            &mut lines,
        );
        let top_area: f64 = vertices
            .chunks_exact(3)
            .filter(|tri| tri.iter().all(|v| v.position[2] == 5.5))
            .map(|tri| {
                let a = tri[0].position;
                let b = tri[1].position;
                let c = tri[2].position;
                ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() * 0.5
            })
            .sum();
        assert!((top_area - 1.56).abs() < 1e-5, "{top_area}");
        assert_eq!(lines.len(), polygon.len() * 6);
        for v in vertices.iter().chain(lines.iter()) {
            assert!(v.position[2] == 1.25 || v.position[2] == 5.5);
            assert!(polygon.contains(&Point::new(v.position[0], v.position[1])));
        }
        assert!(vertices.iter().any(|v| v.position == [0.2, 0.2, 5.5]));
        assert!(lines.iter().all(|v| v.color == edge));
        assert!(
            vertices
                .iter()
                .any(|v| v.position[2] == 5.5 && v.color == selected)
        );
    }

    #[test]
    fn junction_selected_and_mixed_material_meshes_use_every_plan_polygon_vertex() {
        let mut p = Project::default();
        for (angle, material) in [
            (0_f64, Material::Concrete),
            (120., Material::Timber),
            (240., Material::Concrete),
        ] {
            let a = angle.to_radians();
            p.add(
                0,
                material,
                Shape::Wall {
                    start: Point::default(),
                    end: Point::new(a.cos() * 4., a.sin() * 4.),
                    thickness: 0.4,
                    height: 3.,
                },
            );
        }
        let walls: Vec<_> = p.elements.iter().collect();
        let polygons = crate::geometry::plan_wall_polygons(&walls, &p.layers.wall_groups(&walls));
        assert!(
            polygons.iter().any(|polygon| polygon.len() > 4),
            "junction must exercise the non-quad route"
        );
        for (wall, polygon) in walls.iter().zip(&polygons) {
            for selected in [false, true] {
                let base = if selected {
                    [0.31, 0.70, 0.70]
                } else {
                    wall.material.color()
                };
                let mut vertices = vec![];
                let mut lines = vec![];
                append_wall_polygon_prism(
                    polygon,
                    0.,
                    3.,
                    base,
                    base.map(|v| v * 0.42),
                    &mut vertices,
                    &mut lines,
                );
                for point in polygon {
                    assert!(
                        vertices
                            .iter()
                            .any(|v| v.position == [point.x, point.y, 3.])
                    );
                }
                assert!(
                    vertices
                        .iter()
                        .chain(&lines)
                        .all(|v| polygon.contains(&Point::new(v.position[0], v.position[1])))
                );
                assert_eq!(lines.len(), polygon.len() * 6);
            }
        }
        let outlines = crate::geometry::wall_outline_paths_from_segments(
            crate::geometry::wall_polygon_outline_segments(&polygons),
        );
        assert_eq!(outlines.len(), 1);
        let mut vertices = vec![];
        let mut lines = vec![];
        append_joined_wall_prism(
            0.,
            3.,
            Material::Concrete,
            outlines[0].clone(),
            &mut vertices,
            &mut lines,
        );
        assert!(
            vertices
                .iter()
                .all(|v| v.position.iter().all(|x| x.is_finite()))
        );
    }

    #[test]
    fn joined_wall_groups_respect_bottom_and_top_offsets() {
        let mut p = Project::default();
        for i in 0..3 {
            p.add(
                0,
                Material::Concrete,
                Shape::Wall {
                    start: Point::new(i as f64, 0.),
                    end: Point::new(i as f64 + 1., 0.),
                    thickness: 0.2,
                    height: 3.,
                },
            );
        }
        for (i, wall) in p.elements.iter_mut().enumerate() {
            let settings = wall.wall_settings.get_or_insert_with(Default::default);
            settings.geometry.bottom_offset = if i == 2 { 0.5 } else { 1. };
            settings.geometry.top_offset = 2.;
        }
        let walls: Vec<_> = p.elements.iter().collect();
        let groups = wall_level_groups(&p, &walls);
        assert_eq!(groups, vec![(1., 5., vec![0, 1]), (0.5, 5., vec![2])]);
        let mut vertices = vec![];
        let mut lines = vec![];
        append_joined_wall_prism(
            groups[0].0,
            groups[0].1,
            Material::Concrete,
            vec![
                Point::new(0., 0.),
                Point::new(2., 0.),
                Point::new(2., 0.2),
                Point::new(0., 0.2),
            ],
            &mut vertices,
            &mut lines,
        );
        assert!(
            vertices
                .iter()
                .chain(&lines)
                .all(|v| v.position[2] == 1. || v.position[2] == 5.)
        );
    }
}

#[cfg(test)]
mod inclined_column_render_tests {
    use super::*;
    #[test]
    fn inclined_column_emitter_uses_clipped_solid_not_vertical_proxy() {
        let mut editor = crate::editor::Editor::new(Project::default());
        editor
            .place_column_with_inclination(
                Point::default(),
                crate::column::ColumnSection::Rectangular {
                    width: 2.,
                    depth: 2_f64.sqrt(),
                },
                2.,
                Some(crate::column_inclination::ColumnInclination {
                    slant_degrees: 45.,
                    direction_degrees: 0.,
                }),
            )
            .unwrap();
        let solid = editor.column_geometry(1).unwrap().geometry;
        let area = MarqueeArea {
            vertices: vec![
                Point::new(0., -5.),
                Point::new(5., -5.),
                Point::new(5., 5.),
                Point::new(0., 5.),
            ],
            scope: crate::editor::MarqueeScope::AllStories,
            story: 0,
            z_range: Some((1., 2.)),
        };
        let (mut vertices, mut lines) = (Vec::new(), Vec::new());
        append_column_snapshot(&solid, Some(&area), [0.5; 3], &mut vertices, &mut lines);
        let volume: f64 = vertices
            .chunks_exact(3)
            .map(|t| {
                let a = glam::DVec3::from_array(t[0].position);
                let b = glam::DVec3::from_array(t[1].position);
                let c = glam::DVec3::from_array(t[2].position);
                a.dot(b.cross(c)) / 6.
            })
            .sum();
        assert!((volume - 2.).abs() < 0.0001);
        assert!(
            vertices
                .iter()
                .all(|v| v.position[0] >= -1e-6 && v.position[2] >= 1. - 1e-6)
        );
        assert!(vertices.iter().any(|v| v.position[1] > 2.9));
        assert!(!lines.is_empty());
        assert!(
            vertices
                .iter()
                .all(|v| v.color.iter().all(|c| c.is_finite()))
        );
    }
}

#[cfg(test)]
mod shared_slab_clip_tests {
    use super::*;
    #[test]
    fn rendered_slab_triangles_are_shared_pick_mesh_even_for_thin_reversed_z_slice() {
        use crate::{
            editor::{Editor, MarqueeScope},
            model::{Point, SlabContour},
        };
        let p = Point::new;
        let mut e = Editor::new(Project::default());
        e.place_slab_contour(SlabContour {
            points: vec![p(0., 0.), p(6., 0.), p(6., 6.), p(0., 6.)],
            holes: vec![SlabContour {
                points: vec![p(1., 1.), p(2., 1.), p(2., 2.), p(1., 2.)],
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
        let slab = &e.project.elements[0];
        let mut area = MarqueeArea::new(
            vec![
                p(0., 0.),
                p(6., 0.),
                p(6., 3.),
                p(3., 3.),
                p(3., 6.),
                p(0., 6.),
            ],
            MarqueeScope::CurrentStory,
            0,
        )
        .unwrap();
        for range in [
            None,
            Some((-0.100005, -0.1)),
            Some((-0.1, -0.100005)),
            Some((0., 0.)),
            Some((1., 2.)),
        ] {
            area.z_range = range;
            let mesh =
                crate::slab_geometry::try_clipped_mesh(slab, 0., &area.vertices, range).unwrap();
            let expected: Vec<_> = mesh.triangles().flatten().map(|p| p.to_array()).collect();
            let mut vertices = Vec::new();
            let mut lines = Vec::new();
            append_clipped_prism(&e.project, slab, &area, [0.5; 3], &mut vertices, &mut lines);
            assert_eq!(
                vertices.iter().map(|v| v.position).collect::<Vec<_>>(),
                expected
            );
            if range.is_none_or(|(a, b)| a < 0. && b < 0.) {
                assert!(!vertices.is_empty());
            } else {
                assert!(vertices.is_empty());
            }
        }
    }
}

#[cfg(test)]
mod checked_scene_tests {
    use super::*;
    use crate::editor::MarqueeScope;
    fn scene() -> Project {
        let mut p = Project::default();
        let q = Point::new;
        let wall = p.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: q(0., 0.),
                end: q(6., 0.),
                thickness: 0.3,
                height: 3.,
            },
        );
        for shape in [
            Shape::Slab {
                min: q(0., 0.),
                max: q(6., 4.),
                thickness: 0.3,
            },
            Shape::Beam {
                start: q(0., 0.),
                end: q(4., 0.),
                width: 0.3,
                height: 0.5,
                top_offset: 2.,
                slant_angle: 0.,
                arc: None,
            },
            Shape::Column {
                center: q(2., 2.),
                section: crate::column::ColumnSection::Circular { diameter: 0.4 },
                height: 3.,
                bottom_offset: 0.,
                axis_rotation: 0.,
                inclination: None,
                plan: Default::default(),
            },
            Shape::Roof {
                spec: crate::roof::RoofSpec {
                    contour: None,
                    pivot_start: q(0., 0.),
                    pivot_end: q(6., 0.),
                    rise_left: true,
                    min: q(0., 0.),
                    max: q(6., 4.),
                    pitch: 25.,
                    thickness: 0.2,
                    pivot_offset: 3.,
                },
            },
            Shape::Stair {
                spec: Default::default(),
            },
            Shape::Opening {
                host_wall: wall,
                kind: OpeningKind::Window,
                width: 1.,
                height: 1.,
                sill: 0.8,
                offset: 2.,
            },
            Shape::Polyline {
                points: vec![q(0., 0.), q(2., 2.)],
                arcs: vec![],
            },
        ] {
            p.add(0, Material::Concrete, shape);
        }
        p.validate().unwrap();
        p
    }
    #[test]
    fn checked_scene_consumes_all_families_and_rejects_invalid_context_before_any_emission() {
        let p = scene();
        let before = p.clone();
        let invalid = MarqueeArea {
            vertices: vec![
                Point::new(0., 0.),
                Point::new(4., 4.),
                Point::new(0., 3.),
                Point::new(4., 0.),
            ],
            scope: MarqueeScope::AllStories,
            story: 0,
            z_range: None,
        };
        let mut queries = crate::wall_solid::WallQuery::new(&p);
        for e in &p.elements {
            let mut vertices = vec![];
            let mut lines = vec![];
            let d = append_element_checked(
                &p,
                e,
                Some(&invalid),
                [0.5; 3],
                &mut vertices,
                &mut lines,
                &mut queries,
            )
            .unwrap_err();
            assert_eq!(d.element_id, e.id);
            assert_eq!(d.code, "INVALID_VIEW_CLIP");
            assert!(vertices.is_empty() && lines.is_empty());
            append_element_checked(
                &p,
                e,
                None,
                [0.5; 3],
                &mut vertices,
                &mut lines,
                &mut queries,
            )
            .unwrap();
            assert_eq!(
                vertices.is_empty(),
                matches!(e.shape, Shape::Polyline { .. }),
                "{}",
                e.kind()
            );
        }
        assert_eq!(p, before);
    }
    #[test]
    fn public_source_clips_reject_self_crossings_independently_of_the_renderer_gate() {
        let p = scene();
        let clip = vec![
            Point::new(0., 0.),
            Point::new(4., 4.),
            Point::new(0., 3.),
            Point::new(4., 0.),
        ];
        for e in &p.elements {
            let rejected = match &e.shape {
                Shape::ConceptMass { .. } => crate::concept_mass::derive(e, 0.)
                    .unwrap()
                    .clipped(&clip, None)
                    .is_err(),
                Shape::Zone { .. } => crate::zones::derive(e, 0.)
                    .unwrap()
                    .clipped(&clip, None)
                    .is_err(),
                Shape::Wall { .. } => crate::wall_solid::derive(&p, e)
                    .unwrap()
                    .geometry
                    .clipped(Some(&clip), None)
                    .is_err(),
                Shape::Opening { .. } => {
                    let g = crate::opening_geometry::derive(&p, e).unwrap();
                    assert!(
                        g.geometry
                            .pane
                            .as_ref()
                            .unwrap()
                            .clipped(Some(&clip), None)
                            .is_err()
                    );
                    g.geometry.clipped_proxy(Some(&clip), None).is_err()
                }
                Shape::Slab { .. } => crate::slab_geometry::derive(e, 0.)
                    .unwrap()
                    .clipped(&clip, None)
                    .is_err(),
                Shape::Roof { .. } => crate::roof_geometry::derive(e, 0.)
                    .unwrap()
                    .clipped(Some(&clip), None)
                    .is_err(),
                Shape::Beam { .. } => crate::beam_geometry::derive(&p, e)
                    .unwrap()
                    .clipped(Some(&clip), None)
                    .is_err(),
                Shape::Column { .. } => crate::column_geometry::derive(&p, e)
                    .unwrap()
                    .clipped(Some(&clip), None)
                    .is_err(),
                Shape::Stair { .. } => crate::stair_geometry::derive(&p, e)
                    .unwrap()
                    .clipped(Some(&clip), None)
                    .is_err(),
                Shape::Polyline { .. } => continue,
            };
            assert!(rejected, "{}", e.kind());
        }
        let area = MarqueeArea {
            vertices: clip,
            scope: MarqueeScope::AllStories,
            story: 0,
            z_range: None,
        };
        assert!(
            Camera::default()
                .pick_in_view(
                    &p,
                    [0., 0.],
                    1.,
                    crate::geometry::ModelViewFilter {
                        story: None,
                        selected: &[],
                        selection_only: false,
                        marquee: Some(&area),
                        outside_marquee: false
                    }
                )
                .is_none()
        );
    }
    #[test]
    fn failed_source_does_not_append_partial_vertices_or_replace_geometry_with_a_box() {
        let mut p = scene();
        let slab = p
            .elements
            .iter_mut()
            .find(|e| matches!(e.shape, Shape::Slab { .. }))
            .unwrap();
        if let Shape::Slab {
            ref mut thickness, ..
        } = slab.shape
        {
            *thickness = f64::NAN;
        }
        let slab = p
            .elements
            .iter()
            .find(|e| matches!(e.shape, Shape::Slab { .. }))
            .unwrap();
        let mut vertices = vec![Vertex {
            position: [7., 8., 9.],
            color: [0.5; 3],
        }];
        let mut lines = vec![];
        let mut queries = crate::wall_solid::WallQuery::new(&p);
        let d = append_element_checked(
            &p,
            slab,
            None,
            [0.5; 3],
            &mut vertices,
            &mut lines,
            &mut queries,
        )
        .unwrap_err();
        assert_eq!(d.element_id, slab.id);
        assert!(d.code.contains("SLAB"));
        assert_eq!(vertices.len(), 1);
        assert_eq!(vertices[0].position, [7., 8., 9.]);
        assert!(lines.is_empty());
    }
    #[test]
    fn legitimate_empty_clip_is_success_and_invalid_height_is_diagnosed_for_each_family() {
        let p = scene();
        let mut queries = crate::wall_solid::WallQuery::new(&p);
        let mut clip = MarqueeArea::new(
            vec![
                Point::new(-1., -1.),
                Point::new(10., -1.),
                Point::new(10., 10.),
                Point::new(-1., 10.),
            ],
            MarqueeScope::AllStories,
            0,
        )
        .unwrap();
        for e in &p.elements {
            let mut vertices = vec![];
            let mut lines = vec![];
            clip.z_range = Some((100., 101.));
            append_element_checked(
                &p,
                e,
                Some(&clip),
                [0.5; 3],
                &mut vertices,
                &mut lines,
                &mut queries,
            )
            .unwrap();
            assert!(vertices.is_empty() && lines.is_empty(), "{}", e.kind());
            clip.z_range = Some((f64::NAN, 101.));
            assert_eq!(
                append_element_checked(
                    &p,
                    e,
                    Some(&clip),
                    [0.5; 3],
                    &mut vertices,
                    &mut lines,
                    &mut queries
                )
                .unwrap_err()
                .code,
                "INVALID_VIEW_CLIP"
            );
        }
    }
}

#[cfg(test)]
mod candidate_scene_tests {
    use super::*;
    use crate::{concept_mass::ConceptMassSpec, derivation_cache::DerivationCache};
    fn scene() -> Project {
        let mut p = Project::default();
        let q = Point::new;
        let outline = vec![q(10., 20.), q(20., 20.), q(20., 28.), q(10., 28.)];
        let hole = vec![q(12., 22.), q(12., 24.), q(14., 24.), q(14., 22.)];
        p.add(
            0,
            Material::Concrete,
            Shape::ConceptMass {
                spec: ConceptMassSpec {
                    arcs: vec![],
                    hole_arcs: vec![],
                    outline: outline.clone(),
                    holes: vec![hole.clone()],
                    height: 6.,
                    bottom_offset: 2.,
                },
            },
        );
        p.add(
            0,
            Material::Timber,
            Shape::ConceptMass {
                spec: ConceptMassSpec {
                    arcs: vec![],
                    hole_arcs: vec![],
                    outline: outline.iter().map(|v| q(v.x + 30., v.y + 5.)).collect(),
                    holes: vec![],
                    height: 3.,
                    bottom_offset: -1.,
                },
            },
        );
        p.add(
            0,
            Material::Timber,
            Shape::Zone {
                spec: crate::zones::ZoneSpec {
                    surface: Default::default(),
                    purpose: "public comparison test".into(),
                    outline: vec![q(60., 20.), q(64., 20.), q(64., 23.), q(60., 23.)],
                    holes: vec![vec![q(61., 21.), q(62., 21.), q(62., 22.), q(61., 22.)]],
                    height: 3.,
                    bottom_offset: 1.,
                    label_position: None,
                },
            },
        );
        p
    }
    fn top_area(s: &CpuScene, z: f64) -> f64 {
        s.vertices
            .chunks_exact(3)
            .filter(|t| t.iter().all(|v| (v.position[2] - z).abs() < 1e-8))
            .map(|t| {
                let a = glam::DVec3::from_array(t[0].position);
                let b = glam::DVec3::from_array(t[1].position);
                let c = glam::DVec3::from_array(t[2].position);
                (b - a).cross(c - a).length() / 2.
            })
            .sum()
    }
    #[test]
    fn candidate_scene_exact_holed_prisms_materials_cache_and_fail_closed() {
        let mut p = scene();
        let mut cache = DerivationCache::default();
        let mut scenes = CpuSceneCache::default();
        let mut read = cache.read(&p, 0, 1).unwrap();
        let a = scenes.prepare_comparison(&p, &[1], &mut read).clone();
        let b = scenes.prepare_comparison(&p, &[2], &mut read).clone();
        assert!(a.diagnostics.is_empty() && b.diagnostics.is_empty());
        let zone = scenes.prepare_comparison(&p, &[3], &mut read).clone();
        assert!(zone.diagnostics.is_empty());
        assert!(zone.triangle_count() > 0);
        assert!(zone.line_count() > 0);
        assert!(
            (top_area(&zone, 4.) - 11.).abs() < 1e-8,
            "Zone courtyard remains open"
        );

        assert!(
            zone.lines
                .iter()
                .all(|v| v.position[2] >= 1. && v.position[2] <= 4.)
        );

        assert!(
            (top_area(&a, 8.) - 76.).abs() < 1e-8,
            "hole must remain open"
        );
        assert!((top_area(&b, 2.) - 80.).abs() < 1e-8);
        for (s, lo, hi, color) in [
            (
                &a,
                [10., 20., 2.],
                [20., 28., 8.],
                Material::Concrete.color(),
            ),
            (
                &b,
                [40., 25., -1.],
                [50., 33., 2.],
                Material::Timber.color(),
            ),
        ] {
            for axis in 0..3 {
                assert_eq!(
                    s.vertices
                        .iter()
                        .map(|v| v.position[axis])
                        .fold(f64::INFINITY, f64::min),
                    lo[axis]
                );
                assert_eq!(
                    s.vertices
                        .iter()
                        .map(|v| v.position[axis])
                        .fold(f64::NEG_INFINITY, f64::max),
                    hi[axis]
                );
            }
            assert!(s.vertices.iter().any(|v| v.color == color));
            assert!(s.line_count() > 0);
        }
        scenes.prepare_comparison(&p, &[2], &mut read);
        let builds = scenes.stats().builds;
        assert_eq!(&b, scenes.prepare_comparison(&p, &[2], &mut read));
        assert_eq!(scenes.stats().builds, builds);
        assert!(scenes.stats().hits > 0);
        for ids in [vec![], vec![2, 1], vec![1, 1], vec![999], vec![1; 33]] {
            let invalid = scenes.prepare_comparison(&p, &ids, &mut read);
            assert_eq!((invalid.triangle_count(), invalid.line_count()), (0, 0));
            assert!(!invalid.diagnostics.is_empty());
        }
        let other = p.clone();
        assert_eq!(
            scenes
                .prepare_comparison(&other, &[1], &mut read)
                .triangle_count(),
            0
        );
        assert_eq!(&a, scenes.prepare_comparison(&p, &[1], &mut read));
        drop(read);
        p.elements[0].material = Material::Timber;
        let mut read = cache.read(&p, 1, 1).unwrap();
        let recolored = scenes.prepare_comparison(&p, &[1], &mut read).clone();
        assert_ne!(recolored, a);
        assert_eq!(read.stats().source_derivations, 0);
        drop(read);
        let Shape::ConceptMass { spec } = &mut p.elements[0].shape else {
            unreachable!()
        };
        spec.height = 7.;
        let mut read = cache.read(&p, 2, 1).unwrap();
        let changed = scenes.prepare_comparison(&p, &[1], &mut read);
        assert!((top_area(changed, 9.) - 76.).abs() < 1e-8);
        assert_eq!(read.stats().source_derivations, 1);
        drop(read);
        let Shape::ConceptMass { spec } = &mut p.elements[0].shape else {
            unreachable!()
        };
        spec.height = f64::NAN;
        let mut read = cache.read(&p, 3, 1).unwrap();
        let broken = scenes.prepare_comparison(&p, &[1], &mut read);
        assert_eq!(broken.triangle_count(), 0);
        assert!(!broken.diagnostics.is_empty());
    }
}

/// Static reference buffers use their own stable local origin. Camera motion only updates 64 bytes.
struct CadGpuCache {
    key: Option<[u8; 32]>,
    lines: [Option<wgpu::Buffer>; 2],
    count: [u32; 2],
    origin: glam::DVec3,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
}
impl CadGpuCache {
    fn prepare(
        &mut self,
        state: &RenderState,
        p: &Project,
        camera: &Camera,
        size: [u32; 2],
        enabled: bool,
    ) {
        let sources: Vec<_> = p
            .drawings
            .cad_sources
            .iter()
            .map(|s| (s.id, &s.parsed.sha256, s.data.is_some()))
            .collect();
        let key = crate::derived_geometry::input_fingerprint(
            &(sources, &p.drawings.cad_underlays, enabled),
            0.,
        )
        .ok();
        if self.key != key || self.key.is_none() {
            let mut vertices: [Vec<Vertex>; 2] = [vec![], vec![]];
            if enabled {
                for u in p
                    .drawings
                    .cad_underlays
                    .iter()
                    .filter(|u| u.visible && u.show_3d)
                {
                    let Some(data) = p
                        .drawings
                        .cad_sources
                        .iter()
                        .find(|s| s.id == u.source)
                        .and_then(|s| s.data.as_ref())
                    else {
                        continue;
                    };
                    let grey = u.grey() as f32 / 255.;
                    for pair in u.model_segments(data) {
                        for point in pair {
                            vertices[usize::from(u.xray)].push(vertex(point, [grey; 3]));
                        }
                    }
                }
            }
            self.origin = vertices
                .iter()
                .flat_map(|v| v.iter())
                .next()
                .map_or(glam::DVec3::ZERO, |v| glam::DVec3::from_array(v.position));
            for group in 0..2 {
                self.count[group] = vertices[group].len() as u32;
                self.lines[group] = if vertices[group].is_empty() {
                    None
                } else {
                    Some(
                        state
                            .device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Cached CAD reference paths"),
                                contents: bytemuck::cast_slice(&gpu_vertices(
                                    &vertices[group],
                                    self.origin,
                                )),
                                usage: wgpu::BufferUsages::VERTEX,
                            }),
                    )
                };
            }
            self.key = key;
        }
        let mut camera = camera.clone();
        camera.target -= self.origin;
        state.queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::cast_slice(
                &camera
                    .matrix(size[0] as f64 / size[1] as f64)
                    .as_mat4()
                    .to_cols_array(),
            ),
        );
    }
}
