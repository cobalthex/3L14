use std::alloc::Layout;
use std::any::{type_name, Any, TypeId};
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::thread::{Builder, JoinHandle};
use std::time::Duration;
use arc_swap::ArcSwapOption;
use crossbeam::channel::{Sender, Receiver, unbounded};
use egui::Ui;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{Debouncer, FileIdMap};
use parking_lot::Mutex;
use unicase::UniCase;
use crate::engine::graphics::debug_gui::DebugGui;
use crate::engine::ShortTypeName;
use super::*;

struct AssetHandleEntry
{
    untyped_handle: UntypedHandleInner,
    asset_type: TypeId, // assets could probably be bucketed by this
    // handle_alloc_layout: Layout,

    #[cfg(debug_assertions)] // don't expose the asset type in release?
    asset_type_name: &'static str,
}


type AssetHandleBank = HashMap<AssetKey, AssetHandleEntry>; // TODO: this could be done smarter

pub(super) struct AssetsStorage
{
    handles: Mutex<AssetHandleBank>,
    lifecyclers: HashMap<TypeId, Box<dyn UntypedAssetLifecycler>>,
    lifecycle_channel: Sender<AssetLifecycleRequest>,

    // asset paths to assets map
    asset_paths_to_keys: Mutex<HashMap<UniCase<String>, AssetKey>>,

    assets_root: PathBuf, // should be absolute
}
impl AssetsStorage
{
    #[must_use]
    fn create_or_update_handle<A: Asset, S: AssetPath>(&self, asset_path: &S) -> (bool /* pre-existing */, AssetHandle<A>)
    {
        let uncased_asset_path = UniCase::new(asset_path.to_string());
        let key_desc = AssetKeyDesc
        {
            path: &uncased_asset_path,
            type_id: TypeId::of::<A>(),
        };
        let asset_key = (&key_desc).into();

        let mut handle_bank = self.handles.lock();

        let mut pre_existing = true;
        let handle_entry = handle_bank.entry(asset_key).or_insert_with(||
        {
            pre_existing = false;
            /*
            Memory for assets is managed inside the assets storage

            This prevents a race between the handle being dropped, deleting its memory,
              and this function returning an existing (stale) handle.

            By having the pointer created and destroyed by this class,
              there is serialization provided by the mutex preventing use-after-free issues.
            */

            let inner: AssetHandleInner<A> = AssetHandleInner
            {
                ref_count: AtomicIsize::new(0), // this will be incremented below
                path: uncased_asset_path,
                key: asset_key,
                dropper: self.lifecycle_channel.clone(),
                ready_waker: Mutex::new(None),
                generation: AtomicU32::new(0),
                is_reloading: AtomicBool::new(false),
                payload: ArcSwapOption::new(None),
            };
            let layout = Layout::for_value(&inner);
            let ptr = unsafe
            {
                let alloc: *mut AssetHandleInner<A> = std::alloc::alloc(layout).cast();
                std::ptr::write(alloc, inner);
                UntypedHandleInner::new(alloc)
            };

            self.asset_paths_to_keys.lock().insert(UniCase::new(asset_path.to_string()), asset_key);

            AssetHandleEntry
            {
                untyped_handle: ptr,
                asset_type: TypeId::of::<A>(),
                //handle_alloc_layout: layout,
                #[cfg(debug_assertions)]
                asset_type_name: A::short_type_name(),
            }
        });
        let handle = unsafe { AssetHandle::clone_from_untyped(&handle_entry.untyped_handle) };

        (pre_existing, handle)
    }

    fn drop_handle(&self, untyped_handle: UntypedHandleInner)
    {
        let mut handle_bank = self.handles.lock(); // must lock before below to make sure that the handle doesn't get cloned between the drop and below

        // retrieve this info after locking the handle bank so a zero refcount cannot be added to by a load()
        let handle = unsafe { untyped_handle.as_unchecked::<_NullAsset>() };
        if handle.ref_count() != 0
        {
            return;
        }

        match handle_bank.remove(&handle.key)
        {
            None =>
            {
                // without an entry we don't know the size, and cannot free the asset handle
                panic!("Tried to remove a handle that was not registered to this AssetsStorage");
            }
            Some(entry) =>
            unsafe {
                debug_assert_eq!(untyped_handle, entry.untyped_handle);

                // todo: the lifecycler fetch can be moved above to remove the _NullAsset hack
                // proper typing here is necessary to call the correct destructors
                if let Some(lifecycler) = self.lifecyclers.get(&entry.asset_type)
                {
                    let lifecycler = self.lifecyclers.get(&entry.asset_type).expect("No lifecycler for this handle");
                    lifecycler.drop_untyped(&untyped_handle);
                }
                else
                {
                    // TODO: assert that payload is unavailable
                }

                self.asset_paths_to_keys.lock().remove(&handle.path);

                // note: if asset payloads are not stored internally by pointer, this will need to change
                let layout = Layout::for_value(handle);
                std::alloc::dealloc(untyped_handle.into_ptr(), layout);

                //std::alloc::dealloc(request.handle.into_ptr(), entry.handle_alloc_layout);
            }
        }
    }

    #[inline]
    #[must_use]
    pub fn get_lifecycler<A: Asset>(&self) -> Option<&dyn UntypedAssetLifecycler>
    {
        self.lifecyclers.get(&TypeId::of::<A>()).map(|l| l.as_ref())
    }

    #[must_use]
    pub fn enqueue_load<A: Asset, S: AssetPath, F: FnOnce() -> AssetLifecycleRequestKind>(
        self: &Arc<Self>,
        asset_path: &S,
        update_if_exists: bool,
        input_fn: F) -> AssetHandle<A>
    {
        let (pre_existed, asset_handle) = self.create_or_update_handle(asset_path);
        if pre_existed
        {
            match update_if_exists
            {
                true =>
                {
                    eprintln!("Reloading asset <{}> '{}'", type_name::<A>(), asset_path); // return entry which has name?
                    asset_handle.inner().is_reloading.store(true, Ordering::Release); // correct ordering?
                },
                false => { return asset_handle; }
            }
        }

        if self.lifecyclers.contains_key(&TypeId::of::<A>())
        {
            let request = AssetLifecycleRequest
            {
                asset_type: TypeId::of::<A>(),
                untyped_handle: unsafe { asset_handle.clone().into_untyped() },
                kind: input_fn(),
            };

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

    #[inline]
    pub fn translate_asset_path<S: AssetPath>(&self, asset_path: S) -> std::io::Result<PathBuf>
    {
        let path = self.assets_root.as_path().join(Path::new(asset_path.as_ref())).canonicalize()?;
        if path.starts_with(&self.assets_root) { Ok(path) } else { Err(std::io::Error::from(ErrorKind::InvalidInput)) }
    }

    #[inline]
    fn open_asset_from_file(file_path: PathBuf) -> Result<impl AssetReader, std::io::Error>
    {
        std::fs::File::open(file_path)
    }

    pub fn asset_worker_fn(self: Arc<Self>, request_recv: Receiver<AssetLifecycleRequest>) -> impl FnOnce()
    {
        move ||
        {
            eprintln!("Starting asset worker thread");
            'worker: loop
            {
                match request_recv.recv()
                {
                    Ok(request) =>
                    {
                        puffin::profile_scope!("Asset lifecycle request");

                        // note: request.handle must be managed manually here

                        match request.kind
                        {
                            AssetLifecycleRequestKind::StopWorkers =>
                            {
                                #[cfg(debug_assertions)]
                                if !request.untyped_handle.is_null()
                                {
                                    panic!("Asset handle was not null in a 'StopWorkers' asset request");
                                }
                                eprintln!("Shutting down asset worker thread");

                                // clean out any final drop requests
                                while let Ok(final_request) = request_recv.try_recv()
                                {
                                    match final_request.kind
                                    {
                                        AssetLifecycleRequestKind::StopWorkers =>
                                        {
                                            #[cfg(debug_assertions)]
                                            if !request.untyped_handle.is_null()
                                            {
                                                panic!("Asset handle was not null in a 'StopWorkers' asset request");
                                            }
                                        }
                                        AssetLifecycleRequestKind::Drop =>
                                        {
                                            self.drop_handle(final_request.untyped_handle);
                                        }
                                        _ =>
                                        {
                                            let lifecycler = self.lifecyclers.get(&final_request.asset_type)
                                                .expect("Unsupported asset type!"); // this should fail in load()

                                            lifecycler.error_untyped(final_request.untyped_handle, AssetLoadError::Shutdown);
                                        }
                                    }
                                }

                                break 'worker;
                            },
                            AssetLifecycleRequestKind::LoadFileBacked =>
                            {
                                let lifecycler = self.lifecyclers.get(&request.asset_type)
                                    .expect("Unsupported asset type!"); // this should fail in load()

                                let asset_path = unsafe { &request.untyped_handle.as_unchecked::<_NullAsset>().path };
                                let asset_file_path = match self.translate_asset_path(asset_path)
                                {
                                    Err(err) =>
                                    {
                                        eprintln!("Invalid asset file path '{asset_path}': {err}");
                                        lifecycler.error_untyped(request.untyped_handle, AssetLoadError::IOError(err.kind()));
                                        return;
                                    }
                                    Ok(p) => p,
                                };
                                let reader = match Self::open_asset_from_file(asset_file_path)
                                {
                                    Ok(read) => read,
                                    Err(err) =>
                                    {
                                        eprintln!("Failed to read asset file '{asset_path}': {err}");
                                        lifecycler.error_untyped(request.untyped_handle, AssetLoadError::IOError(err.kind()));
                                        return;
                                    }
                                };

                                lifecycler.load_untyped(self.clone(), request.untyped_handle, Box::new(reader));
                            },
                            AssetLifecycleRequestKind::LoadFromMemory(reader) =>
                            {
                                let lifecycler = self.lifecyclers.get(&request.asset_type)
                                    .expect("Unsupported asset type!"); // this should fail in load()

                                lifecycler.load_untyped(self.clone(), request.untyped_handle, reader);
                            },
                            AssetLifecycleRequestKind::Drop =>
                            {
                                self.drop_handle(request.untyped_handle);
                            },
                        }
                    },
                    Err(err) =>
                    {
                        eprintln!("Terminating asset worker thread due to {err}");
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
        let assets_root = Path::new("assets/").canonicalize().expect("Failed to parse assets_root path");

        let (send, recv) = unbounded::<AssetLifecycleRequest>();
        let storage = Arc::new(AssetsStorage
        {
            handles: Mutex::new(AssetHandleBank::new()),
            lifecyclers: asset_lifecyclers.lifecyclers,
            lifecycle_channel: send,
            asset_paths_to_keys: Mutex::new(HashMap::new()),
            assets_root,
        });


        let fs_watcher = if config.enable_fs_watcher { Self::try_fs_watch(storage.clone()).inspect_err(|err|
        {
            // TODO: print message on successful startup
            eprintln!("Failed to start fs watcher for hot-reloading, continuing without: {err:?}");
        }).ok() } else { None };

        // hot reload batching?

        /* TODO: broadcast asset change notifications? -- do per-lifecycler? (both options?)

            + more efficient than indirection
            + dedicated 'watchers'

            - likely more of a spaghetti mess
            - possibly awkward lifetimes

            ~ still need to hold references to assets, but payload can be queried once
            ~ dependency chain management - if dependencies are held by handles rather than payloads, there could be corruption (but mid chain could be updated and not parents)
                - possibly needs tree locking, will need to explore this

            - assets all require two indirections right now, which is inefficient

         */

        // todo: async would maybe nice here (file/network IO, multi-part loads)
        let worker_threads = array_init::array_init::<_, _, NUM_ASSET_JOB_THREADS>(|i|
        {
            let thread = Builder::new()
                .name(format!("Asset worker thread {}", i))
                .spawn(AssetsStorage::asset_worker_fn(storage.clone(), recv.clone())).expect("Failed to create asset worker thread");
            Some(thread)
        });

        Self
        {
            storage,
            fs_watcher,
            worker_threads,
        }
    }

    fn try_fs_watch(assets_storage: Arc<AssetsStorage>) -> notify::Result<Debouncer<RecommendedWatcher, FileIdMap>>
    {
        // batching?
        let mut fs_watcher = notify_debouncer_full::new_debouncer(
            Duration::from_secs(1),
            None,
            |evt: notify_debouncer_full::DebounceEventResult|
            {
                match evt
                {
                    Ok(events) =>
                    {
                        for event in events
                        {
                            if let EventKind::Modify(m) = event.kind
                            {
                                eprintln!("Mod: {m:?}");
                            }
                            // TODO: delete events?
                        }
                    },
                    Err(e) => println!("FS watch error: {:?}", e),
                }
            })?;

        let assets_path = assets_storage.assets_root.as_path();
        fs_watcher.watcher().watch(assets_path, RecursiveMode::Recursive)?;
        Ok(fs_watcher)
    }

    #[must_use]
    pub fn load<A: Asset, S: AssetPath>(&self, asset_path: &S) -> AssetHandle<A>
    {
        self.storage.enqueue_load(asset_path, false,
                                   || AssetLifecycleRequestKind::LoadFileBacked)
    }

    #[must_use]
    pub fn load_from<A: Asset, S: AssetPath, R: AssetReader + 'static /* static not ideal here */>(
        &self,
        asset_path: &S,
        input_data: R, // take box?,
        update_if_exists: bool,
    ) -> AssetHandle<A>
    {
        self.storage.enqueue_load(asset_path, update_if_exists,
                                   || AssetLifecycleRequestKind::LoadFromMemory(Box::new(input_data)))
    }

    pub fn num_active_assets(&self) -> usize
    {
        let handles = self.storage.handles.lock();
        handles.len()
    }

    // Returns true if an asset belonging to the asset key has been loaded
    // Returns false on error/pending, or if the asset does not exist
    // Does not check if dependencies are loaded (requires an asset handle)
    pub fn is_loaded_no_dependencies(&self, key: AssetKey) -> bool
    {
        let handles = self.storage.handles.lock();
        match handles.get(&key)
        {
            None => false,
            Some(entry) =>
            {
                // as long as handles is locked, this handle will always be valid
                let inner = unsafe { entry.untyped_handle.as_unchecked::<_NullAsset>() };
                // not safe to call virtual methods with a punned handle
                //std::mem::discriminant(&*inner.payload()) == std::mem::discriminant(&AssetPayload::Available(NullAsset))
                inner.payload().is_available()
            }
        }
    }

    // prevent any new assets from being loaded
    pub fn shutdown(&self)
    {
        let _ = self.storage.lifecycle_channel.send(AssetLifecycleRequest
        {
            asset_type: TypeId::of::<()>(),
            untyped_handle: UntypedHandleInner::NULL,
            kind: AssetLifecycleRequestKind::StopWorkers,
        }); // will error if already closed
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
                    .num_columns(6)
                    .show(cui, |gui|
                        {
                            gui.heading("Key");
                            gui.heading("Refs");
                            gui.heading("Gen");
                            gui.heading("Type");
                            // gui.heading("State");
                            gui.heading("Path");
                            gui.end_row();

                            for (key, entry) in handle_bank.iter()
                            {
                                // as long as handle_bank is locked, this handle will always be valid
                                // TODO: get this information in a safe way
                                let handle_unsafe = unsafe { entry.untyped_handle.as_unchecked::<_NullAsset>() };

                                gui.label(format!("{key:#?}")); // right click to copy?
                                gui.label(format!("{}", handle_unsafe.ref_count()));
                                gui.label(format!("{}", handle_unsafe.generation()));

                                // TODO: this cleaner
                                #[cfg(debug_assertions)]
                                gui.label(entry.asset_type_name);
                                #[cfg(not(debug_assertions))]
                                gui.label("<UNKNOWN>");

                                // TODO: query availability

                                gui.label(handle_unsafe.path.as_ref());
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
            eprintln!("! Leak detected: {} active asset handle(s):", handle_bank.len());
            for handle in handle_bank.iter()
            {
                eprintln!("    {:?} <{}> '{}'", handle.0, handle.1.asset_type_name, "TODO - PATH"); // TODO
            }
            #[cfg(test)]
            panic!("Leaked assets!");
        }
        handle_bank.clear()
    }
}

#[cfg(test)]
mod tests
{
    use std::sync::atomic::AtomicUsize;
    use super::*;

    #[derive(Debug)]
    struct NestedAsset
    {
        id: usize,
    }
    impl Asset for NestedAsset { }

    #[derive(Debug)]
    struct TestAsset
    {
        name: String,
        nested: Option<AssetHandle<NestedAsset>>,
    }
    impl Asset for TestAsset { }

    struct Passthru
    {
        call_count: usize,
        passthru_fn: fn(AssetLoadRequest) -> AssetPayload<TestAsset>,
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
        fn load(&self, request: AssetLoadRequest) -> AssetPayload<Self::Asset>
        {
            match &mut *self.passthru.lock()
            {
                None => AssetPayload::Unavailable(AssetLoadError::Test(0)),
                Some(passthru) =>
                {
                    passthru.call_count += 1;
                    (passthru.passthru_fn)(request)
                },
            }
        }
    }

    fn set_passthru(assets: &Assets, passthru_fn: Option<fn(AssetLoadRequest) -> AssetPayload<TestAsset>>)
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

    fn await_asset<A: Asset>(handle: AssetHandle<A>) -> Arc<AssetPayload<A>>
    {
        futures::executor::block_on(&handle)
    }

    mod load
    {
        use std::io::Cursor;
        use super::*;
        use crate::engine::DataPayload::*;

        #[test]
        fn missing_lifecycler()
        {
            let assets = Assets::new(AssetLifecyclers::default(), AssetsConfig::test());

            let req: AssetHandle<TestAsset> = assets.load::<TestAsset, _>(&"any_file");
            match &*await_asset(req)
            {
                Unavailable(AssetLoadError::LifecyclerNotRegistered) => {},
                other => panic!("Invalid load result: {other:#?}"),
            }
        }

        #[test]
        fn bad_file()
        {
            let lifecyclers = AssetLifecyclers::default()
                .add_lifecycler(TestAssetLifecycler::default());
            let assets = Assets::new(lifecyclers, AssetsConfig::test());

            let req: AssetHandle<TestAsset> = assets.load::<TestAsset, _>(&"$BAD_FILE$");
            match &*await_asset(req)
            {
                Unavailable(AssetLoadError::IOError(_)) => {},
                other => panic!("Invalid load result: {other:#?}"),
            }
        }

        #[test]
        fn unavailable()
        {
            let lifecyclers = AssetLifecyclers::default()
                .add_lifecycler(TestAssetLifecycler::default());
            let assets = Assets::new(lifecyclers, AssetsConfig::test());

            set_passthru(&assets, Some(|req: AssetLoadRequest|
            {
                Unavailable(AssetLoadError::Test(0))
            }));

            let req: AssetHandle<TestAsset> = assets.load::<TestAsset, _>(&"test_asset_file");
            match &*await_asset(req)
            {
                Unavailable(AssetLoadError::Test(0)) => {},
                other => panic!("Asset not unavailable(Test): {other:#?}"),
            }
        }

        #[test]
        fn pending()
        {
            let lifecyclers = AssetLifecyclers::default()
                .add_lifecycler(TestAssetLifecycler::default());
            let assets = Assets::new(lifecyclers, AssetsConfig::test());

            let req = assets.load::<TestAsset, _>(&"test_asset_file");
            match *req.payload()
            {
                AssetPayload::Pending => {},
                _ => panic!("Asset not pending"),
            }

            drop(await_asset(req));
        }

        #[test]
        fn pending_returns_existing()
        {
            let lifecyclers = AssetLifecyclers::default()
                .add_lifecycler(TestAssetLifecycler::default());
            let assets = Assets::new(lifecyclers, AssetsConfig::test());

            set_passthru(&assets, Some(|_req: AssetLoadRequest| Unavailable(AssetLoadError::Test(0))));

            assert_eq!(Some(0), get_passthru_call_count(&assets));

            let _req1 = assets.load::<TestAsset, _>(&"test_asset_file");
            std::thread::sleep(std::time::Duration::from_secs(1)); // crude
            assert_eq!(Some(1), get_passthru_call_count(&assets));

            let _req2 = assets.load::<TestAsset, _>(&"test_asset_file");
            assert_eq!(Some(1), get_passthru_call_count(&assets));

            let _req3 = assets.load::<TestAsset, _>(&"test_asset_file");
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
                Available(TestAsset { name: "test asset".to_string(), nested: None })
            }));

            let req = assets.load::<TestAsset, _>(&"test_asset_file");
            match &*await_asset(req)
            {
                AssetPayload::Available(a) => assert_eq!(a.name, "test asset"),
                _ => panic!("Asset not available"),
            }
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
                match req.input.read_to_string(&mut name)
                {
                    Ok(_) => Available(TestAsset { name, nested: None }),
                    Err(_) => Unavailable(AssetLoadError::ParseError(0)),
                }
            }));

            let input_bytes = Cursor::new(loaded_asset_name.as_bytes());
            let req = assets.load_from::<TestAsset, _, _>(&"test_asset_file", input_bytes, false);
            match &*await_asset(req)
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
                match req.input.read_to_string(&mut name)
                {
                    Ok(_) => Available(TestAsset { name, nested: None }),
                    Err(_) => Unavailable(AssetLoadError::ParseError(0)),
                }
            }));

            let mut input_bytes = Cursor::new(first_asset_name.as_bytes());
            let mut req = assets.load_from::<TestAsset, _, _>(&"test_asset_file", input_bytes, false);
            match &*await_asset(req)
            {
                AssetPayload::Available(a) => assert_eq!(a.name, first_asset_name),
                _ => panic!("Asset not available"),
            }

            // check that it doesn't reload when update_if_exists is false
            input_bytes = Cursor::new(second_asset_name.as_bytes());
            req = assets.load_from(&"test_asset_file", input_bytes, false);
            match &*await_asset(req)
            {
                AssetPayload::Available(a) => assert_eq!(a.name, first_asset_name),
                _ => panic!("Asset not available"),
            }

            input_bytes = Cursor::new(second_asset_name.as_bytes());
            req = assets.load_from(&"test_asset_file", input_bytes, true);
            match &*await_asset(req)
            {
                AssetPayload::Available(a) => assert_eq!(a.name, second_asset_name),
                _ => panic!("Asset not available"),
            }
        }
    }

    // TODO: asset dependency lifetimes
    // TODO: generation
}
