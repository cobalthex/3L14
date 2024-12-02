use proc_macros_3l14::Flags;
use wgpu::CommandEncoder;

pub mod pbr_opaque;

#[repr(u32)]
#[derive(Flags, Clone, Copy)]
pub enum RenderPassFlags
{
    None = 0b000,
    ContributesToLighting = 0b001,
    IsTranslucent = 0b010,
}
