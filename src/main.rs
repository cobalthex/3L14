// use engine::{*, middlewares::window::{WindowMiddleware, CreateWindow}};
use engine::{middlewares::*, middlewares::{window::*, clock::*}, app::*, core_types::*};
use wgpu::Surface;
mod engine;

generate_middlewares!
{
    // clock: Clock,
    windows: WindowMiddleware,
    // renderer: Renderer,
}

#[derive(Debug, Default)]
pub struct AppContext
{
    state: AppRunState, // todo: not pub
    tick_count: TickCount, // todo: not pub

    // shared, global data
}
#[allow(dead_code)] // todo: remove
impl AppContext
{
    pub fn new() -> Self { Default::default() }

    pub fn state(&self) -> AppRunState { self.state }
    pub fn tick_count(&self) -> TickCount { self.tick_count }
}

fn main()
{
    let mut app = App::new(MiddlewaresImpl::new(), AppContext::new());

    // let wnd = app.middlewares.windows.create_window(CreateWindow { width: 1920, height: 1080, title: "3L14" }).unwrap();
    app.run();
}

struct RendererInternal
{
    backbuffer: Surface,
}


// struct Renderer
// {
//     internal: Option<RendererInternal>,
// }
// impl Renderer
// {
//     pub fn new() -> Self { Self
//     {
//         internal: None,
//     }}
// }
// impl Middleware<AppContext> for Renderer
// {
//     fn startup(&mut self, app: &mut AppContext) -> CompletionState
//     {
//         let inst = wgpu::Instance::default();
//         //let wnd = app.globals.get::<winit::window::Window>().unwrap();
//         // let surface = unsafe { inst.create_surface(wnd) };

//         // if self.internal.is_some() { panic!("{} is not none", nameof::name_of!(internal in Self)); }
//         // self.internal = Some(RendererInternal
//         // {
//         //     backbuffer: surface.unwrap(), // TODO: don't unwrap
//         // });

//         CompletionState::Completed
//     }

//     fn shutdown(&mut self, _app: &mut AppContext) -> CompletionState {
//         CompletionState::Completed
//     }

//     fn run(&mut self, _app: &mut AppContext) -> CompletionState {
//         CompletionState::InProgress
//     }
// }
