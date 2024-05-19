use super::*;

pub trait AssetLifecycler<A: Asset>: Sync
{
    /// Get or create an asset payload for the requested asset
    /// Note: the Asset system will track lifetimes itself, so lifecyclers are not required to maintain their own internal storage
    fn create_or_update(&self, request: AssetLoadRequest<A>); // fills in the output of request
    // reload ?
}

pub trait AssetLifecyclerLookup<A: Asset>
{
    fn lifecycler(&self) -> & impl AssetLifecycler<A>;
}

pub trait AssetLifecyclers: Sync + Send
{
}
