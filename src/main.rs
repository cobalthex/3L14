#![allow(non_snake_case)]

use winit::{event_loop::EventLoop, dpi::LogicalSize};

mod Engine;
use Engine::{*, middlewares::window_manager};

fn main() {
    let mut app = App::new();

    // todo: construct in window system?
    let event_loop = EventLoop::new();
    app.context.globals.try_add(event_loop).unwrap();

    Engine::middlewares::use_common_middlewares(&mut app);
    Engine::middlewares::use_window_middlewares(&mut app);

    app.middlewares.try_add::<Game>(Game::new()).unwrap();

    app.run();
}

struct Game
{

}
impl Game
{
    pub fn new() -> Self { Self
    {

    }}
}
impl Middleware for Game
{
    fn name(&self) -> &str { "Game" }

    fn startup(&mut self, app: &mut AppContext) -> CompletionState
    {
        app.globals.get_mut::<window_manager::Windows>()
            .unwrap()
            .create_window("Test", LogicalSize { width: 1280, height: 800 });//.ok();

        CompletionState::Completed
    }

    fn shutdown(&mut self, app: &mut AppContext) -> CompletionState {
        CompletionState::Completed
    }

    fn run(&mut self, app: &mut AppContext) -> CompletionState {
        CompletionState::Completed
    }
}
