use crate::engine::asset::{Asset, AssetHandle, AssetKey, AssetLifecycler, AssetLoadRequest, AssetTypeId};
use crate::engine::graphics::assets::Texture;
use crate::engine::graphics::colors::Rgba;
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Arc;
use arrayvec::ArrayVec;
use proc_macros_3l14::FancyEnum;
use wgpu::{BindGroup, BindGroupLayout, BindGroupLayoutEntry, BindingType, Buffer, BufferBindingType, BufferSize, BufferUsages, SamplerBindingType, ShaderStages};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use crate::debug_label;
use crate::engine::graphics::Renderer;

pub const MAX_TEXTURE_BINDINGS: usize = 16;

#[derive(PartialEq, Serialize, Deserialize, Encode, Decode, Debug, FancyEnum)]
pub enum MaterialClass
{
    SimpleOpaque,
}

#[repr(C)]
#[derive(Serialize, Deserialize)]
pub struct PbrProps
{
    pub albedo_color: Rgba,
    pub metallicity: f32,
    pub roughness: f32,
}

#[derive(Serialize, Deserialize, Encode, Decode)]
pub struct MaterialFile
{
    pub class: MaterialClass,
    pub textures: ArrayVec<AssetKey, MAX_TEXTURE_BINDINGS>,
    pub props: Box<[u8]>,
}

pub struct Material
{
    pub class: MaterialClass,
    pub props: Buffer,
    pub textures: ArrayVec<AssetHandle<Texture>, MAX_TEXTURE_BINDINGS>,
    pub bind_group_layout: BindGroupLayout,
    pub bind_group: BindGroup,
}
impl Asset for Material
{
    fn asset_type() -> AssetTypeId { AssetTypeId::RenderMaterial }
    fn all_dependencies_loaded(&self) -> bool
    {
        self.textures.iter().all(|t| t.is_loaded_recursive())
    }
}

pub struct MaterialLifecycler
{
    renderer: Arc<Renderer>,
}
impl MaterialLifecycler
{
    pub fn new(renderer: Arc<Renderer>) -> Self { Self { renderer } }
}
impl AssetLifecycler for MaterialLifecycler
{
    type Asset = Material;

    fn load(&self, mut request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>>
    {
        let mtl_file: MaterialFile = request.deserialize()?;

        let textures: ArrayVec<AssetHandle<Texture>, MAX_TEXTURE_BINDINGS> = mtl_file.textures.iter().map(|t|
        {
           request.load_dependency(*t)
        }).collect();

        let props = self.renderer.device().create_buffer_init(&BufferInitDescriptor
        {
            label: debug_label!(&format!("{:?}", request.asset_key)),
            contents: &mtl_file.props,
            usage: BufferUsages::UNIFORM,
        });

        let mut entries = Vec::with_capacity(textures.len() + 2);

        entries.push(BindGroupLayoutEntry
        {
            binding: entries.len() as u32,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Buffer
            {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: BufferSize::new(props.size()),
            },
            count: None,
        });

        if !textures.is_empty()
        {
            entries.push(BindGroupLayoutEntry
            {
                binding: entries.len() as u32,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            });

            for texture in &textures
            {
                entries.push(BindGroupLayoutEntry
                {
                    binding: entries.len() as u32,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::AccelerationStructure,
                    count: None,
                });
            }
        }

        // TODO: this should be stored on the lifecycler, one per material class
        let bind_group_layout = self.renderer.device().create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor
        {
            label: debug_label!(&format!("{:?} layout", request.asset_key)),
            entries: entries.as_ref(),
        });

        let bind_group = self.renderer.device().create_bind_group(&wgpu::BindGroupDescriptor
        {
            label: debug_label!(&format!("{:?}", request.asset_key)),
            layout: &bind_group_layout,
            entries: &[],
        });

        Ok(Material
        {
            class: mtl_file.class,
            textures,
            props,
            bind_group_layout,
            bind_group,
        })
    }
}