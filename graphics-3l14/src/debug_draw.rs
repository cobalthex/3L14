use std::sync::atomic::{AtomicBool, Ordering};
use egui::Ui;
use glam::{Mat4, Vec2, Vec3, Vec3Swizzles, Vec4};
use wgpu::{include_spirv, BlendState, Buffer, BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites, FragmentState, FrontFace, IndexFormat, MultisampleState, PolygonMode, PrimitiveState, PrimitiveTopology, Queue, RenderPass, RenderPipeline, RenderPipelineDescriptor, VertexState};
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

#[repr(packed)]
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
    ibuffer: Buffer,
}

pub struct DebugDraw
{
    pub is_enabled: AtomicBool,

    camera_transform: Mat4,
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
                module: &renderer.device().create_shader_module(include_spirv!("../../assets/shaders/DebugLines.vs.spv")),
                entry_point: ShaderStage::Vertex.entry_point().expect("Shader stage has no entry-point"),
                compilation_options: Default::default(),
                buffers: &[VertexLayout::DebugLines.into()],
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
                module: &renderer.device().create_shader_module(include_spirv!("../../assets/shaders/DebugLines.ps.spv")),
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

        let max_entries = 1024;
        // todo: use a pool
        let lines_vbuffer = renderer.device().create_buffer(&BufferDescriptor
        {
            label: debug_label!("Debug lines vertices"),
            size: size_of::<DebugLineVertex>() as u64 * max_entries,
            usage: BufferUsages::COPY_DST | BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        let lines_ibuffer = renderer.device().create_buffer(&BufferDescriptor
        {
            label: debug_label!("Debug lines indices"),
            size: size_of::<u32>() as u64 * max_entries,
            usage: BufferUsages::COPY_DST | BufferUsages::INDEX,
            mapped_at_creation: false,
        });

        Self
        {
            is_enabled: AtomicBool::new(true),
            camera_transform: Mat4::IDENTITY,
            lines_pipeline,
            lines: DrawLines
            {
                vertices: Vec::with_capacity(max_entries as usize),
                indices: Vec::with_capacity(max_entries as usize),
                vbuffer: lines_vbuffer,
                ibuffer: lines_ibuffer,
            },
        }
    }

    pub fn begin(&mut self, camera: &Camera)
    {
        self.camera_transform = camera.clip_mtx();
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

    pub fn draw_clipspace_line(&mut self, a: Vec2, b: Vec2, color: Rgba)
    {
        let start = self.lines.vertices.len() as u32;

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

            render_pass.set_vertex_buffer(0, self.lines.vbuffer.slice(0..(vb_slice.len() as u64)));
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