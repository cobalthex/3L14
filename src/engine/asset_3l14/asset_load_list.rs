use crate::{Ash, Asset, AssetSnapshot};
use crate::asset_handle::{AshInnerHeader, ErasedAsh};

#[must_use]
pub struct AssetLoadList
{
    // Invariant: ErasedAsh is holding a single, strong reference to the asset
    pending: Vec<(ErasedAsh, unsafe fn(ErasedAsh) -> Option<ErasedAsh>)>,
}
impl AssetLoadList
{
    pub fn new() -> Self
    {
        Self { pending: Vec::new(), }
    }

    // Check if an asset is loaded, returning the handle back if it is not
    #[must_use]
    unsafe fn check_pending<A: Asset>(erased: ErasedAsh) -> Option<ErasedAsh>
    {
        let ash = Ash::<A>::attach_from(erased);
        if ash.is_pending() { Some(ash.into_inner()) } else { None }
    }

    // Push a single asset into the list. This does not check dependencies

    pub fn push<A: Asset>(&mut self, asset: Ash<A>)
    {
        unsafe { self.pending.push((asset.into_inner(), Self::check_pending::<A>)); }
    }

    // TODO: Decide on how to handle errors

    // Check and prune any completed loads. Returns true once all assets have loaded
    #[must_use]
    pub fn check(&mut self) -> bool
    {
        while let Some((erased, load_fn)) = self.pending.pop()
        {
            if let Some(still_pending) = unsafe { load_fn(erased) }
            {
                self.pending.push((still_pending, load_fn));
                return false;
            }
        }
        return true;
    }
}
impl Drop for AssetLoadList
{
    fn drop(&mut self)
    {
        while let Some((erased, _)) = self.pending.pop()
        {
            AshInnerHeader::enqueue_drop(erased.header());
        }
    }
}