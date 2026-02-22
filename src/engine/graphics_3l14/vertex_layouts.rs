use std::hash::{Hash, Hasher};
use bitcode::{Decode, Encode};
use enumflags2::{bitflags, BitFlags};
use serde::{Deserialize, Serialize};
use wgpu::{vertex_attr_array, BufferAddress, VertexAttribute, VertexBufferLayout, VertexFormat, VertexStepMode};

// TODO: generate HLSL structs automatically?

#[bitflags]
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VertexCaps
{
    Static  = 0b0001,
    Skinned = 0b0010,
    // instancing?
}
impl From<BitFlags<VertexCaps>> for VertexLayoutBuilder
{
    fn from(value: BitFlags<VertexCaps>) -> Self
    {
        let mut builder = VertexLayoutBuilder::default();
        for layout in value.iter()
        {
            match layout
            {
                VertexCaps::Static => StaticVertex::layout(&mut builder),
                VertexCaps::Skinned => SkinnedVertex::layout(&mut builder),
            }
        }
        builder
    }
}


// Note: this would be ideal to build statically
#[derive(Default, Hash)]
pub struct VertexLayoutBuilder
{
    attributes: Vec<VertexAttribute>,
    bytes: BufferAddress,
}
impl VertexLayoutBuilder
{
    #[inline]
    pub fn push(&mut self, attributes: &[VertexAttribute])
    {
        let len = self.attributes.len();
        self.attributes.extend(attributes.iter().map(|a| VertexAttribute
        {
            format: a.format,
            offset: { let b = self.bytes; self.bytes += a.format.size(); b },
            shader_location: len as u32 + a.shader_location
        }));
    }

    #[inline]
    pub fn as_vertex_buffer_layout(&self) -> VertexBufferLayout
    {
        VertexBufferLayout
        {
            array_stride: self.bytes,
            step_mode: VertexStepMode::Vertex,
            attributes: &self.attributes,
        }
    }
}

pub trait VertexDecl
{
    fn layout(layout_builder: &mut VertexLayoutBuilder);
}

// TODO: generate vertex layout via macro?

#[repr(C)]
pub struct StaticVertex
{
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coord: [f32; 2],
    // tangent, bitangent?
}
impl VertexDecl for StaticVertex
{
    fn layout(layout_builder: &mut VertexLayoutBuilder)
    {
        layout_builder.push(&vertex_attr_array!
        [
            0 => Float32x3, // position
            1 => Float32x3, // normal
            2 => Float32x2, // tex_coord
        ]);
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
    fn layout(layout_builder: &mut VertexLayoutBuilder)
    {
        layout_builder.push(&vertex_attr_array!
        [
            0 => Uint16x4, // indices
            1 => Float32x4, // weights
        ]);
    }
}
