use crate::debug_label;
use crate::engine::asset::{Asset, AssetHandle, AssetKey, AssetLifecycler, AssetLoadRequest, AssetTypeId};
use crate::engine::graphics::Renderer;
use crate::engine::{AsU8Slice, AABB};
use bitcode::{Decode, Encode};
use std::ops::Range;
use std::sync::Arc;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{BufferSlice, BufferUsages, IndexFormat, VertexBufferLayout};

#[derive(Encode, Decode)]
pub struct GeometryFileMeshVertices
{
    pub stride: u32, // size of one vertex (between array elements)
    pub count: u32,
    pub layout: Box<[u8]>, // maps to wgpu::VertexAttribute -- TODO: well defined layout
    pub data: Box<[u8]>,
}
impl GeometryFileMeshVertices
{
    pub fn layout(&self) -> VertexBufferLayout
    {
        VertexBufferLayout
        {
            array_stride: self.stride as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: unsafe { std::mem::transmute(self.layout.as_ref()) },
        }
    }
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
    bounds: AABB, // note; these are untransformed
    meshes: Box<[GeometryMesh]>,
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
            meshes: meshes.collect(),
        })
    }
}
