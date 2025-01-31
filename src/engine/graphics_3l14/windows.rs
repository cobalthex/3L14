use sdl2::{*, video::*};
use nab_3l14::app::{AppRun, CliArgs};

pub struct Windows
{
    main_window: Window,
}
impl Windows
{
    pub fn new(sdl_video: &VideoSubsystem, app_info: &AppRun<impl CliArgs>) -> Self
    {
        #[cfg(debug_assertions)]
        let window_title = format!(
            "{}  -  v{}  PID:{}  {}",
            app_info.app_name,
            app_info.version_str,
            app_info.pid,
            if app_info.is_elevated { "🛡️" } else { "" });
        #[cfg(not(debug_assertions))]
        let window_title = String::from(app_info.app_name); // todo: make this not allocate

        let main_window = sdl_video
            .window(window_title.as_str(), 1920, 1080)
            .resizable()
            .build()
            .unwrap();
        Self
        {
            main_window,//: main_window.into_canvas().build().unwrap(),
        }
    }

    pub fn main_window(&self) -> &Window { &self.main_window }
    pub fn main_window_mut(&mut self) -> &mut Window { &mut self.main_window }
}
