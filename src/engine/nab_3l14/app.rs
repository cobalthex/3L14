use proc_macros_3l14::FancyEnum;
use std::fmt::Debug;
use std::io::Read;
use std::panic::PanicHookInfo;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::OnceLock;

pub enum AppFolder
{
    Assets,
    Cache,
    PerMachineSettings,
    PerUserSettings
}

// TODO: move out of here
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
#[cfg(debug_assertions)]
const TEST_VAL: u32 = iif_debug!(10, 0);

#[macro_export]
macro_rules! const_assert
{
    ($name:ident : $cond:expr) => { ::paste::paste!( const [<_ $name>]: () = const { assert!($cond); }; ); };
    ($name:ident : $cond:expr, $($arg:tt)+) => { ::paste::paste!( const [<_ $name>]: () = const { assert!($cond, $($arg)+); }; ); };

    ($cond:expr) => { const _: () = { assert!($cond); }; };
    ($cond:expr, $($arg:tt)+) => { const _: () = { assert!($cond, $($arg)+); }; };
}

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

pub trait CliArgs: clap::Parser + Debug { }
impl<T: clap::Parser + Debug> CliArgs for T { }

fn crate_name<T>() -> &'static str // hacky
{
    let name = std::any::type_name::<T>();
    &name[0..name.find("::").unwrap()]
}

#[derive(Debug)]
pub struct AppRun<TCliArgs: CliArgs>
{
    pub app_name: &'static str,
    pub version_str: &'static str,

    pub start_time: chrono::DateTime<chrono::Local>,
    pub args: TCliArgs,
    pub pid: u32,
    pub is_elevated: bool,

    pub app_dir: PathBuf, // where the app exe is located (distinct from working dir)

    exit_reason: AtomicI32,
}
impl<TCliArgs: CliArgs> AppRun<TCliArgs>
{
    pub fn startup(app_name: &'static str, app_version: &'static str) -> Self
    {
        #[cfg(debug_assertions)]
        let default_log_levels = (log::LevelFilter::Warn, log::LevelFilter::Debug);
        #[cfg(not(debug_assertions))]
        let default_log_levels = (log::LevelFilter::Warn, log::LevelFilter::Info);
        let app_crate = crate_name::<TCliArgs>();
        colog::basic_builder()
            .filter_level(default_log_levels.0)
            .filter_module(app_crate, default_log_levels.1)
            .filter_module(crate_name::<Self>(), default_log_levels.1)
            // TODO: automate this
            .filter_module("asset_3l14", default_log_levels.1)
            .filter_module("containers_3l14", default_log_levels.1)
            .filter_module("debug_3l14", default_log_levels.1)
            .filter_module("graphics_3l14", default_log_levels.1)
            .filter_module("input_3l14", default_log_levels.1)
            .filter_module("latch_3l14", default_log_levels.1)
            .filter_module("math_3l14", default_log_levels.1)
            .filter_module("nab_3l14", default_log_levels.1)
            .filter_module("world_3l14", default_log_levels.1)
            .parse_default_env()
            .init();

        let app_dir =
        {
            let mut path = std::env::current_exe().expect("Failed to get the bin dir");
            path.pop();
            path
        };

        let app_run = Self
        {
            app_name,
            version_str: app_version,
            start_time: chrono::Local::now(),
            args: TCliArgs::parse(),
            pid: std::process::id(),
            #[cfg(not(target_family="wasm"))]
            is_elevated: is_root::is_root(),
            #[cfg(target_family="wasm")]
            is_elevated: false,
            app_dir,
            exit_reason: AtomicI32::new(ExitReason::NormalExit as i32),
        };

        log::info!(target: app_crate,
            "=== Starting {} v{} [{}] (PID {}){} at {} ===",
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

    // Get the path to an app-specific folder
    // Cache results where possible
    pub fn get_app_folder(&self, folder: AppFolder) -> PathBuf
    {
        match folder
        {
            AppFolder::Assets => self.app_dir.join("assets"),
            // todo: directories::project_dirs
            AppFolder::Cache => todo!(),
            AppFolder::PerMachineSettings => todo!(),
            AppFolder::PerUserSettings => todo!(),
        }
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

pub trait FatalErrorCode: Debug
{
    fn error_code(&self) -> u16;
}

#[derive(Clone, Copy)]
struct Panic<'p>(&'p PanicHookInfo<'p>);
impl FatalErrorCode for Panic<'_> { fn error_code(&self) -> u16 { 1u16 } }
impl Debug for Panic<'_>
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result
    {
        // TODO: payload_as_str()
        if let Some(payload_str) = self.0.payload().downcast_ref::<&str>()
        {
            f.write_fmt(format_args!("{payload_str}\n"))?;
        }
        else if let Some(payload_str) = self.0.payload().downcast_ref::<String>()
        {
            f.write_fmt(format_args!("{payload_str}\n"))?;
        }

        if let Some(location) = self.0.location()
        {
            Debug::fmt(&location, f)?
        }

        Ok(())
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
            print!("Press any key to exit... ");
            let mut input = [0u8];
            let _ = std::io::stdin().read(&mut input);
        }

        // todo: proper panic value?
        fatal_error(FatalError::Panic, Panic(panic))
    }));
}

#[derive(FancyEnum, Clone, Copy, PartialEq)]
pub enum FatalError
{
    #[enum_prop(short_name = "PNC")]
    Panic = 0,
    #[enum_prop(short_name = "MEM")]
    Memory,
}

pub static FATAL_ERROR_CB: OnceLock<fn(&str)> = OnceLock::new();

// TODO: perhaps this can generate the fatal_error from the calling crate
// Exit the game with a fatal error,
pub fn fatal_error(fatal_error: FatalError, code: impl FatalErrorCode) -> !
{
    let mut error_msg = format!("{}-{:04X}", fatal_error.short_name(), code.error_code());
    if cfg!(debug_assertions)
    {
        error_msg.push_str(&format!("\n\n{:#?}", &code));
    }

    eprintln!("!!! FATAL: {}", error_msg);
    if let Some(error_cb) = FATAL_ERROR_CB.get()
    {
        error_cb(&error_msg);
    }

    eprintln!("Exiting (PID {}) at {} with reason {:?}",
              std::process::id(),
              chrono::Local::now(),
              ExitReason::Panic);

    std::process::exit(ExitReason::Panic as i32)
}