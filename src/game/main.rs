use std::env;
use std::io::Read;
use egui::{pos2};
use sdl2::event::{Event as SdlEvent, WindowEvent as SdlWindowEvent};
use game_3l14::engine::{*, timing::*, input::*, windows::*, graphics::*, world::*, assets::*};
use clap::Parser;
use futures::executor;
use glam::{Quat, Vec3};
use wgpu::*;
use game_3l14::engine::async_completion::AsyncCompletion;

#[derive(Debug, Parser)]
struct CliArgs
{
    #[cfg(debug_assertions)]
    #[arg(long, default_value_t = false)]
    keep_alive_on_panic: bool,
}

fn shitty_join<I>(separator: &str, iter: I) -> String
     where I: Iterator,
           I::Item: std::fmt::Display
{
    let mut out = String::new();
    let mut first = true;
    for i in iter
    {
        match first
        {
            true => { first = false; }
            false => { out.push_str(separator); }
        };
        out.push_str(i.to_string().as_str());
    }
    out
}

fn main()
{
    println!("Started 3L14 at {} with args {}", chrono::Local::now(), shitty_join(" ", env::args()));

    let cli_args = CliArgs::parse();

    #[cfg(debug_assertions)]
    if cli_args.keep_alive_on_panic
    {
        use std::io::{stdin};
        let default_panic_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic|
        {
            default_panic_hook(panic);
            println!("!!! Press enter to exit !!!");
            let _ = stdin().read(&mut [0u8]); // wait to exit
        }));
    }

    let mut clock = Clock::new();

    let assets = AssetCache::default();

    let sdl = sdl2::init().unwrap();
    let mut sdl_events = sdl.event_pump().unwrap();
    let sdl_video = sdl.video().unwrap();

    // windows
    let windows = Windows::new(&sdl_video);
    let mut input = Input::new(&sdl);

    let mut display_app_stats = true;

    // let mut tp_builder = futures::executor::ThreadPoolBuilder::new();
    // let thread_pool = tp_builder.create().unwrap();

    let mut renderer = futures::executor::block_on(Renderer::new(windows.main_window())); // don't block?

    let min_frame_time = std::time::Duration::from_secs_f32(1.0 / 150.0);

    let test_scene = Scene::try_from_file("assets/pawn.glb", renderer.device()).expect("Couldn't import scene");

    let mut camera = Camera::new(renderer.display_aspect_ratio());
    camera.transform.position = Vec3::new(0.0, 2.0, -10.0);
    camera.update_view();

    const MAX_ENTRIES_IN_WORLD_BUF: usize = 64;
    let foo = std::mem::size_of::<TransformUniform>();
    let world_uform_buf = renderer.device().create_buffer(&BufferDescriptor
    {
        label: Some("World uniform buffer"),
        size: (std::mem::size_of::<TransformUniform>() * MAX_ENTRIES_IN_WORLD_BUF) as BufferAddress,
        usage: BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let world_bind_group_layout = renderer.device().create_bind_group_layout(&BindGroupLayoutDescriptor
    {
        entries:
        &[
            wgpu::BindGroupLayoutEntry
            {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer
                {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: None,
                },
                count: None,
            }
        ],
        label: Some("World bind group layout"),
    });
    let world_bind_group = renderer.device().create_bind_group(&wgpu::BindGroupDescriptor
    {
        layout: &world_bind_group_layout,
        entries:
        &[
            wgpu::BindGroupEntry
            {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding
                {
                    buffer: &world_uform_buf,
                    offset: 0,
                    size: Some(unsafe { BufferSize::new_unchecked(std::mem::size_of::<TransformUniform>() as u64) }),
                })
            }
        ],
        label: Some("World bind group"),
    });

    let cam_uform_buf = renderer.device().create_buffer(&BufferDescriptor
    {
        label: Some("Camera uniform buffer"),
        size: std::mem::size_of::<CameraUniform>() as BufferAddress,
        usage: BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let cam_bind_group_layout = renderer.device().create_bind_group_layout(&BindGroupLayoutDescriptor
    {
        entries:
        &[
            wgpu::BindGroupLayoutEntry
            {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer
                {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }
        ],
        label: Some("Camera bind group layout"),
    });
    let cam_bind_group = renderer.device().create_bind_group(&wgpu::BindGroupDescriptor
    {
        layout: &cam_bind_group_layout,
        entries:
        &[
            wgpu::BindGroupEntry
            {
                binding: 0,
                resource: cam_uform_buf.as_entire_binding(),
            }
        ],
        label: Some("Camera bind group"),
    });

    let test_pipeline = test_render_pipeline::new(&renderer, &cam_bind_group_layout, &world_bind_group_layout);

    let mut worlds_buf: [TransformUniform; MAX_ENTRIES_IN_WORLD_BUF] = array_init::array_init(|_| TransformUniform::default());

    let mut frame_number = FrameNumber(0);
    'main: loop
    {
        let mut completion = CompletionState::InProgress;

        frame_number.increment();
        let frame_start_time = clock.tick(); // todo: cap fps

        input.pre_update();

        for event in sdl_events.poll_iter()
        {
            match event
            {
                SdlEvent::Quit {..} =>
                {
                    completion |= CompletionState::Completed;
                },
                // SizeChanged?
                SdlEvent::Window { win_event: SdlWindowEvent::Resized(w, h), .. } =>
                {
                    // todo
                    // renderer.resize(w as u32, h as u32);
                },

                _ => input.handle_event(event, frame_start_time.current_time),
            }
        }

        #[cfg(debug_assertions)]
        if input.keyboard().is_key_down(KeyCode::Q) &&
           input.keyboard().has_keymod(KeyMods::CTRL)
        {
            completion = CompletionState::Completed;
        }

        if (input.keyboard().is_key_press(KeyCode::Backquote))
        {
            input.mouse().set_capture(ToggleState::Toggle);
        }

        let speed = if input.keyboard().has_keymod(KeyMods::SHIFT) { 8.0 } else { 2.0 } * frame_start_time.delta_time.as_secs_f32();
        if input.keyboard().is_key_down(KeyCode::W)
        {
            camera.transform.position += camera.transform.forward() * speed;
        }
        if input.keyboard().is_key_down(KeyCode::A)
        {
            camera.transform.position += camera.transform.left() * speed;
        }
        if input.keyboard().is_key_down(KeyCode::S)
        {
            camera.transform.position += camera.transform.backward() * speed;
        }
        if input.keyboard().is_key_down(KeyCode::D)
        {
            camera.transform.position += camera.transform.right() * speed;
        }
        if input.keyboard().is_key_down(KeyCode::E)
        {
            camera.transform.position += camera.transform.up() * speed;
        }
        if input.keyboard().is_key_down(KeyCode::Q)
        {
            camera.transform.position += camera.transform.down() * speed;
        }
        if input.mouse().is_captured()
        {
            const MOUSE_SCALE: f32 = 0.01;
            let yaw = -input.mouse().delta.x as f32 * MOUSE_SCALE; // left to right
            let pitch = -input.mouse().delta.y as f32 * MOUSE_SCALE; // down to up

            camera.transform.rotation = Quat::normalize(
                Quat::from_axis_angle(WORLD_RIGHT, pitch) *
                camera.transform.rotation *
                Quat::from_axis_angle(WORLD_UP, yaw));
        }
        camera.update_view();

        if input.keyboard().is_key_press(KeyCode::F1)
        {
            display_app_stats = !display_app_stats
        }

        // if let Some(delta) = min_frame_time.checked_sub(frame_start_time.delta_time)
        // {
        //     sleep(delta)
        // }

        let cam_uform = CameraUniform::from(&camera);
        renderer.queue().write_buffer(&cam_uform_buf, 0, unsafe { [cam_uform].as_u8_slice() });

        let mut render_frame = renderer.frame(frame_number, &input);
        {
            let mut encoder = renderer.device().create_command_encoder(&CommandEncoderDescriptor::default());
            {
                let mut test_pass = render_passes::test(
                    &renderer,
                    &render_frame.back_buffer_view,
                    &mut encoder,
                    Some(colors::CORNFLOWER_BLUE));

                test_pass.set_pipeline(&test_pipeline);
                test_pass.set_bind_group(0, &cam_bind_group, &[]);

                let mut world_index = 0;
                for model in test_scene.models.iter()
                {
                    // todo: use DrawIndirect?
                    let world_transform = model.transform.to_world();
                    worlds_buf[world_index].world = world_transform;
                    let offset = (world_index * std::mem::size_of::<TransformUniform>()) as u32;
                    test_pass.set_bind_group(1, &world_bind_group, &[offset]);
                    world_index += 1;

                    for mesh in model.object.meshes()
                    {
                        test_pass.set_vertex_buffer(0, mesh.vertices());
                        test_pass.set_index_buffer(mesh.indices(), mesh.index_format());

                        test_pass.draw_indexed(mesh.index_range(),0,0..1);
                    }

                    if world_index >= MAX_ENTRIES_IN_WORLD_BUF
                    {
                        world_uform_buf.unmap();
                        world_index = 0;
                        break; // testing
                    }
                }
            }
            // todo: only update what was written to
            renderer.queue().write_buffer(&world_uform_buf, 0, unsafe { worlds_buf.as_u8_slice() });
            renderer.queue().submit([encoder.finish()]);

            egui::Window::new("App Stats")
                .open(&mut display_app_stats)
                .movable(true)
                .resizable(false)
                .title_bar(false)
                .default_pos(pos2(40.0, 80.0))
                .show(&render_frame.debug_gui, |ui|
                    {
                        // // todo: figure out how to make whole window draggable
                        // let interact = ui.interact(ui.max_rect(), egui::Id::new("App Stats"), egui::Sense::click_and_drag());
                        // if input.mouse().get_button(MouseButton::Left).state == ButtonState::JustOn // interact 'should' have a value to use here
                        // {
                        //     ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag)
                        // }

                        let fps = 1.0 / frame_start_time.delta_time.as_secs_f32();
                        ui.label(format!("FPS: {fps:.1}"));
                        #[cfg(debug_assertions)]
                        {
                            ui.label(format!("Frame: {frame_number}"));
                            ui.label(format!("App time: {:.1}s", clock.total_runtime().as_secs_f32()));

                            let main_window_size = windows.main_window().size();
                            ui.label(format!("Window: {} x {}", main_window_size.0, main_window_size.1));

                            let viewport_size = renderer.display_size();
                            ui.label(format!("Viewport: {} x {}", viewport_size.x, viewport_size.y));
                        }
                    });
        }
        renderer.present(render_frame);

        if completion == CompletionState::Completed
        {
            break 'main
        }
    }

    renderer.device().destroy();

    println!("Exited 3L14 at {}", chrono::Local::now());
}
