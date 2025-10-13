use crate::{debug_label, Renderer};
use bitcode::{Decode, Encode};
use proc_macros_3l14::{asset, FancyEnum};
use serde::{Deserialize, Serialize};
use std::default::Default;
use std::error::Error;
use triomphe::Arc;
use wgpu::util::{make_spirv, make_spirv_raw};
use wgpu::{ShaderModuleDescriptor, ShaderModuleDescriptorPassthrough};
use wgpu::wgt::ShaderModuleDescriptorSpirV;
use asset_3l14::{AssetLifecycler, AssetLoadRequest};
use debug_3l14::debug_gui::DebugGui;

#[derive(Default, Debug, PartialEq, Hash, Clone, Copy, Serialize, Deserialize, Encode, Decode, FancyEnum)]
pub enum ShaderStage
{
    #[default]
    #[enum_prop(prefix = "vs", entry_point="vs_main")]
    Vertex,
    #[enum_prop(prefix = "ps", entry_point="ps_main")]
    Pixel, // fragment
    #[enum_prop(prefix = "cs", entry_point="cs_main")]
    Compute,
}

#[derive(Encode, Decode, Debug)]
pub struct ShaderFile
{
    pub stage: ShaderStage,
    pub module_bytes: Box<[u8]>,
    pub module_hash: u64,
}

#[asset(debug_type = ShaderDebugData)]
pub struct Shader
{
    pub stage: ShaderStage,
    pub module: wgpu::ShaderModule,
    pub module_hash: u64, // likely duplicates asset key but oh well
}

#[derive(Encode, Decode)]
pub struct ShaderDebugData
{
    pub source_file: String,
}

pub struct ShaderLifecycler
{
    renderer: Arc<Renderer>,
}
impl ShaderLifecycler
{

    pub fn new(renderer: Arc<Renderer>) -> Self
    {
        Self { renderer }
    }
}
impl AssetLifecycler for ShaderLifecycler
{
    type Asset = Shader;

    fn load(&self, mut request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>>
    {
        let shader_file: ShaderFile = request.deserialize()?;

        let module = match
            cfg!(feature = "load_shaders_directly") &&
            self.renderer.supports_feature(wgpu::Features::SPIRV_SHADER_PASSTHROUGH)
        {
            true => unsafe
            {
                self.renderer.device().create_shader_module_passthrough(ShaderModuleDescriptorPassthrough::SpirV(
                ShaderModuleDescriptorSpirV {
                    label: debug_label!(&format!("{:?} ({:?})", request.asset_key, shader_file.stage)),
                    source: make_spirv_raw(&shader_file.module_bytes),
                }))
            },
            false =>
            {
                self.renderer.device().create_shader_module(ShaderModuleDescriptor
                {
                    label: debug_label!(&format!("{:?} ({:?})", request.asset_key, shader_file.stage)),
                    source: make_spirv(&shader_file.module_bytes),
                })
            }
        };

        Ok(Shader
        {
            stage: shader_file.stage,
            module,
            module_hash: shader_file.module_hash,
        })
    }
}
impl DebugGui for ShaderLifecycler
{
    fn display_name(&self) -> &str { "Shaders" }
    fn debug_gui(&self, _ui: &mut egui::Ui) { }
}