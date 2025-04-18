use std::hash::{Hash, Hasher};
use wgpu::{BufferAddress, VertexAttribute, VertexBufferLayout, VertexFormat, VertexStepMode};

// TODO: generate HLSL structs automatically?

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
    pub fn push(&mut self, attribute: VertexFormat)
    {
        self.attributes.push(VertexAttribute
        {
            format: attribute,
            offset: self.bytes,
            shader_location: self.attributes.len() as u32,
        });
        self.bytes += attribute.size();
    }

    // reserve 'additional' more slots
    #[inline]
    pub fn reserve(&mut self, additional: usize)
    {
        self.attributes.reserve(additional);
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
    pub color: [u8; 4],
    // tangent, bitangent?
}
impl VertexDecl for StaticVertex
{
    fn layout(layout_builder: &mut VertexLayoutBuilder)
    {
        layout_builder.reserve(4);
        layout_builder.push(VertexFormat::Float32x3);
        layout_builder.push(VertexFormat::Float32x3);
        layout_builder.push(VertexFormat::Float32x2);
        layout_builder.push(VertexFormat::Uint32);
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
        layout_builder.reserve(2);
        layout_builder.push(VertexFormat::Uint16x4);
        layout_builder.push(VertexFormat::Float32x4);
    }
}
