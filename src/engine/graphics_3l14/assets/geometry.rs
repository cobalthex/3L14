use crate::{debug_label, Renderer};
use asset_3l14::{AssetLifecycler, AssetLoadRequest};
use bitcode::{Decode, Encode};
use debug_3l14::debug_gui::DebugGui;
use math_3l14::{Sphere, AABB};
use proc_macros_3l14::Asset;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{Buffer, BufferUsages, VertexAttribute, VertexBufferLayout, VertexStepMode};

#[repr(u8)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Encode, Decode, Hash)]
pub enum VertexLayout
{
    StaticSimple,
    DebugLines,
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
                    array_stride: V_ATTRS.iter().fold(0, |a, e| a + e.format.size()),
                    step_mode: VertexStepMode::Vertex,
                    attributes: &V_ATTRS,
                }
            },

            VertexLayout::DebugLines =>
            {
                const V_ATTRS: [VertexAttribute; 2] = wgpu::vertex_attr_array!
                [
                    0 => Float32x4, // clip-space position
                    1 => Uint32, // color 0
                ];
                VertexBufferLayout
                {
                    array_stride: V_ATTRS.iter().fold(0, |a, e| a + e.format.size()),
                    step_mode: VertexStepMode::Vertex,
                    attributes: &V_ATTRS,
                }
            }
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

// TODO: switch back to unified geo

#[derive(Encode, Decode)]
pub struct GeometryFileMesh
{
    pub bounds_aabb: AABB,
    pub bounds_sphere: Sphere,
    pub vertex_layout: VertexLayout,
    pub index_format: IndexFormat,
    pub vertex_count: u32,
    pub index_count: u32,
    pub vertices: Box<[u8]>,
    pub indices: Box<[u8]>,
}

// TODO: use structured buffers, possibly non-interleaved
// TODO: switch back to unified geo

#[derive(Encode, Decode)]
pub struct GeometryFile
{
    // note: it may be nice to split vertices into multiple buffers( p,n,t in one buffer, others in a second buffer)
    pub bounds_aabb: AABB,
    pub bounds_sphere: Sphere,
    pub meshes: Box<[GeometryFileMesh]>,
}

pub struct GeometryMesh
{
    pub bounds_aabb: AABB,
    pub bounds_sphere: Sphere,
    pub vertex_layout: VertexLayout,
    pub index_format: wgpu::IndexFormat,
    pub vertex_count: u32,
    pub index_count: u32,
    pub vertices: Buffer,
    pub indices: Buffer,
}

#[derive(Asset)]
pub struct Geometry
{
    pub bounds_aabb: AABB, // note; these are untransformed
    pub bounds_sphere: Sphere,
    pub meshes: Box<[GeometryMesh]>,
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
                bounds_aabb: mesh.bounds_aabb,
                bounds_sphere: mesh.bounds_sphere,
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
            bounds_aabb: mf.bounds_aabb,
            bounds_sphere: mf.bounds_sphere,
            meshes: meshes.collect(),
        })
    }
}
impl DebugGui for GeometryLifecycler
{
    fn name(&self) -> &str { "Geometry" }
    fn debug_gui(&self, _ui: &mut egui::Ui) { }
}