use std::fmt;
use std::fmt::{Display, Formatter};

// TODO: clean up this file

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TickCount(pub u64);
impl TickCount
{
    pub fn system_ticks() -> Self { Self(12345) } // todo
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
pub struct RenderFrameNumber(pub u64);
impl RenderFrameNumber
{
    pub fn increment(&mut self) -> Self { self.0 += 1; *self }
}
impl Display for RenderFrameNumber
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result
    {
        Display::fmt(&self.0, f)
    }
}
impl std::ops::Add for RenderFrameNumber
{
    type Output = Self;
    fn add(self, other: Self) -> Self { Self(self.0 + other.0) }
}
impl std::ops::Sub for RenderFrameNumber
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

#[derive(Debug)]
pub enum ToggleState
{
    Off,
    On,
    Toggle,
}

#[derive(Debug)]
pub enum Progress
{
    InProgress,
    Finished,
}

#[derive(Debug)]
pub enum FallibleProgress
{
    InProgress,
    Finished,
    Failed,
}