use crate::core::{AssetBuilder, AssetBuilderMeta, BuildError, BuildOutputs, SourceInput, VersionStrings};
use crate::helpers::shader_compiler::{ShaderCompilation, ShaderCompileFlags, ShaderCompiler};
use arrayvec::ArrayVec;
use asset_3l14::{AssetKey, AssetKeySynthHash, AssetTypeId};
use debug_3l14::debug_gui::DebugGuiBase;
use glam::{Mat4, Quat, Vec3};
use gltf::animation::util::{ReadOutputs, Translations};
use gltf::image::Format;
use gltf::mesh::util::ReadIndices;
use graphics_3l14::assets::{AnimFrameNumber, BoneId, GeometryFile, GeometryMesh, IndexFormat, MaterialClass, MaterialFile, ModelFile, ModelFileSurface, PbrProps, Shader, ShaderDebugData, ShaderFile, ShaderStage, SkeletalAnimation, Skeleton, SkeletonDebugData, TextureFile, TextureFilePixelFormat, VertexLayout};
use graphics_3l14::vertex_layouts::{SkinnedVertex, StaticVertex, VertexDecl, VertexLayoutBuilder};
use log::kv::Key;
use math_3l14::{DualQuat, Ratio, Sphere, AABB};
use metrohash::MetroHash64;
use nab_3l14::timing::FSeconds;
use nab_3l14::utils::alloc_slice::{alloc_slice_default, alloc_slice_uninit, alloc_u8_slice};
use nab_3l14::utils::as_u8_array;
use nab_3l14::utils::inline_hash::InlineWriteHash;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{btree_map, BTreeMap, HashMap};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::iter::Peekable;
use std::path::{Path, PathBuf};
use toml::value::Index;
use unicase::UniCase;
use wgpu::VertexBufferLayout;

const DEFAULT_ANIM_SAMPLE_RATE: Ratio<u32> = Ratio::new(1, 30);

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
    DuplicateBoneIndices,
    DuplicateBoneParents,
    TooManyBones,
    UnnamedBones, // bone names are required
    AnimationTimesOutOfOrder,
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

    fn build_assets(
        &self,
        _config: Self::BuildConfig,
        input: &mut SourceInput,
        outputs: &mut BuildOutputs)
    -> Result<(), Box<dyn Error>>
    {
        if input.file_extension() == &UniCase::new("glb") ||
            input.file_extension() == &UniCase::new("gltf")
        {
            let gltf::Gltf { document, blob } = gltf::Gltf::from_reader(input)?;

            let buffers =  gltf::import_buffers(&document, None, blob)?;
            let images = gltf::import_images(&document, None, &buffers)?;

            let skeletons: Box<_> = document.skins()
                .map(|in_skin| self.parse_gltf_skin(&in_skin, &buffers, outputs))
                .collect::<Result<_, _>>()?;

            for anim in document.animations()
            {
                self.parse_gltf_anim(anim, &buffers, outputs)?;
            }

            for gltf_node in document.nodes()
            {
                self.parse_gltf_mesh(gltf_node, &buffers, &images, &skeletons, outputs)?;
            }
        }

        Ok(())
    }
}
impl ModelBuilder
{
    fn parse_gltf_mesh(
        &self,
        in_node: gltf::Node,
        buffers: &Vec<gltf::buffer::Data>,
        images: &Vec<gltf::image::Data>,
        skeletons: &[SkelInfo],
        outputs: &mut BuildOutputs)
    -> Result<(), Box<dyn Error>>
    {
        // pass in node?
        let Some(in_mesh) = in_node.mesh() else { return Ok(()); };
        let in_skin = in_node.skin();

        log::debug!("Parsing gLTF mesh {} '{}'", in_mesh.index(), in_mesh.name().unwrap_or(""));

        let mut meshes = Vec::new();
        let mut surfaces = Vec::new();
        let mut model_bounds_aabb = AABB::MAX_MIN;

        // TODO: split up this file

        let mut vertex_layout = VertexLayout::Static;
        let maybe_skel_info = if let Some(skin) = &in_skin
        {
            vertex_layout |= VertexLayout::Skinned;
            let skel = skeletons.iter().find(|s| s.gltf_index == skin.index())
                .expect("Node has a skin not in the document skins list??");
            Some(skel)
        } else { None };

        // acts as versioning for the vertex formats
        let vertex_layout_hash =
        {
            let mut hasher = MetroHash64::new();
            VertexLayoutBuilder::from(vertex_layout).hash(&mut hasher);
            hasher.finish()
        };

        // TODO: rethink vertex parsing

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
            for pos in positions
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
                    // let iremap = |ind: [u16;4]| ind.map(|i| skel_info.remapped_bone_indices[i as usize]);

                    // todo: verify matching attrib counts?
                    let skinned_vertex = SkinnedVertex
                    {
                        indices: maybe_joints.as_mut().and_then(|j| j.next()/*.map(iremap)*/).unwrap_or([0, 0, 0, 0]),
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

                let tex_asset = outputs.add_output(AssetTypeId::Texture, |mut tex_output|
                {
                    tex.texture().name().map(|n| tex_output.set_name(n));

                    let (pixel_format, need_conv) = match tex_data.format
                    {
                        Format::R8 => (TextureFilePixelFormat::R8, false),
                        Format::R8G8 => (TextureFilePixelFormat::Rg8, false),
                        Format::R8G8B8 => (TextureFilePixelFormat::Rgba8, true),
                        Format::R8G8B8A8 => (TextureFilePixelFormat::Rgba8, false),
                        Format::R16 => todo!("R16 textures"),
                        Format::R16G16 => todo!("R16G16 textures"),
                        Format::R16G16B16 => todo!("R16G16B16 textures"),
                        Format::R16G16B16A16 => todo!("R16G16B16A16 textures"),
                        Format::R32G32B32FLOAT => todo!("R32G32B32FLOAT textures"),
                        Format::R32G32B32A32FLOAT => todo!("R32G32B32A32FLOAT textures"),
                    };

                    tex_output.serialize(&TextureFile
                    {
                        width: tex_data.width,
                        height: tex_data.height,
                        depth: 1,
                        mip_count: 1,
                        mip_offsets: Default::default(),
                        pixel_format,
                    })?;

                    // TODO: texture compression
                    if need_conv
                    {
                        match tex_data.format
                        {
                            Format::R8G8B8 =>
                            {
                                // todo: check length
                                for i in 0..(tex_data.width * tex_data.height) as usize
                                {
                                    tex_output.write_all(&tex_data.pixels[(i * 3)..((i + 1) * 3)])?;
                                    tex_output.write_all(&[u8::MAX])?;
                                }
                            }
                            _ => todo!("Other texture format conversions"),
                        }
                    } else {
                        tex_output.write_all(&tex_data.pixels)?;
                    }

                    Ok(())
                })?;

                textures.try_push(tex_asset)?;
            }

            // TODO: read material info from gltf
            let material_class = MaterialClass::SimpleOpaque; // TODO
            let material = outputs.add_output(AssetTypeId::Material, |mtl_output|
            {
                // call into MaterialBuilder?
                mtl_output.set_name(format!("{:?}", material_class));
                mtl_output.depends_on_multiple(textures.clone());

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

                Ok(())
            })?;

            let shader_compile_flags = ShaderCompileFlags::none(); // Debug

            // todo: better asset key
            let vertex_shader_key = AssetKeySynthHash::generate(ShaderHash
            {
                stage: ShaderStage::Vertex,
                material_class,
                vertex_layout_hash,
                compile_flags: shader_compile_flags,
            });
            outputs.add_synthetic(AssetTypeId::Shader, vertex_shader_key, |mut vshader_output|
            {
                log::debug!("Compiling vertex shader {:?}", vshader_output.asset_key());

                let shader_file = self.shaders_root.join(format!("SkinnedOpaque.vs.hlsl"));
                shader_file.to_str().map(|sf| vshader_output.set_name(sf));

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

                vshader_output.serialize_debug::<Shader>(&ShaderDebugData
                {
                    source_file: shader_source,
                })?;

                Ok(())
            })?;

            // todo: better asset key
            let pixel_shader_key = AssetKeySynthHash::generate(ShaderHash
            {
                stage: ShaderStage::Pixel,
                material_class,
                vertex_layout_hash,
                compile_flags: shader_compile_flags,
            });
            outputs.add_synthetic(AssetTypeId::Shader, pixel_shader_key, |mut pshader_output|
            {
                log::debug!("Compiling pixel shader {:?}", pshader_output.asset_key());

                let shader_file = self.shaders_root.join(format!("{material_class:?}.ps.hlsl"));
                shader_file.to_str().map(|sf| pshader_output.set_name(sf));

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

                pshader_output.serialize_debug::<Shader>(&ShaderDebugData
                {
                    source_file: shader_source,
                })?;

                Ok(())
            })?;
            // TODO: material depends on shaders

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

        let geometry = outputs.add_output(AssetTypeId::Geometry, |geom_output|
        {
            in_mesh.name().map(|n| geom_output.set_name(n));
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

            Ok(())
        })?;

        outputs.add_output(AssetTypeId::Model, |mut model_output|
        {
            in_node.name().map(|n| model_output.set_name(n)); // different name from mesh?
            model_output.depends_on(geometry);
            if let Some(skel_info) = maybe_skel_info { model_output.depends_on(skel_info.asset); };
            model_output.depends_on_multiple(surfaces.iter().map(|s| s.material));

            model_output.serialize(&ModelFile
            {
                geometry,
                skeleton: maybe_skel_info.map(|s| s.asset),
                surfaces: surfaces.into_boxed_slice(),
            })?;

            Ok(())
        })?;

        Ok(())
    }

    fn parse_gltf_skin(
        &self,
        in_skin: &gltf::Skin,
        buffers: &[gltf::buffer::Data],
        outputs: &mut BuildOutputs)
    -> Result<SkelInfo, Box<dyn Error>>
    {
        log::debug!("Parsing gLTF skin {} '{}'", in_skin.index(), in_skin.name().unwrap_or(""));

        // todo: enumerate nodes to determine hierarchy
        let mut bone_child_to_parent = HashMap::new(); // gltf index -> skel index
        let mut bone_names = alloc_slice_default(in_skin.joints().len());
        let mut skel_bind_poses = alloc_slice_default(in_skin.joints().len());
        for (i, joint) in in_skin.joints().enumerate()
        {
            if i > i16::MAX as usize
            {
                return Err(Box::new(ModelImportError::TooManyBones));
            }

            for child in joint.children()
            {
                let None = bone_child_to_parent.insert(child.index(), i as i16)
                    else { return Err(ModelImportError::DuplicateBoneParents.into()) };
            }
            let _parent = *bone_child_to_parent.entry(joint.index()).or_insert_with(|| -1);

            bone_names[i] = joint.name().ok_or(ModelImportError::UnnamedBones)?.to_string();

            let (trans, rot, _scale) = joint.transform().decomposed();
            skel_bind_poses[i] = DualQuat::from_rot_trans(Quat::from_array(rot), Vec3::from_array(trans));
        }

        #[derive(Hash)]
        struct BoneRelation { id: BoneId, parent_index: i16 };
        let bone_relations: Box<_> = in_skin.joints().enumerate().map(|(i, joint)|
        {
            let Some(parent_index) = bone_child_to_parent.get(&joint.index()).cloned()
                else { panic!("joint {} (#{i}) does not have a parent?", joint.index()) };
            BoneRelation
            {
                id: BoneId::from_name(&bone_names[i]),
                parent_index,
            }
        }).collect();

        // TODO: apply global transform

        let mut skel_inv_bind_pose = alloc_slice_default(bone_relations.len());
        let reader = in_skin.reader(|b| Some(&buffers[b.index()]));
        for (i, ibm) in reader.read_inverse_bind_matrices().unwrap().enumerate() // error handling?
        {
            let mtx = Mat4::from_cols_array_2d(&ibm);
            let dq = DualQuat::from(&mtx);
            skel_inv_bind_pose[i] = dq;
        }

        // TODO: this probably is not sufficient to determine uniqueness
        let skel_key = AssetKeySynthHash::generate(&bone_relations);
        let skeleton = outputs.add_synthetic(AssetTypeId::Skeleton, skel_key, |skel_output|
        {
            skel_output.serialize(&Skeleton
            {
                bone_ids: bone_relations.as_ref().iter().map(|b| b.id).collect(),
                parent_indices: bone_relations.as_ref().iter().map(|b| b.parent_index).collect(),
                bind_poses: skel_bind_poses,
                inverse_bind_poses: skel_inv_bind_pose,
            })?;
            skel_output.serialize_debug::<Skeleton>(&SkeletonDebugData { bone_names, })?;

            Ok(())
        })?;

        Ok(SkelInfo
        {
            asset: skeleton,
            gltf_index: in_skin.index(),
        })
    }

    fn parse_gltf_anim(
        &self,
        in_anim: gltf::Animation,
        buffers: &[gltf::buffer::Data],
        outputs: &mut BuildOutputs)
    -> Result<(), Box<dyn Error>>
    {
        log::debug!("Parsing gLTF animation {} '{}'", in_anim.index(), in_anim.name().unwrap_or(""));

        let mut anim_name_buf = String::new();
        let anim_name = in_anim.name().unwrap_or_else(||
        {
            anim_name_buf.clear();
            std::fmt::Write::write_fmt(&mut anim_name_buf, format_args!("animation_{}", in_anim.index())).unwrap();
            &anim_name_buf
        });

        let sample_rate = DEFAULT_ANIM_SAMPLE_RATE;
        let sample_rate_f = sample_rate.to_f32();

        #[derive(Default)]
        struct BoneData<'n>
        {
            name: Option<&'n str>,
            translations: Vec<Vec3>,
            rotations: Vec<Quat>,
        }

        fn for_each_subval<F: FnMut(f32)>(min: f32, max: f32, rate: f32, mut callback: F) -> Result<(), Box<dyn Error>>
        {
            let range = max - min;
            if range < 0.0
            {
                // todo: add more context to error
                return Err(Box::new(ModelImportError::AnimationTimesOutOfOrder));
            }

            let start = (min / rate).ceil() as u32;
            let end = (max / rate).ceil() as u32;

            for i in start..end
            {
                // todo: interoplation method
                let t = ((i as f32 * rate) - min) / range;
                callback(t);
            }

            Ok(())
        }

        let mut bone_keyframes: HashMap<usize, BoneData> = HashMap::new();
        let mut frame_count = 0;

        // todo: This can be parallelized
        for in_chan in in_anim.channels()
        {
            let ch_reader = in_chan.reader(|b| Some(&buffers[b.index()]));
            let target_node = in_chan.target().node();

            // todo: figure out if in_chan.sampler().interpolation() is necessary

            let inputs = ch_reader.read_inputs().unwrap();
            match ch_reader.read_outputs().unwrap()
            {
                ReadOutputs::Translations(read_translations) =>
                {
                    let translations = &mut bone_keyframes.entry(target_node.index())
                        .or_insert_with(|| BoneData { name: target_node.name(), .. Default::default() })
                        .translations;

                    let mut outputs = inputs.zip(read_translations).peekable();
                    let mut cur = outputs.peek().cloned().unwrap_or_default();
                    while let Some(next) = outputs.next()
                    {
                        let a = Vec3::from_array(cur.1);
                        let b = Vec3::from_array(next.1);
                        for_each_subval(cur.0, next.0, sample_rate_f, |t| translations.push(Vec3::lerp(a, b, t)))?;
                        cur = next;
                    }

                    // any frames after the last timestep will just extend the last value
                    translations.push(Vec3::from_array(cur.1));
                    frame_count = frame_count.max(translations.len());
                }
                ReadOutputs::Rotations(read_rotations) =>
                {
                    let rotations = &mut bone_keyframes.entry(target_node.index())
                        .or_insert_with(|| BoneData { name: target_node.name(), .. Default::default() })
                        .rotations;

                    let mut outputs = inputs.zip(read_rotations.into_f32()).peekable();
                    let mut cur = outputs.peek().cloned().unwrap_or_default();
                    while let Some(next) = outputs.next()
                    {
                        let a = Quat::from_array(cur.1);
                        let b = Quat::from_array(next.1);
                        for_each_subval(cur.0, next.0, sample_rate_f, |t| rotations.push(Quat::slerp(a, b, t)))?;
                        cur = next;
                    }

                    // any frames after the last timestep will just extend the last value
                    rotations.push(Quat::from_array(cur.1));
                    frame_count = frame_count.max(rotations.len());
                },
                ReadOutputs::Scales(_) => { log::warn!("Animating scale is not supported"); } // unsupported (currently)
                ReadOutputs::MorphTargetWeights(_) => {} // unsupported
            }
        }

        // TODO: need to sort bones and poses based on bones
        let mut bone_ids = alloc_slice_default(bone_keyframes.len());
        let mut poses = alloc_slice_default(bone_ids.len() * frame_count);

        for (bone, (gltf_index, bone_data)) in bone_keyframes.iter().enumerate()
        {
            let id = BoneId::from_name(bone_data.name.ok_or(ModelImportError::UnnamedBones)?);
            bone_ids[bone] = id;

            for fr in 0..frame_count
            {
                let translation = bone_data.translations.get(usize::min(fr, bone_data.translations.len() - 1))
                    .cloned().unwrap_or_default();
                let rotation = bone_data.rotations.get(usize::min(fr, bone_data.rotations.len() - 1))
                    .cloned().unwrap_or_default();
                poses[fr * bone_ids.len() + bone] = DualQuat::from_rot_trans(rotation, translation);
            }
        }
        
        outputs.add_output(AssetTypeId::SkeletalAnimation, |anim_output|
        {
            in_anim.name().map(|n| anim_output.set_name(n));
            anim_output.serialize(&SkeletalAnimation
            {
                sample_rate,
                frame_count: AnimFrameNumber(frame_count as u32),
                bones: bone_ids,
                poses,
            })?;

            Ok(())
        })?;

        // todo: output sorted based on hash of bone name

        Ok(())
    }
}

struct SkelInfo
{
    asset: AssetKey,
    gltf_index: usize,
}