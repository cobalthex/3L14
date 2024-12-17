use crate::engine::asset::Asset;
use crate::engine::graphics::assets::Model;
use crate::engine::world::{Camera, TransformUniform, ViewMtx};
use glam::{Mat4, Vec4Swizzles};
use std::sync::Arc;
use wgpu::RenderPass;
use crate::engine::graphics::pipeline_cache::{DebugMode, PipelineCache};
use crate::engine::graphics::{pipeline_sorter, Renderer};
use crate::engine::graphics::pipeline_sorter::PipelineSorter;
use crate::engine::graphics::uniforms_pool::{UniformsPool, UniformsPoolEntryGuard, WgpuBufferWriter};
use crate::engine::write_index;

const MAX_ENTRIES_IN_WORLD_BUF: usize = 64;

// TODO: This needs to exist until the frame has been submitted fully
pub struct View<'f>
{
    renderer: &'f Renderer,
    pipeline_cache: &'f PipelineCache,
    uniforms_pool: &'f UniformsPool,
    debug_mode: DebugMode,
    camera_view: ViewMtx,
    sorter: PipelineSorter,
    used_transforms: Vec<UniformsPoolEntryGuard<'f>>
    // translucent_pass: TranslucentPass,
}
impl<'f> View<'f>
{
    pub fn new(renderer: &'f Renderer, camera: &Camera, pipeline_cache: &'f PipelineCache, uniforms_pool: &'f UniformsPool) -> Self
    {
        Self
        {
            renderer,
            pipeline_cache,
            uniforms_pool,
            debug_mode: DebugMode::None, // todo
            camera_view: camera.view(),
            sorter: PipelineSorter::default(),
            used_transforms: Vec::new(),
        }
    }

    pub fn draw(&mut self, object_transform: Mat4, model: Arc<Model>) -> bool
    {
        // todo: use closest OBB point instead of center?
        let depth = self.camera_view.0.transform_vector3(object_transform.w_axis.xyz()).z;

        // this may be heavy-handed
        if !model.all_dependencies_loaded()
        {
            return false;
        }

        let mut uniforms = self.uniforms_pool.take_transforms();
        let mut next_uniform = 0;
        let mut uniforms_writer = uniforms.write(self.renderer.queue());

        let geo = model.geometry.payload().unwrap();
        for mesh_index in 0..model.mesh_count
        {
            if next_uniform >= self.used_transforms.len()
            {
                drop(uniforms_writer);
                let mut swap_uniforms = self.uniforms_pool.take_transforms();
                std::mem::swap(&mut uniforms, &mut swap_uniforms);
                self.used_transforms.push(swap_uniforms);
                uniforms_writer = uniforms.write(self.renderer.queue());

                next_uniform = 0;
            }

            // todo: one per model?
            write_index(&mut uniforms_writer, next_uniform, TransformUniform
            {
                world: object_transform,
            });
            let uniform_id = next_uniform as u32;

            let (mtl, vsh, psh) =
            {
                let surf = &model.surfaces[mesh_index as usize];
                (
                    surf.material.payload().unwrap(),
                    surf.vertex_shader.payload().unwrap(),
                    surf.pixel_shader.payload().unwrap(),
                )
            };

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
                vshader: vsh,
                pshader: psh,
            });
        }

        true
    }

    // TODO: compute lights influence
    // TODO: instancing

    pub fn submit(&mut self, render_pass: &mut RenderPass)
    {
        puffin::profile_scope!("View submission");

        for (pipeline_hash, draws) in self.sorter.sort()
        {
            if !self.pipeline_cache.try_apply(render_pass, pipeline_hash)
            {
                panic!("can this happen?");
            }

            for draw in draws
            {
                let mesh = &draw.geometry.meshes[draw.mesh_index as usize];

                render_pass.set_vertex_buffer(0, mesh.vertices.slice(0..));
                render_pass.set_index_buffer(mesh.indices.slice(0..), mesh.index_format);
                render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }
    }
}
