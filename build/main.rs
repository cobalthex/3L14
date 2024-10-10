use std::{env, fs, io};
use std::ffi::OsStr;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use winres::WindowsResource;

fn main()
{
    eprintln!(">>> Running build scripts <<<");

    if env::var_os("CARGO_CFG_WINDOWS").is_some()
    {
        WindowsResource::new()
        .set_icon("res/App.ico")
            .compile().expect("! Failed to compile windows resource definitions");
    }

    let project_root: PathBuf = env::var("CARGO_MANIFEST_DIR").expect("! Failed to get project root").into();

    // construct with Env:CARGO_MANIFEST_DIR \target\ Env:PROFILE ?
    let mut out_dir: PathBuf = env::var("OUT_DIR").expect("! Failed to get build target dir").into();
    out_dir.push("../../.."); // gross
    out_dir = out_dir.canonicalize().expect("! Failed to canonicalize Env:OUT_DIR");

    let mut assets_symlink_target = out_dir.clone();
    assets_symlink_target.push("assets");
    
    let mut assets_symlink_src = project_root.clone();
    assets_symlink_src.push("assets/build");
    match assets_symlink_src.canonicalize() {
        Ok(src_path) =>
        {
            // TODO: copy in release builds
            match std::fs::symlink_metadata(&assets_symlink_target)
            {
                // don't panic?
                Ok(meta) if meta.is_symlink() => {},
                Ok(_) => panic!("! out-dir asset dir existed but was not a symlink"),

                Err(err) if err.kind() != ErrorKind::NotFound => panic!("! out-dir asset file '{assets_symlink_target:?}' was unreadable: {err:?}"),

                _ => symlink::symlink_dir(assets_symlink_src, assets_symlink_target).expect("! Failed to symlink asset directory"),
            }
        }
        Err(err) => println!("cargo::warning=Failed to find assets build dir: {err}"),
    }


    // if let Some(bin_name) = env::var_os("CARGO_BIN_NAME")
    // {
    //     if bin_name == "assets_builder"
    //     {
    // symlink?
    if let Err(e) = copy_dir_all(Path::new("3rdparty/dxc"), &out_dir, Some(&[OsStr::new("dll")]))
    {
        println!("cargo::warning=Failed to copy DXC: {e}");
    }

    if let Err(e) = copy_dir_all(Path::new("3rdparty/sdl"), &out_dir, Some(&[OsStr::new("dll")]))
    {
        println!("cargo::warning=Failed to copy SDL: {e}");
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
