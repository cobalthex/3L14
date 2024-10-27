mod core;
mod builders;
mod helpers;

use crate::core::{AssetMetadata, AssetsBuilder, AssetsBuilderConfig, ScanError};
use std::path::{Path, PathBuf};
use clap::{Parser, Subcommand};
use log::log;
use game_3l14::AppRun;

#[derive(Debug, Subcommand)]
pub enum CliCommands
{
    #[clap(about = "Build sources into assets")]
    Build
    {
        #[arg(long, exclusive = true)]
        all: bool,
        // extension/type
        #[arg(long, exclusive = true, value_delimiter = ',', num_args = 1..)]
        source: Vec<String>,

        // build IDs ?
    },
    #[clap(about = "List known source files and their source ID")]
    Sources,
    #[clap(about = "List known assets and info about them")]
    Assets,
    
    // server mode - watch for fs changes and auto build new assets
}

#[derive(Debug, Parser)]
struct CliArgs
{
    #[command(subcommand)]
    command: CliCommands,

    // todo:
    //  - reset source build config

}

fn main()
{
    let app_run = AppRun::<CliArgs>::startup("Assets Builder");
    game_3l14::set_panic_hook(false);

    let Ok(assets_root) = Path::new("assets").canonicalize() else { return; }; // TODO: error handling
    let src_assets_root = assets_root.join("src");
    let built_assets_root = assets_root.join("build");

    let mut builder_cfg = AssetsBuilderConfig::new(&src_assets_root, &built_assets_root);
    builder_cfg.add_builder(builders::ModelBuilder);
    builder_cfg.add_builder(builders::TextureBuilder);
    builder_cfg.add_builder(builders::MaterialBuilder);
    builder_cfg.add_builder(builders::ModelBuilder::new(&src_assets_root));

    let builder = AssetsBuilder::new(builder_cfg);

    match &app_run.args.command
    {
        CliCommands::Build { all: true, .. } =>
        {
            todo!();
        },
        CliCommands::Build { all: false, source: sources } =>
        {
            for source in sources
            {
                let src_path = Path::new(&source);

                match builder.build_source(src_path)
                {
                    Ok(results) =>
                    {
                        log::info!("Successfully built {src_path:?} into {results:#?}");
                    }
                    Err(err) =>
                    {
                        log::error!("Failed to build {src_path:?}: {err:#}");
                    }
                }
            }
        }
        CliCommands::Sources =>
        {
            for source in builder.scan_sources()
            {
                println!("{source:?}");
            }
        }
        CliCommands::Assets =>
        {
            for asset in builder.scan_assets()
            {
                match asset
                {
                    Ok(ass) => println!("{:?} {:?} {:?}",
                        ass.1.key.asset_type(),
                        ass.1.key,
                        ass.1.source_path),
                    Err(err) => println!("{err}"),
                }
            }
        }
    }
}