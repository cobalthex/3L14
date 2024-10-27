use std::error::Error;
use bitcode::{Decode, Encode};
use crate::engine::asset::{Asset, AssetHandle, AssetKey, AssetLifecycler, AssetLoadRequest, AssetTypeId};
use super::{Material, Model, RenderPipeline};

#[derive(Encode, Decode)]
struct ModelLookFileMesh
{
    pub material: AssetKey,
    pub pipeline: AssetKey,
}

#[derive(Encode, Decode)]
pub struct ModelLookFile
{
    pub model: AssetKey,
    pub meshes: Box<[ModelLookFileMesh]>,
}

pub struct ModelLookMesh
{
    pub material: AssetHandle<Material>,
    pub pipeline: AssetHandle<RenderPipeline>,
}

pub struct ModelLook
{
    pub model: AssetHandle<Model>,
    pub meshes: Box<[ModelLookMesh]>,
}
impl Asset for ModelLook
{
    fn asset_type() -> AssetTypeId { AssetTypeId::ModelLook }

    fn all_dependencies_loaded(&self) -> bool
    {
        self.model.is_loaded_recursive() &&
            self.meshes.iter().all(|mesh| 
                mesh.pipeline.is_loaded_recursive() && mesh.material.is_loaded_recursive())
    }
}

pub struct ModelLookLifecycler;
impl AssetLifecycler for ModelLookLifecycler
{
    type Asset = ModelLook;

    fn load(&self, mut request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>>
    {
        let look_file: ModelLookFile = request.deserialize()?;

        let meshes = look_file.meshes.iter().map(|m|
        {
            ModelLookMesh
            {
                material: request.load_dependency(m.material),
                pipeline: request.load_dependency(m.pipeline),
            }
        });

        Ok(ModelLook
        {
            model: request.load_dependency(look_file.model),
            meshes: meshes.collect(),
        })
    }
}