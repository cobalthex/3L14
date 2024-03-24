use nameof::name_of_type;
use wgpu::{BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferBindingType, ShaderStages, TextureSampleType, TextureViewDimension};
use wgpu::core::resource::TextureViewNotRenderableReason::Dimension;
use crate::engine::graphics::colors::Color;

// todo: in the future these should be data driven (maybe parametric?)

pub trait Material<'a>
{
    fn bind_layout() -> &'a wgpu::BindGroupLayoutDescriptor<'a>;
}

// todo: uniforms should be in their own section

pub struct OpaquePbrMaterial
{
    pub albedo_map: Option<wgpu::Texture>,
    pub albedo_color: Color,
    pub metalness: f32,
    pub roughness: f32,
}
impl<'a> Material<'a> for OpaquePbrMaterial
{
    fn bind_layout() -> &'a BindGroupLayoutDescriptor<'a>
    {
        &BindGroupLayoutDescriptor
        {
            label: Some(name_of_type!(Self)),
            entries:
            &[
                BindGroupLayoutEntry
                {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture
                    {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry
                {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer
                    {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        }
    }
}