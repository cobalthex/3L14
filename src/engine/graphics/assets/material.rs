use crate::engine::assets::{Asset, AssetHandle, AssetKey, AssetLifecycler, AssetLoadError, AssetLoadRequest, AssetPayload, AssetTypeId};
use crate::engine::graphics::assets::Texture;
use crate::engine::graphics::colors::Rgba;
use crate::engine::graphics::Renderer;
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use wgpu::{AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, FilterMode, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, TextureSampleType, TextureViewDimension};

pub struct MaterialCache
{
    // TODO: runtime modified hashmap
    // TODO: figure out how to create render passes
    pub bind_group_layouts: BindGroupLayout,
    pub samplers: Sampler,
}
impl MaterialCache
{
    pub fn new(renderer: &Renderer) -> Self
    {
        Self
        {
            bind_group_layouts: Self::create_bind_group_layout(renderer),
            samplers: Self::create_sampler(renderer),
        }
    }

    fn create_bind_group_layout(renderer: &Renderer) -> BindGroupLayout
    {
        renderer.device().create_bind_group_layout(&BindGroupLayoutDescriptor
        {
            label: Some("texture bind group layout"),
            entries:
            &[
                BindGroupLayoutEntry
                {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture
                    {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::default(),
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry
                {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                }
            ],
        })
    }

    fn create_sampler(renderer: &Renderer) -> Sampler
    {
        renderer.device().create_sampler(&SamplerDescriptor
        {
            label: Some("sampler"),
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

    pub fn get_or_create_bind_group<'m>(&self, material: &'m Material, renderer: &Renderer) -> Option<BindGroup>
    {
        let mut bind_group_entries = Vec::new();
        for tex in &material.textures
        {
            let AssetPayload::Available(tex_payload) = &*tex.payload() else
            {
                return None;
            };

            bind_group_entries.push(BindGroupEntry
            {
                binding: bind_group_entries.len() as u32,
                resource: BindingResource::TextureView(&tex_payload.gpu_view),
            })
        }

        bind_group_entries.push(BindGroupEntry
        {
            binding: bind_group_entries.len() as u32,
            resource: BindingResource::Sampler(&self.samplers),
        });

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

#[derive(Serialize, Deserialize, Encode, Decode)]
pub struct PbrProps
{
    pub albedo_color: Rgba,
    pub metallicity: f32,
    pub roughness: f32,
}

#[derive(Serialize, Deserialize, Encode, Decode)]
pub struct MaterialFile
{
    pub textures: Box<[AssetKey]>,
    pub pbr_props: PbrProps,
}

pub struct Material
{
    pub textures: Box<[AssetHandle<Texture>]>,
    pub prb_props: PbrProps, // todo: cbuffer ptr
}
impl Asset for Material
{
    fn asset_type() -> AssetTypeId { AssetTypeId::RenderMaterial }
}

struct MaterialLifecycler;
impl AssetLifecycler for MaterialLifecycler
{
    type Asset = Material;

    fn load(&self, mut request: AssetLoadRequest) -> AssetPayload<Self::Asset>
    {
        let mtl_file: MaterialFile = match request.deserialize()
        {
            Ok(m) => m,
            Err(err) =>
            {
                eprintln!("Error deserializing material: {err}");
                return AssetPayload::Unavailable(AssetLoadError::ParseError(0));
            }
        };

        let textures = mtl_file.textures.iter().map(|t|
        {
           request.load_dependency::<Texture>(*t)
        });
        
        AssetPayload::Available(Material
        {
            textures: textures.collect(),
            prb_props: mtl_file.pbr_props,
        })
    }
}