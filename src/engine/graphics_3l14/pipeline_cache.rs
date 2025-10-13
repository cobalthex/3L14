use crate::assets::{Geometry, Material, Shader, ShaderStage, VertexLayout};
use crate::uniforms_pool::UniformsPool;
use crate::{debug_label, Renderer};
use debug_3l14::debug_gui::DebugGui;
use egui::Ui;
use metrohash::MetroHash64;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use triomphe::Arc;
use arrayvec::ArrayVec;
use wgpu::{AddressMode, BindGroupLayout, BindGroupLayoutEntry, ColorTargetState, ColorWrites, CompareFunction, DepthBiasState, DepthStencilState, Face, FilterMode, FragmentState, FrontFace, MultisampleState, PipelineCompilationOptions, PipelineLayout, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPass, RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerDescriptor, StencilState, TextureFormat, VertexBufferLayout, VertexState};
use crate::vertex_layouts::{SkinnedVertex, StaticVertex, VertexDecl, VertexLayoutBuilder};

#[derive(Debug, Clone, Copy, Hash)]
pub enum DebugMode // debug only?
{
    None,
    Wireframe,
}

#[derive(Hash, PartialEq, Eq, Copy, Clone)]
pub struct PipelineHash(u64);

pub struct PipelineCache
{
    renderer: Arc<Renderer>,
    pub uniforms: UniformsPool,

    // TODO: callback from renderer when one of the global settings changes
    // invalidate all pipelines

    pipeline_layouts: Mutex<HashMap<u64, PipelineLayout>>,
    pipelines: RwLock<HashMap<PipelineHash, RenderPipeline>>,

    default_sampler: Sampler,
}
impl PipelineCache
{
    pub fn default_sampler(&self) -> &Sampler { &self.default_sampler }

    #[must_use]
    pub fn new(renderer: Arc<Renderer>) -> Self
    {
        let default_sampler = Self::create_sampler(&renderer);
        
        Self
        {
            renderer: renderer.clone(),
            uniforms: UniformsPool::new(renderer),
            pipeline_layouts: Mutex::default(),
            pipelines: RwLock::default(),
            default_sampler,
        }
    }

    pub fn try_apply(&self, render_pass: &mut RenderPass, pipeline_hash: PipelineHash) -> bool
    {
        let pipelines = self.pipelines.read(); // recursive?
        let Some(pipeline) = pipelines.get(&pipeline_hash) else { return false; };
        render_pass.set_pipeline(pipeline);
        true
    }

    pub fn get_or_create(
        &self,
        geometry: &Geometry,
        material: &Material,
        vertex_shader: &Shader,
        pixel_shader: &Shader,
        mode: DebugMode) -> PipelineHash
    {
        // if shaders change their hashes and in turn asset keys should change
        let pipeline_hash =
        {
            let mut hasher = MetroHash64::default();
            // TODO: there may be some material properties that can affect this
            material.class.hash(&mut hasher);
            vertex_shader.module_hash.hash(&mut hasher); // will have a unique vertex layout
            pixel_shader.module_hash.hash(&mut hasher);
            mode.hash(&mut hasher); // TODO
            PipelineHash(hasher.finish())
        };

        let mut pipelines = self.pipelines.upgradable_read();
        if let Some(pipeline) = pipelines.get(&pipeline_hash)
        {
            return pipeline_hash;
        }

        pipelines.with_upgraded(|p|
        {
            let pipeline = self.create_pipeline(geometry.vertex_layout, material, vertex_shader, pixel_shader, mode);
            p.insert(pipeline_hash, pipeline);
        });

        pipeline_hash
    }

    #[must_use]
    fn create_pipeline(
        &self,
        vertex_layout: VertexLayout,
        material: &Material,
        vertex_shader: &Shader,
        pixel_shader: &Shader,
        mode: DebugMode) -> RenderPipeline
    {
        // move up?
        puffin::profile_scope!("create render pipeline");

        assert_eq!(vertex_shader.stage, ShaderStage::Vertex);
        assert_eq!(pixel_shader.stage, ShaderStage::Pixel);

        let layout_hash =
        {
            let mut hasher = MetroHash64::default();
            material.class.hash(&mut hasher);
            material.textures.len().hash(&mut hasher); // TODO: this needs to use the actual texture formats
            hasher.finish()
        };

        let mut pipeline_layouts = self.pipeline_layouts.lock();
        let pipeline_layout = pipeline_layouts.entry(layout_hash).or_insert_with(||
        {
            self.renderer.device().create_pipeline_layout(&PipelineLayoutDescriptor
            {
                label: debug_label!(&format!("{:?}+{} tex pipeline layout", material.class, material.textures.len())),
                bind_group_layouts: &[
                    &self.uniforms.camera_bind_layout,
                    &self.uniforms.transform_bind_layout,
                    &self.uniforms.poses_bind_layout,
                    &material.bind_layout,
                ],
                push_constant_ranges: &[],
            })
        });

        // todo: if these update, this will invalidate the pipeline
        let renderer_surface_format = self.renderer.surface_format();
        let renderer_msaa_count = self.renderer.msaa_max_sample_count();

        let vbuffers = VertexLayoutBuilder::from(vertex_layout);

        self.renderer.device().create_render_pipeline(&RenderPipelineDescriptor
        {
            label: debug_label!("TODO RenderPipeline Name"), // TODO
            layout: Some(pipeline_layout),
            vertex: VertexState
            {
                module: &vertex_shader.module,
                entry_point: Some(ShaderStage::Vertex.entry_point()),
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[vbuffers.as_vertex_buffer_layout()],
            },
            primitive: PrimitiveState
            {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Cw,
                cull_mode: Some(Face::Back),
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            // TODO: fetch from renderer surface config + material params
            depth_stencil: Some(DepthStencilState
            {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: CompareFunction::Less,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            // TODO
            multisample: MultisampleState
            {
                count: renderer_msaa_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(FragmentState
            {
                module: &pixel_shader.module,
                entry_point: Some(ShaderStage::Pixel.entry_point()),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(ColorTargetState
                {
                    format: renderer_surface_format,
                    blend: None, // todo: material settings
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None, // todo
        })
    }

    // todo
    #[must_use]
    fn create_sampler(renderer: &Renderer) -> Sampler
    {
        renderer.device().create_sampler(&SamplerDescriptor
        {
            label: Some("Default sampler"),
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        })
    }
}
impl DebugGui for PipelineCache
{
    fn display_name(&self) -> &str { "Render pipelines" }

    fn debug_gui(&self, ui: &mut Ui)
    {
        ui.label(format!("Layouts: {}", self.pipeline_layouts.lock().len()));
        ui.label(format!("Pipelines: {}", self.pipelines.read().len()));

        ui.collapsing(self.uniforms.display_name(), |cui| self.uniforms.debug_gui(cui));
    }
}