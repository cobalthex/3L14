use sdl2::{*, video::*};

pub struct Windows
{
    main_window: Window,
}
impl Windows
{
    pub fn new(sdl_video: &VideoSubsystem) -> Self
    {
        let main_window = sdl_video
            .window("3L14", 1920, 1080)
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
