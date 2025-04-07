use crate::core::{AssetBuilder, AssetBuilderMeta, BuildOutputs, SourceInput, VersionStrings};
use crate::helpers::shader_compiler::{ShaderCompilation, ShaderCompiler};
use arrayvec::ArrayVec;
use gltf::image::Format;
use gltf::mesh::util::ReadIndices;
use metrohash::MetroHash64;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use glam::Vec3;
use asset_3l14::{AssetKey, AssetKeySynthHash, AssetTypeId};
use unicase::UniCase;
use graphics_3l14::assets::{GeometryFile, GeometryMesh, IndexFormat, MaterialClass, MaterialFile, ModelFile, ModelFileSurface, PbrProps, ShaderFile, ShaderStage, TextureFile, TextureFilePixelFormat};
use graphics_3l14::vertex_layouts::{SkinnedVertex, StaticVertex, VertexDecl};
use math_3l14::{Sphere, AABB};
use nab_3l14::utils::alloc_slice::alloc_u8_slice;
use nab_3l14::utils::as_u8_array;
use nab_3l14::utils::inline_hash::InlineWriteHash;

#[derive(Hash)]
struct ShaderHash
{
    stage: ShaderStage,
    material_class: MaterialClass,
    vertex_layout_hash: u64, // all the layouts used hashed together
    // custom file name
}

// bit flags for which vertex types are avail?

#[derive(Debug)]
pub enum ModelImportError
{
    NoPositionData,
    NoNormalData,
    NoIndexData,
    MismatchedVertexCount,
}
impl Display for ModelImportError
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { std::fmt::Debug::fmt(&self, f) }
}
impl Error for ModelImportError { }

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelBuildConfig
{
    // optimize (meshoptimizer)
}
pub struct ModelBuilder
{
    shader_compiler: ShaderCompiler,
    shaders_root: PathBuf,
}
impl ModelBuilder
{
    pub fn new(assets_root: impl AsRef<Path>) -> Self
    {
        let shaders_root = assets_root.as_ref().join("shaders");
        Self
        {
            shader_compiler: ShaderCompiler::new(assets_root.as_ref(), None).expect("Failed to create shader compiler"), // return error?
            shaders_root,
        }
    }
}
impl AssetBuilderMeta for ModelBuilder
{
    fn supported_input_file_extensions() -> &'static [&'static str]
    {
        &["glb", "gltf"]
    }

    fn builder_version() -> VersionStrings
    {
        &[
            b"Initial"
        ]
    }

    fn format_version() -> VersionStrings
    {
        // TODO: hash the serialized type layouts
        &[
            b"Initial"
        ]
    }
}
impl AssetBuilder for ModelBuilder
{
    type BuildConfig = ModelBuildConfig;

    fn build_assets(&self, _config: Self::BuildConfig, input: &mut SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        if input.file_extension() == &UniCase::new("glb") ||
            input.file_extension() == &UniCase::new("gltf")
        {
            let gltf::Gltf { document, blob } = gltf::Gltf::from_reader(input)?;

            let buffers =  gltf::import_buffers(&document, None, blob)?;
            let images = gltf::import_images(&document, None, &buffers)?;

            for gltf_node in document.nodes()
            {
                self.parse_gltf(gltf_node   , &buffers, &images, outputs)?;
            }
        }

        Ok(())
    }
}
impl ModelBuilder
{
    fn parse_gltf(&self, in_node: gltf::Node, buffers: &Vec<gltf::buffer::Data>, images: &Vec<gltf::image::Data>, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        // pass in node?
        let Some(in_mesh) = in_node.mesh() else { return Ok(()); };
        let in_skin = in_node.skin();

        let mut model_output = outputs.add_output(AssetTypeId::Model)?;

        let mut meshes = Vec::new();
        let mut surfaces = Vec::new();
        let mut model_bounds_aabb = AABB::MAX_MIN;

        // TODO: split up this file
        // TODO: rethink vertex parsing

        let mut static_vertex_data = Vec::new();
        let mut skinned_vertex_data = Vec::new();
        let mut total_vertex_count = 0;
        let mut index_data = Vec::new();
        let mut total_index_count = 0;

        let vertex_layout_hash =
        {
            let mut hasher = MetroHash64::new();
            StaticVertex::layout().hash(&mut hasher);
            // TODO: verify vertex data is available
            // TODO: perhaps generate VertexLayout and have it hash
            if in_skin.is_some()
            {
                StaticVertex::layout().hash(&mut hasher);
            }
            hasher.finish()
        };

        let mut model_bounds_sphere = Sphere::EMPTY;

        let mut mesh_points = Vec::<Vec3>::new();

        // todo: iter.map() ?
        for in_prim in in_mesh.primitives()
        {
            let bb = in_prim.bounding_box();
            let mesh_bounds = AABB::new(bb.min.into(), bb.max.into());
            model_bounds_aabb.union_with(mesh_bounds);

            let prim_reader = in_prim.reader(|b| Some(&buffers[b.index()]));
            let positions = prim_reader.read_positions().ok_or(ModelImportError::NoPositionData)?; // not required?
            let mut normals = prim_reader.read_normals();
            let mut tangents = prim_reader.read_tangents();
            let mut tex_coords = prim_reader.read_tex_coords(0).map(|t| t.into_f32());
            let mut colors = prim_reader.read_colors(0).map(|c| c.into_rgba_u8());

            let mut mesh_vertex_count = 0;
            for pos in positions.into_iter()
            {
                mesh_points.push(pos.into());

                // todo: verify matching attrib counts?
                let static_vertex = StaticVertex
                {
                    position: pos,
                    normal: normals.as_mut().and_then(|mut r| r.next()).unwrap_or([0.0, 0.0, 1.0]),
                    tex_coord: tex_coords.as_mut().and_then(|mut r| r.next()).unwrap_or([0.0, 0.0]),
                    color: colors.as_mut().and_then(|mut r| r.next()).unwrap_or([u8::MAX, u8::MAX, u8::MAX, u8::MAX]),
                };
                static_vertex_data.write_all(unsafe { as_u8_array(&static_vertex) })?;
                mesh_vertex_count += 1;
            };

            if in_skin.is_some()
            {
                let mut maybe_joints = prim_reader.read_joints(0);
                let mut maybe_weights = prim_reader.read_weights(0);
                if maybe_joints.is_none() || maybe_weights.is_none()
                {
                    log::warn!("gLTF node '{}' (#{}) has a skin but no bone influence data",
                        in_node.name().unwrap_or_default(),
                        in_node.index());
                }
                else
                {
                    // TODO: make sure vertex counts match
                    let mut joints = maybe_joints.unwrap().into_u16();
                    let mut weights = maybe_weights.unwrap().into_f32();
                    for joint in joints
                    {
                        // todo: verify matching attrib counts?
                        let skinned_vertex = SkinnedVertex
                        {
                            indices: joint,
                            weights: weights.next().unwrap_or([0.0, 0.0, 0.0, 0.0]),
                        };

                        skinned_vertex_data.write_all(unsafe { as_u8_array(&skinned_vertex) })?;
                    }
                }
            }

            // TODO: create indices if missing (?)

            let index_format;
            let mut mesh_index_count = 0;
            match prim_reader.read_indices()
            {
                None => todo!("Need to add create-index fallback support"),
                Some(in_indices) =>
                {
                    match in_indices
                    {
                        ReadIndices::U8(_u8s) => todo!("is this common?"),
                        ReadIndices::U16(u16s) =>
                        {
                            index_format = IndexFormat::U16;
                            for i in u16s
                            {
                                index_data.write_all(&i.to_le_bytes())?;
                                mesh_index_count += 1;
                            }
                        }
                        ReadIndices::U32(u32s) =>
                        {
                            index_format = IndexFormat::U32;
                            for i in u32s
                            {
                                index_data.write_all(&i.to_le_bytes())?;
                                mesh_index_count += 1;
                            }
                        }
                    };
                }
            }

            let mut textures = ArrayVec::new();

            let pbr = in_prim.material().pbr_metallic_roughness();
            if let Some(tex) = pbr.base_color_texture()
            {
                let tex_index = tex.texture().source().index();
                let tex_data = &images[tex_index];

                let mut tex_output = outputs.add_output(AssetTypeId::Texture)?;

                tex_output.serialize(&TextureFile
                {
                    width: tex_data.width,
                    height: tex_data.height,
                    depth: 1,
                    mip_count: 1,
                    mip_offsets: Default::default(),
                    pixel_format: match tex_data.format
                    {
                        Format::R8 => TextureFilePixelFormat::R8,
                        Format::R8G8 => TextureFilePixelFormat::Rg8,
                        Format::R8G8B8 => todo!("R8G8B8 textures"),
                        Format::R8G8B8A8 => TextureFilePixelFormat::Rgba8,
                        Format::R16 => todo!("R16 textures"),
                        Format::R16G16 => todo!("R16G16 textures"),
                        Format::R16G16B16 => todo!("R16G16B16 textures"),
                        Format::R16G16B16A16 => todo!("R16G16B16A16 textures"),
                        Format::R32G32B32FLOAT => todo!("R32G32B32FLOAT textures"),
                        Format::R32G32B32A32FLOAT => todo!("R32G32B32A32FLOAT textures"),
                    }
                })?;

                tex_output.write_all(&tex_data.pixels)?;

                let tex = tex_output.finish()?;
                textures.try_push(tex)?;
                model_output.depends_on(tex);
            }

            let material_class = MaterialClass::SimpleOpaque; // TODO
            let material =
            {
                // call into MaterialBuilder?
                let mut mtl_output = outputs.add_output(AssetTypeId::Material)?;

                mtl_output.depends_on_multiple(&textures);

                mtl_output.serialize(&MaterialFile
                {
                    class: material_class,
                    textures,
                    props: alloc_u8_slice(PbrProps
                    {
                        albedo_color: pbr.base_color_factor().into(),
                        metallicity: pbr.metallic_factor(),
                        roughness: pbr.roughness_factor(),
                    })?,
                })?;
                mtl_output.finish()?
            };

            // todo: better asset key
            let vertex_shader_key = AssetKeySynthHash::generate(ShaderHash
            {
                stage: ShaderStage::Vertex,
                material_class,
                vertex_layout_hash,
            });
            const FORCE_BUILD_SHADERS: bool = true;
            if let Some(mut vshader_output) = outputs.add_synthetic(AssetTypeId::Shader, vertex_shader_key, FORCE_BUILD_SHADERS)?
            {
                log::debug!("Compiling vertex shader {:?}", vshader_output.asset_key());

                let shader_file = self.shaders_root.join(format!("{material_class:?}.vs.hlsl"));

                let shader_source = std::fs::read_to_string(&shader_file)?;
                let mut shader_module = InlineWriteHash::<MetroHash64, _>::new(Vec::new());
                let _ = self.shader_compiler.compile_hlsl(&mut shader_module, ShaderCompilation
                {
                    source_text: &shader_source,
                    filename: &shader_file, // todo: for debugging, use asset key?
                    stage: ShaderStage::Vertex,
                    debug: true,
                    emit_symbols: false,
                    defines: vec![],
                })?;
                let (module_hash, module_bytes) = shader_module.finish();
                vshader_output.serialize(&ShaderFile
                {
                    stage: ShaderStage::Vertex,
                    module_bytes: module_bytes.into_boxed_slice(),
                    module_hash,
                })?;
                vshader_output.finish()?;
            }

            // todo: better asset key
            let pixel_shader_key = AssetKeySynthHash::generate(ShaderHash
            {
                stage: ShaderStage::Pixel,
                material_class,
                vertex_layout_hash,
            });
            if let Some(mut pshader_output) = outputs.add_synthetic(AssetTypeId::Shader, pixel_shader_key, FORCE_BUILD_SHADERS)?
            {
                log::debug!("Compiling pixel shader {:?}", pshader_output.asset_key());

                let shader_file = self.shaders_root.join(format!("{material_class:?}.ps.hlsl"));

                let shader_source = std::fs::read_to_string(&shader_file)?;
                let mut shader_module = InlineWriteHash::<MetroHash64, _>::new(Vec::new());
                let _ = self.shader_compiler.compile_hlsl(&mut shader_module, ShaderCompilation
                {
                    source_text: &shader_source,
                    filename: &shader_file, // todo: for debugging, use asset key?
                    stage: ShaderStage::Pixel,
                    debug: true,
                    emit_symbols: false,
                    defines: vec![], // TODO
                })?;
                let (module_hash, module_bytes) = shader_module.finish();
                pshader_output.serialize(&ShaderFile
                {
                    stage: ShaderStage::Pixel,
                    module_bytes: module_bytes.into_boxed_slice(),
                    module_hash,
                })?;
                pshader_output.finish()?;
            }

            let mesh_bounds_aabb = AABB::new(bb.min.into(), bb.max.into());
            model_bounds_aabb.union_with(mesh_bounds_aabb);

            // TODO
            let mesh_bounds_sphere = Sphere::new(Vec3::ZERO, 1.0);//Sphere::from_points(&mesh_points);
            mesh_points.clear();
            model_bounds_sphere += mesh_bounds_sphere;

            meshes.push(GeometryMesh
            {
                bounds_aabb: mesh_bounds_aabb,
                bounds_sphere: mesh_bounds_sphere,
                vertex_range: (total_vertex_count, total_vertex_count + mesh_vertex_count),
                index_range: (total_index_count, total_index_count + mesh_index_count),
            });

            total_vertex_count += mesh_vertex_count;
            total_index_count += mesh_index_count;

            surfaces.push(ModelFileSurface
            {
                material,
                vertex_shader: AssetKey::synthetic(AssetTypeId::Shader, vertex_shader_key),
                pixel_shader: AssetKey::synthetic(AssetTypeId::Shader, pixel_shader_key),
            });
        }

        let geometry =
        {
            let mut geom_output = outputs.add_output(AssetTypeId::Geometry)?;

            geom_output.serialize(&GeometryFile
            {
                bounds_aabb: model_bounds_aabb,
                bounds_sphere: model_bounds_sphere,
                static_vertices: static_vertex_data.into_boxed_slice(),
                skinned_vertices: if skinned_vertex_data.len() > 0 { Some(skinned_vertex_data.into_boxed_slice()) } else { None },
                index_format: IndexFormat::U16,
                indices: index_data.into_boxed_slice(),
                meshes: meshes.into_boxed_slice(),
            })?;
            geom_output.finish()?
        };

        model_output.serialize(&ModelFile
        {
            geometry,
            surfaces: surfaces.into_boxed_slice(),
        })?;
        model_output.finish()?;
        Ok(())
    }
}