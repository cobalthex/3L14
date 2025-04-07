use crate::{debug_label, Renderer};
use asset_3l14::{AssetLifecycler, AssetLoadRequest};
use bitcode::{Decode, Encode};
use debug_3l14::debug_gui::DebugGui;
use math_3l14::{Sphere, AABB};
use proc_macros_3l14::{Asset, Flags};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{Buffer, BufferUsages, VertexAttribute, VertexBufferLayout, VertexStepMode};

#[derive(Flags)]
#[repr(u16)]
pub enum VertexLayout
{
    Static      = 0b00000001,
    Skinned     = 0b00000010,
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

// TODO: use structured buffers, possibly non-interleaved
// TODO: switch back to unified geo

#[derive(Encode, Decode)]
pub struct GeometryFile
{
    pub bounds_aabb: AABB,
    pub bounds_sphere: Sphere,
    pub static_vertices: Box<[u8]>,
    pub skinned_vertices: Option<Box<[u8]>>,
    pub index_format: IndexFormat,
    pub indices: Box<[u8]>,
    pub meshes: Box<[GeometryMesh]>,
}

#[derive(Encode, Decode)]
pub struct GeometryMesh
{
    pub bounds_aabb: AABB, // note; these are untransformed
    pub bounds_sphere: Sphere,
    pub vertex_range: (u32, u32), // start, end
    pub index_range: (u32, u32), // start, end
}

#[derive(Asset)]
pub struct Geometry
{
    pub bounds_aabb: AABB, // note; these are untransformed
    pub bounds_sphere: Sphere,
    pub vertex_layout: VertexLayout,
    pub index_format: wgpu::IndexFormat,
    // all meshes in this model are slices of this buffer
    pub static_vertices: Buffer,
    pub skinned_vertices: Option<Buffer>,
    pub indices: Buffer,
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

        let mut vertex_layout = VertexLayout::Static;

        let static_verts = self.renderer.device().create_buffer_init(&BufferInitDescriptor
        {
            label: debug_label!(format!("{:?} static vertices", request.asset_key).as_str()),
            contents: mf.static_vertices.as_ref(),
            usage: BufferUsages::VERTEX,
        });
        let skinned_verts = if let Some(skinned_vertices) = mf.skinned_vertices.as_ref()
        {
            vertex_layout |= VertexLayout::Skinned;
            Some(self.renderer.device().create_buffer_init(&BufferInitDescriptor
            {
                label: debug_label!(format!("{:?} skinned vertices", request.asset_key).as_str()),
                contents: skinned_vertices,
                usage: BufferUsages::VERTEX,
            }))
        } else { None };
        let indices = self.renderer.device().create_buffer_init(&BufferInitDescriptor
        {
            label: debug_label!(format!("{:?} indices", request.asset_key).as_str()),
            contents: mf.indices.as_ref(),
            usage: BufferUsages::INDEX,
        });

        Ok(Geometry
        {
            bounds_aabb: mf.bounds_aabb,
            bounds_sphere: mf.bounds_sphere,
            vertex_layout,
            index_format: match mf.index_format
            {
                IndexFormat::U16 => wgpu::IndexFormat::Uint16,
                IndexFormat::U32 => wgpu::IndexFormat::Uint32,
            },
            static_vertices: static_verts,
            skinned_vertices: skinned_verts,
            indices,
            meshes: mf.meshes,
        })
    }
}
impl DebugGui for GeometryLifecycler
{
    fn name(&self) -> &str { "Geometry" }
    fn debug_gui(&self, _ui: &mut egui::Ui) { }
}