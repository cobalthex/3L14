use super::*;
use bitcode::DecodeOwned;
use std::any::TypeId;
use std::collections::HashMap;
use std::error::Error;
use std::io::{Cursor, Read, Seek, SeekFrom};
use enumflags2::{bitflags, BitFlags};
use debug_3l14::debug_gui::DebugGui;
use nab_3l14::utils::alloc_slice::alloc_slice_uninit;
use nab_3l14::utils::{varint, ShortTypeName};
use triomphe::Arc;

pub struct AssetLoadRequest
{
    pub asset_key: AssetKey,
    pub input: AssetRead, // TODO: memory mapped buffer?

    storage: Arc<AssetsStorage>,

    // timer?
    // is_reloading?
    // dependencies
}
impl AssetLoadRequest
{
    // TODO: unify implementations between this and asset builder
    fn deserialize_data<T: DecodeOwned>(input: &mut AssetRead) -> Result<T, Box<dyn Error>>
    {
        let size = varint::decode_from(input)?;
        let mut bytes = unsafe { alloc_slice_uninit(size as usize) }; // todo: cache this (bitcode Buffer)
        input.read_exact(&mut bytes)?;
        Ok(bitcode::decode::<T>(&bytes)?)
    }

    // deserialize a pre-sized type from the stream
    pub fn deserialize<T: DecodeOwned>(&mut self) -> Result<T, Box<dyn Error>>
    {
        Self::deserialize_data(&mut self.input)
    }

    // read a size-prefixed span of bytes, all or nothing
    pub fn read_sized(&mut self) -> Result<&[u8], Box<dyn Error>>
    {
        let size = varint::decode_from(&mut self.input)?;
        self.read_n_bytes(size as usize)
    }

    pub fn read_n_bytes(&mut self, n: usize) -> Result<&[u8], Box<dyn Error>>
    {
        let pos = self.input.position() as usize;
        self.input.seek(SeekFrom::Current(n as i64))?;
        let buf = self.input.get_ref();
        Ok(&buf[pos..(pos + n)])
    }

    pub fn read_to_end(&mut self) -> Result<&[u8], Box<dyn Error>>
    {
        let pos = self.input.position() as usize;
        self.input.seek(SeekFrom::End(0))?;
        let buf = self.input.get_ref();
        Ok(&buf[pos..self.input.position() as usize])
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
    pub fn load_dependency<A: Asset>(&self, asset_key: AssetKey) -> Ash<A>
    {
        // pattern matches Assets::load()
        AssetsStorage::enqueue_load(&self.storage, asset_key, |h| AssetLifecycleRequest::LoadFileBacked(h))
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

pub trait TrivialAssetLifecycler: Sync + Send { type Asset: Asset + DecodeOwned; }
impl<L: TrivialAssetLifecycler> AssetLifecycler for L
{
    type Asset = L::Asset;
    fn load(&self, mut request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>>
    {
        request.deserialize::<Self::Asset>()
    }
}

// only for use internally in the asset system, mostly just utility methods for interacting with generics
pub(super) trait UntypedAssetLifecycler: Sync + Send
{
    fn load_untyped(
        &self,
        storage: Arc<AssetsStorage>,
        untyped_handle: UntypedAssetHandle,
        input: AssetRead,
        #[cfg(feature = "asset_debug_data")] maybe_debug_input: Option<AssetRead>);

    fn error_untyped(
        &self,
        untyped_handle: UntypedAssetHandle,
        error: AssetLoadError);

    fn display_name(&self) -> &str;
}
impl<A: Asset, L: AssetLifecycler<Asset=A>> UntypedAssetLifecycler for L
{
    fn load_untyped(
        &self,
        storage: Arc<AssetsStorage>,
        untyped_handle: UntypedAssetHandle,
        input: AssetRead,
        #[cfg(feature = "asset_debug_data")] mut maybe_debug_input: Option<AssetRead>)
    {
        // TODO: asset storage should prevent this from running on multiple threads for the same asset concurrently

        let retyped = unsafe { Ash::<A>::attach_from(untyped_handle) };

        #[cfg(feature = "asset_debug_data")]
        retyped.inner().store_debug_data::<A>(None);

        match self.load(AssetLoadRequest { asset_key: retyped.key(), input, storage })
        {
            Ok(asset) =>
            {
                retyped.store_payload(AssetPayload::Available(Arc::new(asset)))
            }
            Err(err) =>
            {
                log::error!("Failed to load {retyped:#?}: {err:?}");
                retyped.store_payload(AssetPayload::Unavailable(AssetLoadError::Parse))
            },
        }

        #[cfg(feature = "asset_debug_data")]
        if let Some(debug_input) = &mut maybe_debug_input
        {
            let hydrated: A::DebugData = match AssetLoadRequest::deserialize_data(debug_input)
            {
                Ok(data) => data,
                Err(err) =>
                {
                    log::error!("Failed to load debug data for {retyped:?}: {err:?}");
                    return;
                },
            };
            retyped.inner().store_debug_data::<A>(Some(Arc::new(hydrated)));
        }
    }

    // this doesn't really make sense here
    // special case for internal errors
    fn error_untyped(&self, untyped_handle: UntypedAssetHandle, error: AssetLoadError)
    {
        let retyped = unsafe { Ash::<A>::attach_from(untyped_handle) };

        #[cfg(feature = "asset_debug_data")]
        retyped.inner().store_debug_data::<A>(None);

        retyped.store_payload(AssetPayload::Unavailable(error));
    }

    fn display_name(&self) -> &str
    {
        A::short_type_name()
    }
}

#[bitflags]
#[derive(Copy, Clone)]
#[repr(u8)]
pub(super) enum AssetLifecyclerFeature
{
    HasDebugGui = 0b0000_0001,
}

pub(super) struct RegisteredAssetLifecycler
{
    pub lifecycler: Box<dyn UntypedAssetLifecycler>,
    #[cfg(debug_assertions)]
    pub type_id: TypeId,
    pub features: BitFlags<AssetLifecyclerFeature>,
    pub debug_gui_fn: Option<usize>, // TODO: use *mut () instead of usize
}

pub(super) struct RegisteredAssetType
{
    pub type_id: TypeId,
    #[allow(dead_code)]
    #[cfg(debug_assertions)] // use one of the features?
    pub type_name: &'static str,
    pub dealloc_fn: fn(UntypedAssetHandle),
}

#[derive(Default)]
pub struct AssetLifecyclers
{
    pub(super) lifecyclers: HashMap<AssetTypeId, RegisteredAssetLifecycler>,
    pub(super) registered_asset_types: HashMap<AssetTypeId, RegisteredAssetType>,
}
impl AssetLifecyclers
{
    #[allow(private_bounds)]
    pub fn add_lifecycler<A: Asset, L: AssetLifecycler<Asset=A> + UntypedAssetLifecycler + 'static>(mut self, lifecycler: L) -> Self
    {
        // warn/fail on duplicates?
        self.lifecyclers.insert(A::asset_type(), RegisteredAssetLifecycler
        {
            lifecycler: Box::new(lifecycler),
            #[cfg(debug_assertions)]
            type_id: TypeId::of::<L>(),
            features: BitFlags::empty(),
            debug_gui_fn: None,
        });
        self.registered_asset_types.insert(A::asset_type(), RegisteredAssetType
        {
            type_id: TypeId::of::<A>(),
            #[cfg(debug_assertions)]
            type_name: A::short_type_name(),
            dealloc_fn: |h| unsafe { h.dealloc::<A>() },
        });
        self
    }

    // todo: specialization would be better here
    pub fn add_lifecycler_with_gui<A: Asset, L: AssetLifecycler<Asset=A> + DebugGui + 'static>(mut self, lifecycler: L) -> Self
    {
        // todo: dedupe

        let debug_gui_fn = L::debug_gui as usize;

        // warn/fail on duplicates?
        self.lifecyclers.insert(A::asset_type(), RegisteredAssetLifecycler
        {
            lifecycler: Box::new(lifecycler),
            #[cfg(debug_assertions)]
            type_id: TypeId::of::<L>(),
            features: AssetLifecyclerFeature::HasDebugGui.into(),
            debug_gui_fn: Some(debug_gui_fn),
        });
        self.registered_asset_types.insert(A::asset_type(), RegisteredAssetType
        {
            type_id: TypeId::of::<A>(),
            #[cfg(debug_assertions)]
            type_name: A::short_type_name(),
            dealloc_fn: |h| unsafe { h.dealloc::<A>() },
        });
        self
    }
}

pub type AssetRead = Cursor<Box<[u8]>>;
pub(super) enum AssetLifecycleRequest
{
    StopWorkers,
    Drop(UntypedAssetHandle),
    LoadFileBacked(UntypedAssetHandle), // loads the file pointed by the asset path
    LoadFromMemory(UntypedAssetHandle, AssetRead),
}


/* TODO

- spin-up extra worker threads if there's a high queue depth?

- notification callbacks when a certain asset type is built ?
= reverse dependency chain update notifications (e.g Material needs to rebind when texture/shader rebuild)

- while updates are being pushed, lock 'sender' and wait for all loads to finish before deduping then sending out notifications

 */
