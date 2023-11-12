#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TickCount(pub u64);

impl Default for TickCount
{
    fn default() -> Self { Self(0) }
}

#[derive(PartialEq)]
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
            Self::InProgress => self,
        }
    }
}
impl std::ops::BitOr for CompletionState
{
    type Output = CompletionState;

    fn bitor(self, rhs: Self) -> Self::Output
    {
        match self
        {
            Self::Completed => self,
            Self::InProgress => rhs,
        }
    }
}