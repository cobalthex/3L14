use Engine::{*, middlewares::window::WindowMiddleware};
use wgpu::Surface;
mod Engine;

fn main()
{
    let z = middlewares![Renderer::new(), WindowMiddleware::new()];

    let mut app = App::new(z);

    app.run();
}

struct RendererInternal
{
    backbuffer: Surface,
}


struct Renderer
{
    internal: Option<RendererInternal>,
}
impl Renderer
{
    pub fn new() -> Self { Self
    {
        internal: None,
    }}
}
impl Middleware for Renderer
{
    fn name(&self) -> &str { "Renderer" }

    fn startup(&mut self, app: &mut AppContext) -> CompletionState
    {
        let inst = wgpu::Instance::default();
        //let wnd = app.globals.get::<winit::window::Window>().unwrap();
        // let surface = unsafe { inst.create_surface(wnd) };

        // if self.internal.is_some() { panic!("{} is not none", nameof::name_of!(internal in Self)); }
        // self.internal = Some(RendererInternal
        // {
        //     backbuffer: surface.unwrap(), // TODO: don't unwrap
        // });

        CompletionState::Completed
    }

    fn shutdown(&mut self, _app: &mut AppContext) -> CompletionState {
        CompletionState::Completed
    }

    fn run(&mut self, _app: &mut AppContext) -> CompletionState {
        CompletionState::InProgress
    }
}




