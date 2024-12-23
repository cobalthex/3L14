use std::ops::Mul;
use crate::engine::asset::Asset;
use crate::engine::graphics::assets::Model;
use crate::engine::graphics::pipeline_cache::{DebugMode, PipelineCache};
use crate::engine::graphics::pipeline_sorter::PipelineSorter;
use crate::engine::graphics::uniforms_pool::{UniformsPool, UniformsPoolEntryGuard, WgpuBufferWriter, WriteTyped};
use crate::engine::graphics::{pipeline_sorter, Renderer};
use crate::engine::world::{Camera, CameraUniform, TransformUniform, ViewMtx};
use glam::{Mat4, Vec4Swizzles};
use std::sync::Arc;
use std::time::Duration;
use arrayvec::ArrayVec;
use wgpu::{BindGroupDescriptor, BindGroupEntry, BindingResource, RenderPass};
use crate::debug_label;

// TODO: This needs to exist until the frame has been submitted fully
pub struct View<'f>
{
    runtime: Duration,
    renderer: &'f Renderer,
    pipeline_cache: &'f PipelineCache,
    debug_mode: DebugMode,
    camera: &'f Camera,
    sorter: PipelineSorter,
    used_transforms: Vec<UniformsPoolEntryGuard<'f>>
    // translucent_pass: TranslucentPass,
}

impl<'f> View<'f>
{
    pub fn new(
        runtime: Duration,
        renderer: &'f Renderer,
        camera: &'f Camera,
        pipeline_cache: &'f PipelineCache) -> Self
    {
        Self
        {
            runtime,
            renderer,
            pipeline_cache,
            debug_mode: DebugMode::None, // todo
            camera,
            sorter: PipelineSorter::default(),
            used_transforms: Vec::new(),
        }
    }

    pub fn draw(&mut self, object_transform: Mat4, model: Arc<Model>) -> bool
    {
        // todo: use closest OBB point instead of center?
        let depth = self.camera.view().0.transform_vector3(object_transform.w_axis.xyz()).z;

        // this may be heavy-handed
        if !model.all_dependencies_loaded()
        {
            return false;
        }

        // todo: these need to reuse between draw calls
        let mut uniforms = self.pipeline_cache.uniforms.take_transforms();
        let mut next_uniform = 0;
        let mut uniforms_writer = uniforms.write(self.renderer.queue());

        let geo = model.geometry.payload().unwrap();
        for mesh_index in 0..model.mesh_count
        {
            if next_uniform >= self.used_transforms.len()
            {
                drop(uniforms_writer);
                let mut swap_uniforms = self.pipeline_cache.uniforms.take_transforms();
                std::mem::swap(&mut uniforms, &mut swap_uniforms);
                self.used_transforms.push(swap_uniforms);
                uniforms_writer = uniforms.write(self.renderer.queue());

                next_uniform = 0;
            }

            // todo: one per model?
            uniforms_writer.write_typed(next_uniform, TransformUniform
            {
                world: object_transform,
            });
            let uniform_id = (((self.used_transforms.len() - 1) << 8) + next_uniform) as u32; // todo: ensure bits are enough
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
                &geo.meshes[mesh_index as usize],
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
        self.used_transforms.push(uniforms);

        true
    }

    // TODO: compute lights influence
    // TODO: instancing

    pub fn submit(&mut self, render_pass: &mut RenderPass)
    {
        puffin::profile_scope!("View submission");

        let camera = self.pipeline_cache.uniforms.take_camera();
        {
            let mut camera_writer = camera.write(self.renderer.queue());
            camera_writer.write_typed(0, CameraUniform::new(self.camera, self.runtime));
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
                self.used_transforms[(draw.uniform_id >> 8) as usize].bind(render_pass, 1, draw.uniform_id as u8);

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

                render_pass.set_vertex_buffer(0, mesh.vertices.slice(0..));
                render_pass.set_index_buffer(mesh.indices.slice(0..), mesh.index_format);
                render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }
    }
}
