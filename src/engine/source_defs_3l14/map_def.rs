use std::collections::HashMap;
use asset_3l14::AssetKey;
use glam::{Vec3, Quat};
use nab_3l14::Ident;
use serde::{Serialize, Deserialize, Serializer};
use math_3l14::YawPitchRoll;
use world_3l14::Light;

pub const fn default_scale() -> Vec3 { Vec3::ONE }

#[derive(Serialize, Deserialize)]
pub struct StaticPlacement<T>
{
    pub object: T,
    pub position: Vec3,
    #[serde(default)]
    pub orientation: YawPitchRoll,
    #[serde(default = "default_scale")]
    pub scale: Vec3,
}
#[derive(Serialize, Deserialize)]
pub struct EntityPlacement
{
    pub entity: u32, // todo
    pub position: Vec3,
    #[serde(default)]
    pub orientation: YawPitchRoll,
    pub id: Option<Ident>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MapLayer
{
    // activation flags
    pub models: Vec<StaticPlacement<AssetKey>>,
    pub lights: Vec<Light>,
    pub entities: Vec<EntityPlacement>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MapDef
{
    pub name: String,
    // all activation flags

    // reference layers explicitly?
}
