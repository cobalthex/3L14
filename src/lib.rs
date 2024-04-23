pub mod engine;

extern crate proc_macros_3l14;

use std::env::Args;

macro_rules! iif_debug {
    ($a:expr, $b:expr) =>
    {
        match cfg!(debug_assertions)
        {
            true => $a,
            false => $b,
        }
    };
}

pub const TEST_VAL: u32 = iif_debug!(10, 0);

#[cfg_attr(debug_assertions, derive(Debug))]
pub struct AppRun
{
    pub app_name: &'static str,
    pub version_str: &'static str,

    pub start_time: chrono::DateTime<chrono::Local>,
    pub args: Args,
    pub pid: u32,
    pub is_elevated: bool,
}
impl Default for AppRun
{
    fn default() -> Self
    {
        Self
        {
            app_name: "3L14",
            version_str: env!("CARGO_PKG_VERSION"),

            start_time: chrono::Local::now(),
            args: std::env::args(),
            pid: std::process::id(),
            is_elevated: is_root::is_root(),
        }
        // todo: print start message here?
    }
}