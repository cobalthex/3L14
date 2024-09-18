use std::any::TypeId;
use super::*;
use std::collections::HashMap;
use std::io::{Read, Seek};
use std::sync::Arc;
use crate::engine::ShortTypeName;

pub struct AssetLoadRequest
{
    pub asset_key: AssetKey,
    pub input: Box<dyn AssetRead>, // TODO: memory mapped buffer?
    storage: Arc<AssetsStorage>,

    // timer?
    // is_reloading?
}
impl AssetLoadRequest
{
    // Load a dependency
    // Assets/lifecyclers are responsible for tracking/maintaining dependency references
    #[must_use]
    pub fn load_dependency<D: Asset>(&self, asset_key: AssetKey) -> AssetHandle<D>
    {
        // pattern matches Assets::load()
        self.storage.enqueue_load(asset_key, |h| AssetLifecycleRequest::LoadFileBacked(h))
    }

    // Load a dependency from a specified source
    // Assets/lifecyclers are responsible for tracking/maintaining dependency references
    #[must_use]
    pub fn load_dependency_from<D: Asset, R: AssetRead + 'static>(
        &self,
        asset_key: AssetKey,
        input_data: R // take box?
    ) -> AssetHandle<D>
    {
        // pattern matches Assets::load_from()
        self.storage.enqueue_load(asset_key, |h| AssetLifecycleRequest::LoadFromMemory(h, Box::new(input_data)))
    }
}

pub trait AssetLifecycler: Sync + Send
{
    type Asset: Asset;

    /// Get or create an asset payload for the requested asset
    fn load(&self, request: AssetLoadRequest) -> AssetPayload<Self::Asset>;
    // reload ?
}

// only for use internally in the asset system, mostly just utility methods for interacting with generics
pub(super) trait UntypedAssetLifecycler: Sync + Send
{
    // takes ownership of the untyped handle
    fn load_untyped(&self, storage: Arc<AssetsStorage>, untyped_handle: UntypedAssetHandle, input: Box<dyn AssetRead>);

    // takes ownership of the untyped handle
    fn error_untyped(&self, untyped_handle: UntypedAssetHandle, error: AssetLoadError);
}
impl<A: Asset, L: AssetLifecycler<Asset=A>> UntypedAssetLifecycler for L
{
    fn load_untyped(&self, storage: Arc<AssetsStorage>, untyped_handle: UntypedAssetHandle, input: Box<dyn AssetRead>)
    {
        let result = self.load(AssetLoadRequest
        {
            asset_key: untyped_handle.as_ref().key(),
            input,
            storage,
        });

        untyped_handle.as_ref().store_payload::<A>(result);
    }

    // this doesn't really make sense here
    // todo: can probably have an untyped 'asset handle' similar to the untyped lifecycler
    fn error_untyped(&self, untyped_handle: UntypedAssetHandle, error: AssetLoadError)
    {
        untyped_handle.as_ref().store_payload::<A>(AssetPayload::Unavailable(error));
    }
}

pub(super) struct RegisteredAssetType
{
    pub type_id: TypeId,
    pub type_name: &'static str,
    pub dealloc_fn: fn(UntypedAssetHandle),
}
impl RegisteredAssetType
{
    fn drop_fn_impl<A: Asset>(untyped_handle: UntypedAssetHandle)
    {
        unsafe { untyped_handle.dealloc::<A>(); }
    }
}

// TODO: make this a trait?
#[derive(Default)]
pub struct AssetLifecyclers
{
    pub(super) lifecyclers: HashMap<AssetTypeId, Box<dyn UntypedAssetLifecycler>>,
    pub(super) registered_asset_types: HashMap<AssetTypeId, RegisteredAssetType>,
}
impl AssetLifecyclers
{
    pub fn add_lifecycler<A: Asset, L: AssetLifecycler<Asset=A> + UntypedAssetLifecycler + 'static>(mut self, lifecycler: L) -> Self
    {
        // warn/fail on duplicates?
        self.lifecyclers.insert(A::asset_type(), Box::new(lifecycler));
        self.registered_asset_types.insert(A::asset_type(), RegisteredAssetType
        {
            type_id: TypeId::of::<A>(),
            type_name: A::short_type_name(),
            dealloc_fn: |h| unsafe { h.dealloc::<A>() },
        });
        self
    }
}

pub trait AssetRead: Read + Seek + Send { }
impl<T: Read + Seek + Send> AssetRead for T { }

pub(super) enum AssetLifecycleRequest
{
    StopWorkers,
    Drop(UntypedAssetHandle),
    LoadFileBacked(UntypedAssetHandle), // loads the file pointed by the asset path
    LoadFromMemory(UntypedAssetHandle, Box<dyn AssetRead>),
}
