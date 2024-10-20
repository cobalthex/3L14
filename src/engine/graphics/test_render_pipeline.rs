use wgpu::*;
use crate::engine::graphics::assets::ShaderStage;
use crate::engine::graphics::Renderer;

pub fn new(
    renderer: &Renderer,
    vertex_shader: &ShaderModule,
    pixel_shader: &ShaderModule,
    camera_bind_group: &BindGroupLayout,
    transform_bind_group: &BindGroupLayout,
    tex_bind_group: &BindGroupLayout)
    -> RenderPipeline
{
    todo!()
    // renderer.device().create_render_pipeline(&wgpu::RenderPipelineDescriptor
    // {
    //     label: Some("Test render pipeline"),
    //     layout: Some(&renderer.device().create_pipeline_layout(&wgpu::PipelineLayoutDescriptor
    //     {
    //         label: Some("Test render pipeline layout"),
    //         bind_group_layouts: &[camera_bind_group, transform_bind_group, tex_bind_group],
    //         push_constant_ranges: &[],
    //     })),
    //     vertex: VertexState
    //     {
    //         module: vertex_shader,
    //         entry_point: ShaderStage::Vertex.entry_point(),
    //         buffers: &[VertexPosNormTexCol::layout()],
    //         compilation_options: PipelineCompilationOptions::default(), // vertex pulling?
    //     },
    //     primitive: PrimitiveState
    //     {
    //         topology: PrimitiveTopology::TriangleList,
    //         strip_index_format: None,
    //         front_face: FrontFace::Cw,
    //         cull_mode: Some(Face::Back),
    //         polygon_mode: PolygonMode::Fill,
    //         unclipped_depth: false,
    //         conservative: false,
    //     },
    //     depth_stencil: Some(wgpu::DepthStencilState
    //     {
    //         format: TextureFormat::Depth32Float, // depth + stencil?
    //         depth_write_enabled: true,
    //         depth_compare: CompareFunction::Less,
    //         stencil: StencilState::default(),
    //         bias: DepthBiasState::default(),
    //     }),
    //     multisample: MultisampleState // todo: MSAA support in back buffer
    //     {
    //         count: renderer.msaa_max_sample_count(), // TODO .current_sample_count(),
    //         mask: !0,
    //         alpha_to_coverage_enabled: false,
    //     },
    //     fragment: Some(FragmentState
    //     {
    //         module: pixel_shader,
    //         entry_point: ShaderStage::Pixel.entry_point(),
    //         targets: &[Some(ColorTargetState
    //         {
    //             format: renderer.surface_format(),
    //             blend: Some(BlendState::REPLACE),
    //             write_mask: ColorWrites::ALL,
    //         })],
    //         compilation_options: PipelineCompilationOptions::default(),
    //     }),
    //     multiview: None,
    //     cache: None, // TODO
    // })
}
