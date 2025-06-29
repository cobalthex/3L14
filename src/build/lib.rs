use std::{env, fs};
use std::path::PathBuf;

pub mod winres;

pub struct ProjectDirs
{
    pub root_dir: PathBuf,
    pub out_dir: PathBuf,
}
impl Default for ProjectDirs
{
    fn default() -> Self
    {
        let root_dir: PathBuf = env::var("CARGO_MANIFEST_DIR").expect("! Failed to get project root").into();

        let out_dir =
        {
            // construct with Env:CARGO_MANIFEST_DIR \target\ Env:PROFILE ?
            let mut out_dir: PathBuf = env::var("OUT_DIR").expect("! Failed to get build target dir").into();
            out_dir.push("../../.."); // gross
            out_dir.canonicalize().expect("! Failed to canonicalize Env:OUT_DIR")
        };

        Self
        {
            root_dir,
            out_dir,
        }
    }
}

pub fn build_exe()
{
    winres::generate_windows_resources();

    // codegen?
}