use std::collections::HashMap;
use asset_3l14::AssetKey;
use glam::{Vec3, Quat};
use nab_3l14::Ident;
use serde::{Serialize, Deserialize, Serializer};
use world_3l14::Light;

#[derive(Serialize, Deserialize)]
pub struct ModelPlacement
{
    model: u32,
    position: Vec3,
    orientation: Quat,
    scale: Vec3,
}
#[derive(Serialize, Deserialize)]
pub struct EntityPlacement
{
    entity: u32,
    position: Vec3,
    orientation: Quat,
    id: Ident,
}

#[derive(Serialize, Deserialize)]
pub struct MapLayer
{
    // activation flags
    name: String,
    models: Vec<ModelPlacement>,
    lights: Vec<Light>,
    entities: Vec<EntityPlacement>,
}

#[derive(Serialize, Deserialize)]
pub struct MapDef
{
    pub name: Option<String>,
    pub model_palette: Vec<AssetKey>,
    pub entity_palette: Vec<AssetKey>,
    // all activation flags
}