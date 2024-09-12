use std::io::SeekFrom;
use std::ops::Range;
use std::sync::Arc;
use glam::{Quat, Vec2, Vec3};
use gltf::mesh::util::ReadIndices;
use wgpu::{BufferSlice, BufferUsages, IndexFormat, vertex_attr_array, VertexBufferLayout};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use crate::engine::{AABB, AsU8Slice};
use crate::engine::assets::{Asset, AssetHandle, AssetLifecycler, AssetLoadError, AssetLoadRequest, AssetPayload, AssetTypeId, HasAssetDependencies};
use crate::engine::graphics::assets::texture::Texture;
use crate::engine::graphics::material::Material;
use crate::engine::graphics::Renderer;
use crate::engine::world::Transform;

use super::colors::Rgba;

pub trait WgpuVertexDecl
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

pub struct SceneNode<T>
{
    pub object: T,
    pub transform: Transform,
}

pub struct Scene
{
    pub models: Vec<SceneNode<Model>>,
}
impl Asset for Scene
{
    fn asset_type() -> AssetTypeId { AssetTypeId::Scene }

    fn all_dependencies_loaded(&self) -> bool
    {
        self.models.iter().all(|m| m.object.all_dependencies_loaded())
    }
}
impl Scene
{
}

pub struct GltfTexture
{
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub texel_data: Vec<u8>,
    read_offset: usize,
}
impl std::io::Read for GltfTexture
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize>
    {
        let slice = self.texel_data.as_slice();
        match (&slice[self.read_offset..]).read(buf)
        {
            Ok(ok) =>
            {
                self.read_offset += ok;
                Ok(ok)
            }
            Err(e) => Err(e)
        }
    }
}
impl std::io::Seek for GltfTexture
{
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64>
    {
        match pos
        {
            SeekFrom::Start(off) => { self.read_offset = off as usize; Ok(off) },
            SeekFrom::End(off) =>
            {
                match self.texel_data.len().checked_add_signed(off as isize)
                {
                    None => Err(std::io::Error::from(std::io::ErrorKind::InvalidInput)),
                    Some(new) => { self.read_offset = new; Ok(new as u64) }
                }
            }
            SeekFrom::Current(off) =>
            {
                match self.read_offset.checked_add_signed(off as isize)
                {
                    None => Err(std::io::Error::from(std::io::ErrorKind::InvalidInput)),
                    Some(new) => { self.read_offset = new; Ok(new as u64) }
                }
            }
        }
    }
}

pub struct SceneLifecycler
{
    renderer: Arc<Renderer>,
}
impl AssetLifecycler for SceneLifecycler
{
    type Asset = Scene;

    fn load(&self, request: AssetLoadRequest) -> AssetPayload<Self::Asset>
    {
        match self.load_internal(request)
        {
            Ok(scene) =>
            {
                AssetPayload::Available(scene)
            }
            Err(err) =>
            {
                eprintln!("Error importing scene: {err:?}");
                AssetPayload::Unavailable(match err
                {
                    SceneImportError::GltfError(_) => AssetLoadError::ParseError(0),
                    _ => AssetLoadError::ParseError(1 /* TODO */)
                })
            }
        }
    }
}
impl SceneLifecycler
{
    pub fn new(renderer: Arc<Renderer>) -> Self
    {
        Self { renderer }
    }

    fn load_internal(&self, mut request: AssetLoadRequest) -> Result<Scene, SceneImportError>
    {
        // todo: vertex buffer/index buffer allocator

        let asset_name = "TODO";

        //let (document, buffers, _img) = gltf::import(file).map_err(SceneImportError::GltfError)?;
        let gltf::Gltf { document, blob } = gltf::Gltf::from_reader(&mut request.input).map_err(SceneImportError::GltfError)?;

        let buffers =  gltf::import_buffers(&document, None, blob).map_err(SceneImportError::GltfError)?;
        let images = gltf::import_images(&document, None, &buffers).map_err(SceneImportError::GltfError)?;

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
                let mut colors = prim_reader.read_colors(0).map(|c| c.into_rgba_u8());

                for p in positions.into_iter()
                {
                    let n = normals.next().ok_or(SceneImportError::MismatchedVertexAttributeLengths)?;
                    let tc = tex_coords.next().ok_or(SceneImportError::MismatchedVertexAttributeLengths)?;
                    let c = match &mut colors
                    {
                        Some(c) => c.next().ok_or(SceneImportError::MismatchedVertexAttributeLengths)?.into(),
                        None => Rgba::from(in_prim.index() as u32 * 10000 + 20000),
                    };

                    vertices.push(VertexPosNormTexCol
                    {
                        position: p.into(),
                        normal: n.into(),
                        tex_coord: tc.into(),
                        color: c,
                    });
                }

                let vbuffer = self.renderer.device().create_buffer_init(&BufferInitDescriptor
                {
                    label: Some(format!("{asset_name} vertices").as_str()),
                    contents: unsafe { vertices.as_u8_slice() },
                    usage: BufferUsages::VERTEX,
                });

                let indices = prim_reader.read_indices().ok_or(SceneImportError::MissingIndices)?;

                let ibuffer_label = format!("{asset_name} indices");

                let index_fmt;
                let index_count;
                let ibuffer = match indices
                {
                    ReadIndices::U8(u8s) =>
                    {
                        let vec = u8s.map(|u| u as u16).collect::<Vec<u16>>();
                        index_fmt = IndexFormat::Uint16;
                        index_count = vec.len();
                        self.renderer.device().create_buffer_init(&BufferInitDescriptor
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
                        self.renderer.device().create_buffer_init(&BufferInitDescriptor
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
                        self.renderer.device().create_buffer_init(&BufferInitDescriptor
                        {
                            label: Some(ibuffer_label.as_str()),
                            contents: unsafe { vec.as_u8_slice() },
                            usage: BufferUsages::INDEX,
                        })
                    },
                };

                let pbr = in_prim.material().pbr_metallic_roughness();
                let albedo_map = match pbr.base_color_texture()
                {
                    None => None,
                    Some(tex) =>
                    {
                        let tex_index = tex.texture().source().index();
                        let data = &images[tex_index];
                        let tex_name = tex.texture().name().map_or_else(|| { format!("{asset_name}:tex{}", tex_index) }, |n| n.to_string());
                        let reader = GltfTexture
                        {
                            name: tex_name.clone(),
                            width: data.width,
                            height: data.height,
                            texel_data: data.pixels.clone(),
                            read_offset: 0,
                        };
                        // let tex: AssetHandle<Texture> = request.load_dependency_from(&tex_name, reader);
                        // // todo: this needs to reconcile the image format
                        // Some(tex)
                        None
                    }
                };

                let material = Material
                {
                    albedo_map,
                    albedo_color: pbr.base_color_factor().into(),
                    metallicity: pbr.metallic_factor(),
                    roughness: pbr.roughness_factor(),
                };

                meshes.push(Mesh
                {
                    bounds: mesh_bounds,
                    vertices: vbuffer,
                    vertex_count: vertices.len() as u32,
                    indices: ibuffer,
                    index_count: index_count as u32,
                    index_format: index_fmt,
                    material,
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
            // todo: probably need to dedupe meshes

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