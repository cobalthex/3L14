use super::core_types::CompletionState;
use chrono::{*, format::*};

//todo: use a log frmework
pub fn log_time<'a>() -> DelayedFormat<StrftimeItems<'a>> { Local::now().format("[%Y-%m-%d %H:%M:%S.%3f]") }

// Singletons should implement interior mutability
pub trait GlobalSingleton
{
    fn global_init();
    // fn global_uninit();
    fn get<'s>() -> &'s Self;
}

/// Middlewares can interact with the app but are otherwise isolated. These are what drive the app
/// Any data that needs to be shared should be stored in globals
/// Only private data to the middleware should be stored with the middleware
/// Middlewares are stored/evaluated in order of insertion
/// Middlewares should use interior mutability
pub trait Middleware: GlobalSingleton
{
    // async startup/shutdown?
    fn startup(&self) -> CompletionState { CompletionState::Completed } // Initialize this middleware, this is called every tick and should return Completed once ready
    fn shutdown(&self) -> CompletionState { CompletionState::Completed } // Uninitialize this middleware, this is called every tick and should return Completed once torn down
    fn run(&self) -> CompletionState { CompletionState::InProgress } // Run this middleware, this is called every tick and should return Completed when the app should shutdown (any middleware completing will cause the app to shut down)
}

pub trait Middlewares
{
    fn startup(&mut self) -> CompletionState;
    fn shutdown(&mut self) -> CompletionState;
    fn run(&mut self) -> CompletionState;
}

// Generate middlewares for the app
// example usag`e: generate_middlewares!{a: FooMiddleware, b: BarMiddleware}
// creates a struct named MiddlewaresImpl that implements Middlewares
#[macro_export]
macro_rules! generate_middlewares
{
    ($($middleware:ty),* $(,)?) =>
    {
        struct MiddlewaresImpl;

        impl MiddlewaresImpl
        {
            fn new() -> Self
            {
                $( <$middleware>::global_init(); )*
                Self
            }
        }

        impl Middlewares for MiddlewaresImpl
        {
            fn startup(&mut self) -> CompletionState
            {
                return CompletionState::Completed
                    $( & <$middleware>::get().startup() )*;
            }
            fn shutdown(&mut self) -> CompletionState
            {
                return CompletionState::Completed
                    $( & <$middleware>::get().shutdown() )*;
            }
            fn run(&mut self) -> CompletionState
            {
                // this may create a lot of code duplication
                $(
                    if <$middleware>::get().run() == CompletionState::Completed
                    {
                        eprintln!("{} Middleware {} requested shutdown",
                            log_time(), stringify!($middleware));
                        return CompletionState::Completed;
                    }
                )*
                CompletionState::InProgress
            }
        }
    };
}
