use crate::AssetTypeId;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use bitcode::{Decode, DecodeOwned, Encode};
use unicase::UniCase;
use proc_macros_3l14::FancyEnum;

#[derive(FancyEnum)]
pub enum AssetFileType // TODO: better name?
{
    #[enum_prop(file_extension="ass")]
    Asset,
    #[enum_prop(file_extension="mass")]
    MetaData,
    #[enum_prop(file_extension="dass")]
    DebugData,
}

pub trait Asset: Send + 'static
{
    type DebugData: Encode + DecodeOwned;

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
