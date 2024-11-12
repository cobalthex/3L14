use crate::debug_label;
use crate::engine::asset::{Asset, AssetHandle, AssetKey, AssetLifecycler, AssetLoadRequest, AssetTypeId};
use crate::engine::graphics::Renderer;
use crate::engine::{AsU8Slice, AABB};
use bitcode::{Decode, Encode};
use serde::{Serialize, Deserialize};
use std::ops::Range;
use std::sync::Arc;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{Buffer, BufferSlice, BufferUsages, VertexAttribute, VertexBufferLayout, VertexStepMode};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Encode, Decode, Hash)]
pub enum VertexLayout
{
    StaticSimple,
}
impl From<VertexLayout> for wgpu::VertexBufferLayout<'static>
{
    fn from(value: VertexLayout) -> Self
    {
        match value
        {
            VertexLayout::StaticSimple =>
            {
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
                    array_stride: V_ATTRS.iter().fold(0, |a, e| a + e.format.size()), // passed in from file?
                    step_mode: VertexStepMode::Vertex,
                    attributes: &V_ATTRS,
                }
            },
        }
    }
}

#[derive(Encode, Decode)]
pub enum IndexFormat
{
    U16,
    U32,
}

#[derive(Encode, Decode)]
pub struct GeometryFile
{
    // note: it may be nice to split vertices into multiple buffers( p,n,t in one buffer, others in a second buffer)
    pub bounds: AABB,
    pub vertex_layout: VertexLayout, // does it ever make sense for this to be per-mesh?
    pub vertices: Box<[u8]>,
    pub index_format: IndexFormat,
    pub indices: Box<[u8]>,
    pub meshes: Box<[GeometryMesh]>,
}

#[derive(Encode, Decode)]
pub struct GeometryMesh
{
    pub bounds: AABB, // note; these are untransformed
    pub vertex_range: (u32, u32), // start, end
    pub index_range: (u32, u32), // start, end
}

pub struct GeometrySlice<'g>
{
    pub vertex_slice: BufferSlice<'g>,
    pub vertex_range: Range<u32>,
    pub index_slice: BufferSlice<'g>,
    pub index_range: Range<u32>,
}

pub struct Geometry
{
    pub bounds: AABB, // note; these are untransformed
    pub vertex_layout: VertexLayout, // does it ever make sense for this to be per-mesh?
    pub index_format: wgpu::IndexFormat,
    // all meshes in this model are slices of this buffer
    pub vertices: Buffer,
    pub indices: Buffer,
    pub meshes: Box<[GeometryMesh]>,
}
impl Geometry
{
    pub fn mesh(&self, index: u32) -> GeometrySlice
    {
        assert!(index < self.meshes.len() as u32);

        let mesh = &self.meshes[index as usize];
        GeometrySlice
        {
            vertex_slice: self.vertices.slice(mesh.vertex_range.0 as u64 .. mesh.index_range.1 as u64),
            vertex_range: mesh.vertex_range.0 .. mesh.vertex_range.1,
            index_slice: self.indices.slice(mesh.index_range.0 as u64 .. mesh.index_range.1 as u64),
            index_range: mesh.index_range.0 .. mesh.index_range.1,
        }
    }
}
impl Asset for Geometry
{
    fn asset_type() -> AssetTypeId { AssetTypeId::Geometry }
}

pub struct GeometryLifecycler
{
    pub renderer: Arc<Renderer>,
}
impl GeometryLifecycler
{
    pub fn new(renderer: Arc<Renderer>) -> Self
    {
        Self { renderer }
    }
}
impl AssetLifecycler for GeometryLifecycler
{
    type Asset = Geometry;

    fn load(&self, mut request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn std::error::Error>>
    {
        let mf = request.deserialize::<GeometryFile>()?;

        let vbuffer = self.renderer.device().create_buffer_init(&BufferInitDescriptor
        {
            label: debug_label!(format!("{:?} vertices", request.asset_key).as_str()),
            contents: mf.vertices.as_ref(),
            usage: BufferUsages::VERTEX,
        });
        let ibuffer = self.renderer.device().create_buffer_init(&BufferInitDescriptor
        {
            label: debug_label!(format!("{:?} indices", request.asset_key).as_str()),
            contents: mf.indices.as_ref(),
            usage: BufferUsages::INDEX,
        });

        Ok(Geometry
        {
            bounds: mf.bounds,
            vertex_layout: mf.vertex_layout,
            index_format: match mf.index_format
            {
                IndexFormat::U16 => wgpu::IndexFormat::Uint16,
                IndexFormat::U32 => wgpu::IndexFormat::Uint32,
            },
            vertices: vbuffer,
            indices: ibuffer,
            meshes: mf.meshes,
        })
    }
}
