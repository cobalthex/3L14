use crate::engine::asset::{Asset, AssetHandle, AssetKey, AssetLifecycler, AssetLoadRequest, AssetTypeId};
use crate::engine::graphics::assets::Texture;
use crate::engine::graphics::colors::Rgba;
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::error::Error;

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
    pub pbr_props: PbrProps, // todo: cbuffer ptr
}
impl Asset for Material
{
    fn asset_type() -> AssetTypeId { AssetTypeId::RenderMaterial }
}

pub struct MaterialLifecycler;
impl AssetLifecycler for MaterialLifecycler
{
    type Asset = Material;

    fn load(&self, mut request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>>
    {
        let mtl_file: MaterialFile = request.deserialize()?;

        let textures = mtl_file.textures.iter().map(|t|
        {
           request.load_dependency::<Texture>(*t)
        });

        Ok(Material
        {
            textures: textures.collect(),
            pbr_props: mtl_file.pbr_props,
        })
    }
}