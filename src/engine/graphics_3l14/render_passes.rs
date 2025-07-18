use wgpu::*;
use super::renderer::*;
use super::colors;

pub fn scene<'f>(
    render_frame: &'f RenderFrame,
    encoder: &'f mut CommandEncoder,
    clear_color: Option<colors::Rgba>) -> RenderPass<'f>
{
    let (view, resolve_target) = match &render_frame.msaa_config
    {
        Some(msaa) => (&msaa.buffer, Some(&render_frame.back_buffer_view)),
        None => (&render_frame.back_buffer_view, None)
    };

    encoder.begin_render_pass(&RenderPassDescriptor
    {
        label: Some("Scene render pass"),
        color_attachments: &[Some(
            RenderPassColorAttachment
            {
                // todo: optimize
                view,
                depth_slice: None,
                resolve_target,
                ops: Operations
                {
                    load: clear_color.map_or(LoadOp::Load, |c| LoadOp::Clear(c.to_srgb().into())),
                    store: StoreOp::Store,
                },
            },
        )],
        depth_stencil_attachment: Some(RenderPassDepthStencilAttachment
        {
            view: &render_frame.depth_buffer_view,
            depth_ops: Some(Operations { load: LoadOp::Clear(1.0), store: StoreOp::Store }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
    })
}

pub fn debug<'f>(
    render_frame: &'f RenderFrame,
    encoder: &'f mut CommandEncoder) -> RenderPass<'f>
{
    let (view, resolve_target) = match &render_frame.msaa_config
    {
        Some(msaa) => (&msaa.buffer, Some(&render_frame.back_buffer_view)),
        None => (&render_frame.back_buffer_view, None)
    };

    encoder.begin_render_pass(&RenderPassDescriptor
    {
        label: Some("Debug render pass"),
        color_attachments: &[Some(
            RenderPassColorAttachment
            {
                view,
                resolve_target,
                depth_slice: None,
                ops: Operations
                {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            },
        )],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    })
}
