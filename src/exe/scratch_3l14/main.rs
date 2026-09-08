use std::error::Error;
use std::thread::sleep;
use std::time::Duration;
use clap::Parser;
use asset_3l14::{Asset, AssetKey, AssetLifecycler, AssetLifecyclers, AssetLoadRequest, AssetTypeId, Assets, AssetsConfig, StubAsset};
use nab_3l14::app::{AppFolder, AppRun, ExitReason};

#[derive(Parser, Debug)]
struct CliArgs
{
}
impl nab_3l14::app::CliArgs for CliArgs { }

struct TextureStub;
impl StubAsset for TextureStub
{
    const ASSET_TYPE: AssetTypeId = AssetTypeId::Texture;
    fn new() -> Self { Self }
}

fn main() -> ExitReason
{
    let app_run = AppRun::<CliArgs>::startup("Scratch", "0.1.0");

    let assets_config = AssetsConfig
    {
        assets_root: app_run.get_app_folder(AppFolder::Assets),
        enable_fs_watcher: false,
    };
    let assets = Assets::new(AssetLifecyclers::default()
        .add_stub_lifecycler::<TextureStub>(),
        assets_config);

    let tex = assets.load::<TextureStub>(AssetKey::from(0x00600000e0c7c3f3));
    sleep(Duration::from_secs(3));

    println!("!!! {:#?}", tex);

    app_run.get_exit_reason()
}
