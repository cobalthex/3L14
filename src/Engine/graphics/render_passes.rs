use wgpu::*;
use super::renderer::*;
use super::colors;

pub fn test<'f>(
    renderer: &'f Renderer,
    target: &'f TextureView,
    depth_stencil: &'f TextureView,
    encoder: &'f mut CommandEncoder,
    clear_color: Option<colors::Color>) -> RenderPass<'f>
{
    encoder.begin_render_pass(&RenderPassDescriptor
    {
        label: Some("Scene render pass"),
        color_attachments: &[Some(
            RenderPassColorAttachment
            {
                // todo: optimize
                view: renderer.msaa_buffer().unwrap_or(target),
                resolve_target: renderer.msaa_buffer().map(|_| target),
                ops: Operations
                {
                    load: clear_color.map_or(LoadOp::Load, |c| LoadOp::Clear(c.to_srgb().into())),
                    store: StoreOp::Store,
                },
            },
        )],
        depth_stencil_attachment: Some(RenderPassDepthStencilAttachment
        {
            view: depth_stencil,
            depth_ops: Some(Operations { load: LoadOp::Clear(1.0), store: StoreOp::Store }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
    })
}
