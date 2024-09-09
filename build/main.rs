use std::{env, fs, io};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use winres::WindowsResource;

fn main()
{
    if env::var_os("CARGO_CFG_WINDOWS").is_some()
    {
        WindowsResource::new()
            .set_icon("res/App.ico")
            .compile().expect("! Failed to compile windows resource definitions");
    }

    let out_dir = env::var("OUT_DIR").expect("! Failed to get build target dir");

    let mut assets_symlink = PathBuf::new();
    assets_symlink.push(&out_dir);
    assets_symlink.push("assets");

    match std::fs::symlink_metadata(&assets_symlink)
    {
        Ok(meta) if meta.is_symlink() => {},
    Ok(_) => panic!("! out-dir assets file existed but was not a symlink"),
        Err(err) if err.kind() != ErrorKind::NotFound => panic!("! out-dir assets file '{assets_symlink:?}' was unreadable: {err:?}"),
        _ => symlink::symlink_dir("assets/", assets_symlink).expect("! Failed to symlink asset directory"),
    }

    if let Some(bin_name) = env::var_os("CARGO_BIN_NAME")
    {
        if bin_name == "assets_builder"
        {
            // symlink?
            let _ = copy_dir_all(Path::new("3rdparty/dxc"), &out_dir);
        }
    }
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<usize>
{
    let mut copied_count = 0usize;

    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)?
    {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir()
        {
            copied_count += copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
        else
        {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
            copied_count += 1;
        }
    }

    Ok(copied_count)
}
