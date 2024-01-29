use sdl2::{*, video::*};
use sdl2::render::WindowCanvas;

pub struct Windows
{
    main_window: WindowCanvas,
}
impl Windows
{
    pub fn new(sdl_video: &VideoSubsystem) -> Self
    {
        let main_window = sdl_video.window("3L14", 1920, 1080).build().unwrap();
        Self
        {
            main_window: main_window.into_canvas().build().unwrap(),
        }
    }

    pub fn main_window(&self) -> &WindowCanvas { &self.main_window }
    pub fn main_window_mut(&mut self) -> &mut WindowCanvas { &mut self.main_window }
}
