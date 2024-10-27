use std::collections::HashMap;
use std::hash::Hasher;
use std::sync::Arc;
use metrohash::MetroHash64;
use wgpu::{AddressMode, BindGroup, BindGroupDescriptor, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, FilterMode, RenderPipeline, Sampler, SamplerDescriptor, ShaderStages, TextureSampleType, TextureViewDimension};
use crate::engine::asset::{AssetHandle, AssetPayload};
use crate::engine::graphics::assets::Material;
use crate::engine::graphics::{Model, Renderer};

type LayoutHash = u64;

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

    fn get_or_create_pipeline(&mut self, model: &Model) -> Option<&RenderPipeline> // return DataPayload?
    {
        let layout_hash =
        {
            let mut hasher = MetroHash64::default();
            model.layout_hash(&mut hasher);
            hasher.finish()
        };

        if let Some(pipeline) = self.pipelines.get(&layout_hash)
        {
            return Some(pipeline);
        }

        let AssetPayload::Available(geometry) = model.geometry.payload() else { return None; };
        let AssetPayload::Available(material) = model.material.payload() else { return None; };

        let layout = self.pipelines.entry(layout_hash).or_insert_with(||
        {
            let mut entries = Vec::new();
            for tex in &model.material.textures
            {
                entries.push(BindGroupLayoutEntry
                {
                    binding: entries.len() as u32,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture
                    {
                        sample_type: TextureSampleType::Float { filterable: true }, // TODO: material props
                        view_dimension: TextureViewDimension::D2, // TODO: get from texture
                        multisampled: false,
                    },
                    count: None,
                });
            }

            self.renderer.device().create_bind_group_layout(&BindGroupLayoutDescriptor
            {
                label: Some("TEST -- texture bind group layout"),
                entries: &entries,
            })
        });
        Some(layout)
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
