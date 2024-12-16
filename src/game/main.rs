use arrayvec::ArrayVec;
use clap::Parser;
use game_3l14::engine::graphics::assets::texture::TextureLifecycler;
use game_3l14::engine::graphics::assets::{material, Geometry, GeometryLifecycler, Material, MaterialLifecycler, Model, ModelLifecycler, Shader, ShaderLifecycler, Texture};
use game_3l14::engine::graphics::debug_gui::debug_menu::{DebugMenu, DebugMenuMemory};
use game_3l14::engine::graphics::debug_gui::sparkline::Sparkline;
use game_3l14::engine::graphics::pipeline_cache::{DebugMode, PipelineCache};
use game_3l14::engine::{asset::*, graphics::*, input::*, timing::*, windows::*, world::*, *};
use game_3l14::{debug_label, ExitReason};
use glam::{Mat4, Quat, Vec3};
use sdl2::event::{Event as SdlEvent, WindowEvent as SdlWindowEvent};
use std::io::Read;
use std::ops::Deref;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use wgpu::{BindGroupDescriptor, BindGroupEntry, BindingResource, BufferAddress, BufferBinding, BufferDescriptor, BufferSize, BufferUsages, CommandEncoderDescriptor};
use game_3l14::engine::graphics::pipeline_sorter::PipelineSorter;
use game_3l14::engine::graphics::uniforms_pool::UniformsPool;
use game_3l14::engine::graphics::view::View;

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
            debug_menu_memory = DebugMenuMemory::default();
            debug_menu_memory.set_active_by_name::<debug_gui::AppStats>("App Stats", true); // a big fragile...
        }

        // let min_frame_time = Duration::from_secs_f32(1.0 / 150.0); // todo: this should be based on display refresh-rate

        let model_key: AssetKey = 0x00800000042f8fe4c6e9839688654c23.into();
        let test_model = assets.load::<Model>(model_key);

        let mut camera = Camera::new(Some("fp_cam"), CameraProjection::Perspective
        {
            aspect_ratio: renderer.display_aspect_ratio(),
            fov: Degrees(59.0).into()
        });
        camera.transform.position = Vec3::new(0.0, 2.0, -10.0);
        camera.update_view();

        let mut pipeline_cache = PipelineCache::new(renderer.clone());

        const MAX_ENTRIES_IN_WORLD_BUF: usize = 64;
        let world_uform_buf = renderer.device().create_buffer(&BufferDescriptor
        {
            label: Some("World uniform buffer"),
            size: (std::mem::size_of::<TransformUniform>() * MAX_ENTRIES_IN_WORLD_BUF) as BufferAddress,
            usage: BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let world_bind_group = renderer.device().create_bind_group(&BindGroupDescriptor
        {
            layout: &pipeline_cache.common_layouts().world_transform,
            entries:
            &[
                BindGroupEntry
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

        // ꙮ

        let cam_uform_buf = renderer.device().create_buffer(&BufferDescriptor
        {
            label: Some("Camera uniform buffer"),
            size: std::mem::size_of::<CameraUniform>() as BufferAddress,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let cam_bind_group = renderer.device().create_bind_group(&BindGroupDescriptor
        {
            layout: &pipeline_cache.common_layouts().camera,
            entries:
            &[
                BindGroupEntry
                {
                    binding: 0,
                    resource: cam_uform_buf.as_entire_binding(),
                }
            ],
            label: Some("Camera bind group"),
        });

        let uniforms_pool = UniformsPool::new(renderer.clone());

        let mut obj_rot = Quat::IDENTITY;

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
                camera.transform.rotate(yaw, pitch, roll);
            }

            let speed = if input.keyboard().has_keymod(KeyMods::SHIFT) { 20.0 } else { 8.0 } * frame_time.delta_time.as_secs_f32();
            if kbd.is_down(KeyCode::W)
            {
                camera.transform.position += camera.transform.forward() * speed;
            }
            if kbd.is_down(KeyCode::A)
            {
                camera.transform.position += camera.transform.left() * speed;
            }
            if kbd.is_down(KeyCode::S)
            {
                camera.transform.position += camera.transform.backward() * speed;
            }
            if kbd.is_down(KeyCode::D)
            {
                camera.transform.position += camera.transform.right() * speed;
            }
            if kbd.is_down(KeyCode::E)
            {
                camera.transform.position += camera.transform.up() * speed;
            }
            if kbd.is_down(KeyCode::Q)
            {
                camera.transform.position += camera.transform.down() * speed;
            }
            if kbd.is_press(KeyCode::Z)
            {
                let rounding = std::f32::consts::FRAC_PI_4;
                let (axis, mut angle) = camera.transform.rotation.to_axis_angle();
                angle = f32::round(angle / rounding) * rounding;
                camera.transform.rotation = Quat::from_axis_angle(axis, angle);
            }
            if kbd.is_press(KeyCode::X)
            {
                camera.transform.rotation = Quat::IDENTITY;
            }
            camera.update_view();

            obj_rot *= Quat::from_rotation_y(0.5 * frame_time.delta_time.as_secs_f32());

            #[cfg(debug_assertions)]
            if kbd.is_press(KeyCode::F1)
            {
                if kbd.has_keymod(KeyMods::ALT)
                {
                    debug_menu_memory.toggle_active(&debug_gui::FrameProfiler);
                }
                else
                {
                    debug_menu_memory.is_active ^= true;
                }
            }

            let cam_uform = CameraUniform::new(&camera, &clock);
            renderer.queue().write_buffer(&cam_uform_buf, 0, unsafe { [cam_uform].as_u8_slice() });

            let render_frame = renderer.frame(frame_number, &input);
            {
                puffin::profile_scope!("Render frame");

                let mut encoder = renderer.device().create_command_encoder(&CommandEncoderDescriptor::default());
                {
                    fn get_model_mesh(model: &Model, mesh: u32) -> Option<(Arc<Geometry>, Arc<Material>, Arc<Shader>, Arc<Shader>)>
                    {
                        let AssetPayload::Available(geometry) = model.geometry.payload() else { return None; };
                        let AssetPayload::Available(material) = model.surfaces[mesh as usize].material.payload() else { return None; };
                        let AssetPayload::Available(vshader) = model.surfaces[mesh as usize].vertex_shader.payload() else { return None; };
                        let AssetPayload::Available(pshader) = model.surfaces[mesh as usize].pixel_shader.payload() else { return None; };
                        Some((geometry, material, vshader, pshader))
                    }

                    let mut test_pass = render_passes::test(
                        &render_frame,
                        &mut encoder,
                        Some(colors::CORNFLOWER_BLUE));

                    let mut view = View::new(&renderer, &camera, &pipeline_cache, &uniforms_pool);

                    if let AssetPayload::Available(model) = test_model.payload()
                    {
                        let obj_world = Mat4::from_rotation_translation(obj_rot, Vec3::new(3.0, 0.0, 0.0));
                        view.draw(obj_world, model);
                    }

                    view.submit(&mut test_pass);
                     //             test_pass.set_bind_group(0, &cam_bind_group, &[]);
                    //
                    //             // todo: use DrawIndirect?
                    //             worlds_buf[world_index].world =
                    //             let offset = (world_index * std::mem::size_of::<TransformUniform>()) as u32;
                    //             test_pass.set_bind_group(1, &world_bind_group, &[offset]);
                    //             world_index += 1;
                    //
                    //             let mesh = &geom.meshes[i as usize];
                    //             test_pass.set_vertex_buffer(0, mesh.vertices.slice(0..));
                    //             test_pass.set_index_buffer(mesh.indices.slice(0..), mesh.index_format);
                    //
                    //             let mut bge = ArrayVec::<_, 18>::new();
                    //             bge.push(BindGroupEntry
                    //             {
                    //                 binding: bge.len() as u32,
                    //                 resource: mtl.props.as_entire_binding(),
                    //             });
                    //             if !textures.is_empty()
                    //             {
                    //                 bge.push(BindGroupEntry
                    //                 {
                    //                     binding: bge.len() as u32,
                    //                     resource: BindingResource::Sampler(pipeline_cache.default_sampler())
                    //                 });
                    //                 for tex in &textures
                    //                 {
                    //                     bge.push(BindGroupEntry
                    //                     {
                    //                         binding: bge.len() as u32,
                    //                         resource: BindingResource::TextureView(&tex.gpu_view),
                    //                     })
                    //                 }
                    //             }
                    //
                    //             let bind_group = renderer.device().create_bind_group(&BindGroupDescriptor
                    //             {
                    //                 label: debug_label!("TODO mtl bind group"), // TODO
                    //                 layout: &mtl.bind_layout,
                    //                 entries: &bge,
                    //             });
                    //
                    //             test_pass.set_bind_group(2, &bind_group, &[]);
                    //
                    //             test_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                    //         }
                    //     }
                    // }
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
                    app_runtime: clock.total_runtime().as_secs_f64(),
                    main_window_size: windows.main_window().size(),
                    viewport_size: renderer.display_size().into(),
                };

                let mut debug_menu = DebugMenu::new(&mut debug_menu_memory, &render_frame.debug_gui);
                debug_menu.add(&app_stats);
                debug_menu.add(&debug_gui::FrameProfiler);
                debug_menu.add(&input);
                debug_menu.add(&camera);
                debug_menu.add(&assets);
                debug_menu.add(renderer.deref());
                debug_menu.present();
            }

            renderer.present(render_frame);

            if completion == CompletionState::Completed
            {
                break 'main_loop
            }
        }
    }
    
    std::mem::drop(renderer);
    std::mem::drop(windows);
    std::mem::drop(assets);

    std::thread::sleep(Duration::from_micros(10)); // allow logs to flush -- TEMP

    app_run.get_exit_reason()
}
