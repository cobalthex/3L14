use arc_swap::ArcSwapOption;
use wgpu::*;
use crate::engine::assets::AssetHandle;
use crate::engine::graphics::assets::shader::Shader;
use crate::engine::graphics::Renderer;
use super::model::{VertexPosNormTexCol, WgpuVertexDecl};

pub fn new(
    renderer: &Renderer,
    camera_bind_group: &BindGroupLayout,
    transform_bind_group: &BindGroupLayout,
    tex_bind_group: &BindGroupLayout) -> RenderPipeline
{
    let test_shader = renderer.device().create_shader_module(
        include_wgsl!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/src/shaders/test.wgsl")));

    TestPipeline::create_pipeline(renderer, &test_shader, &test_shader, camera_bind_group, transform_bind_group, tex_bind_group)
}

pub struct TestPipeline
{
    vertex_shader: AssetHandle<Shader>,
    pixel_shader: AssetHandle<Shader>,
    render_pipeline: ArcSwapOption<RenderPipeline>,
}
impl TestPipeline
{
    pub fn new() -> Self { todo!() }

    pub fn try_get(&self) -> Option<&RenderPipeline>
    {
        match self.render_pipeline.load().as_ref()
        {
            // Some(rp) => Some(rp.as_ref()),
            _ => todo!(),
            None =>
            {
                if !self.vertex_shader.is_loaded_recursive() ||
                    !self.pixel_shader.is_loaded_recursive()
                {
                    return None;
                }

                todo!()
            }
        }
    }

    fn create_pipeline(
        renderer: &Renderer,
        vertex_shader: &ShaderModule,
        pixel_shader: &ShaderModule,
        camera_bind_group: &BindGroupLayout,
        transform_bind_group: &BindGroupLayout,
        tex_bind_group: &BindGroupLayout)
        -> RenderPipeline
    {
        renderer.device().create_render_pipeline(&wgpu::RenderPipelineDescriptor
        {
            label: Some("Test render pipeline"),
            layout: Some(&renderer.device().create_pipeline_layout(&wgpu::PipelineLayoutDescriptor
            {
                label: Some("Test render pipeline layout"),
                bind_group_layouts: &[camera_bind_group, transform_bind_group, &tex_bind_group],
                push_constant_ranges: &[],
            })),
            vertex: VertexState
            {
                module: vertex_shader,
                entry_point: "vs_main",
                buffers: &[VertexPosNormTexCol::layout()],
                compilation_options: Default::default(),
            },
            primitive: PrimitiveState
            {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Cw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState
            {
                format: TextureFormat::Depth32Float, // depth + stencil?
                depth_write_enabled: true,
                depth_compare: CompareFunction::Less,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState // todo: MSAA support in back buffer
            {
                count: renderer.msaa_max_sample_count(), // TODO .current_sample_count(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(FragmentState
            {
                module: pixel_shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState
                {
                    format: renderer.surface_format(),
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None, // TODO
        })
    }
}
