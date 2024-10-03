use std::error::Error;
use game_3l14::engine::asset::AssetTypeId;
use game_3l14::engine::graphics::assets::material::MaterialFile;
use crate::core::{AssetBuilder, AssetBuilderMeta, BuildOutputs, SourceInput, VersionStrings};

pub struct MaterialBuilder;
impl AssetBuilderMeta for MaterialBuilder
{
    fn supported_input_file_extensions() -> &'static [&'static str]
    {
        &["matl"]
    }

    fn builder_version() -> VersionStrings
    {
        &[
            b"Initial"
        ]
    }

    fn format_version() -> VersionStrings
    {
        &[
            b"Initial"
        ]
    }
}
impl AssetBuilder for MaterialBuilder
{
    type Config = ();

    fn build_assets(&self, config: Self::Config, input: SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        let mtl_source: MaterialFile = ron::de::from_reader(input)?;
        let mut mtl_output = outputs.add_output(AssetTypeId::RenderMaterial)?;
        mtl_output.serialize(&mtl_source)?;
        mtl_output.finish()?;
        Ok(())
    }
}