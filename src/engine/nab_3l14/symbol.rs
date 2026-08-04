// Symbols are unique values that act as sentinels in code or data
pub trait Symbol
{
    type Repr;
    const TYPE_NAME: &'static str;
    const TYPE_NAME_HASH: u32;

    #[must_use]
    fn from_raw(raw: SYMBOLS_REPR) -> Self;
}
pub type SYMBOLS_REPR = u32;

macro_rules! define_symbol {

    ($name:ident) =>
    {
        #[repr(transparent)]
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, bitcode::Encode, bitcode::Decode)]
        pub struct $name(core::num::NonZero<<Self as Symbol>::Repr>);
        impl $name
        {
            // cfg(test) doesn't work
            #[inline] #[must_use]
            pub const fn test(n: char) -> Self { Self(unsafe { core::num::NonZero::new_unchecked(0xbe577e57 + n as <Self as Symbol>::Repr) }) }
        }
        impl Symbol for $name
        {
            type Repr = SYMBOLS_REPR;
            const TYPE_NAME: &'static str = stringify!($name);
            const TYPE_NAME_HASH: u32 = proc_macros_3l14::ident_hash32!($name);
            // Construct this symbol from a raw u32 value. This should only be used by deserialization code

            #[inline]
            fn from_raw(raw: SYMBOLS_REPR) -> Self { Self(unsafe { core::num::NonZero::new_unchecked(raw) }) }
        }
    }
}

define_symbol!(Signal);
define_symbol!(Ident);