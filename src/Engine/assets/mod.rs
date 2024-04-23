#![allow(private_bounds)] // todo: https://github.com/rust-lang/rust/issues/115475

use std::any::TypeId;
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::pin::Pin;
use std::sync::Arc;
use std::io::Read;
use std::task::{Context, Poll, Waker};
use arc_swap::{ArcSwap, DefaultStrategy, Guard};
use egui::Ui;
use unicase::UniCase;
use std::thread::Builder;
use crossbeam::channel::{unbounded, Sender};
use parking_lot::Mutex;
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

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd)]
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

pub trait Asset: Sync + Send
{
}

#[derive(Debug)]
pub enum AssetLoadError
{
    NotFound,

    #[cfg(test)]
    TestEmpty,
}

#[derive(Debug)]
pub enum AssetPayload<A: Asset>
{
    Pending,
    Available(A),
    Unavailable(AssetLoadError), // shouldn't be stored in-cache
}

// TODO: lifetime management?
pub struct AssetHandleInner<A: Asset>
{
    key: AssetKey,
    payload: ArcSwap<AssetPayload<A>>,
    ready_waker: Mutex<Option<Waker>>,

    // todo: drop sender channel
    // calls lifecycle destroy
}
impl<A: Asset> AssetHandleInner<A>
{
    fn new_handle(key: AssetKey) -> AssetHandle<A>
    {
        AssetHandle::new(Self
        {
            key,
            payload: ArcSwap::from_pointee(AssetPayload::Pending),
            ready_waker: Mutex::new(None),
        })
    }

    pub fn key(&self) -> AssetKey { self.key }
    pub fn payload(&self) -> Guard<Arc<AssetPayload<A>>, DefaultStrategy>
    {
        self.payload.load()
    }

    fn signal_waker(&self)
    {
        let mut locked = self.ready_waker.lock();
        if let Some(waker) = (&mut *locked).take()
        {
            waker.wake();
        }
    }
    // downgrade?
}
impl<A: Asset> std::future::Future for &AssetHandleInner<A> // non-reffing requires being able to consume an Arc
{
    type Output = Arc<AssetPayload<A>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>
    {
        match self.payload.load().as_ref()
        {
            AssetPayload::Pending =>
            {
                let mut locked = self.ready_waker.lock();
                std::mem::swap(&mut *locked, &mut Some(cx.waker().clone()));
                Poll::Pending
            },
            AssetPayload::Available(_) | AssetPayload::Unavailable(_) => Poll::Ready(self.payload.load_full()),
        }
    }
}


type AssetHandle<A> = Arc<AssetHandleInner<A>>;

pub struct AssetLoadRequest<A: Asset>
{
    pub key: AssetKey,
    pub input: Box<dyn Read>,

    output: AssetHandle<A>,
}
impl<A: Asset> AssetLoadRequest<A>
{
    pub fn finish(self, payload: A)
    {
        self.output.payload.store(Arc::new(AssetPayload::Available(payload)));
        self.output.signal_waker();
    }

    pub fn error(self, error: AssetLoadError)
    {
        self.output.payload.store(Arc::new(AssetPayload::Unavailable(error)));
        self.output.signal_waker();
    }
}

pub struct AssetLifecyclerStats
{
    pub active_count: usize,
}

pub trait AssetLifecycler<A: Asset>: Sync
{
    fn get_or_create(&self, request: AssetLoadRequest<A>); // fills in the output of request
    // reload ?

    // todo: controlled destruction
    fn stats(&self) -> AssetLifecyclerStats;
}

// macro_rules! define_asset_lifecyclers
// {
//     ( $( $name:ident )+ ) =>
//     {
//         impl<$($name: AssetLifecycler),+> AssetLifecyclerLookup for ($($name,)+)
//         {
//             fn do_something(self) -> Self {
//                 let ($($name,)+) = self;
//                 ($($name.do_something(),)+)
//             }
//         }
//     };
// }

pub trait AssetLifecyclerLookup<A: Asset>
{
    fn lifecycler(&self) -> &impl AssetLifecycler<A>;
}

pub trait AssetLifecyclers: Sync + Send
{
    // todo: generic lookup?
}

type AssetLoadJob = Box<dyn FnOnce() + Send>;

pub struct Assets<L: AssetLifecyclers>
{
    job_scheduler: Sender<AssetLoadJob>,
    handles: Mutex<HashMap<AssetKey, usize>>, // type erased AssetHandles

    pub lifecyclers: Arc<L>, // todo: this shouldn't need to be an arc

}
impl<L: AssetLifecyclers> Assets<L>
{
    pub fn new(asset_lifecyclers: L) -> Self
    {
        let lifecyclers = Arc::new(asset_lifecyclers);

        const NUM_ASSET_JOB_THREADS: usize = 1;
        let (send, recv) = unbounded::<AssetLoadJob>();
        let threads = alloc_slice_fn(NUM_ASSET_JOB_THREADS, |i|
        {
            let this_recv = recv.clone();
            Builder::new()
                .name(format!("Asset worker thread {0}", i))
                .spawn(move ||
                {
                    eprintln!("Starting asset worker {}", i);
                    'worker: loop
                    {
                        match this_recv.recv()
                        {
                            Ok(f) =>
                            {
                                // eprintln!("received asset load request on worker {}", i);
                                f()
                            },
                            Err(_) =>
                            {
                                eprintln!("Terminating asset worker {}", i);
                                break 'worker;
                            }
                        }
                    }
            }).unwrap()
        });

        Self
        {
            job_scheduler: send,
            lifecyclers,

            handles: Mutex::new(HashMap::new()),
        }
    }

    pub fn load<A: Asset + 'static, S: AssetPath>(
        &self,
        asset_path: UniCase<&S>,
    ) -> AssetHandle<A>
        where L: AssetLifecyclerLookup<A> + 'static
    {
        let key_desc = AssetKeyDesc
        {
            path: UniCase::unicode(asset_path.as_ref()),
            type_id: TypeId::of::<A>(),
        };
        let asset_key = (&key_desc).into();

        let mut handle_bank= self.handles.lock();
        let untyped_handle = handle_bank.entry(asset_key).or_insert_with(||
        {
            let handle = AssetHandleInner::<A>::new_handle(asset_key);

            // TODO: drop fn

            Arc::into_raw(handle) as usize
        });

        // todo: don't store ref internally? custom drop manages?
        let handle = unsafe { AssetHandle::from_raw(*untyped_handle as *const AssetHandleInner<A>) }.clone();

        // create and enqueue load job
        {
            let asset_key_copy = asset_key.clone();
            let asset_path_copy = UniCase::unicode(asset_path.to_string());
            let handle_copy = handle.clone();
            let lifecyclers_copy = self.lifecyclers.clone();
            let job = Box::new(move ||
            {
                Self::load_job_fn(asset_path_copy, asset_key_copy,handle_copy, lifecyclers_copy.lifecycler())
            });
            self.job_scheduler.send(job).unwrap(); // todo: error handling
        }
        handle
    }

    fn load_job_fn<A: Asset>(
        asset_path: UniCase<String>,
        asset_key: AssetKey,
        asset_handle: AssetHandle<A>,
        asset_lifecycler: &dyn AssetLifecycler<A>)
    {
        // todo: put this elsewhere
        fn open_asset_from_file<S: AssetPath>(asset_path: &UniCase<S>) -> Result<impl Read, std::io::Error>
        {
            let path = std::path::Path::new("assets").join(asset_path.as_ref());
            // todo: restrict to this subpath
            std::fs::File::open(path)
        }

        let input = open_asset_from_file(&asset_path);
        match input
        {
            Ok(reader) =>
            {
                let load = AssetLoadRequest
                {
                    key: asset_key,
                    input: Box::new(reader),
                    output: asset_handle,
                };
                asset_lifecycler.get_or_create(load);
            }
            Err(err) =>
            {
                println!("Error loading asset: {0}\n", err);
                asset_handle.payload.store(Arc::new(AssetPayload::Unavailable(AssetLoadError::NotFound)));
                asset_handle.signal_waker();
            }
        };
    }
}
impl<'a, L: AssetLifecyclers> DebugGui<'a> for Assets<L>
{
    fn name(&self) -> &'a str
    {
        "Assets"
    }
    fn debug_gui(&self, ui: &mut Ui)
    {
        let mut total_active_count = 0;
        egui::CollapsingHeader::new("TestAssets")
            .default_open(true)
            .show(
                ui,
                |cui|
                {
                    cui.label("TODO");

                    // fn display_asset_stats(stats: &AssetLifecyclerStats, ui: &mut Ui)
                    // {
                    //     ui.label(format!("Active: {}", stats.active_count));
                    // }

                    // let stats = self.lifecyclers.test_assets.stats();
                    // total_active_count += stats.active_count;
                    // display_asset_stats(&stats, cui);
                },
            );
        ui.separator();
        ui.label(format!("Total active: {0}", total_active_count));
    }
}
#[cfg(debug_assertions)]
impl<L: AssetLifecyclers> Drop for Assets<L>
{
    fn drop(&mut self)
    {
        let mut handle_bank = self.handles.lock();
        if !handle_bank.is_empty()
        {
            eprintln!("! Leak detected: {} active asset handles", handle_bank.len());

            // TODO: per-lifecycler printout
        }
        // if refcount 0 internally, this isn't necessary
        // for (_, untyped_handle) in handle_bank.iter()
        // {
        //     let handle = unsafe { Arc::from_raw(std::mem::transmute(untyped_handle)) };
        // }

        // let stats = self.test_asets.stats();
        // if stats.active_count != 0
        // {
        //
        //     {
        //     };
        // }
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
        name: &'static str,
    }
    impl Asset for TestAsset { }

    #[derive(Default)]
    pub struct TestAssetLifecycler
    {
        count: AtomicUsize,
        pub passthru: Mutex<Option<fn(AssetLoadRequest<TestAsset>)>>,
    }
    impl AssetLifecycler<TestAsset> for TestAssetLifecycler
    {
        fn get_or_create(&self, request: AssetLoadRequest<TestAsset>)
        {
            match *self.passthru.lock()
            {
                None => request.error(AssetLoadError::TestEmpty),
                Some(f) => f(request),
            }
        }

        fn stats(&self) -> AssetLifecyclerStats
        {
            AssetLifecyclerStats { active_count: self.count.load(Ordering::SeqCst), }
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
        let mut locked = assets.lifecyclers.test_assets.passthru.lock();
        *locked = passthru;
    }

    fn await_asset<A: Asset>(handle: AssetHandle<A>) -> Arc<AssetPayload<A>>
    {
        futures::executor::block_on(&*handle)
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
                (other) => panic!("Invalid load result: {other:#?}"),
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
                (other) => panic!("Asset not unavailable(TestEmpty): {other:#?}"),
            }
        }

        #[test]
        fn pending()
        {
            let assets = Assets::new(TestLifecyclers::default());

            let req = assets.load(UniCase::new(&"test_asset_file"));
            match req.payload.load().as_ref()
            {
                AssetPayload::Pending => {},
                _ => panic!("Asset not pending"),
            }

        }

        #[test]
        fn available()
        {
            let assets = Assets::new(TestLifecyclers::default());
            set_passthru(&assets, Some(|req: AssetLoadRequest<TestAsset>|
            {
                req.finish(TestAsset { name: "test asset" });
            }));

            let req = assets.load(UniCase::new(&"test_asset_file"));
            match &*await_asset(req)
            {
                AssetPayload::Available(a) => assert_eq!(a.name, "test asset"),
                _ => panic!("Asset not available"),
            }
        }
    }
}
