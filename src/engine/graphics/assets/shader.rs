use std::borrow::Cow;
use crate::engine::asset::{Asset, AssetLifecycler, AssetLoadRequest, AssetTypeId};
use crate::engine::graphics::Renderer;
use std::error::Error;
use std::io::Read;
use std::sync::Arc;
use bitcode::{Decode, Encode};
use proc_macros_3l14::EnumWithProps;
use serde::{Deserialize, Serialize};
use wgpu::{ShaderModule, ShaderModuleDescriptorSpirV};
use wgpu::ShaderModuleDescriptor;
use wgpu::util::{make_spirv, make_spirv_raw};

#[derive(Default, Serialize, Deserialize, Encode, Decode, EnumWithProps)]
pub enum ShaderStage
{
    #[default]
    // #[enum_prop(prefix = "vs")]
    Vertex,
    // #[enum_prop(prefix = "ps")]
    Pixel, // fragment
    // #[enum_prop(prefix = "cs")]
    Compute,
}

impl ShaderStage
{
    pub const fn prefix(&self) -> &'static str
    {
        match self
        {
            ShaderStage::Vertex => "vs",
            ShaderStage::Pixel => "ps",
            ShaderStage::Compute => "cs",
        }
    }
    pub fn entry_point(&self) -> &'static str
    {
        match self
        {
            ShaderStage::Vertex => "vs_main",
            ShaderStage::Pixel => "ps_main",
            ShaderStage::Compute => "cs_main",
        }
    }
}

#[derive(Encode, Decode)]
pub struct ShaderFile
{
    pub stage: ShaderStage,
}


pub struct Shader
{
    pub module: ShaderModule,
    pub stage: ShaderStage,
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
        const LOAD_DIRECT: bool = false; // wgpu requires Features::SPIRV_SHADER_PASSTHROUGH if true

        let shader_file: ShaderFile = request.deserialize()?;

        let mut module_bytes = Vec::new();
        request.input.read_to_end(&mut module_bytes)?;

        let module = match LOAD_DIRECT
        {
            true => unsafe 
            {
                self.renderer.device().create_shader_module_spirv(&ShaderModuleDescriptorSpirV
                {
                    label: Some(&format!("{:?}", request.asset_key)),
                    source: make_spirv_raw(&module_bytes),
                })
            },
            false =>
            {
                self.renderer.device().create_shader_module(ShaderModuleDescriptor
                {
                    label: Some(&format!("{:?}", request.asset_key)),
                    source: make_spirv(&module_bytes),
                })
            }
        };

        Ok(Shader
        {
            module,
            stage: shader_file.stage,
        })
    }
}
