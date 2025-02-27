use debug_3l14::debug_gui::DebugGui;
use crate::{debug_label, Renderer};
use std::sync::Arc;
use egui::Ui;
use wgpu::{BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BufferAddress, BufferBindingType, BufferDescriptor, BufferSize, BufferUsages, QueueWriteBufferView, RenderPass, ShaderStages};
use containers_3l14::{ObjectPool, ObjectPoolEntryGuard};
use math_3l14::TransformUniform;
use nab_3l14::utils::ShortTypeName;
use crate::camera::CameraUniform;

pub struct UniformBufferEntry
{
    pub entry_size: u32,
    pub count: u32,
    pub buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

pub struct UniformsPool
{
    renderer: Arc<Renderer>,
    max_ubo_size: usize,
    cameras: ObjectPool<UniformBufferEntry>,
    transforms: ObjectPool<UniformBufferEntry>,

    pub camera_bind_layout: BindGroupLayout,
    pub transform_bind_layout: BindGroupLayout,
}
impl UniformsPool
{
    #[must_use]
    pub fn new(renderer: Arc<Renderer>) -> Self
    {
        let max_ubo_size = renderer.device().limits().max_uniform_buffer_binding_size as usize;

        let camera_bind_layout = renderer.device().create_bind_group_layout(&BindGroupLayoutDescriptor
        {
            entries:
            &[
                wgpu::BindGroupLayoutEntry
                {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer
                    {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                    count: None,
                }
            ],
            label: debug_label!("Camera vsh bind layout"),
        });

        let transform_bind_layout = renderer.device().create_bind_group_layout(&BindGroupLayoutDescriptor
        {
            entries:
            &[
                BindGroupLayoutEntry
                {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer
                    {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                    count: None,
                }
            ],
            label: debug_label!("World transform vsh bind layout"),
        });

        Self
        {
            max_ubo_size,
            cameras: ObjectPool::default(),
            transforms: ObjectPool::default(),
            renderer,
            camera_bind_layout,
            transform_bind_layout,
        }
    }

    #[must_use]
    fn create_pool_entry<T: 'static>(&self, bind_group_layout: &BindGroupLayout, max_count: Option<usize>) -> UniformBufferEntry
    {
        assert!(!std::mem::needs_drop::<T>());

        let count = max_count.unwrap_or_else(|| self.max_ubo_size / size_of::<T>());
        let buffer = self.renderer.device().create_buffer(&BufferDescriptor
        {
            label: debug_label!(&format!("{} x {} uniform buffer (pooled)", T::short_type_name(), count)),
            size: (count * size_of::<T>()) as BufferAddress,
            usage: BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = self.renderer.device().create_bind_group(&BindGroupDescriptor
        {
            label: debug_label!(&format!("[{}; {}] uniform bind group (pooled)", T::short_type_name(), count)),
            layout: bind_group_layout,
            entries: &[BindGroupEntry
            {
                binding: 0,
                resource: BindingResource::Buffer(wgpu::BufferBinding
                {
                    buffer: &buffer,
                    offset: 0,
                    size: Some(unsafe { BufferSize::new_unchecked(size_of::<T>() as u64) }),
                }),
            }],
        });

        UniformBufferEntry
        {
            entry_size: size_of::<T>() as u32,
            count: count as u32,
            buffer,
            bind_group,
        }
    }

    #[inline] #[must_use]
    pub fn take_camera(&self) -> ObjectPoolEntryGuard<'_, UniformBufferEntry>
    {
        // re-evaluate max count here?
        self.cameras.take(|_| self.create_pool_entry::<CameraUniform>(&self.camera_bind_layout, Some(2)))
    }

    #[inline] #[must_use]
    pub fn take_transforms(&self) -> ObjectPoolEntryGuard<'_, UniformBufferEntry>
    {
        self.transforms.take(|_| self.create_pool_entry::<TransformUniform>(&self.transform_bind_layout, None))
    }
}
impl DebugGui for UniformsPool
{
    fn name(&self) -> &str { "Uniforms" }

    fn debug_gui(&self, ui: &mut Ui)
    {
        ui.label(format!("Max UBO size: {}", self.max_ubo_size));
        ui.label(format!("Cameras: {} free, {} total", self.cameras.free_count(), self.cameras.total_count()));
        ui.label(format!("Transforms: {} free, {} total", self.transforms.free_count(), self.transforms.total_count()));
    }
}

pub type UniformsPoolEntryGuard<'a> = ObjectPoolEntryGuard<'a, UniformBufferEntry>;

pub trait WgpuBufferWriter<'q>
{
    fn write(&'q self, queue: &'q wgpu::Queue) -> QueueWriteBufferView<'q>;

    fn bind(&self, render_pass: &mut RenderPass, bind_index: u32, buffer_index: u8);
}
impl<'p> WgpuBufferWriter<'p> for UniformsPoolEntryGuard<'p>
{
    fn write(&'p self, queue: &'p wgpu::Queue) -> QueueWriteBufferView<'p>
    {
        let buf_size = unsafe { BufferSize::new_unchecked(self.buffer.size()) };
        queue.write_buffer_with(&self.buffer, 0, buf_size).unwrap()
    }

    #[inline]
    fn bind(&self, render_pass: &mut RenderPass, bind_index: u32, buffer_index: u8)
    {
        let offset = buffer_index as u32 * self.entry_size;
        render_pass.set_bind_group(bind_index, &self.bind_group, &[offset]);
    }
}

// todo: better name, impl for &[u8] ?
pub trait WriteTyped
{
    fn write_typed<T>(&mut self, index: usize, value: T);
}

impl<'p> WriteTyped for QueueWriteBufferView<'p>
{
    #[inline]
    fn write_typed<T>(&mut self, index: usize, value: T)
    {
        unsafe
        {
            let ptr = self.as_mut_ptr() as *mut T;
            std::ptr::write_unaligned(ptr.add(index), value);
        }
    }
}