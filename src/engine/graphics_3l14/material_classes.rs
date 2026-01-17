use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use wgpu::{BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferBindingType, BufferSize, SamplerBindingType, ShaderStages, TextureSampleType, TextureViewDimension};
use crate::{debug_label, Rgba};

#[repr(u8)]
#[derive(PartialEq, Eq, Copy, Clone, Debug, Hash, Serialize, Deserialize, Encode, Decode)]
pub enum MaterialClass
{
    DebugLines,
    SimpleOpaque,
}
impl MaterialClass
{
}

#[repr(C)]
#[derive(Serialize, Deserialize)]
pub struct PbrProps
{
    pub albedo_color: Rgba,
    pub metallicity: f32,
    pub roughness: f32,
}
#[repr(C)]
pub struct SimpleOpaque
{
    pub pbr: PbrProps,
}
