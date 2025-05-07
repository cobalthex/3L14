use std::cmp::Ordering;
use std::ops::{Add, Div, Mul, Sub};
use std::time::Duration;
use bitcode::{Decode, Encode};

macro_rules! generate_time_primitive
{
    ($name:ident, $type:ty) =>
    {
        #[derive(Debug, Default, Clone, Copy, PartialEq, PartialOrd, Encode, Decode)]
        pub struct $name(pub $type);
        impl Ord for $name
        {
            fn cmp(&self, other: &Self) -> Ordering
            {
                self.0.partial_cmp(&other.0).unwrap()
            }
        }
        impl Eq for $name { }
        impl Add for $name
        {
            type Output = Self;
            fn add(self, rhs: Self) -> Self::Output { Self(self.0 + rhs.0) }
        }
        impl Sub for $name
        {
            type Output = Self;
            fn sub(self, rhs: Self) -> Self::Output { Self(self.0 - rhs.0) }
        }
        impl Mul for $name
        {
            type Output = Self;
            fn mul(self, rhs: Self) -> Self::Output { Self(self.0 * rhs.0) }
        }
        impl Div for $name
        {
            type Output = Self;
            fn div(self, rhs: Self) -> Self::Output { Self(self.0 / rhs.0) }
        }
    };
}

generate_time_primitive!(FSeconds, f32);
generate_time_primitive!(FMilliseconds, f32);

impl From<FSeconds> for FMilliseconds { fn from(sec: FSeconds) -> Self { Self(sec.0 * 1_000.0) } }
impl From<FMilliseconds> for FSeconds { fn from(ms: FMilliseconds) -> Self { Self(ms.0 / 1_000.0) } }

impl From<FMilliseconds> for Duration { fn from(ms: FMilliseconds) -> Self { Self::from_millis(ms.0 as u64) } }
impl From<FSeconds> for Duration { fn from(sec: FSeconds) -> Self { Self::from_secs_f32(sec.0) } }
