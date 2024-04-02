use std::env;
use std::io::ErrorKind;
use std::path::PathBuf;
use winres::WindowsResource;

fn main()
{
    if env::var_os("CARGO_CFG_WINDOWS").is_some()
    {
        WindowsResource::new()
            .set_icon("res/App.ico")
            .compile().expect("! Failed to compile windows resource definitions");
    }

    let mut assets_symlink = PathBuf::new();
    assets_symlink.push(env::var("OUT_DIR").expect("! Failed to get build target dir"));
    assets_symlink.push("assets");

    match std::fs::symlink_metadata(&assets_symlink)
    {
        Ok(meta) if meta.is_symlink() => {},
        Ok(_) => panic!("! out-dir assets file existed but was not a symlink"),
        Err(err) if err.kind() != ErrorKind::NotFound => panic!("! out-dir assets file '{assets_symlink:?}' was unreadable: {err:?}"),
        _ => symlink::symlink_dir("assets/", assets_symlink).expect("! Failed to symlink asset directory"),
    }
}