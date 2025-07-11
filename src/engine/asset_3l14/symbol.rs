pub trait Symbol { const INVALID: Self; }

macro_rules! define_symbol {

    ($name:ident) =>
    {
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
        pub struct $name(pub u32);
        impl $name
        {
        }
        impl Symbol for $name
        {
            pub const INVALID: Self = Self(0);
        }
    }
}

define_symbol!(Signal);
