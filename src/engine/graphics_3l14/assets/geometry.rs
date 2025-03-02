use std::ops::Range;
use crate::{debug_label, Renderer};
use asset_3l14::{AssetLifecycler, AssetLoadRequest};
use bitcode::{Decode, Encode};
use debug_3l14::debug_gui::DebugGui;
use math_3l14::{Sphere, AABB};
use proc_macros_3l14::Asset;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use egui::epaint::Vertex;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{Buffer, BufferSlice, BufferUsages, VertexAttribute, VertexBufferLayout, VertexStepMode};

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

// TODO: use structured buffers, possibly non-interleaved
// TODO: switch back to unified geo

#[derive(Encode, Decode)]
pub struct GeometryFile
{
    pub bounds_aabb: AABB,
    pub bounds_sphere: Sphere,
    pub vertex_layout: VertexLayout, // does it ever make sense for this to be per-mesh?
    pub vertices: Box<[u8]>,
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
    pub vertex_layout: VertexLayout, // does it ever make sense for this to be per-mesh?
    pub index_format: wgpu::IndexFormat,
    // all meshes in this model are slices of this buffer
    pub vertices: Buffer,
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
            bounds_aabb: mf.bounds_aabb,
            bounds_sphere: mf.bounds_sphere,
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
impl DebugGui for GeometryLifecycler
{
    fn name(&self) -> &str { "Geometry" }
    fn debug_gui(&self, _ui: &mut egui::Ui) { }
}