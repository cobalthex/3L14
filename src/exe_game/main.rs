use asset_3l14::{Asset, AssetKey, AssetLifecyclers, AssetPayload, Assets, AssetsConfig};
use clap::Parser;
use debug_3l14::debug_gui;
use debug_3l14::debug_menu::{DebugMenu, DebugMenuMemory};
use debug_3l14::sparkline::Sparkline;
use glam::{FloatExt, Mat4, Quat, Vec3, Vec4};
use graphics_3l14::assets::{GeometryLifecycler, MaterialLifecycler, Model, ModelLifecycler, ShaderLifecycler, TextureLifecycler};
use graphics_3l14::camera::{Camera, CameraProjection};
use graphics_3l14::debug_draw::DebugDraw;
use graphics_3l14::pipeline_cache::{DebugMode, PipelineCache};
use graphics_3l14::uniforms_pool::UniformsPool;
use graphics_3l14::view::{Draw, View};
use graphics_3l14::windows::Windows;
use graphics_3l14::{colors, render_passes, renderer, Renderer, Rgba};
use input_3l14::{Input, KeyCode, KeyMods};
use nab_3l14::app;
use nab_3l14::app::{AppRun, ExitReason};
use nab_3l14::core_types::{CompletionState, FrameNumber, ToggleState};
use math_3l14::{Degrees, Frustum, Plane, Radians, Transform};
use nab_3l14::timing::Clock;
use sdl2::event::{Event as SdlEvent, WindowEvent as SdlWindowEvent};
use std::ops::Deref;
use std::time::Duration;
use wgpu::{BindingResource, BufferAddress, BufferBinding, BufferDescriptor, BufferSize, BufferUsages, CommandEncoderDescriptor};

#[derive(Debug, Parser)]
struct CliArgs
{
    #[cfg(debug_assertions)]
    #[arg(long, default_value_t = false)]
    keep_alive_on_panic: bool,
}

fn main() -> ExitReason
{
    let app_run = AppRun::<CliArgs>::startup("3L14", env!("CARGO_PKG_VERSION"));
    {
        #[cfg(debug_assertions)]
        let keep_alive = app_run.args.keep_alive_on_panic;
        #[cfg(not(debug_assertions))]
        let keep_alive = false;
        app::set_panic_hook(keep_alive);
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

        let model_key: AssetKey = 0x009000008dd00f81.into();
        let test_model = assets.load::<Model>(model_key);

        let mut camera = Camera::default();
        camera.update_projection(CameraProjection::Perspective
        {
            fov: Degrees(90.0).into(),
            aspect_ratio: renderer.display_aspect_ratio(),
        }, 0.1, 1000.0);
        let mut cam_transform = Transform
        {
            position: Vec3::new(0.0, 0.0, -10.0),
            rotation: Quat::default(),
            scale: Vec3::default(),
        };
        camera.update_view(cam_transform.clone());

        let pipeline_cache = PipelineCache::new(renderer.clone());
        // ꙮ
        let uniforms_pool = UniformsPool::new(renderer.clone());

        let mut obj_rot = Quat::IDENTITY;

        let mut views: [_; renderer::MAX_CONSECUTIVE_FRAMES] = array_init::array_init(|_| View::new(renderer.clone(), &pipeline_cache));

        let mut debug_draw = DebugDraw::new(&renderer);

        let mut clip_camera = None;

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

                // todo: ideally move elsewhere
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
                // todo: apply slight curve to input scale
                // TODO: scale by FOV
                const MOUSE_SCALE: f32 = 0.005;
                let md = input.mouse().position_delta;
                let mdl = (md.length_squared() as f32).sqrt();
                let yaw = (md.x as f32 * MOUSE_SCALE); // left to right
                let pitch = (md.y as f32 * MOUSE_SCALE); // down to up
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
            if kbd.is_press(KeyCode::X)
            {
                cam_transform.rotation = Quat::IDENTITY;
            }
            if kbd.is_press(KeyCode::T)
            {
                let mut cam = camera.clone();
                cam.update_projection(cam.projection().clone(), 0.1, 50.0);
                clip_camera = Some(cam);
            }

            if let CameraProjection::Perspective { fov, aspect_ratio} = camera.projection()
            {
                let mut fov2 = *fov;
                let is_zoomed = kbd.is_down(KeyCode::Z);
                fov2 = Degrees(f32::lerp(fov.to_degrees_f32(), if is_zoomed { 30.0 } else { 90.0 }, frame_time.delta_time.as_secs_f32() * 5.0)).into();
                camera.update_projection(CameraProjection::Perspective { fov: fov2, aspect_ratio: *aspect_ratio }, camera.near_clip(), camera.far_clip());
            }

            camera.update_view(cam_transform.clone());

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

                debug_draw.begin(&camera, renderer.debug_gui());

                if let Some(cam) = &clip_camera
                {
                    for corner in Frustum::get_corners(&cam.projection().to_matrix(0.1, 50.0))
                    {
                        debug_draw.draw_cross3(Mat4::from_translation(corner), colors::RED);
                    }

                    debug_draw.draw_frustum(cam, colors::WHITE);
                    let mut light = 0.0;
                    for plane in Frustum::from_matrix(&cam.matrix()).planes
                    {
                        let z = cam.matrix().transform_point3(plane.origin());
                        debug_draw.draw_polyline(&plane.into_quad(4.0, 4.0), true, Rgba::from_hsla(30.0, 0.7, light, 1.0));
                        debug_draw.draw_arrow(plane.origin(), plane.origin() + plane.normal() * 2.0, Vec3::Y, colors::MAGENTA);
                        light += 1.0 / 6.0;
                    }
                }

                let mut encoder = renderer.device().create_command_encoder(&CommandEncoderDescriptor::default());
                {
                    {
                        let mut scene_pass = render_passes::scene(
                            &render_frame,
                            &mut encoder,
                            Some(colors::CORNFLOWER_BLUE));

                        let view = &mut views[frame_number.0 as usize % views.len()];
                        view.begin(frame_time.total_runtime, &camera, clip_camera.as_ref().unwrap_or(&camera), DebugMode::None);

                        debug_draw.draw_solid_cube(Mat4::IDENTITY, colors::RED);
                        debug_draw.draw_wire_cube(Mat4::IDENTITY, colors::YELLOW);

                        debug_draw.draw_solid_cone(Mat4::from_translation(Vec3::new(3.0, 0.0, 0.0)), colors::GOOD_PURPLE);
                        debug_draw.draw_wire_cone(Mat4::from_translation(Vec3::new(3.0, 0.0, 0.0)), colors::MAGENTA);

                        debug_draw.draw_solid_sphere(Mat4::from_translation(Vec3::new(-3.0, 0.0, 0.0)), colors::LIME);
                        debug_draw.draw_wire_sphere(Mat4::from_translation(Vec3::new(-3.0, 0.0, 0.0)), colors::GREEN);

                        if let AssetPayload::Available(model) = test_model.payload()
                        {
                            if model.all_dependencies_loaded()
                            {
                                let mut obj_world = Mat4::from_rotation_translation(obj_rot, Vec3::new(25.0, 0.0, 0.0));
                                view.draw(obj_world, model.clone());
                                debug_draw.draw_wire_cube(obj_world, colors::WHITE);

                                let geo = model.geometry.payload().unwrap();
                                let sp_txfm = obj_world * Mat4::from_scale(Vec3::splat(geo.bounds_sphere.radius()));
                                debug_draw.draw_wire_sphere(sp_txfm, colors::TOMATO);

                                obj_world = Mat4::from_rotation_translation(obj_rot.inverse(), Vec3::new(-5.0, 0.0, -2.0));
                                view.draw(obj_world, model);
                                debug_draw.draw_wire_cube(obj_world, colors::WHITE);
                            }
                        }

                        view.submit(&mut scene_pass);
                    }

                    {
                        let mut debug_pass = render_passes::debug(&render_frame, &mut encoder);
                        debug_draw.submit(renderer.queue(), &mut debug_pass);
                    }
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

                let mut debug_menu = DebugMenu::new(&mut debug_menu_memory, renderer.debug_gui());
                debug_menu.add(&app_stats);
                debug_menu.add(&fps_sparkline);
                debug_menu.add(&debug_gui::FrameProfiler);
                debug_menu.add(&input);
                debug_menu.add(&camera);
                debug_menu.add(&assets);
                debug_menu.add(renderer.deref());
                debug_menu.add(&debug_draw);
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

    std::thread::sleep(Duration::from_micros(10)); // allow logs to flush -- TEMP

    drop(renderer);
    drop(windows);
    drop(assets);

    std::thread::sleep(Duration::from_micros(10)); // allow logs to flush -- TEMP

    app_run.get_exit_reason()
}
