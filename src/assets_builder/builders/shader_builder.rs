use crate::core::{AssetBuilder, AssetBuilderMeta, BuildOutputs, SourceInput, VersionStrings};
use game_3l14::engine::asset::AssetTypeId;
use game_3l14::engine::graphics::assets::{ShaderFile, ShaderStage};
use hassle_rs::{Dxc, DxcIncludeHandler, Dxil, HassleError};
use serde::{Deserialize, Serialize, Serializer};
use std::error::Error;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ShaderBuildConfig
{
    stage: ShaderStage,
    debug: bool,
    emit_symbols: bool,
    skip_validation: bool,
}

struct IncludeHandler<'a>
{
    shaders_root: &'a Path,
}
impl<'a> DxcIncludeHandler for IncludeHandler<'a>
{
    fn load_source(&mut self, filename: String) -> Option<String>
    {
        match std::fs::File::open(self.shaders_root.join(filename))
        {
            Ok(mut f) =>
            {
                let mut content = String::new();
                f.read_to_string(&mut content).ok()?;
                Some(content)
            }
            Err(_) => None,
        }
    }
}

pub struct ShaderBuilder
{
    shaders_root: PathBuf,
}
impl ShaderBuilder
{
    pub fn new(assets_root: impl AsRef<Path>) -> Self
    {
        Self
        {
            shaders_root: assets_root.as_ref().to_path_buf(),
        }
    }
}
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
        // TODO: hash the serialized type layouts
        &[
            b"Initial"
        ]
    }
}
impl AssetBuilder for ShaderBuilder
{
    type BuildConfig = ShaderBuildConfig;

    fn build_assets(&self, config: Self::BuildConfig, input: &mut SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        let mut src_text = String::new();
        input.read_to_string(&mut src_text)?;

        // TODO: wgsl

        let entry_point = config.stage.entry_point();
        let profile = format!("{}_6_0", config.stage.prefix());

        let mut defines = Vec::new();

        let mut dxc_args = vec![
            "-spirv", // emit Spir-V
            "-fspv-target-env=universal1.5",
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
            // dxc_args.push("-fspv-debug=line");
        }

        // -Fd <file|directory\> - debug info
        // -Zi - debug info
        // -Od - disable optimizations
        // -Zss - Build debug name considering source information

        // matrix ordering? (Zpc vs Zpr for col vs row)

        let dxc = Dxc::new(None)?; // TODO: specify path
        let dxc_compiler = dxc.create_compiler()?;
        let dxc_library = dxc.create_library()?;

        let blob = dxc_library.create_blob_with_encoding_from_str(src_text.as_ref())?;

        let mut include_handler = IncludeHandler { shaders_root: self.shaders_root.as_path() };

        let spirv = match dxc_compiler.compile(
            &blob,
            &input.source_path_string(),
            &entry_point,
            &profile,
            &dxc_args,
            Some(&mut include_handler),
            &defines,
        )
        {
            Err(result) =>
            {
                let error_blob = result.0.get_error_buffer()?;
                let error_str = dxc_library.get_blob_as_string(&error_blob.into())?;
                Err(HassleError::CompileError(error_str))
            },
            Ok(result) =>
            {
                let result_blob = result.get_result()?;
                Ok(result_blob.to_vec()) // todo: This could be no-copy
            }
        }?;

        let dxil = Dxil::new(None)?; // TODO: specify path
        let dxc_validator = dxil.create_validator()?;

        let blob_encoding = dxc_library.create_blob_with_encoding(&spirv)?;

        let module = spirv;
        // let module = match dxc_validator.validate(blob_encoding.into())
        // {
        //     Ok(blob) => Ok(blob.to_vec()), // todo: This could be no-copy
        //     Err(result) =>
        //     {
        //         let error_blob = result.0.get_error_buffer()?;
        //         let error_str = dxc_library.get_blob_as_string(&error_blob.into())?;
        //         Err(HassleError::ValidationError(error_str))
        //     }
        // }?;

        let mut output = outputs.add_output(AssetTypeId::Shader)?;

        output.serialize(&ShaderFile
        {
            stage: config.stage,
        })?;

        output.write_all(module.as_ref())?;
        output.finish()?;

        Ok(())
    }
}