use wgpu::{BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, Buffer, BufferDescriptor, BufferUsages, IndexFormat, Queue, RenderPass};
use nab_3l14::utils::{AsU8Slice, ShortTypeName};
use crate::{debug_label, Renderer};

pub struct DynamicGeo<TVertex>
{
    pub vertices: Vec<TVertex>,
    pub indices: Vec<u32>,
    vbuffer: Buffer,
    vbuffer_binding: BindGroup,
    ibuffer: Buffer,
}
impl<TVertex> DynamicGeo<TVertex>
{
    const MAX_VERTICES: u64 = 1024;
    const MAX_INDICES: u64 = Self::MAX_VERTICES * 3;

    pub fn new(renderer: &Renderer, layout: &BindGroupLayout) -> Self
    {
        let vbuffer = renderer.device().create_buffer(&BufferDescriptor
        {
            label: debug_label!(&format!("{} vertices", TVertex::short_type_name())),
            size: size_of::<TVertex>() as u64 * Self::MAX_VERTICES,
            usage: BufferUsages::COPY_DST | BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let ibuffer = renderer.device().create_buffer(&BufferDescriptor
        {
            label: debug_label!(&format!("{} indices", TVertex::short_type_name())),
            size: size_of::<u32>() as u64 * Self::MAX_INDICES,
            usage: BufferUsages::COPY_DST | BufferUsages::INDEX,
            mapped_at_creation: false,
        });

        let vbuffer_binding = renderer.device().create_bind_group(&BindGroupDescriptor
        {
            label: debug_label!(&format!("{} vertices binding", TVertex::short_type_name())),
            layout,
            entries: &[BindGroupEntry
            {
                binding: 0,
                resource: vbuffer.as_entire_binding(),
            }],
        });

        Self
        {
            vertices: Vec::new(), // with cap?
            indices: Vec::new(), // with cap?
            vbuffer,
            vbuffer_binding,
            ibuffer,
        }
    }

    pub fn begin(&mut self)
    {
        self.vertices.clear();
        self.indices.clear();
    }

    // Submit the geo, assumes the pipeline is already set
    pub fn submit(&self, queue: &Queue, render_pass: &mut RenderPass)
    {
        let num_indices = self.indices.len() as u32;
        if num_indices <= 0 { return; }

        let vb_slice = unsafe { self.vertices.as_u8_slice() };
        let ib_slice = unsafe { self.indices.as_u8_slice() };

        // TODO: write verts/indices directly to buffer
        queue.write_buffer(&self.vbuffer, 0, vb_slice);
        queue.write_buffer(&self.ibuffer, 0, ib_slice);

        // render_pass.set_vertex_buffer(0, self.vbuffer.slice(0..(vb_slice.len() as u64)));
        render_pass.set_bind_group(0, &self.vbuffer_binding, &[]);
        render_pass.set_index_buffer(self.ibuffer.slice(0..(ib_slice.len() as u64)), IndexFormat::Uint32);
        render_pass.draw_indexed(0..num_indices, 0, 0..1);
    }
}
