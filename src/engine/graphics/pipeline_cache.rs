use crate::debug_label;
use crate::engine::asset::AssetPayload;
use crate::engine::graphics::assets::{Geometry, GeometryMesh, Material, MaterialClass, Shader, ShaderStage};
use crate::engine::graphics::{Model, Renderer};
use metrohash::MetroHash64;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use wgpu::{AddressMode, BindGroup, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferBindingType, BufferSize, ColorTargetState, ColorWrites, CompareFunction, DepthBiasState, DepthStencilState, Face, FilterMode, FragmentState, FrontFace, MultisampleState, PipelineCompilationOptions, PipelineLayout, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPass, RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, StencilState, TextureFormat, TextureSampleType, TextureViewDimension, VertexState};

#[derive(Debug, Clone, Copy, Hash)]
pub enum DebugMode // debug only?
{
    None,
    Wireframe,
}

pub struct CommonBindLayouts
{
    pub camera: BindGroupLayout,
    pub world_transform: BindGroupLayout,
}

pub struct PipelineCache
{
    renderer: Arc<Renderer>,

    // Todo: better threading solve? (Reference tracking makes this a pain)

    pipeline_layouts: Mutex<HashMap<u64, PipelineLayout>>,
    pipelines: Mutex<HashMap<u64, RenderPipeline>>,

    common_bind_layouts: CommonBindLayouts, // TODO: don't hard-code ?

    bind_groups: Mutex<HashMap<u64, BindGroup>>,
    default_sampler: Sampler,
}
impl PipelineCache
{
    // todo: this differently
    pub fn common_layouts(&self) -> &CommonBindLayouts { &self.common_bind_layouts }
    
    pub fn default_sampler(&self) -> &Sampler { &self.default_sampler }

    pub fn new(renderer: Arc<Renderer>) -> Self
    {
        let common_bind_layouts = CommonBindLayouts
        {
            camera: renderer.device().create_bind_group_layout(&BindGroupLayoutDescriptor
            {
                entries:
                &[
                    wgpu::BindGroupLayoutEntry
                    {
                        binding: 0,
                        visibility: ShaderStages::VERTEX,
                        ty: BindingType::Buffer
                        {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }
                ],
                label: debug_label!("Camera vsh bind layout"),
            }),

            world_transform: renderer.device().create_bind_group_layout(&BindGroupLayoutDescriptor
            {
                entries:
                &[
                    BindGroupLayoutEntry
                    {
                        binding: 0,
                        visibility: ShaderStages::VERTEX,
                        ty: BindingType::Buffer
                        {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: None,
                        },
                        count: None,
                    }
                ],
                label: debug_label!("World transform vsh bind layout"),
            }),
        };

        let default_sampler = Self::create_sampler(&renderer);
        
        Self
        {
            renderer,
            pipeline_layouts: Mutex::default(),
            pipelines: Mutex::default(),
            common_bind_layouts,
            bind_groups: Mutex::default(),
            default_sampler,
        }
    }

    // Try getting or creating a render pipeline and applying it to a render pass
    // Returns false if the pipeline was unable to be created
    pub fn try_apply(&mut self, render_pass: &mut RenderPass, model: &Model, mesh: u32, mode: DebugMode) -> bool
    {
        let model_surf = &model.surfaces[mesh as usize];

        // if shaders change their hashes and in turn asset keys should change
        let pipeline_hash =
        {
            let mut hasher = MetroHash64::default();
            model_surf.vertex_shader.key().hash(&mut hasher);
            model_surf.pixel_shader.key().hash(&mut hasher);
            // mode.hash(&mut hasher); // TODO
            hasher.finish()
        };

        let mut pipelines = self.pipelines.lock();
        if let Some(pipeline) = pipelines.get(&pipeline_hash)
        {
            render_pass.set_pipeline(pipeline);
            return true;
        }

        let AssetPayload::Available(geometry) = model.geometry.payload() else { return false; };
        let AssetPayload::Available(material) = model_surf.material.payload() else { return false; };
        let AssetPayload::Available(vshader) = model_surf.vertex_shader.payload() else { return false; };
        let AssetPayload::Available(pshader) = model_surf.pixel_shader.payload() else { return false; };

        let Some(pipeline) = self.create_pipeline(&geometry.meshes[mesh as usize], &material, &vshader, &pshader, mode) else { return false; };

        render_pass.set_pipeline(&pipeline);
        pipelines.insert(pipeline_hash, pipeline);

        true
    }

    fn create_pipeline(
        &self,
        geometry: &GeometryMesh,
        material: &Material,
        vshader: &Shader,
        pshader: &Shader,
        mode: DebugMode) -> Option<RenderPipeline>
    {
        // move up?
        puffin::profile_scope!("create render pipeline");

        assert_eq!(vshader.stage, ShaderStage::Vertex);
        assert_eq!(pshader.stage, ShaderStage::Pixel);

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
                    &self.common_bind_layouts.camera,
                    &self.common_bind_layouts.world_transform,
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
            label: Some("TODO RenderPass Name"), // TODO
            layout: Some(pipeline_layout),
            vertex: VertexState
            {
                module: &vshader.module,
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
                module: &pshader.module,
                entry_point: ShaderStage::Pixel.entry_point().expect("Shader stage has no entry-point"),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(ColorTargetState
                {
                    format: TextureFormat::Bgra8UnormSrgb, // TODO: based on material, maybe render pass (must match pass), or get from renderer surface format
                    blend: None, // todo: material settings
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None, // todo
        };

        let pipeline = self.renderer.device().create_render_pipeline(&desc);
        Some(pipeline)
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
