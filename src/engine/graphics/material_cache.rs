use std::collections::HashMap;
use std::hash::Hasher;
use std::sync::Arc;
use metrohash::MetroHash64;
use wgpu::{AddressMode, BindGroup, BindGroupDescriptor, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, FilterMode, Sampler, SamplerDescriptor, ShaderStages, TextureSampleType, TextureViewDimension};
use crate::engine::asset::{AssetHandle, AssetPayload};
use crate::engine::graphics::assets::Material;
use crate::engine::graphics::Renderer;

type LayoutHash = u64;

struct Samplers
{
    default: Sampler, // 'quality-backed' sampler
}

pub struct MaterialCache
{
    renderer: Arc<Renderer>,
    bind_group_layouts: HashMap<LayoutHash, BindGroupLayout>,
    samplers: Samplers,
}
impl MaterialCache
{
    pub fn new(renderer: Arc<Renderer>) -> Self
    {
        Self
        {
            bind_group_layouts: HashMap::new(),
            samplers: Samplers
            {
                default: Self::create_sampler(&renderer),
            },
            renderer,
        }
    }

    fn create_bind_group_layout(&mut self, material: &Material) -> &BindGroupLayout
    {
        //todo: texture arrays

        let layout_hash =
        {
            let mut hasher = MetroHash64::new();
            hasher.write_usize(material.textures.len());
            hasher.write_usize(size_of_val(&material.pbr_props)); // todo: layout
            hasher.finish()
        };

        self.bind_group_layouts.entry(layout_hash).or_insert_with(||
        {
            let mut entries = Vec::new();
            for tex in &material.textures
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
        })
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

    pub fn get_or_create_bind_group(&self, material_handle: &AssetHandle<Material>, renderer: &Renderer) -> Option<BindGroup>
    {
        // TODO: placeholder bind groups?

        // todo: the ergonomics of this aren't great

        let material = match material_handle.payload()
        {
            AssetPayload::Pending => { return None; }
            AssetPayload::Unavailable(_) => { return None; }
            AssetPayload::Available(m) => { m }
        };

        let mut texes = Vec::new();
        for tex in &material.textures
        {
            match tex.payload()
            {
                AssetPayload::Available(p) => { texes.push(p); },
                _ => { return None; }
            }
        }

        let mut bind_group_entries = Vec::new();
        bind_group_entries.reserve_exact(texes.len() + 1);
        //
        // for tex in &texes
        // {
        //     bind_group_entries.push(BindGroupEntry
        //     {
        //         binding: bind_group_entries.len() as u32,
        //         resource: BindingResource::TextureView(&tex.gpu_view),
        //     })
        // }
        //
        // bind_group_entries.push(BindGroupEntry
        // {
        //     binding: bind_group_entries.len() as u32,
        //     resource: BindingResource::Sampler(&self.samplers),
        // });

        let bind_group_layout = &self.bind_group_layouts;

        // todo: create a bind group layout for missing texture?

        Some(renderer.device().create_bind_group(&BindGroupDescriptor
        {
            label: Some("material bind group"),
            layout: bind_group_layout,
            entries: &bind_group_entries,
        }))
    }
}
