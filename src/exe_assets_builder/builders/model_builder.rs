use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use crate::core::{AssetBuilder, AssetBuilderMeta, BuildOutputs, SourceInput, VersionStrings};
use crate::helpers::shader_compiler::{ShaderCompileFlags, ShaderCompilation, ShaderCompiler};
use arrayvec::ArrayVec;
use asset_3l14::{AssetKey, AssetKeySynthHash, AssetTypeId};
use glam::{Mat4, Quat, Vec3};
use gltf::image::Format;
use gltf::mesh::util::ReadIndices;
use graphics_3l14::assets::{GeometryFile, GeometryMesh, IndexFormat, MaterialClass, MaterialFile, ModelFile, ModelFileSurface, PbrProps, ShaderFile, ShaderStage, Skeleton, SkeletonDebugData, TextureFile, TextureFilePixelFormat, VertexLayout};
use graphics_3l14::vertex_layouts::{SkinnedVertex, StaticVertex, VertexDecl, VertexLayoutBuilder};
use math_3l14::{DualQuat, Sphere, AABB};
use metrohash::MetroHash64;
use nab_3l14::utils::alloc_slice::{alloc_slice_default, alloc_slice_uninit, alloc_u8_slice};
use nab_3l14::utils::as_u8_array;
use nab_3l14::utils::inline_hash::InlineWriteHash;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use gltf::animation::util::ReadOutputs;
use unicase::UniCase;
use wgpu::VertexBufferLayout;
use nab_3l14::timing::FSeconds;

#[derive(Hash)]
struct ShaderHash
{
    stage: ShaderStage,
    material_class: MaterialClass,
    vertex_layout_hash: u64, // all the layouts used hashed together
    // custom file name
    compile_flags: ShaderCompileFlags,
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

            for anim in document.animations()
            {
                self.parse_gltf_anim(anim, &buffers, outputs)?;
            }

            for gltf_node in document.nodes()
            {
                self.parse_gltf_mesh(gltf_node, &buffers, &images, outputs)?;
            }
        }

        Ok(())
    }
}
impl ModelBuilder
{

    fn parse_gltf_mesh(&self, in_node: gltf::Node, buffers: &Vec<gltf::buffer::Data>, images: &Vec<gltf::image::Data>, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
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

        struct SkelInfo
        {
            asset: AssetKey,
            remapped_bone_indices: Box<[u16]>,
            joint_name_hashes: Box<[u64]>,
        }

        let mut vertex_layout = VertexLayout::Static;
        let maybe_skel_info = if let Some(skin) = &in_skin
        {
            vertex_layout |= VertexLayout::Skinned;

            let bone_names: Box<[String]> = skin.joints().enumerate().map(|(i, n)|
            {
                n.name().map(|n| n.to_string()).unwrap_or_else(|| format!("{}", i))
            }).collect();
            let bone_name_hashes: Box<_> = bone_names.iter().map(|jn| hash_bone_name(&jn)).collect();
            let mut remapped_bone_indices: Box<_> = (0..bone_name_hashes.len() as u16).collect();
            remapped_bone_indices.sort_by_key(|i| bone_name_hashes[*i as usize]); // explicit sort rule?

            let mut skel_inv_bind_pose = alloc_slice_default(bone_name_hashes.len());
            let reader = skin.reader(|b| Some(&buffers[b.index()]));
            for (i, ibm) in reader.read_inverse_bind_matrices().unwrap().enumerate() // error handling?
            {
                let mtx = Mat4::from_cols_array_2d(&ibm);
                let dq = DualQuat::from(&mtx);
                skel_inv_bind_pose[remapped_bone_indices[i] as usize] = dq;
            }

            let skeleton =
            {
                let skel_key = AssetKeySynthHash::generate(&bone_name_hashes);
                if let Some(mut output) = outputs.add_synthetic(AssetTypeId::Skeleton, skel_key, false)?
                {
                    output.serialize(&Skeleton { inv_bind_pose: skel_inv_bind_pose })?;
                    output.serialize_debug::<Skeleton>(&SkeletonDebugData { bone_names, })?;
                    output.finish()?;
                }
                AssetKey::synthetic(AssetTypeId::Skeleton, skel_key)
            };

            Some(SkelInfo
            {
                asset: skeleton,
                remapped_bone_indices,
                joint_name_hashes: bone_name_hashes,
            })
        } else { None };

        // acts as versioning for the vertex formats
        let vertex_layout_hash =
        {
            let mut hasher = MetroHash64::new();
            VertexLayoutBuilder::from(vertex_layout).hash(&mut hasher);
            hasher.finish()
        };

        let mut vertex_data = Vec::new();
        let mut total_vertex_count = 0;
        let mut index_data = Vec::new();
        let mut total_index_count = 0;

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
            let mut maybe_joints = prim_reader.read_joints(0).map(|j| j.into_u16());
            let mut maybe_weights = prim_reader.read_weights(0).map(|w| w.into_f32());

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
                vertex_data.write_all(unsafe { as_u8_array(&static_vertex) })?;
                mesh_vertex_count += 1;

                // todo: cleanup
                if let Some(skel_info) = &maybe_skel_info
                {
                    let iremap = |ind: [u16;4]| ind.map(|i| skel_info.remapped_bone_indices[i as usize]);

                    // todo: verify matching attrib counts?
                    let skinned_vertex = SkinnedVertex
                    {
                        indices: maybe_joints.as_mut().and_then(|j| j.next().map(iremap)).unwrap_or([0, 0, 0, 0]),
                        weights: maybe_weights.as_mut().and_then(|w| w.next()).unwrap_or([0.0, 0.0, 0.0, 0.0]),
                    };
                    vertex_data.write_all(unsafe { as_u8_array(&skinned_vertex) })?;
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
                    }),
                })?;
                mtl_output.finish()?
            };

            let shader_compile_flags = ShaderCompileFlags::Debug;

            // todo: better asset key
            let vertex_shader_key = AssetKeySynthHash::generate(ShaderHash
            {
                stage: ShaderStage::Vertex,
                material_class,
                vertex_layout_hash,
                compile_flags: shader_compile_flags,
            });
            const FORCE_BUILD_SHADERS: bool = true;
            if let Some(mut vshader_output) = outputs.add_synthetic(AssetTypeId::Shader, vertex_shader_key, FORCE_BUILD_SHADERS)?
            {
                log::debug!("Compiling vertex shader {:?}", vshader_output.asset_key());

                let shader_file = self.shaders_root.join(format!("SkinnedOpaque.vs.hlsl"));

                let shader_source = std::fs::read_to_string(&shader_file)?;
                let mut shader_module = InlineWriteHash::<MetroHash64, _>::new(Vec::new());
                let _ = self.shader_compiler.compile_hlsl(&mut shader_module, ShaderCompilation
                {
                    source_text: &shader_source,
                    filename: &shader_file, // todo: for debugging, use asset key?
                    stage: ShaderStage::Vertex,
                    flags: shader_compile_flags,
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
                compile_flags: shader_compile_flags,
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
                    flags: shader_compile_flags,
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
                vertex_layout: vertex_layout.into(),
                index_format: IndexFormat::U16,
                vertices: vertex_data.into_boxed_slice(),
                indices: index_data.into_boxed_slice(),
                meshes: meshes.into_boxed_slice(),
            })?;
            geom_output.finish()?
        };

        model_output.serialize(&ModelFile
        {
            geometry,
            skeleton: maybe_skel_info.map(|s| s.asset),
            surfaces: surfaces.into_boxed_slice(),
        })?;
        model_output.finish()?;

        Ok(())
    }

    fn parse_gltf_anim(&self, in_anim: gltf::Animation, buffers: &Vec<gltf::buffer::Data>, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        let mut anim_name_buf = String::new();
        let anim_name = in_anim.name().unwrap_or_else(||
        {
            anim_name_buf.clear();
            std::fmt::Write::write_fmt(&mut anim_name_buf, format_args!("animation_{}", in_anim.index())).unwrap();
            &anim_name_buf
        });

        #[derive(Default, Debug)]
        struct Keyframe
        {
            translation: Option<Vec3>,
            rotation: Option<Quat>,
        }

        let mut anims = HashMap::new();

        for in_chan in in_anim.channels()
        {
            let ch_reader = in_chan.reader(|b| Some(&buffers[b.index()]));
            let target_node = in_chan.target().node();
            let mut keyframes = &mut anims.entry(target_node.index()).or_insert_with(|| (target_node.name(), BTreeMap::new())).1; // TODO: need to group by node

            // todo: figure out if in_chan.sampler().interpolation() is necessary

            let mut inputs = ch_reader.read_inputs().unwrap();
            match ch_reader.read_outputs().unwrap()
            {
                ReadOutputs::Translations(translations) =>
                {
                    for (time, translation) in inputs.zip(translations)
                    {
                        let mut entry = keyframes.entry(FSeconds(time)).or_insert_with(|| Keyframe::default());
                        // round to n digits?
                        entry.translation = Some(translation.into());
                    }
                }
                ReadOutputs::Rotations(rotations) =>
                {
                    for (time, rotation) in inputs.zip(rotations.into_f32())
                    {
                        let mut entry = keyframes.entry(FSeconds(time)).or_insert_with(|| Keyframe::default());
                        entry.rotation = Some(Quat::from_array(rotation)); // normalize?
                    }
                }
                ReadOutputs::Scales(_) => {} // unsupported (currently)
                ReadOutputs::MorphTargetWeights(_) => {} // unsupported
            }
        }

        let mut test = std::fs::File::create(format!("C:\\users\\matt\\desktop\\dump_{}.anim", in_anim.name().unwrap_or("ZZZ")))?;
        for (bone, (name, keyframes)) in anims.iter()
        {
            writeln!(test, "{} {:?}:", bone, name)?;
            for keyframe in keyframes.iter()
            {
                writeln!(test, "  {:.4} - {:?}", keyframe.0.0, keyframe.1)?;
            }
        }

        // todo: output sorted based on hash of bone name

        Ok(())
    }
}

fn hash_bone_name(name: &str) -> u64
{
    let mut hasher = MetroHash64::new();
    name.hash(&mut hasher);
    hasher.finish()
}