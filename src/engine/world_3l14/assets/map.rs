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
struct Placement<T>
{
    pub object: T,
    pub position: Vec3,
    pub orientation: Quat,
    // scale?
}

pub struct Statics
{
    pub hierarchy: AabbTree,
    pub geo: Box<[Placement<Ash<Model>>]>,
    pub lights: Box<[Placement<Light>]>,
}

#[derive(Encode, Decode)]
pub struct StaticsFile
{
    pub hierarchy: AabbTree,
    pub geo: Box<[Placement<u32>]>,
    pub lights: Box<[Placement<Light>]>,
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
                    Placement
                    {
                        object: dep,
                        position: placement.position,
                        orientation: placement.orientation,
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
