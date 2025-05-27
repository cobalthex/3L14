#[macro_export]
macro_rules! debug_panic
{
    ($($arg:tt)*) =>
    {
        if cfg!(debug_assertions)
        {
            panic!($($arg)*)
        }
    }
}