use crate::{const_assert, debug_label};
use crate::engine::containers::{ObjectPool, ObjectPoolEntryGuard};
use crate::engine::graphics::Renderer;
use crate::engine::world::{CameraUniform, TransformUniform};
use crate::engine::ShortTypeName;
use std::sync::Arc;
use wgpu::{BindGroupDescriptor, BindGroupLayout, BufferAddress, BufferDescriptor, BufferSize, BufferUsages, QueueWriteBufferView};

pub struct UniformBufferEntry
{
    pub count: u32,
    pub buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

pub struct UniformsPool
{
    cameras: ObjectPool<UniformBufferEntry>,
    transforms: ObjectPool<UniformBufferEntry>,
}
impl UniformsPool
{
    pub fn new(renderer: Arc<Renderer>) -> Self
    {
        let max_ubo_size = renderer.device().limits().max_uniform_buffer_binding_size as usize;

        Self
        {
            cameras: ObjectPool::new(Self::create_object_pool::<CameraUniform>(renderer.clone(), size_of::<CameraUniform>())),
            transforms: ObjectPool::new(Self::create_object_pool::<TransformUniform>(renderer.clone(), max_ubo_size)),
        }
    }

    fn create_object_pool<T: 'static>(renderer: Arc<Renderer>, max_buffer_size: usize, bind_group_layout: &BindGroupLayout) -> impl Fn(usize) -> UniformBufferEntry
    {
        assert!(!std::mem::needs_drop::<T>());

        let count = max_buffer_size / size_of::<T>();
        move |_|
        {
            let buffer = renderer.device().create_buffer(&BufferDescriptor
            {
                label: debug_label!(&format!("{} x {} uniform buffer (pooled)", T::short_type_name(), count)),
                size: (count * size_of::<T>()) as BufferAddress,
                usage: BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let bind_group = renderer.device().create_bind_group(&BindGroupDescriptor
            {
                label: debug_label!(&format!("{} x {} uniform bind group (pooled)", T::short_type_name(), count)),
                layout: bind_group_layout,
                entries: &[],
            });

            UniformBufferEntry
            {
                count: count as u32,
                buffer,
                bind_group: bind_group,
            }
        }
    }

    pub fn take_camera(&self) -> ObjectPoolEntryGuard<'_, UniformBufferEntry>
    {
        self.cameras.take()
    }

    pub fn take_transforms(&self) -> ObjectPoolEntryGuard<'_, UniformBufferEntry>
    {
        self.transforms.take()
    }
}

pub type UniformsPoolEntryGuard<'a> = ObjectPoolEntryGuard<'a, UniformBufferEntry>;

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

// todo: better name, impl for &[u8] ?
pub trait WriteTyped
{
    fn write_typed<T>(&mut self, index: usize, value: T);
}

impl<'p> WriteTyped for QueueWriteBufferView<'p>
{
    fn write_typed<T>(&mut self, index: usize, value: T)
    {
        unsafe
        {
            let ptr = self.as_mut_ptr() as *mut T;
            std::ptr::write_unaligned(ptr.add(index), value);
        }
    }
}