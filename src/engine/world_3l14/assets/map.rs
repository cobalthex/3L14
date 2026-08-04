use bitcode::{Decode, Encode};
use asset_3l14::{Ash, AssetKey, AssetLifecycler, AssetLoadRequest};
use proc_macros_3l14::{asset, LayoutHash};
use std::error::Error;
use containers_3l14::AabbTree;
use graphics_3l14::assets::Model;
use crate::Light;

#[asset]
pub struct Map
{
    pub statics: Statics,
}

#[derive(LayoutHash, Encode, Decode)]
pub struct MapFile
{
    // FUTURE: Split into map chunks (possibly spatially or by activation)
    pub statics: StaticsFile,
}

#[repr(u8)]
pub enum StaticClassification
{
    Model = 0,
    Light = 1,
}

pub struct Statics
{
    pub hierarchy: AabbTree,
    pub geo: Box<[Ash<Model>]>,
    pub lights: Box<[Light]>,
}

#[derive(Encode, Decode)]
struct StaticsFile
{
    pub hierarchy: AabbTree,
    pub geo: Box<[AssetKey]>,
    pub lights: Box<[Light]>,
}

pub struct MapLifecycler;
impl AssetLifecycler for MapLifecycler
{
    type Asset = Map;

    fn load(&self, mut request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>>
    {
        let input: MapFile = request.deserialize()?;
        let map = Map
        {
            statics: Statics
            {
                hierarchy: input.statics.hierarchy,
                geo: input.statics.geo.iter().map(|asset_key| request.load_dependency(*asset_key)).collect(),
                lights: input.statics.lights,
            },
        };
        Ok(map)
    }
}
