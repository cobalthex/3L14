use std::error::Error;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use hassle_rs::{Dxc, DxcCompiler, DxcIncludeHandler, DxcLibrary, DxcValidator, Dxil, HassleError};
use game_3l14::engine::graphics::assets::ShaderStage;

pub struct ShaderCompilation<'s>
{
    pub source: &'s str,
    pub filename: &'s str,
    pub include_root: &'s Path,
    pub stage: ShaderStage,
    pub debug: bool,
    pub emit_symbols: bool,
    pub defines: Vec<(&'s str, Option<&'s str>)>,
}

// shader feature flags (turn into constants)
// pre-defined vertex layouts

pub struct ShaderCompiler
{
    shaders_root: PathBuf,
    dxc_compiler: DxcCompiler,
    dxc_library: DxcLibrary,
    dxc_validator: DxcValidator,
}
impl ShaderCompiler
{
    pub fn new(shaders_root: impl AsRef<Path>) -> Result<Self, impl Error>
    {
        let dxc = Dxc::new(None)?; // TODO: specify path
        let dxc_compiler = dxc.create_compiler()?;
        let dxc_library = dxc.create_library()?;

        let dxil = Dxil::new(None)?; // TODO: specify path
        let dxc_validator = dxil.create_validator()?;

        Ok(Self
        {
            shaders_root: shaders_root.as_ref().to_path_buf(),
            dxc_compiler,
            dxc_library,
            dxc_validator,
        })
    }

    pub fn compile_hlsl(&mut self, output: &mut impl Write, compilation: ShaderCompilation) -> Result<usize, Box<dyn Error>>
    {
        // note: mut self only needed for include header, can split out if necessary

        let entry_point = compilation.stage.entry_point();
        let profile = format!("{}_6_0", compilation.stage.prefix());

        let mut defines = compilation.defines;

        let mut dxc_args = vec![
            "-spirv", // emit Spir-V
            "-fspv-target-env=universal1.5",
        ];

        if compilation.debug
        {
            defines.push(("DEBUG", Some("1")));
            dxc_args.push("-Od");
        }

        if compilation.emit_symbols
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

        let blob = self.dxc_library.create_blob_with_encoding_from_str(compilation.source)?;

        let spirv = match self.dxc_compiler.compile(
            &blob,
            compilation.filename,
            &entry_point,
            &profile,
            &dxc_args,
            Some(&mut self),
            &defines,
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
        }?;


        let blob_encoding = self.dxc_library.create_blob_with_encoding(&spirv)?;

        let module = spirv;
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
        // }?;
        output.write_all(&module)?;
        Ok(module.len())
    }
}
impl DxcIncludeHandler for ShaderCompiler
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
