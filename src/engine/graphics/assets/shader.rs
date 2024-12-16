use crate::debug_label;
use crate::engine::asset::{Asset, AssetKey, AssetLifecycler, AssetLoadRequest, AssetTypeId};
use crate::engine::graphics::Renderer;
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::default::Default;
use std::error::Error;
use std::sync::Arc;
use proc_macros_3l14::FancyEnum;
use wgpu::util::{make_spirv, make_spirv_raw};
use wgpu::{BufferAddress, FragmentState, MultisampleState, ShaderModule, ShaderModuleDescriptor, ShaderModuleDescriptorSpirV, VertexBufferLayout, VertexState};
use crate::engine::graphics::assets::MaterialLifecycler;
use crate::engine::graphics::debug_gui::DebugGui;

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

#[derive(Encode, Decode)]
pub struct ShaderFile
{
    pub stage: ShaderStage,
    pub module_bytes: Box<[u8]>,
    pub module_hash: u64,
}

pub struct Shader
{
    pub stage: ShaderStage,
    pub module: wgpu::ShaderModule,
    pub module_hash: u64, // likely duplicates asset key but oh well
}
impl Asset for Shader
{
    fn asset_type() -> AssetTypeId { AssetTypeId::Shader }
}

pub struct ShaderLifecycler
{
    renderer: Arc<Renderer>,
}
impl ShaderLifecycler
{
    const LOAD_SHADERS_DIRECT: bool = false;

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

        let module = match Self::LOAD_SHADERS_DIRECT && self.renderer.supports_feature(wgpu::Features::SPIRV_SHADER_PASSTHROUGH)
        {
            true => unsafe
            {
                self.renderer.device().create_shader_module_spirv(&ShaderModuleDescriptorSpirV
                {
                    label: debug_label!(&format!("{:?} ({:?})", request.asset_key, shader_file.stage)),
                    source: make_spirv_raw(&shader_file.module_bytes),
                })
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
    fn name(&self) -> &str { "Shaders" }
    fn debug_gui(&self, ui: &mut egui::Ui) { }
}