use chrono::Local;
use chrono::format::{DelayedFormat, StrftimeItems};

use super::core_types::CompletionState;

//todo: use a log frmework
pub fn log_time<'a>() -> DelayedFormat<StrftimeItems<'a>> { Local::now().format("[%Y-%m-%d %H:%M:%S.%3f]") }

/// Middlewares can interact with the app but are otherwise isolated. These are what drive the app
/// Any data that needs to be shared should be stored in globals
/// Only private data to the middleware should be stored with the middleware
/// Middlewares are stored/evaluated in order of insertion
pub trait Middleware
{
    // async startup/shutdown?
    fn startup(&mut self) -> CompletionState { CompletionState::Completed } // Initialize this middleware, this is called every tick and should return Completed once ready
    fn shutdown(&mut self) -> CompletionState { CompletionState::Completed } // Uninitialize this middleware, this is called every tick and should return Completed once torn down
    fn run(&mut self) -> CompletionState { CompletionState::InProgress } // Run this middleware, this is called every tick and should return Completed when the app should shutdown (any middleware completing will cause the app to shut down)
}

pub trait Singleton
{
    fn init<'s>() -> &'s mut Self;
    fn uninit();
    fn get<'s>() -> Option<&'s mut Self>;
}

pub trait Middlewares
{
    fn startup(&mut self) -> CompletionState;
    fn shutdown(&mut self) -> CompletionState;
    fn run(&mut self) -> CompletionState;

    fn each<F>(&self, func: F) where F: Fn(&dyn Middleware);
}

// Generate middlewares for the app
// example usage: generate_middlewares!{a: FooMiddleware, b: BarMiddleware}
// creates a struct named MiddlewaresImpl that implements Middlewares
#[macro_export]
macro_rules! generate_middlewares
{
    ($($member:ident : $type:ty),* $(,)?) =>
    {
        struct MiddlewaresImpl
        {
            $( $member: $type, )*
        }

        impl MiddlewaresImpl
        {
            pub fn new() -> Self
            {
                Self
                {
                    $( $member: <$type>::new(), )*
                }
            }
        }

        impl Middlewares for MiddlewaresImpl
        {
            fn startup(&mut self) -> CompletionState
            {
                return CompletionState::Completed
                    $( & self.$member.startup(context) )*;
            }
            fn shutdown(&mut self, context: &mut AppContext) -> CompletionState
            {
                return CompletionState::Completed
                    $( & self.$member.shutdown(context) )*;
            }
            fn run(&mut self, context: &mut AppContext) -> CompletionState
            {
                // this may create a lot of code duplication
                $(
                    if self.$member.run(context) == CompletionState::Completed
                    {
                        eprintln!("{} Middleware {}:{} requested shutdown", log_time(), stringify!($member), stringify!($type));
                        return CompletionState::Completed;
                    }
                )*
                CompletionState::InProgress
            }

            fn each<F>(&self, func: F) where F: Fn(&dyn Middleware<AppContext>)
            {
                $( func(&self.$member); )*
            }
        }
    };
}
