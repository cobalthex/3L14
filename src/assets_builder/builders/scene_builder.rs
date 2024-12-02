use std::error::Error;
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};
use game_3l14::engine::asset::AssetKey;
use crate::core::{AssetBuilder, BuildOutputs, SourceInput};

#[derive(Serialize, Deserialize)]
pub struct StaticPlacement
{
    model: AssetKey,
    position: Vec3,
    orientation: Quat,
    scale: Vec3,
}

#[derive(Serialize, Deserialize)]
pub struct SceneDescFile
{
    static_placements: Box<[StaticPlacement]>,
}

pub struct SceneBuilder
{

}
impl AssetBuilder for SceneBuilder
{
    type BuildConfig = ();

    fn build_assets(&self, config: Self::BuildConfig, input: &mut SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        todo!()
    }
}