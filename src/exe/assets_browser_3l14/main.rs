use std::any::Any;
use std::convert::Infallible;
use std::error::Error;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use asset_3l14::{AssetFileType, AssetKey, AssetMetadata, AssetTypeId, SourceMetadata, TomlRead};
use clap::Parser;
use graphics_3l14::windows::Windows;
use graphics_3l14::Renderer;
use input_3l14::{Input, KeyCode, KeyMods};
use nab_3l14::app::{AppFolder, AppRun, ExitReason};
use nab_3l14::{CompletionState, RenderFrameNumber};
use sdl2::event::{Event as SdlEvent, WindowEvent as SdlWindowEvent};
use std::time::Instant;
use egui::TextBuffer;
use unicase::UniCase;
// TODO: use epaint for this?

#[derive(Debug)]
struct AssetInfo
{
    display_name: String,
    asset_meta: AssetMetadata,
    // source_meta: SourceMetadata, // TODO: pass in 'src directory' to read?
}

fn parse_asset_type(arg: &str) -> Result<AssetTypeId, &'static str>
{
    let type_caseless = UniCase::new(arg);
    let ty = match AssetTypeId::unit_variants().iter()
        .find_map(|(name, value)| if UniCase::new(name) == type_caseless { Some(*value) } else { None })
    {
        Some(ty) => ty,
        None => return Err("Unknown asset type"),
    };
    Ok(ty)
}

#[derive(Parser, Debug)]
struct CliArgs
{
    //// CLI ////

    #[arg(long, group="cli", conflicts_with="gui")]
    asset_key: Option<String>,

    //// GUI ////

    #[arg(long, group="gui", conflicts_with="cli", value_parser=parse_asset_type)] // alias=type?
    only_type: Option<AssetTypeId>,
}

fn main() -> ExitReason
{
    let app_run = AppRun::<CliArgs>::startup("Assets Browser", "0.1.0");

    let meta_ext = AssetFileType::MetaData.file_extension();

    //// CLI args parsing ////

    if let Some(asset_key) = &app_run.args.asset_key
    {
        let fname = format!("{asset_key}.{meta_ext}");
        let path = PathBuf::from_iter(&[app_run.get_app_folder(AppFolder::Assets), fname.into()]);
        let Ok(mut reader) = std::fs::File::open(&path)
        else
        {
            log::error!("Failed to open asset meta for {path:?}");
            return ExitReason::CliError;
        };
        let Ok(asset_meta) = AssetMetadata::load(&mut reader)
        else
        {
            log::error!("Failed to parse asset meta for {path:?}");
            return ExitReason::CliError;
        };

        let info = AssetInfo
        {
            display_name: match &asset_meta.name
            {
                None => format!("{:#?}", asset_meta.key),
                Some(name) => format!("{name} ({:?})", asset_meta.key.asset_type()),
            },
            asset_meta,
        };
        log::info!("{:#?}", info);

        return ExitReason::NormalExit
    }

    //// GUI args parsing ////

    //// Startup ////

    let sdl = sdl2::init().unwrap();
    let mut sdl_events = sdl.event_pump().unwrap();
    let sdl_video = sdl.video().unwrap();
    let mut app_frame_number = RenderFrameNumber(0);

    // windows
    let windows = Windows::new(&sdl_video, &app_run);
    let mut input = Input::new(&sdl);

    let assets_list: Box<[_]> = std::fs::read_dir(app_run.get_app_folder(AppFolder::Assets))
        .expect("Failed to read assets dir")
        .filter_map(|entry|
    {
        let Ok(entry) = entry else { return None; };
        let Ok(ft) = entry.file_type() else { return None; };
        if ft.is_file()
        {
            if let Some(filter_by_type) = app_run.args.only_type
            {
                let entry_path = entry.path();
                let key_str = entry_path.file_stem().expect("File has no name???").to_string_lossy();
                let key = match AssetKey::try_from(key_str.as_str())
                {
                    Ok(key) => key,
                    Err(err) =>
                    {
                        log::error!("Failed to parse asset key from {:?} to test against type filter", entry.file_name());
                        return None;
                    }
                };
                if key.asset_type() != filter_by_type
                {
                    return None;
                }
            }

            // to_str should be allowed but rust is dumb
            let fname = entry.file_name().into_string().unwrap_or_default();
            if fname.ends_with(&meta_ext)
            {
                let Ok(mut reader) = std::fs::File::open(entry.path())
                    else { log::error!("Failed to open asset meta for {fname:?}"); return None; };
                let Ok(asset_meta) = AssetMetadata::load(&mut reader)
                    else { log::error!("Failed to parse asset meta for {fname:?}"); return None; };

                // let mut source_path = PathBuf::from("assets/src"); // TODO: HAX
                // source_path.push(&asset_meta.source_path);
                // source_path.add_extension("sork"); // TODO: const
                // let Ok(mut reader) = std::fs::File::open(source_path)
                //     else { log::error!("Failed to open source meta for {fname:?}"); return None; };
                // let Ok(source_meta) = SourceMetadata::load(&mut reader)
                //     else { log::error!("Failed to parse source meta for {fname:?}"); return None; };

                return Some(AssetInfo
                {
                    display_name: match &asset_meta.name
                    {
                        None => format!("{:#?}", asset_meta.key),
                        Some(name) => format!("{name} ({:?})", asset_meta.key.asset_type()),
                    },
                    asset_meta,
                });
            }
        }
        None
        // else if ft.is_dir()
        // {
        //
        // }
    }).collect();
    let mut selected_asset_index = usize::MAX;

    let renderer = Renderer::new(windows.main_window());
    'main_loop: loop
    {
        let mut completion = CompletionState::InProgress;

        std::thread::sleep(std::time::Duration::from_millis(20));

        {
            let time_now = Instant::now();
            app_frame_number.increment();
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

                    _ => input.handle_event(event, time_now),
                }
            }
        }

        let kbd = input.keyboard();

        if kbd.is_down(KeyCode::Q) &&
            kbd.has_keymod(KeyMods::CTRL)
        {
            completion = CompletionState::Completed;
        }

        let render_frame = renderer.frame(app_frame_number, &input);
        let view_size = renderer.display_size();
        let asset_list = egui::SidePanel::left("asset_list")
            .resizable(true)
            .default_width(400.0)
            .show(renderer.debug_gui(), |ui|
            {
                ui.heading("Assets");
                if let Some(filter_by_type) = app_run.args.only_type
                {
                    // same line?
                    ui.label(format!("Filtered by type: {:?}", app_run.args.only_type));
                }
                ui.separator();

                let row_height = ui.text_style_height(&egui::TextStyle::Body);
                let z = egui::ScrollArea::vertical().show_rows(ui, row_height, assets_list.len(),|sui, vis|
                {
                   for (i, asset) in assets_list[vis.clone()].iter().enumerate()
                   {
                       let idx = i + vis.start;
                       let is_selected = idx == selected_asset_index;
                       let resp = sui.selectable_label(is_selected, asset.display_name.as_str());
                       if resp.clicked()
                       {
                           selected_asset_index = idx;
                       }
                       else if resp.secondary_clicked()
                       {
                           let text = format!("{:#x}", asset.asset_meta.key);
                           // egui clipboard not working
                           let _ = sdl_video.clipboard().set_clipboard_text(&text);
                       }
                   }
                });
            });

        let info_panel = egui::CentralPanel::default()
            .show(renderer.debug_gui(), |ui|
            {
                if selected_asset_index != usize::MAX
                {
                    let asset = &assets_list[selected_asset_index];

                    let build_time = chrono::DateTime::<chrono::Local>::from(asset.asset_meta.build_timestamp);

                    ui.heading(&asset.display_name);
                    ui.add_space(20.0);
                    // TODO: table
                    ui.monospace(format!("       Name: {}", asset.asset_meta.name.as_deref().unwrap_or_default()));
                    ui.monospace(format!("        Key: {:#x}", asset.asset_meta.key));
                    ui.monospace(format!(" Build time: {}", build_time.format("%Y-%m-%d %H:%M:%S").to_string()));
                    ui.monospace(format!("Source path: {}", asset.asset_meta.source_path.display()));

                    // if ui.button("debug").clicked()
                    // {
                    //     #[cfg(target_arch = "x86_64")]
                    //     unsafe { std::arch::asm!("int3"); }
                    //     #[cfg(target_arch = "aarch64")]
                    //     unsafe { std::arch::asm!("brk #0xf000"); }
                    // }
                }
                else
                {
                    ui.centered_and_justified(|cui| cui.label("No asset selected"));
                }
            });

        renderer.present(render_frame);
        if completion == CompletionState::Completed
        {
            break 'main_loop;
        }
    }

    app_run.get_exit_reason()
}
