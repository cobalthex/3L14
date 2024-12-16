use crate::debug_label;
use crate::engine::containers::{ObjectPool, ObjectPoolEntryGuard};
use crate::engine::graphics::Renderer;
use crate::engine::world::TransformUniform;
use crate::engine::ShortTypeName;
use std::sync::Arc;
use wgpu::{BufferAddress, BufferDescriptor, BufferSize, BufferUsages, QueueWriteBufferView};

pub struct UniformsPool
{
    transforms: ObjectPool<UniformBufferEntry>,
}
impl UniformsPool
{
    pub fn new(renderer: Arc<Renderer>) -> Self
    {
        let max_buffer_size = renderer.device().limits().max_uniform_buffer_binding_size as usize;

        Self
        {
            transforms: ObjectPool::new(Self::create_object_pool::<TransformUniform>(renderer.clone(), max_buffer_size)),
        }
    }

    fn create_object_pool<T: 'static>(renderer: Arc<Renderer>, max_buffer_size: usize) -> impl Fn(usize) -> UniformBufferEntry
    {
        let count = max_buffer_size / size_of::<T>();
        move |_| UniformBufferEntry
        {
            count: count as u32,
            buffer: renderer.device().create_buffer(&BufferDescriptor
            {
                label: debug_label!(&format!("{} x {} uniforms buffer", T::short_type_name(), count)),
                size: (count * size_of::<T>()) as BufferAddress,
                usage: BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        }
    }

    pub fn take_transforms(&self) -> ObjectPoolEntryGuard<'_, UniformBufferEntry>
    {
        self.transforms.take()
    }
}

pub struct UniformBufferEntry
{
    pub count: u32,
    pub buffer: wgpu::Buffer,
}

pub type UniformsPoolEntryGuard<'p> = ObjectPoolEntryGuard<'p, UniformBufferEntry>;

pub trait WgpuBufferWriter<'q>
{
    fn write(&'q self, queue: &'q wgpu::Queue) -> QueueWriteBufferView<'q>;
}

impl<'p> WgpuBufferWriter<'p> for UniformsPoolEntryGuard<'p>
{
    fn write(&'p self, queue: &'p wgpu::Queue) -> QueueWriteBufferView<'p>
    {
        let buf_size = unsafe { BufferSize::new_unchecked(self.buffer.size()) };
        queue.write_buffer_with(&self.buffer, 0, buf_size).unwrap()
    }
}