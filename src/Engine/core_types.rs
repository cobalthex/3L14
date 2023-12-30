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

struct TimeLimit
{
    limit: TickCount,
    deadline: TickCount,
}
impl TimeLimit
{
    pub fn new(limit: TickCount) -> Self { Self{ limit: limit, deadline: TickCount(0) }}
    pub fn get_limit(&self) -> TickCount { self.limit }
    pub fn is_expired(&self) -> bool { Self::now() < self.deadline }
    pub fn start(&mut self) { self.deadline = Self::now() + self.limit }

    fn now() -> TickCount { TickCount(0) } /* TODO */
}

#[macro_export]
macro_rules! runtime_id_u32
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
            static [<PRIVATE_ $type_name:snake:upper _COUNTER>]:
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
                    id: [<PRIVATE_ $type_name:snake:upper _COUNTER>]
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