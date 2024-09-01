use super::*;
use std::any::TypeId;
use std::collections::HashMap;
use std::io::{Read, Seek};
use std::path::PathBuf;
use std::sync::Arc;
use unicase::UniCase;

pub struct AssetLoadRequest
{
    pub input: Box<dyn AssetReader>,
    storage: Arc<AssetsStorage>,

    // timer?
    // is_reloading?
}
impl AssetLoadRequest
{
    // Assets/lifecyclers are responsible for tracking/maintaining references

    #[must_use]
    pub fn load_dependency<D: Asset, S: AssetPath>(&self, asset_path: &S) -> AssetHandle<D>
    {
        // pattern matches Assets::load()
        self.storage.enqueue_load(asset_path, false,
                                  || AssetLifecycleRequestKind::LoadFileBacked)
    }

    #[must_use]
    pub fn load_dependency_from<D: Asset, S: AssetPath, R: AssetReader + 'static>(
        &self,
        asset_path: &S,
        input_data: R, // take box?,
        update_if_exists: bool,
    ) -> AssetHandle<D>
    {
        // pattern matches Assets::load_from()
        self.storage.enqueue_load(asset_path, update_if_exists,
                                  || AssetLifecycleRequestKind::LoadFromMemory(Box::new(input_data)))
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
    fn load_untyped(&self, storage: Arc<AssetsStorage>, untyped_handle: UntypedHandleInner, input: Box<dyn AssetReader>);

    // takes ownership of the untyped handle
    fn error_untyped(&self, untyped_handle: UntypedHandleInner, error: AssetLoadError);

    // call an asset's destructor, be very sure that you're at a 0 refcount
    fn drop_untyped(&self, untyped_handle: &UntypedHandleInner);
}
impl<A: Asset, L: AssetLifecycler<Asset=A>> UntypedAssetLifecycler for L
{
    fn load_untyped(&self, storage: Arc<AssetsStorage>, untyped_handle: UntypedHandleInner, input: Box<dyn AssetReader>)
    {
        let result = self.load(AssetLoadRequest
        {
            input,
            storage,
        });

        let handle = unsafe { AssetHandle::<A>::attach_from_untyped(untyped_handle) };
        handle.store_payload(result);
    }

    // this doesn't really make sense here
    // todo: can probably have an untyped 'asset handle' similar to the untyped lifecycler
    fn error_untyped(&self, untyped_handle: UntypedHandleInner, error: AssetLoadError)
    {
        let handle = unsafe { AssetHandle::<A>::attach_from_untyped(untyped_handle) };
        handle.store_payload(AssetPayload::Unavailable(error));
    }

    fn drop_untyped(&self, untyped_handle: &UntypedHandleInner)
    {
        let handle = unsafe { untyped_handle.as_unchecked::<A>() };
        debug_assert_eq!(handle.ref_count(), 0);
        handle.payload.swap(None);
    }
}

// TODO: make this a trait?
#[derive(Default)]
pub struct AssetLifecyclers
{
    pub(super) lifecyclers: HashMap<TypeId, Box<dyn UntypedAssetLifecycler>>,
}
impl AssetLifecyclers
{
    pub fn add_lifecycler<A: Asset, L: AssetLifecycler<Asset=A> + UntypedAssetLifecycler + 'static>(mut self, lifecycler: L) -> Self
    {
        // warn/fail on duplicates?
        self.lifecyclers.insert(TypeId::of::<A>(), Box::new(lifecycler));
        self
    }
}

pub trait AssetReader: Read + Seek + Send { }
impl<T: Read + Seek + Send> AssetReader for T { }

pub(super) struct _NullAsset;
impl Asset for _NullAsset { }

pub(super) enum AssetLifecycleRequestKind
{
    StopWorkers,
    Drop,
    LoadFileBacked, // loads the file pointed by the asset path
    LoadFromMemory(Box<dyn AssetReader>),
}

pub(super) struct AssetLifecycleRequest
{
    pub asset_type: TypeId,
    pub kind: AssetLifecycleRequestKind,
    pub untyped_handle: UntypedHandleInner, // must be a strong ref, must be cloned to use externally
}
