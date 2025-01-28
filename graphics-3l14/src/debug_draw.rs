use glam::{Mat4, Vec2, Vec3, Vec3Swizzles};
use wgpu::{include_spirv, BlendState, Buffer, BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites, FragmentState, FrontFace, IndexFormat, MultisampleState, PolygonMode, PrimitiveState, PrimitiveTopology, Queue, RenderPass, RenderPipeline, RenderPipelineDescriptor, VertexState};
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

struct DebugLineVertex
{
    position: Vec2,
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
            size: 16 * max_entries,
            usage: BufferUsages::COPY_DST | BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        let lines_ibuffer = renderer.device().create_buffer(&BufferDescriptor
        {
            label: debug_label!("Debug lines indices"),
            size: 4 * max_entries,
            usage: BufferUsages::COPY_DST | BufferUsages::INDEX,
            mapped_at_creation: false,
        });

        Self
        {
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
        let start = self.lines.vertices.len() as u32;
        let mut transform = camera.clip_mtx();
        let corners = Frustum::get_corners(transform.inverse());
        transform = (transform * world_transform).inverse();
        for corner in corners
        {
            self.lines.vertices.push(DebugLineVertex
            {
                position: transform.transform_vector3(corner).xy(),
                color: color.into(),
            });
        }
        for i in CUBOID_INDICES
        {
            self.lines.indices.push(start + i);
        }

        // TODO: use draw wire box
    }

    pub fn draw_wire_box(&mut self, mut transform: Mat4, color: Rgba)
    {
        let start = self.lines.vertices.len() as u32;

        transform = self.camera_transform * transform;

        const CUBOID_CORNERS: [Vec3; 8] =
        [
            Vec3::new(-1.0, -1.0, -1.0), // near bottom left
            Vec3::new( 1.0, -1.0, -1.0), // near bottom right
            Vec3::new(-1.0,  1.0, -1.0), // near top left
            Vec3::new( 1.0,  1.0, -1.0), // near top right
            Vec3::new(-1.0, -1.0,  1.0), // far bottom left
            Vec3::new( 1.0, -1.0,  1.0), // far bottom right
            Vec3::new(-1.0,  1.0,  1.0), // far top left
            Vec3::new( 1.0,  1.0,  1.0), // far top right
        ];

        for corner in CUBOID_CORNERS
        {
            self.lines.vertices.push(DebugLineVertex
            {
                position: transform.project_point3(corner).xy(),
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
            position: a,
            color: color.into(),
        });
        self.lines.vertices.push(DebugLineVertex
        {
            position: b,
            color: color.into(),
        });

        self.lines.indices.push(start);
        self.lines.indices.push(start + 1);
    }

    pub fn submit(&mut self, queue: &Queue, render_pass: &mut RenderPass)
    {
        puffin::profile_scope!("DebugDraw submission");

        let num_indices = self.lines.indices.len() as u32;
        if num_indices > 0
        {
            // TODO: write verts/indices directly to buffer
            queue.write_buffer(&self.lines.vbuffer, 0, unsafe { self.lines.vertices.as_u8_slice() });
            queue.write_buffer(&self.lines.ibuffer, 0, unsafe { self.lines.indices.as_u8_slice() });

            render_pass.set_vertex_buffer(0, self.lines.vbuffer.slice(..));
            render_pass.set_index_buffer(self.lines.ibuffer.slice(..), IndexFormat::Uint32);
            render_pass.set_pipeline(&self.lines_pipeline);
            render_pass.draw_indexed(0..num_indices, 0, 0..1);
        }
    }
}