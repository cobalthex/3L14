use std::sync::Arc;

pub trait Asset
{
}

#[derive(Default)]
pub struct AssetRef<TAsset: Asset>
{
    asset: Arc<TAsset>,
}

pub struct AssetCache
{
}
