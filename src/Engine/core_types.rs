#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TickCount(pub u64);

impl Default for TickCount
{
    fn default() -> Self { Self(0) }
}

pub enum CompletionState
{
    InProgress,
    Completed,
    // Error(ErrorType?) -- will deconstruct
}
impl From<CompletionState> for bool
{
    fn from(value: CompletionState) -> Self
    {
        match value
        {
            CompletionState::Completed => true,
            CompletionState::InProgress => false,
        }
    }
}

pub mod Errors
{
    #[derive(Debug)]
    pub struct AlreadyExists;
}
