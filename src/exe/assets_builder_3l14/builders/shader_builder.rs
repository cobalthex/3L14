use std::fmt::{Debug, Formatter};
use std::error::Error;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use enumflags2::{bitflags, BitFlag, BitFlags};
use graphics_3l14::assets::{ShaderFile, ShaderStage};
use hassle_rs::{Dxc, DxcCompiler, DxcIncludeHandler, DxcLibrary, DxcValidator, Dxil, HassleError};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use asset_3l14::AssetTypeId;
use nab_3l14::utils::ShortTypeName;
use crate::core::{AssetBuilder, BuildOutputs, SourceInput, VersionBuilder};

#[bitflags]
#[repr(u8)]
#[derive(Hash, Copy, Clone, Debug)]
pub enum ShaderCompileFlag
{
    Debug       = 0b0001,
    EmitSymbols = 0b0010,
}

#[derive(Debug)]
pub struct ShaderCompilation<'s>
{
    pub source_text: &'s str,
    pub filename: &'s Path,
    pub stage: ShaderStage,
    pub flags: BitFlags<ShaderCompileFlag>,
    pub defines: Vec<(&'s str, Option<&'s str>)>,
}

struct Includer
{
    pub shaders_root: PathBuf,
}
impl DxcIncludeHandler for Includer
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

#[derive(Serialize, Deserialize)]
pub struct ShaderBuildConfig
{
    pub stage: ShaderStage,
    // TODO: feature flags
}
impl Default for ShaderBuildConfig
{
    fn default() -> Self
    {
        Self
        {
            stage: ShaderStage::Vertex,
        }
    }
}

pub struct ShaderBuilder
{
    includer: Mutex<Includer>,
    dxc_compiler: DxcCompiler,
    dxc_library: DxcLibrary,
    dxc_validator: DxcValidator,
    // these must be at the end b/c ^ don't correctly lifetime these
    dxc: Dxc,
    dxil: Dxil,
}
impl ShaderBuilder
{
    pub fn new(shaders_root: impl Into<PathBuf>, dxc_dir: Option<PathBuf>) -> Result<Self, Box<dyn Error>>
    {
        let dxc = Dxc::new(dxc_dir.clone())?;
        let dxc_compiler = dxc.create_compiler()?;
        let dxc_library = dxc.create_library()?;

        let dxil = Dxil::new(dxc_dir)?;
        let dxc_validator = dxil.create_validator()?;

        Ok(Self
        {
            includer: Mutex::new(Includer { shaders_root: shaders_root.into() }),
            dxc,
            dxil,
            dxc_compiler,
            dxc_library,
            dxc_validator,
        })
    }

    pub fn compile_hlsl(&self, mut compilation: ShaderCompilation) -> Result<Box<[u8]>, Box<dyn Error>>
    {
        // note: mut self only needed for include header, can split out if necessary

        let entry_point = compilation.stage.entry_point();
        let profile = format!("{}_6_0", compilation.stage.prefix());

        let mut dxc_args = vec![
            "-spirv", // emit Spir-V
            "-fspv-target-env=universal1.5",
        ];

        if compilation.flags.contains(ShaderCompileFlag::Debug)
        {
            compilation.defines.push(("DEBUG", Some("1")));
            dxc_args.push("-Od");
        }

        if compilation.flags.contains(ShaderCompileFlag::EmitSymbols)
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

        let mut includer = self.includer.lock();
        let file_path = includer.shaders_root.join(compilation.filename);

        log::debug!("[DXC] Compiling {:?} with arguments {:?}", compilation, dxc_args);

        let blob = self.dxc_library.create_blob_with_encoding_from_str(compilation.source_text)
            .map_err(|e| sc_err(file_path.clone(), compilation.stage, e))?;

        // todo: compile_with_debug
        let spirv = match self.dxc_compiler.compile(
            &blob,
            file_path.to_string_lossy().as_ref(),
            entry_point,
            &profile,
            &dxc_args,
            Some(&mut *includer),
            &compilation.defines,
        )
        {
            Err(result) =>
            {
                let error_blob = result.0.get_error_buffer()?;
                let error_str = self.dxc_library.get_blob_as_string(&error_blob.into())?;
                Err(HassleError::CompileError(error_str))
            },
            Ok(result) =>
            {
                let result_blob = result.get_result()?;
                Ok(result_blob.to_vec()) // todo: This could be no-copy
            }
        }.map_err(|e| sc_err(file_path.clone(), compilation.stage, e))?;

        let blob_encoding = self.dxc_library.create_blob_with_encoding(&spirv)
            .map_err(|e| sc_err(file_path.clone(), compilation.stage, e))?;

        // TODO: currently broken
        // let module = match self.dxc_validator.validate(blob_encoding.into())
        // {
        //     Ok(blob) => Ok(blob.to_vec()), // todo: This could be no-copy
        //     Err(result) =>
        //     {
        //         let error_blob = result.0.get_error_buffer()?;
        //         let error_str = self.dxc_library.get_blob_as_string(&error_blob.into())?;
        //         Err(HassleError::ValidationError(error_str))
        //     }
        // }.map_err(|e| sc_err(file_path.clone(), compilation.stage, e))?;

        Ok(spirv.into_boxed_slice())
    }
}
impl AssetBuilder for ShaderBuilder
{
    type BuildConfig = ShaderBuildConfig;

    fn supported_input_file_extensions(&self) -> &'static [&'static str]
    {
        &["hlsl"]
    }

    fn builder_version(&self, vb: &mut VersionBuilder)
    {
        let dxc_version = interop_3l14::exe_version::get_exe_version("dxcompiler.dll")
            .expect("Failed to get DXC version");
        vb.append(
        &[
            b"DXC", &dxc_version.as_bytes(),
            b"Shader compiler - initial",
        ]);
    }

    fn build_assets(&self, config: Self::BuildConfig, input: &mut SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        // todo: features; permutation for each feature -- possibly simplify into 'sets' of supported features

        let mut source_text = String::new();
        input.read_to_string(&mut source_text)?;

        outputs.add_output(AssetTypeId::Shader, |output|
        {
            let compilation = ShaderCompilation
            {
                source_text: &source_text,
                filename: input.source_path(), // just the filename?
                stage: config.stage,
                flags: ShaderCompileFlag::empty(),
                defines: Vec::new(),
            };

            let module_bytes = self.compile_hlsl(compilation)?;
            output.serialize(&ShaderFile
            {
                stage: config.stage,
                module_bytes,
            })?;

            Ok(())
        })?;

        Ok(())
    }
}

fn sc_err(file_path: PathBuf, stage: ShaderStage, error: HassleError) -> ShaderBuildError
{
    ShaderBuildError
    {
        file_path,
        stage,
        error,
    }
}

#[derive(Debug)]
pub struct ShaderBuildError
{
    pub file_path: PathBuf,
    pub stage: ShaderStage,
    pub error: HassleError,
}
impl std::fmt::Display for ShaderBuildError
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { std::fmt::Debug::fmt(&self, f) }
}
impl Error for ShaderBuildError { }

#[cfg(test)]
mod tests
{
    use std::env;
    use std::path::Path;
    use super::*;

    // #[test]
    // #[cfg(target_os = "windows")] // TODO: cross platform support
    // pub fn compile_vertex_shader()
    // {
    //     let shader_source = r#"
    //     float4 vs_main(float3 in_position : POSITION) : SV_POSITION
    //     {
    //         return float4(in_position, 1.0);
    //     }
    //     "#;
    //
    //     // TODO: clean up, re-use vertion in build_main
    //     let dxc_dir =
    //         {
    //             // construct with Env:CARGO_MANIFEST_DIR \target\ Env:PROFILE ?
    //             let mut out_dir: PathBuf = env::var("OUT_DIR").expect("! Failed to get build target dir").into();
    //             out_dir.push("../../.."); // gross
    //             out_dir.canonicalize().expect("! Failed to canonicalize Env:OUT_DIR")
    //         };
    //
    //     let compiler = ShaderCompiler::new("$$ INVALID $$", Some(dxc_dir)).unwrap();
    //
    //     let mut output = Vec::new();
    //     compiler.compile_hlsl(&mut output, ShaderCompilation
    //     {
    //         source_text: shader_source,
    //         filename: Path::new("TEST_FILE.vs.hlsl"),
    //         stage: ShaderStage::Vertex,
    //         flags: ShaderCompileFlags::none(),
    //         defines: vec![],
    //     }).unwrap();
    // }
}
