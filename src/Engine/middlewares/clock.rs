use std::time::{Instant, Duration};
use std::mem::MaybeUninit;
use crate::engine::{middleware::*, core_types::CompletionState};
use proc_macros_3l14::GlobalSingleton;
use parking_lot::RwLock;

#[derive(Debug, Clone, Copy)]
pub struct Time
{
    current_time: Instant,
    last_time: Instant,
    delta_time: Duration,
}

#[derive(GlobalSingleton, Debug)]
pub struct Clock(RwLock<MaybeUninit<Time>>);

impl Clock
{
    fn new() -> Self
    {
        Self(RwLock::new(MaybeUninit::zeroed()))
    }

    fn tick(&self)
    {
        let mut locked = self.0.write();
        let mut internal = unsafe { locked.assume_init() };
        internal.last_time = internal.current_time;
        internal.current_time = Instant::now();
        internal.delta_time = internal.current_time - internal.last_time;
        locked.write(internal);
    }

    pub fn time(&self) -> Time { unsafe { self.0.read().assume_init() } }
}

impl Middleware for Clock
{
    fn startup(&self) -> CompletionState
    {
        let now = Instant::now();
        let delta = Duration::new(0, 1); // smallest time unit so that it's non-zero
        let internal = Time
        {
            current_time: now,
            last_time: now - delta,
            delta_time: delta,
        };
        self.0.write().write(internal);

        CompletionState::Completed
    }
    fn shutdown(&self) -> CompletionState
    {
        unsafe { self.0.write().assume_init_drop(); }
        CompletionState::Completed
    }
    fn run(&self) -> CompletionState
    {
        self.tick();
        CompletionState::InProgress
    }
}