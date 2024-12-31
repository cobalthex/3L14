use crate::core::{AssetBuilder, AssetBuilderMeta, BuildOutputs, SourceInput, VersionStrings};
use game_3l14::engine::asset::AssetTypeId;
use game_3l14::engine::graphics::assets::material::MaterialFile;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::Read;

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MaterialBuildConfig
{
}

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
        // TODO: hash the serialized type layouts
        &[
            b"Initial"
        ]
    }
}
impl AssetBuilder for MaterialBuilder
{
    type BuildConfig = MaterialBuildConfig;

    fn build_assets(&self, config: Self::BuildConfig, mut input: &mut SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        let mut mtl_source = String::new();
        input.read_to_string(&mut mtl_source)?;
        let mtl: MaterialFile = toml::from_str(&mtl_source)?;

        let mut mtl_output = outputs.add_output(AssetTypeId::Material)?;
        mtl_output.depends_on_multiple(&mtl.textures);
        mtl_output.serialize(&mtl)?;
        mtl_output.finish()?;
        Ok(())
    }
}