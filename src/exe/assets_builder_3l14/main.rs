mod core;
mod builders;
mod helpers;

use crate::core::{validate_symbols, AssetsBuilder, AssetsBuilderConfig, BuildRule};
use std::path::Path;
use clap::{Parser, Subcommand};
use unicase::UniCase;
use asset_3l14::AssetTypeId;
use latch_3l14::block_meta::BlockBuildMeta;
use nab_3l14::app::{set_panic_hook, AppRun, ExitReason};

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


        #[arg(long, alias = "rule")]
        build_rule: Option<BuildRule>,
        // build IDs ?
    },
    #[clap(about = "List known source files and their source ID")]
    Sources,
    #[clap(about = "List known assets and info about them")]
    Assets,

    #[clap(about = "Reset the import settings of one or more source assets")]
    ResetImport
    {
        #[arg(long, value_delimiter = ',', num_args = 1..)]
        source: Vec<String>,
    },

    #[clap(about = "List all known latch types")]
    DumpLatchTypes
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
    let built_assets_root = assets_root.join("built");

    let mut builder_cfg = AssetsBuilderConfig::new(&src_assets_root, &built_assets_root);
    builder_cfg.add_builder(builders::ModelBuilder);
    builder_cfg.add_builder(builders::TextureBuilder);
    builder_cfg.add_builder(builders::ShaderBuilder::new(src_assets_root.join("shaders"), None).unwrap());
    builder_cfg.add_builder(builders::CircuitBuilder::new());
    let builder = AssetsBuilder::new(builder_cfg);

    match &app_run.args.command
    {
        CliCommands::Build { all: true, build_rule: rule, .. } =>
        {
            let _todo = builder.build_all(rule.unwrap_or(BuildRule::OnlyIfChanged));
            let _todo = validate_symbols(assets_root.join("symbols"));
        },
        CliCommands::Build { symbols: true, .. } =>
        {
            let _validation = validate_symbols(assets_root.join("symbols"));
        }
        CliCommands::Build { source: sources, build_rule: rule, .. } =>
        {
            for source in sources
            {
                let src_path = Path::new(&source);
                let build_rule = rule.unwrap_or(BuildRule::ForceBuildAll);

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
                log::info!("{source:?}");
            }
        }

        CliCommands::Assets =>
        {
            for asset in builder.scan_assets()
            {
                match asset
                {
                    Ok(ass) => log::info!("{:?} {:?} {:?}",
                        ass.1.key.asset_type(),
                        ass.1.key,
                        ass.1.source_path),
                    Err(err) => log::error!("{err}"),
                }
            }
        }

        CliCommands::ResetImport { source: sources } =>
        {
            for source in sources
            {
                match builder.reset_import(Path::new(&source))
                {
                    Ok(_) => log::debug!("Regenerated source meta for {source}"),
                    Err(err) => log::error!("Failed to regenerate source meta for {source}: {err}"),
                }
            }
        }

        CliCommands::DumpLatchTypes =>
        {
            log::info!("Impulses\n========");
            for impulse in inventory::iter::<BlockBuildMeta<0>>()
            {
                log::info!("{}", impulse.type_name);
            }
            log::info!("\nLatches\n=======");
            for latch in inventory::iter::<BlockBuildMeta<1>>()
            {
                log::info!("{}", latch.type_name);
            }
        }
    }
}
