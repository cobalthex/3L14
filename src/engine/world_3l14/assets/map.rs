use bitcode::{Decode, Encode};
use asset_3l14::{Ash, AssetKey, AssetLifecycler, AssetLoadRequest};
use proc_macros_3l14::{asset, LayoutHash};
use std::error::Error;
use glam::{Quat, Vec3};
use containers_3l14::AabbTree;
use graphics_3l14::assets::Model;
use crate::Light;

#[asset(structured_type=MapFile)]
pub struct Map
{
    pub statics: Statics,
}

#[derive(LayoutHash, Encode, Decode)]
pub struct MapFile
{
    // FUTURE: Split into map chunks (possibly spatially or by activation)
    pub model_palette: Box<[AssetKey]>, // move to statics?
    pub statics: StaticsFile,
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

pub struct Statics
{
    pub hierarchy: AabbTree,
    pub geo: Box<[StaticPlacement<Ash<Model>>]>,
    pub lights: Box<[StaticPlacement<Light>]>,
}

#[derive(Encode, Decode)]
pub struct StaticsFile
{
    pub hierarchy: AabbTree,
    pub geo: Box<[StaticPlacement<u32>]>,
    pub lights: Box<[StaticPlacement<Light>]>,
}

pub struct MapLifecycler;
impl AssetLifecycler for MapLifecycler
{
    type Asset = Map;

    fn load(&self, mut request: AssetLoadRequest<Self::Asset>) -> Result<Self::Asset, Box<dyn Error>>
    {
        let geo = request.structured_data.statics.geo.iter()
            .map(|placement|
                {
                    let dep = request.load_dependency(request.structured_data.model_palette[placement.object as usize]);
                    StaticPlacement
                    {
                        object: dep,
                        position: placement.position,
                        orientation: placement.orientation,
                        scale: placement.scale,
                    }
                })
            .collect();
        let statics = request.structured_data.statics;
        let map = Map
        {
            statics: Statics
            {
                hierarchy: statics.hierarchy,
                geo,
                lights: statics.lights,
            },
        };
        Ok(map)
    }
}
