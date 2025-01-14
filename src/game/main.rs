use game_3l14::engine::graphics::view::Draw;
use clap::Parser;
use egui::Widget;
use game_3l14::engine::graphics::assets::texture::TextureLifecycler;
use game_3l14::engine::graphics::assets::{material, Geometry, GeometryLifecycler, Material, MaterialLifecycler, Model, ModelLifecycler, Shader, ShaderLifecycler, Texture};
use game_3l14::engine::graphics::debug_gui::debug_menu::{DebugMenu, DebugMenuMemory};
use game_3l14::engine::graphics::debug_gui::sparkline::Sparkline;
use game_3l14::engine::graphics::pipeline_cache::{DebugMode, PipelineCache};
use game_3l14::engine::graphics::uniforms_pool::UniformsPool;
use game_3l14::engine::graphics::view::View;
use game_3l14::engine::math::Degrees;
use game_3l14::engine::{asset::*, graphics::*, input::*, timing::*, windows::*, world::*, *};
use game_3l14::ExitReason;
use glam::{Mat4, Quat, Vec3};
use sdl2::event::{Event as SdlEvent, WindowEvent as SdlWindowEvent};
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use wgpu::{BindGroupEntry, BindingResource, BufferAddress, BufferBinding, BufferDescriptor, BufferSize, BufferUsages, CommandEncoderDescriptor};

#[derive(Debug, Parser)]
struct CliArgs
{
    #[cfg(debug_assertions)]
    #[arg(long, default_value_t = false)]
    keep_alive_on_panic: bool,
}

fn main() -> ExitReason
{
    let app_run = game_3l14::AppRun::<CliArgs>::startup("3L14");
    {
        #[cfg(debug_assertions)]
        let keep_alive = app_run.args.keep_alive_on_panic;
        #[cfg(not(debug_assertions))]
        let keep_alive = false;
        game_3l14::set_panic_hook(keep_alive);
    }

    #[cfg(debug_assertions)]
    {
        let server_addr = format!("0.0.0.0:{}", puffin_http::DEFAULT_PORT);
        match puffin_http::Server::new(&server_addr)
        {
            Ok(_) => log::debug!("Puffin serving on {server_addr}"),
            Err(e) => log::warn!("Failed to start puffin server on {server_addr}: {e}"),
        }
        puffin::set_scopes_on(true);
    }

    let mut clock = Clock::new();

    let sdl = sdl2::init().unwrap();
    let mut sdl_events = sdl.event_pump().unwrap();
    let sdl_video = sdl.video().unwrap();

    // windows
    let windows = Windows::new(&sdl_video, &app_run);
    let mut input = Input::new(&sdl);

    let renderer = Renderer::new(windows.main_window());

    let assets_config = AssetsConfig
    {
        enable_fs_watcher: cfg!(debug_assertions)
    };
    let assets = Assets::new(AssetLifecyclers::default()
            .add_lifecycler(ModelLifecycler::new(renderer.clone()))
            .add_lifecycler(TextureLifecycler::new(renderer.clone()))
            .add_lifecycler(ShaderLifecycler::new(renderer.clone()))
            .add_lifecycler(MaterialLifecycler::new(renderer.clone()))
            .add_lifecycler(GeometryLifecycler::new(renderer.clone()))
        , assets_config);

    {
        #[cfg(debug_assertions)]
        let mut debug_menu_memory;
        #[cfg(debug_assertions)]
        {
            debug_menu_memory = DebugMenuMemory::load("debug_gui.state");
            debug_menu_memory.set_state_active_by_name::<debug_gui::AppStats>("App Stats", true); // a big fragile...
        }

        // let min_frame_time = Duration::from_secs_f32(1.0 / 150.0); // todo: this should be based on display refresh-rate

        let model_key: AssetKey = 0x008000008dd00f81.into();
        let test_model = assets.load::<Model>(model_key);

        let mut camera = Camera::new(Some("cam"));
        camera.update_projection(CameraProjection::Perspective
        {
            fov: Degrees(90.0).into(),
            aspect_ratio: renderer.display_aspect_ratio(),
        }, 0.1, 1000.0);
        let mut cam_transform = Transform
        {
            position: Vec3::new(0.0, 2.0, -10.0),
            rotation: Quat::default(),
            scale: Vec3::default(),
        };
        camera.update_view(&cam_transform);

        let pipeline_cache = PipelineCache::new(renderer.clone());
        // ꙮ
        let uniforms_pool = UniformsPool::new(renderer.clone());

        let mut obj_rot = Quat::IDENTITY;

        let mut views: [_; renderer::MAX_CONSECUTIVE_FRAMES] = array_init::array_init(|_| View::new(renderer.clone(), &pipeline_cache));

        let mut frame_number = FrameNumber(0);
        let mut fps_sparkline = Sparkline::<100>::new(); // todo: use
        'main_loop: loop
        {
            let mut completion = CompletionState::InProgress;

            puffin::GlobalProfiler::lock().new_frame();

            frame_number.increment();
            let frame_time = clock.tick();
            fps_sparkline.add(frame_time.fps());

            {
                puffin::profile_scope!("Read input");

                input.pre_update();

                for event in sdl_events.poll_iter()
                {
                    match event
                    {
                        SdlEvent::Quit { .. } =>
                        {
                            completion |= CompletionState::Completed;
                        },
                        // SizeChanged?
                        SdlEvent::Window { win_event: SdlWindowEvent::Resized(w, h), .. } =>
                        {
                            renderer.resize(w as u32, h as u32);
                        },
                        SdlEvent::Window { win_event: SdlWindowEvent::DisplayChanged(index), .. } => 'arm:
                        {
                            let Ok(wind_index) = windows.main_window().display_index() else { break 'arm };

                            if wind_index == index
                            {
                                // todo: find a way to recalculate refresh rate -- reconfigure surface_config does not work
                            }
                        },

                        _ => input.handle_event(event, frame_time.current_time),
                    }
                }
            }

            let kbd = input.keyboard();

            #[cfg(debug_assertions)]
            if kbd.is_down(KeyCode::Q) &&
                kbd.has_keymod(KeyMods::CTRL)
            {
                completion = CompletionState::Completed;
            }

            if kbd.is_press(KeyCode::Backquote)
            {
                input.mouse().set_capture(ToggleState::Toggle);
            }

            if input.mouse().is_captured()
            {
                const MOUSE_SCALE: f32 = 0.015;
                let yaw = input.mouse().position_delta.x as f32 * MOUSE_SCALE; // left to right
                let pitch = input.mouse().position_delta.y as f32 * MOUSE_SCALE; // down to up
                let roll = 0.0;
                cam_transform.rotate(yaw, pitch, roll);
            }

            let speed = if input.keyboard().has_keymod(KeyMods::SHIFT) { 20.0 } else { 8.0 } * frame_time.delta_time.as_secs_f32();
            if kbd.is_down(KeyCode::W)
            {
                cam_transform.position += cam_transform.forward() * speed;
            }
            if kbd.is_down(KeyCode::A)
            {
                cam_transform.position += cam_transform.left() * speed;
            }
            if kbd.is_down(KeyCode::S)
            {
                cam_transform.position += cam_transform.backward() * speed;
            }
            if kbd.is_down(KeyCode::D)
            {
                cam_transform.position += cam_transform.right() * speed;
            }
            if kbd.is_down(KeyCode::E)
            {
                cam_transform.position += cam_transform.up() * speed;
            }
            if kbd.is_down(KeyCode::Q)
            {
                cam_transform.position += cam_transform.down() * speed;
            }
            if kbd.is_press(KeyCode::Z)
            {
                let rounding = std::f32::consts::FRAC_PI_4;
                let (axis, mut angle) = cam_transform.rotation.to_axis_angle();
                angle = f32::round(angle / rounding) * rounding;
                cam_transform.rotation = Quat::from_axis_angle(axis, angle);
            }
            if kbd.is_press(KeyCode::X)
            {
                cam_transform.rotation = Quat::IDENTITY;
            }
            camera.update_view(&cam_transform);

            obj_rot *= Quat::from_rotation_y(0.5 * frame_time.delta_time.as_secs_f32());

            #[cfg(debug_assertions)]
            if kbd.is_press(KeyCode::F1)
            {
                if kbd.has_keymod(KeyMods::ALT)
                {
                    debug_menu_memory.toggle_state_active(&debug_gui::FrameProfiler);
                }
                else
                {
                    debug_menu_memory.toggle_active();
                }
            }

            let render_frame = renderer.frame(frame_number, &input);
            {
                puffin::profile_scope!("Render frame");

                let mut encoder = renderer.device().create_command_encoder(&CommandEncoderDescriptor::default());
                {
                    let mut test_pass = render_passes::test(
                        &render_frame,
                        &mut encoder,
                        Some(colors::CORNFLOWER_BLUE));

                    let view = &mut views[frame_number.0 as usize % views.len()];
                    view.start(frame_time.total_runtime, &camera, DebugMode::None);

                    if let AssetPayload::Available(model) = test_model.payload()
                    {
                        if model.all_dependencies_loaded()
                        {
                            let mut obj_world = Mat4::from_rotation_translation(obj_rot, Vec3::new(3.0, 0.0, 0.0));
                            view.draw(obj_world, model.clone());

                            obj_world = Mat4::from_rotation_translation(obj_rot.inverse(), Vec3::new(-3.0, 0.0, -2.0));
                            view.draw(obj_world, model);
                        }
                    }

                    view.submit(&mut test_pass);
                }

                // todo: only update what was written to
                renderer.queue().submit([encoder.finish()]);
            }

            // TODO: basic app stats can be displayed in release
            #[cfg(debug_assertions)]
            {
                let app_stats = debug_gui::AppStats
                {
                    fps: frame_time.fps(),
                    frame_number,
                    app_runtime: frame_time.total_runtime.as_secs_f64(),
                    main_window_size: windows.main_window().size(),
                    viewport_size: renderer.display_size().into(),
                };

                let mut debug_menu = DebugMenu::new(&mut debug_menu_memory, &render_frame.debug_gui);
                debug_menu.add(&app_stats);
                debug_menu.add(&fps_sparkline);
                debug_menu.add(&debug_gui::FrameProfiler);
                debug_menu.add(&input);
                debug_menu.add(&camera);
                debug_menu.add(&assets);
                debug_menu.add(renderer.deref());
                debug_menu.add(&pipeline_cache);
                debug_menu.present();

                debug_menu_memory.save_if_dirty("debug_gui.state");
            }

            renderer.present(render_frame);

            if completion == CompletionState::Completed
            {
                break 'main_loop
            }
        }
    }
    
    drop(renderer);
    drop(windows);
    drop(assets);

    std::thread::sleep(Duration::from_micros(10)); // allow logs to flush -- TEMP

    app_run.get_exit_reason()
}
