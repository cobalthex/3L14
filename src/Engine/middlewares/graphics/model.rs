use std::ops::Range;
use glam::{Mat4, Quat, Vec2, Vec3};
use gltf::mesh::util::ReadIndices;
use wgpu::{BufferSlice, BufferUsages, Device, IndexFormat, VertexBufferLayout};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use crate::engine::{AABB, AsU8Slice};
use crate::engine::middlewares::graphics::colors::Color;

pub trait WgpuVertex
{
    fn layout() -> VertexBufferLayout<'static>;
}

// todo: parametric vertex support
#[repr(packed)]
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct VertexPosNormTexCol
{
    pub position: Vec3,
    pub normal: Vec3,
    pub tex_coord: Vec2,
    pub color: Color,
}
impl WgpuVertex for VertexPosNormTexCol
{
    fn layout() -> VertexBufferLayout<'static>
    {
        VertexBufferLayout
        {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute
                {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute
                {
                    offset: std::mem::size_of::<Vec3>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute
                {
                    offset: (std::mem::size_of::<Vec3>() + std::mem::size_of::<Vec3>()) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute
                {
                    offset: (std::mem::size_of::<Vec3>() + std::mem::size_of::<Vec3>() + std::mem::size_of::<Vec2>()) as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}

#[derive(Debug)]
pub enum SceneImportError
{
    GltfError(gltf::Error),
    MissingVertexAttributes,
    MismatchedVertexAttributeLengths,
    MissingIndices,
}

pub struct Mesh
{
    bounds: AABB,
    vertices: wgpu::Buffer,
    vertex_count: u32,

    indices: wgpu::Buffer,
    index_count: u32,
    index_format: IndexFormat,
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

    pub fn new<Vertex, Index>(vertices: &[Vertex], indices: &[Index], device: &mut wgpu::Device) -> Self
    {
        let vbuffer = device.create_buffer_init(&BufferInitDescriptor
        {
            label: Some(format!("vertices").as_str()), // todo
            contents: unsafe { vertices.as_u8_slice() },
            usage: BufferUsages::VERTEX,
        });
        let ibuffer = device.create_buffer_init(&BufferInitDescriptor
        {
            label: Some(format!("indices").as_str()), // todo
            contents: unsafe { indices.as_u8_slice() },
            usage: BufferUsages::INDEX,
        });

        Self
        {
            bounds: Default::default(), // todo
            vertices: vbuffer,
            vertex_count: vertices.len() as u32,
            indices: ibuffer,
            index_count: indices.len() as u32,
            index_format: match std::mem::size_of::<Index>()
            {
                2 => IndexFormat::Uint16,
                4 => IndexFormat::Uint32,
                _ => panic!("Unsupported index format"),
            }
        }
    }
}

pub struct Model
{
    name: Option<String>, //debug only?
    bounds: AABB,
    meshes: Box<[Mesh]>,
}
impl Model
{
    pub fn meshes(&self) -> &[Mesh]
    {
        self.meshes.as_ref()
    }
}

#[derive(Default, Debug, Clone)]
pub struct Transform
{
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}
impl Transform
{
    pub fn as_matrix(&self) -> Mat4
    {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }
}

pub struct SceneNode<T>
{
    pub object: T,
    pub transform: Transform,
}

pub struct Scene
{
    pub models: Vec<SceneNode<Model>>,
}

impl Scene
{
    // todo: make async
    pub fn try_from_file(file: &str, device: &mut Device) -> Result<Self, SceneImportError>
    {
        // todo: vertex buffer/index buffer allocator

        let (document, buffers, _img) = gltf::import(file).map_err(SceneImportError::GltfError)?;

        let parse_mesh = |in_mesh: gltf::Mesh|
        {
            let mut model_bounds = AABB { min: Vec3::MAX, max: Vec3::MIN };
            let mut meshes: Vec<Mesh> = Vec::new();

            for in_prim in in_mesh.primitives()
            {
                let bb = in_prim.bounding_box();
                let mesh_bounds = AABB::new(bb.min.into(), bb.max.into());
                model_bounds.union_with(mesh_bounds);

                let mut vertices = Vec::new();

                let prim_reader = in_prim.reader(|b| Some(&buffers[b.index()]));
                let positions = prim_reader.read_positions().ok_or(SceneImportError::MissingVertexAttributes)?;
                let mut normals = prim_reader.read_normals().ok_or(SceneImportError::MissingVertexAttributes)?;
                let mut tex_coords = prim_reader.read_tex_coords(0).ok_or(SceneImportError::MissingVertexAttributes)?.into_f32();
                // let mut colors = reader.read_colors(0).ok_or(SceneImportError::MissingVertexAttributes)?.into_rgba_u8();

                for (i, p) in positions.enumerate()
                {
                    let n = normals.next().ok_or(SceneImportError::MismatchedVertexAttributeLengths)?;
                    let tc = tex_coords.next().ok_or(SceneImportError::MismatchedVertexAttributeLengths)?;
                    // let c = colors.next().unwrap_or_default();
                    let c = Color::from((i * 100) as u32); // random

                    vertices.push(VertexPosNormTexCol
                    {
                        position: p.into(),
                        normal: n.into(),
                        tex_coord: tc.into(),
                        color: c,
                    });
                }
                let vbuffer = device.create_buffer_init(&BufferInitDescriptor
                {
                    label: Some(format!("{file} vertices").as_str()),
                    contents: unsafe { vertices.as_u8_slice() },
                    usage: BufferUsages::VERTEX,
                });

                let indices = prim_reader.read_indices().ok_or(SceneImportError::MissingIndices)?;

                let ibuffer_label = format!("{file} indices");

                let index_fmt;
                let index_count;
                let ibuffer = match indices
                {
                    ReadIndices::U8(u8s) =>
                    {
                        let vec = u8s.map(|u| u as u16).collect::<Vec<u16>>();
                        index_fmt = IndexFormat::Uint16;
                        index_count = vec.len();
                        device.create_buffer_init(&BufferInitDescriptor
                        {
                            label: Some(ibuffer_label.as_str()),
                            contents: unsafe { vec.as_u8_slice() },
                            usage: BufferUsages::INDEX,
                        })
                    },
                    ReadIndices::U16(u16s) =>
                    {
                        let vec = u16s.collect::<Vec<u16>>();
                        index_fmt = IndexFormat::Uint16;
                        index_count = vec.len();
                        device.create_buffer_init(&BufferInitDescriptor
                        {
                            label: Some(ibuffer_label.as_str()),
                            contents: unsafe { vec.as_u8_slice() },
                            usage: BufferUsages::INDEX,
                        })
                    },
                    ReadIndices::U32(u32s) =>
                    {
                        let vec = u32s.collect::<Vec<u32>>();
                        index_fmt = IndexFormat::Uint32;
                        index_count = vec.len();
                        device.create_buffer_init(&BufferInitDescriptor
                        {
                            label: Some(ibuffer_label.as_str()),
                            contents: unsafe { vec.as_u8_slice() },
                            usage: BufferUsages::INDEX,
                        })
                    },
                };

                meshes.push(Mesh
                {
                    bounds: mesh_bounds,
                    vertices: vbuffer,
                    vertex_count: vertices.len() as u32,
                    indices: ibuffer,
                    index_count: index_count as u32,
                    index_format: index_fmt,
                });
            }

            Ok(Model
            {
                name: in_mesh.name().map(|s| s.to_string()) ,
                bounds: model_bounds,
                meshes: meshes.into_boxed_slice(),
            })
        };

        let mut models: Vec<SceneNode<Model>> = Vec::new();
        for node in document.nodes()
        {
            if let Some(in_mesh) = node.mesh()
            {
                let mesh = parse_mesh(in_mesh)?;
                let (position, rotation, scale) = node.transform().decomposed();
                models.push(SceneNode
                {
                    object: mesh,
                    transform: Transform
                    {
                        position: position.into(),
                        rotation: Quat::from_array(rotation),
                        scale: scale.into(),
                    },
                });
            }
        }

        Ok(Scene
        {
            models
        })
    }
}