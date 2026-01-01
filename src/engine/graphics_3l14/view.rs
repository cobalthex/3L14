use crate::pipeline_sorter::PipelineSorter;
use crate::{debug_label, pipeline_sorter, render_passes, Renderer};
use arrayvec::ArrayVec;
use glam::{Mat3, Mat4, Vec2, Vec3, Vec4Swizzles};
use triomphe::Arc;
use std::time::Duration;
use wgpu::{BindGroupDescriptor, BindGroupEntry, BindingResource, Extent3d, QueueWriteBufferView, RenderPass, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView};
use wgpu::util::{DeviceExt, TextureDataOrder};
use asset_3l14::{Asset, AssetPayload};
use math_3l14::{Affine3, CanSee, DualQuat, Frustum, IsOnOrInside, Sphere, StaticGeoUniform};
use nab_3l14::debug_panic;
use crate::assets::{Geometry, Model, MAX_SKINNED_BONES};
use crate::camera::{Camera, CameraProjection, CameraUniform};
use crate::pipeline_cache::{DebugMode, PipelineCache};
use crate::uniforms_pool::{UniformsPoolEntryGuard, WgpuBufferWriter, BufferWrite};

struct CurrentUniformsWriter
{
    renderer: Arc<Renderer>,
    writer: QueueWriteBufferView,
    next_slot: usize,
}

#[derive(Default)]
struct CameraClip
{
    eye: Vec3,
    forward: Vec3,
    right: Vec3,
    up: Vec3,
    near_clip: f32,
    far_clip: f32,
    depth_scalar: f32,
    sphere_scalar: Vec2,
    aspect_ratio: f32,
}
impl CameraClip
{
    pub fn new(camera: &Camera) -> Self
    {
        let t = camera.transform();

        let (half_fov, aspect_ratio) = match camera.projection()
        {
            CameraProjection::Perspective { fov, aspect_ratio } => (fov.to_radians() / 2.0, *aspect_ratio),
            CameraProjection::Orthographic { left, top, right, bottom } =>
            {
                // perspective: W = depth * tan(fov_x /2), H = depth * tan(fov_y / 2)
                // ortho: W = (right - left) / 2, H = (bottom - top) / 2
                todo!()
            }
        };

        let depth_scalar = f32::tan(half_fov);
        let scale_x =
        {
            let q = depth_scalar * aspect_ratio;
            f32::sqrt(q * q + 1.0) // equivalent to 1.0 / cos(atan(depth_scalar * ratio))
        };

        Self
        {
            eye: t.position,
            forward: t.forward(),
            right: t.right(),
            up: t.up(),
            near_clip: camera.near_clip(),
            far_clip: camera.far_clip(),
            depth_scalar,
            sphere_scalar: Vec2::new(scale_x, 1.0 / f32::cos(half_fov)),
            aspect_ratio,
        }
    }
}
impl CanSee<Vec3> for CameraClip
{
    #[inline]
    fn can_see(&self, pos: Vec3) -> bool { self.can_see(Sphere::new(pos, 0.0)) }
}
impl CanSee<Sphere> for CameraClip
{
    // TODO: does this work for orthographic?
    fn can_see(&self, other: Sphere) -> bool
    {
        let v = other.center() - self.eye;
        let r = other.radius();
        // TODO: simd (3x3 row matrix of r,u,f * v)
        let z = v.dot(self.forward);
        if z < self.near_clip - r || z > self.far_clip + r
        {
            return false;
        }

        let zd = z * self.depth_scalar;

        {
            let y = v.dot(self.up);
            let rdy = self.sphere_scalar.y * r;
            if y.abs() > zd + rdy
            {
                return false;
            }
        }

        {
            let x = v.dot(self.right);
            let rdx = self.sphere_scalar.x * r;
            let hdist = zd * self.aspect_ratio;
            if x.abs() > hdist + rdx
            {
                return false;
            }
        }

        true
    }
}

// TODO: This needs to exist until the frame has been submitted fully
pub struct View<'f>
{
    // TODO: move debug_draw into here?
    renderer: Arc<Renderer>,
    pipeline_cache: &'f PipelineCache,
    runtime: Duration,
    debug_mode: DebugMode,
    camera_mtx: Mat4,
    camera_pos: Vec3,
    camera_clip: CameraClip,
    sorter: PipelineSorter,
    used_uniforms_pools: Vec<UniformsPoolEntryGuard<'f>>,
    // current_txfms_writer: CurrentUniformsWriter<'f>,

    placeholder_texture: Texture,
    placeholder_texture_view: TextureView,
}
impl<'f> View<'f>
{
    #[must_use]
    pub fn new(renderer: Arc<Renderer>, pipeline_cache: &'f PipelineCache) -> Self
    {
        let used_uniforms = vec![pipeline_cache.uniforms.take_transforms()];
        // let rc = renderer.clone();
        // let current_txfms_writer = CurrentUniformsWriter
        // {
        //     writer: used_uniforms[0].write(rc.queue()),
        //     renderer: rc,
        //     next_slot: 0,
        // };

        let placeholder_texture = renderer.device().create_texture_with_data(renderer.queue(), &TextureDescriptor
        {
            label: Some("Placeholder texture"),
            size: Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8Unorm,
            usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
            view_formats: &[TextureFormat::R8Unorm],
        }, TextureDataOrder::LayerMajor, &[255u8]);
        let placeholder_texture_view = placeholder_texture.create_view(&Default::default());

        Self
        {
            pipeline_cache,
            runtime: Duration::new(0, 0),
            debug_mode: DebugMode::None,
            camera_mtx: Mat4::IDENTITY,
            camera_pos: Vec3::ZERO,
            camera_clip: CameraClip::default(),
            sorter: PipelineSorter::default(),
            used_uniforms_pools: used_uniforms,
            // current_txfms_writer,
            renderer,
            placeholder_texture,
            placeholder_texture_view,
        }
    }

    pub fn begin(&mut self, runtime: Duration, camera: &Camera, clip_camera: &Camera, debug_mode: DebugMode)
    {
        self.runtime = runtime;
        self.debug_mode = debug_mode;
        self.camera_mtx = camera.matrix();
        self.camera_pos = camera.transform().position;
        self.camera_clip = CameraClip::new(clip_camera);
        self.sorter.clear();
        self.used_uniforms_pools.clear();
    }

    // TODO: compute lights influence
    // TODO: instancing

    pub fn submit(&mut self, render_pass: &mut RenderPass)
    {
        puffin::profile_scope!("View submission");

        let camera = self.pipeline_cache.uniforms.take_camera();
        {
            let mut camera_writer = camera.record(self.renderer.queue());
            // TODO: view/proj order may be arch dependent?
            camera_writer.write_type(0, CameraUniform::new(self.camera_mtx, self.runtime));
        }

        for (pipeline_hash, draws) in self.sorter.sort()
        {
            if !self.pipeline_cache.try_apply(render_pass, pipeline_hash)
            {
                panic!("can this happen?");
            }

            for draw in draws
            {
                let mesh = &draw.geometry.meshes[draw.mesh_index as usize];

                camera.bind(render_pass, 0, 0);
                self.used_uniforms_pools[(draw.transform_uniform_id >> 8) as usize].bind(render_pass, 1, draw.transform_uniform_id as u8);
                if let Some(poses_uniform_id) = draw.poses_uniform_id
                {
                    self.used_uniforms_pools[(poses_uniform_id >> 8) as usize].bind(render_pass, 2, poses_uniform_id as u8);
                }

                // TODO: don't create on the fly
                let mut bge = ArrayVec::<_, 18>::new();
                bge.push(BindGroupEntry
                {
                    binding: bge.len() as u32,
                    resource: draw.material.props.as_entire_binding(),
                });
                bge.push(BindGroupEntry
                {
                    binding: bge.len() as u32,
                    resource: BindingResource::Sampler(self.pipeline_cache.default_sampler())
                });
                for tex in &draw.textures
                {
                    bge.push(BindGroupEntry
                    {
                        binding: bge.len() as u32,
                        resource: BindingResource::TextureView(&tex.gpu_view),
                    })
                }
                // TODO: TEMP HACK
                if draw.material.textures.is_empty()
                {
                    bge.push(BindGroupEntry
                    {
                        binding: bge.len() as u32,
                        resource: BindingResource::TextureView(&self.placeholder_texture_view),
                    })
                }


                let mtl_bind_group = self.renderer.device().create_bind_group(&BindGroupDescriptor
                {
                    label: debug_label!("TODO mtl bind group"),
                    layout: &draw.material.bind_layout,
                    entries: &bge,
                });
                render_pass.set_bind_group(3, &mtl_bind_group, &[]);

                // bind sub-buffers?
                render_pass.set_vertex_buffer(0, draw.geometry.vertices.slice(..));
                render_pass.set_index_buffer(draw.geometry.indices.slice(..), draw.geometry.index_format);
                render_pass.draw_indexed(mesh.index_range.0..mesh.index_range.1, mesh.vertex_range.0 as i32, 0..1);
            }
        }

        self.used_uniforms_pools.push(camera);
    }

    fn can_see(&self, geo: &Geometry, object_transform: Mat4) -> bool
    {
        let geo_transform = geo.bounds_sphere.transform(&(object_transform));
        self.camera_clip.can_see(geo_transform)
    }

    fn draw_model_common(&mut self, model: Arc<Model>, world_transform: Mat4, poses_uniforms: Option<u32>) -> bool
    {
        // this may be heavy-handed
        if !model.all_dependencies_loaded()
        {
            return false;
        }

        let geo = model.geometry.payload().unwrap();
        if !self.can_see(&geo, world_transform) { return false; }

        // TODO: these should be per-mesh
        let rad = world_transform.x_axis.x.max(world_transform.y_axis.y.max(world_transform.z_axis.z));
        let depth = world_transform.w_axis.xyz().distance_squared(self.camera_pos) - (rad * rad);

        // todo: these need to reuse between draw calls
        let mut txfm_uniforms = self.pipeline_cache.uniforms.take_transforms();
        let mut next_txfm_uniform = 0;
        let mut txfm_uniforms_writer = txfm_uniforms.record(self.renderer.queue());
        let txfm_uniform_id =
        {
            if next_txfm_uniform >= txfm_uniforms.count as usize
            {
                drop(txfm_uniforms_writer);
                let mut swap_uniforms = self.pipeline_cache.uniforms.take_transforms();
                std::mem::swap(&mut txfm_uniforms, &mut swap_uniforms);
                self.used_uniforms_pools.push(swap_uniforms);
                txfm_uniforms_writer = txfm_uniforms.record(self.renderer.queue());

                next_txfm_uniform = 0;
            }
            txfm_uniforms_writer.write_type(next_txfm_uniform, StaticGeoUniform
            {
                world: world_transform,
            });
            let uniform_id = ((self.used_uniforms_pools.len() << 8) + next_txfm_uniform) as u32; // todo: ensure bits are enough
            next_txfm_uniform += 1;
            uniform_id
        };

        for mesh_index in 0..model.mesh_count
        {
            let (mtl, vsh, psh) =
            {
                let surf = &model.surfaces[mesh_index as usize];
                (
                    surf.material.payload().unwrap(),
                    surf.vertex_shader.payload().unwrap(),
                    surf.pixel_shader.payload().unwrap(),
                )
            };

            let textures = mtl.textures.iter().map(|t| t.payload().unwrap()).collect();

            let pipeline_hash = self.pipeline_cache.get_or_create(
                &geo,
                &mtl,
                &vsh,
                &psh,
                self.debug_mode);

            self.sorter.push(pipeline_sorter::Draw
            {
                transform: world_transform,
                depth,
                mesh_index,
                transform_uniform_id: txfm_uniform_id,
                poses_uniform_id: poses_uniforms,
                pipeline_hash,
                geometry: geo.clone(),
                material: mtl,
                textures,
                vshader: vsh,
                pshader: psh,
            });
        }

        drop(txfm_uniforms_writer);
        self.used_uniforms_pools.push(txfm_uniforms);
        true
    }

    pub fn draw_model_static(&mut self, model: Arc<Model>, world_transform: Mat4) -> bool
    {
        self.draw_model_common(model, world_transform, None)
    }

    pub fn draw_model_skinned(&mut self, model: Arc<Model>, world_transform: Mat4, poses: &[DualQuat]) -> bool
    {
        // TODO: this needs to pass vis-checks first

        // todo: cleanup/standardize this logic
        let mut poses_uniforms = self.pipeline_cache.uniforms.take_poses();
        {
            let mut poses_uniforms_writer = poses_uniforms.record(self.renderer.queue());
            poses_uniforms_writer.write_slice(0, poses);
        }

        let poses_uniform_id = (self.used_uniforms_pools.len() << 8) as u32; // todo: ensure bits are enough
        self.used_uniforms_pools.push(poses_uniforms);

        self.draw_model_common(model, world_transform, Some(poses_uniform_id))
    }
}