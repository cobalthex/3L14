pub mod engine;

extern crate proc_macros_3l14;

use std::env::Args;
use std::fmt::Debug;
use std::io::Read;
use std::process::ExitCode;
use std::sync::atomic::{AtomicI32, Ordering};

#[macro_export]
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

#[macro_export]
macro_rules! const_assert
{
    ($($tt:tt)*) =>
    {
        const _: () = assert!($($tt)*);
    }
}

pub const TEST_VAL: u32 = iif_debug!(10, 0);

fn shitty_join<I>(separator: &str, iter: I) -> String
where I: Iterator,
      I::Item: std::fmt::Display
{
    let mut out = String::new();
    let mut first = true;
    for i in iter
    {
        match first
        {
            true => { first = false; }
            false => { out.push_str(separator); }
        };
        out.push_str(i.to_string().as_str());
    }
    out
}

#[derive(Debug)]
#[repr(i32)]
pub enum ExitReason
{
    Unset = !1, // this should never be set
    NormalExit = 0,
    Panic = -99,
}
impl std::process::Termination for ExitReason
{
    fn report(self) -> ExitCode
    {
        (self as u8).into()
    }
}

pub trait CliArgs: clap::Parser + Debug { }
impl<T: clap::Parser + Debug> CliArgs for T { }

#[derive(Debug)]
pub struct AppRun<TCliArgs: CliArgs>
{
    pub app_name: &'static str,
    pub version_str: &'static str,

    pub start_time: chrono::DateTime<chrono::Local>,
    pub args: TCliArgs,
    pub pid: u32,
    pub is_elevated: bool,

    exit_reason: AtomicI32,
}
impl<TCliArgs: CliArgs> AppRun<TCliArgs>
{
    pub fn startup(app_name: &'static str) -> Self
    {
        colog::init();

        let app_run = Self
        {
            app_name,
            version_str: env!("CARGO_PKG_VERSION"),
            start_time: chrono::Local::now(),
            args: TCliArgs::parse(),
            pid: std::process::id(),
            is_elevated: is_root::is_root(),
            exit_reason: AtomicI32::new(ExitReason::NormalExit as i32),
        };

        log::info!(target: "app",
            ": Starting {} v{} [{}] (PID {}){} at {}",
            app_run.app_name,
            app_run.version_str,
            shitty_join(" ", std::env::args()),
            app_run.pid,
            if app_run.is_elevated { " elevated" } else { "" },
            app_run.start_time);

        app_run
    }

    pub fn set_exit_reason(&self, exit_reason: ExitReason)
    {
        self.exit_reason.store(exit_reason as i32, Ordering::SeqCst);
    }
    pub fn get_exit_reason(&self) -> ExitReason
    {
        unsafe { std::mem::transmute(self.exit_reason.load(Ordering::SeqCst)) }
    }
}
impl<TCliArgs: CliArgs> Drop for AppRun<TCliArgs>
{
    fn drop(&mut self)
    {
        log::info!(target: "app",
            "Exiting {} (PID {}) at {} with reason {:?}",
            self.app_name,
            self.pid,
            chrono::Local::now(),
            self.get_exit_reason());
    }
}

pub fn set_panic_hook(wait_for_exit: bool)
{
    let default_panic_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic|
    {
        default_panic_hook(panic);

        if wait_for_exit
        {
            println!("<<< Press enter to exit >>>");
            let _ = std::io::stdin().read(&mut [0u8]); // wait to exit
        }

        eprintln!("Exiting (PID {}) at {} with reason {:?}",
                  std::process::id(),
                  chrono::Local::now(),
                  ExitReason::Panic);

        std::process::exit(ExitReason::Panic as i32);
    }));
}