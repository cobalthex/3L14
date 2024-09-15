use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::ops::Range;
use wgpu::{vertex_attr_array, BufferSlice, IndexFormat, VertexBufferLayout};

use crate::engine::assets::{Asset, AssetLifecycler, AssetLoadRequest, AssetPayload, AssetTypeId, HasAssetDependencies};
use crate::engine::graphics::material::Material;
use crate::engine::AABB;
use crate::engine::graphics::Renderer;
use super::colors::Rgba;

pub trait WgpuVertexDecl
{
    fn layout() -> VertexBufferLayout<'static>;
}

// todo: parametric vertex support
#[repr(packed)]
#[allow(dead_code)]
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct VertexPosNormTexCol
{
    pub position: Vec3,
    pub normal: Vec3,
    pub tex_coord: Vec2,
    pub color: Rgba,
}
impl VertexPosNormTexCol
{
    const LAYOUT: VertexBufferLayout<'static> = VertexBufferLayout
    {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &vertex_attr_array!
        [
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x2,
            3 => Uint32,
        ],
    };
}
impl WgpuVertexDecl for VertexPosNormTexCol
{
    fn layout() -> VertexBufferLayout<'static> { Self::LAYOUT }
}

pub struct Mesh
{
    bounds: AABB, // note; these are untransformed
    vertices: wgpu::Buffer,
    vertex_count: u32,

    indices: wgpu::Buffer,
    index_count: u32,
    index_format: IndexFormat,

    // todo: materials
    material: Material,
}
impl Mesh
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
    pub fn index_format(&self) -> IndexFormat { self.index_format }
    
    pub fn material(&self) -> &Material { &self.material }
}

#[derive(Serialize, Deserialize)]
pub enum ModelFileMeshIndices
{
    U16(Box<[u16]>),
    U32(Box<[u32]>),
}

#[derive(Serialize, Deserialize)]
pub struct ModelFileMesh
{
    pub vertices: Box<[VertexPosNormTexCol]>,
    pub indices: ModelFileMeshIndices,
    pub bounds: AABB,
    // TODO: materials
}

#[derive(Serialize, Deserialize)]
pub struct ModelFile
{
    pub meshes: Box<[ModelFileMesh]>,
}

pub struct Model
{
    name: Option<String>, //debug only?
    bounds: AABB, // note; these are untransformed
    meshes: Box<[Mesh]>,
}
impl Model
{
    pub fn meshes(&self) -> &[Mesh]
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
            m.material.asset_dependencies_loaded()
        })
    }
}

pub struct ModelLifecycler
{
    pub renderer: Renderer,
}
impl AssetLifecycler for ModelLifecycler
{
    type Asset = Model;

    fn load(&self, request: AssetLoadRequest) -> AssetPayload<Self::Asset>
    {
        

        todo!()
    }
}