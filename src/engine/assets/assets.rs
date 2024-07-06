use std::any::{type_name, TypeId};
use std::collections::HashMap;
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::thread::{Builder, JoinHandle};
use arc_swap::ArcSwapOption;
use crossbeam::channel::{Sender, SendError, unbounded};
use egui::{Ui, Widget};
use parking_lot::Mutex;
use unicase::UniCase;
use crate::engine::AsIterator;
use crate::engine::graphics::debug_gui::DebugGui;

use super::*;

struct HandleEntry
{
    untyped_handle: UntypedHandleInner,
    asset_path: UniCase<String>, // don't expose in release?
    #[cfg(debug_assertions)] // don't expose the asset type in release?
    asset_type: &'static str,
    dependencies: Vec<AssetKey>,
}
type HandleBank = HashMap<AssetKey, HandleEntry>;

struct AssetsStorage<L: AssetLifecyclers>
{
    handles: Mutex<HandleBank>,
    lifecyclers: L,
    lifecycle_channel: Sender<AssetLifecycleJob>,
}
impl<L: AssetLifecyclers> AssetsStorage<L>
{
    #[must_use]
    fn create_or_update_handle<A: Asset, S: AssetPath>(&self, asset_path: &S) -> (bool /* pre-existing */, AssetHandle<A>)
    {
        let key_desc = AssetKeyDesc
        {
            path: UniCase::new(asset_path.as_ref()),
            type_id: TypeId::of::<()>(), // TypeId::of::<A>(),
        };
        let asset_key = (&key_desc).into();

        let mut handle_bank = self.handles.lock();

        let mut pre_existing = true;
        let handle_entry = handle_bank.entry(asset_key).or_insert_with(||
            {
                pre_existing = false;
                /*
                Memory for assets is managed inside the assets class.

                This prevents a race between the handle being dropped, deleting its memory,
                  and this function returning an existing (stale) handle.

                By having the pointer created and destroyed by this class,
                  there is serialization provided by the mutex preventing use-after-free issues.
                */

                let inner: AssetHandleInner<A> = AssetHandleInner
                {
                    ref_count: AtomicUsize::new(0), // this will be incremented below
                    key: asset_key,
                    payload: ArcSwapOption::new(None),
                    dropper: self.lifecycle_channel.clone(),
                    ready_waker: Mutex::new(None),
                };
                let ptr = Box::into_raw(Box::new(inner)) as UntypedHandleInner; // manage memory through Box

                HandleEntry
                {
                    untyped_handle: ptr,
                    asset_path: UniCase::new(asset_path.to_string()),
                    #[cfg(debug_assertions)]
                    asset_type:
                    {
                        let asset_type_name = std::any::type_name::<A>();
                        match asset_type_name.rfind(':')
                        {
                            None => asset_type_name,
                            Some(i) => &asset_type_name[(i + 1)..]
                        }
                    },
                    dependencies: Vec::new(),
                }
            });
        let handle = unsafe { AssetHandle::clone_from_untyped(handle_entry.untyped_handle) };

        (pre_existing, handle)
    }

    #[must_use]
    pub fn load<A: Asset, S: AssetPath>(self: &Arc<Self>, asset_path: &S) -> AssetHandle<A>
        where L: AssetLifecyclerLookup<A>
    {
        let (pre_existed, asset_handle) = self.create_or_update_handle(asset_path);
        if pre_existed
        {
            return asset_handle;
        }

        // create and enqueue load job
        {
            // pass-thru pre-existence?
            let asset_path_copy = UniCase::new(asset_path.to_string());
            let handle_copy = asset_handle.clone();
            let storage_copy = self.clone();
            let job = Box::new(move ||
            {
                //Self::load_job_fn(asset_path_copy, handle_copy, storage_copy)
            });

            if self.lifecycle_channel.send(AssetLifecycleJob::Load(job)).is_err()
            {
                asset_handle.inner().store_payload(AssetPayload::Unavailable(AssetLoadError::Shutdown));
            }
        }

        asset_handle
    }

    #[must_use]
    pub fn load_from<A: Asset, S: AssetPath, R: Read + Send>(
        self: &Arc<Self>,
        asset_path: &S,
        input_data: R, // take box?,
        update_if_exists: bool,
    ) -> AssetHandle<A>
        where L: AssetLifecyclerLookup<A>
    {
        let (pre_existed, asset_handle) = self.create_or_update_handle(asset_path);
        if pre_existed
        {
            match update_if_exists
            {
                true =>
                    {
                        eprintln!("Reloading asset <{}> '{}'", type_name::<A>(), asset_path); // return entry which has name?
                        // don't replace payload until the new one is available
                    },
                false => { return asset_handle; }
            }
        }

        // create and enqueue load job
        {
            // pass-thru pre-existence?

            // TODO: convert this to a struct w/ fn() ?
            let input_data_box = Box::new(input_data);
            let handle_copy = asset_handle.clone();
            let storage_copy = self.clone();
            let job = Box::new(move ||
            {
                //Self::load_from_job_fn(input_data_box, handle_copy, storage_copy)
            });

            if self.lifecycle_channel.send(AssetLifecycleJob::Load(job)).is_err()
            {
                asset_handle.inner().store_payload(AssetPayload::Unavailable(AssetLoadError::Shutdown));
            }
        }

        asset_handle
    }

    fn load_job_fn<A: Asset>(
        asset_path: UniCase<String>,
        asset_handle: AssetHandle<A>,
        assets_storage: Arc<AssetsStorage<L>>)
        where L: AssetLifecyclerLookup<A>
    {
        puffin::profile_function!();

        // todo: put this elsewhere
        fn open_asset_from_file<S: AssetPath>(asset_path: &UniCase<S>) -> Result<impl Read, std::io::Error>
        {
            let path = std::path::Path::new("assets").join(asset_path.as_ref());
            // todo: restrict to this subpath
            std::fs::File::open(path)
        }

        let input = open_asset_from_file(&asset_path);
        let inner = asset_handle.inner();
        match input
        {
            Ok(reader) =>
            {
                let load = AssetLoadRequest
                {
                    key: inner.key,
                    input: Box::new(reader),
                    output: asset_handle,
                    storage: assets_storage.clone(),
                    dependencies: Vec::new(),
                };
                assets_storage.lifecyclers.lifecycler().create_or_update(load);
            }
            Err(err) =>
            {
                println!("Error loading asset: {0}\n", err);
                inner.store_payload(AssetPayload::Unavailable(AssetLoadError::NotFound));
                inner.signal_waker();
            }
        };
    }

    fn load_from_job_fn<A: Asset>(
        input_data: Box<dyn Read>,
        asset_handle: AssetHandle<A>,
        assets_storage: Arc<AssetsStorage<L>>)
        where L: AssetLifecyclerLookup<A>
    {
        puffin::profile_function!();

        let load = AssetLoadRequest
        {
            key: asset_handle.key(),
            input: input_data,
            output: asset_handle,
            storage: assets_storage.clone(),
            dependencies: Vec::new(),
        };
        assets_storage.lifecyclers.lifecycler().create_or_update(load);
    }

    fn update_dependencies(&self, asset: AssetKey, dependencies: &mut Vec<AssetKey>)
    {
        let mut handle_bank = self.handles.lock();
        match handle_bank.get_mut(&asset)
        {
            None =>
            {
                eprintln!("Tried to update dependencies for an asset that doesn't exist");
            }
            Some(entry) =>
            {
                std::mem::swap(&mut entry.dependencies, dependencies); // safe?
            }
        }
    }
}

const NUM_ASSET_JOB_THREADS: usize = 1;
pub struct Assets<L: AssetLifecyclers>
{
    storage: Arc<AssetsStorage<L>>,
    worker_threads: [Option<JoinHandle<()>>; NUM_ASSET_JOB_THREADS],
}
impl<L: AssetLifecyclers> Assets<L>
{
    #[must_use]
    pub fn new(asset_lifecyclers: L) -> Self
    {
        let (send, recv) = unbounded::<AssetLifecycleJob>();
        let storage = Arc::new(AssetsStorage
        {
            handles: Mutex::new(HandleBank::new()),
            lifecyclers: asset_lifecyclers,
            lifecycle_channel: send,
        });
        //
        // let fs_watcher = notify_debouncer_mini::new_debouncer(
        //     Duration::from_secs(3),
        //     |evt|
        //     {
        //
        //     });

        // TODO: broadcast change notifications

        let worker_threads = array_init::array_init::<_, _, NUM_ASSET_JOB_THREADS>(|i|
        {
            let this_recv = recv.clone();
            let this_storage = storage.clone();
            let thread = Builder::new()
                .name(format!("Asset worker thread {}", i))
                .spawn(move ||
                {
                    eprintln!("Starting asset worker thread {}", i);
                    'worker: loop
                    {
                        match this_recv.recv()
                        {
                            Ok(job) =>
                            {
                                match job
                                {
                                    AssetLifecycleJob::StopWorkers => { break 'worker; },
                                    AssetLifecycleJob::Load(load) => { (load)(); },
                                    AssetLifecycleJob::Reload =>
                                    {
                                        // TODO
                                    },
                                    AssetLifecycleJob::Drop(dropping) =>
                                    {
                                        let mut handle_bank = this_storage.handles.lock(); // must lock before below to make sure that the handle doesn't get cloned between the drop and below

                                        // this uses some co-operative multiplayer w/ AssetHandleInner to check and/or destroy the handle inner
                                        // could cast to some null type asset handle, but that is very not safe
                                        match (dropping.drop_fn)(dropping.handle)
                                        {
                                            None => { }
                                            Some(key) =>
                                            {
                                                match handle_bank.remove(&key)
                                                {
                                                    None =>
                                                    {
                                                        eprintln!("An asset was dropped, but it didn't exist in the Assets handle bank!")
                                                    },
                                                    Some(entry) =>
                                                    {
                                                        // handle no longer valid but can be latent verified
                                                        debug_assert_eq!(dropping.handle, entry.untyped_handle);
                                                        eprintln!("Unloaded asset <{}> '{}'", entry.asset_type, entry.asset_path);
                                                    }
                                                }
                                            }
                                        }
                                    },
                                }
                            },
                            Err(err) =>
                            {
                                eprintln!("Terminating asset worker thread {} due to {err}", i);
                                break 'worker;
                            }
                        }
                    }
                }).expect("Failed to create asset worker thread");
            Some(thread)
        });

        Self
        {
            storage,
            worker_threads,
        }
    }

    #[inline]
    #[must_use]
    pub fn load<A: Asset, S: AssetPath>(&self, asset_path: &S) -> AssetHandle<A>
        where L: AssetLifecyclerLookup<A>
    {
        self.storage.load(asset_path)
    }

    #[inline]
    #[must_use]
    pub fn load_from<A: Asset, S: AssetPath, R: Read + Send>(
        &self,
        asset_path: &S,
        input_data: R, // take box?,
        update_if_exists: bool,
    ) -> AssetHandle<A>
        where L: AssetLifecyclerLookup<A>
    {
        self.storage.load_from(asset_path, input_data, update_if_exists)
    }

    pub fn lifecyclers(&self) -> &L
    {
        &self.storage.lifecyclers
    }

    // todo: never unload assets?

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
                let inner = unsafe { &*(entry.untyped_handle as *const AssetHandleInner::<NullAsset>) };
                // not safe to call virtual methods with a punned handle
                //std::mem::discriminant(&*inner.payload()) == std::mem::discriminant(&AssetPayload::Available(NullAsset))
                inner.payload().is_available()
            }
        }
    }

    // prevent any new assets from being loaded
    pub fn shutdown(&self)
    {
        let _ = self.storage.lifecycle_channel.send(AssetLifecycleJob::StopWorkers); // will error if already closed
    }
}
impl<'i, 'a: 'i, L: AssetLifecyclers> DebugGui<'a> for Assets<L>
    where L: AsIterator<'i, Item=&'a dyn DebugGui<'a>>
{
    fn name(&self) -> &'a str
    {
        "Assets"
    }
    fn debug_gui(&'a self, ui: &mut Ui)
    {
        for l in self.storage.lifecyclers.as_iter()
        {
            egui::CollapsingHeader::new(l.name())
                .default_open(true)
                .show(ui, |cui|
                    {
                        l.debug_gui(cui);
                    });
        }

        ui.separator();

        let handle_bank = self.storage.handles.lock();

        let total_active_count = handle_bank.len();
        ui.label(format!("Total active handles: {0}", total_active_count));

        ui.collapsing("Handles", |cui|
            {
                egui::Grid::new("Handles table")
                    .striped(true)
                    .num_columns(5)
                    .show(cui, |gui|
                        {
                            gui.heading("Key");
                            gui.heading("Refs");
                            gui.heading("Dependencies");
                            gui.heading("Type");
                            gui.heading("Path");
                            gui.end_row();

                            for (key, entry) in handle_bank.iter()
                            {
                                // as long as handle_bank is locked, this handle will always be valid
                                let handle_unsafe = unsafe { &*(entry.untyped_handle as *const AssetHandleInner::<NullAsset>) };

                                gui.label(format!("{key:#?}")); // right click to copy?
                                gui.label(format!("{}", handle_unsafe.ref_count()));

                                let depsResponse = egui::Label::new(format!("{}", entry.dependencies.len()))
                                    .sense(egui::Sense::hover())
                                    .ui(gui);
                                if entry.dependencies.len() > 0 &&
                                    depsResponse.hovered()
                                {
                                    egui::show_tooltip(&depsResponse.ctx, egui::Id::new("AssetHandleDepsPopup"), |tui|
                                    {
                                        for dep in entry.dependencies.iter()
                                        {
                                            tui.label(format!("{:#?}", dep));
                                        }
                                    });
                                }

                                gui.label(entry.asset_type);
                                gui.label(entry.asset_path.as_ref());
                                gui.end_row();
                            }
                        });
            });
    }
}
#[cfg(debug_assertions)]
impl<L: AssetLifecyclers> Drop for Assets<L>
{
    fn drop(&mut self)
    {
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
                eprintln!("    {:?} <{}> '{}'", handle.0, handle.1.asset_type, handle.1.asset_path);
            }
            #[cfg(test)]
            panic!("Leaked assets!");
        }
        handle_bank.clear()
    }
}

pub(super) struct AssetDropJob
{
    pub handle: UntypedHandleInner,
    pub drop_fn: fn(UntypedHandleInner) -> Option<AssetKey>
}

pub(super) type AssetLoadJob = Box<dyn FnOnce() + Send>;

pub(super) enum AssetLifecycleJob
{
    StopWorkers, // signal to the worker threads to stop work

    Load(AssetLoadJob),
    Reload, // TODO
    Drop(AssetDropJob),
}

pub struct AssetLoadRequest<A: Asset, L: AssetLifecyclers>
{
    pub key: AssetKey,
    pub input: Box<dyn Read>,

    // timer?
    // is_reloading?

    output: AssetHandle<A>,
    storage: Arc<AssetsStorage<L>>,
    dependencies: Vec<AssetKey>,
}
impl<A: Asset, L: AssetLifecyclers> AssetLoadRequest<A, L>
{
    pub fn finish(mut self, payload: A)
    {
        let handle = self.output.inner();
        self.storage.update_dependencies(handle.key, &mut self.dependencies);
        handle.store_payload(AssetPayload::Available(payload));
        handle.signal_waker();
    }

    pub fn error(self, error: AssetLoadError)
    {
        let handle = self.output.inner();
        handle.store_payload(AssetPayload::Unavailable(error));
        handle.signal_waker();
    }

    pub fn load_dependency<D: Asset, S: AssetPath>(&mut self, asset_path: &S) -> AssetHandle<D>
        where L: AssetLifecyclerLookup<D>
    {
        let load = self.storage.load(asset_path);
        self.dependencies.push(load.key());
        load
    }
}

// An internal only asset that can be used for type erasure
// This should not be used publicly, and dynamic dispatch cannot be used
struct NullAsset;
impl Asset for NullAsset { }



#[cfg(test)]
mod tests
{
    use std::sync::atomic::AtomicUsize;
    use super::*;

    #[derive(Debug)]
    pub struct TestAsset
    {
        name: String,
    }
    impl Asset for TestAsset { }

    struct Passthru
    {
        call_count: usize,
        passthru_fn: fn(AssetLoadRequest<TestAsset, TestLifecyclers>),
    }

    #[derive(Default)]
    pub struct TestAssetLifecycler
    {
        active_count: AtomicUsize,
        pub passthru: Mutex<Option<Passthru>>,
    }
    impl<'a> AssetLifecycler<TestAsset, TestLifecyclers> for TestAssetLifecycler
    {
        fn create_or_update(&self, request: AssetLoadRequest<TestAsset, TestLifecyclers>)
        {
            match &mut *self.passthru.lock()
            {
                None => request.error(AssetLoadError::TestEmpty),
                Some(passthru) =>
                    {
                        passthru.call_count += 1;
                        (passthru.passthru_fn)(request)
                    },
            }
        }
    }

    #[derive(Default)]
    struct TestLifecyclers
    {
        pub test_assets: TestAssetLifecycler,
    }
    impl AssetLifecyclers for TestLifecyclers { }
    impl AssetLifecyclerLookup<TestAsset> for TestLifecyclers
    {
        fn lifecycler(&self) -> &impl AssetLifecycler<TestAsset, Self>
        {
            &self.test_assets
        }
    }

    fn set_passthru(assets: &Assets<TestLifecyclers>, passthru: Option<fn(AssetLoadRequest<TestAsset, TestLifecyclers>)>)
    {
        let mut locked = &mut *assets.lifecyclers().test_assets.passthru.lock();
        *locked = passthru.map(|f| Passthru { call_count: 0, passthru_fn: f });
    }
    fn get_passthru_call_count(assets: &Assets<TestLifecyclers>) -> Option<usize>
    {
        let locked = &*assets.lifecyclers().test_assets.passthru.lock();
        locked.as_ref().map(|p| p.call_count)
    }

    fn await_asset<A: Asset>(handle: AssetHandle<A>) -> Arc<AssetPayload<A>>
    {
        futures::executor::block_on(&handle)
    }

    // define_assets![self::TestAsset];

    mod load
    {
        use super::*;

        #[test]
        fn bad_file()
        {
            let assets = Assets::new(TestLifecyclers::default());

            let req: AssetHandle<TestAsset> = assets.load(&"$BAD_FILE$");
            match &*await_asset(req)
            {
                AssetPayload::Unavailable(AssetLoadError::NotFound) => {},
                other => panic!("Invalid load result: {other:#?}"),
            }
        }

        #[test]
        fn unavailable()
        {
            let assets = Assets::new(TestLifecyclers::default());
            set_passthru(&assets, Some(|req: AssetLoadRequest<TestAsset, TestLifecyclers>|
                {
                    req.error(AssetLoadError::TestEmpty);
                }));

            let req: AssetHandle<TestAsset> = assets.load(&"test_asset_file");
            match &*await_asset(req)
            {
                AssetPayload::Unavailable(AssetLoadError::TestEmpty) => {},
                other => panic!("Asset not unavailable(TestEmpty): {other:#?}"),
            }
        }

        #[test]
        fn pending()
        {
            let assets = Assets::new(TestLifecyclers::default());

            let req = assets.load(&"test_asset_file");
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
            let assets = Assets::new(TestLifecyclers::default());
            set_passthru(&assets, Some(|_req: AssetLoadRequest<TestAsset, TestLifecyclers>| { }));

            assert_eq!(Some(0), get_passthru_call_count(&assets));

            let _req1 = assets.load(&"test_asset_file");
            std::thread::sleep(std::time::Duration::from_secs(1)); // crude
            assert_eq!(Some(1), get_passthru_call_count(&assets));

            let _req2 = assets.load(&"test_asset_file");
            assert_eq!(Some(1), get_passthru_call_count(&assets));

            let _req3 = assets.load(&"test_asset_file");
            assert_eq!(Some(1), get_passthru_call_count(&assets));
        }

        #[test]
        fn available()
        {
            let assets = Assets::new(TestLifecyclers::default());
            set_passthru(&assets, Some(|req: AssetLoadRequest<TestAsset, TestLifecyclers>|
                {
                    req.finish(TestAsset { name: "test asset".to_string() });
                }));

            let req = assets.load(&"test_asset_file");
            match &*await_asset(req)
            {
                AssetPayload::Available(a) => assert_eq!(a.name, "test asset"),
                _ => panic!("Asset not available"),
            }
        }

        #[test]
        fn load_from()
        {
            let loaded_asset_name = "loaded asset name";

            let assets = Assets::new(TestLifecyclers::default());
            set_passthru(&assets, Some(|mut req: AssetLoadRequest<TestAsset, TestLifecyclers>|
                {
                    let mut name = String::new();
                    match req.input.read_to_string(&mut name)
                    {
                        Ok(_) => req.finish(TestAsset { name }),
                        Err(_) => req.error(AssetLoadError::ParseError),
                    }
                }));

            let input_bytes = loaded_asset_name.as_bytes();
            let req = assets.load_from(&"test_asset_file", input_bytes, false);
            match &*await_asset(req)
            {
                AssetPayload::Available(a) => assert_eq!(a.name, loaded_asset_name),
                _ => panic!("Asset not available"),
            }
        }

        #[test]
        fn reload()
        {
            let first_asset_name = "first";
            let second_asset_name = "second";

            let assets = Assets::new(TestLifecyclers::default());
            set_passthru(&assets, Some(|mut req: AssetLoadRequest<TestAsset, TestLifecyclers>|
                {
                    let mut name = String::new();
                    match req.input.read_to_string(&mut name)
                    {
                        Ok(_) => req.finish(TestAsset { name }),
                        Err(_) => req.error(AssetLoadError::ParseError),
                    }
                }));

            let mut input_bytes = first_asset_name.as_bytes();
            let mut req = assets.load_from(&"test_asset_file", input_bytes, false);
            match &*await_asset(req)
            {
                AssetPayload::Available(a) => assert_eq!(a.name, first_asset_name),
                _ => panic!("Asset not available"),
            }

            // check that it doesn't reload when update_if_exists is false
            input_bytes = second_asset_name.as_bytes();
            req = assets.load_from(&"test_asset_file", input_bytes, false);
            match &*await_asset(req)
            {
                AssetPayload::Available(a) => assert_eq!(a.name, first_asset_name),
                _ => panic!("Asset not available"),
            }

            req = assets.load_from(&"test_asset_file", input_bytes, true);
            match &*await_asset(req)
            {
                AssetPayload::Available(a) => assert_eq!(a.name, second_asset_name),
                _ => panic!("Asset not available"),
            }
        }
    }

    // todo: hot-reloading
}
