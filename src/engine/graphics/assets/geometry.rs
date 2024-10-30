use crate::debug_label;
use crate::engine::asset::{Asset, AssetHandle, AssetKey, AssetLifecycler, AssetLoadRequest, AssetTypeId};
use crate::engine::graphics::Renderer;
use crate::engine::{AsU8Slice, AABB};
use bitcode::{Decode, Encode};
use serde::{Serialize, Deserialize};
use std::ops::Range;
use std::sync::Arc;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{BufferSlice, BufferUsages, IndexFormat, VertexAttribute, VertexBufferLayout, VertexStepMode};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Encode, Decode)]
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
pub struct GeometryFileMeshVertices
{
    pub count: u32,
    pub data: Box<[u8]>,
}

#[derive(Encode, Decode)]
pub enum GeometryFileMeshIndices
{
    U16(Box<[u8]>),
    U32(Box<[u8]>),
}

#[derive(Encode, Decode)]
pub struct GeometryFileMesh
{
    pub vertices: GeometryFileMeshVertices,
    pub indices: GeometryFileMeshIndices,
    pub bounds: AABB,
}

#[derive(Encode, Decode)]
pub struct GeometryFile
{
    pub bounds: AABB,
    pub vertex_layout: VertexLayout, // does it ever make sense for this to be per-mesh?
    pub meshes: Box<[GeometryFileMesh]>,
}
pub struct GeometryMesh
{
    pub bounds: AABB, // note; these are untransformed
    pub vertices: wgpu::Buffer,
    pub vertex_count: u32,

    pub indices: wgpu::Buffer,
    pub index_count: u32,
    pub index_format: IndexFormat,
}
impl GeometryMesh
{
    pub fn vertices(&self) -> BufferSlice
    {
        self.vertices.slice(..)
    }
    pub fn vertex_range(&self) -> Range<u32>
    {
        0 .. self.vertex_count
    }

    pub fn indices(&self) -> BufferSlice
    {
        self.indices.slice(..)
    }
    pub fn index_range(&self) -> Range<u32>
    {
        0 .. self.index_count
    }

    // pub fn material(&self) -> &Material { &self.material }
}

pub struct Geometry
{
    pub bounds: AABB, // note; these are untransformed
    pub vertex_layout: VertexLayout, // does it ever make sense for this to be per-mesh?
    pub meshes: Box<[GeometryMesh]>,
}
impl Geometry
{
    pub fn meshes(&self) -> &[GeometryMesh]
    {
        self.meshes.as_ref()
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

        // combine buffers?
        let meshes =
            (&mf.meshes).iter().map(|mesh|
                {
                    let vbuffer = self.renderer.device().create_buffer_init(&BufferInitDescriptor
                    {
                        label: debug_label!(format!("{:?} vertices", request.asset_key).as_str()),
                        contents: mesh.vertices.data.as_ref(),
                        usage: BufferUsages::VERTEX,
                    });

                    let index_count;
                    let ibuffer = self.renderer.device().create_buffer_init(&BufferInitDescriptor
                    {
                        label: debug_label!(format!("{:?} indices", request.asset_key).as_str()),
                        contents: match &mesh.indices
                        {
                            GeometryFileMeshIndices::U16(u16s) => { index_count = u16s.len() / 2; u16s }
                            GeometryFileMeshIndices::U32(u32s) => { index_count = u32s.len() / 4; u32s }
                        },
                        usage: BufferUsages::INDEX,
                    });

                    GeometryMesh
                    {
                        bounds: mesh.bounds,
                        vertices: vbuffer,
                        vertex_count: mesh.vertices.count,
                        indices: ibuffer,
                        index_count: index_count as u32,
                        index_format: match mesh.indices
                        {
                            GeometryFileMeshIndices::U16(_) => IndexFormat::Uint16,
                            GeometryFileMeshIndices::U32(_) => IndexFormat::Uint32,
                        },
                    }
                });

        Ok(Geometry
        {
            bounds: mf.bounds,
            vertex_layout: mf.vertex_layout,
            meshes: meshes.collect(),
        })
    }
}
