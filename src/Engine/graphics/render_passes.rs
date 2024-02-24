use wgpu::*;
use super::renderer::*;
use super::colors;

pub fn test(frame: &mut RenderFrame, clear_color: Option<colors::Color>) -> RenderPass
{
    frame.encoder.begin_render_pass(&RenderPassDescriptor
    {
        label: Some("Scene render pass"),
        color_attachments: &[Some(
            RenderPassColorAttachment
            {
                view: &frame.back_buffer_view,
                resolve_target: None,
            ops: Operations
                {
                    load: clear_color.map_or(LoadOp::Load, |c| LoadOp::Clear(c.to_srgb().into())),
                    store: StoreOp::Store,
                },
            },
        )],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    })
}

pub fn debug_gui(frame: &mut RenderFrame) -> RenderPass
{
    frame.encoder.begin_render_pass(&RenderPassDescriptor
    {
        label: Some("egui render pass"),
        color_attachments: &[Some(
            RenderPassColorAttachment
            {
                view: &frame.back_buffer_view,
                resolve_target: None,
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