use std::time::{Instant, Duration};
use parking_lot::RwLock;

#[derive(Debug, Clone, Copy)]
pub struct Time
{
    pub current_time: Instant,
    pub last_time: Instant,
    pub delta_time: Duration,
    pub total_runtime: Duration,
}
impl Time
{
    // calculate the instantaneous fps between now and the previous frame
    pub fn fps(&self) -> f32 { 1.0 / self.delta_time.as_secs_f32() }
}

pub struct Clock
{
    start_time: Instant,
    time: RwLock<Time>,
}

impl Clock
{
    pub const MIN_DURATION: Duration = Duration::new(0, 1); // zero delta may cause issues for some use cases

    pub fn new() -> Self
    {
        let now = Instant::now();
        Self
        {
            start_time: now,
            time: RwLock::new(Time
            {
                current_time: now,
                last_time: now - Self::MIN_DURATION,
                delta_time: Self::MIN_DURATION,
                total_runtime: Self::MIN_DURATION,
            }),
        }
    }

    pub fn tick(&mut self) -> Time
    {
        let mut locked = self.time.write();
        locked.last_time = locked.current_time;
        locked.current_time = Instant::now();
        locked.delta_time = locked.current_time - locked.last_time;
        locked.total_runtime = locked.current_time - self.start_time;
        *locked
    }

    pub fn time(&self) -> Time { *self.time.read() }

    // debug set time?
}
impl Default for Clock
{
    fn default() -> Self { Self::new() }
}

// todo
// struct TimeLimit
// {
//     limit: Duration,
//     deadline: Instant,
// }
// impl TimeLimit
// {
//     pub fn new(limit: Duration) -> Self { Self{ limit: limit, deadline:  }}
//     pub fn get_limit(&self) -> TickCount { self.limit }
//     pub fn is_expired(&self) -> bool { Self::now() < self.deadline }
//     pub fn start(&mut self) { self.deadline = Self::now() + self.limit }

//     fn now() -> TickCount { TickCount(0) } /* TODO */
// }
