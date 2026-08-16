use crate::{Ash, AssetTypeId};
use std::fmt::{Debug, Display};
use std::hash::Hash;
use bitcode::{DecodeOwned, Encode};
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

pub trait Asset: Send + Sync + 'static
{
    // How the primary, standard structured data is stored on-disk
    type StructuredData: Encode + DecodeOwned;

    // How (optional) related debug data is stored on-disk.
    // Debug data is stored in a separate file and only available with the `asset_debug_data` feature.
    type DebugData: Encode + DecodeOwned;

    fn asset_type() -> AssetTypeId;

    // Have all dependencies of this asset been loaded? (always true if no dependencies)
    fn all_dependencies_loaded(&self) -> bool { true }
}

pub trait AssetPath: AsRef<str> + Hash + Display + Debug { }
impl<T> AssetPath for T where T: AsRef<str> + Hash + Display + Debug { }

pub trait HasAssetDependencies
{
    fn asset_dependencies_loaded(self) -> bool;
}
impl<'i, A: Asset, I: Iterator<Item=Ash<A>>> HasAssetDependencies for &'i mut I
{
    fn asset_dependencies_loaded(self) -> bool
    {
        self.all(|a| a.is_loaded_recursive())
    }
}
// TODO: unify dependency_loaded function names