use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::Write;
use glam::Vec3;
use gltf::mesh::util::ReadIndices;
use unicase::UniCase;
use game_3l14::engine::AABB;
use game_3l14::engine::assets::AssetTypeId;
use game_3l14::engine::graphics::{ModelFile, ModelFileMesh, ModelFileMeshIndices, Rgba, VertexPosNormTexCol};
use crate::core::{AssetBuilder, BuildOutputs, SourceInput, VersionStrings};

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
impl std::error::Error for ModelImportError { }

pub struct ModelBuilder;
impl AssetBuilder for ModelBuilder
{
    fn supported_input_file_extensions(&self) -> &'static [&'static str]
    {
        &["glb", "gltf"]
    }

    fn builder_version(&self) -> VersionStrings
    {
        &[
            b"Initial"
        ]
    }

    fn format_version(&self) -> VersionStrings
    {
        &[
            b"Initial"
        ]
    }

    fn build_assets(&self, input: SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        if input.file_extension() == &UniCase::new("glb") ||
            input.file_extension() == &UniCase::new("gltf")
        {
        //let (document, buffers, _img) = gltf::import(file)?;
            let gltf::Gltf { document, blob } = gltf::Gltf::from_reader(input)?;

            let buffers =  gltf::import_buffers(&document, None, blob)?;
            let images = gltf::import_images(&document, None, &buffers)?;

            for mesh in document.meshes()
            {
                let model = parse_gltf(mesh, &buffers, &images)?;
                let mut output = outputs.add_output(AssetTypeId::Model)?;

                output.serialize(&model)?;
                output.finish()?;
            }
        }

        Ok(())
    }
}


fn parse_gltf(in_mesh: gltf::Mesh, buffers: &Vec<gltf::buffer::Data>, images: &Vec<gltf::image::Data>) -> Result<ModelFile, ModelImportError>
{
    let mut model_bounds = AABB { min: Vec3::MAX, max: Vec3::MIN };
    let mut meshes: Vec<ModelFileMesh> = Vec::new();

    let mut model_bounds = AABB::zero();

    // todo: iter.map() ?
    for in_prim in in_mesh.primitives()
    {
        let bb = in_prim.bounding_box();
        let mesh_bounds = AABB::new(bb.min.into(), bb.max.into());
        model_bounds.union_with(mesh_bounds);

        let prim_reader = in_prim.reader(|b| Some(&buffers[b.index()]));
        let positions = prim_reader.read_positions().ok_or(ModelImportError::NoPositionData)?;
        let mut normals = prim_reader.read_normals().ok_or(ModelImportError::NoNormalData)?;
        let mut tex_coords = prim_reader.read_tex_coords(0).ok_or(ModelImportError::NoTexcoordData)?.into_f32();
        let mut colors = prim_reader.read_colors(0).map(|c| c.into_rgba_u8());

        let mut vertices = Vec::new();
        for p in positions.into_iter()
        {
            let n = normals.next().ok_or(ModelImportError::MismatchedVertexCount)?;
            let tc = tex_coords.next().ok_or(ModelImportError::MismatchedVertexCount)?;
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

        // let pbr = in_prim.material().pbr_metallic_roughness();
        // let albedo_map = match pbr.base_color_texture()
        // {
        //     None => None,
        //     Some(tex) =>
        //         {
        //             let tex_index = tex.texture().source().index();
        //             let data = &images[tex_index];
        //             let tex_name = tex.texture().name().map_or_else(|| { format!("{asset_name}:tex{}", tex_index) }, |n| n.to_string());
        //             let reader = GltfTexture
        //             {
        //                 name: tex_name.clone(),
        //                 width: data.width,
        //                 height: data.height,
        //                 texel_data: data.pixels.clone(),
        //                 read_offset: 0,
        //             };
        //             // let tex: AssetHandle<Texture> = request.load_dependency_from(&tex_name, reader);
        //             // // todo: this needs to reconcile the image format
        //             // Some(tex)
        //             None
        //         }
        // };
        // let material = Material
        // {
        //     albedo_map,
        //     albedo_color: pbr.base_color_factor().into(),
        //     metallicity: pbr.metallic_factor(),
        //     roughness: pbr.roughness_factor(),
        // };

        let mesh_bounds = AABB::new(bb.min.into(), bb.max.into());
        model_bounds.union_with(mesh_bounds);

        // TODO: assert vertex/index count < 2^32
        meshes.push(ModelFileMesh
        {
            vertices: vertices.into_boxed_slice(),
            indices,
            bounds: mesh_bounds,
        });
    }

    Ok(ModelFile
    {
        bounds: model_bounds,
        meshes: meshes.into_boxed_slice(),
    })
}