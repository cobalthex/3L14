use std::error::Error;
use std::io::{Read, Write};
use serde::Deserialize;
use game_3l14::engine::assets::AssetTypeId;
use crate::core::{AssetBuilder, BuildOutputs, SourceInput, VersionStrings};

#[derive(Deserialize)]
struct ShaderBuilderConfig
{
}

pub struct ShaderBuilder;
impl AssetBuilder for ShaderBuilder
{
    fn supported_input_file_extensions(&self) -> &'static [&'static str]
    {
        &["hlsl"]
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

    fn build_assets(&self, mut input: SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        let mut src_text = String::new();
        input.read_to_string(&mut src_text)?;

        const ENTRY_POINT: &'static str = "SH_ENTRY"; // TODO

        // TODO: wgsl

        let spirv = hassle_rs::compile_hlsl(
            &input.source_path_string(),
            src_text.as_ref(),
            ENTRY_POINT,
            "ps_6_0",
            &["-spirv"],
            &[])?;

        let mut output = outputs.add_output(AssetTypeId::Shader)?;
        output.write_all(spirv.as_ref())?;
        output.finish()?;

        Ok(())
    }
}