use std::error::Error;
use serde::{Deserialize, Serialize};
use crate::core::{AssetBuilder, BuildOutputs, SourceInput, VersionBuilder};
use world_3l14::SceneFile;

pub struct SceneBuilder;
impl AssetBuilder for SceneBuilder
{
    type BuildConfig = SceneBuilderConfig;

    fn build_assets(&self, config: Self::BuildConfig, input: &mut SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        todo!()
    }

    fn supported_input_file_extensions(&self) -> &'static [&'static str]
    {
        &["scene"]
    }

    fn builder_version(&self, vb: &mut VersionBuilder)
    {
        vb.push(b"Scene builder - initial");
    }

    fn format_version(&self, vb: &mut VersionBuilder)
    {
        vb.push_prehashed(SceneFile::TYPE_LAYOUT_HASH);
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct SceneBuilderConfig
{

}
