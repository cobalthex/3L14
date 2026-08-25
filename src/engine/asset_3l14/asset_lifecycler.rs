use super::*;
use std::any::TypeId;
use std::collections::HashMap;
use std::error::Error;
use debug_3l14::debug_gui::DebugGui;
use nab_3l14::utils::{varint, ShortTypeName};

pub struct AssetLoadRequest<'r, A: Asset>
{
    assets: &'r Assets,

    pub asset_key: AssetKey,
    pub structured_data: A::StructuredData,
    pub opaque_data: &'r [u8],

    // timer?
    // is_reloading?
    // dependencies
}
impl<A: Asset> AssetLoadRequest<'_, A>
{
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
    pub fn load_dependency<D: Asset>(&self, asset_key: AssetKey) -> Ash<D>
    {
        // pattern matches Assets::load()
        self.assets.load(asset_key)
    }
    //
    // // Load a reference from a specified source
    // // Assets/lifecyclers are responsible for tracking/maintaining reference references
    // #[must_use]
    // pub fn load_dependency_from<A: Asset, R: AssetPayload + 'static>(
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
    fn load(&self, request: AssetLoadRequest<Self::Asset>) -> Result<Self::Asset, Box<dyn Error>>;
    // reload ?
}

pub trait TrivialAssetLifecycler: Sync + Send { type Asset: Asset; }
impl<TL: TrivialAssetLifecycler> AssetLifecycler for TL
    where TL::Asset: Asset<StructuredData=TL::Asset>
{
    type Asset = TL::Asset;
    fn load(&self, request: AssetLoadRequest<Self::Asset>) -> Result<Self::Asset, Box<dyn Error>>
    {
        Ok(request.structured_data)
    }
}

// only for use internally in the asset system, mostly just utility methods for interacting with generics
pub(super) trait UntypedAssetLifecycler: Sync + Send
{
    fn load_untyped(
        &self,
        assets: &Assets,
        untyped_handle: ErasedAsh,
        input: &[u8],
        #[cfg(feature = "asset_debug_data")] maybe_debug_input: Option<&[u8]>);

    fn error_untyped(
        &self,
        untyped_handle: ErasedAsh,
        error: AssetLoadError);

    fn display_name(&self) -> &str;
}
impl<A: Asset, L: AssetLifecycler<Asset=A>> UntypedAssetLifecycler for L
{
    fn load_untyped(
        &self,
        assets: &Assets,
        untyped_handle: ErasedAsh,
        mut input: &[u8],
        #[cfg(feature = "asset_debug_data")] mut maybe_debug_input: Option<&[u8]>)
    {
        // TODO: asset storage should prevent this from running on multiple threads for the same asset concurrently

        let retyped = unsafe { Ash::<A>::attach_from(untyped_handle) };

        #[cfg(feature = "asset_debug_data")]
        retyped.inner().store_debug_data(None);

        // TODO: store this centrally so asset builder can share

        let (structured_data_size, structured_data_start) = varint::decode(&mut input);
        let opaque_data_start = structured_data_start + structured_data_size as usize;
        if structured_data_start == 0 ||
            input.len() < opaque_data_start
        {
            log::debug!("Input data for {:?} is not long enough to parse", retyped.key());
            retyped.inner().store_data(Some(AssetData::Unavailable(AssetLoadError::PayloadTooSmall)));
            return;
        }

        let structured_data =
        {
            let bytes = &input[structured_data_start..opaque_data_start];
            match bitcode::decode::<A::StructuredData>(bytes)
            {
                Ok(data) => data,
                Err(err) =>
                {
                    log::debug!("Failed to parse structured data for {:?}: {}", retyped.key(), err);
                    retyped.inner().store_data(Some(AssetData::Unavailable(AssetLoadError::Parse)));
                    return;
                }
            }
        };
        let opaque_data = &input[opaque_data_start..];

        match self.load(AssetLoadRequest
        {
            asset_key: retyped.key(),
            structured_data,
            opaque_data,
            assets,
        })
        {
            Ok(asset) =>
            {
                retyped.store_data(Some(AssetData::Available(asset)));
            }
            Err(err) =>
            {
                log::error!("Failed to load {retyped:#?}: {err:?}");
                retyped.store_data(Some(AssetData::Unavailable(AssetLoadError::Parse)));
            },
        }

        #[cfg(feature = "asset_debug_data")]
        if let Some(debug_input) = &mut maybe_debug_input
        {
            match bitcode::decode(&debug_input)
            {
                Ok(hydrated) => retyped.inner().store_debug_data(Some(hydrated)),
                Err(err) =>
                {
                    log::debug!("Failed to parse debug data for {:#?}: {}", retyped.key(), err);
                }
            }
        }
    }

    // this doesn't really make sense here
    // special case for internal errors
    fn error_untyped(&self, untyped_handle: ErasedAsh, error: AssetLoadError)
    {
        let retyped = unsafe { Ash::<A>::attach_from(untyped_handle) };

        #[cfg(feature = "asset_debug_data")]
        retyped.inner().store_debug_data(None);

        retyped.store_data(Some(AssetData::Unavailable(error)));
    }

    fn display_name(&self) -> &str
    {
        A::short_type_name()
    }
}

pub(super) struct RegisteredAssetLifecycler
{
    pub lifecycler: Box<dyn UntypedAssetLifecycler>,
    #[cfg(debug_assertions)]
    pub type_id: TypeId,
    pub debug_gui_fn: Option<usize>,
}

pub(super) struct RegisteredAssetType
{
    pub type_id: TypeId,
    #[allow(dead_code)]
    #[cfg(debug_assertions)] // use one of the features?
    pub type_name: &'static str,
    pub dealloc_fn: fn(ErasedAsh),
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

        fn debug_gui_fn<L: DebugGui>(lifecycler: &dyn UntypedAssetLifecycler, ui: &mut egui::Ui)
        {
            unsafe { &*(lifecycler as *const _ as *const L) }.debug_gui(ui);
        }

        // warn/fail on duplicates?
        self.lifecyclers.insert(A::asset_type(), RegisteredAssetLifecycler
        {
            lifecycler: Box::new(lifecycler),
            #[cfg(debug_assertions)]
            type_id: TypeId::of::<L>(),
            debug_gui_fn: Some(debug_gui_fn::<L> as *const () as usize),
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

pub(super) enum AssetLifecycleRequest
{
    StopWorkers,
    Drop(ErasedAsh),
    LoadFileBacked(ErasedAsh), // loads the file pointed by the asset path
    LoadFromMemory(ErasedAsh, Box<[u8]>),
}


/* TODO

- spin-up extra worker threads if there's a high queue depth?

- notification callbacks when a certain asset type is built ?
= reverse dependency chain update notifications (e.g Material needs to rebind when texture/shader rebuild)

- while updates are being pushed, lock 'sender' and wait for all loads to finish before deduping then sending out notifications

 */
