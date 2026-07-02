use crate::assets::{Geometry, Material, EngineRenderPass, Shader, ShaderStage, shader_key};
use crate::uniforms_pool::UniformsPool;
use crate::{debug_label, Renderer};
use debug_3l14::debug_gui::DebugGui;
use egui::Ui;
use metrohash::MetroHash64;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use arrayvec::ArrayVec;
use dashmap::DashMap;
use dashmap::mapref::one::Ref;
use triomphe::Arc;
use enumflags2::BitFlags;
use wgpu::{AddressMode, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferBindingType, BufferSize, ColorTargetState, ColorWrites, CompareFunction, DepthBiasState, DepthStencilState, Face, FilterMode, FragmentState, FrontFace, MultisampleState, PipelineCompilationOptions, PipelineLayout, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPass, RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, StencilState, TextureFormat, TextureSampleType, TextureViewDimension, VertexState};
use asset_3l14::{Ash, AssetKey, AssetData, AssetTypeId, Assets, AssetSnapshot, AssetView};
use crate::assets::shader_key::pixel;
use crate::material_classes::{MaterialClass, SimpleOpaque};
use crate::vertex_layouts::{VertexCaps, VertexLayoutBuilder};

#[derive(Debug, Clone, Copy, Hash)]
pub enum DebugMode // debug only?
{
    None,
    Wireframe,
}

#[derive(Hash, PartialEq, Eq, Copy, Clone)]
pub struct PipelineKey(u64);

enum MaybePipeline
{
    Pending
    { // box?
        vertex_shader: Ash<Shader>,
        vertex_layout: BitFlags<VertexCaps>,
        material: Option<(MaterialClass, Ash<Shader>)>,
    },
    Created(RenderPipeline),
}

pub struct PipelineCache
{
    renderer: Arc<Renderer>,
    assets: Arc<Assets>,
    pub uniforms: UniformsPool,
    bind_layouts: DashMap<MaterialClass, BindGroupLayout>,

    // TODO: callback from renderer when one of the global settings changes
    // invalidate all pipelines

    pipelines: DashMap<PipelineKey, MaybePipeline>,

    default_sampler: Sampler,
}
impl PipelineCache
{
    pub fn default_sampler(&self) -> &Sampler { &self.default_sampler }

    #[must_use]
    pub fn new(renderer: Arc<Renderer>, assets: Arc<Assets>) -> Self
    {
        let default_sampler = Self::create_sampler(&renderer);

        Self
        {
            renderer: renderer.clone(),
            assets,
            uniforms: UniformsPool::new(renderer),
            bind_layouts: DashMap::new(),
            pipelines: DashMap::new(),
            default_sampler,
        }
    }

    pub fn get_or_create_bind_layout(&self, material_class: MaterialClass) -> Ref<MaterialClass, BindGroupLayout>
    {
        return self.bind_layouts.entry(material_class).or_insert_with(||
        {
            self.renderer.device().create_bind_group_layout(&BindGroupLayoutDescriptor
            {
                label: debug_label!(&format!("{:?} layout", material_class)),
                entries: match material_class
                {
                    MaterialClass::DebugLines => const { &[] }, // todo: uniforms?
                    MaterialClass::PbrOpaque => const
                    {&[
                        uniform::<SimpleOpaque>(0),
                        sampler(1),
                        tex2D(2, "albedo"),
                    ]},

                }
            })
        }).downgrade();

        const fn uniform<T>(binding: u32) -> BindGroupLayoutEntry
        {
            BindGroupLayoutEntry
            {
                binding,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer
                {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: BufferSize::new(size_of::<T>() as u64),
                },
                count: None,
            }
        }
        const fn tex2D(binding: u32, _name: &'static str) -> BindGroupLayoutEntry
        {
            BindGroupLayoutEntry
            {
                binding,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture
                {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }
        }
        const fn sampler(binding: u32) -> BindGroupLayoutEntry
        {
            BindGroupLayoutEntry
            {
                binding,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            }
        }
    }

    pub fn try_apply(&self, render_pass: &mut RenderPass, pipeline_hash: PipelineKey) -> bool
    {
        // fast path
        match self.pipelines.get(&pipeline_hash)
        {
            Some(maybe_pipeline) =>
            {
                if let MaybePipeline::Created(pipeline) = maybe_pipeline.value()
                {
                    render_pass.set_pipeline(pipeline);
                    return true;
                }
            }
            None => { return false; }
        }

        // slow path
        let Some(mut maybe_pipeline) = self.pipelines.get_mut(&pipeline_hash) else { return false; };
        match &*maybe_pipeline
        {
            MaybePipeline::Pending
            {
                vertex_shader,
                vertex_layout,
                material,
            } =>
            {
                let AssetSnapshot::Available(vsh) = vertex_shader.data() else { return false; };
                let mtl = material.as_ref().and_then(|(material_class, pixel_shader)|
                {
                    if let AssetSnapshot::Available(psh) = pixel_shader.data()
                    { 
                        Some((*material_class, psh)) 
                    }
                    else { None }
                });
                let debug_mode = DebugMode::None; // TODO
                let pipeline = self.create_pipeline(*vertex_layout, vsh, mtl, debug_mode);
                render_pass.set_pipeline(&pipeline);
                *maybe_pipeline.value_mut() = MaybePipeline::Created(pipeline);
            }
            MaybePipeline::Created(pipeline) =>
            {
                render_pass.set_pipeline(pipeline);
            }
        }
        true
    }

    // Get or create a pipeline
    pub fn get_or_create(
        &self,
        pass: EngineRenderPass,
        vertex_layout: BitFlags<VertexCaps>,
        material_class: Option<MaterialClass>,
        debug_mode: DebugMode) -> PipelineKey
    {
        // if shaders change their hashes and in turn asset keys should change
        let pipeline_key =
        {
            // TODO: bitmath instead of hashing
            let mut hasher = MetroHash64::default();
            pass.hash(&mut hasher);
            vertex_layout.hash(&mut hasher);
            material_class.hash(&mut hasher);
            debug_mode.hash(&mut hasher);

            PipelineKey(hasher.finish())
        };

        if let None = self.pipelines.get_mut(&pipeline_key)
        {
            let vsh = shader_key::vertex(vertex_layout, pass);
            let material = material_class.map(|mc|
            {
                let key = shader_key::pixel(mc, pass);
                (mc, self.assets.load(AssetKey::synthetic(AssetTypeId::Shader, key)))
            });

            let new_pipe = MaybePipeline::Pending
            {
                vertex_layout,
                vertex_shader: self.assets.load(AssetKey::synthetic(AssetTypeId::Shader, vsh)),
                material,
            };
            self.pipelines.insert(pipeline_key, new_pipe);
        }

        pipeline_key
    }

    #[must_use]
    fn create_pipeline(
        &self,
        vertex_layout: BitFlags<VertexCaps>,
        vertex_shader: AssetView<Shader>,
        material: Option<(MaterialClass, AssetView<Shader>)>,
        debug_mode: DebugMode) -> RenderPipeline
    {
        // move up?
        puffin::profile_scope!("Create render pipeline");

        let mtl_layout = material.as_ref().map(|(class, _)|
            { self.get_or_create_bind_layout(*class) });

        let mut bind_group_layouts: ArrayVec<_, 8> = ArrayVec::new();
        // Todo: define based on render pass
        bind_group_layouts.push(&self.uniforms.camera_bind_layout);
        bind_group_layouts.push(&self.uniforms.transform_bind_layout);
        bind_group_layouts.push(&self.uniforms.poses_bind_layout);

        if let Some(layout) = &mtl_layout
        {
            bind_group_layouts.push(layout.value());
        }

        #[cfg(feature = "debug_gpu_labels")]
        let layout_name = format!("({vertex_layout})+{:?} pipeline", material.as_ref().map(|m| m.0));

        // if there end up being a lot of pipelines created, it may be worth saving
        let pipeline_layout = self.renderer.device().create_pipeline_layout(&PipelineLayoutDescriptor
        {
            // TODO: add pass name + debug mode
            label: debug_label!(&format!("{} layout", layout_name)),
            bind_group_layouts: &bind_group_layouts,
            push_constant_ranges: &[],
        });

        // todo: if these update, this will invalidate the pipeline
        let renderer_surface_format = self.renderer.surface_format();
        let renderer_msaa_count = self.renderer.msaa_max_sample_count();

        let vbuffers = VertexLayoutBuilder::from(vertex_layout);

        // todo: only generate if mtl exists
        let fragment_targets = [Some(ColorTargetState
        {
            format: renderer_surface_format,
            blend: None, // todo: material settings
            write_mask: ColorWrites::ALL,
        })];
        let fragment = material.as_ref().map(|(_, module)| FragmentState
        {
            module: &module.module,
            entry_point: Some(ShaderStage::Pixel.entry_point()),
            compilation_options: PipelineCompilationOptions::default(),
            targets: &fragment_targets,
        });

        let pipeline = self.renderer.device().create_render_pipeline(&RenderPipelineDescriptor
        {
            label: debug_label!(&layout_name),
            layout: Some(&pipeline_layout),
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
                cull_mode: Some(Face::Back), // don't cull wireframe?
                unclipped_depth: false,
                polygon_mode: match debug_mode
                {
                    DebugMode::Wireframe => PolygonMode::Line,
                    _ => PolygonMode::Fill,
                },
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
            fragment,
            multiview: None,
            cache: None, // todo
        });

        pipeline
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
        ui.label(format!("Pipelines: {}", self.pipelines.len()));

        ui.collapsing(self.uniforms.display_name(), |cui| self.uniforms.debug_gui(cui));
    }
}
