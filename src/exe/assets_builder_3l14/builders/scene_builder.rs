use std::error::Error;
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};
use asset_3l14::AssetKey;
use world_3l14::{Scene, SceneFile};
use crate::core::{AssetBuilder, AssetBuilderMeta, BuildOutputs, SourceInput, VersionBuilder};

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

pub struct SceneBuilder;
impl AssetBuilder for SceneBuilder
{
    type BuildConfig = SceneBuilderConfig;

    fn build_assets(&self, config: Self::BuildConfig, input: &mut SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        todo!()
    }
}
impl AssetBuilderMeta for SceneBuilder
{
    fn supported_input_file_extensions() -> &'static [&'static str]
    {
        &["scene"]
    }

    fn builder_version(vb: &mut VersionBuilder)
    {
        vb.push(b"Scene builder - initial");
    }

    fn format_version(vb: &mut VersionBuilder)
    {
        vb.push_prehashed(SceneFile::TYPE_LAYOUT_HASH);
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct SceneBuilderConfig
{

}
