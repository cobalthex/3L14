use game_3l14::{engine::{*, middlewares::clock::Clock}, generate_middlewares};
use proc_macros_3l14::GlobalSingleton;
use wgpu::Surface;

use game_3l14::engine::state_logic;

generate_middlewares![Clock];
    // windows: WindowMiddleware,
    // renderer: Renderer,

trait Job
{
    fn execute();
}

struct Scheduler
{

}

impl Scheduler
{
    fn new() -> Self { Self
    {

    }}

    fn run()
    {

    }
}

fn main()
{


    // let mut app = App::new(MiddlewaresImpl::new());

    // // // let wnd = app.middlewares.windows.create_window(CreateWindow { width: 1920, height: 1080, title: "3L14" }).unwrap();
    // app.run();
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
