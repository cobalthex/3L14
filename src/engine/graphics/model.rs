use std::io::Read;
use glam::{Vec2, Vec3};
use std::ops::Range;
use std::sync::Arc;
use bitcode::{Decode, Encode};
use wgpu::{vertex_attr_array, BufferSlice, BufferUsages, IndexFormat, VertexBufferLayout};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use crate::engine::assets::{Asset, AssetLifecycler, AssetLoadError, AssetLoadRequest, AssetPayload, AssetTypeId, HasAssetDependencies};
use crate::engine::graphics::material::Material;
use crate::engine::{AsU8Slice, AABB};
use crate::engine::assets::AssetLoadError::ParseError;
use crate::engine::graphics::Renderer;
use super::colors::Rgba;

pub trait WgpuVertexDecl
{
    fn layout() -> VertexBufferLayout<'static>;
}

// todo: parametric vertex support
#[repr(C)]
#[allow(dead_code)]
#[derive(Debug, Default, Encode, Decode)]
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

pub struct ModelMesh
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
    pub fn index_format(&self) -> IndexFormat { self.index_format }

    pub fn material(&self) -> &Material { &self.material }
}

#[derive(Encode, Decode)]
pub enum ModelFileMeshIndices
{
    U16(Box<[u16]>),
    U32(Box<[u32]>),
}

#[derive(Encode, Decode)]
pub struct ModelFileMesh
{
    pub vertices: Box<[VertexPosNormTexCol]>,
    pub indices: ModelFileMeshIndices,
    pub bounds: AABB,
    // TODO: materials
}

#[derive(Encode, Decode)]
pub struct ModelFile
{
    pub bounds: AABB,
    pub meshes: Box<[ModelFileMesh]>,
}

pub struct Model
{
    name: Option<String>, //debug only?
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
            m.material.all_dependencies_loaded()
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

    fn load(&self, mut request: AssetLoadRequest) -> AssetPayload<Self::Asset>
    {
        match request.deserialize::<ModelFile>()
        {
            Ok(mf) =>
            {
                // combine buffers?
                let meshes =
                mf.meshes.iter().map(|mesh|
                {
                    let vbuffer = self.renderer.device().create_buffer_init(&BufferInitDescriptor
                    {
                        label: Some(format!("{:?} vertices", request.asset_key).as_str()),
                        contents: unsafe { mesh.vertices.as_u8_slice() },
                        usage: BufferUsages::VERTEX,
                    });

                    let index_count;
                    let ibuffer = self.renderer.device().create_buffer_init(&BufferInitDescriptor
                    {
                        label: Some(format!("{:?} indices", request.asset_key).as_str()),
                        contents: match &mesh.indices
                        {
                            ModelFileMeshIndices::U16(u16s) => unsafe { index_count = u16s.len(); u16s.as_u8_slice() }
                            ModelFileMeshIndices::U32(u32s) => unsafe { index_count = u32s.len(); u32s.as_u8_slice() }
                        },
                        usage: BufferUsages::INDEX,
                    });

                    ModelMesh
                    {
                        bounds: mesh.bounds,
                        vertices: vbuffer,
                        vertex_count: mesh.vertices.len() as u32,
                        indices: ibuffer,
                        index_count: index_count as u32,
                        index_format: match mesh.indices
                        {
                            ModelFileMeshIndices::U16(_) => IndexFormat::Uint16,
                            ModelFileMeshIndices::U32(_) => IndexFormat::Uint32,
                        },
                        material: Material
                        {
                            albedo_map: Some(request.load_dependency(0x00400000d8355d3edc9b042bc8f71a39u128.into())),
                            .. Default::default()
                        },
                    }
                });

                let model = Model
                {
                    bounds: mf.bounds,
                    name: Some(format!("{:?}", request.asset_key)), // debug name?
                    meshes: meshes.collect(),
                };

                AssetPayload::Available(model)
            },
            Err(err) =>
            {
                eprintln!("Error parsing model file: {err}");
                AssetPayload::Unavailable(ParseError(1))
            }
        }
    }
}
