use Engine::{*, middlewares::window::{WindowMiddleware, CreateWindow}};
use wgpu::Surface;
mod Engine;

trait TFoo
{
    fn foo(&self);
}
struct Z;
impl TFoo for Z
{
    fn foo(&self) {
        println!("!!!");
    }
}

macro_rules! generate_middlewares
{
    ($($member:ident : $type:ty),*) =>
    {
        struct ZMiddlewares
        {
            $(
                $member: $type,
            )*
        }

        impl ZMiddlewares
        {
            fn new() -> Self
            {
                Self
                {
                    $(
                        $member: <$type>::new(),
                    )*
                }
            }

            fn iterate_over_members(&self)
            {
                $(
                    println!("{}", self.$member.name());
                )*
            }
        }
    };
}

generate_middlewares!
{
    a: WindowMiddleware,
    b: Renderer
}
struct Zap
{
    pub context: AppContext,
    pub middlewares: ZMiddlewares,
}
impl Zap
{
    fn new() -> Self
    {
        Self { context: AppContext::default(), middlewares: ZMiddlewares::new() }
    }
}

fn main()
{
    let z = Zap::new();
    z.middlewares.iterate_over_members();

    let mwares = middlewares![Renderer::new(), WindowMiddleware::new()];

    let wnd = WindowMiddleware::create_window(&mwares.1.0, CreateWindow { width: 1920, height: 1080, title: "3L14" }).unwrap();

    let mut app = App::new(mwares);

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
