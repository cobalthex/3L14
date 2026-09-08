use bitcode::{Decode, Encode};
use asset_3l14::{Ash, AssetKey, AssetLifecycler, AssetLoadRequest};
use proc_macros_3l14::{asset, LayoutHash};
use std::error::Error;
use glam::{Quat, Vec3};
use containers_3l14::AabbTree;
use crate::light::Light;

#[asset]
#[derive(LayoutHash, Encode, Decode)]
pub struct Map
{
    pub model_palette: Box<[AssetKey]>, // move to statics?
    pub statics: Statics,
}

#[repr(u8)]
pub enum StaticClassification
{
    Model = 0,
    Light = 1,
}

#[derive(Encode, Decode)]
pub struct StaticPlacement<T>
{
    pub object: T,
    pub position: Vec3,
    pub orientation: Quat,
    pub scale: Vec3,
}

#[derive(Encode, Decode)]
pub struct Statics
{
    pub hierarchy: AabbTree,
    pub geo: Box<[StaticPlacement<u32>]>,
    pub lights: Box<[StaticPlacement<Light>]>,
}

#[derive(Default)]
pub struct MapLifecycler
{
}
impl AssetLifecycler for MapLifecycler
{
    type Asset = Map;

    fn load(&self, request: AssetLoadRequest<Self::Asset>) -> Result<Self::Asset, Box<dyn Error>>
    {
        let map = Map
        {
            model_palette: request.structured_data.model_palette,
            statics: request.structured_data.statics,
        };
        Ok(map)
    }
}
