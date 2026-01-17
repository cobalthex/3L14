use crate::assets::{Geometry, Material, RenderPassName, Shader, ShaderKey, ShaderStage, VertexLayout};
use crate::uniforms_pool::UniformsPool;
use crate::{debug_label, Renderer};
use debug_3l14::debug_gui::DebugGui;
use egui::Ui;
use metrohash::MetroHash64;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use dashmap::DashMap;
use dashmap::mapref::one::Ref;
use triomphe::Arc;
use enumflags2::BitFlags;
use wgpu::{AddressMode, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferBindingType, BufferSize, ColorTargetState, ColorWrites, CompareFunction, DepthBiasState, DepthStencilState, Face, FilterMode, FragmentState, FrontFace, MultisampleState, PipelineCompilationOptions, PipelineLayout, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPass, RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, StencilState, TextureFormat, TextureSampleType, TextureViewDimension, VertexState};
use asset_3l14::{Ash, AssetPayload, Assets};
use crate::material_classes::{MaterialClass, SimpleOpaque};
use crate::vertex_layouts::VertexLayoutBuilder;

#[derive(Debug, Clone, Copy, Hash)]
pub enum DebugMode // debug only?
{
    None,
    Wireframe,
}

#[derive(Hash, PartialEq, Eq, Copy, Clone)]
pub struct PipelineHash(u64);

enum MaybePipeline
{
    Pending
    { // box?
        vertex_shader: Ash<Shader>,
        pixel_shader: Ash<Shader>,
        vertex_layout: BitFlags<VertexLayout>,
        material_class: MaterialClass,
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

    pipelines: DashMap<PipelineHash, MaybePipeline>,

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
                    MaterialClass::SimpleOpaque => const
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

    pub fn try_apply(&self, render_pass: &mut RenderPass, pipeline_hash: PipelineHash) -> bool
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
        let Some(maybe_pipeline) = self.pipelines.get_mut(&pipeline_hash) else { return false; };
        match &*maybe_pipeline
        {
            MaybePipeline::Pending
            {
                vertex_shader,
                pixel_shader,
                vertex_layout,
                material_class,
            } =>
            {
                if let AssetPayload::Available(vsh) = vertex_shader.payload() &&
                   let AssetPayload::Available(psh) = pixel_shader.payload()
                {
                    let debug_mode = DebugMode::None; // TODO
                    let pipeline = self.create_pipeline(*vertex_layout, *material_class, &vsh, &psh, debug_mode);
                    render_pass.set_pipeline(&pipeline);
                    let _ = self.pipelines.insert(pipeline_hash, MaybePipeline::Created(pipeline));
                }
                else
                {
                    return false;
                }
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
        pass: RenderPassName,
        geometry: &Geometry,
        material: &Material,
        mode: DebugMode) -> PipelineHash
    {
        // if shaders change their hashes and in turn asset keys should change
        let pipeline_hash =
        {
            let mut hasher = MetroHash64::default();
            material.class.hash(&mut hasher);
            mode.hash(&mut hasher);

            PipelineHash(hasher.finish())
        };

        if let None = self.pipelines.get_mut(&pipeline_hash)
        {
            let vsh = ShaderKey::vertex(geometry.vertex_layout, pass);
            let psh = ShaderKey::pixel(material.class, pass);

            let new_pipe = MaybePipeline::Pending
            {
                vertex_layout: geometry.vertex_layout,
                material_class: material.class,
                vertex_shader: self.assets.load(vsh.to_assetkey()),
                pixel_shader: self.assets.load(psh.to_assetkey()),
            };
            self.pipelines.insert(pipeline_hash, new_pipe);
        }

        pipeline_hash
    }

    #[must_use]
    fn create_pipeline(
        &self,
        vertex_layout: BitFlags<VertexLayout>,
        material_class: MaterialClass,
        vertex_shader: &Shader,
        pixel_shader: &Shader,
        mode: DebugMode) -> RenderPipeline
    {
        // move up?
        puffin::profile_scope!("Create render pipeline");

        // if there end up being a lot of pipelines created, it may be worth saving
        let pipeline_layout = self.renderer.device().create_pipeline_layout(&PipelineLayoutDescriptor
        {
            label: debug_label!(&format!("{material_class:?} pipeline layout")),
            bind_group_layouts: &[
                // Todo: define based on render pass
                &self.uniforms.camera_bind_layout,
                &self.uniforms.transform_bind_layout,
                &self.uniforms.poses_bind_layout,
                &self.get_or_create_bind_layout(material_class),
            ],
            push_constant_ranges: &[],
        });

        // todo: if these update, this will invalidate the pipeline
        let renderer_surface_format = self.renderer.surface_format();
        let renderer_msaa_count = self.renderer.msaa_max_sample_count();

        let vbuffers = VertexLayoutBuilder::from(vertex_layout);

        let pipeline = self.renderer.device().create_render_pipeline(&RenderPipelineDescriptor
        {
            label: debug_label!("TODO RenderPipeline Name"), // TODO
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
