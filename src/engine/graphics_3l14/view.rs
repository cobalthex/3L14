use crate::pipeline_sorter::PipelineSorter;
use crate::{debug_label, pipeline_sorter, render_passes, Renderer};
use arrayvec::ArrayVec;
use glam::{Mat3, Mat4, Vec2, Vec3, Vec4Swizzles};
use std::sync::Arc;
use std::time::Duration;
use wgpu::{BindGroupDescriptor, BindGroupEntry, BindingResource, QueueWriteBufferView, RenderPass};
use asset_3l14::Asset;
use math_3l14::{Affine3, CanSee, Frustum, IsOnOrInside, Sphere, TransformUniform};
use crate::assets::Model;
use crate::camera::{Camera, CameraProjection, CameraUniform};
use crate::pipeline_cache::{DebugMode, PipelineCache};
use crate::uniforms_pool::{UniformsPoolEntryGuard, WgpuBufferWriter, WriteTyped};

struct CurrentUniformsWriter<'f>
{
    renderer: Arc<Renderer>,
    writer: QueueWriteBufferView<'f>,
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
            CameraProjection::Perspective { fov, aspect_ratio } => (fov.0 / 2.0, *aspect_ratio),
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
            let mut camera_writer = camera.write(self.renderer.queue());
            // TODO: view/proj order may be arch dependent?
            camera_writer.write_typed(0, CameraUniform::new(self.camera_mtx, self.runtime));
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
                self.used_uniforms_pools[(draw.uniform_id >> 8) as usize].bind(render_pass, 1, draw.uniform_id as u8);

                // TODO: don't create on the fly
                let mut bge = ArrayVec::<_, 18>::new();
                bge.push(BindGroupEntry
                {
                    binding: bge.len() as u32,
                    resource: draw.material.props.as_entire_binding(),
                });
                if !draw.material.textures.is_empty()
                {
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
                }
                let mtl_bind_group = self.renderer.device().create_bind_group(&BindGroupDescriptor
                {
                    label: debug_label!("TODO mtl bind group"),
                    layout: &draw.material.bind_layout,
                    entries: &bge,
                });
                render_pass.set_bind_group(2, &mtl_bind_group, &[]);

                // bind sub-buffers?
                render_pass.set_vertex_buffer(0, draw.geometry.vertices.slice(..));
                render_pass.set_index_buffer(draw.geometry.indices.slice(..), draw.geometry.index_format);
                render_pass.draw_indexed(mesh.index_range.0..mesh.index_range.1, mesh.vertex_range.0 as i32, 0..1);
            }
        }

        self.used_uniforms_pools.push(camera);
    }

    pub fn draw_model_static(&mut self, model: Arc<Model>, object_transform: Mat4) -> bool
    {
        // this may be heavy-handed
        if !model.all_dependencies_loaded()
        {
            return false;
        }

        let geo = model.geometry.payload().unwrap();
        let geo_transform = geo.bounds_sphere.transform(&(object_transform));
        if !self.camera_clip.can_see(geo_transform)
        {
            return false;
        }

        // TODO: verify this math all works
        let rad = object_transform.x_axis.x.max(object_transform.y_axis.y.max(object_transform.z_axis.z));
        let depth = object_transform.w_axis.xyz().distance_squared(self.camera_pos) - (rad * rad);

        // todo: these need to reuse between draw calls
        let mut uniforms = self.pipeline_cache.uniforms.take_transforms();
        let mut next_uniform = 0;
        let mut uniforms_writer = uniforms.write(self.renderer.queue());

        for mesh_index in 0..model.mesh_count
        {
            if next_uniform >= uniforms.count as usize
            {
                drop(uniforms_writer);
                let mut swap_uniforms = self.pipeline_cache.uniforms.take_transforms();
                std::mem::swap(&mut uniforms, &mut swap_uniforms);
                self.used_uniforms_pools.push(swap_uniforms);
                uniforms_writer = uniforms.write(self.renderer.queue());

                next_uniform = 0;
            }

            // todo: one per model?
            uniforms_writer.write_typed(next_uniform, TransformUniform
            {
                world: object_transform,
            });
            let uniform_id = ((self.used_uniforms_pools.len() << 8) + next_uniform) as u32; // todo: ensure bits are enough
            next_uniform += 1;

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
                transform: object_transform,
                depth,
                mesh_index,
                uniform_id,
                pipeline_hash,
                geometry: geo.clone(),
                material: mtl,
                textures,
                vshader: vsh,
                pshader: psh,
            });
        }

        drop(uniforms_writer);
        self.used_uniforms_pools.push(uniforms);
        true
    }

    fn draw_model_skinned(&mut self, model: Arc<Model>, object_transform: Mat4, pose: &[Affine3]) -> bool
    {
        todo!()
    }
}