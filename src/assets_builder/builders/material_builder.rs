use std::error::Error;
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
        "Initial"
    }

    fn format_version(&self) -> VersionStrings
    {
        "Initial"
    }

    fn build_assets(&self, input: SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {


        todo!()
    }
}