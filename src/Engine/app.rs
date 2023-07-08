use chrono::{format::{DelayedFormat, StrftimeItems}, Local};
use super::*;

fn log_time<'a>() -> DelayedFormat<StrftimeItems<'a>> { Local::now().format("[%Y-%m-%d %H:%M:%S.%3f]") }

#[derive(Copy, Clone, PartialEq,Debug)]
pub enum AppRunState
{
    NotRunning,
    StartingUp,
    ShuttingDown,
    Running,
}
impl Default for AppRunState
{
    fn default() -> Self { AppRunState::NotRunning }
}

#[derive(Debug, Default)]
pub struct AppContext
{
    state: AppRunState,
    tick_count: TickCount,

    /// data that can be accessed by any middleware, unique per type
    pub globals: Globals,
}
#[allow(dead_code)] // todo: remove
impl AppContext
{
    pub fn state(&self) -> AppRunState { self.state }
    pub fn tick_count(&self) -> TickCount { self.tick_count }
}

#[derive(Debug, Default)]
pub struct App
{
    pub context: AppContext,
    pub middlewares: Middlewares,
}

impl App
{
    pub fn new() -> Self { Self::default() }

    fn run_once(&mut self)
    {
        self.context.tick_count.0 += 1;

        match self.context.state
        {
            AppRunState::NotRunning => return,
            AppRunState::StartingUp =>
            {
                // todo: measure startup/shutdown time, abort if too slow?
                let mut all_ready = true;
                for (_, middleware) in self.middlewares.iter_mut()
                {
                    all_ready &= Into::<bool>::into(middleware.startup(&mut self.context));
                }
                if all_ready
                {
                    self.context.state = AppRunState::Running;
                    eprintln!("{} App looping", log_time());
                }
            }
            AppRunState::ShuttingDown =>
            {
                let mut all_ready = true;
                for (_, middleware) in self.middlewares.iter_mut()
                {
                    all_ready &= Into::<bool>::into(middleware.shutdown(&mut self.context));
                }
                if all_ready
                {
                    self.context.state = AppRunState::NotRunning;
                    eprintln!("{} App shut down", log_time());
                }
            }
            AppRunState::Running =>
            {
                let mut any_finished = false;
                for (_, middleware) in self.middlewares.iter_mut()
                {
                    let did_finish = Into::<bool>::into(middleware.run(&mut self.context));
                    if did_finish
                    {
                        eprintln!("{} Middleware '{}' requested shutdown", log_time(), middleware.name());
                        any_finished = true;
                    }
                }
                if any_finished
                {
                    self.context.state = AppRunState::ShuttingDown;
                    eprintln!("{} App Shutting down", log_time());
                }
            }
        }
    }

    pub fn run(&mut self)
    {
        assert_eq!(AppRunState::NotRunning, self.context.state);
        self.context.state = AppRunState::StartingUp;

        eprintln!("{} App starting up", log_time());

        while self.context.state != AppRunState::NotRunning
        {
            self.run_once();
        }

        // event_loop.run(move |event, _, control_flow|
        // {
        //     control_flow.set_poll(); // set_poll for continuous

        //     match event
        //     {
        //         Event::WindowEvent
        //         {
        //             event: WindowEvent::CloseRequested,
        //             window_id,
        //         } if window_id == window.id() => control_flow.set_exit(),

        //         // // prob want to use device events for this
        //         // Event::WindowEvent
        //         // {
        //         //     window_id,
        //         //     event: WindowEvent::KeyboardInput { input: KeyboardInput { virtual_keycode: Some(keycode), state, .. }, .. /* synthetic? */ },
        //         // } if window_id == window.id() =>
        //         // {
        //         //     app.input_state.keyboard_state.set_key(keycode, state == ElementState::Pressed)
        //         // },
        //         // Event::WindowEvent
        //         // {
        //         //     window_id,
        //         //     event: WindowEvent::ModifiersChanged(mods),
        //         // } if window_id == window.id() =>
        //         // {
        //         //     app.input_state.keyboard_state.modifiers = mods;
        //         // }
        //         // TODO: mouse events

        //         Event::NewEvents(_) =>
        //         {
        //         }
        //         Event::MainEventsCleared =>
        //         {
        //             app.run_once();
        //         },
        //         _ => (),
        //     }
        // });
    }
}