use std::collections::HashMap;
use asset_3l14::AssetKey;
use glam::{Vec3, Quat};
use nab_3l14::Ident;
use serde::{Serialize, Deserialize, Serializer};
use math_3l14::YawPitchRoll;
use world_3l14::Light;

#[derive(Serialize, Deserialize)]
pub struct Placement<T>
{
    pub object: T,
    pub position: Vec3,
    pub orientation: YawPitchRoll,
    pub scale: Vec3,
}
#[derive(Serialize, Deserialize)]
pub struct EntityPlacement
{
    pub entity: u32, // todo
    pub position: Vec3,
    pub orientation: YawPitchRoll,
    pub id: Ident,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MapLayer
{
    // activation flags
    pub models: Vec<Placement<AssetKey>>,
    pub lights: Vec<Light>,
    pub entities: Vec<EntityPlacement>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MapDef
{
    pub name: String,
    // all activation flags
}
