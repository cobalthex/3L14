use std::error::Error;

use bitcode::{Decode, Encode};
use glam::Vec3;
use asset_3l14::{Ash, AssetKey, AssetLifecycler};
use containers_3l14::AabbTree;
use graphics_3l14::assets::Model;
use math_3l14::Angle;
use proc_macros_3l14::asset;

#[derive(Encode, Decode)]
pub enum Light
{
    Point(Vec3),
    Directional(Vec3),
    Spot
    {
        angle: Angle,
        range: f32,
    },
    // rect/disc area lights
}

struct Statics
{
    geo: Box<[Ash<Model>]>,
    lights: Box<[Light]>,
}
#[derive(Encode, Decode)]
struct StaticsFile
{
    geo: Box<[AssetKey]>,
    lights: Box<[Light]>,
}

#[asset]
pub struct Scene
{
    statics: AabbTree<Statics>,
}
#[derive(Encode, Decode)]
struct SceneFile
{
    statics: AabbTree<StaticsFile>,
}

pub struct SceneLifecycler
{

}
impl AssetLifecycler for SceneLifecycler
{
    type Asset = Scene;

    fn load(&self, mut request: asset_3l14::AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>>
    {
        let mut input: SceneFile = request.deserialize()?;

        let scene = Scene
        {
            statics: input.statics.map(|st| Statics
            {
                geo: st.geo.iter().map(|asset_key| request.load_dependency(*asset_key)).collect(),
                lights: st.lights,
            }),
        };
        Ok(scene)
    }
}
