use crate::AssetTypeId;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use unicase::UniCase;

pub const ASSET_FILE_EXTENSION: UniCase<&'static str> = UniCase::unicode("ass");
pub const ASSET_META_FILE_EXTENSION: UniCase<&'static str> = UniCase::unicode("mass");

pub trait Asset: Sync + Send + 'static
{
    fn asset_type() -> AssetTypeId;

    // Have all dependencies of this asset been loaded? (always true if no dependencies)
    fn all_dependencies_loaded(&self) -> bool { true }
}

pub trait AssetPath: AsRef<str> + Hash + Display + Debug { }
impl<T> AssetPath for T where T: AsRef<str> + Hash + Display + Debug { }

pub trait HasAssetDependencies
{
    fn asset_dependencies_loaded(&self) -> bool;
}

pub trait AssetDebugData: Asset
{
    type DebugData;
}