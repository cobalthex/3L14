use super::*;

pub trait AssetLifecycler<A: Asset, L: AssetLifecyclers>: Sync
{
    /// Get or create an asset payload for the requested asset
    /// Note: the Asset system will track lifetimes itself, so lifecyclers are not required to maintain their own internal storage
    fn create_or_update(&self, request: AssetLoadRequest<A, L>);
    // reload ?
}
impl<A: Asset, L: AssetLifecyclers> dyn AssetLifecycler<A, L>
{
    pub(super) unsafe fn downcast_unsafe(handle: UntypedHandleInner) -> AssetHandle<A>
    {
        AssetHandle::clone_from_untyped(handle)
    }
}

pub trait AssetLifecyclerLookup<A: Asset, L: AssetLifecyclers = Self>
{
    fn lifecycler(&self) -> &impl AssetLifecycler<A, L>;
}

pub trait AssetLifecyclers: Sync + Send
{
}
