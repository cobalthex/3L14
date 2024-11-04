use super::*;
use crate::engine::alloc_slice::alloc_slice_uninit;
use crate::engine::{varint, ShortTypeName};
use bitcode::DecodeOwned;
use std::any::TypeId;
use std::collections::HashMap;
use std::error::Error;
use std::io::{Read, Seek};
use std::sync::Arc;

pub struct AssetLoadRequest
{
    pub asset_key: AssetKey,
    pub input: Box<dyn AssetRead>, // TODO: memory mapped buffer?
    storage: Arc<AssetsStorage>,

    // timer?
    // is_reloading?
    // dependencies
}
impl AssetLoadRequest
{
    // deserialize a pre-sized type from the stream
    pub fn deserialize<T: DecodeOwned>(&mut self) -> Result<T, Box<dyn Error>>
    {
        let size = varint::decode_from(&mut self.input)?;
        let mut input = unsafe { alloc_slice_uninit(size as usize) }?; // todo: cache this (bitcode Buffer)
        self.input.read_exact(&mut input)?;
        Ok(bitcode::decode::<T>(&input)?)
    }

    // read a size-prefixed span of bytes, all or nothing
    pub fn read_sized(&mut self) -> Result<Box<[u8]>, Box<dyn Error>>
    {
        let size = varint::decode_from(&mut self.input)?;
        let mut input = unsafe { alloc_slice_uninit(size as usize) }?; // todo: cache this
        self.input.read_exact(&mut input)?;
        Ok(input)
    }
    // 
    // // Load another asset, but don't reload this asset if the requested asset is reloaded
    // #[must_use]
    // pub fn load_reference<A: Asset>(&self, asset_key: AssetKey) -> AssetHandle<A>
    // {
    //     // pattern matches Assets::load()
    //     self.storage.enqueue_load(asset_key, |h| AssetLifecycleRequest::LoadFileBacked(h))
    // }

    // Load another asset and queue this asset for reloading if the requested asset is reloaded
    #[must_use]
    pub fn load_dependency<A: Asset>(&self, asset_key: AssetKey) -> AssetHandle<A>
    {
        // pattern matches Assets::load()
        self.storage.enqueue_load(asset_key, |h| AssetLifecycleRequest::LoadFileBacked(h))
    }
    //
    // // Load a reference from a specified source
    // // Assets/lifecyclers are responsible for tracking/maintaining reference references
    // #[must_use]
    // pub fn load_dependency_from<A: Asset, R: AssetRead + 'static>(
    //     &self,
    //     asset_key: AssetKey,
    //     input_data: R // take box?
    // ) -> AssetHandle<A>
    // {
    //     // pattern matches Assets::load_from()
    //     self.storage.enqueue_load(asset_key, |h| AssetLifecycleRequest::LoadFromMemory(h, Box::new(input_data)))
    // }
}

pub trait AssetLifecycler: Sync + Send
{
    type Asset: Asset;

    /// Get or create an asset payload for the requested asset
    fn load(&self, request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>>;
    // reload ?
}

// only for use internally in the asset system, mostly just utility methods for interacting with generics
pub(super) trait UntypedAssetLifecycler: Sync + Send
{
    fn load_untyped(&self, storage: Arc<AssetsStorage>, untyped_handle: UntypedAssetHandle, input: Box<dyn AssetRead>);

    fn error_untyped(&self, untyped_handle: UntypedAssetHandle, error: AssetLoadError);
}
impl<A: Asset, L: AssetLifecycler<Asset=A>> UntypedAssetLifecycler for L
{
    fn load_untyped(&self, storage: Arc<AssetsStorage>, untyped_handle: UntypedAssetHandle, input: Box<dyn AssetRead>)
    {
        let retyped = unsafe { AssetHandle::<A>::attach_from(untyped_handle) };
        match self.load(AssetLoadRequest { asset_key: retyped.key(), input, storage })
        {
            Ok(asset) =>
            {
                retyped.store_payload(AssetPayload::Available(Arc::new(asset)))
            }
            Err(err) =>
            {
                log::warn!("Failed to load {retyped:#?}: {err}");
                retyped.store_payload(AssetPayload::Unavailable(AssetLoadError::Parse))
            },
        }
    }

    // this doesn't really make sense here
    // special case for internal errors
    fn error_untyped(&self, untyped_handle: UntypedAssetHandle, error: AssetLoadError)
    {
        let retyped = unsafe { AssetHandle::<A>::attach_from(untyped_handle) };
        retyped.store_payload(AssetPayload::Unavailable(error));
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



/* TODO

- spin-up extra worker threads if there's a high queue depth?

- notification callbacks when a certain asset type is built ?
= reverse dependency chain update notifications (e.g Material needs to rebind when texture/shader rebuild)

- while updates are being pushed, lock 'sender' and wait for all loads to finish before deduping then sending out notifications

 */