use game_3l14::{engine::{*, middlewares::clock::Clock}, generate_middlewares};
use proc_macros_3l14::GlobalSingleton;
use wgpu::Surface;

#[derive(GlobalSingleton, Debug)]
struct TestMiddleware;
impl TestMiddleware
{
    const fn new() -> Self { Self }
}
impl Middleware for TestMiddleware
{
    fn startup(&self) -> CompletionState { CompletionState::Completed }

    fn shutdown(&self) -> CompletionState { CompletionState::Completed }

    fn run(&self) -> CompletionState
    {
        let now = Clock::get().now();
        println!("The current time is {:?}", now);

        CompletionState::InProgress
    }
}

generate_middlewares![Clock, TestMiddleware];
    // windows: WindowMiddleware,
    // renderer: Renderer,


fn main()
{
    let mut app = App::new(MiddlewaresImpl::new());

    // // let wnd = app.middlewares.windows.create_window(CreateWindow { width: 1920, height: 1080, title: "3L14" }).unwrap();
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
