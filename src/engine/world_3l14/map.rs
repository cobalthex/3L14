use bitcode::{Decode, Encode};
use asset_3l14::{Ash, AssetKey, AssetLifecycler};
use containers_3l14::AabbTree;
use graphics_3l14::assets::Model;
use proc_macros_3l14::{asset, LayoutHash};
use std::error::Error;
use crate::Light;

#[repr(u8)]
enum StaticClassification
{
    Model = 0,
    Light = 1,
}

struct Statics
{
    hierarchy: AabbTree,
    geo: Box<[Ash<Model>]>,
    lights: Box<[Light]>,
}
#[derive(Encode, Decode)]
struct StaticsFile
{
    hierarchy: AabbTree,
    geo: Box<[AssetKey]>,
    lights: Box<[Light]>,
}

#[asset]
pub struct Map
{
    statics: Statics,
}
#[derive(LayoutHash, Encode, Decode)]
pub struct MapFile
{
    statics: StaticsFile,
}

pub struct MapLifecycler
{

}
impl AssetLifecycler for MapLifecycler
{
    type Asset = Map;

    fn load(&self, mut request: asset_3l14::AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>>
    {
        let mut input: MapFile = request.deserialize()?;

        let scene = Map
        {
            statics: Statics
            {
                hierarchy: input.statics.hierarchy,
                geo: input.statics.geo.iter().map(|asset_key| request.load_dependency(*asset_key)).collect(),
                lights: input.statics.lights,
            },
        };
        Ok(scene)
    }
}
