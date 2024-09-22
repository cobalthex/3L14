use std::error::Error;
use game_3l14::engine::assets::AssetTypeId;
use game_3l14::engine::graphics::material::MaterialFile;
use crate::core::{AssetBuilder, BuildOutputs, SourceInput, VersionStrings};

pub struct MaterialBuilder;
impl AssetBuilder for MaterialBuilder
{
    fn supported_input_file_extensions(&self) -> &'static [&'static str]
    {
        &["matl"]
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

    fn build_assets(&self, input: SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        let mtl_source: MaterialFile = ron::de::from_reader(input)?;
        let mut mtl_output = outputs.add_output(AssetTypeId::RenderMaterial)?;
        mtl_output.serialize(&mtl_source)?;
        mtl_output.finish()?;
        Ok(())
    }
}