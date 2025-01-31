use std::sync::atomic::{AtomicBool, Ordering};
use egui::Ui;
use glam::{FloatExt, Mat4, Vec2, Vec3, Vec3Swizzles, Vec4};
use wgpu::{include_spirv, BindGroup, BindGroupDescriptor, BindGroupEntry, BlendState, Buffer, BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites, FragmentState, FrontFace, IndexFormat, MultisampleState, PolygonMode, PrimitiveState, PrimitiveTopology, Queue, RenderPass, RenderPipeline, RenderPipelineDescriptor, VertexState};
use debug_3l14::debug_gui::DebugGui;
use nab_3l14::math::Frustum;
use nab_3l14::utils::AsU8Slice;
use crate::{debug_label, Renderer, Rgba};
use crate::assets::{ShaderStage, VertexLayout};
use crate::camera::Camera;

// Indices to draw lines
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

// #[repr(packed)]
#[repr(align(16))]
struct DebugLineVertex
{
    // TODO: vec4 nicer here, this type should probably be a pow2 size
    position: [f32; 4],
    color: u32,
}

struct DrawLines
{
    vertices: Vec<DebugLineVertex>,
    indices: Vec<u32>,
    vbuffer: Buffer,
    vbuffer_binding: BindGroup,
    ibuffer: Buffer,
}

pub struct DebugDraw
{
    pub is_enabled: AtomicBool,

    camera_transform: Mat4,
    cam_aspect_ratio: f32,
    lines_pipeline: RenderPipeline,
    lines: DrawLines,
}
impl DebugDraw
{
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
                entry_point: ShaderStage::Vertex.entry_point().expect("Shader stage has no entry-point"),
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
                entry_point: ShaderStage::Pixel.entry_point().expect("Shader stage has no entry-point"),
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

        // TODO: switch to storage (structured) buffers, use SV_VertexID to index

        let max_entries = 1024;
        // todo: use a pool
        let lines_vbuffer = renderer.device().create_buffer(&BufferDescriptor
        {
            label: debug_label!("Debug lines vertices"),
            size: size_of::<DebugLineVertex>() as u64 * max_entries,
            usage: BufferUsages::COPY_DST | BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let lines_ibuffer = renderer.device().create_buffer(&BufferDescriptor
        {
            label: debug_label!("Debug lines indices"),
            size: size_of::<u32>() as u64 * max_entries,
            usage: BufferUsages::COPY_DST | BufferUsages::INDEX,
            mapped_at_creation: false,
        });

        let vbuffer_binding = renderer.device().create_bind_group(&BindGroupDescriptor
        {
            label: debug_label!("Debug line vertices binding"),
            layout: &lines_pipeline.get_bind_group_layout(0),
            entries: &[BindGroupEntry
            {
                binding: 0,
                resource: lines_vbuffer.as_entire_binding(),
            }],
        });

        Self
        {
            is_enabled: AtomicBool::new(true),
            camera_transform: Mat4::IDENTITY,
            cam_aspect_ratio: 1.0,
            lines_pipeline,
            lines: DrawLines
            {
                vertices: Vec::with_capacity(max_entries as usize),
                indices: Vec::with_capacity(max_entries as usize),
                vbuffer: lines_vbuffer,
                vbuffer_binding,
                ibuffer: lines_ibuffer,
            },
        }
    }

    pub fn begin(&mut self, camera: &Camera)
    {
        self.camera_transform = camera.clip_mtx();
        self.cam_aspect_ratio = camera.projection().aspect_ratio();
        self.lines.vertices.clear();
        self.lines.indices.clear();
    }

    pub fn draw_frustum(&mut self, camera: &Camera, world_transform: Mat4, color: Rgba)
    {
        let mut transform = camera.clip_mtx().inverse();
        self.draw_wire_box(transform, color);
    }

    pub fn draw_wire_box(&mut self, mut transform: Mat4, color: Rgba)
    {
        let start = self.lines.vertices.len() as u32;

        transform = self.camera_transform * transform;

        const CUBOID_CORNERS: [Vec4; 8] =
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

        self.lines.vertices.reserve(CUBOID_CORNERS.len() as usize);
        self.lines.indices.reserve(CUBOID_INDICES.len() as usize);

        for corner in CUBOID_CORNERS
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

    pub fn draw_wire_sphere(&mut self, mut transform: Mat4, mut color: Rgba)
    {
        let start = self.lines.vertices.len() as u32;

        let num_lats = 8;
        let num_longs = 16;

        let num_verts = (num_longs * (num_lats - 2) + 2) as usize;
        self.lines.vertices.reserve(num_verts);
        self.lines.indices.reserve(num_verts * 4 + num_longs as usize);

        transform = self.camera_transform * transform;

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

                let mut b: u32 = start + (lat - 1) * num_longs + 1;
                let z = (b + long).saturating_sub(num_longs);
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
                    f32::sin(i as f32 * inc) * radius * self.cam_aspect_ratio + center.y,
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

        let num_indices = self.lines.indices.len() as u32;
        if num_indices > 0
        {
            let vb_slice = unsafe { self.lines.vertices.as_u8_slice() };
            let ib_slice = unsafe { self.lines.indices.as_u8_slice() };

            // TODO: write verts/indices directly to buffer
            queue.write_buffer(&self.lines.vbuffer, 0, vb_slice);
            queue.write_buffer(&self.lines.ibuffer, 0, ib_slice);
            //
            // render_pass.set_vertex_buffer(0, self.lines.vbuffer.slice(0..(vb_slice.len() as u64)));
            render_pass.set_bind_group(0, &self.lines.vbuffer_binding, &[]);
            render_pass.set_index_buffer(self.lines.ibuffer.slice(0..(ib_slice.len() as u64)), IndexFormat::Uint32);
            render_pass.set_pipeline(&self.lines_pipeline);
            render_pass.draw_indexed(0..num_indices, 0, 0..1);
        }
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
    }
}