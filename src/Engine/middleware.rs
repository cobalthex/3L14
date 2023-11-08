use chrono::Local;
use chrono::format::{DelayedFormat, StrftimeItems};

use super::app::AppContext;
use super::core_types::CompletionState;

//todo: use a log frmework
pub fn log_time<'a>() -> DelayedFormat<StrftimeItems<'a>> { Local::now().format("[%Y-%m-%d %H:%M:%S.%3f]") }

/// Middlewares can interact with the app but are otherwise isolated. These are what drive the app
/// Any data that needs to be shared should be stored in globals
/// Only private data to the middleware should be stored with the middleware
/// Middlewares are stored/evaluated in order of insertion
pub trait Middleware
{
    fn name(&self) -> &str; // a canonical name for this middleware

    // async startup/shutdown?

    fn startup(&mut self, _app: &mut AppContext) -> CompletionState { CompletionState::Completed } // Initialize this middleware, this is called every tick and should return Completed once ready
    fn shutdown(&mut self, _app: &mut AppContext) -> CompletionState { CompletionState::Completed } // Uninitialize this middleware, this is called every tick and should return Completed once torn down
    fn run(&mut self, _app: &mut AppContext) -> CompletionState { CompletionState::InProgress } // Run this middleware, this is called every tick and should return Completed when the app should shutdown (any middleware completing will cause the app to shut down)
}

// heterogeneous list of middlewares
pub trait Middlewares
{
    fn startup(&mut self, context: &mut AppContext) -> CompletionState;
    fn shutdown(&mut self, context: &mut AppContext) -> CompletionState;
    fn run(&mut self, context: &mut AppContext) -> CompletionState;

    fn each<F>(&self, func: F) where F: Fn(&dyn Middleware);
}
pub struct NoMiddlewares;
impl Middlewares for NoMiddlewares
{
    fn startup(&mut self, _context: &mut AppContext) -> CompletionState { CompletionState::Completed }
    fn shutdown(&mut self, _context: &mut AppContext) -> CompletionState { CompletionState::Completed }
    fn run(&mut self, _context: &mut AppContext) -> CompletionState { CompletionState::InProgress }

    fn each<F>(&self, _func: F) where F: Fn(&dyn Middleware) { }
}

// recursive middlewares container
impl<Head, Tail> Middlewares for (Head, Tail)
where
    Head: Middleware,
    Tail: Middlewares
{
    fn startup(&mut self, context: &mut AppContext) -> CompletionState
    {
        self.0.startup(context) & self.1.startup(context)
    }
    fn shutdown(&mut self, context: &mut AppContext) -> CompletionState
    {
        self.0.shutdown(context) & self.1.shutdown(context)
    }
    fn run(&mut self, context: &mut AppContext) -> CompletionState
    {
        match self.0.run(context)
        {
            CompletionState::Completed =>
            {
                eprintln!("{} Middleware '{}' requested shutdown", log_time(), self.0.name());
                CompletionState::Completed
            }
            CompletionState::InProgress =>
            {
                self.1.run(context)
            }
        }
    }

    fn each<F>(&self, func: F) where F: Fn(&dyn Middleware) { func(&self.0); self.1.each(&func); }
}

// stolen from frunk
#[macro_export]
macro_rules! middlewares {
    () => { $crate::middleware::NoMiddlewares };
    (...$tail:expr) => { $tail };
    ($h:expr) => { $crate::middlewares![$h,] };
    ($h:expr, $($tok:tt)*) => {
        (
            $h,
            $crate::middlewares![$($tok)*],
        )
    };
}
