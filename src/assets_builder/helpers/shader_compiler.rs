use std::error::Error;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use hassle_rs::{Dxc, DxcCompiler, DxcIncludeHandler, DxcLibrary, DxcValidator, Dxil, HassleError};
use parking_lot::Mutex;
use game_3l14::engine::graphics::assets::ShaderStage;

pub struct ShaderCompilation<'s>
{
    pub source_text: &'s str,
    pub filename: &'s Path,
    pub stage: ShaderStage,
    pub debug: bool,
    pub emit_symbols: bool,
    pub defines: Vec<(&'s str, Option<&'s str>)>,
}

// shader feature flags (turn into constants)
// pre-defined vertex layouts

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


pub struct ShaderCompiler
{
    includer: Mutex<Includer>,
    dxc_compiler: DxcCompiler,
    dxc_library: DxcLibrary,
    dxc_validator: DxcValidator,
    // these must be at the end b/c ^ don't correctly lifetime these
    dxc: Dxc,
    dxil: Dxil,
}
impl ShaderCompiler
{
    pub fn new(shaders_root: impl AsRef<Path>, dxc_dir: Option<PathBuf>) -> Result<Self, Box<dyn Error>>
    {
        let dxc = Dxc::new(dxc_dir.clone())?;
        let dxc_compiler = dxc.create_compiler()?;
        let dxc_library = dxc.create_library()?;

        let dxil = Dxil::new(dxc_dir)?;
        let dxc_validator = dxil.create_validator()?;

        Ok(Self
        {
            includer: Mutex::new(Includer { shaders_root: shaders_root.as_ref().to_path_buf() }),
            dxc,
            dxil,
            dxc_compiler,
            dxc_library,
            dxc_validator,
        })
    }

    pub fn compile_hlsl(&self, output: &mut impl Write, compilation: ShaderCompilation) -> Result<usize, Box<dyn Error>>
    {
        // note: mut self only needed for include header, can split out if necessary

        let entry_point = compilation.stage.entry_point();
        let profile = format!("{}_6_0", compilation.stage.prefix().expect("Shader stage does not have a profile prefix!"));

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

        let mut includer = self.includer.lock();
        let file_path = includer.shaders_root.join(compilation.filename);

        let blob = self.dxc_library.create_blob_with_encoding_from_str(compilation.source_text).map_err(|e| sc_err(file_path.clone(), compilation.stage, e))?;

        // todo: compile_with_debug
        let spirv = match self.dxc_compiler.compile(
            &blob,
            file_path.to_string_lossy().as_ref(),
            &entry_point.expect("Shader stage does not have a entry point!"),
            &profile,
            &dxc_args,
            Some(&mut *includer),
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
        }.map_err(|e| sc_err(file_path.clone(), compilation.stage, e))?;

        let blob_encoding = self.dxc_library.create_blob_with_encoding(&spirv).map_err(|e| sc_err(file_path.clone(), compilation.stage, e))?;

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
        // }.map_err(|e| sc_err(file_path.clone(), compilation.stage, e))?;
        output.write_all(&module)?;
        Ok(module.len())
    }
}

fn sc_err(file_path: PathBuf, stage: ShaderStage, error: HassleError) -> ShaderCompilationError
{
    ShaderCompilationError
    {
        file_path,
        stage,
        error,
    }
}

#[derive(Debug)]
pub struct ShaderCompilationError
{
    pub file_path: PathBuf,
    pub stage: ShaderStage,
    pub error: HassleError,
}
impl std::fmt::Display for ShaderCompilationError
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { std::fmt::Debug::fmt(&self, f) }
}
impl Error for ShaderCompilationError { }

#[cfg(test)]
mod tests
{
    use std::path::Path;
    use super::*;

    #[test]
    pub fn compile_vertex_shader()
    {
        let shader_source = r#"
        float4 vs_main(float3 in_position : POSITION) : SV_POSITION
        {
            return float4(in_position, 1.0);
        }
        "#;

        let dxc_dir = "3rdparty/dxc"; // hacky

        let compiler = ShaderCompiler::new("$$ INVALID $$", Some(dxc_dir.into())).unwrap();

        let mut output = Vec::new();
        compiler.compile_hlsl(&mut output, ShaderCompilation
        {
            source_text: shader_source,
            filename: Path::new("TEST_FILE.vs.hlsl"),
            stage: ShaderStage::Vertex,
            debug: false,
            emit_symbols: false,
            defines: vec![],
        }).unwrap();
    }
}