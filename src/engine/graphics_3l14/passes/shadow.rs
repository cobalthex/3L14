use triomphe::Arc;
use wgpu::{TextureDescriptor, TextureDimension, TextureFormat, TextureUsages};
use wgpu::wgt::TextureViewDescriptor;
use crate::{debug_label, Renderer};

pub struct ShadowPass
{
    depth: wgpu::TextureView,
}
impl ShadowPass
{
    pub fn new(renderer: Arc<Renderer>) -> Self
    {
        let depth_tex = renderer.device().create_texture(&TextureDescriptor
        {
            label: debug_label!("Shadow pass depth texture"),
            size: Default::default(),
            mip_level_count: 0,
            sample_count: 0,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[TextureFormat::R8Unorm],
        });

        Self
        {
            depth: depth_tex.create_view(&TextureViewDescriptor::default()),
        }
    }
}