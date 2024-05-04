use std::path::PathBuf;
use std::sync::Arc;
use std::thread::current;
use arc_swap::ArcSwapOption;
use egui::epaint::Shadow;
use egui::{Pos2, Rect, Rounding, Stroke, Visuals};
use egui_wgpu::ScreenDescriptor;
use parking_lot::RwLock;
use sdl2::video::Window;
#[allow(deprecated)]
use wgpu::rwh::{HasRawDisplayHandle, HasRawWindowHandle};
use wgpu::*;
use crate::engine::FrameNumber;

pub const MAX_CONSEC_FRAMES: usize = 3;

struct RenderFrameData
{
    last_submission: Option<SubmissionIndex>,
    depth_buffer: Texture,
}

// TODO: need a way to hold the reference for the duration of the frame
//  ArcSwap inside RenderFarmeData + deref?
pub struct MSAAConfiguration
{
    pub current_sample_count: u32, // pipelines and ms-framebuffer will need to be recreated if this is changed
    pub buffer: TextureView,
}

pub struct Renderer<'r>
{
    device: Device,
    queue: Queue,

    surface: Surface<'r>,

    surface_config: RwLock<SurfaceConfiguration>,

    max_sample_count: u32,
    msaa_config: ArcSwapOption<MSAAConfiguration>,

    debug_gui: egui::Context,
    debug_gui_renderer: egui_wgpu::Renderer,

    // todo: this needs to know when a new frame is available before picking one
    render_frames: RwLock<[RenderFrameData; MAX_CONSEC_FRAMES]>,
}
impl<'r> Renderer<'r>
{
    pub async fn new(window: &Window) -> Renderer
    {
        puffin::profile_function!();

        let allow_msaa = true; // should be in some settings somewhere

        let instance = Instance::new(InstanceDescriptor
        {
            backends: Backends::PRIMARY,
            flags: InstanceFlags::from_build_config(),
            dx12_shader_compiler: Dx12Compiler::Dxc
            {
                dxc_path: Some(PathBuf::from(r"3rdparty/dxc/dxc.exe")),
                dxil_path: Some(PathBuf::from(r"3rdparty/dxc/dxil.dll")),
            },
            gles_minor_version: Gles3MinorVersion::default(),
        });

        #[allow(deprecated)]
        let handle = SurfaceTargetUnsafe::RawHandle
        {
            raw_display_handle: window.raw_display_handle().expect("Failed to get display handle"),
            raw_window_handle: window.raw_window_handle().expect("Failed to get window handle"),
        };
        let surface = unsafe { instance.create_surface_unsafe(handle).expect("Failed to create swap-chain") };

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
        println!("Creating render device with {:?}", adapter_info);

        // Create the logical device and command queue
        let (device, queue) = adapter
            .request_device(&DeviceDescriptor
                {
                    label: Some("Primary WGPU device"),
                    required_features:
                        Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES |
                        Features::PUSH_CONSTANTS,
                    // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                    required_limits: Limits
                    {
                        max_push_constant_size: 256, // doesn't support WebGPU
                        .. Default::default()
                    }.using_resolution(adapter.limits()),
                },
                None,
            )
            .await
            .expect("Failed to create device");

        let window_size = window.size();
        let surface_config = surface
            .get_default_config(&adapter, window_size.0, window_size.1)
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
        let msaa_config = if
            allow_msaa &&
            current_sample_count > 1
        {
            Some(MSAAConfiguration
            {
                current_sample_count,
                buffer: Self::create_msaa_buffer(current_sample_count, &device, surface_config.width, surface_config.height, surface_config.format)
            })
        }
        else
        {
            None
        };

        let debug_gui = egui::Context::default();
        debug_gui.set_visuals(Visuals
        {
            window_shadow: Shadow::NONE,
            window_stroke: Stroke::new(1.0, egui::Color32::BLACK),
            window_fill: egui::Color32::from_rgba_unmultiplied(24, 24, 24, 240),
            window_rounding: Rounding::same(4.0),
            .. Visuals::dark()
        });
        let debug_gui_renderer = egui_wgpu::Renderer::new(&device, surface_config.format, None, 1);

        // todo: recreate on resize
        let depth_buffer_desc = TextureDescriptor
        {
            label: Some("Depth buffer"),
            size: Extent3d
            {
                width: surface_config.width,
                height: surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: current_sample_count,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };
        let render_frames = std::array::from_fn::<_, MAX_CONSEC_FRAMES, _>(|_|
        {
            RenderFrameData
            {
                last_submission: None,
                depth_buffer: device.create_texture(&depth_buffer_desc),
            }
        });

        Renderer
        {
            device,
            queue,
            surface,
            surface_config: RwLock::new(surface_config),
            max_sample_count,
            msaa_config: ArcSwapOption::from_pointee(msaa_config),
            debug_gui,
            debug_gui_renderer,
            render_frames: RwLock::new(render_frames),
        }
    }

    pub fn resize(&self, new_width: u32, new_height: u32)
    {
        puffin::profile_function!();

        if new_width == 0 || new_height == 0
        {
            panic!("Render width/height cannot be zero");
        }

        let surface_format;
        {
            let mut surf_config = self.surface_config.write();
            surface_format = surf_config.format;

            println!("Resizing renderer from {}x{} to {}x{}",
                     surf_config.width, surf_config.height,
                     new_width, new_height);

            surf_config.width = new_width;
            surf_config.height = new_height;
            self.surface.configure(&self.device, &surf_config);
        }

        let mut current_sample_count = 1;
        // max_sample_count may need to be recreated
        self.msaa_config.store(self.msaa_config.load().as_ref().map(|c|
        {
            current_sample_count = c.current_sample_count;
            Arc::new(MSAAConfiguration
            {
                current_sample_count,
                buffer: Self::create_msaa_buffer(current_sample_count, &self.device, new_width, new_height, surface_format),
            })
        }));

        let depth_buffer_desc = TextureDescriptor
        {
            label: Some("Depth buffer"),
            size: Extent3d
            {
                width: new_width,
                height: new_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: current_sample_count,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };

        {
            let mut render_frames = self.render_frames.write();
            for f in render_frames.as_mut()
            {
                f.depth_buffer = self.device.create_texture(&depth_buffer_desc);
                f.last_submission = None;
            }
        }
    }

    pub fn device(&self) -> &Device { &self.device }
    pub fn queue(&self) -> &Queue { &self.queue }

    pub fn display_size(&self) -> glam::UVec2
    {
        let surf_conf = self.surface_config.read();
        glam::UVec2::new(surf_conf.width, surf_conf.height)
    }
    pub fn display_aspect_ratio(&self) -> f32
    {
        let surf_conf = self.surface_config.read();
        (surf_conf.width as f32) / (surf_conf.height as f32)
    }
    pub fn surface_format(&self) -> TextureFormat { self.surface_config.read().format }

    pub fn msaa_max_sample_count(&self) -> u32 { self.max_sample_count }

    pub fn frame(&'r self, frame_number: FrameNumber, input: &crate::engine::input::Input) -> RenderFrame
    {
        puffin::profile_function!();

        let back_buffer;
        let depth_buffer_view;

        {
            puffin::profile_scope!("Wait for frame ready");
            back_buffer = match self.surface.get_current_texture()
            {
                Ok(texture) => texture,
                Err(SurfaceError::Timeout) => self.surface.get_current_texture().expect("Get swap chain target timed out"),
                Err(SurfaceError::Outdated | SurfaceError::Lost | SurfaceError::OutOfMemory) =>
                    {
                        let surf_conf = self.surface_config.read();
                        self.surface.configure(&self.device, &surf_conf);
                        self.surface.get_current_texture().expect("Failed to get swap chain target")
                    }
            };

            let render_frames = self.render_frames.read();

            let rf_data = &render_frames[frame_number.0 as usize % render_frames.len()];
            match &rf_data.last_submission
            {
                Some(i) => { self.device.poll(Maintain::WaitForSubmissionIndex(i.clone())); }, // not web-gpu compatible
                None => {},
            };
            depth_buffer_view = rf_data.depth_buffer.create_view(&TextureViewDescriptor::default());
        }

        let back_buffer_view = back_buffer.texture.create_view(&TextureViewDescriptor::default());

        let debug_gui = self.debug_gui.clone();
        let mut raw_input: egui::RawInput = input.into();
        // todo: raw_input. max_texture_size, time, focused
        raw_input.screen_rect = Some(Rect::from_min_max(
            Pos2::ZERO, Pos2::new(back_buffer.texture.width() as f32, back_buffer.texture.height() as f32)));
        debug_gui.begin_frame(raw_input);

        RenderFrame
        {
            frame_number,
            back_buffer,
            back_buffer_view,
            depth_buffer_view,
            msaa_config: self.msaa_config.load_full(),
            debug_gui,
        }
    }

    fn render_debug_gui(&self, clip_size: [u32; 2], target: &TextureView) -> CommandBuffer
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

    pub fn present(&self, frame: RenderFrame)
    {
        puffin::profile_function!();

        let back_buffer_size = frame.back_buffer.texture.size();
        let gui_commands = self.render_debug_gui(
            [back_buffer_size.width, back_buffer_size.height],
            &frame.back_buffer_view);
        self.queue.submit([gui_commands]);
        frame.back_buffer.present();
    }

    // taken from MSAA line sample
    fn create_msaa_buffer(sample_count: u32, device: &wgpu::Device, width: u32, height: u32, surface_format: TextureFormat) -> wgpu::TextureView
    {
        let multisampled_texture_extent = wgpu::Extent3d
        {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let multisampled_frame_descriptor = &wgpu::TextureDescriptor {
            label: Some("MSAA framebuffer texture"),
            size: multisampled_texture_extent,
            mip_level_count: 1,
            sample_count,
            dimension: TextureDimension::D2,
            format: surface_format,
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
    pub frame_number: FrameNumber,

    back_buffer: SurfaceTexture,
    pub back_buffer_view: TextureView,
    pub depth_buffer_view: TextureView,

    pub msaa_config: Option<Arc<MSAAConfiguration>>,

    pub debug_gui: egui::Context,
}
impl RenderFrame
{
}
