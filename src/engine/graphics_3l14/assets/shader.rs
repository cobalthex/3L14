use std::borrow::Cow;
use crate::{debug_label, Renderer};
use bitcode::{Decode, Encode};
use proc_macros_3l14::{asset, FancyEnum};
use serde::{Deserialize, Serialize};
use std::default::Default;
use std::error::Error;
use enumflags2::BitFlags;
use triomphe::Arc;
use wgpu::util::make_spirv;
use wgpu::{ShaderModuleDescriptor, ShaderModuleDescriptorPassthrough};
use asset_3l14::{AssetKey, AssetKeySynthHash, AssetLifecycler, AssetLoadRequest, AssetTypeId};
use debug_3l14::debug_gui::DebugGui;
use crate::assets::VertexLayout;
use crate::material_classes::MaterialClass;

#[repr(u8)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Encode, Decode, FancyEnum)]
pub enum ShaderStage
{
    #[enum_prop(prefix = "vs", entry_point="vs_main")]
    Vertex = 0,
    #[enum_prop(prefix = "ps", entry_point="ps_main")]
    Pixel = 1, // fragment
    #[enum_prop(prefix = "cs", entry_point="cs_main")]
    Compute = 2,
    #[enum_prop(prefix = "ms", entry_point="ms_main")]
    Mesh = 3,
    // separate task shaders?
}

// move?
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum RenderPassName // todo: better name to not clash with wgpu::RenderPass
{
    Debug,
    // depth pre-pass?
    LightCull,
    Opaque,
    Transparent,
    // post-fx? fog, color grading, bloom, etc
    UI,
}

pub struct ShaderKey(u32);
impl ShaderKey
{
    #[inline] #[must_use]
    pub fn vertex(layout: BitFlags<VertexLayout>, pass: RenderPassName) -> Self
    {
        let mut val = 0b0001u32 << 28;
        val |= (pass as u32) << 20;
        val |= (layout.bits() as u32) << 12;
        Self(val)
    }

    #[inline] #[must_use]
    pub fn pixel(class: MaterialClass, pass: RenderPassName) -> Self
    {
        let mut val = 0b0010u32 << 28;
        val |= (pass as u32) << 20;
        val |= (class as u32) << 12;
        Self(val)
    }
    
    #[inline] #[must_use]
    pub fn to_assetkey(self) -> AssetKey
    {
        let synth = AssetKeySynthHash(self.0 as u64);
        AssetKey::synthetic(AssetTypeId::Shader, synth)
    }
}


#[derive(Encode, Decode, Debug)]
pub struct ShaderFile
{
    pub stage: ShaderStage,
    pub module_bytes: Box<[u8]>, // can this be a ref?
}

#[asset(debug_type = ShaderDebugData)]
pub struct Shader
{
    pub stage: ShaderStage,
    pub module: wgpu::ShaderModule,
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
            self.renderer.supports_feature(wgpu::Features::EXPERIMENTAL_PASSTHROUGH_SHADERS)
        {
            true => unsafe
            {
                assert!(shader_file.module_bytes.len().is_multiple_of(size_of::<u32>()));
                self.renderer.device().create_shader_module_passthrough(ShaderModuleDescriptorPassthrough
                {
                    label: debug_label!(&format!("{:?} ({:?})", request.asset_key, shader_file.stage)),
                    runtime_checks: Default::default(),
                    spirv: Some(Cow::Borrowed(std::mem::transmute(shader_file.module_bytes.as_ref()))),
                    .. Default::default()
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
        })
    }
}
impl DebugGui for ShaderLifecycler
{
    fn display_name(&self) -> &str { "Shaders" }
    fn debug_gui(&self, _ui: &mut egui::Ui) { }
}
