use std::io::Write;
use glam::Mat4;
use wgpu::{include_spirv, BlendState, Buffer, BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites, FragmentState, FrontFace, MultisampleState, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPass, RenderPipeline, RenderPipelineDescriptor, TextureFormat, VertexState};
use wgpu::util::BufferInitDescriptor;
use crate::debug_label;
use crate::engine::graphics::{Renderer, Rgba};
use crate::engine::graphics::assets::ShaderStage;
use crate::engine::world::{Camera, Frustum};

const CUBOID_INDICES: [u32; 24] =
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
];

struct DrawLines
{
    vertices: Vec<u8>,
    indices: Vec<u32>,
    vbuffer: Buffer,
    ibuffer: Buffer,
}

pub struct DebugDraw
{
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

        let max_entries = 1024;
        // todo: use a pool
        let lines_vbuffer = renderer.device().create_buffer(&BufferDescriptor
        {
            label: debug_label!("Debug lines vertices"),
            size: 16 * max_entries,
            usage: BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lines_ibuffer = renderer.device().create_buffer(&BufferDescriptor
        {
            label: debug_label!("Debug lines vertices"),
            size: 4 * max_entries,
            usage: BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self
        {
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

    pub fn draw_frustum(&mut self, transform: Mat4, camera: &Camera, color: Rgba)
    {
        let start = self.lines.vertices.len() as u32;
        let corners = Frustum::get_corners(camera.projection().0 * camera.view().0 * transform);
        for corner in corners
        {
            self.lines.vertices.extend_from_slice(&corner.x.to_le_bytes());
            self.lines.vertices.extend_from_slice(&corner.y.to_le_bytes());
            self.lines.vertices.extend_from_slice(&corner.z.to_le_bytes());
            self.lines.vertices.extend_from_slice(&<[u8; 4]>::from(color));
        }
        for i in CUBOID_INDICES
        {
            self.lines.indices.push(i + start);
        }
    }

    pub fn submit(&mut self, render_pass: &mut RenderPass)
    {
        puffin::profile_scope!("DebugDraw submission");
        //
        // render_pass.set_vertex_buffer(0, self.lines.slice(0..));
        // render_pass.set_index_buffer(mesh.indices.slice(0..), mesh.index_format);
        // render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }
}