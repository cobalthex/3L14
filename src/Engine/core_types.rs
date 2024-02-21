use std::fmt;
use std::fmt::Formatter;
use serde::Serializer;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TickCount(pub u64);
impl TickCount
{
    pub fn system_ticks() -> Self { Self(12345) } // todo
}
impl Default for TickCount
{
    fn default() -> Self { Self(0) }
}
impl std::ops::Add for TickCount
{
    type Output = Self;
    fn add(self, other: Self) -> Self { Self(self.0 + other.0) }
}
impl std::ops::Sub for TickCount
{
    type Output = Self;
    fn sub(self, other: Self) -> Self { Self(self.0 - other.0) }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FrameNumber(pub u64);
impl FrameNumber
{
    pub fn increment(&mut self) -> Self { self.0 += 1; *self }
}
impl std::fmt::Display for FrameNumber
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result
    {
        f.serialize_u64(self.0)
    }
}
impl std::ops::Add for FrameNumber
{
    type Output = Self;
    fn add(self, other: Self) -> Self { Self(self.0 + other.0) }
}
impl std::ops::Sub for FrameNumber
{
    type Output = Self;
    fn sub(self, other: Self) -> Self { Self(self.0 - other.0) }
}

#[derive(PartialEq, Clone, Copy)]
pub enum CompletionState
{
    InProgress,
    Completed,
}
impl std::ops::BitAnd for CompletionState
{
    type Output = CompletionState;

    fn bitand(self, rhs: Self) -> Self::Output
    {
        match self
        {
            Self::Completed => rhs,
            Self::InProgress => Self::InProgress,
        }
    }
}
impl std::ops::BitAndAssign for CompletionState
{
    fn bitand_assign(&mut self, rhs: Self)
    {
        *self = *self & rhs;
    }
}
impl std::ops::BitOr for CompletionState
{
    type Output = CompletionState;

    fn bitor(self, rhs: Self) -> Self::Output
    {
        match self
        {
            Self::Completed => Self::Completed,
            Self::InProgress => rhs,
        }
    }
}
impl std::ops::BitOrAssign for CompletionState
{
    fn bitor_assign(&mut self, rhs: Self)
    {
        *self = *self | rhs;
    }
}

#[macro_export]
macro_rules! define_runtime_id_u32
{
    ($type_name:ident) =>
    {
        #[doc="An opaque ID type that can be used to uniquely identify objects during runtime"]
        #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $type_name
        {
            id: u32,
        }
        paste::paste!
        {
            static [<AUTOGEN_PRIV__ $type_name:snake:upper _COUNTER>]:
                std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
        }
        impl $type_name
        {
            pub const INVALID: Self = Self::invalid();
            pub const fn invalid() -> Self { Self { id: 0 } }

            paste::paste!
            {
                #[doc="Generate a new unique ID. Note: there are no ordering guarantees around ID generation"]
                pub fn new() -> Self { Self
                {
                    id: [<AUTOGEN_PRIV__ $type_name:snake:upper _COUNTER>]
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                }}
            }

            pub fn as_u32(&self) -> u32 { self.id }
        }
        impl std::default::Default for $type_name
        {
            fn default() -> Self { Self::new() }
        }
    }
}