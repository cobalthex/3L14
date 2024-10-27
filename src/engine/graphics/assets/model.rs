use std::error::Error;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use bitcode::{Decode, Encode};
use crate::engine::asset::{Asset, AssetHandle, AssetKey, AssetLifecycler, AssetLoadRequest, AssetPayload, AssetTypeId};
use crate::engine::graphics::Renderer;
use super::{Geometry, Material, Shader};

#[derive(Encode, Decode)]
pub struct ModelFile
{
    pub geometry: AssetKey,
    pub material: AssetKey,
    pub vertex_shader: AssetKey,
    pub pixel_shader: AssetKey,
}

pub struct Model
{
    pub geometry: AssetHandle<Geometry>,
    pub material: AssetHandle<Material>,
    pub vertex_shader: AssetHandle<Shader>,
    pub pixel_shader: AssetHandle<Shader>,
}
impl Model
{
    pub fn layout_hash(&self, state: &mut impl Hasher)
    {
        // vertex/pixel shaders must be compatible with geometry and material layouts
        self.vertex_shader.key().hash(state);
        self.pixel_shader.key().hash(state);
    }
}
impl Asset for Model
{
    fn asset_type() -> AssetTypeId { AssetTypeId::Model }
    fn all_dependencies_loaded(&self) -> bool
    {
        self.geometry.is_loaded_recursive() &&
        self.material.is_loaded_recursive() &&
        self.vertex_shader.is_loaded_recursive() &&
        self.pixel_shader.is_loaded_recursive()
    }
}

pub struct ModelLifecycler
{
    renderer: Arc<Renderer>,
}
impl ModelLifecycler
{
    pub fn new(renderer: Arc<Renderer>) -> Self
    {
        Self { renderer }
    }
}
impl AssetLifecycler for ModelLifecycler
{
    type Asset = Model;

    fn load(&self, mut request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>>
    {
        let model_file: ModelFile = request.deserialize()?;
        
        Ok(Model
        {
            geometry: request.load_dependency(model_file.geometry),
            material: request.load_dependency(model_file.material),
            vertex_shader: request.load_dependency(model_file.vertex_shader),
            pixel_shader: request.load_dependency(model_file.pixel_shader),
        })
    }
}