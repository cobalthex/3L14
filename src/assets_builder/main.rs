mod core;
mod builders;

use crate::core::{AssetsBuilder, AssetsBuilderConfig};
use std::path::Path;
use clap::{Parser, Subcommand};

#[derive(Debug, Subcommand)]
pub enum CliCommands
{
    #[clap(about = "Build sources into assets")]
    Build
    {
        #[arg(long, exclusive = true)]
        all: bool,
        #[arg(long, exclusive = true, value_delimiter = ',', num_args = 1..)]
        source: Vec<String>,

        // build IDs ?
    },
    #[clap(about = "List known source assets and their source ID")]
    Sources,
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
    let cli_args = <CliArgs as clap::Parser>::parse();

    let Ok(assets_root) = Path::new("assets").canonicalize() else { return; }; // TODO: error handling
    let src_assets_root = assets_root.join("src");
    let built_assets_root = assets_root.join("build");

    let mut builder_cfg = AssetsBuilderConfig::new(&src_assets_root, &built_assets_root);
    builder_cfg.add_builder(builders::ModelBuilder);
    builder_cfg.add_builder(builders::TextureBuilder);
    builder_cfg.add_builder(builders::MaterialBuilder);
    builder_cfg.add_builder(builders::ShaderBuilder::new(&src_assets_root));

    let builder = AssetsBuilder::new(builder_cfg);

    match cli_args.command
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
                        eprintln!("Successfully built {src_path:?} into {results:#?}");
                    }
                    Err(err) =>
                    {
                        eprintln!("Failed to build {src_path:?}: {err:#}");
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
    }
}