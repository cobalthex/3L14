// Symbols are unique values that act as sentinels in code or data
pub trait Symbol { const INVALID: Self; }

macro_rules! define_symbol {

    ($name:ident) =>
    {
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, serde::Deserialize, bitcode::Encode, bitcode::Decode)]
        pub struct $name(u32);
        impl $name
        {
            // cfg(test) doesn't work
            #[inline] #[must_use]
            pub const fn test(n: char) -> Self { Self(0xbe577e57 + n as u32) }
        }
        impl Symbol for $name
        {
            const INVALID: Self = Self(0);
        }
    }
}

define_symbol!(Signal);
define_symbol!(Ident);
