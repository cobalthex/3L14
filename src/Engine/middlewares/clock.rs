use std::time::{Instant, Duration};
use crate::engine::{Middleware, core_types::CompletionState};
use crate::AppContext;

#[derive(Debug)]
pub struct Time
{
    current_time: Instant,
    last_time: Instant,
    delta_time: Duration,

    // real world time?
}

impl Time
{
    pub fn new() -> Self
    {
        let now = Instant::now();
        let delta = Duration::new(0, 1); // smallest time unit so that it's non-zero
        Self {
            current_time: now,
            last_time: now - delta,
            delta_time: delta,
        }
    }

    fn tick(&mut self)
    {
        self.last_time = self.current_time;
        self.current_time = Instant::now();
        self.delta_time = self.current_time - self.last_time;
    }
}

pub struct Clock;
impl Middleware<AppContext> for Clock
{
    fn startup(&mut self) -> CompletionState
    {
        // app.globals.try_add(Time::new()).expect("Time is managed by the Clock middleware");
        CompletionState::Completed
    }
    fn shutdown(&mut self) -> CompletionState
    {
        // app.globals.remove::<Time>();
        CompletionState::Completed
    }
    fn run(&mut self) -> CompletionState
    {
        // app.globals.get_mut::<Time>().unwrap().tick();
        CompletionState::InProgress
    }
}