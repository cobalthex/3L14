use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use metrohash::MetroHash64;
use wgpu::{AddressMode, BindGroup, BindGroupDescriptor, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendState, ColorTargetState, ColorWrites, Face, FilterMode, FragmentState, FrontFace, MultisampleState, PipelineCompilationOptions, PipelineLayout, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPass, RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerDescriptor, ShaderStages, TextureFormat, TextureSampleType, TextureViewDimension, VertexState};
use crate::engine::asset::{AssetHandle, AssetPayload};
use crate::engine::graphics::assets::{Material, ShaderStage};
use crate::engine::graphics::{Model, Renderer};

type LayoutHash = u64;

#[derive(Debug, Clone, Copy, Hash)]
pub enum DebugMode // debug only?
{
    None,
    Wireframe,
}

struct Samplers
{
    default: Sampler, // 'quality-backed' sampler
}

pub struct PipelineCache
{
    pipelines: HashMap<u64, RenderPipeline>,
    renderer: Arc<Renderer>,
}
impl PipelineCache
{
    pub fn new(renderer: Arc<Renderer>) -> Self
    {
        Self
        {
            pipelines: HashMap::new(),
            renderer,
        }
    }

    // Try getting or creating a render pipeline and applying it to a render pass
    // Returns false if the pipeline was unable to be created
    pub fn try_apply(&mut self, render_pass: &mut RenderPass, model: &Model, mode: DebugMode) -> bool
    {
        // if shaders change their hashes and in turn asset keys should change
        let layout_hash =
        {
            let mut hasher = MetroHash64::default();
            model.layout_hash(&mut hasher);
            mode.hash(&mut hasher);
            hasher.finish()
        };

        if let Some(pipeline) = self.pipelines.get(&layout_hash)
        {
            render_pass.set_pipeline(pipeline);
            return true;
        }

        let AssetPayload::Available(geometry) = model.geometry.payload() else { return false; };
        let AssetPayload::Available(material) = model.material.payload() else { return false; };
        let AssetPayload::Available(vshader) = model.vertex_shader.payload() else { return false; };
        let AssetPayload::Available(pshader) = model.pixel_shader.payload() else { return false; };

        // move up?
        puffin::profile_scope!("create render pipeline");

        assert_eq!(vshader.stage, ShaderStage::Vertex);
        assert_eq!(pshader.stage, ShaderStage::Pixel);

        let layout_dtor = PipelineLayoutDescriptor
        {
            label: None, // TODO
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        };
        let layout = self.renderer.device().create_pipeline_layout(&layout_dtor);

        let desc = RenderPipelineDescriptor
        {
            label: None, // TODO
            layout: Some(&layout), // TODO
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
            depth_stencil: None, // TODO
            multisample: MultisampleState // TODO
            {
                count: 1,
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
                    format: TextureFormat::Rgba8Unorm, // TODO: based on material, maybe render pass (must match pass)
                    blend: None, // todo: material settings
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None, // todo
        };

        let pipeline = self.renderer.device().create_render_pipeline(&desc);
        render_pass.set_pipeline(&pipeline);
        self.pipelines.insert(layout_hash, pipeline);
        true
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
