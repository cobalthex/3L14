use egui::epaint::Shadow;
use egui::{Rounding, Stroke, Visuals};
use egui_wgpu::ScreenDescriptor;
use sdl2::video::Window;
#[allow(deprecated)]
use wgpu::rwh::{HasRawDisplayHandle, HasRawWindowHandle};
use wgpu::*;
use crate::engine::FrameNumber;
use super::render_passes;

pub struct RenderPipelines
{

}

pub struct Renderer<'r>
{
    device: Device,
    queue: Queue,

    surface: Surface<'r>,
    surface_config: SurfaceConfiguration,

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
            flags: InstanceFlags::default(),
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
                    label: None,
                    required_features: Features::empty(),
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
    }

    pub fn device(&self) -> &Device { &self.device }
    pub fn device_mut(&mut self) -> &mut Device { &mut self.device }
    pub fn queue(&self) -> &Queue { &self.queue }

    pub fn surface_config(&self) -> &SurfaceConfiguration { &self.surface_config }
    pub fn display_size(&self) -> glam::UVec2 { glam::UVec2::new(self.surface_config.width, self.surface_config.height) }
    pub fn display_aspect_ratio(&self) -> f32 { (self.surface_config.width as f32) / (self.surface_config.height as f32) }

    pub fn frame(&self, frame_number: FrameNumber, input: &crate::engine::input::Input) -> RenderFrame
    {
        let back_buffer = self.surface.get_current_texture().unwrap();
        let back_buffer_view = back_buffer.texture.create_view(&TextureViewDescriptor::default());

        let debug_gui = self.debug_gui.clone();
        let raw_input = input.into();
        // todo: parallel render support
        debug_gui.begin_frame(raw_input);

        RenderFrame
        {
            frame_number,
            encoder: self.device.create_command_encoder(&CommandEncoderDescriptor
            {
                label: Some("frame encoder"),
            }),
            back_buffer,
            back_buffer_view,
            debug_gui,
        }
    }
}

pub struct RenderFrame
{
    frame_number: FrameNumber,
    back_buffer: SurfaceTexture,

    pub encoder: CommandEncoder,
    pub back_buffer_view: TextureView,
    pub debug_gui: egui::Context,
}
impl RenderFrame
{
    pub fn frame_number(&self) -> FrameNumber { self.frame_number }

    fn present_gui(&mut self, renderer: &mut Renderer)
    {
        let output = self.debug_gui.end_frame();
        output.textures_delta.set.iter().for_each(|td|
            renderer.debug_gui_renderer.update_texture(&renderer.device, &renderer.queue, td.0, &td.1));
        output.textures_delta.free.iter().for_each(|t|
            renderer.debug_gui_renderer.free_texture(t));

        let back_buffer_size = self.back_buffer.texture.size();
        let desc = ScreenDescriptor { size_in_pixels: [back_buffer_size.width, back_buffer_size.height], pixels_per_point: output.pixels_per_point };

        let primitives = self.debug_gui.tessellate(output.shapes, output.pixels_per_point);
        renderer.debug_gui_renderer.update_buffers(
            &renderer.device,
            &renderer.queue,
            &mut self.encoder,
            primitives.as_slice(),
            &desc);

        let mut debug_gui_render_pass = render_passes::debug_gui(self);
        renderer.debug_gui_renderer.render(&mut debug_gui_render_pass, &primitives, &desc);
    }

    pub fn present(mut self, renderer: &mut Renderer)
    {
        self.present_gui(renderer);
        renderer.queue.submit(Some(self.encoder.finish()));
        self.back_buffer.present();
    }
}
