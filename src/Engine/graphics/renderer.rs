use egui::epaint::Shadow;
use egui::{Rounding, Stroke, Visuals};
use egui_wgpu::ScreenDescriptor;
use sdl2::video::Window;
#[allow(deprecated)]
use wgpu::rwh::{HasRawDisplayHandle, HasRawWindowHandle};
use wgpu::*;
use crate::engine::FrameNumber;

pub struct RenderPipelines
{

}

pub struct Renderer<'r>
{
    device: Device,
    queue: Queue,

    surface: Surface<'r>,
    surface_config: SurfaceConfiguration,

    max_sample_count: u32,
    current_sample_count: u32, // pipelines and ms-framebuffer will need to be recreated if this is changed
    msaa_buffer: TextureView,

    debug_gui: egui::Context,
    debug_gui_renderer: egui_wgpu::Renderer,
}
impl<'r> Renderer<'r>
{
    pub async fn new(window: &Window) -> Renderer
    {
        let instance = Instance::new(InstanceDescriptor
        {
            backends: Backends::PRIMARY,
            flags: InstanceFlags::DEBUG,
            dx12_shader_compiler: Default::default(), // todo
            gles_minor_version: Default::default(),
        });

        #[allow(deprecated)]
        let handle = SurfaceTargetUnsafe::RawHandle
        {
            raw_display_handle: window.raw_display_handle().unwrap(),
            raw_window_handle: window.raw_window_handle().unwrap(),
        };
        let surface = unsafe { instance.create_surface_unsafe(handle).unwrap() };

        // enumerate adapters?
        let adapter = instance
            .request_adapter(&RequestAdapterOptions
            {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                // Request an adapter which can render to our surface
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let adapter_info = adapter.get_info();
        println!("Selected adapter: {:?}", adapter_info);

        // Create the logical device and command queue
        let (device, queue) = adapter
            .request_device(&DeviceDescriptor
                {
                    label: Some("Primary WGPU device"),
                    required_features:
                        Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES |
                        Features::PUSH_CONSTANTS,
                    // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                    required_limits: Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                },
                None,
            )
            .await
            .expect("Failed to create device");

        let wnd_size = window.size();
        let surface_config = surface
            .get_default_config(&adapter, wnd_size.0, wnd_size.1)
            .unwrap();
        surface.configure(&device, &surface_config);

        let sample_flags = adapter
            .get_texture_format_features(surface_config.format)
            .flags;
        let max_sample_count: u32 =
        {
            if sample_flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X16) { 16 }
            else if sample_flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X8) { 8 }
            else if sample_flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4) { 4 }
            else if sample_flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X2) { 2 }
            else { 1 }
        };

        let current_sample_count = max_sample_count;
        // don't create if only one sample?
        let msaa_buffer = Self::create_msaa_buffer(current_sample_count, &device, &surface_config);

        let debug_gui = egui::Context::default();
        debug_gui.set_visuals(Visuals
        {
            window_shadow: Shadow::NONE,
            window_stroke: Stroke::new(1.0, egui::Color32::BLACK),
            window_rounding: Rounding::same(4.0),
            .. Visuals::dark()
        });
        let debug_gui_renderer = egui_wgpu::Renderer::new(&device, surface_config.format, None, 1);

        Renderer
        {
            device,
            queue,
            surface,
            surface_config,
            max_sample_count,
            current_sample_count,
            msaa_buffer,
            debug_gui,
            debug_gui_renderer,
        }
    }

    pub fn resize(&mut self, new_width: u32, new_height: u32)
    {
        if new_width == 0 || new_height == 0
        {
            panic!("Render width/height cannot be zero");
        }

        self.surface_config.width = new_width;
        self.surface_config.height = new_height;
        self.surface.configure(&self.device, &self.surface_config);
        // max_sample_count may need to becreated
        self.msaa_buffer = Self::create_msaa_buffer(self.current_sample_count, &self.device, &self.surface_config);
    }

    pub fn device(&self) -> &Device { &self.device }
    pub fn queue(&self) -> &Queue { &self.queue }

    pub fn surface_config(&self) -> &SurfaceConfiguration { &self.surface_config }
    pub fn display_size(&self) -> glam::UVec2 { glam::UVec2::new(self.surface_config.width, self.surface_config.height) }
    pub fn display_aspect_ratio(&self) -> f32 { (self.surface_config.width as f32) / (self.surface_config.height as f32) }

    pub fn max_sample_count(&self) -> u32 { self.max_sample_count }
    pub fn current_sample_count(&self) -> u32 { self.current_sample_count }
    pub fn msaa_buffer(&self) -> &TextureView { &self.msaa_buffer }

    pub fn frame(&'r self, frame_number: FrameNumber, input: &crate::engine::input::Input) -> RenderFrame
    {
        let back_buffer = self.surface.get_current_texture().expect("Failed to get swap chain target");
        let back_buffer_view = back_buffer.texture.create_view(&TextureViewDescriptor::default());

        let debug_gui = self.debug_gui.clone();
        let raw_input = input.into();
        // todo: parallel render support
        debug_gui.begin_frame(raw_input);

        RenderFrame
        {
            frame_number,
            back_buffer,
            back_buffer_view,
            debug_gui,
        }
    }

    fn present_debug_gui(&mut self, clip_size: [u32; 2], target: &wgpu::TextureView) -> CommandBuffer
    {
        let output = self.debug_gui.end_frame();
        output.textures_delta.set.iter().for_each(|td|
            self.debug_gui_renderer.update_texture(&self.device, &self.queue, td.0, &td.1));
        output.textures_delta.free.iter().for_each(|t|
            self.debug_gui_renderer.free_texture(t));

        let desc = ScreenDescriptor
        {
            size_in_pixels: clip_size,
            pixels_per_point: output.pixels_per_point
        };

        let primitives = self.debug_gui.tessellate(output.shapes, output.pixels_per_point);

        let mut command_encoder = self.device.create_command_encoder(&CommandEncoderDescriptor
        {
            label: Some("Debug GUI encoder"),
        });
        self.debug_gui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut command_encoder,
            primitives.as_slice(),
            &desc);

        {
            let mut render_pass = command_encoder.begin_render_pass(&RenderPassDescriptor
            {
                label: Some("egui render pass"),
                color_attachments: &[Some(
                    RenderPassColorAttachment
                    {
                        view: target,
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
            });
            self.debug_gui_renderer.render(&mut render_pass, &primitives, &desc);
        }
        command_encoder.finish()
    }

    pub fn present(&mut self, frame: RenderFrame)
    {
        let back_buffer_size =frame.back_buffer.texture.size();
        let gui_commands = self.present_debug_gui(
            [back_buffer_size.width, back_buffer_size.height],
            &frame.back_buffer_view);
        self.queue.submit([gui_commands]);
        frame.back_buffer.present();
    }

    // taken from MSAA line sample
    fn create_msaa_buffer(sample_count: u32, device: &wgpu::Device, surface_config: &SurfaceConfiguration) -> wgpu::TextureView
    {
        let multisampled_texture_extent = wgpu::Extent3d
        {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };
        let multisampled_frame_descriptor = &wgpu::TextureDescriptor {
            label: Some("MSAA framebuffer texture"),
            size: multisampled_texture_extent,
            mip_level_count: 1,
            sample_count,
            dimension: TextureDimension::D2,
            format: surface_config.format,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };

        device
            .create_texture(multisampled_frame_descriptor)
            .create_view(&TextureViewDescriptor
            {
                label: Some("MSAA framebuffer view"),
                .. Default::default()
            })
    }
}

pub struct RenderFrame
{
    frame_number: FrameNumber,
    back_buffer: SurfaceTexture,

    pub back_buffer_view: TextureView,
    pub debug_gui: egui::Context,
}
impl RenderFrame
{
    pub fn frame_number(&self) -> FrameNumber { self.frame_number }
}
