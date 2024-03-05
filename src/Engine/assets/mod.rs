use std::any::{type_name, TypeId};
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::future::Future;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::{Arc, Weak};
use std::task::{Context, Poll, Waker};
use parking_lot::Mutex;
use unicase::UniCase;

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
        let mut hasher =DefaultHasher::default();
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

pub enum AssetLoadError
{
   NotFound,
}

pub enum AssetPayload<A: Asset + ?Sized>
{
    Pending,
    Available(Box<A>),
    Unavailable(AssetLoadError), // shouldn't be stored in-cache
}


pub struct AssetData
{
    key: AssetKey,

    // split into own struct?
    #[cfg(debug_assertions)]
    debug_type_id: TypeId,
    #[cfg(debug_assertions)]
    debug_path: UniCase<String>,

    payload: Arc<AssetPayload<dyn Asset>>,
}
impl AssetData
{
    pub fn key(&self) -> AssetKey { self.key }

    pub fn debug_type_id(&self) -> Option<TypeId>
    {
        match cfg!(debug_assertions)
        {
            true => Some(self.debug_type_id),
            false => None,
        }
    }
    pub fn debug_asset_path(&self) -> Option<&str>
    {
        match cfg!(debug_assertions)
        {
            true => Some(self.debug_path.as_str()),
            false => None,
        }
    }

    pub fn payload<A: Asset>(&self) -> Arc<AssetPayload<A>>
    {
        todo!()
    }
}

pub type AssetHandle = Arc<AssetData>;
pub type WeakAssetHandle = Weak<AssetData>;

pub struct AssetLoadRequest<'a>
{
    key_desc: AssetKeyDesc<&'a str>,
    handle: AssetHandle,

    // method to mark loaded (privately)
}

pub trait AssetLifecycler
{
    fn load(&mut self, request: AssetLoadRequest); // load handle is async?
    fn unload(&mut self, handle: AssetHandle); // does this function make sense, how will it be called?

    // reload
}

#[derive(Default)]
struct AssetCacheInternal
{
    cache: HashMap<AssetKey, WeakAssetHandle>, // todo: make more efficient storage?
    lifecyclers: HashMap<TypeId, Box<dyn AssetLifecycler>>,
}

#[derive(Default)]
pub struct AssetCache
{
    storage: Mutex<AssetCacheInternal>,
}
impl AssetCache
{
    // todo: multi-load requests

    /// Load or retrieve an existing asset.
    pub fn load<A: Asset + 'static, S: AssetPath>(&self, asset_path: UniCase<&S>) -> AssetHandle
    {
        let key_desc = AssetKeyDesc
        {
            path: UniCase::unicode(asset_path.as_ref()),
            type_id: TypeId::of::<A>(),
        };
        let asset_key = (&key_desc).into();

        let mut locked = self.storage.lock();
        if let Some(existing) = locked.cache.get(&asset_key)
        {
            if let Some(strong) = existing.upgrade()
            {
                assert_eq!(strong.key, asset_key);
                #[cfg(debug_assertions)]
                assert_eq!(strong.debug_type_id, TypeId::of::<A>());
                #[cfg(debug_assertions)]
                assert_eq!(strong.debug_path, asset_path);

                return strong;
            }
        }

        let Some(lifecycler) = locked.lifecyclers.get_mut(&TypeId::of::<A>()) else
        {
            // return result?
            panic!("Unknown asset type! Asset type {} is missing an asset lifecycler", type_name::<A>());
        };

        let asset = AssetData
        {
            key: asset_key,
            #[cfg(debug_assertions)]
            debug_type_id: TypeId::of::<A>(),
            #[cfg(debug_assertions)]
            debug_path: asset_path.to_string().into(), // store elsewhere?
            payload: Arc::new(AssetPayload::Pending),
        };
        let handle = AssetHandle::new(asset);

        let load = AssetLoadRequest
        {
            key_desc,
            handle: handle.clone(),
        };
        lifecycler.load(load);

        let _ = locked.cache.insert(asset_key, Arc::downgrade(&handle));
        handle
    }

    pub fn prune(&self)
    {
        let mut locked = self.storage.lock();
        locked.cache.retain(|_, v| v.strong_count() != 0);
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    struct TestAsset
    {
        asset_name: String,
    }
    impl Asset for TestAsset { }

    struct TestLoader;
    impl AssetLifecycler for TestLoader
    {
        fn load(&mut self, request: AssetLoadRequest)
        {
            let payload = Box::new(TestAsset { asset_name: request.key_desc.path.to_string() });
            Arc::get_mut(&request.handle.payload) = Arc::new(AssetPayload::Available(payload));
        }

        fn unload(&mut self, key: AssetHandle)
        {
            todo!()
        }
    }

    #[test]
    #[should_panic]
    fn unsupported_asset_loader()
    {
        let cache = AssetCache::default();
        let handle = cache.load::<TestAsset, _>(UniCase::unicode(&"test"));
    }

    #[test]
    fn pending_asset_load()
    {
        let cache = AssetCache::default();
        let handle = cache.load::<TestAsset, _>(UniCase::unicode(&"test"));
        match *handle.payload
        {
            AssetPayload::Pending => {},
            _ => panic!("Payload is not pending"),
        }
    }
}