mod model_builder;
pub use model_builder::ModelBuilder;

////////

use game_3l14::engine::assets::AssetPath;
use crate::asset_builder::SourceInputRead;

pub fn build_asset<S: AssetPath>(output_asset_path: S, input: Box<dyn SourceInputRead>)
{
    // TODO
}