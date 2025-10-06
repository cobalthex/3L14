use std::fmt::{Debug, Formatter};
use std::time::Duration;
use crate::timing::FSeconds;

pub type TimeNanos = u64; // nanoseconds

#[derive(Default, Copy, Clone, PartialEq)]
pub struct Stopwatch
{
    start: TimeNanos, // if high bit is clear, stopwatch is not running
    elapsed: TimeNanos,
}
impl Stopwatch
{
    const ACTIVE_TEST_BIT: TimeNanos = 1 << 63;

    #[inline]
    pub fn restart(&mut self, now: TimeNanos)
    {
        self.start = now | Self::ACTIVE_TEST_BIT;
        self.elapsed = 0;
    }

    #[inline]
    pub fn set_elapsed_time(&mut self, elapsed: Duration)
    {
        let nanos_u64 = elapsed.as_nanos() as u64;
        debug_assert!(nanos_u64 < Self::ACTIVE_TEST_BIT);
        self.elapsed = nanos_u64;
    }

    #[inline] #[must_use]
    pub fn is_running(&self) -> bool
    {
        (self.start & Self::ACTIVE_TEST_BIT) != 0
    }

    #[inline] #[must_use]
    pub fn elapsed(&self, now: TimeNanos) -> Duration
    {
        debug_assert!(now < Self::ACTIVE_TEST_BIT);

        match self.is_running()
        {
            true => Duration::from_nanos(now - (self.start & !Self::ACTIVE_TEST_BIT) + self.elapsed),
            false => Duration::from_nanos(self.elapsed),
        }
    }
    #[inline] #[must_use]
    pub fn elapsed_secs(&self, now: TimeNanos) -> FSeconds
    {
        // faster but introduces more error: FSeconds((self.elapsed / 1_000_000_000) as f32)
        FSeconds(self.elapsed(now).as_secs_f32())
    }
    // elapsed_secs_fast vs precise?

    #[inline]
    pub fn start(&mut self, now: TimeNanos)
    {
        debug_assert!(now < Self::ACTIVE_TEST_BIT);
        self.start = now | Self::ACTIVE_TEST_BIT;
    }
    #[inline]
    pub fn stop(&mut self, now: TimeNanos)
    {
        if self.is_running()
        {
            self.elapsed += now - self.start;
            self.start &= !Self::ACTIVE_TEST_BIT;
        }
    }

    #[inline]
    pub fn reset(&mut self)
    {
        self.start = 0;
        self.elapsed = 0;
    }
}
impl Debug for Stopwatch
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        f.debug_struct("Stopwatch")
            .field("Is running", &self.is_running())
            .field("Start time", &Duration::from_nanos(self.start & !Self::ACTIVE_TEST_BIT).as_secs_f64())
            .field("Elapsed time", &Duration::from_nanos(self.elapsed).as_secs_f64())
            .finish()
    }
}

// stopwatch takes TimeSource? two versions of TimeSource, one static one instanced?

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    pub fn initial_state()
    {
        let stopwatch = Stopwatch::default();
        assert!(!stopwatch.is_running());
        assert_eq!(stopwatch.elapsed(0), Duration::default());
        assert_eq!(stopwatch.elapsed(12345), Duration::default());
    }

    #[test]
    pub fn elapsed()
    {
        let mut stopwatch = Stopwatch::default();
        let test_time = Duration::from_secs(123);
        stopwatch.set_elapsed_time(test_time);
        assert_eq!(stopwatch.elapsed(0), test_time);
        assert_eq!(stopwatch.elapsed(12345), test_time);
        assert_eq!(stopwatch.elapsed_secs(12345), FSeconds(123.0));
    }

    // start, stop
    // reset, restart
}