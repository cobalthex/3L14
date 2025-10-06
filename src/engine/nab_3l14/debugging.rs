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

pub fn append_file(path: impl AsRef<std::path::Path>, data: impl AsRef<[u8]>) -> std::io::Result<()>
{
    use std::fs::OpenOptions;
    use std::io::Write;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(data.as_ref())?;
    Ok(())
}
#[macro_export]
macro_rules! append_file
{
    ($file_path:expr, $($arg:tt)*) =>
    (
        $crate::debugging::append_file($file_path, format!($($arg)*)).unwrap()
    )
}