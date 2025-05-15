#[macro_export]
macro_rules! debug_panic
{
    ( $msg:expr ) =>
    {
        #[cfg(debug_assertions)]
        panic!($msg)
    }
}