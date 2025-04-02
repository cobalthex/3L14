use crate::assets::ShaderStage;
use crate::camera::Camera;
use crate::{colors, debug_label, Renderer, Rgba};
use debug_3l14::debug_gui::DebugGui;
use egui::{Align2, Color32, FontId, Painter, Pos2, Ui};
use glam::{FloatExt, Mat4, Quat, Vec2, Vec3, Vec4};
use math_3l14::{Degrees, Plane, Radians, WORLD_FORWARD, WORLD_RIGHT, WORLD_UP};
use std::sync::atomic::{AtomicBool, Ordering};
use wgpu::{include_spirv, BlendState, BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites, Face, FragmentState, FrontFace, IndexFormat, MultisampleState, PolygonMode, PrimitiveState, PrimitiveTopology, Queue, RenderPass, RenderPipeline, RenderPipelineDescriptor, VertexState};
use crate::dynamic_geo::DynamicGeo;

const CUBOID_VERTICES: [Vec4; 8] =
[
    Vec4::new(-1.0, -1.0, -1.0, 1.0), // near bottom left
    Vec4::new( 1.0, -1.0, -1.0, 1.0), // near bottom right
    Vec4::new(-1.0,  1.0, -1.0, 1.0), // near top left
    Vec4::new( 1.0,  1.0, -1.0, 1.0), // near top right
    Vec4::new(-1.0, -1.0,  1.0, 1.0), // far bottom left
    Vec4::new( 1.0, -1.0,  1.0, 1.0), // far bottom right
    Vec4::new(-1.0,  1.0,  1.0, 1.0), // far top left
    Vec4::new( 1.0,  1.0,  1.0, 1.0), // far top right
];

// #[repr(packed)]
#[repr(align(16))]
struct DebugLineVertex
{
    // TODO: vec4 nicer here, this type should probably be a pow2 size
    position: [f32; 4],
    color: u32,
}

pub struct DebugDraw
{
    pub is_enabled: AtomicBool,

    gui_painter: Option<Painter>,

    camera_forward: Vec3,
    camera_clip_mtx: Mat4,
    camera_aspect_ratio: f32,
    lines_pipeline: RenderPipeline,
    lines: DynamicGeo<DebugLineVertex>,
    solids_pipeline: RenderPipeline,
    solids: DynamicGeo<DebugLineVertex>,
}
impl DebugDraw
{
    #[must_use]
    pub fn new(renderer: &Renderer) -> Self
    {
        let renderer_surface_format = renderer.surface_format();
        let renderer_msaa_count = renderer.msaa_max_sample_count();

        // TODO: load shaders better
        // TODO: buffer pool (and use 16 bit vertices)

        let lines_pipeline = renderer.device().create_render_pipeline(&RenderPipelineDescriptor
        {
            label: debug_label!("Debug lines"),
            layout: None,
            vertex: VertexState
            {
                module: &renderer.device().create_shader_module(include_spirv!("../../../assets/shaders/DebugLines.vs.spv")),
                entry_point: ShaderStage::Vertex.entry_point(),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: PrimitiveState
            {
                topology: PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: FrontFace::Cw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            // TODO
            multisample: MultisampleState
            {
                count: renderer_msaa_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(FragmentState
            {
                module: &renderer.device().create_shader_module(include_spirv!("../../../assets/shaders/DebugLines.ps.spv")),
                entry_point: ShaderStage::Pixel.entry_point(),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState
                {
                    format: renderer_surface_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });
        let lines_geo = DynamicGeo::new(renderer, &lines_pipeline.get_bind_group_layout(0));

        let solids_pipeline = renderer.device().create_render_pipeline(&RenderPipelineDescriptor
        {
            label: debug_label!("Debug solids"),
            layout: None,
            vertex: VertexState
            {
                module: &renderer.device().create_shader_module(include_spirv!("../../../assets/shaders/DebugLines.vs.spv")),
                entry_point: ShaderStage::Vertex.entry_point(),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: PrimitiveState
            {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Cw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            // TODO
            multisample: MultisampleState
            {
                count: renderer_msaa_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(FragmentState
            {
                module: &renderer.device().create_shader_module(include_spirv!("../../../assets/shaders/DebugLines.ps.spv")),
                entry_point: ShaderStage::Pixel.entry_point(),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState
                {
                    format: renderer_surface_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });
        let solids_geo = DynamicGeo::new(renderer, &solids_pipeline.get_bind_group_layout(0));

        Self
        {
            is_enabled: AtomicBool::new(true),
            gui_painter: None,
            camera_clip_mtx: Mat4::IDENTITY,
            camera_forward: Vec3::Z,
            camera_aspect_ratio: 1.0,
            lines_pipeline,
            lines: lines_geo,
            solids_pipeline,
            solids: solids_geo,
        }
    }

    pub fn begin(&mut self, camera: &Camera, egui_ctx: &egui::Context)
    {
        self.gui_painter = Some(egui_ctx.layer_painter(egui::LayerId::background()));
        self.camera_clip_mtx = camera.matrix();
        self.camera_forward = camera.transform().forward();
        self.camera_aspect_ratio = camera.projection().aspect_ratio();
        self.lines.begin();
        self.solids.begin();
    }

    pub fn draw_text(&mut self, text: &str, center: Vec3, color: Rgba)
    {
        let Some(painter) = self.gui_painter.as_mut() else { return };

        let mut position = self.camera_clip_mtx.mul_vec4(center.extend(1.0));
        if position.w < 0.0 { return } // < near clip?

        let clip = painter.clip_rect().size();
        position.x = ((position.x / position.w + 1.0) / 2.0) * clip.x;
        position.y = ((-position.y / position.w + 1.0) / 2.0) * clip.y;
        painter.text(
            Pos2::new(position.x, position.y),
            Align2::CENTER_CENTER,
            text,
            FontId::proportional(20.0), // todo
            Color32::from_rgba_unmultiplied(color.red, color.green, color.blue, color.alpha));
    }

    pub fn draw_frustum(&mut self, camera: &Camera, color: Rgba)
    {
        // let clip_mtx = camera.clip_mtx().inverse();
        let clip_mtx = camera.matrix().inverse();
        self.draw_wire_cube(clip_mtx, color);
    }

    pub fn draw_wire_cube(&mut self, mut transform: Mat4, color: Rgba)
    {
        let start = self.lines.vertices.len() as u32;

        transform = self.camera_clip_mtx * transform;

        const CUBOID_INDICES: [u32; 26] =
        [
            0, 4,
            0, 2,
            2, 6,
            2, 3,
            3, 7,
            3, 1,
            1, 3,
            1, 0,
            4, 6,
            6, 7,
            7, 5,
            5, 4,
            1, 5,
        ];

        self.lines.vertices.reserve(CUBOID_VERTICES.len());
        self.lines.indices.reserve(CUBOID_INDICES.len());

        for corner in CUBOID_VERTICES
        {
            self.lines.vertices.push(DebugLineVertex
            {
                position: transform.mul_vec4(corner).into(),
                color: color.into(),
            });
        }
        for i in CUBOID_INDICES
        {
            self.lines.indices.push(start + i);
        }
    }

    // draw a 3D cross (-x -> x),(-y -> y),(-z -> z)
    pub fn draw_cross3(&mut self, mut transform: Mat4, color: Rgba)
    {
        transform = self.camera_clip_mtx * transform;

        const SCALE: f32 = 0.5;
        let points =
        [
             transform.mul_vec4(Vec4::new(-SCALE,  0.0,  0.0, 1.0)),
             transform.mul_vec4(Vec4::new( SCALE,  0.0,  0.0, 1.0)),
             transform.mul_vec4(Vec4::new( 0.0, -SCALE,  0.0, 1.0)),
             transform.mul_vec4(Vec4::new( 0.0,  SCALE,  0.0, 1.0)),
             transform.mul_vec4(Vec4::new( 0.0,  0.0, -SCALE, 1.0)),
             transform.mul_vec4(Vec4::new( 0.0,  0.0,  SCALE, 1.0)),
        ];

        let start = self.lines.vertices.len() as u32;
        self.lines.vertices.extend(points.map(|p| DebugLineVertex
        {
            position: p.into(),
            color: color.into(),
        }));
        self.lines.indices.reserve(2 * points.len());
        for i in 0..points.len() as u32
        {
            self.lines.indices.push(start + i * 2);
            self.lines.indices.push(start + i * 2 + 1);
        }
    }

    // TODO: capsulify?
    // draw a wireframe 'UV' (quad faced) sphere
    pub fn draw_wire_sphere(&mut self, mut transform: Mat4, color: Rgba)
    {
        let start = self.lines.vertices.len() as u32;

        // calc based on size?
        let num_lats = 6;
        let num_longs = 12;

        let num_verts = (num_longs * (num_lats - 2) + 2) as usize;
        self.lines.vertices.reserve(num_verts);
        self.lines.indices.reserve(num_verts * 4 + num_longs as usize);

        transform = self.camera_clip_mtx * transform;

        self.lines.vertices.push(DebugLineVertex
        {
            position: transform.mul_vec4(Vec4::new(0.0, -1.0, 0.0, 1.0)).into(),
            color: color.into(),
        });

        let lat_step = 2.0 / (num_lats - 1) as f32;
        let long_step = (2.0 * std::f32::consts::PI) / num_longs as f32;
        for lat in 1..(num_lats - 1)
        {
            let y = (-1.0 + lat_step * lat as f32).min(1.0);
            let width = (1.0 - y * y).sqrt();

            for long in 0..num_longs
            {
                let phi = long as f32 * long_step;

                let pos = Vec4::new(f32::cos(phi) * width, y, f32::sin(phi) * width, 1.0);
                self.lines.vertices.push(DebugLineVertex
                {
                    position: transform.mul_vec4(pos).into(),
                    color: color.into(),
                });

                let mut b: u32 = (lat - 1) * num_longs + 1;
                let z = start + (b + long).saturating_sub(num_longs);
                b += start;

                self.lines.indices.push(b + long);
                self.lines.indices.push(b + (long + 1) % num_longs);

                self.lines.indices.push(b + long);
                self.lines.indices.push(z);
            }
        }

        self.lines.vertices.push(DebugLineVertex
        {
            position: transform.mul_vec4(Vec4::new(0.0, 1.0, 0.0, 1.0)).into(),
            color: color.into(),
        });

        let end = start + (num_verts as u32 - 1);
        for i in 0..num_longs
        {
            self.lines.indices.push(end - i - 1);
            self.lines.indices.push(end);
        }
    }

    // draw a wire cone. untransformed, tip points to +Z, base to -Z
    pub fn draw_wire_cone(&mut self, mut transform: Mat4, color: Rgba)
    {
        let start = self.lines.vertices.len() as u32;

        // calc based on size
        let num_points = 8;

        transform = self.camera_clip_mtx * transform;

        self.lines.vertices.reserve(num_points as usize + 1);
        self.lines.indices.reserve(num_points as usize * 2);

        self.lines.vertices.push(DebugLineVertex
        {
            position: transform.mul_vec4(Vec4::new(0.0, 0.0, 1.0, 1.0)).into(),
            color: color.into(),
        });

        let step = std::f32::consts::TAU / num_points as f32;
        for i in 0..num_points
        {
            let theta = step * i as f32;
            self.lines.vertices.push(DebugLineVertex
            {
                position: transform.mul_vec4(Vec4::new(f32::cos(theta), f32::sin(theta), -1.0, 1.0)).into(),
                color: color.into(),
            });

            let z = i + 1;
            self.lines.indices.push(start);
            self.lines.indices.push(start + z);

            self.lines.indices.push(start + z);
            self.lines.indices.push(start + z % num_points + 1);
        }

        // fill in the base?
    }

    // TODO: draw wire/solid cylinder (combine w/ cone?)

    pub fn draw_solid_cube(&mut self, mut transform: Mat4, color: Rgba)
    {
        let start = self.solids.vertices.len() as u32;

        transform = self.camera_clip_mtx * transform;

        self.solids.vertices.reserve(CUBOID_VERTICES.len());
        self.solids.indices.reserve(36);

        for corner in CUBOID_VERTICES
        {
            self.solids.vertices.push(DebugLineVertex
            {
                position: transform.mul_vec4(corner).into(),
                color: color.into(),
            });
        }

        const INDICES: [u32; 6 * 6] =
        [
            0, 2, 1, 2, 3, 1, // front
            0, 4, 6, 6, 2, 0, // left
            4, 5, 6, 5, 7, 6, // back
            5, 1, 7, 1, 3, 7, // right
            2, 6, 7, 7, 3, 2, // top
            0, 1, 5, 0, 5, 4, // bottom
        ];

        for i in INDICES
        {
            self.solids.indices.push(i + start);
        }
    }

    // draw a solid 'UV' (quad faced) sphere
    pub fn draw_solid_sphere(&mut self, mut transform: Mat4, color: Rgba)
    {
        let start = self.solids.vertices.len() as u32;

        // calc based on size?
        let num_lats = 6; // rows
        let num_longs = 12; // columns

        let num_verts = (num_longs * (num_lats - 2) + 2) as usize;
        self.solids.vertices.reserve(num_verts);
        self.solids.indices.reserve(num_verts * 6); // todo: probably wrong

        transform = self.camera_clip_mtx * transform;

        self.solids.vertices.push(DebugLineVertex
        {
            position: transform.mul_vec4(Vec4::new(0.0, -1.0, 0.0, 1.0)).into(),
            color: color.into(),
        });

        // bottom cap
        for i in 0..num_longs
        {
            self.solids.indices.push(start);
            self.solids.indices.push(start + 1 + i);
            self.solids.indices.push(start + 1 + (i + 1) % num_longs);
        }

        let lat_step = 2.0 / (num_lats - 1) as f32;
        let long_step = (2.0 * std::f32::consts::PI) / num_longs as f32;
        for lat in 1..(num_lats - 1)
        {
            let y = (-1.0 + lat_step * lat as f32).min(1.0);
            let width = (1.0 - y * y).sqrt();

            for long in 0..num_longs
            {
                let phi = long as f32 * long_step;

                let pos = Vec4::new(f32::cos(phi) * width, y, f32::sin(phi) * width, 1.0);
                self.solids.vertices.push(DebugLineVertex
                {
                    position: transform.mul_vec4(pos).into(),
                    color: color.into(),
                });

                let bl = start + 1 + long + (lat - 1) * num_longs;
                let br = start + 1 + (long + 1) % num_longs + (lat - 1) * num_longs;
                let tl = bl + num_longs;
                let tr = br + num_longs;

                self.solids.indices.push(bl);
                self.solids.indices.push(tl);
                self.solids.indices.push(tr);

                self.solids.indices.push(bl);
                self.solids.indices.push(tr);
                self.solids.indices.push(br);
            }
        }

        self.solids.vertices.push(DebugLineVertex
        {
            position: transform.mul_vec4(Vec4::new(0.0, 1.0, 0.0, 1.0)).into(),
            color: color.into(),
        });

        // top cap
        let end = start + (num_verts as u32 - 1);
        for i in 0..num_longs
        {
            self.solids.indices.push(end);
            self.solids.indices.push(end - 1 - i);
            self.solids.indices.push(end - 1 - (i + 1) % num_longs);
        }
    }

    // draw a solid cone. untransformed, tip points to +Z, base to -Z
    pub fn draw_solid_cone(&mut self, mut transform: Mat4, color: Rgba)
    {
        let start = self.solids.vertices.len() as u32;

        // calc based on size
        let num_points = 8;

        transform = self.camera_clip_mtx * transform;

        self.solids.vertices.reserve(num_points as usize + 1);
        self.solids.indices.reserve(num_points as usize * 3);

        self.solids.vertices.push(DebugLineVertex
        {
            position: transform.mul_vec4(Vec4::new(0.0, 0.0, 1.0, 1.0)).into(),
            color: color.into(),
        });

        let step = std::f32::consts::TAU / num_points as f32;
        for i in 0..num_points
        {
            let theta = step * i as f32;
            self.solids.vertices.push(DebugLineVertex
            {
                position: transform.mul_vec4(Vec4::new(f32::cos(theta), f32::sin(theta), -1.0, 1.0)).into(),
                color: color.into(),
            });

            let z = i + 1;
            self.solids.indices.push(start);
            self.solids.indices.push(start + z);
            self.solids.indices.push(start + z % num_points + 1);
        }
    }

    pub fn draw_arrow(&mut self, tail: Vec3, nose: Vec3, wing_normal: Vec3, color: Rgba)
    {
        const WING_ANGLE: Radians = Degrees(30.0).to_radians();

        let wing_length = (tail - nose).length();
        let wing_tangent = (tail - nose) / 3.0;

        // todo: can prob just use (a+b)/2 to make diagonals
        let left = nose + Quat::from_axis_angle(wing_normal, -WING_ANGLE.0) * wing_tangent;
        let right = nose + Quat::from_axis_angle(wing_normal, WING_ANGLE.0) * wing_tangent;
        self.draw_polyline(&[tail, nose, left, nose, right], false, color);
    }

    // take points by value w/ template size?
    pub fn draw_polyline(&mut self, points: &[Vec3], connect_ends: bool, color: Rgba)
    {
        if points.len() <= 2 { return; } // not bother?

        self.lines.vertices.reserve(points.len());
        self.lines.indices.reserve(2 * points.len());

        let start = self.lines.vertices.len() as u32;
        for point in points
        {
            self.lines.vertices.push(DebugLineVertex
            {
                position: self.camera_clip_mtx.mul_vec4(point.extend(1.0)).into(),
                color: color.into(),
            });
        }
        for i in 1..points.len() as u32
        {
            self.lines.indices.push(start + i - 1);
            self.lines.indices.push(start + i);
        }
        if connect_ends
        {
            self.lines.indices.push(start + points.len() as u32 - 1);
            self.lines.indices.push(start);
        }
    }

    pub fn draw_clipspace_line(&mut self, a: Vec2, b: Vec2, color: Rgba)
    {
        let start = self.lines.vertices.len() as u32;
        self.lines.vertices.reserve(2);
        self.lines.indices.reserve(2);

        self.lines.vertices.push(DebugLineVertex
        {
            position: [a.x, a.y, 0.0, 1.0],
            color: color.into(),
        });
        self.lines.vertices.push(DebugLineVertex
        {
            position: [b.x, b.y, 0.0, 1.0],
            color: color.into(),
        });

        self.lines.indices.push(start);
        self.lines.indices.push(start + 1);
    }

    pub fn draw_clipspace_circle(&mut self, center: Vec2, radius: f32, color: Rgba)
    {
        let start = self.lines.vertices.len() as u32;

        let points = f32::lerp(8.0, 32.0, radius) as u32;

        self.lines.vertices.reserve(points as usize);
        self.lines.indices.reserve(points as usize * 2);

        let inc = (2.0 * std::f32::consts::PI) / points as f32;
        for i in 0..points
        {
            self.lines.vertices.push(DebugLineVertex
            {
                position:
                [
                    f32::cos(i as f32 * inc) * radius + center.x,
                    f32::sin(i as f32 * inc) * radius * self.camera_aspect_ratio + center.y,
                    0.0,
                    1.0
                ],
                color: color.into(),
            });
            self.lines.indices.push(start + i);
            self.lines.indices.push(start + (i + 1) % points);
        }
    }

    pub fn submit(&mut self, queue: &Queue, render_pass: &mut RenderPass)
    {
        if !self.is_enabled.load(Ordering::Relaxed) { return; }

        puffin::profile_scope!("DebugDraw submission");

        render_pass.set_pipeline(&self.solids_pipeline);
        self.solids.submit(queue, render_pass);
        render_pass.set_pipeline(&self.lines_pipeline);
        self.lines.submit(queue, render_pass);
    }
}
impl DebugGui for DebugDraw
{
    fn name(&self) -> &str { "Debug drawing" }

    fn debug_gui(&self, ui: &mut Ui)
    {
        let mut is_enabled = self.is_enabled.load(Ordering::Relaxed);
        if ui.checkbox(&mut is_enabled, "Enabled").changed()
        {
            self.is_enabled.store(is_enabled, Ordering::Relaxed);
        }

        ui.label(format!("Line vertex count: {}", self.lines.vertices.len()));
        ui.label(format!("Solids vertex count: {}", self.solids.vertices.len()));
    }
}