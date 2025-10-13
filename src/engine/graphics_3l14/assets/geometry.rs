use crate::{debug_label, Renderer};
use asset_3l14::{AssetLifecycler, AssetLoadRequest};
use bitcode::{Decode, Encode};
use debug_3l14::debug_gui::DebugGui;
use math_3l14::{Sphere, AABB};
use proc_macros_3l14::{asset, Flags};
use triomphe::Arc;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{Buffer, BufferUsages};
use nab_3l14::FlagsEnum;
use crate::vertex_layouts::{SkinnedVertex, StaticVertex, VertexDecl, VertexLayoutBuilder};

// The vertex buffers used for a particular piece of geometry.
// vertex buffer attrib locations are ordered based on attributes present
// Buffers with lower VertexLqyout values are ordered first, numbers are sequential
// TODO: possibly could specify fixed locations, but often limited to 32 locations on GPU
#[derive(Flags)]
#[repr(u16)]
pub enum VertexLayout
{
    Static      = 0b00000001,
    Skinned     = 0b00000010,
}
impl From<VertexLayout> for VertexLayoutBuilder
{
    fn from(value: VertexLayout) -> Self
    {
        let mut builder = VertexLayoutBuilder::default();
        for layout in value.iter_set_flags()
        {
            match layout
            {
                VertexLayout::Static => StaticVertex::layout(&mut builder),
                VertexLayout::Skinned => SkinnedVertex::layout(&mut builder),
            }
        }
        builder
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

// TODO: maybe use structured buffers, possibly non-interleaved

#[derive(Encode, Decode)]
pub struct GeometryFile
{
    pub bounds_aabb: AABB,
    pub bounds_sphere: Sphere,
    pub vertex_layout: <VertexLayout as FlagsEnum<VertexLayout>>::Repr, // must store as underlying type due to limitation of bitcode
    pub index_format: IndexFormat,
    pub vertices: Box<[u8]>, // Contains composite vertices of type vertex_layout
    pub indices: Box<[u8]>, // contains indices of type index_format
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

#[asset]
pub struct Geometry
{
    pub bounds_aabb: AABB, // note; these are untransformed
    pub bounds_sphere: Sphere,
    pub vertex_layout: VertexLayout,
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
        let gf = request.deserialize::<GeometryFile>()?;
        
        let vertices = self.renderer.device().create_buffer_init(&BufferInitDescriptor
        {
            label: debug_label!(format!("{:?} static vertices", request.asset_key).as_str()),
            contents: gf.vertices.as_ref(),
            usage: BufferUsages::VERTEX,
        });
        let indices = self.renderer.device().create_buffer_init(&BufferInitDescriptor
        {
            label: debug_label!(format!("{:?} indices", request.asset_key).as_str()),
            contents: gf.indices.as_ref(),
            usage: BufferUsages::INDEX,
        });

        Ok(Geometry
        {
            bounds_aabb: gf.bounds_aabb,
            bounds_sphere: gf.bounds_sphere,
            vertex_layout: gf.vertex_layout.into(),
            index_format: match gf.index_format
            {
                IndexFormat::U16 => wgpu::IndexFormat::Uint16,
                IndexFormat::U32 => wgpu::IndexFormat::Uint32,
            },
            vertices,
            indices,
            meshes: gf.meshes,
        })
    }
}
impl DebugGui for GeometryLifecycler
{
    fn display_name(&self) -> &str { "Geometry" }
    fn debug_gui(&self, _ui: &mut egui::Ui) { }
}