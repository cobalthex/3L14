use crate::{debug_label, Renderer};
use arrayvec::ArrayVec;
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::error::Error;
use triomphe::Arc;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{Buffer, BufferUsages};
use asset_3l14::{Ash, Asset, AssetKey, AssetLifecycler, AssetLoadRequest, AssetTypeId};
use debug_3l14::debug_gui::DebugGui;
use crate::assets::Texture;
use crate::material_classes::MaterialClass;

pub const MAX_MATERIAL_TEXTURE_BINDINGS: usize = 16;

#[derive(Serialize, Deserialize, Encode, Decode)]
pub struct MaterialFile
{
    pub class: MaterialClass,
    pub textures: ArrayVec<AssetKey, MAX_MATERIAL_TEXTURE_BINDINGS>,
    pub props: Box<[u8]>,
}

pub struct Material
{
    pub class: MaterialClass,
    pub props: Buffer,
    pub textures: ArrayVec<Ash<Texture>, MAX_MATERIAL_TEXTURE_BINDINGS>,
}
impl Asset for Material
{
    type DebugData = ();
    fn asset_type() -> AssetTypeId { AssetTypeId::Material }
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
    pub fn new(renderer: Arc<Renderer>) -> Self
    {
        Self { renderer }
    }
}
impl AssetLifecycler for MaterialLifecycler
{
    type Asset = Material;

    fn load(&self, mut request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>>
    {
        let mtl_file: MaterialFile = request.deserialize()?;

        let textures: ArrayVec<Ash<Texture>, MAX_MATERIAL_TEXTURE_BINDINGS> = mtl_file.textures.iter().map(|t|
        {
           request.load_dependency(*t)
        }).collect();

        let props = self.renderer.device().create_buffer_init(&BufferInitDescriptor
        {
            label: debug_label!(&format!("{:#?}", request.asset_key)),
            contents: &mtl_file.props,
            usage: BufferUsages::UNIFORM,
        });

        Ok(Material
        {
            class: mtl_file.class,
            textures,
            props,
        })
    }
}
impl DebugGui for MaterialLifecycler
{
    fn display_name(&self) -> &str { "Materials" }
    fn debug_gui(&self, _ui: &mut egui::Ui) { }
}
