use crate::debug_label;
use crate::engine::graphics::assets::{GeometryMesh, Material, Model, Shader, ShaderStage};
use crate::engine::graphics::Renderer;
use metrohash::MetroHash64;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use egui::Ui;
use wgpu::{AddressMode, BindGroup, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferBindingType, ColorTargetState, ColorWrites, CompareFunction, DepthBiasState, DepthStencilState, Face, FilterMode, FragmentState, FrontFace, MultisampleState, PipelineCompilationOptions, PipelineLayout, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPass, RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, StencilState, TextureFormat, TextureSampleType, TextureViewDimension, VertexState};
use crate::engine::graphics::debug_gui::DebugGui;
use crate::engine::graphics::uniforms_pool::UniformsPool;

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

    // Todo: better threading solve? (Reference tracking makes this a pain)

    pipeline_layouts: Mutex<HashMap<u64, PipelineLayout>>,
    pipelines: RwLock<HashMap<PipelineHash, RenderPipeline>>,

    default_sampler: Sampler,
}
impl PipelineCache
{
    pub fn default_sampler(&self) -> &Sampler { &self.default_sampler }

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
        geometry: &GeometryMesh,
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
            let pipeline = self.create_pipeline(geometry, material, vertex_shader, pixel_shader, mode);
            p.insert(pipeline_hash, pipeline);
        });

        pipeline_hash
    }

    fn create_pipeline(
        &self,
        geometry: &GeometryMesh,
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
                    &material.bind_layout,
                ],
                push_constant_ranges: &[],
            })
        });

        // todo: if these update, this will invalidate the pipeline
        let renderer_surface_config = self.renderer.surface_format();
        let renderer_msaa_count = self.renderer.msaa_max_sample_count();

        let desc = RenderPipelineDescriptor
        {
            label: Some("TODO RenderPipeline Name"), // TODO
            layout: Some(pipeline_layout),
            vertex: VertexState
            {
                module: &vertex_shader.module,
                entry_point: ShaderStage::Vertex.entry_point().expect("Shader stage has no entry-point"),
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[geometry.vertex_layout.into()],
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
                entry_point: ShaderStage::Pixel.entry_point().expect("Shader stage has no entry-point"),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(ColorTargetState
                {
                    format: TextureFormat::Bgra8UnormSrgb, // TODO: based on material, maybe render pass (must match pass), or get from renderer surface format
                    // format: TextureFormat::Rgba8UnormSrgb, // TODO: based on material, maybe render pass (must match pass), or get from renderer surface format
                    blend: None, // todo: material settings
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None, // todo
        };

        let pipeline = self.renderer.device().create_render_pipeline(&desc);
        pipeline
    }

    // todo
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
    fn name(&self) -> &str { "Render pipelines" }

    fn debug_gui(&self, ui: &mut Ui)
    {
        ui.label(format!("Layouts: {}", self.pipeline_layouts.lock().len()));
        ui.label(format!("Pipelines: {}", self.pipelines.read().len()));

        ui.collapsing(self.uniforms.name(), |cui| self.uniforms.debug_gui(cui));
    }
}