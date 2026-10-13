//! Native wgpu offscreen scene. The depth-tested texture is composited by egui.
use crate::{
    editor::MarqueeArea,
    geometry::{Camera, FACES, corners},
    model::*,
};
use eframe::{
    egui,
    egui_wgpu::{RenderState, wgpu},
};
use wgpu::util::DeviceExt;
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

fn vertex(p: glam::Vec3, color: [f32; 3]) -> Vertex {
    Vertex { position: p.to_array(), color }
}

fn push_triangle(out: &mut Vec<Vertex>, a: glam::Vec3, b: glam::Vec3, c: glam::Vec3, color: [f32; 3]) {
    out.extend([vertex(a,color), vertex(b,color), vertex(c,color)]);
}

/// Draws connected, coplanar walls as one extruded perimeter. This removes
/// overlapping internal faces at clean orthogonal wall junctions.
fn append_joined_wall_prism(
    project: &Project,
    story: u32,
    height: f32,
    material: Material,
    mut outline: Vec<crate::model::Point>,
    vertices: &mut Vec<Vertex>,
    lines: &mut Vec<Vertex>,
) {
    use crate::geometry::triangulate_polygon;
    if outline.len() > 2 && outline[0].distance(*outline.last().unwrap()) <= 1e-3 {
        outline.pop();
    }
    if outline.len() < 3 { return; }
    let bottom = project.elevation(story);
    let top = bottom + height;
    let at = |point: crate::model::Point, z: f32| glam::Vec3::new(point.x, point.y, z);
    let base = material.color();
    for [a, b, c] in triangulate_polygon(&outline) {
        push_triangle(vertices, at(a, bottom), at(c, bottom), at(b, bottom), base.map(|x| x * 0.48));
        push_triangle(vertices, at(a, top), at(b, top), at(c, top), base);
    }
    let side = base.map(|x| x * 0.73);
    let edge = base.map(|x| x * 0.42);
    for index in 0..outline.len() {
        let a = outline[index];
        let b = outline[(index + 1) % outline.len()];
        let (a0, b0, a1, b1) = (at(a, bottom), at(b, bottom), at(a, top), at(b, top));
        push_triangle(vertices, a0, b0, b1, side);
        push_triangle(vertices, a0, b1, a1, side);
        lines.extend([vertex(a0, edge), vertex(b0, edge), vertex(b1, edge), vertex(a1, edge), vertex(a0, edge), vertex(a1, edge)]);
    }
}

fn edge_on_polygon(a: crate::model::Point, b: crate::model::Point, polygon: &[crate::model::Point]) -> bool {
    use crate::geometry::point_on_segment;
    (0..polygon.len()).any(|i| {
        let x = polygon[i]; let y = polygon[(i+1)%polygon.len()];
        point_on_segment(a,x,y,2e-4) && point_on_segment(b,x,y,2e-4)
    })
}

/// Emits the solid remaining after intersecting an element's vertical prism
/// with a Marquee prism. Ear-clipped fragments partition concave areas; only
/// actual source or Marquee boundaries receive vertical faces, so triangulation
/// seams never appear as false cut edges.
fn append_clipped_prism(
    project: &Project,
    element: &Element,
    marquee: &MarqueeArea,
    base: [f32;3],
    vertices: &mut Vec<Vertex>,
    lines: &mut Vec<Vertex>,
) {
    use crate::geometry::{footprint, intersect_footprint_with_polygon};
    let original = footprint(element);
    let fragments = intersect_footprint_with_polygon(&original, &marquee.vertices);
    if fragments.is_empty() { return; }
    let c = corners(project, element);
    let low = c.iter().map(|p| p.z).fold(f32::INFINITY, f32::min);
    let high = c.iter().map(|p| p.z).fold(f32::NEG_INFINITY, f32::max);
    let (z0,z1) = marquee.z_range.map_or((low,high), |(a,b)| (low.max(a.min(b)),high.min(a.max(b))));
    if z1 - z0 < 1e-5 { return; }
    let cut = base.map(|x| (x * 0.58 + 0.12).min(1.));
    for poly in fragments {
        if poly.len() < 3 { continue; }
        let at = |p: crate::model::Point, z: f32| glam::Vec3::new(p.x,p.y,z);
        let bottom = base.map(|x| x * 0.48);
        let top = base;
        let p0 = at(poly[0],z0);
        for i in 1..poly.len()-1 {
            push_triangle(vertices,p0,at(poly[i+1],z0),at(poly[i],z0),bottom);
            push_triangle(vertices,at(poly[0],z1),at(poly[i],z1),at(poly[i+1],z1),top);
        }
        for i in 0..poly.len() {
            let a=poly[i]; let b=poly[(i+1)%poly.len()];
            let source = edge_on_polygon(a,b,&original);
            let is_cut = edge_on_polygon(a,b,&marquee.vertices);
            if !source && !is_cut { continue; }
            let col = if is_cut && !source { cut } else { base.map(|x| x * 0.72) };
            let a0=at(a,z0); let b0=at(b,z0); let a1=at(a,z1); let b1=at(b,z1);
            push_triangle(vertices,a0,b0,b1,col);
            push_triangle(vertices,a0,b1,a1,col);
            let line_color = if is_cut { [0.08,0.20,0.24] } else { base.map(|x|x*0.42) };
            lines.extend([vertex(a0,line_color),vertex(b0,line_color),vertex(b1,line_color),vertex(a1,line_color),vertex(a0,line_color),vertex(a1,line_color)]);
        }
        // Top and bottom outline follows the true clipped perimeter only.
        for i in 0..poly.len() {
            let a=poly[i]; let b=poly[(i+1)%poly.len()];
            if !edge_on_polygon(a,b,&original) && !edge_on_polygon(a,b,&marquee.vertices) { continue; }
            let col=[0.12,0.23,0.26];
            lines.extend([vertex(at(a,z0),col),vertex(at(b,z0),col),vertex(at(a,z1),col),vertex(at(b,z1),col)]);
        }
    }
}
pub struct SceneRenderer {
    state: RenderState,
    pipeline: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
    multisample: wgpu::TextureView,
    pub texture: egui::TextureId,
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let make = |topology, write, bias| {
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
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
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
        let pipeline = make(wgpu::PrimitiveTopology::TriangleList, true, 1);
        let lines = make(wgpu::PrimitiveTopology::LineList, false, 0);
        let (color, depth, multisample) = Self::targets(device, [2, 2]);
        let texture = state.renderer.write().register_native_texture(
            device,
            &color,
            wgpu::FilterMode::Linear,
        );
        Self {
            state,
            pipeline,
            lines,
            uniform,
            bind,
            color,
            depth,
            multisample,
            texture,
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
    pub fn render(
        &mut self,
        p: &Project,
        selected: &[u64],
        camera: &Camera,
        size: [u32; 2],
        story: Option<u32>,
        marquee: Option<&MarqueeArea>,
        outside_marquee: bool,
        selection_only: bool,
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
        self.state.queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::cast_slice(
                &camera
                    .matrix(size[0] as f32 / size[1] as f32)
                    .to_cols_array(),
            ),
        );
        let mut vertices = vec![];
        let mut lines = vec![];
        // A restrained construction grid, generated in world space.
        for i in -10..=25 {
            let k = i as f32;
            let c = if i % 5 == 0 {
                [0.74, 0.77, 0.77]
            } else {
                [0.82, 0.84, 0.83]
            };
            for pos in [
                [k, -10., -0.32],
                [k, 25., -0.32],
                [-10., k, -0.32],
                [25., k, -0.32],
            ] {
                lines.push(Vertex {
                    position: pos,
                    color: c,
                });
            }
        }
        let mut wall_display_footprints = Vec::new();
        let mut attached_wall_ends = Vec::new();
        let mut merged_wall_ids = Vec::new();
        let join_3d_display = marquee.is_none() && !outside_marquee && !selection_only;
        let stories: Vec<u32> = story.map_or_else(|| p.stories.iter().map(|item| item.id).collect(), |id| vec![id]);
        for story_id in stories {
            for material in Material::ALL {
                let walls: Vec<_> = p.elements.iter().filter(|element| {
                    element.story == story_id
                        && element.material == material
                        && matches!(element.shape, Shape::Wall { .. })
                }).collect();
                let footprints = crate::geometry::plan_wall_footprints(&walls);
                attached_wall_ends.extend(crate::geometry::plan_wall_attached_ends(&walls, &footprints));
                wall_display_footprints.extend(walls.iter().zip(footprints.iter().copied()).map(|(wall, footprint)| (wall.id, footprint)));
                if join_3d_display && !walls.iter().any(|wall| selected.contains(&wall.id)) {
                    let mut height_groups: Vec<(f32, Vec<usize>)> = Vec::new();
                    for (index, wall) in walls.iter().enumerate() {
                        let Shape::Wall { height, .. } = wall.shape else { continue };
                        if let Some((_, indices)) = height_groups.iter_mut().find(|(candidate, _)| (height - *candidate).abs() <= 1e-4) {
                            indices.push(index);
                        } else {
                            height_groups.push((height, vec![index]));
                        }
                    }
                    for (height, indices) in height_groups {
                        if indices.len() < 2 { continue; }
                        let group_footprints: Vec<_> = indices.iter().map(|index| footprints[*index]).collect();
                        let outlines = crate::geometry::wall_outline_paths(&group_footprints);
                        // A single closed loop is the common connected L/T
                        // junction case. Groups with separate loops or holes
                        // retain per-element geometry.
                        if outlines.len() == 1 && outlines[0].len() > 3
                            && outlines[0][0].distance(*outlines[0].last().unwrap()) <= 1e-3
                        {
                            append_joined_wall_prism(p, story_id, height, material, outlines.into_iter().next().unwrap(), &mut vertices, &mut lines);
                            merged_wall_ids.extend(indices.iter().map(|index| walls[*index].id));
                        }
                    }
                }
            }
        }
        for e in p
            .elements
            .iter()
            .filter(|e| story.is_none_or(|s| e.story == s))
            .filter(|e| !selection_only || selected.contains(&e.id))
            .filter(|e| {
                let Some(marquee) = marquee else { return true };
                if !marquee.includes_story(e.story) { return false; }
                let intersects = crate::geometry::element_intersects_polygon(e, &marquee.vertices)
                    && marquee.z_range.is_none_or(|range| crate::geometry::element_overlaps_z_range(p, e, range));
                if outside_marquee {
                    !crate::geometry::element_fully_inside_volume(p, e, &marquee.vertices, marquee.z_range)
                } else {
                    intersects
                }
            })
        {
            if join_3d_display && merged_wall_ids.contains(&e.id) { continue; }
            let mut element_corners = corners(p, e);
            let base = if selected.contains(&e.id) {
                [0.31, 0.70, 0.70]
            } else {
                e.material.color()
            };
            if !outside_marquee && !selection_only {
                if let Some(area) = marquee {
                    append_clipped_prism(p,e,area,base,&mut vertices,&mut lines);
                    continue;
                }
            }
            if join_3d_display && matches!(e.shape, Shape::Wall { .. }) {
                if let Some((_, footprint)) = wall_display_footprints.iter().find(|(id, _)| *id == e.id) {
                    for i in 0..4 {
                        element_corners[i].x = footprint[i].x;
                        element_corners[i].y = footprint[i].y;
                        element_corners[i + 4].x = footprint[i].x;
                        element_corners[i + 4].y = footprint[i].y;
                    }
                }
            }
            let start_joined = join_3d_display && attached_wall_ends.contains(&(e.id, true));
            let end_joined = join_3d_display && attached_wall_ends.contains(&(e.id, false));
            for (fi, face) in FACES.iter().enumerate() {
                // The extended rectangular wall bodies overlap at a join, so
                // their internal end faces must not be drawn.
                if (fi == 3 && end_joined) || (fi == 5 && start_joined) { continue; }
                let shade = [0.55, 1., 0.73, 0.87, 0.81, 0.66][fi];
                let c = base.map(|x| x * shade);
                for i in [0, 1, 2, 0, 2, 3] {
                    vertices.push(Vertex {
                        position: element_corners[face[i]].to_array(),
                        color: c,
                    });
                }
            }
            let color = if selected.contains(&e.id) {
                [0.07, 0.40, 0.42]
            } else {
                base.map(|x| x * 0.48)
            };
            for [a, b] in [
                [0, 1],
                [1, 2],
                [2, 3],
                [3, 0],
                [4, 5],
                [5, 6],
                [6, 7],
                [7, 4],
                [0, 4],
                [1, 5],
                [2, 6],
                [3, 7],
            ] {
                let is_start_cap_edge = [[3, 0], [7, 4], [3, 7], [0, 4]].contains(&[a, b])
                    || [[3, 0], [7, 4], [3, 7], [0, 4]].contains(&[b, a]);
                let is_end_cap_edge = [[1, 2], [5, 6], [1, 5], [2, 6]].contains(&[a, b])
                    || [[1, 2], [5, 6], [1, 5], [2, 6]].contains(&[b, a]);
                if (start_joined && is_start_cap_edge) || (end_joined && is_end_cap_edge) { continue; }
                lines.push(Vertex {
                    position: element_corners[a].to_array(),
                    color,
                });
                lines.push(Vertex {
                    position: element_corners[b].to_array(),
                    color,
                });
            }
        }
        let vb = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Scene triangles"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let lb = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Scene edges"),
            contents: bytemuck::cast_slice(&lines),
            usage: wgpu::BufferUsages::VERTEX,
        });
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
            pass.set_pipeline(&self.lines);
            pass.set_vertex_buffer(0, lb.slice(..));
            pass.draw(0..lines.len() as u32, 0..1);
        }
        self.state.queue.submit([encoder.finish()]);
    }
}

#[cfg(test)]
mod clipping_tests {
    use super::*;
    use crate::{editor::MarqueeScope, model::{Material, Point, Shape}};

    #[test]
    fn joined_wall_prism_emits_only_the_exposed_l_outline() {
        let mut project = Project::default();
        project.add(0, Material::Concrete, Shape::Wall {
            start: Point::new(0., 0.), end: Point::new(3., 0.), thickness: 0.2, height: 3.,
        });
        project.add(0, Material::Concrete, Shape::Wall {
            start: Point::new(3., 0.), end: Point::new(3., 2.), thickness: 0.2, height: 3.,
        });
        let walls: Vec<_> = project.elements.iter().collect();
        let footprints = crate::geometry::plan_wall_footprints(&walls);
        let outlines = crate::geometry::wall_outline_paths(&footprints);
        assert_eq!(outlines.len(), 1, "an L junction has one exposed perimeter");
        let outline = &outlines[0];
        let unique_count = outline.len() - 1;
        assert!(outline[0].distance(*outline.last().unwrap()) <= 1e-3);
        for index in 0..unique_count {
            let a = outline[index];
            let b = outline[index + 1];
            assert!((a.x - b.x).abs() <= 1e-4 || (a.y - b.y).abs() <= 1e-4,
                "L junction outline has no diagonal bevel edge: {a:?} -> {b:?}");
        }

        let mut triangles = Vec::new();
        let mut edges = Vec::new();
        append_joined_wall_prism(
            &project, 0, 3., Material::Concrete, outline.clone(), &mut triangles, &mut edges,
        );
        assert_eq!(edges.len(), unique_count * 6, "only exposed perimeter edges are emitted");
        assert_eq!(triangles.len() % 3, 0);
        assert!(!triangles.is_empty());
    }

    #[test]
    fn clipped_prism_is_cut_in_xy_and_z() {
        let mut project=Project::default();
        project.add(0,Material::Concrete,Shape::Wall{
            start:Point::new(-1.,0.),end:Point::new(3.,0.),thickness:1.,height:3.,
        });
        let element=&project.elements[0];
        let marquee=MarqueeArea{
            vertices:vec![Point::new(0.,-1.),Point::new(2.,-1.),Point::new(2.,1.),Point::new(0.,1.)],
            scope:MarqueeScope::CurrentStory,story:0,z_range:Some((1.,2.)),
        };
        let mut triangles=Vec::new(); let mut edges=Vec::new();
        append_clipped_prism(&project,element,&marquee,[0.7,0.7,0.7],&mut triangles,&mut edges);
        assert!(!triangles.is_empty());
        for v in triangles.iter().chain(edges.iter()) {
            let [x,y,z]=v.position;
            assert!((-1e-4..=2.0001).contains(&x),"x={x}");
            assert!((-1.0001..=1.0001).contains(&y),"y={y}");
            assert!((0.9999..=2.0001).contains(&z),"z={z}");
        }
        assert!(triangles.iter().any(|v| v.color != [0.7,0.7,0.7]),"cut caps should be shaded");
    }
}
