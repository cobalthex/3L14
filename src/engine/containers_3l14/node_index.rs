use bitcode::{Decode, Encode};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct NodeIndex(pub u32);
impl NodeIndex
{
    const NONE: u32 = u32::MAX;

    #[inline] #[must_use] pub const fn none() -> Self { Self(Self::NONE) }
    #[inline] #[must_use] pub const fn some(n: u32) -> Self { Self(n) }

    #[inline] #[must_use] pub const fn is_none(self) -> bool { self.0 == Self::NONE }
    #[inline] #[must_use] pub const fn is_some(self) -> bool { self.0 != Self::NONE }
}
impl Default for NodeIndex
{
    fn default() -> Self { Self::none() }
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn basic()
    {
        assert!(NodeIndex::none().is_none());
        assert!(NodeIndex::some(0).is_some());
        assert!(NodeIndex::some(1).is_some());
        assert!(NodeIndex::some(u32::MAX - 1).is_some());
    }
}
