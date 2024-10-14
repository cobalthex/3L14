use crate::core::{AssetBuilder, AssetBuilderMeta, BuildOutputs, SourceInput, VersionStrings};
use game_3l14::engine::asset::AssetTypeId;
use game_3l14::engine::graphics::assets::material::{MaterialFile, PbrProps};
use game_3l14::engine::graphics::assets::{TextureFile, TextureFilePixelFormat};
use game_3l14::engine::graphics::{ModelFile, ModelFileMesh, ModelFileMeshIndices, Rgba, VertexPosNormTexCol};
use game_3l14::engine::AABB;
use gltf::image::Format;
use gltf::mesh::util::{ReadIndices, ReadTexCoords};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::Write;
use unicase::UniCase;

#[derive(Debug)]
pub enum ModelImportError
{
    NoPositionData,
    NoNormalData,
    NoTexcoordData,
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

pub struct ModelBuilder;
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

    fn build_assets(&self, _config: Self::BuildConfig, input: SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        if input.file_extension() == &UniCase::new("glb") ||
            input.file_extension() == &UniCase::new("gltf")
        {
            let gltf::Gltf { document, blob } = gltf::Gltf::from_reader(input)?;

            let buffers =  gltf::import_buffers(&document, None, blob)?;
            let images = gltf::import_images(&document, None, &buffers)?;

            for gltf_mesh in document.meshes()
            {
                let model = parse_gltf(gltf_mesh, &buffers, &images, outputs)?;
                let mut model_output = outputs.add_output(AssetTypeId::Model)?;

                for mesh in &model.meshes
                {
                    model_output.depends_on(mesh.material);
                }

                model_output.serialize(&model)?;
                model_output.finish()?;
            }
        }

        Ok(())
    }
}


fn parse_gltf(in_mesh: gltf::Mesh, buffers: &Vec<gltf::buffer::Data>, images: &Vec<gltf::image::Data>, outputs: &mut BuildOutputs) -> Result<ModelFile, Box<dyn Error>>
{
    let mut meshes: Vec<ModelFileMesh> = Vec::new();
    let mut model_bounds = AABB::max_min();

    // todo: iter.map() ?
    for in_prim in in_mesh.primitives()
    {
        let bb = in_prim.bounding_box();
        let mesh_bounds = AABB::new(bb.min.into(), bb.max.into());
        model_bounds.union_with(mesh_bounds);

        let prim_reader = in_prim.reader(|b| Some(&buffers[b.index()]));
        let positions = prim_reader.read_positions().ok_or(ModelImportError::NoPositionData)?;
        let mut normals = prim_reader.read_normals().ok_or(ModelImportError::NoNormalData)?;
        let mut tex_coords = prim_reader.read_tex_coords(0).map(|t| t.into_f32());
        let mut colors = prim_reader.read_colors(0).map(|c| c.into_rgba_u8());

        let mut vertices = Vec::new();
        for p in positions.into_iter()
        {
            let n = normals.next().ok_or(ModelImportError::MismatchedVertexCount)?;
            let tc = match &mut tex_coords
            {
                None => [0.0, 0.0],
                Some(tc) => tc.next().ok_or(ModelImportError::MismatchedVertexCount)?,
            };
            let c = match &mut colors
            {
                Some(c) => c.next().ok_or(ModelImportError::MismatchedVertexCount)?.into(),
                None => Rgba::from(in_prim.index() as u32 * 10000 + 20000), // TODO: testing
            };

            vertices.push(VertexPosNormTexCol
            {
                position: p.into(),
                normal: n.into(),
                tex_coord: tc.into(),
                color: c,
            });
        };

        let indices = match prim_reader.read_indices().ok_or(ModelImportError::NoIndexData)?
        {
            ReadIndices::U8(u8s) =>
            {
                ModelFileMeshIndices::U16(u8s.map(|u| u as u16).collect())
            },
            ReadIndices::U16(u16s) =>
            {
                ModelFileMeshIndices::U16(u16s.collect())
            },
            ReadIndices::U32(u32s) =>
            {
                ModelFileMeshIndices::U32(u32s.collect())
            },
        };

        let mut textures = Vec::new();

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

            textures.push(tex_output.finish()?);
        }

        let material =
        {
            // call into MaterialBuilder?
            let mut mtl_output = outputs.add_output(AssetTypeId::RenderMaterial)?;

            mtl_output.depends_on_multiple(&textures);

            mtl_output.serialize(&MaterialFile
            {
                textures: textures.into_boxed_slice(),
                pbr_props: PbrProps
                {
                    albedo_color: pbr.base_color_factor().into(),
                    metallicity: pbr.metallic_factor(),
                    roughness: pbr.roughness_factor(),
                },
            })?;
            mtl_output.finish()?
        };

        let mesh_bounds = AABB::new(bb.min.into(), bb.max.into());
        model_bounds.union_with(mesh_bounds);

        // TODO: assert vertex/index count < 2^32
        meshes.push(ModelFileMesh
        {
            vertices: vertices.into_boxed_slice(),
            indices,
            bounds: mesh_bounds,
            material,
        });
    }

    Ok(ModelFile
    {
        bounds: model_bounds,
        meshes: meshes.into_boxed_slice(),
    })
}