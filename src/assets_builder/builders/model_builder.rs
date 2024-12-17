use game_3l14::engine::graphics::assets::ModelFileSurface;
use game_3l14::engine::graphics::assets::ModelFile;
use crate::core::{AssetBuilder, AssetBuilderMeta, BuildOutputs, SourceInput, VersionStrings};
use crate::helpers::shader_compiler::{ShaderCompilation, ShaderCompiler};
use arrayvec::ArrayVec;
use game_3l14::engine::alloc_slice::alloc_u8_slice;
use game_3l14::engine::asset::{AssetKey, AssetKeySynthHash, AssetTypeId};
use game_3l14::engine::graphics::assets::material::{MaterialFile, PbrProps};
use game_3l14::engine::graphics::assets::{GeometryFile, GeometryFileMesh, GeometryMesh, IndexFormat, MaterialClass, ShaderFile, ShaderStage, TextureFile, TextureFilePixelFormat, VertexLayout};
use game_3l14::engine::{as_u8_array, AABB};
use gltf::image::Format;
use gltf::mesh::util::ReadIndices;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::mem;
use std::path::{Path, PathBuf};
use metrohash::MetroHash64;
use unicase::UniCase;
use game_3l14::engine::inline_hash::InlineWriteHash;

#[derive(Hash)]
struct ShaderHash
{
    stage: ShaderStage,
    material_class: MaterialClass,
    vertex_layout: VertexLayout,
    // custom file name
}

#[repr(C)]
struct StaticSimpleVertex
{
    position: [f32; 3],
    normal: [f32; 3],
    tex_coord: [f32; 2],
    color: [u8; 4],
}

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

            for gltf_mesh in document.meshes()
            {
                self.parse_gltf(gltf_mesh, &buffers, &images, outputs)?;
            }
        }

        Ok(())
    }
}
impl ModelBuilder
{
    fn parse_gltf(&self, in_mesh: gltf::Mesh, buffers: &Vec<gltf::buffer::Data>, images: &Vec<gltf::image::Data>, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        let mut model_output = outputs.add_output(AssetTypeId::Model)?;

        let mut meshes = Vec::new();
        let mut surfaces = Vec::new();
        let mut model_bounds = AABB::max_min();

        let vertex_layout = VertexLayout::StaticSimple; // TODO: figure out from model

        // todo: iter.map() ?
        for in_prim in in_mesh.primitives()
        {
            let bb = in_prim.bounding_box();
            let mesh_bounds = AABB::new(bb.min.into(), bb.max.into());
            model_bounds.union_with(mesh_bounds);

            let prim_reader = in_prim.reader(|b| Some(&buffers[b.index()]));
            let positions = prim_reader.read_positions().ok_or(ModelImportError::NoPositionData)?; // not required?
            let mut normals = prim_reader.read_normals();
            let mut tangents = prim_reader.read_tangents();
            let mut tex_coords = prim_reader.read_tex_coords(0).map(|t| t.into_f32());
            let mut colors = prim_reader.read_colors(0).map(|c| c.into_rgba_u8());

            let mut vertex_count = 0;
            let mut vertices = Vec::new();
            for pos in positions.into_iter()
            {
                let vertex = StaticSimpleVertex
                {
                    position: pos,
                    normal: normals.as_mut().and_then(|mut r| r.next()).unwrap_or([0.0, 0.0, 1.0]),
                    tex_coord: tex_coords.as_mut().and_then(|mut r| r.next()).unwrap_or([0.0, 0.0]),
                    color: colors.as_mut().and_then(|mut r| r.next()).unwrap_or([u8::MAX, u8::MAX, u8::MAX, u8::MAX]),
                };

                // TODO: byte order
                vertices.write_all(unsafe { as_u8_array(&vertex) })?;
                vertex_count += 1;
            };

            // TODO: create indices if missing

            let index_format;
            let mut index_count = 0;
            let mut indices = Vec::new();
            match prim_reader.read_indices()
            {
                None => todo!("Need to add create-index fallback support"),
                Some(in_indices) =>
                {
                    match in_indices
                    {
                        ReadIndices::U8(u8s) => todo!("is this common?"),
                        ReadIndices::U16(u16s) =>
                        {
                            index_format = IndexFormat::U16;
                            for i in u16s
                            {
                                indices.write_all(&i.to_le_bytes())?;
                                index_count += 1;
                            }
                        }
                        ReadIndices::U32(u32s) =>
                        {
                            index_format = IndexFormat::U32;
                            for i in u32s
                            {
                                indices.write_all(&i.to_le_bytes())?;
                                index_count += 1;
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
                let mut mtl_output = outputs.add_output(AssetTypeId::RenderMaterial)?;

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
                vertex_layout,
            });
            const FORCE_BUILD_SHADERS: bool = true;
            if let Some(mut vshader_output) = outputs.add_synthetic(AssetTypeId::Shader, vertex_shader_key, FORCE_BUILD_SHADERS)?
            {
                log::debug!("Compiling vertex shader {:?}", vshader_output.asset_key());

                let shader_file = self.shaders_root.join(format!("{material_class:?}.vs.hlsl"));

                let shader_source = std::fs::read_to_string(&shader_file)?;
                let mut shader_module = InlineWriteHash::<MetroHash64, _>::new(Vec::new());
                let vshader = self.shader_compiler.compile_hlsl(&mut shader_module, ShaderCompilation
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
                vertex_layout,
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

            let mesh_bounds = AABB::new(bb.min.into(), bb.max.into());
            model_bounds.union_with(mesh_bounds);

            meshes.push(GeometryFileMesh
            {
                bounds: mesh_bounds,
                vertex_layout,
                index_format,
                vertex_count,
                index_count,
                vertices: vertices.into_boxed_slice(),
                indices: indices.into_boxed_slice(),
            });

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
                bounds: model_bounds,
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