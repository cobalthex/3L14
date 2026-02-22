use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use crate::Rgba;

#[repr(u8)]
#[derive(PartialEq, Eq, Copy, Clone, Debug, Hash, Serialize, Deserialize, Encode, Decode)]
pub enum MaterialClass
{
    DebugLines,
    PbrOpaque, // todo: split up?
    // PbrTransparent,
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
