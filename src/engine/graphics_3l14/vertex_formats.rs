use std::hash::{Hash, Hasher};
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use wgpu::{vertex_attr_array, BufferAddress, VertexAttribute, VertexBufferLayout, VertexStepMode};

// TODO: generate HLSL structs automatically?

#[derive(PartialEq, Eq, Copy, Clone, Debug, Hash, Serialize, Deserialize, Encode, Decode)]
pub enum VertexFormat
{
    Static,
    Skinned,
}
impl From<VertexFormat> for wgpu::VertexBufferLayout<'static>
{
    fn from(value: VertexFormat) -> Self
    {
        match value
        {
            VertexFormat::Static => StaticVertex::layout(),
            VertexFormat::Skinned => SkinnedVertex::layout(),
        }
    }
}

// TODO: generate vertex layout via macro?

trait VertexAttrs
{
    const STEP_MODE: VertexStepMode = VertexStepMode::Vertex;
    fn attrs() -> &'static [VertexAttribute];
}
trait VertexLayout
{
    fn layout() -> VertexBufferLayout<'static>;
}
impl<V: VertexAttrs> VertexLayout for V
{
    fn layout() -> VertexBufferLayout<'static>
    {
        VertexBufferLayout
        {
            array_stride: size_of::<V>() as u64,
            step_mode: V::STEP_MODE,
            attributes: &V::attrs(),
        }
    }
}

#[repr(C)]
pub struct StaticVertex
{
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coord: [f32; 2],
    // tangent, bitangent?
}
impl VertexAttrs for StaticVertex
{
    fn attrs() -> &'static [VertexAttribute]
    {
        const LAYOUT: [VertexAttribute; 3] = vertex_attr_array!
        [
            0 => Float32x3, // position
            1 => Float32x3, // normal
            2 => Float32x2, // tex_coord
        ];
        &LAYOUT
    }
}

#[repr(C)]
pub struct SkinnedVertex
{
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coord: [f32; 2],

    pub indices: [u16; 4], // index of a bone/joint in the linked skeleton influenced by
    pub weights: [f32; 4], // how much influence said bone has (0-1)
}
impl VertexAttrs for SkinnedVertex
{
    fn attrs() -> &'static [VertexAttribute]
    {
        const LAYOUT: [VertexAttribute; 5] = vertex_attr_array!
        [
            0 => Float32x3, // position
            1 => Float32x3, // normal
            2 => Float32x2, // tex_coord

            3 => Uint16x4, // bone indices
            4 => Float32x4, // bone weights
        ];
        &LAYOUT
    }
}
