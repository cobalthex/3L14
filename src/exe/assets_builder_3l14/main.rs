mod core;
mod builders;
mod helpers;

use crate::core::{validate_symbols, AssetsBuilder, AssetsBuilderConfig, BuildRule, ScanError};
use std::path::{Path, PathBuf};
use clap::{Parser, Subcommand};
use log::log;
use nab_3l14::app::{set_panic_hook, AppRun};

#[derive(Debug, Subcommand)]
pub enum CliCommands
{
    #[clap(about = "Build sources into assets")]
    Build
    {
        #[arg(long, group = "build_what")]
        all: bool, // includes symbols

        // extension/type
        #[arg(long, group = "build_what", value_delimiter = ',', num_args = 1..)]
        source: Vec<String>,

        #[arg(long, group = "build_what")]
        symbols: bool,

        #[arg(long)]
        rule: Option<BuildRule>,
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
    let app_run = AppRun::<CliArgs>::startup("Assets Builder", env!("CARGO_PKG_VERSION"));
    set_panic_hook(false);

    let Ok(assets_root) = Path::new("assets").canonicalize() else { return; }; // TODO: error handling
    let src_assets_root = assets_root.join("src");
    let built_assets_root = assets_root.join("build");

    let mut builder_cfg = AssetsBuilderConfig::new(&src_assets_root, &built_assets_root);
    builder_cfg.add_builder(builders::ModelBuilder::new(&assets_root));
    let builder = AssetsBuilder::new(builder_cfg);

    match &app_run.args.command
    {
        CliCommands::Build { all: true, .. } =>
        {
            // TODO: scan assets and build, validate symbols
            todo!();
        },
        CliCommands::Build { symbols: true, .. } =>
        {
            let _validation = validate_symbols(assets_root.join("symbols"));
        }
        CliCommands::Build { all: false, symbols: false, source: sources, rule } =>
        {
            for source in sources
            {
                let src_path = Path::new(&source);
                let build_rule = rule.unwrap_or_default();

                match builder.build_source(src_path, build_rule)
                {
                    Ok(results) =>
                    {
                        log::info!("Successfully built {src_path:?} into {results:#?}"); // log debug?
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