use std::hash::{Hash, Hasher};
use crate::asset_builder::{AssetBuildError, AssetBuilder, BuiltAsset, SourceInput};

pub struct ModelBuilder;

impl AssetBuilder for ModelBuilder
{
    fn format_hash<H: Hasher>(hasher: &mut H)
    {
        "Initial test".hash(hasher)
    }

    fn builder_hash<H: Hasher>(hasher: &mut H)
    {
        "V1".hash(hasher)
    }

    fn build_asset(&mut self, input: SourceInput) -> Result<BuiltAsset, AssetBuildError>
    {
        Err(AssetBuildError::InvalidInputData)
    }
}