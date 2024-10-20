use crate::core::{AssetBuilder, AssetBuilderMeta, BuildOutputs, SourceInput, VersionStrings};
use game_3l14::engine::asset::AssetTypeId;
use game_3l14::engine::graphics::assets::material::{MaterialFile, PbrProps};
use game_3l14::engine::graphics::assets::{TextureFile, TextureFilePixelFormat};
use game_3l14::engine::graphics::{ModelFile, ModelFileMesh, ModelFileMeshIndices, ModelFileMeshVertices, Rgba, VertexPosNormTexCol};
use game_3l14::engine::{AsU8Slice, IntoU8Box, AABB};
use gltf::image::Format;
use gltf::mesh::util::{ReadIndices, ReadNormals, ReadTexCoords};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::Write;
use futures::AsyncWriteExt;
use unicase::UniCase;
use wgpu::{VertexAttribute, VertexFormat};

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
        let positions = prim_reader.read_positions().ok_or(ModelImportError::NoPositionData)?; // not required?
        let mut normals = prim_reader.read_normals();
        let mut tangents = prim_reader.read_tangents();
        let mut tex_coords = prim_reader.read_tex_coords(0).map(|t| t.into_f32());
        let mut colors = prim_reader.read_colors(0).map(|c| c.into_rgba_u8());

        // todo: import settings define which vertex attributes (extra colors/texcoords)

        let mut layout = Vec::new();
        let mut next_offset = 0;
        let push_attr = |format|
        {
            layout.push(VertexAttribute
            {
                format,
                offset: next_offset,
                shader_location: layout.len() as wgpu::ShaderLocation,
            });
            next_offset += format.size();
        };
        push_attr(VertexFormat::Float32x3);
        if normals.is_some() { push_attr(VertexFormat::Float32x3) };
        if tex_coords.is_some() { push_attr(VertexFormat::Float32x2) };
        if colors.is_some() { push_attr(VertexFormat::Uint32) };

        let mut vertices = Vec::new();
        let mut vertex_count = 0;
        for pos in positions.into_iter()
        {
            vertex_count += 1; // TODO: test vertex/index count < 2^32
            // TODO: byte order

            vertices.write_all(unsafe { pos.as_u8_slice() })?;

            // todo: iterate all available attributes?

            if let Some(rn) = &mut normals
            {
                let norm = rn.next().ok_or(ModelImportError::MismatchedVertexCount)?;
                vertices.write_all(unsafe { norm.as_u8_slice() })?;
            }

            if let Some(rt) = &mut tangents
            {
                let tan = rt.next().ok_or(ModelImportError::MismatchedVertexCount)?;
                vertices.write_all(unsafe { tan.as_u8_slice() })?;
            }

            if let Some(rtc) = &mut tex_coords
            {
                let texcoord = rtc.next().ok_or(ModelImportError::MismatchedVertexCount)?;
                vertices.write_all(unsafe { texcoord.as_u8_slice() })?;
            }

            if let Some(cl) = &mut colors
            {
                let col = cl.next().ok_or(ModelImportError::MismatchedVertexCount)?;
                vertices.write_all(unsafe { cl.as_u8_slice() })?;
            }
        };

        let indices = match prim_reader.read_indices().ok_or(ModelImportError::NoIndexData)?
        {
            ReadIndices::U8(u8s) =>
            {
                // TODO: endianness
                ModelFileMeshIndices::U16(u8s.map(|u| (u as u16).to_le_bytes()).collect())
            },
            ReadIndices::U16(u16s) =>
            {
                ModelFileMeshIndices::U16(u16s.map(|u| u.to_le_bytes()).collect())
            },
            ReadIndices::U32(u32s) =>
            {
                ModelFileMeshIndices::U32(u32s.map(|u| u.to_le_bytes()).collect())
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

        meshes.push(ModelFileMesh
        {
            vertices: ModelFileMeshVertices
            {
                stride: next_offset,
                count: vertex_count,
                layout: unsafe { layout.into_u8_box() },
                data: vertices.into_boxed_slice(),
            },
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