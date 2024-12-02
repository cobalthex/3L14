use crate::debug_label;
use crate::engine::asset::{Asset, AssetLifecycler, AssetLoadRequest, AssetTypeId};
use crate::engine::graphics::debug_gui::DebugGui;
use crate::engine::graphics::Renderer;
use crate::engine::AABB;
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{Buffer, BufferUsages, VertexAttribute, VertexBufferLayout, VertexStepMode};

#[repr(u8)]
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

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub enum IndexFormat
{
    U16,
    U32,
}
impl From<IndexFormat> for wgpu::IndexFormat
{
    fn from(value: IndexFormat) -> Self
    {
        match value
        {
            IndexFormat::U16 => wgpu::IndexFormat::Uint16,
            IndexFormat::U32 => wgpu::IndexFormat::Uint32,
        }
    }
}

#[derive(Encode, Decode)]
pub struct GeometryFileMesh
{
    pub bounds: AABB, // note; these are untransformed
    pub vertex_layout: VertexLayout,
    pub index_format: IndexFormat,
    pub vertex_count: u32,
    pub index_count: u32,
    pub vertices: Box<[u8]>,
    pub indices: Box<[u8]>,
}

#[derive(Encode, Decode)]
pub struct GeometryFile
{
    // note: it may be nice to split vertices into multiple buffers( p,n,t in one buffer, others in a second buffer)
    pub bounds: AABB,
    pub meshes: Box<[GeometryFileMesh]>,
}

pub struct GeometryMesh
{
    pub bounds: AABB,
    pub vertex_layout: VertexLayout,
    pub index_format: wgpu::IndexFormat,
    pub vertex_count: u32,
    pub index_count: u32,
    pub vertices: Buffer,
    pub indices: Buffer,
}

pub struct Geometry
{
    pub bounds: AABB, // note; these are untransformed
    pub meshes: Box<[GeometryMesh]>,
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
        let meshes = (&mf.meshes).iter().map(|mesh|
        {
            let vbuffer = self.renderer.device().create_buffer_init(&BufferInitDescriptor
            {
                label: debug_label!(format!("{:?} vertices", request.asset_key).as_str()),
                contents: &mesh.vertices,
                usage: BufferUsages::VERTEX,
            });

            let ibuffer = self.renderer.device().create_buffer_init(&BufferInitDescriptor
            {
                label: debug_label!(format!("{:?} indices", request.asset_key).as_str()),
                contents: &mesh.indices,
                usage: BufferUsages::INDEX,
            });
            GeometryMesh
            {
                bounds: mesh.bounds,
                vertex_layout: mesh.vertex_layout,
                index_format: mesh.index_format.into(),
                vertex_count: mesh.vertex_count,
                index_count: mesh.index_count,
                vertices: vbuffer,
                indices: ibuffer,
            }
        });

        Ok(Geometry
        {
            bounds: mf.bounds,
            meshes: meshes.collect(),
        })
    }
}
impl DebugGui for GeometryLifecycler
{
    fn name(&self) -> &str { "Geometry" }
    fn debug_gui(&self, ui: &mut egui::Ui) { }
}