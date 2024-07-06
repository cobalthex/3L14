use std::any::TypeId;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::thread::{Builder, JoinHandle};
use arc_swap::ArcSwapOption;
use crossbeam::channel::{Sender, unbounded};
use nameof::name_of_type;
use parking_lot::Mutex;

mod asset;
pub use asset::*;

pub struct AssetLoadRequest<A: Asset>
{
    pub key: AssetId,
    pub input: Box<dyn Read>,

    // timer?
    // is_reloading?

    output: AssetHandle<A>,
    dependencies: Vec<AssetId>,
}
impl<A: Asset> AssetLoadRequest<A>
{
    pub fn finish(mut self, payload: A)
    {
        let handle = self.output.inner();
        // self.storage.update_dependencies(handle.key, &mut self.dependencies);
        handle.store_payload(AssetPayload::Available(payload));
        handle.signal_waker();
    }

    pub fn error(self, error: AssetLoadError)
    {
        let handle = self.output.inner();
        handle.store_payload(AssetPayload::Unavailable(error));
        handle.signal_waker();
    }

    // load_dependency
}

pub trait AssetLifecycler<A: Asset>
{
    fn create_or_update(&self, request: AssetLoadRequest<A>);
}

pub(super) struct AssetLoadJob
{
    type_id: TypeId,
    asset_id: AssetId,
}

pub(super) struct AssetDropJob
{
    type_id: TypeId,
    asset_id: AssetId,
}

pub(super) enum AssetLifecycleJob
{
    StopWorkers, // signal to the worker threads to stop work

    Load(AssetLoadJob),
    Reload, // TODO
    Drop(AssetDropJob),
}

pub struct AssetStorage<A: Asset>
{
    handles: Mutex<HashMap<AssetId, AssetHandle<A>>>,
    lifecycler: Box<dyn AssetLifecycler<A>>, // pointer+vtable here not ideal
    lifecycle_channel: Sender<AssetLifecycleJob>,
}
impl<A: Asset> AssetStorage<A>
{
    fn get_or_create_handle<S: AsRef<str> + Hash>(&self, asset_path: &S) -> AssetHandle<A>
    {
        let handle_bank = self.handles.lock();

        let id =
        {
            let mut hasher = DefaultHasher::new();
            asset_path.hash(&mut hasher);
            hasher.finish()
        };

        let inner: AssetHandleInner<A> = AssetHandleInner
        {
            ref_count: AtomicUsize::new(1),
            id,
            payload: ArcSwapOption::new(None),
            ready_waker: Mutex::new(None),
        };
        let inner_ptr = Box::into_raw(Box::new(inner)); // manage memory through Box

        let handle = AssetHandle
        {
            inner: inner_ptr,
            phantom: Default::default(),
        };
        handle
    }

    pub fn load<S: AsRef<str> + Hash>(&self, asset_path: &S) -> AssetHandle<A>
    {
        let handle = self.get_or_create_handle(asset_path);

        // TODO

        handle
    }

    pub(super) fn load_cb(&self, request: AssetLoadRequest<A>)
    {
        self.lifecycler.create_or_update(request);
    }
}

trait DynAssetStorage
{
    // total hax

    fn untyped_load(&self, asset_id: AssetId);
    fn untyped_unload(&self, asset_id: AssetId); // take in untyped handle ptr?
}
impl<A: Asset> DynAssetStorage for AssetStorage<A>
{
    fn untyped_load(&self, asset_path: &S)
    {
        todo!()
    }

    fn untyped_unload(&self, asset_id: AssetId)
    {
        todo!()
    }
}

pub struct AssetStorages
{
    assets: HashMap<TypeId, Arc<dyn DynAssetStorage>>, // todo: trait based lookup?
    lifecycle_channel: Sender<AssetLifecycleJob>,
}

pub struct Assets
{
    storage: Arc<AssetStorages>,
    worker_threads: [Option<JoinHandle<()>>; NUM_ASSET_JOB_THREADS],
}
impl Assets
{
    pub fn register_type<A: Asset + 'static, L: AssetLifecycler<A> + 'static>(&mut self, lifecycler: L)
    {
        let asset_type_id = TypeId::of::<A>();

        let existed = self.storage.assets.insert(asset_type_id, Arc::new(AssetStorage
        {
            handles: Default::default(),
            lifecycler: Box::new(lifecycler),
            lifecycle_channel: self.storage.lifecycle_channel.clone(),
        }));
        if existed.is_some()
        {
            panic!("Assets already has a registered handler for {}", name_of_type!(A));
        }
    }

    pub fn load<A: Asset + 'static, S: AsRef<str> + Hash>(&self, asset_path: &S) -> AssetHandle<A>
    {
        let asset_type_id = TypeId::of::<A>();
        match self.storage.assets.get(&asset_type_id)
        {
            None => { panic!("Unsupported asset type - Did you make sure to register the asset type '{}'", name_of_type!(A)); }
            Some(storage) =>
            {
                let storage= unsafe { &*(storage.as_ref() as *const dyn DynAssetStorage as *const AssetStorage<A>) };
                storage.load(asset_path)
            }
        }
    }
}
const NUM_ASSET_JOB_THREADS: usize = 1;
impl Default for Assets
{
    fn default() -> Self
    {
        let (send, recv) = unbounded::<AssetLifecycleJob>();

        let storage = Arc::new(AssetStorages
        {
            assets: Default::default(),
            lifecycle_channel: send,
        });

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
                                    AssetLifecycleJob::Load(load) =>
                                    {
                                        let storage = this_storage.assets.get(&load.type_id).expect("Failed to get asset storage during load job");


                                    },
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
            storages: Default::default(),
            lifecycle_channel: send,
        }
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    pub struct TestAsset
    {
        value: u64,
    }
    impl Asset for TestAsset { }

    pub struct TestAssetLifecycler
    {
    }
    impl AssetLifecycler<TestAsset> for TestAssetLifecycler
    {
        fn create_or_update(&self, request: AssetLoadRequest<TestAsset>)
        {
            println!("TEST");
        }
    }

    #[test]
    fn basic_load()
    {
        let assets =
        {
            let mut assets = Assets::default();
            assets.register_type(TestAssetLifecycler{});
            assets
        };


    }
}