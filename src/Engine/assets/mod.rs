#![allow(private_bounds)] // todo: https://github.com/rust-lang/rust/issues/115475

use std::any::{type_name, TypeId};
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::pin::Pin;
use std::sync::Arc;
use std::io::Read;
use std::marker::PhantomData;
use std::mem::{size_of, swap};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::{Context, Poll, Waker};
use arc_swap::{ArcSwapOption};
use egui::Ui;
use unicase::UniCase;
use std::thread::{Builder, JoinHandle};
use std::time::Duration;
use arc_swap::access::Access;
use crossbeam::channel::{unbounded, Sender, Receiver};
use crossbeam::channel::internal::SelectHandle;
use egui_extras::Column;
use parking_lot::{Mutex, MutexGuard};
use crate::engine::AsIterator;
use crate::engine::graphics::debug_gui::DebugGui;
use crate::engine::utils::alloc_slice::alloc_slice_fn;

pub mod texture;
pub mod material;

pub trait AssetPath: AsRef<str> + Hash + Display + Debug { }
impl<T> AssetPath for T where T: AsRef<str> + Hash + Display + Debug { }

#[derive(Debug, Hash, Clone)]
pub struct AssetKeyDesc<S: AssetPath>
{
    pub path: UniCase<S>,
    pub type_id: TypeId,
}

#[derive(Hash, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub struct AssetKey(u64);
impl<S: AssetPath> From<&AssetKeyDesc<S>> for AssetKey
{
    fn from(desc: &AssetKeyDesc<S>) -> Self
    {
        let mut hasher = DefaultHasher::default();
        desc.hash(&mut hasher);
        Self(hasher.finish())
    }
}
impl<S: AssetPath> From<AssetKeyDesc<S>> for AssetKey
{
    fn from(desc: AssetKeyDesc<S>) -> Self { Self::from(&desc) }
}
impl Debug for AssetKey
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        let sizeof = size_of::<Self>();
        match f.alternate()
        {
            true => f.write_fmt(format_args!("0x{:0width$x}", self.0, width = sizeof)),
            false => f.write_fmt(format_args!("⨇⟨{:0width$x}⟩", self.0, width = sizeof)),
        }
    }
}

pub trait Asset: Sync + Send
{
}

#[derive(Debug)]
pub enum AssetLoadError
{
    NotFound = 100,

    InvalidFormat = 200,

    ParseError = 300,

    #[cfg(test)]
    TestEmpty = 100000000,
}

#[derive(Debug)]
pub enum AssetPayload<A: Asset>
{
    Pending, // This state is must only be set while there is an active AssetLoadRequest being queued/processed. Store sentinel of the asset load request?
    Available(A),
    Unavailable(AssetLoadError), // shouldn't be stored in-cache? (reload on error?)
}

pub struct AssetHandleInner<A: Asset>
{
    ref_count: AtomicUsize,

    key: AssetKey,
    payload: ArcSwapOption<AssetPayload<A>>, // will be None while pending
    dropper: Sender<AssetLifecycleJob>,

    // necessary?
    ready_waker: Mutex<Option<Waker>>,
}
impl<A: Asset> AssetHandleInner<A>
{
    fn signal_waker(&self)
    {
        let mut locked = self.ready_waker.lock();
        if let Some(waker) = locked.take()
        {
            waker.wake();
        }
    }
}
pub struct AssetHandle<A: Asset>
{
    inner: *const AssetHandleInner<A>,
    phantom: PhantomData<AssetHandleInner<A>>,
}

type _PayloadProjectionFn<A> = fn(&Option<Arc<AssetPayload<A>>>) -> &AssetPayload<A>;
pub type PayloadGuard<A> = arc_swap::access::MapGuard<
    arc_swap::Guard<Option<Arc<AssetPayload<A>>>, arc_swap::DefaultStrategy>,
    _PayloadProjectionFn<A>,
    Option<Arc<AssetPayload<A>>>,
    AssetPayload<A>>; // holy shit this is ugly; this *should* be safe;

impl<A: Asset> AssetHandle<A>
{
    // creation managed by <Assets>

    #[inline]
    pub fn key(&self) -> AssetKey
    {
        self.inner().key
    }

    fn map_payload(payload: &Option<Arc<AssetPayload<A>>>) -> &AssetPayload<A>
    {
        match payload
        {
            None => &AssetPayload::Pending,
            Some(s) => s.as_ref()
        }
    }

    // Retrieve the current payload, holds a strong reference for as long as the guard exists
    #[inline]
    pub fn payload(&self) -> PayloadGuard<A>
    {
        self.inner().payload.map(Self::map_payload as _PayloadProjectionFn<A>).load()
    }

    #[inline]
    pub fn ref_count(&self) -> usize
    {
        self.inner().ref_count.load(Ordering::Acquire)
    }

    // Create a handle from an untyped inner handle, note: does not increment ref-count
    #[inline]
    fn attach_from_untyped(untyped: UntypedHandleInner) -> Self
    {
        Self
        {
            inner: untyped as *const AssetHandleInner<A>,
            phantom: Default::default(),
        }
    }

    #[inline]
    fn add_ref(&self) // add a ref (will cause a leak if misused)
    {
        // see Arc::clone() for details on ordering requirements
        let _old_refs = self.inner().ref_count.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    fn inner(&self) -> &AssetHandleInner<A>
    {
        unsafe { &*self.inner }
    }

    fn maybe_drop(untyped: UntypedHandleInner) -> Option<AssetKey>
    {
        let retyped = unsafe { &*(untyped as *const AssetHandleInner<A>) };
        match retyped.ref_count.load(Ordering::Acquire)
        {
            0 =>
            {
                let rv = Some(retyped.key); // retyped cannot be used after this
                let _ = unsafe { Box::from_raw(untyped as *mut AssetHandleInner<A>) }; // will drop at the end of this function
                rv
            },
            _ => None,
        }
    }
}
unsafe impl<A: Asset> Send for AssetHandle<A> {}
unsafe impl<A: Asset> Sync for AssetHandle<A> {}
impl<A: Asset> Clone for AssetHandle<A>
{
    fn clone(&self) -> Self
    {
        self.add_ref();
        Self
        {
            inner: self.inner.clone(),
            phantom: self.phantom.clone(),
        }
    }
}
impl<A: Asset> Drop for AssetHandle<A>
{
    fn drop(&mut self)
    {
        // see Arc::drop() for details on ordering requirements
        let inner = unsafe { &*self.inner };
        if inner.ref_count.fetch_sub(1, Ordering::Release) != 1
        {
            return;
        }

        let inner_untyped =
        {
            let mut fill = std::ptr::null(); // reset to null for help w/ detecting bugs
            swap(&mut self.inner, &mut fill);
            fill
        };

        inner.dropper.send(AssetLifecycleJob::Drop(AssetDropJob { handle: (inner_untyped as UntypedHandleInner), drop_fn: Self::maybe_drop })).unwrap(); // todo: error handling
    }
}
impl<A: Asset> Future for &AssetHandle<A> // non-reffing requires being able to consume an Arc
{
    type Output = Arc<AssetPayload<A>>;

    // TODO: re-evaluate
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>
    {
        let inner = self.inner();
        match *self.payload()
        {
            AssetPayload::Pending =>
            {
                let mut locked = inner.ready_waker.lock();
                swap(&mut *locked, &mut Some(cx.waker().clone()));
                Poll::Pending
            },
            AssetPayload::Available(_) | AssetPayload::Unavailable(_) => Poll::Ready(inner.payload.load_full().unwrap()),
        }
    }
}

pub struct AssetLoadRequest<A: Asset>
{
    pub key: AssetKey,
    pub input: Box<dyn Read>,

    // timer?
    // is_reloading?

    output: AssetHandle<A>,
}
impl<A: Asset> AssetLoadRequest<A>
{
    pub fn finish(self, payload: A)
    {
        let handle = self.output.inner();
        handle.payload.store(Some(Arc::new(AssetPayload::Available(payload))));
        handle.signal_waker();
    }

    pub fn error(self, error: AssetLoadError)
    {
        let handle = self.output.inner();
        handle.payload.store(Some(Arc::new(AssetPayload::Unavailable(error))));
        handle.signal_waker();
    }
}

pub trait AssetLifecycler<A: Asset>: Sync
{
    /// Get or create an asset payload for the requested asset
    /// Note: the Asset system will track lifetimes itself, so lifecyclers are not required to maintain their own internal storage
    fn create_or_update(&self, request: AssetLoadRequest<A>); // fills in the output of request
    // reload ?
}

pub trait AssetLifecyclerLookup<A: Asset>
{
    fn lifecycler(&self) -> & impl AssetLifecycler<A>;
}

pub trait AssetLifecyclers: Sync + Send
{
}

struct AssetDropJob
{
    handle: UntypedHandleInner,
    drop_fn: fn(UntypedHandleInner) -> Option<AssetKey>
}

type AssetLoadJob = Box<dyn FnOnce() + Send>;

enum AssetLifecycleJob
{
    StopWorkers, // signal to the worker threads to stop work

    Load(AssetLoadJob),
    Reload, // TODO
    Drop(AssetDropJob),
}

type UntypedHandleInner = usize; // For internal use
struct HandleEntry
{
    untyped_handle: UntypedHandleInner,
    asset_path: UniCase<String>, // don't expose in release?
    #[cfg(debug_assertions)] // don't expose the asset type in release?
    asset_type: &'static str,
}
type HandleBank = HashMap<AssetKey, HandleEntry>;

struct AssetsStorage<L: AssetLifecyclers>
{
    handles: Mutex<HandleBank>,
    lifecyclers: L,
}

const NUM_ASSET_JOB_THREADS: usize = 1;
pub struct Assets<L: AssetLifecyclers>
{
    storage: Arc<AssetsStorage<L>>,

    worker_threads: [Option<JoinHandle<()>>; NUM_ASSET_JOB_THREADS],
    lifecycle_channel: Sender<AssetLifecycleJob>,
}
impl<L: AssetLifecyclers> Assets<L>
{
    #[must_use]
    pub fn new(asset_lifecyclers: L) -> Self
        where L: 'static
    {
        let storage = Arc::new(AssetsStorage
        {
            handles: Mutex::new(HandleBank::new()),
            lifecyclers: asset_lifecyclers,
        });
        //
        // let fs_watcher = notify_debouncer_mini::new_debouncer(
        //     Duration::from_secs(3),
        //     |evt|
        //     {
        //
        //     });

        let (send, recv) = unbounded::<AssetLifecycleJob>();
        let worker_threads = array_init::array_init::<_, _, NUM_ASSET_JOB_THREADS>(|i|
        {
            let this_recv = recv.clone();
            let this_storage = storage.clone();
            let thread = Builder::new()
                .name(format!("Asset worker thread {0}", i))
                .spawn(move ||
                {
                    eprintln!("Starting asset worker {}", i);
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
                                    AssetLifecycleJob::Reload => {}, // TODO
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
                                                        eprintln!("Unloaded asset {} '{}'", entry.asset_type, entry.asset_path);
                                                    }
                                                }
                                            }
                                        }
                                    },
                                }
                            },
                            Err(err) =>
                            {
                                eprintln!("Terminating asset worker {} due to {err}", i);
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
            lifecycle_channel: send,
        }
    }

    pub fn lifecycler<A: Asset + 'static>(&self) -> &(impl AssetLifecycler<A> + '_)
        where L: AssetLifecyclerLookup<A>
    {
        self.storage.lifecyclers.lifecycler()
    }

    pub fn lifecyclers(&self) -> &L
    {
        &self.storage.lifecyclers
    }

    // todo: never unload assets?

    #[must_use]
    fn create_or_update_handle<A: Asset + 'static, S: AssetPath>(&self, asset_path: UniCase<&S>) -> (bool /* pre-existing */, AssetHandle<A>)
    {
        let key_desc = AssetKeyDesc
        {
            path: UniCase::unicode(asset_path.as_ref()),
            type_id: TypeId::of::<A>(),
        };
        let asset_key = (&key_desc).into();

        let mut handle_bank = self.storage.handles.lock();

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
                asset_path: UniCase::unicode(asset_path.to_string()),
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
            }
        });
        let handle = AssetHandle::attach_from_untyped(handle_entry.untyped_handle);
        handle.add_ref();

        (pre_existing, handle)
    }

    #[must_use]
    pub fn load<A: Asset + 'static, S: AssetPath>(&self, asset_path: UniCase<&S>) -> AssetHandle<A>
        where L: AssetLifecyclerLookup<A> + 'static
    {
        let (pre_existed, asset_handle) = self.create_or_update_handle(asset_path);
        if pre_existed
        {
            return asset_handle;
        }

        // create and enqueue load job
        {
            // pass-thru pre-existence?
            let storage_copy = self.storage.clone();
            let asset_path_copy = UniCase::unicode(asset_path.to_string());
            let handle_copy = asset_handle.clone();
            let storage_copy = self.storage.clone();
            let job = Box::new(move ||
            {
                Self::load_job_fn(asset_path_copy, handle_copy, storage_copy.lifecyclers.lifecycler())
            });
            self.lifecycle_channel.send(AssetLifecycleJob::Load(job)).unwrap(); // todo: error handling
        }

        asset_handle
    }

    #[must_use]
    pub fn load_from<A: Asset + 'static, S: AssetPath, R: Read + Send + 'static>(
        &self,
        asset_path: UniCase<&S>,
        input_data: R, // take box?,
        update_if_exists: bool,
    ) -> AssetHandle<A>
        where L: AssetLifecyclerLookup<A> + 'static
    {
        let (pre_existed, asset_handle) = self.create_or_update_handle(asset_path);
        if pre_existed
        {
            match update_if_exists
            {
                true =>
                {
                    eprintln!("Reloading asset {} '{}'", type_name::<A>(), asset_path); // return entry which has name?
                    asset_handle.inner().payload.store(None);
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
            let storage_copy = self.storage.clone();
            let job = Box::new(move ||
            {
                Self::load_from_job_fn(input_data_box, handle_copy, storage_copy.lifecyclers.lifecycler())
            });
            self.lifecycle_channel.send(AssetLifecycleJob::Load(job)).unwrap(); // todo: error handling
        }

        asset_handle
    }

    pub fn num_active_assets(&self) -> usize
    {
        let handles = self.storage.handles.lock();
        handles.len()
    }

    fn load_job_fn<A: Asset>(
        asset_path: UniCase<String>,
        asset_handle: AssetHandle<A>,
        asset_lifecycler: &dyn AssetLifecycler<A>)
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
                };
                asset_lifecycler.create_or_update(load);
            }
            Err(err) =>
            {
                println!("Error loading asset: {0}\n", err);
                inner.payload.store(Some(Arc::new(AssetPayload::Unavailable(AssetLoadError::NotFound))));
                inner.signal_waker();
            }
        };
    }

    fn load_from_job_fn<A: Asset>(
        input_data: Box<dyn Read>,
        asset_handle: AssetHandle<A>,
        asset_lifecycler: &dyn AssetLifecycler<A>)
    {
        puffin::profile_function!();
        let load = AssetLoadRequest
        {
            key: asset_handle.key(),
            input: input_data,
            output: asset_handle,
        };
        asset_lifecycler.create_or_update(load);
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
                .num_columns(3)
                .show(cui, |gui|
                {
                    gui.heading("Asset Key");
                    gui.heading("Asset Type");
                    gui.heading("Asset Path");
                    gui.end_row();

                    for (key, handle) in handle_bank.iter()
                    {
                        gui.label(format!("{key:#?}")); // right click to copy?
                        gui.label(handle.asset_type);
                        gui.label(handle.asset_path.as_ref());
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
        self.lifecycle_channel.send(AssetLifecycleJob::StopWorkers).unwrap();
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
                eprintln!("    {:?} {} '{}'", handle.0, handle.1.asset_type, handle.1.asset_path);
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use super::*;
    use unicase::UniCase;

    #[derive(Debug)]
    pub struct TestAsset
    {
        name: String,
    }
    impl Asset for TestAsset { }

    struct Passthru
    {
        call_count: usize,
        passthru_fn: fn(AssetLoadRequest<TestAsset>),
    }

    #[derive(Default)]
    pub struct TestAssetLifecycler
    {
        active_count: AtomicUsize,
        pub passthru: Mutex<Option<Passthru>>,
    }
    impl<'a> AssetLifecycler<TestAsset> for TestAssetLifecycler
    {
        fn create_or_update(&self, request: AssetLoadRequest<TestAsset>)
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
        fn lifecycler(&self) -> &impl AssetLifecycler<TestAsset>
        {
            &self.test_assets
        }
    }

    fn set_passthru(assets: &Assets<TestLifecyclers>, passthru: Option<fn(AssetLoadRequest<TestAsset>)>)
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

            let req: AssetHandle<TestAsset> = assets.load(UniCase::new(&"$BAD_FILE$"));
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
            set_passthru(&assets, Some(|req: AssetLoadRequest<TestAsset>|
            {
                req.error(AssetLoadError::TestEmpty);
            }));

            let req: AssetHandle<TestAsset> = assets.load(UniCase::new(&"test_asset_file"));
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

            let req = assets.load(UniCase::new(&"test_asset_file"));
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
            set_passthru(&assets, Some(|_req: AssetLoadRequest<TestAsset>| { }));

            assert_eq!(Some(0), get_passthru_call_count(&assets));

            let _req1 = assets.load(UniCase::new(&"test_asset_file"));
            std::thread::sleep(std::time::Duration::from_secs(1)); // crude
            assert_eq!(Some(1), get_passthru_call_count(&assets));

            let _req2 = assets.load(UniCase::new(&"test_asset_file"));
            assert_eq!(Some(1), get_passthru_call_count(&assets));

            let _req3 = assets.load(UniCase::new(&"test_asset_file"));
            assert_eq!(Some(1), get_passthru_call_count(&assets));
        }

        #[test]
        fn available()
        {
            let assets = Assets::new(TestLifecyclers::default());
            set_passthru(&assets, Some(|req: AssetLoadRequest<TestAsset>|
            {
                req.finish(TestAsset { name: "test asset".to_string() });
            }));

            let req = assets.load(UniCase::new(&"test_asset_file"));
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
            set_passthru(&assets, Some(|mut req: AssetLoadRequest<TestAsset>|
            {
                let mut name = String::new();
                match req.input.read_to_string(&mut name)
                {
                    Ok(_) => req.finish(TestAsset { name }),
                    Err(_) => req.error(AssetLoadError::ParseError),
                }
            }));

            let input_bytes = loaded_asset_name.as_bytes();
            let req = assets.load_from(UniCase::new(&"test_asset_file"), input_bytes, false);
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
            set_passthru(&assets, Some(|mut req: AssetLoadRequest<TestAsset>|
                {
                    let mut name = String::new();
                    match req.input.read_to_string(&mut name)
                    {
                        Ok(_) => req.finish(TestAsset { name }),
                        Err(_) => req.error(AssetLoadError::ParseError),
                    }
                }));

            let mut input_bytes = first_asset_name.as_bytes();
            let mut req = assets.load_from(UniCase::new(&"test_asset_file"), input_bytes, false);
            match &*await_asset(req)
            {
                AssetPayload::Available(a) => assert_eq!(a.name, first_asset_name),
                _ => panic!("Asset not available"),
            }

            // check that it doesn't reload when update_if_exists is false
            input_bytes = second_asset_name.as_bytes();
            req = assets.load_from(UniCase::new(&"test_asset_file"), input_bytes, false);
            match &*await_asset(req)
            {
                AssetPayload::Available(a) => assert_eq!(a.name, first_asset_name),
                _ => panic!("Asset not available"),
            }

            req = assets.load_from(UniCase::new(&"test_asset_file"), input_bytes, true);
            match &*await_asset(req)
            {
                AssetPayload::Available(a) => assert_eq!(a.name, second_asset_name),
                _ => panic!("Asset not available"),
            }
        }
    }

    // todo: hot-reloading
}
