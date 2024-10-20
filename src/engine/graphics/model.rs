use crate::debug_label;
use crate::engine::asset::{Asset, AssetHandle, AssetKey, AssetLifecycler, AssetLoadRequest, AssetTypeId};
use crate::engine::graphics::assets::material::Material;
use crate::engine::graphics::Renderer;
use crate::engine::{AsU8Slice, AABB};
use bitcode::{Decode, Encode};
use std::ops::Range;
use std::sync::Arc;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{BufferSlice, BufferUsages, IndexFormat, VertexBufferLayout};

// store in the material/etc?
#[derive(Encode, Decode)]
pub struct ModelFileMeshVertices
{
    pub stride: u32, // size of one vertex (between array elements)
    pub count: u32,
    pub layout: Box<[u8]>, // maps to wgpu::VertexAttribute
    pub data: Box<[u8]>,
}
impl ModelFileMeshVertices
{
    pub fn layout(&self) -> wgpu::VertexBufferLayout
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
pub enum ModelFileMeshIndices
{
    U16(Box<[u8]>),
    U32(Box<[u8]>),
}

#[derive(Encode, Decode)]
pub struct ModelFileMesh
{
    pub vertices: ModelFileMeshVertices,
    pub indices: ModelFileMeshIndices,
    pub bounds: AABB,
    pub material: AssetKey,
}

#[derive(Encode, Decode)]
pub struct ModelFile
{
    pub bounds: AABB,
    pub meshes: Box<[ModelFileMesh]>,
}
pub struct ModelMesh
{
    pub bounds: AABB, // note; these are untransformed
    pub vertices: wgpu::Buffer,
    pub vertex_count: u32,

    pub indices: wgpu::Buffer,
    pub index_count: u32,
    pub index_format: IndexFormat,

    pub material: AssetHandle<Material>,
}
impl ModelMesh
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

pub struct Model
{
    bounds: AABB, // note; these are untransformed
    meshes: Box<[ModelMesh]>,
}
impl Model
{
    pub fn meshes(&self) -> &[ModelMesh]
    {
        self.meshes.as_ref()
    }
}
impl Asset for Model
{
    fn asset_type() -> AssetTypeId { AssetTypeId::Model }

    fn all_dependencies_loaded(&self) -> bool
    {
        self.meshes.iter().all(|m|
        {
            m.material.is_loaded_recursive()
        })
    }
}

pub struct ModelLifecycler
{
    pub renderer: Arc<Renderer>,
}
impl ModelLifecycler
{
    pub fn new(renderer: Arc<Renderer>) -> Self
    {
        Self { renderer }
    }
}
impl AssetLifecycler for ModelLifecycler
{
    type Asset = Model;

    fn load(&self, mut request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn std::error::Error>>
    {
        let mf = request.deserialize::<ModelFile>()?;
        
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
                    ModelFileMeshIndices::U16(u16s) => { index_count = u16s.len() / 2; u16s }
                    ModelFileMeshIndices::U32(u32s) => { index_count = u32s.len() / 4; u32s }
                },
                usage: BufferUsages::INDEX,
            });

            ModelMesh
            {
                bounds: mesh.bounds,
                vertices: vbuffer,
                vertex_count: mesh.vertices.count,
                indices: ibuffer,
                index_count: index_count as u32,
                index_format: match mesh.indices
                {
                    ModelFileMeshIndices::U16(_) => IndexFormat::Uint16,
                    ModelFileMeshIndices::U32(_) => IndexFormat::Uint32,
                },
                material: request.load_dependency(mesh.material),
            }
        });

        Ok(Model
        {
            bounds: mf.bounds,
            meshes: meshes.collect(),
        })
    }
}
