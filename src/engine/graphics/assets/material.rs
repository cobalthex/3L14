use crate::engine::asset::{Asset, AssetHandle, AssetKey, AssetLifecycler, AssetLoadRequest, AssetTypeId};
use crate::engine::graphics::assets::Texture;
use crate::engine::graphics::colors::Rgba;
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Arc;
use proc_macros_3l14::FancyEnum;
use wgpu::{Buffer, BufferUsages};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use crate::debug_label;
use crate::engine::graphics::Renderer;

#[derive(PartialEq, Serialize, Deserialize, Encode, Decode, Debug, FancyEnum)]
pub enum MaterialClass
{
    SimpleOpaque,
}
impl MaterialClass
{
    pub fn bind_layout(&self) -> &wgpu::BindGroupLayoutDescriptor
    {
        
        
        todo!()
    }
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
    pub textures: Box<[AssetKey]>,
    pub props: Box<[u8]>,
}

pub struct Material
{
    pub class: MaterialClass,
    pub textures: Box<[AssetHandle<Texture>]>,
    pub props: Buffer,
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

        let textures = mtl_file.textures.iter().map(|t|
        {
           request.load_dependency(*t)
        });

        let props = self.renderer.device().create_buffer_init(&BufferInitDescriptor
        {
            label: debug_label!(&format!("{:?}", request.asset_key)),
            contents: &mtl_file.props,
            usage: BufferUsages::UNIFORM,
        });

        Ok(Material
        {
            class: mtl_file.class,
            textures: textures.collect(),
            props,
        })
    }
}