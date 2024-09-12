use glam::Vec3;
use gltf::Error;
use unicase::UniCase;
use game_3l14::engine::AABB;
use game_3l14::engine::graphics::{Mesh, VertexPosNormTexCol};
use game_3l14::engine::graphics::Rgba;
use crate::core::{AssetBuilder, BuildError, BuildOutputs, SourceInput};

pub struct ModelBuilder;
impl AssetBuilder for ModelBuilder
{
    fn supported_input_file_extensions(&self) -> &'static [&'static str]
    {
        &["glb", "gltf"]
    }

    fn build_assets(&self, input: SourceInput, outputs: &mut BuildOutputs) -> Result<(), BuildError>
    {
        fn gltf_err(err: gltf::Error) -> BuildError
        {
            match err
            {
                Error::Io(ioerr) => BuildError::SourceIOError(ioerr),
                _ => BuildError::Other(Box::new(err)),
            }
        }

        if input.file_extension() == &UniCase::new("glb") ||
            input.file_extension() == &UniCase::new("gltf")
        {
            //let (document, buffers, _img) = gltf::import(file).map_err(SceneImportError::GltfError)?;
            let gltf::Gltf { document, blob } = gltf::Gltf::from_reader(&mut input).map_err(SceneImportError::GltfError)?;

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
                }
            };
        }

        Ok(())
    }
}