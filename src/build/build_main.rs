mod winres;
mod crates_codegen;

use std::{env, fs, io};
use std::ffi::OsStr;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

fn main()
{
    eprintln!(">>> Running build scripts <<<");

    if env::var_os("CARGO_CFG_WINDOWS").is_some()
    {
        winres::generate_windows_resources();
    }

    let project_root: PathBuf = env::var("CARGO_MANIFEST_DIR").expect("! Failed to get project root").into();
    let out_dir =
    {
        // construct with Env:CARGO_MANIFEST_DIR \target\ Env:PROFILE ?
        let mut out_dir: PathBuf = env::var("OUT_DIR").expect("! Failed to get build target dir").into();
        out_dir.push("../../.."); // gross
        out_dir.canonicalize().expect("! Failed to canonicalize Env:OUT_DIR")
    };
    let generated_dir: PathBuf = out_dir.join("generated");
    fs::create_dir_all(&generated_dir).expect("! Failed to create generated dir");

    crates_codegen::crates_codegen(&generated_dir);

    let mut assets_symlink_target = out_dir.clone();
    assets_symlink_target.push("assets");
    
    let mut assets_symlink_src = project_root.clone();
    assets_symlink_src.push("assets/build");
    match assets_symlink_src.canonicalize()
    {
        Ok(src_path) =>
        {
            // TODO: copy in release builds
            match std::fs::symlink_metadata(&assets_symlink_target)
            {
                // don't panic?
                Ok(meta) if meta.is_symlink() => {},
                Ok(_) => panic!("! out-dir asset dir existed but was not a symlink"),

                Err(err) if err.kind() != ErrorKind::NotFound => panic!("! out-dir asset file '{assets_symlink_target:?}' was unreadable: {err:?}"),

                _ => symlink::symlink_dir(src_path, assets_symlink_target).expect("! Failed to symlink asset directory"),
            }
        }
        Err(err) =>
        {
            println!("cargo::warning=Failed to find assets build dir: {err}\nMaking new one");
            fs::create_dir_all(assets_symlink_target).expect("Failed to create empty assets target dir");
        },
    }

    let arch_name =
    {
        if cfg!(target_arch = "x86_64") { "x64" }
        else if cfg!(target_arch = "aarch64") { "arm64" }
        else { panic!("Unsupported architecture") }
    };

    // if let Some(bin_name) = env::var_os("CARGO_BIN_NAME")
    // {
    //     if bin_name == "assets_builder"
    //     {
    // symlink?
    if let Ok(thirdparty_dir) = Path::new("3rdparty").canonicalize()
    {
        let mut dxc_path = thirdparty_dir.join("dxc");
        dxc_path.push(arch_name);
        if let Err(e) = copy_dir_all(dxc_path, &out_dir, Some(&[OsStr::new("dll")]))
        {
            println!("cargo::warning=Failed to copy DXC: {e}");
        }

        let mut sdl_path = thirdparty_dir.join("sdl");
        sdl_path.push(arch_name);
        println!(r"cargo:rustc-link-search={}", sdl_path.display());
        if let Err(e) = copy_dir_all(sdl_path, &out_dir, Some(&[OsStr::new("dll")]))
        {
            println!("cargo::warning=Failed to copy SDL: {e}");
        }
    }
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>, file_exts: Option<&[&OsStr]>) -> io::Result<usize>
{
    let mut copied_count = 0usize;

    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)?
    {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir()
        {
            copied_count += copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()), file_exts)?;
        }
        else
        {
            if let Some(exts) = file_exts
            {
                // hacky slop
                if !exts.contains(&entry.path().extension().unwrap_or(OsStr::new(""))) { continue; }
            }

            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
            copied_count += 1;
        }
    }

    Ok(copied_count)
}
