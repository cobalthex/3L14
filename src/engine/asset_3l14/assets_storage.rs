use super::*;
use crossbeam::channel::{unbounded, Receiver, Sender};
use debug_3l14::debug_gui::DebugGui;
use egui::Ui;
use nab_3l14::utils::array::init_array;
use parking_lot::Mutex;
use std::any::TypeId;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::{Builder, JoinHandle};
use std::time::Duration;
// TODO: probably don't pass around UniCase publicly

#[cfg(feature = "hot_reloading")]
use notify::{event::ModifyKind, EventKind, RecommendedWatcher, RecursiveMode};
#[cfg(feature = "hot_reloading")]
use notify_debouncer_full::{Debouncer, RecommendedCache};

type AssetHandleBank = HashMap<AssetKey, UntypedAssetHandle>;

pub enum AssetNotification
{
    Reload(AssetKey),
}

pub(super) struct AssetsStorage
{
    registered_asset_types: HashMap<AssetTypeId, RegisteredAssetType>,
    lifecyclers: HashMap<AssetTypeId, RegisteredAssetLifecycler>,

    handles: Mutex<AssetHandleBank>,
    lifecycle_channel: Sender<AssetLifecycleRequest>,
    notification_channel: (Sender<AssetNotification>, Receiver<AssetNotification>),

    assets_root: PathBuf, // should be absolute
}
impl AssetsStorage
{
    #[must_use]
    fn create_or_update_handle<A: Asset>(&self, asset_key: AssetKey) -> (bool /* pre-existing */, Ash<A>)
    {
        // debug assert?
        assert_eq!(A::asset_type(), asset_key.asset_type()); // todo: return an error handle
        match self.registered_asset_types.get(&A::asset_type())
        {
            None => panic!("Asset type {:?} does not have a registered lifecycler", A::asset_type()),
            Some(rat) => assert_eq!(rat.type_id, TypeId::of::<A>()),
        }

        let mut handle_bank = self.handles.lock();

        let mut pre_existing = true;
        let handle = handle_bank.entry(asset_key).or_insert_with(||
        {
            pre_existing = false;
            /*
            Memory for asset is managed inside the asset storage

            This prevents a race between the handle being dropped, deleting its memory,
              and this function returning an existing (stale) handle.

            By having the pointer created and destroyed by this class,
              there is serialization provided by the mutex preventing use-after-free issues.
            */

            UntypedAssetHandle::alloc::<A>(asset_key, self.lifecycle_channel.clone())
        });

        (pre_existing, unsafe { Ash::<A>::clone_from(handle) })
    }

    fn drop_handle(&self, untyped_handle: UntypedAssetHandle)
    {
        let mut handle_bank = self.handles.lock(); // must lock before below to make sure that the handle doesn't get cloned between the drop and below

        let inner = untyped_handle.as_ref();

        // retrieve this info after locking the handle bank so a zero refcount cannot be added to by a load()
        if inner.ref_count() != 0
        {
            return;
        }

        match handle_bank.remove(&inner.key())
        {
            None =>
            {
                // without an entry we don't know the size, and cannot free the asset handle
                panic!("Tried to remove a handle that was not registered to this AssetsStorage");
            }
            Some(stored_handle) =>
            {
                debug_assert!(stored_handle == untyped_handle);
                let registered_type = self.registered_asset_types.get(&inner.asset_type())
                    .expect("Can't drop asset, asset type unregistered. How did you get here?");
                (registered_type.dealloc_fn)(untyped_handle);
            }
        }
    }

    #[inline] #[must_use]
    pub fn get_lifecycler<A: Asset>(&self) -> Option<&RegisteredAssetLifecycler>
    {
        self.lifecyclers.get(&A::asset_type())
    }

    #[must_use]
    pub fn enqueue_load<A: Asset, F: FnOnce(UntypedAssetHandle) -> AssetLifecycleRequest>(
        self: &Arc<Self>,
        asset_key: AssetKey,
        input_fn: F) -> Ash<A>
    {
        let (pre_existed, asset_handle) = self.create_or_update_handle(asset_key);

        // todo: what to do if already queued for load?

        if self.lifecyclers.contains_key(&asset_key.asset_type())
        {
            let request = input_fn(unsafe { asset_handle.clone().into_inner() });

            // don't clear payload?
            asset_handle.store_payload(AssetPayload::Pending);
            if pre_existed
            {
                self.notification_channel.0.send(AssetNotification::Reload(asset_key)).unwrap(); // todo: error handling
            }

            if self.lifecycle_channel.send(request).is_err()
            {
                asset_handle.store_payload(AssetPayload::Unavailable(AssetLoadError::Shutdown));
            }
        }
        else
        {
            asset_handle.store_payload(AssetPayload::Unavailable(AssetLoadError::LifecyclerNotRegistered));
        }

        asset_handle
    }

    // TODO
    // // Reload an asset from it's known asset path - this will set an error if the new data is bad
    // pub fn try_reload_path<S: AssetPath>(self: &Arc<Self>, asset_path: &S) -> bool /* returns false if unable to enqueue a reload */
    // {
    //     let owned_path = UniCase::<String>::new(asset_path.to_string());
    //     let Some(&asset_key) = self.asset_paths_to_keys.lock().get(&owned_path) else { return false; };
    //
    //     let handle_bank = self.handles.lock();
    //     let Some(entry) = handle_bank.get(&asset_key) else { return false; };
    //     if !self.lifecyclers.contains_key(&entry.asset_type) { return false; }
    //
    //     let asset_handle = unsafe { AssetHandle::<_NullAsset>::clone_from_untyped(&entry.untyped_handle) };
    //     asset_handle.inner().is_reloading.store(true, Ordering::Release); // correct ordering?
    //
    //     let request = AssetLifecycleRequest
    //     {
    //         asset_type: entry.asset_type,
    //         untyped_handle: unsafe { asset_handle.clone().into_untyped() },
    //         kind: AssetLifecycleRequestKind::LoadFileBacked,
    //     };
    //
    //     if self.lifecycle_channel.send(request).is_err()
    //     {
    //         asset_handle.store_payload(AssetPayload::Unavailable(AssetLoadError::Shutdown));
    //         return false;
    //     }
    //
    //     true
    // }

    #[inline]
    pub fn asset_key_to_file_path(&self, asset_key: AssetKey, fty: AssetFileType) -> PathBuf
    {
        self.assets_root.as_path().join(asset_key.as_file_name(fty))
    }

    #[inline]
    fn open_asset_from_file<P: AsRef<Path>>(file_path: P) -> Result<impl AssetRead, std::io::Error>
    {
        std::fs::File::open(file_path)
    }

    pub fn asset_worker_fn(self: Arc<Self>, request_recv: Receiver<AssetLifecycleRequest>) -> impl FnOnce()
    {
        move ||
        {
            log::debug!("Starting asset worker thread");
            'worker: loop
            {
                match request_recv.recv()
                {
                    Ok(request) =>
                    {
                        puffin::profile_scope!("Asset lifecycle request");

                        // note: request.handle must be managed manually here

                        match request
                        {
                            AssetLifecycleRequest::StopWorkers =>
                            {
                                log::debug!("Shutting down asset worker thread");

                                // clean out any final drop requests
                                while let Ok(final_request) = request_recv.try_recv()
                                {
                                    match final_request
                                    {
                                        AssetLifecycleRequest::StopWorkers => {},
                                        AssetLifecycleRequest::Drop(untyped_handle) =>
                                        {
                                            self.drop_handle(untyped_handle);
                                        }
                                        AssetLifecycleRequest::LoadFromMemory(untyped_handle, _) |
                                        AssetLifecycleRequest::LoadFileBacked(untyped_handle) =>
                                        {
                                            let asset_type = untyped_handle.as_ref().asset_type();
                                            let lifecycler = &self.lifecyclers.get(&asset_type)
                                                .expect("Unsupported asset type!").lifecycler; // this should fail in load()

                                            lifecycler.error_untyped(untyped_handle, AssetLoadError::Shutdown);
                                        }
                                    }
                                }

                                break 'worker;
                            },
                            AssetLifecycleRequest::LoadFileBacked(untyped_handle) =>
                            {
                                let inner = untyped_handle.as_ref();
                                let lifecycler = &self.lifecyclers.get(&inner.asset_type())
                                    .expect("Unsupported asset type!").lifecycler; // this should fail in load()

                                #[cfg(feature = "asset_debug_data")]
                                let debug_asset_data: Option<Box<dyn AssetRead>> =
                                {
                                    let asset_debug_path = self.asset_key_to_file_path(inner.key(), AssetFileType::DebugData);
                                    match Self::open_asset_from_file(asset_debug_path)
                                    {
                                        Ok(dbg_data) => Some(Box::new(dbg_data)),
                                        Err(_) => None, // log specific errors?
                                    }
                                };

                                let asset_file_path = self.asset_key_to_file_path(inner.key(), AssetFileType::Asset);
                                match Self::open_asset_from_file(asset_file_path)
                                {
                                    Ok(read) => lifecycler.load_untyped(
                                        self.clone(),
                                        untyped_handle,
                                        Box::new(read),
                                        #[cfg(feature = "asset_debug_data")] debug_asset_data),
                                    Err(err) =>
                                    {
                                        log::warn!("Failed to read {:?} asset file {:?}: {err}",
                                            inner.asset_type(),
                                            self.asset_key_to_file_path(inner.key(), AssetFileType::Asset));
                                        lifecycler.error_untyped(untyped_handle, AssetLoadError::Fetch);
                                    }
                                };
                            },
                            AssetLifecycleRequest::LoadFromMemory(untyped_handle, reader) =>
                            {
                                let lifecycler = &self.lifecyclers.get(&untyped_handle.as_ref().asset_type())
                                    .expect("Unsupported asset type!").lifecycler; // this should fail in load()

                                lifecycler.load_untyped(
                                    self.clone(),
                                    untyped_handle,
                                    reader,
                                    #[cfg(feature = "asset_debug_data")] None);
                            },
                            AssetLifecycleRequest::Drop(untyped_handle) =>
                            {
                                self.drop_handle(untyped_handle);
                            },
                        }
                    },
                    Err(err) =>
                    {
                        log::error!("Terminating asset worker thread due to {err}");
                        break 'worker;
                    }
                }
            }
        }
    }
}

pub struct AssetsConfig
{
    pub assets_root: PathBuf,
    pub enable_fs_watcher: bool,
}
impl AssetsConfig
{
    #[cfg(test)]
    pub fn test() -> Self
    {
        Self { assets_root: PathBuf::from("TEST_DIR"), enable_fs_watcher: false }
    }
}

struct AssetsDebugState
{
    inspected_lifecycler: AssetTypeId,
}
impl Default for AssetsDebugState
{
    fn default() -> Self
    {
        Self
        {
            inspected_lifecycler: AssetTypeId::Invalid,
        }
    }
}

const NUM_ASSET_JOB_THREADS: usize = 1;
pub struct Assets
{
    storage: Arc<AssetsStorage>,
    #[cfg(feature = "hot_reloading")]
    fs_watcher: Option<Debouncer<RecommendedWatcher, RecommendedCache>>,
    worker_threads: [Option<JoinHandle<()>>; NUM_ASSET_JOB_THREADS],

    debug_state: Mutex<AssetsDebugState>, // only one place ever calls this (MAKE SURE OF THIS)
}
impl Assets
{
    #[must_use]
    pub fn new(asset_lifecyclers: AssetLifecyclers, config: AssetsConfig) -> Self
    {
        #[cfg(debug_assertions)]
        log::debug!("Serving assets from {:?}", config.assets_root);

        // TODO: pqueue

        // TODO: flume is probably a better choice here
        let (lifecycle_send, lifecycle_recv) = unbounded::<AssetLifecycleRequest>();
        let storage = Arc::new(AssetsStorage
        {
            registered_asset_types: asset_lifecyclers.registered_asset_types,
            lifecyclers: asset_lifecyclers.lifecyclers,
            handles: Mutex::new(AssetHandleBank::new()),
            lifecycle_channel: lifecycle_send,
            notification_channel: unbounded::<AssetNotification>(),

            assets_root: config.assets_root,
        });

        #[cfg(feature = "hot_reloading")]
        let fs_watcher = if config.enable_fs_watcher { Self::try_fs_watch(storage.clone()).inspect_err(|err|
        {
            // TODO: print message on successful startup
            log::error!("Failed to start fs watcher for hot-reloading, continuing without: {err:?}");
        }).ok() } else { None };

        // hot reload batching?

        /* TODO: broadcast asset change notifications? -- do per-lifecycler? (both options?)

            + more efficient than indirection
            + dedicated 'watchers'

            - likely more of a spaghetti mess
            - possibly awkward lifetimes

            ~ still need to hold references to asset, but payload can be queried once
            ~ dependency chain management - if dependencies are held by handles rather than payloads, there could be corruption (but mid chain could be updated and not parents)
                - possibly needs tree locking, will need to explore this

            - asset all require two indirections right now, which is inefficient

         */

        // todo: async would maybe nice here (file/network IO, multi-part loads)
        let worker_threads = init_array::<_, NUM_ASSET_JOB_THREADS>(|i|
        {
            let thread = Builder::new()
                .name(format!("Asset worker thread {}", i))
                .spawn(AssetsStorage::asset_worker_fn(storage.clone(), lifecycle_recv.clone())).expect("Failed to create asset worker thread");
            Some(thread)
        });

        Self
        {
            storage,
            #[cfg(feature = "hot_reloading")]
            fs_watcher,
            worker_threads,

            debug_state: Mutex::new(AssetsDebugState::default()),
        }
    }

    pub fn subscribe_to_notifications(&self) -> Receiver<AssetNotification>
    {
        self.storage.notification_channel.1.clone()
    }

    #[cfg(feature = "hot_reloading")]
    fn try_fs_watch(assets_storage: Arc<AssetsStorage>) -> notify::Result<Debouncer<RecommendedWatcher, RecommendedCache>>
    {
        let assets_storage_clone = assets_storage.clone();
        // batching?
        let mut fs_watcher = notify_debouncer_full::new_debouncer(
            Duration::from_secs(1),
            None,
            move |evt: notify_debouncer_full::DebounceEventResult|
            {
                match evt
                {
                    Ok(events) =>
                    {
                        for event in events
                        {
                            let EventKind::Modify(m) = event.kind else { continue; };
                            let ModifyKind::Data(_) = m else { continue; };

                            if event.paths.is_empty() { continue; }
                            let asset_file_path = &event.paths[0];

                            // todo: convert file path to asset path
                            // TODO
                            // assets_storage_clone.try_reload_path(&asset_path);

                            // track renames?
                        }
                    },
                    Err(e) => log::error!("FS watch error: {:?}", e),
                }
            })?;

        let assets_path = assets_storage.assets_root.as_path();
        fs_watcher.watch(assets_path, RecursiveMode::Recursive)?;
        Ok(fs_watcher)
    }

    #[must_use]
    pub fn load<A: Asset>(&self, asset_key: AssetKey) -> Ash<A>
    {
        self.storage.enqueue_load(asset_key, AssetLifecycleRequest::LoadFileBacked)
    }

    #[must_use]
    pub fn load_from<A: Asset>(
        &self,
        asset_key: AssetKey,
        input_data: impl AssetRead + 'static // static not ideal here
    ) -> Ash<A>
    {
        self.storage.enqueue_load(asset_key, |h| AssetLifecycleRequest::LoadFromMemory(h, Box::new(input_data)))
    }

    #[must_use]
    #[cfg(test)]
    pub fn load_direct_from<A: Asset>(
        &self,
        asset_key: AssetKey,
        input_data: impl AssetRead + 'static // static not ideal here
    ) -> Ash<A>
    {
        let lifecycler = self.storage.get_lifecycler::<A>().expect("No lifecycler found for asset type");
        let handle = self.storage.create_or_update_handle(asset_key);
        lifecycler.lifecycler.load_untyped(
            self.storage.clone(),
            unsafe { handle.1.clone().into_inner() },
            Box::new(input_data),
            #[cfg(feature = "asset_debug_data")] None);
        handle.1
    }

    #[must_use]
    pub fn num_active_assets(&self) -> usize
    {
        let handles = self.storage.handles.lock();
        handles.len()
    }

    #[inline] #[must_use]
    // Get an asset lifecycler
    // NOTE: This will verify that the type IDs match in debug, but otherwise makes no guarantees about correct types
    pub fn get_lifecycler<L: AssetLifecycler + 'static>(&self) -> Option<&L>
    {
        // TODO: in debug, check type IDs?
        let maybe = self.storage.get_lifecycler::<L::Asset>();
        maybe.map(|l|
        unsafe {
            #[cfg(debug_assertions)]
            assert_eq!(TypeId::of::<L>(), l.type_id); // debug_assert won't work here
            &*(l.lifecycler.as_ref() as *const dyn UntypedAssetLifecycler as *const L)
        })
    }

    // prevent any new asset from being loaded
    pub fn shutdown(&self)
    {
        let _ = self.storage.lifecycle_channel.send(AssetLifecycleRequest::StopWorkers); // will error if already closed
    }
}
impl DebugGui for Assets
{
    fn display_name(&self) -> &str
    {
        "Assets"
    }
    fn debug_gui(&self, ui: &mut Ui)
    {
        let mut debug_state = self.debug_state.lock();

        let inspected_lifecycler = self.storage.lifecyclers.get(&debug_state.inspected_lifecycler);

        egui::ComboBox::from_label("Lifecyclers")
            .selected_text(inspected_lifecycler.map_or("(None)", |l| l.lifecycler.display_name()))
            .show_ui(ui, |cui|
            {
                for (asset_type, lifecycler) in &self.storage.lifecyclers
                {
                    if lifecycler.debug_gui_fn.is_none() { continue; }
                    cui.selectable_value(&mut debug_state.inspected_lifecycler, *asset_type, lifecycler.lifecycler.display_name());
                }
            });

        if let Some(lifecycler) = inspected_lifecycler
        {
            // TODO
            // ui.group(|gui| { lifecycler.lifecycler.debug_gui(gui) });
        }

        #[cfg(feature = "hot_reloading")]
        {
            let mut has_fswatcher = self.fs_watcher.is_some();
            ui.checkbox(&mut has_fswatcher, "FS watcher enabled");
        }

        ui.separator();

        let handle_bank = self.storage.handles.lock();

        let total_active_count = handle_bank.len();
        ui.label(format!("Total active handles: {0}", total_active_count));

        ui.collapsing("Handles", |cui|
            {
                egui::Grid::new("Handles table")
                    .striped(true)
                    .num_columns(3)
                    .show(cui, |gui|
                        {
                            gui.heading("Key");
                            gui.heading("Refs");
                            gui.heading("Gen");
                            // gui.heading("State");
                            gui.end_row();

                            for (key, untyped_handle) in handle_bank.iter()
                            {
                                let inner = untyped_handle.as_ref();
                                gui.label(format!("{key:#?}")); // right click to copy?
                                gui.label(format!("{}", inner.ref_count()));

                                // TODO: query availability

                                gui.end_row();
                            }
                        });
            });
    }
}
#[cfg(debug_assertions)]
impl Drop for Assets
{
    fn drop(&mut self)
    {
        #[cfg(feature = "hot_reloading")]
        {
            self.fs_watcher = None;
        }
        
        self.shutdown();

        for thread in &mut self.worker_threads
        {
            thread.take()
                .unwrap() // this should never be None
                .join().unwrap();
        }

        let mut handle_bank = self.storage.handles.lock();
        if !handle_bank.is_empty()
        {
            log::error!("! Leak detected: {} active asset handle(s):", handle_bank.len());
            // multiline log?
            for handle in handle_bank.iter()
            {
                log::error!("    {:?}", handle.0);
            }
            #[cfg(test)]
            panic!("Leaked asset!");
        }
        handle_bank.clear()
    }
}

#[cfg(test)]
mod tests
{
    use super::*;
    use std::error::Error;
    use std::fmt::{Display, Formatter};
    use std::sync::atomic::AtomicUsize;

    // TODO: should probably make sure there are no mem leaks in these tests

    const TEST_ASSET_1: AssetKey = AssetKey::unique(AssetTypeId::Test1, AssetKeyDerivedId::test(), AssetKeySourceId::test(1));
    const TEST_ASSET_2: AssetKey = AssetKey::synthetic(AssetTypeId::Test2, AssetKeySynthHash::test(123));

    #[derive(Debug)]
    struct NestedAsset
    {
        id: usize,
    }
    impl Asset for NestedAsset
    {
        type DebugData = ();
        fn asset_type() -> AssetTypeId { AssetTypeId::Test2 }
    }

    #[derive(Debug)]
    struct TestError;
    impl Display for TestError
    {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { std::fmt::Debug::fmt(self, f) }
    }
    impl Error for TestError { }

    #[derive(Debug)]
    struct TestAsset
    {
        name: String,
        nested: Option<Ash<NestedAsset>>,
    }
    impl Asset for TestAsset
    {
        type DebugData = ();
        fn asset_type() -> AssetTypeId { AssetTypeId::Test1 }
    }

    struct Passthru<T: Asset>
    {
        call_count: usize,
        passthru_fn: fn(AssetLoadRequest) -> Result<T, Box<dyn Error>>,
    }

    trait TestLifecycler: AssetLifecycler
    {
        fn set_passthru(&self, pfn: Option<Passthru<Self::Asset>>);
        fn get_passthru_call_count(&self) -> Option<usize>;
    }

    #[derive(Default)]
    struct TestAssetLifecycler
    {
        active_count: AtomicUsize,
        pub passthru: Mutex<Option<Passthru<TestAsset>>>,
    }
    impl AssetLifecycler for TestAssetLifecycler
    {
        type Asset = TestAsset;
        fn load(&self, request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>>
        {
            match &mut *self.passthru.lock()
            {
                None => Err(Box::new(TestError)),
                Some(passthru) =>
                {
                    passthru.call_count += 1;
                    (passthru.passthru_fn)(request)
                },
            }
        }
    }
    impl TestLifecycler for TestAssetLifecycler
    {
        fn set_passthru(&self, pfn: Option<Passthru<Self::Asset>>)
        {
            let mut locked = self.passthru.lock();
            *locked = pfn;
        }
        fn get_passthru_call_count(&self) -> Option<usize>
        {
            self.passthru.lock().as_ref().map(|p| p.call_count)
        }
    }

    #[derive(Default)]
    struct NestedAssetLifecycler
    {
        active_count: AtomicUsize,
        pub passthru: Mutex<Option<Passthru<NestedAsset>>>,
    }
    impl AssetLifecycler for NestedAssetLifecycler
    {
        type Asset = NestedAsset;
        fn load(&self, request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>>
        {
            match &mut *self.passthru.lock()
            {
                None => Err(Box::new(TestError)),
                Some(passthru) =>
                {
                    passthru.call_count += 1;
                    (passthru.passthru_fn)(request)
                },
            }
        }
    }
    impl TestLifecycler for NestedAssetLifecycler
    {
        fn set_passthru(&self, pfn: Option<Passthru<Self::Asset>>)
        {
            let mut locked = self.passthru.lock();
            *locked = pfn;
        }
        fn get_passthru_call_count(&self) -> Option<usize>
        {
            self.passthru.lock().as_ref().map(|p| p.call_count)
        }
    }

    fn set_passthru<A: Asset, L: TestLifecycler<Asset = A> + 'static>(assets: &Assets, passthru_fn: Option<fn(AssetLoadRequest) -> Result<A, Box<dyn Error>>>)
    {
        let tal = assets.get_lifecycler::<L>().unwrap();
        tal.set_passthru(passthru_fn.map(|pfn| Passthru
        {
            call_count: 0,
            passthru_fn: pfn,
        }));
    }

    fn get_passthru_call_count<L: TestLifecycler + 'static>(assets: &Assets) -> Option<usize>
    {
        let tal = assets.get_lifecycler::<L>().unwrap();
        tal.get_passthru_call_count()
    }

    fn await_asset<A: Asset>(handle: &Ash<A>) -> AssetPayload<A>
    {
        futures::executor::block_on(handle)
    }

    mod load
    {
        use super::*;
        use std::io::Cursor;
        use parking_lot::Condvar;
        // TODO: disable threading and add 'loop_once' function for worker

        #[test]
        #[should_panic]
        fn missing_lifecycler()
        {
            let assets = Assets::new(AssetLifecyclers::default(), AssetsConfig::test());

            let req: Ash<TestAsset> = assets.load_from::<TestAsset>(TEST_ASSET_1, Cursor::new([]));
            await_asset(&req);
        }

        #[test]
        fn bad_file()
        {
            let lifecyclers = AssetLifecyclers::default()
                .add_lifecycler(TestAssetLifecycler::default());
            let assets = Assets::new(lifecyclers, AssetsConfig::test());

            let req: Ash<TestAsset> = assets.load::<TestAsset>(TEST_ASSET_1);
            match await_asset(&req)
            {
                AssetPayload::Unavailable(AssetLoadError::Fetch) => {},
                other => panic!("Invalid load result: {other:#?}"),
            }
        }

        #[test]
        fn unavailable()
        {
            let lifecyclers = AssetLifecyclers::default()
                .add_lifecycler(TestAssetLifecycler::default());
            let assets = Assets::new(lifecyclers, AssetsConfig::test());

            set_passthru::<_, TestAssetLifecycler>(&assets, Some(|_req: AssetLoadRequest|
            {
                Err(Box::new(TestError))
            }));

            let req: Ash<TestAsset> = assets.load_from::<TestAsset>(TEST_ASSET_1, Cursor::new([]));
            match await_asset(&req)
            {
                AssetPayload::Unavailable(AssetLoadError::Parse) => {},
                other => panic!("Asset not unavailable(Test): {other:#?}"),
            }
        }

        #[test]
        fn pending()
        {
            let lifecyclers = AssetLifecyclers::default()
                .add_lifecycler(TestAssetLifecycler::default());
            let assets = Assets::new(lifecyclers, AssetsConfig::test());

            let req: Ash<TestAsset> = assets.load_from::<TestAsset>(TEST_ASSET_1, Cursor::new([]));
            match req.payload()
            {
                AssetPayload::Pending => {},
                _ => panic!("Asset not pending"),
            }

            drop(await_asset(&req));
        }

        #[test]
        fn pending_returns_existing()
        {
            let lifecyclers = AssetLifecyclers::default()
                .add_lifecycler(TestAssetLifecycler::default());
            let assets = Assets::new(lifecyclers, AssetsConfig::test());

            const READY_WAITER: Mutex<()> = Mutex::new(()); // gross

            set_passthru::<_, TestAssetLifecycler>(&assets, Some(|_req: AssetLoadRequest|
            {
                let _ = READY_WAITER.lock();
                Ok(TestAsset { name: "pending asset".to_string(), nested: None })
            }));

            assert_eq!(Some(0), get_passthru_call_count::<TestAssetLifecycler>(&assets));

            let _req1: Ash<TestAsset> = assets.load_from::<TestAsset>(TEST_ASSET_1, Cursor::new([]));
            await_asset(&_req1);
            assert_eq!(Some(1), get_passthru_call_count::<TestAssetLifecycler>(&assets));

            let _req2: Ash<TestAsset>;
            {
                let _ = READY_WAITER.lock();
                _req2 = assets.load_from::<TestAsset>(TEST_ASSET_1, Cursor::new([]));
                // TODO: this appears to be non-deterministic
                assert_eq!(Some(1), get_passthru_call_count::<TestAssetLifecycler>(&assets));
            }
            await_asset(&_req2);
            assert_eq!(Some(2), get_passthru_call_count::<TestAssetLifecycler>(&assets));
        }

        #[test]
        fn available()
        {
            let lifecyclers = AssetLifecyclers::default()
                .add_lifecycler(TestAssetLifecycler::default())
                .add_lifecycler(NestedAssetLifecycler::default());
            let assets = Assets::new(lifecyclers, AssetsConfig::test());

            set_passthru::<_, TestAssetLifecycler>(&assets, Some(|_req: AssetLoadRequest|
            {
                Ok(TestAsset { name: "test asset".to_string(), nested: None })
            }));
            set_passthru::<_, NestedAssetLifecycler>(&assets, Some(|_req: AssetLoadRequest|
            {
                Ok(NestedAsset { id: 123 })
            }));
            // TODO: broken

            let req: Ash<TestAsset> = assets.load_direct_from::<TestAsset>(TEST_ASSET_1, Cursor::new([]));
            assert!(req.is_loaded_recursive());

            let req2: Ash<NestedAsset> = assets.load_from::<NestedAsset>(TEST_ASSET_2, Cursor::new([]));
            match await_asset(&req2)
            {
                AssetPayload::Available(a) => assert_eq!(a.id, 123),
                other => panic!("Asset not available: {other:?}"),
            }
            assert!(req2.is_loaded_recursive());
        }

        #[test]
        fn load_from()
        {
            let lifecyclers = AssetLifecyclers::default()
                .add_lifecycler(TestAssetLifecycler::default());
            let assets = Assets::new(lifecyclers, AssetsConfig::test());

            let loaded_asset_name = "loaded asset name";
            set_passthru::<_, TestAssetLifecycler>(&assets, Some(|mut req: AssetLoadRequest|
            {
                let mut name = String::new();
                req.input.read_to_string(&mut name)?;
                Ok(TestAsset { name, nested: None })
            }));

            let input_bytes = Cursor::new(loaded_asset_name.as_bytes());
            let req = assets.load_from::<TestAsset>(TEST_ASSET_1, input_bytes);
            match await_asset(&req)
            {
                AssetPayload::Available(a) => assert_eq!(a.name, loaded_asset_name),
                _ => panic!("Asset not available"),
            }
        }

        #[test]
        fn reload()
        {
            let lifecyclers = AssetLifecyclers::default()
                .add_lifecycler(TestAssetLifecycler::default());
            let assets = Assets::new(lifecyclers, AssetsConfig::test());

            let first_asset_name = "first";
            let second_asset_name = "second";

            set_passthru::<_, TestAssetLifecycler>(&assets, Some(|mut req: AssetLoadRequest|
            {
                let mut name = String::new();
                req.input.read_to_string(&mut name)?;
                Ok(TestAsset { name, nested: None })
            }));

            let mut input_bytes = Cursor::new(first_asset_name.as_bytes());
            let mut req = assets.load_from::<TestAsset>(TEST_ASSET_1, input_bytes);
            match await_asset(&req)
            {
                AssetPayload::Available(a) => assert_eq!(a.name, first_asset_name),
                _ => panic!("Asset not available"),
            }

            input_bytes = Cursor::new(second_asset_name.as_bytes());
            req = assets.load_from::<TestAsset>(TEST_ASSET_1, input_bytes);
            std::thread::sleep(Duration::from_millis(10)); // TODO: HACK
            match await_asset(&req)
            {
                AssetPayload::Available(a) => assert_eq!(a.name, second_asset_name),
                _ => panic!("Asset not available"),
            }
        }
    }

    // TODO: asset dependency lifetimes
    // TODO: generation
}
