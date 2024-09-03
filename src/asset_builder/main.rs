use clap::Parser;
use game_3l14::engine::assets::AssetPath;
use crate::asset_builder::{AssetBuilder, SourceInputRead};

mod builders;
mod asset_builder;

// TODO: use asset types, make an AssetTypeId ?

// Construct a builder for a given input file extension (omitting .)
pub fn get_builder_for_file_extension(extension: &str) -> Option<impl AssetBuilder>
{
    match extension
    {
        "glb" => Some(builders::ModelBuilder),

        _ => None,
    }
}

#[derive(Debug, Parser)]
struct CliArgs
{
    // A list of asset paths to build
    build: Vec<String>,
}

fn main()
{
    let cli_args = CliArgs::parse();

    for build in cli_args.build
    {

    }
}


/* TODO: how to translate between source and built asset paths */