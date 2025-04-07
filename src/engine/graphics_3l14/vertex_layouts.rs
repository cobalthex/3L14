use std::hash::{Hash, Hasher};
use metrohash::MetroHash64;
use wgpu::{VertexAttribute, VertexBufferLayout, VertexStepMode};
use proc_macros_3l14::Flags;

pub trait VertexDecl
{
    fn layout(base_offset: u32) -> wgpu::VertexBufferLayout<'static>;
    fn layout_hash(hasher: &mut impl Hasher)
    {
        let layout = Self::layout(0);
        layout.hash(hasher);
    }
}

#[repr(C)]
pub struct StaticVertex
{
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coord: [f32; 2],
    pub color: [u8; 4],
    // tangent, bitangent?
}
impl VertexDecl for StaticVertex
{
    fn layout(base_offset: u32) -> VertexBufferLayout<'static>
    {
        // TODO: this needs to account for base offset
        const V_ATTRS: [VertexAttribute; 4] = wgpu::vertex_attr_array!
        [
            0 => Float32x3, // position
            1 => Float32x3, // normal
            // tangent?
            2 => Float32x2, // texcoord 0
            3 => Uint32, // color 0
        ];
        VertexBufferLayout
        {
            array_stride: V_ATTRS.iter().fold(0, |a, e| a + e.format.size()),
            step_mode: VertexStepMode::Vertex,
            attributes: &V_ATTRS,
        }
    }
}

#[repr(C)]
pub struct SkinnedVertex
{
    pub indices: [u16; 4], // index of a bone/joint in the linked skeleton influenced by
    pub weights: [f32; 4], // how much influence said bone has (0-1)
}
impl VertexDecl for SkinnedVertex
{
    fn layout() -> VertexBufferLayout<'static>
    {
        const V_ATTRS: [VertexAttribute; 2] = wgpu::vertex_attr_array!
        [
            0 => Uint16x2, // bone/joint indices
            1 => Float32x4, // weights
        ];
        VertexBufferLayout
        {
            array_stride: V_ATTRS.iter().fold(0, |a, e| a + e.format.size()),
            step_mode: VertexStepMode::Vertex,
            attributes: &V_ATTRS,
        }
    }
}
