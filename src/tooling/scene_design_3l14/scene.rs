use asset_3l14::AssetKey;
use glam::{Vec3, Quat};
use nab_3l14::Ident;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct StaticPlacement
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
pub struct SceneLayer
{
    pub name: String,
    // activation flags
    statics: Vec<StaticPlacement>,
    entities: Vec<EntityPlacement>,
}

#[derive(Serialize, Deserialize)]
pub struct Scene
{
    pub model_palette: Vec<AssetKey>,
    pub entity_palette: Vec<AssetKey>,
    pub layers: Vec<SceneLayer>,
    // all activation flags
}
