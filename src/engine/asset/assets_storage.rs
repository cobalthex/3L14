use super::*;
use crate::engine::graphics::debug_gui::DebugGui;
use crossbeam::channel::{unbounded, Receiver, Sender};
use egui::Ui;
use notify::event::ModifyKind;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{Debouncer, FileIdMap};
use parking_lot::Mutex;
use std::any::TypeId;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::{Builder, JoinHandle};
use std::time::Duration;
// TODO: probably don't pass around UniCase publicly

type AssetHandleBank = HashMap<AssetKey, UntypedAssetHandle>;

pub enum AssetNotification
{
    Reload(AssetKey),
}

pub(super) struct AssetsStorage
{
    registered_asset_types: HashMap<AssetTypeId, RegisteredAssetType>,
    lifecyclers: HashMap<AssetTypeId, Box<dyn UntypedAssetLifecycler>>,

    handles: Mutex<AssetHandleBank>,
    lifecycle_channel: Sender<AssetLifecycleRequest>,
    notification_channel: (Sender<AssetNotification>, Receiver<AssetNotification>),

    assets_root: PathBuf, // should be absolute
}
impl AssetsStorage
{
    #[must_use]
    fn create_or_update_handle<A: Asset>(&self, asset_key: AssetKey) -> (bool /* pre-existing */, AssetHandle<A>)
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

        (pre_existing, unsafe { AssetHandle::<A>::clone_from(handle) })
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

    #[inline]
    #[must_use]
    pub fn get_lifecycler<A: Asset>(&self) -> Option<&dyn UntypedAssetLifecycler>
    {
        self.lifecyclers.get(&A::asset_type()).map(|l| l.as_ref())
    }

    #[must_use]
    pub fn enqueue_load<A: Asset, F: FnOnce(UntypedAssetHandle) -> AssetLifecycleRequest>(
        self: &Arc<Self>,
        asset_key: AssetKey,
        input_fn: F) -> AssetHandle<A>
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
    pub fn asset_key_to_file_path(&self, asset_key: AssetKey) -> PathBuf
    {
        self.assets_root.as_path().join(asset_key.as_file_name())
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
                                            let lifecycler = self.lifecyclers.get(&asset_type)
                                                .expect("Unsupported asset type!"); // this should fail in load()

                                            lifecycler.error_untyped(untyped_handle, AssetLoadError::Shutdown);
                                        }
                                    }
                                }

                                break 'worker;
                            },
                            AssetLifecycleRequest::LoadFileBacked(untyped_handle) =>
                            {
                                let inner = untyped_handle.as_ref();
                                let lifecycler = self.lifecyclers.get(&inner.asset_type())
                                    .expect("Unsupported asset type!"); // this should fail in load()

                                let asset_file_path = self.asset_key_to_file_path(inner.key());
                                let reader = match Self::open_asset_from_file(asset_file_path)
                                {
                                    Ok(read) => read,
                                    Err(err) =>
                                    {
                                        log::warn!("Failed to read asset file {:?}: {err}", self.asset_key_to_file_path(inner.key()));
                                        lifecycler.error_untyped(untyped_handle, AssetLoadError::Fetch);
                                        return;
                                    }
                                };

                                lifecycler.load_untyped(self.clone(), untyped_handle, Box::new(reader));
                            },
                            AssetLifecycleRequest::LoadFromMemory(untyped_handle, reader) =>
                            {
                                let lifecycler = self.lifecyclers.get(&untyped_handle.as_ref().asset_type())
                                    .expect("Unsupported asset type!"); // this should fail in load()

                                lifecycler.load_untyped(self.clone(), untyped_handle, reader);
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
    pub enable_fs_watcher: bool,
}
impl AssetsConfig
{
    #[cfg(test)]
    pub fn test() -> Self
    {
        Self { enable_fs_watcher: false }
    }
}

const NUM_ASSET_JOB_THREADS: usize = 1;
pub struct Assets
{
    storage: Arc<AssetsStorage>,
    fs_watcher: Option<Debouncer<RecommendedWatcher, FileIdMap>>,
    worker_threads: [Option<JoinHandle<()>>; NUM_ASSET_JOB_THREADS],
}
impl Assets
{
    pub fn new(asset_lifecyclers: AssetLifecyclers, config: AssetsConfig) -> Self
    {
        let mut assets_root = std::env::current_exe().expect("Failed to get the bin dir");
        assets_root.pop();
        assets_root.push("assets"); // canonicalize?

        #[cfg(debug_assertions)]
        log::debug!("Serving assets from {assets_root:?}");

        // TODO: pqueue

        let (lifecycle_send, lifecycle_recv) = unbounded::<AssetLifecycleRequest>();
        let storage = Arc::new(AssetsStorage
        {
            registered_asset_types: asset_lifecyclers.registered_asset_types,
            lifecyclers: asset_lifecyclers.lifecyclers,
            handles: Mutex::new(AssetHandleBank::new()),
            lifecycle_channel: lifecycle_send,
            notification_channel: unbounded::<AssetNotification>(),

            assets_root,
        });


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
        let worker_threads = array_init::array_init::<_, _, NUM_ASSET_JOB_THREADS>(|i|
        {
            let thread = Builder::new()
                .name(format!("Asset worker thread {}", i))
                .spawn(AssetsStorage::asset_worker_fn(storage.clone(), lifecycle_recv.clone())).expect("Failed to create asset worker thread");
            Some(thread)
        });

        Self
        {
            storage,
            fs_watcher,
            worker_threads,
        }
    }

    pub fn subscribe_to_notifications(&self) -> Receiver<AssetNotification>
    {
        self.storage.notification_channel.1.clone()
    }

    fn try_fs_watch(assets_storage: Arc<AssetsStorage>) -> notify::Result<Debouncer<RecommendedWatcher, FileIdMap>>
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
        fs_watcher.watcher().watch(assets_path, RecursiveMode::Recursive)?;
        Ok(fs_watcher)
    }

    #[must_use]
    pub fn load<A: Asset>(&self, asset_key: AssetKey) -> AssetHandle<A>
    {
        self.storage.enqueue_load(asset_key, |h| AssetLifecycleRequest::LoadFileBacked(h))
    }

    #[must_use]
    pub fn load_from<A: Asset, R: AssetRead + 'static /* static not ideal here */>(
        &self,
        asset_key: AssetKey,
        input_data: R // take box?,
    ) -> AssetHandle<A>
    {
        self.storage.enqueue_load(asset_key, |h| AssetLifecycleRequest::LoadFromMemory(h, Box::new(input_data)))
    }

    pub fn num_active_assets(&self) -> usize
    {
        let handles = self.storage.handles.lock();
        handles.len()
    }

    // prevent any new asset from being loaded
    pub fn shutdown(&self)
    {
        let _ = self.storage.lifecycle_channel.send(AssetLifecycleRequest::StopWorkers); // will error if already closed
    }
}
impl<'i, 'a: 'i> DebugGui<'a> for Assets
{
    fn name(&self) -> &'a str
    {
        "Assets"
    }
    fn debug_gui(&'a self, ui: &mut Ui)
    {
        // for (_, lifecycler) in self.storage.lifecyclers
        // {
        //     egui::CollapsingHeader::new(lifecycler.name())
        //         .default_open(true)
        //         .show(ui, |cui|
        //         {
        //             lifecycler.debug_gui(cui);
        //         });
        // }

        let mut has_fswatcher = self.fs_watcher.is_some();
        ui.checkbox(&mut has_fswatcher, "FS watcher enabled");

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
        self.fs_watcher = None;
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
        nested: Option<AssetHandle<NestedAsset>>,
    }
    impl Asset for TestAsset
    {
        fn asset_type() -> AssetTypeId { AssetTypeId::Test1 }
    }

    struct Passthru
    {
        call_count: usize,
        passthru_fn: fn(AssetLoadRequest) -> Result<TestAsset, Box<dyn Error>>,
    }

    #[derive(Default)]
    struct TestAssetLifecycler
    {
        active_count: AtomicUsize,
        pub passthru: Mutex<Option<Passthru>>,
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

    fn set_passthru(assets: &Assets, passthru_fn: Option<fn(AssetLoadRequest) -> Result<TestAsset, Box<dyn Error>>>)
    {
        let lifecycler = assets.storage.get_lifecycler::<TestAsset>().unwrap();
        let tal = unsafe { &*(lifecycler as *const dyn UntypedAssetLifecycler as *const TestAssetLifecycler) };
        *tal.passthru.lock() = passthru_fn.map(|pfn| Passthru
        {
            call_count: 0,
            passthru_fn: pfn,
        });
    }

    fn get_passthru_call_count(assets: &Assets) -> Option<usize>
    {
        let lifecycler = assets.storage.get_lifecycler::<TestAsset>().unwrap();
        let tal = unsafe { &*(lifecycler as *const dyn UntypedAssetLifecycler as *const TestAssetLifecycler) };
        tal.passthru.lock().as_ref().map(|p| p.call_count)
    }

    fn await_asset<A: Asset>(handle: &AssetHandle<A>) -> AssetPayload<A>
    {
        futures::executor::block_on(handle)
    }

    mod load
    {
        use super::*;
        use crate::engine::DataPayload::*;
        use std::io::Cursor;

        // TODO: disable threading and add 'loop_once' function for worker

        #[test]
        #[should_panic]
        fn missing_lifecycler()
        {
            let assets = Assets::new(AssetLifecyclers::default(), AssetsConfig::test());

            let req: AssetHandle<TestAsset> = assets.load_from::<TestAsset, _>(TEST_ASSET_1, Cursor::new([]));
            await_asset(&req);
        }

        #[test]
        fn bad_file()
        {
            let lifecyclers = AssetLifecyclers::default()
                .add_lifecycler(TestAssetLifecycler::default());
            let assets = Assets::new(lifecyclers, AssetsConfig::test());

            let req: AssetHandle<TestAsset> = assets.load::<TestAsset>(TEST_ASSET_1);
            match await_asset(&req)
            {
                Unavailable(AssetLoadError::Fetch) => {},
                other => panic!("Invalid load result: {other:#?}"),
            }
        }

        #[test]
        fn unavailable()
        {
            let lifecyclers = AssetLifecyclers::default()
                .add_lifecycler(TestAssetLifecycler::default());
            let assets = Assets::new(lifecyclers, AssetsConfig::test());

            set_passthru(&assets, Some(|_req: AssetLoadRequest|
            {
                Err(Box::new(TestError))
            }));

            let req: AssetHandle<TestAsset> = assets.load_from::<TestAsset, _>(TEST_ASSET_1, Cursor::new([]));
            match await_asset(&req)
            {
                Unavailable(AssetLoadError::Parse) => {},
                other => panic!("Asset not unavailable(Test): {other:#?}"),
            }
        }

        #[test]
        fn pending()
        {
            let lifecyclers = AssetLifecyclers::default()
                .add_lifecycler(TestAssetLifecycler::default());
            let assets = Assets::new(lifecyclers, AssetsConfig::test());

            let req: AssetHandle<TestAsset> = assets.load_from::<TestAsset, _>(TEST_ASSET_1, Cursor::new([]));
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

            set_passthru(&assets, Some(|_req: AssetLoadRequest| Err(Box::new(TestError))));

            assert_eq!(Some(0), get_passthru_call_count(&assets));

            let _req1: AssetHandle<TestAsset> = assets.load_from::<TestAsset, _>(TEST_ASSET_1, Cursor::new([]));
            std::thread::sleep(std::time::Duration::from_secs(1)); // crude
            assert_eq!(Some(1), get_passthru_call_count(&assets));

            let _req2: AssetHandle<TestAsset> = assets.load_from::<TestAsset, _>(TEST_ASSET_1, Cursor::new([]));
            assert_eq!(Some(1), get_passthru_call_count(&assets));

            let _req3: AssetHandle<TestAsset> = assets.load_from::<TestAsset, _>(TEST_ASSET_1, Cursor::new([]));
            assert_eq!(Some(1), get_passthru_call_count(&assets));
        }

        #[test]
        fn available()
        {
            let lifecyclers = AssetLifecyclers::default()
                .add_lifecycler(TestAssetLifecycler::default());
            let assets = Assets::new(lifecyclers, AssetsConfig::test());

            set_passthru(&assets, Some(|_req: AssetLoadRequest|
            {
                Ok(TestAsset { name: "test asset".to_string(), nested: None })
            }));

            let req: AssetHandle<TestAsset> = assets.load_from::<TestAsset, _>(TEST_ASSET_1, Cursor::new([]));
            let dup = req.clone();
            match await_asset(&req)
            {
                AssetPayload::Available(a) => assert_eq!(a.name, "test asset"),
                _ => panic!("Asset not available"),
            }

            assert!(dup.is_loaded_recursive());
        }

        #[test]
        fn load_from()
        {
            let lifecyclers = AssetLifecyclers::default()
                .add_lifecycler(TestAssetLifecycler::default());
            let assets = Assets::new(lifecyclers, AssetsConfig::test());

            let loaded_asset_name = "loaded asset name";
            set_passthru(&assets, Some(|mut req: AssetLoadRequest|
            {
                let mut name = String::new();
                req.input.read_to_string(&mut name)?;
                Ok(TestAsset { name, nested: None })
            }));

            let input_bytes = Cursor::new(loaded_asset_name.as_bytes());
            let req = assets.load_from::<TestAsset, _>(TEST_ASSET_1, input_bytes);
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

            set_passthru(&assets, Some(|mut req: AssetLoadRequest|
            {
                let mut name = String::new();
                req.input.read_to_string(&mut name)?;
                Ok(TestAsset { name, nested: None })
            }));

            let mut input_bytes = Cursor::new(first_asset_name.as_bytes());
            let mut req = assets.load_from::<TestAsset, _>(TEST_ASSET_1, input_bytes);
            match await_asset(&req)
            {
                AssetPayload::Available(a) => assert_eq!(a.name, first_asset_name),
                _ => panic!("Asset not available"),
            }

            input_bytes = Cursor::new(second_asset_name.as_bytes());
            req = assets.load_from::<TestAsset, _>(TEST_ASSET_1, input_bytes);
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
