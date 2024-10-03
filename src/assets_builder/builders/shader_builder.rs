use std::error::Error;
use std::io::{Read, Write};
use serde::{Deserialize, Serialize, Serializer};
use game_3l14::engine::asset::AssetTypeId;
use crate::core::{AssetBuilder, AssetBuilderMeta, BuildOutputs, SourceInput, VersionStrings};

#[derive(Default, Serialize, Deserialize)]
enum ShaderStage
{
    #[default]
    Vertex = 0,
    Pixel = 1, // fragment
    Compute = 2,
}

#[derive(Default, Serialize, Deserialize)]
struct ShaderBuilderConfig
{
    stage: ShaderStage,
    debug: bool,
    emit_symbols: bool,
}

pub struct ShaderBuilder;
impl AssetBuilderMeta for ShaderBuilder
{
    fn supported_input_file_extensions() -> &'static [&'static str]
    {
        &["hlsl"]
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
impl AssetBuilder for ShaderBuilder
{
    type Config = ShaderBuilderConfig;

    fn build_assets(&self, config: Self::Config, mut input: SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        let mut src_text = String::new();
        input.read_to_string(&mut src_text)?;

        // TODO: wgsl

        const ENTRY_POINT_SUFFIX: &'static str = "main";

        // todo: can all this be optimized to const?
        let profile = match config.stage
        {
            ShaderStage::Vertex => "vs",
            ShaderStage::Pixel => "ps",
            ShaderStage::Compute => "cs",
        };

        let entry_point = format!("{profile}_{ENTRY_POINT_SUFFIX}");
        let profile = format!("{profile}_6_5");

        let mut defines = Vec::new();

        let mut dxc_args = vec![
            "-spirv", // emit Spir-V
        ];

        if config.debug
        {
            defines.push(("DEBUG", Some("1")));
            dxc_args.push("-Od");
        }

        if config.emit_symbols
        {
            dxc_args.push("-Zi");
            dxc_args.push("-Zss");
            // dxc_args.push("-Fd");
        }

        // -Fd <file|directory\> - debug info
        // -Zi - debug info
        // -Od - disable optimizations
        // -Zss - Build debug name considering source information

        // matrix ordering? (Zpc vs Zpr for col vs row)

        let spirv = hassle_rs::compile_hlsl(
            &input.source_path_string(),
            src_text.as_ref(),
            &entry_point,
            &profile,
            &dxc_args,
            &defines)?;
        let bytecode = hassle_rs::validate_dxil(&spirv)?;

        let mut output = outputs.add_output(AssetTypeId::Shader)?;
        output.write_all(bytecode.as_ref())?;
        output.finish()?;

        Ok(())
    }
}