use std::path::Path;
use clap::Parser;
use futures::future::err;
use crate::asset_builder::{AssetBuilder, SourceInputRead};
use crate::assets_builder::{AssetsBuilder, AssetsBuilderConfig};

mod builders;

mod asset_builder;

mod assets_builder;


// TODO: use asset types, make an AssetTypeId ?

#[derive(Debug, Parser)]
struct CliArgs
{
    // A list of asset paths to build
    #[arg(short, long)]
    build: Vec<String>, // TODO: should this input source or built asset paths?
}

fn main()
{
    let Ok(assets_root) = Path::new("assets").canonicalize() else { return; }; // TODO: error handling
    let src_assets_root = assets_root.join("src");
    let built_assets_root = assets_root.join("build");

    let mut builder_cfg = AssetsBuilderConfig::new(&src_assets_root, &built_assets_root);
    builder_cfg.add_builder(builders::TextureBuilder);
    builder_cfg.add_builder(builders::ModelBuilder);

    eprintln!("Assets Builder :: version:{:x}", builder_cfg.builders_version_hash());
    let builder = AssetsBuilder::new(builder_cfg);

    let cli_args = CliArgs::parse();

    for build in cli_args.build
    {
        let src_path = Path::new(&build);

        match builder.build_assets(src_path)
        {
            Ok(_) =>
            {
                eprintln!("Successfully built {src_path:?}");
            }
            Err(err) =>
            {
                eprintln!("Failed to build {src_path:?}: {err:?}");
            }
        }
    }
}