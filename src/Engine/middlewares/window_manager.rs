use crate::Engine::{AppContext, CompletionState, Middleware};

use winit::{
    event::*,
    event_loop::*,
    window::*,
    dpi::LogicalSize,
    error::OsError,
};

pub struct GameWindow
{
    pub window: Window,
    //pub input: WindowInputState,
}

// global
pub struct Windows
{
    windows: Vec<GameWindow>, // window 0 is always the main window
    shutdown_after_main_window_closed: bool,
}
impl Windows
{
    pub fn get_main_window(&self) -> Option<&GameWindow>
    {
        self.windows.first()
    }

    // Create a window. The first window created will be the main window
    pub fn create_window(&mut self, title: &str, size: LogicalSize<i32>) //-> Result<WindowId, OsError>
    {
        println!("Will create a window some day");
        // let window = WindowBuilder::new()
        //     .with_title(title)
        //     .with_inner_size(size)
        //     .build(&event_loop)?;

        // let id = window.id();
        // self.windows.push(window);
        // return id;
        //Err(OsError)
    }
}
impl Default for Windows
{
    fn default() -> Self { Self
    {
        windows: Vec::new(),
        shutdown_after_main_window_closed: true,
    }}
}

pub struct WindowManager;
impl Middleware for WindowManager
{
    fn name(&self) -> &str { "Windows" }

    fn startup(&mut self, app: &mut AppContext) -> CompletionState
    {
        match app.globals.try_init::<Windows>()
        {
            Ok(_) => "Created windows global",
            Err(_) => "Failed to create windows global",
        };
        CompletionState::Completed
    }
    fn shutdown(&mut self, app: &mut AppContext) -> CompletionState
    {
        // todo: destroy all windows
        CompletionState::Completed
    }
    fn run(&mut self, app: &mut AppContext) -> CompletionState
    {
        // todo: event loop
        CompletionState::InProgress
    }
}