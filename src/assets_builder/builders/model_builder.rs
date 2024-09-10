use crate::core::{AssetBuilder, BuildError, BuildOutputs, SourceInput};

pub struct ModelBuilder;
impl AssetBuilder for ModelBuilder
{
    fn supported_input_file_extensions(&self) -> &'static [&'static str]
    {
        &["glb", "gltf"]
    }


    fn build_assets(&self, input: SourceInput, outputs: &mut BuildOutputs) -> Result<(), BuildError>
    {
        Ok(())
        /* TODO

        - how to ID multi-output assets
        - esp if multiple of the same type (custom sub-ID?)

         */
    }
}