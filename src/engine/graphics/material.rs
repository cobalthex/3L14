use crate::engine::assets::{AssetHandle, AssetPayload, HasAssetDependencies};
use crate::engine::graphics::assets::Texture;
use crate::engine::graphics::colors::Color;
use crate::engine::graphics::{colors, Renderer};
use wgpu::{AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, FilterMode, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, TextureAspect, TextureSampleType, TextureViewDescriptor, TextureViewDimension};
// todo: in the future these should be data driven (maybe parametric?)

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

    pub fn get_or_create_bind_group(&self, material: &Material, renderer: &Renderer) -> Option<BindGroup>
    {
        // TODO: let chaining?
        let Some(albedo_map_asset) = &material.albedo_map else { return None; };
        let AssetPayload::Available(albedo_map) = &*albedo_map_asset.payload() else { return None; };

        let bind_group_layout = &self.bind_group_layouts;
        let sampler = &self.samplers;

        Some(renderer.device().create_bind_group(&BindGroupDescriptor
        {
            label: Some("material bind group"),
            layout: bind_group_layout,
            entries:
            &[
                BindGroupEntry
                {
                    binding: 0,
                    resource: BindingResource::TextureView(&albedo_map.gpu_handle().create_view(&TextureViewDescriptor
                    {
                        label: None,
                        format: None,
                        dimension: None,
                        aspect: TextureAspect::default(),
                        base_mip_level: 0,
                        mip_level_count: None,
                        base_array_layer: 0,
                        array_layer_count: None,
                    })),
                },
                BindGroupEntry
                {
                    binding: 1,
                    resource: BindingResource::Sampler(&sampler),
                }
            ],
        }))
    }
}

pub struct Material
{
    pub albedo_map: Option<AssetHandle<Texture>>,
    pub albedo_color: Color,
    pub metallicity: f32,
    pub roughness: f32,
}
impl Default for Material
{
    fn default() -> Self
    {
        Self
        {
            albedo_map: None,
            albedo_color: colors::WHITE,
            metallicity: 0.5,
            roughness: 0.5,
        }
    }
}
impl HasAssetDependencies for Material
{
    fn asset_dependencies_loaded(&self) -> bool
    {
        self.albedo_map.as_ref().map_or(true, |m| m.is_loaded_recursive())
    }
}