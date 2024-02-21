use wgpu::*;
use crate::engine::middlewares::graphics::model::{WgpuVertex, VertexPosNormTexCol};

pub fn new(device: &mut Device, camera_bind_group: &BindGroupLayout) -> RenderPipeline
{
    let test_shader = device.create_shader_module(
        include_wgsl!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/shaders/test.wgsl")));

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor
    {
        label: Some("Test render pipeline"),
        layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor
        {
            label: Some("Test render pipeline layout"),
            bind_group_layouts: &[camera_bind_group],
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
        multisample: MultisampleState
        {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        fragment: Some(FragmentState
        {
            module: &test_shader,
            entry_point: "fs_main",
            targets: &[Some(ColorTargetState
            {
                format: TextureFormat::Bgra8UnormSrgb, // todo
                blend: Some(BlendState::REPLACE),
                write_mask: ColorWrites::ALL,
            })],
        }),
        multiview: None,
    })
}