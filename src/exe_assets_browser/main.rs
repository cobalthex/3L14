use std::error::Error;
use std::fmt::Write;
use asset_3l14::{AssetFileType, AssetMetadata, TomlRead};
use clap::Parser;
use graphics_3l14::windows::Windows;
use graphics_3l14::Renderer;
use input_3l14::{Input, KeyCode, KeyMods};
use nab_3l14::app::{AppFolder, AppRun, ExitReason};
use nab_3l14::{CompletionState, RenderFrameNumber};
use sdl2::event::{Event as SdlEvent, WindowEvent as SdlWindowEvent};
use std::time::Instant;
// TODO: use epaint for this?

struct AssetInfo
{
    display_name: String,
    meta: AssetMetadata,
}

#[derive(Parser, Debug)]
struct CliArgs
{

}

fn main() -> ExitReason
{
    let app_run = AppRun::<CliArgs>::startup("Assets Browser", "0.1.0");

    let sdl = sdl2::init().unwrap();
    let mut sdl_events = sdl.event_pump().unwrap();
    let sdl_video = sdl.video().unwrap();
    let mut app_frame_number = RenderFrameNumber(0);

    // windows
    let windows = Windows::new(&sdl_video, &app_run);
    let mut input = Input::new(&sdl);

    let mut assets_list = Vec::new();
    (||
    {
        let meta_ext = AssetFileType::MetaData.file_extension();
        for maybe_entry in std::fs::read_dir(app_run.get_app_folder(AppFolder::Assets))?
        {
            let Ok(entry) = maybe_entry else { continue };
            let Ok(ft) = entry.file_type() else { continue; };
            if ft.is_file()
            {
                // to_str should be allowed but rust is dumb
                let fname = entry.file_name().into_string().unwrap_or_default();
                println!("> {fname}");
                if fname.ends_with(&meta_ext)
                {
                    let mut reader = std::fs::File::open(entry.path())?;
                    let asset_meta = AssetMetadata::load(&mut reader)?;
                    assets_list.push(AssetInfo
                    {
                        display_name: match &asset_meta.name
                        {
                            None => format!("{:#?}", asset_meta.key),
                            Some(name) => format!("{name} ({:?})", asset_meta.key.asset_type()),
                        },
                        meta: asset_meta,
                    });
                }
            }
            // else if ft.is_dir()
            // {
            //
            // }
        }
        Ok::<(), Box<dyn Error>>(())
    })().expect("Failed to load assets");
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
                let text_style = egui::TextStyle::Body;
                let row_height = ui.text_style_height(&text_style);
                egui::ScrollArea::vertical().show_rows(ui, row_height, assets_list.len(),|sui, vis|
                {
                   for (i, asset) in assets_list[vis].iter().enumerate()
                   {
                       let is_selected = i == selected_asset_index;
                       let resp = sui.selectable_label(is_selected, asset.display_name.as_str());
                       if resp.clicked_by(egui::PointerButton::Primary)
                       {
                           selected_asset_index = i;
                       }
                       else if resp.clicked_by(egui::PointerButton::Secondary)
                       {
                           let text = format!("{:#x}", asset.meta.key);
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

                    let build_time = chrono::DateTime::<chrono::Local>::from(asset.meta.build_timestamp);

                    ui.heading(&asset.display_name);
                    ui.add_space(20.0);
                    // TODO: table
                    ui.monospace(format!("      Name: {}", asset.meta.name.as_deref().unwrap_or_default()));
                    ui.monospace(format!("       Key: {:#x}", asset.meta.key));
                    ui.monospace(format!("Build time: {}", build_time.format("%Y-%m-%d %H:%M:%S").to_string()));
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