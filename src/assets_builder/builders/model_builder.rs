use std::io::Write;
use game_3l14::engine::assets::AssetTypeId;
use crate::asset_builder::{AssetBuilder, BuildError, BuildOutput, SourceInput, VersionStrings};

pub struct ModelBuilder;
impl AssetBuilder for ModelBuilder
{
    fn supported_input_file_extensions(&self) -> &'static [&'static str]
    {
        &["glb", "gltf"]
    }

    fn builder_version(&self) -> VersionStrings
    {
        &[
            b"Initial"
        ]
    }

    fn format_version(&self) -> VersionStrings
    {
        &[
            b"Initial"
        ]
    }

    fn build_assets(&self, input: SourceInput) -> Result<Iterator<BuildOutput>, BuildError>
    {
        Ok(Vec::new())

        /* TODO

        - how to ID multi-output assets
        - esp if multiple of the same type (custom sub-ID?)

         */
    }
}