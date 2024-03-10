use wgpu::*;
use crate::engine::graphics::Renderer;
use super::model::{VertexPosNormTexCol, WgpuVertexDecl};

pub fn new(
    renderer: &Renderer,
    camera_bind_group: &BindGroupLayout,
    transform_bind_group: &BindGroupLayout)
    -> RenderPipeline
{

    let test_shader = renderer.device().create_shader_module(
        include_wgsl!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/shaders/test.wgsl")));

    renderer.device().create_render_pipeline(&wgpu::RenderPipelineDescriptor
    {
        label: Some("Test render pipeline"),
        layout: Some(&renderer.device().create_pipeline_layout(&wgpu::PipelineLayoutDescriptor
        {
            label: Some("Test render pipeline layout"),
            bind_group_layouts: &[camera_bind_group, transform_bind_group],
            push_constant_ranges: &[],
        })),
        vertex: VertexState
        {
            module: &test_shader,
            entry_point: "vs_main",
            buffers: &[VertexPosNormTexCol::layout()],
        },
        primitive: PrimitiveState
        {
            topology: PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: FrontFace::Ccw,
            cull_mode: Some(Face::Back),
            polygon_mode: PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: MultisampleState // todo: MSAA support in back buffer
        {
            count: renderer.current_sample_count(),
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        fragment: Some(FragmentState
        {
            module: &test_shader,
            entry_point: "fs_main",
            targets: &[Some(ColorTargetState
            {
                format: renderer.surface_config().format,
                blend: Some(BlendState::REPLACE),
                write_mask: ColorWrites::ALL,
            })],
        }),
        multiview: None,
    })
}