use crate::engine::graphics::assets::ShaderStage;
use crate::engine::graphics::{VertexPosNormTexCol, WgpuVertexDecl};
use metrohash::MetroHash64;
use std::hash::{Hash, Hasher};
use wgpu::{BindGroupLayout, BlendState, ColorTargetState, ColorWrites, Face, FragmentState, FrontFace, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPipelineDescriptor, ShaderModule, TextureFormat, VertexState};

#[derive(Hash)]
pub enum DebugMode
{
    Normal,
    Wireframe,
    WireframeNoCull,
}

#[derive(Hash)]
pub enum PipelineClass
{
    Static,
    Billboard,
    // Skinned,
    // Gui,
    // Fx,
    // etc
}

pub struct ShaderRef<'r>(pub &'r ShaderModule);
impl<'r> Hash for ShaderRef<'r>
{
    fn hash<H: Hasher>(&self, state: &mut H)
    {
        self.0.global_id().hash(state)
    }
}

#[derive(Hash)]
pub struct PipelineState<'p>
{
    pub mode: DebugMode,
    pub class: PipelineClass,
    pub blend_state: BlendState,
    pub texture_count: u32,
    pub vertex_shader: ShaderRef<'p>,
    pub pixel_shader: ShaderRef<'p>,
}

pub struct PipelineCache
{
    camera_layout: BindGroupLayout,
    transform_layout: BindGroupLayout,

    cached_surface_format: TextureFormat,
    // cached multisample state
    //

    // pipelines: HashMap<u64, RenderPipeline>

}
impl PipelineCache
{
    pub fn new() -> Self
    {
        todo!()
    }

    pub fn get_or_create_pipeline(&self, ps: PipelineState)
    {
        let ps_hash =
        {
            let mut hasher = MetroHash64::new();
            ps.hash(&mut hasher);
            hasher.finish()
        };

        // TODO: check cache

        let (layout, cull_mode) = match ps.class
        {
            PipelineClass::Static =>
            (
                VertexPosNormTexCol::layout(),
                Some(Face::Back),
            ),
            PipelineClass::Billboard =>
            (
                VertexPosNormTexCol::layout(),
                None,
            ),
        };

        let (cull_mode, polygon_mode) = match ps.mode
        {
            DebugMode::Normal => (cull_mode, PolygonMode::Fill),
            DebugMode::Wireframe => (cull_mode, PolygonMode::Line),
            DebugMode::WireframeNoCull => (None, PolygonMode::Line),
        };

        let desc = RenderPipelineDescriptor
        {
            label: None, // TODO
            layout: None,
            vertex: VertexState
            {
                module: ps.vertex_shader.0,
                entry_point: ShaderStage::Vertex.entry_point(),
                compilation_options: Default::default(),
                buffers: &[], // TODO
            },
            primitive: PrimitiveState
            {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Cw,
                cull_mode,
                unclipped_depth: false,
                polygon_mode,
                conservative: false,
            },
            depth_stencil: None, // todo
            multisample: Default::default(), // todo
            fragment: Some(FragmentState
            {
                module: ps.pixel_shader.0,
                entry_point: ShaderStage::Pixel.entry_point(),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState
                {
                    format: self.cached_surface_format,
                    blend: Some(ps.blend_state),
                    write_mask: ColorWrites::ALL, // COLOR for opaque? (blend_state.alpha = REPLACE)
                })],
            }),
            multiview: None,
            cache: None, // todo
        };
    }
}
