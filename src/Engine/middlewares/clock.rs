use std::time::{Instant, Duration};
use crate::engine::{middleware::*, core_types::CompletionState};
use proc_macros_3l14::GlobalSingleton;
use parking_lot::RwLock;

#[derive(Debug)]
struct ClockInternal
{
    current_time: Instant,
    last_time: Instant,
    delta_time: Duration,
}

#[derive(GlobalSingleton, Debug)]
pub struct Clock(RwLock<Option<ClockInternal>>);

impl Clock
{
    fn new() -> Self
    {
        Self(RwLock::new(None))
    }

    fn tick(&self)
    {
        let mut lock = self.0.write();
        let internal = lock.as_mut().unwrap();
        internal.last_time = internal.current_time;
        internal.current_time = Instant::now();
        internal.delta_time = internal.current_time - internal.last_time;
    }

    pub fn now(&self) -> Instant { self.0.read().as_ref().unwrap().current_time }
    pub fn delta(&self) -> Duration { self.0.read().as_ref().unwrap().delta_time }
}

impl Middleware for Clock
{
    fn startup(&self) -> CompletionState
    {
        let now = Instant::now();
        let delta = Duration::new(0, 1); // smallest time unit so that it's non-zero
        let internal = ClockInternal
        {
            current_time: now,
            last_time: now - delta,
            delta_time: delta,
        };
        *self.0.write() = Some(internal);


        CompletionState::Completed
    }
    fn shutdown(&self) -> CompletionState
    {
        CompletionState::Completed
    }
    fn run(&self) -> CompletionState
    {
        self.tick();
        CompletionState::InProgress
    }
}