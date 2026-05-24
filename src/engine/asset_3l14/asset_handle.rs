use super::*;
use crossbeam::channel::Sender;
use parking_lot::Mutex;
use std::alloc::Layout;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::marker::PhantomData;
use std::mem::swap;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering};
use std::task::{Context, Poll, Waker};
use arc_swap::ArcSwap;
use triomphe::Arc;

#[derive(Debug)]
#[repr(u16)]
pub enum AssetLoadError
{
    Shutdown = 1, // The asset system has been shutdown and no new asset can be loaded
    Cancelled, // load request was cancelled
    MismatchedAssetType, // asset key does not match handle type
    LifecyclerNotRegistered,
    Fetch,
    Parse,
}
impl Display for AssetLoadError
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { Debug::fmt(self, f) }
}
impl Error for AssetLoadError { }

#[derive(Debug)]
pub enum AssetData<Asset>
{
    Unavailable(AssetLoadError), // TODO: store this elsewhere? (loaders spin forever?)
    Available(Asset),
}
impl<A: Asset> AssetData<A>
{
    // Is this + all dependencies loaded/available?
    #[inline]
    pub fn is_loaded_recursive(&self) -> bool
    {
        match self
        {
            Self::Available(a) => a.all_dependencies_loaded(),
            _ => false,
        }
    }

    #[inline] #[must_use]
    pub fn unwrap(&self) -> &A
    {
        match self
        {
            AssetData::Available(a) => a.clone(),
            AssetData::Unavailable(err) => panic!("Asset payload of type {:?} is unavailable (error: {err:?})", A::asset_type()),
        }
    }

    #[inline]
    pub fn map<U>(self, map_fn: impl FnOnce(A) -> U) -> Option<U>
    {
        match self
        {
            AssetData::Available(a) => Some(map_fn(a)),
            _ => None,
        }
    }
}

pub type AssetRefCnt<A> = arc_swap::ArcSwapAny<Option<triomphe::Arc<A>>>;

// todo: just use visitor?
pub struct AssetGuard<A: Asset>(arc_swap::Guard<Option<triomphe::Arc<AssetData<A>>>>);
impl<A: Asset> AssetGuard<A>
{
    #[inline] #[must_use]
    pub fn unwrap(&self) -> &'_ A
    {
        if let Some(asset) = self.0.as_ref()
        {
            match &**asset
            {
                AssetData::Unavailable(_) => panic!("Asset is unavailable"),
                AssetData::Available(a) => a,
            }
        }
        else { panic!("Asset is pending, cannot unwrap") }
    }

    // better name?
    #[inline]
    pub fn snapshot(&self) -> AssetState<'_, A>
    {
        self.0.as_ref().map_or(AssetState::Pending, |d| match &**d
        {
            AssetData::Unavailable(err) => AssetState::Unavailable(err),
            AssetData::Available(asset) => AssetState::Available(asset),
        })
    }
}
#[must_use]
#[derive(Debug)]
pub enum AssetState<'a, A>
{
    Pending,
    Unavailable(&'a AssetLoadError),
    Available(&'a A),
}

// A strong reference to the asset data that can be dereferenced to access
// Data must be available or this is UB
#[must_use]
pub struct AssetView<A: Asset>
{
    arc: Arc<AssetData<A>>,
    ptr: *const A,
}
impl<A: Asset> Deref for AssetView<A>
{
    type Target = A;
    fn deref(&self) -> &Self::Target
    {
        debug_assert!(matches!(self.arc.deref(), AssetData::Available(_)));
        unsafe { &*self.ptr }
    }
}
impl<A: Debug + Asset> Debug for AssetView<A>
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { self.deref().fmt(f) }
}

// non-generic members of AssetHandleInner
#[repr(C)]
pub(super) struct AssetHandleInnerHeader
{
    pub ref_count: AtomicIsize,

    pub key: AssetKey,
    pub dropper: Sender<AssetLifecycleRequest>,
    //
    // #[cfg(feature = "asset_names")]
    // name: Arc<str>, // stores optional

    // test only?
    pub ready_waker: Mutex<Option<Waker>>,

    pub is_reloading: AtomicBool, // cleared before payload is set
}
impl AssetHandleInnerHeader
{
    #[inline] #[must_use]
    pub fn ref_count(&self) -> isize { self.ref_count.load(Ordering::Acquire) }
}

// The inner pointer to each asset handle. This should generally not be used by itself b/c it does not have any RAII for ref-counting
#[repr(C)]
pub(super) struct AssetHandleInner<A: Asset>
{
    header: AssetHandleInnerHeader,

    data: AssetRefCnt<AssetData<A>>,

    #[cfg(feature = "asset_debug_data")]
    debug_data: AssetRefCnt<A::DebugData>, // if 0, no debug data is stored, else stores Arc<AssetDebugData>
}

impl<A: Asset> AssetHandleInner<A>
{
    #[inline]
    pub fn store_data(&self, new_data: Option<AssetData<A>>)
    {
        debug_assert_eq!(A::asset_type(), self.header.key.asset_type());
        // (debug) store a TypeId to verify templates match?

        #[cfg(feature = "debug_asset_lifetimes")]
        log::debug!("{:?} storing new payload", self.key());

        self.data.store(new_data.map(|d| Arc::new(d)));
    }

    #[cfg(feature = "asset_debug_data")]
    #[inline]
    pub fn store_debug_data(&self, debug_data: Option<A::DebugData>)
    {
        #[cfg(feature = "debug_asset_lifetimes")]
        log::debug!("{:?} storing new debug data", self.key());

        self.debug_data.store(debug_data.map(|d| Arc::new(d)));
    }
}

#[derive(PartialEq, Eq)]
pub(super) struct ErasedAssetHandle(*const ()); // Internal only; does no ref-counting, use AssetHandle<A> - must never be null
impl ErasedAssetHandle
{
    #[must_use]
    pub fn alloc<A: Asset>(key: AssetKey, dropper: Sender<AssetLifecycleRequest>) -> Self
    {
        let inner = AssetHandleInner
        {
            header: AssetHandleInnerHeader
            {
                ref_count: AtomicIsize::new(0), // this must be incremented by the caller
                key,
                dropper,
                ready_waker: Mutex::new(None),
                is_reloading: AtomicBool::new(false),
            },
            data: AssetRefCnt::new(None),
            #[cfg(feature = "asset_debug_data")]
            debug_data: AssetRefCnt::new(None),
        };

        #[cfg(feature = "debug_asset_lifetimes")]
        log::debug!("{key:?} alloc");

        let layout = Layout::for_value(&inner);
        Self(unsafe
        {
            let alloc: *mut AssetHandleInner<A> = std::alloc::alloc(layout).cast();
            std::ptr::write(alloc, inner);
            alloc as *const ()
        })
    }

    pub unsafe fn dealloc<A: Asset>(self)
    {
        debug_assert!(!self.0.is_null());
        debug_assert_eq!(A::asset_type(), unsafe { &*(self.0 as *const AssetHandleInner<A>) }.header.key.asset_type());

        unsafe { &*(self.0 as *const AssetHandleInner<A>) }.store_data(None); // clears the stored payload

        #[cfg(feature = "debug_asset_lifetimes")]
        log::debug!("{:?} de-alloc asset handle", unsafe { &*self.0 }.key);

        let layout = Layout::for_value(unsafe { &*self.0 });
        unsafe { std::alloc::dealloc(self.0 as *mut u8, layout) };
    }

    #[inline] #[must_use]
    pub fn header(&self) -> &AssetHandleInnerHeader
    {
        unsafe { &*(self.0 as *const AssetHandleInnerHeader) }
    }
}
impl<A: Asset> AsRef<AssetHandleInner<A>> for ErasedAssetHandle
{
    fn as_ref(&self) -> &AssetHandleInner<A>
    {
        unsafe { &*(self.0 as *const AssetHandleInner<A>) }
    }
}
unsafe impl Sync for ErasedAssetHandle { }
unsafe impl Send for ErasedAssetHandle { }

// A hot-reloadable handle to an asset.
// Do not store references to the internal payload longer than necessary
#[repr(transparent)]
pub struct Ash<A: Asset>
{
    pub(super) inner: *const AssetHandleInner<A>,
    pub(super) phantom: PhantomData<A>,
}
impl<A: Asset> Ash<A>
{
    #[must_use]
    pub(super) unsafe fn into_inner(self) -> ErasedAssetHandle
    {
        let untyped = ErasedAssetHandle(self.inner as *const ());
        std::mem::forget(self);
        untyped
    }

    #[must_use]
    pub(super) unsafe fn clone_from(untyped_handle: &ErasedAssetHandle) -> Self
    {
        let handle = Self
        {
            inner: untyped_handle.0 as *const AssetHandleInner<A>,
            phantom: PhantomData::default(),
        };
        handle.debug_assert_type();
        handle.add_ref();
        handle
    }

    #[must_use]
    pub(super) unsafe fn attach_from(untyped_handle: ErasedAssetHandle) -> Self
    {
        let handle = Self
        {
            inner: untyped_handle.0 as *const AssetHandleInner<A>,
            phantom: PhantomData::default(),
        };
        handle.debug_assert_type();
        handle
    }

    #[inline]
    fn debug_assert_type(&self)
    {
        debug_assert_eq!(A::asset_type(), self.key().asset_type());
    }

    // creation managed by <Assets>

    #[inline] #[must_use]
    pub(super) fn inner(&self) -> &AssetHandleInner<A> { unsafe { &*self.inner } }

    // The key uniquely identifying this asset
    #[inline] #[must_use]
    pub fn key(&self) -> AssetKey
    {
        self.inner().header.key
    }

    // The name of this asset, only available with asset names enabled ("" otherwise)
    // #[inline]
    // pub fn name(&self) -> Option<&str>
    // {
    //     #[cfg(feature = "asset_names")]
    //     return self.inner().name.as_ref();
    //     #[cfg(not(feature = "asset_names"))]
    //     return "";
    // }

    // Get an owned pointer to the asset data. Prefer visit()
    #[inline] #[must_use]
    pub fn unwrap(&self) -> AssetView<A>
    {
        let arc = self.inner().data.load_full().unwrap();
        AssetView
        {
            ptr: &*arc as *const _ as *const A,
            arc,
        }
    }

    // Inspect the data inside the visitor function
    #[inline]
    pub fn visit(&self, visitor: impl FnOnce(AssetState<'_, A>))
    {
        let guard = self.inner().data.load();
        visitor(guard.as_ref().map_or(AssetState::Pending, |d| match &**d
        {
            AssetData::Unavailable(err) => AssetState::Unavailable(err),
            AssetData::Available(asset) => AssetState::Available(asset),
        }))
    }

    #[inline] #[must_use]
    pub fn map<Out>(&self, mapper: impl FnOnce(AssetState<'_, A>) -> Out) -> Out
    {
        let guard = self.inner().data.load();
        mapper(guard.as_ref().map_or(AssetState::Pending, |d| match &**d
        {
            AssetData::Unavailable(err) => AssetState::Unavailable(err),
            AssetData::Available(asset) => AssetState::Available(asset),
        }))
    }

    // Retrieve optional debug data for this asset. Returns none if the asset_debug_data feature is disabled
    #[inline] #[must_use]
    pub fn debug_data(&self) -> AssetState<A::DebugData>
    {
        #[cfg(feature = "asset_debug_data")]
        todo!();
        // return self.inner().debug_data.load();
        #[cfg(not(feature = "asset_debug_data"))]
        return None;
    }

    // The number of references to this asset
    #[inline] #[must_use]
    pub fn ref_count(&self) -> isize { self.inner().header.ref_count() }

    // // Is this asset + all dependencies loaded
    #[inline] #[must_use]
    pub fn is_loaded_recursive(&self) -> bool
    {
        let guard = self.inner().data.load();
        guard.as_ref().map_or(false, |d| match &**d
        {
            AssetData::Unavailable(err) => false,
            AssetData::Available(asset) => true,
        })
    }

    #[inline]
    pub(super) fn add_ref(&self) // add a ref (will cause a leak if misused)
    {
        // see Arc::clone() for details on ordering requirements
        let old_refs = self.inner().header.ref_count.fetch_add(1, Ordering::Acquire);
        debug_assert_ne!(old_refs, isize::MAX);

        #[cfg(feature = "debug_asset_lifetimes")]
        log::debug!("{self:?} increment ref to {}", old_refs + 1);
    }

    #[inline]
    pub(super) fn store_data(&self, data: Option<AssetData<A>>)
    {
        let inner = unsafe { &*self.inner };
        inner.store_data(data);
    }
}
unsafe impl<A: Asset> Send for Ash<A> {}
unsafe impl<A: Asset> Sync for Ash<A> {}
impl<A: Asset> Clone for Ash<A>
{
    fn clone(&self) -> Self
    {
        self.add_ref();
        Self
        {
            inner: self.inner,
            phantom: self.phantom,
        }
    }
}
impl<A: Asset> Drop for Ash<A>
{
    fn drop(&mut self)
    {
        // see Arc::drop() for details on ordering requirements
        let inner = unsafe { &*self.inner };
        let old_refs = inner.header.ref_count.fetch_sub(1, Ordering::Release);

        #[cfg(feature = "debug_asset_lifetimes")]
        log::debug!("{self:?} decrement ref to {}", old_refs - 1);

        if old_refs != 1
        {
            debug_assert!(old_refs > 0);
            return;
        }

        #[cfg(feature = "debug_asset_lifetimes")]
        log::debug!("{:?} dropping", self.key());

        inner.header.dropper.send(AssetLifecycleRequest::Drop(ErasedAssetHandle(self.inner as *const ())))
            .expect("Failed to drop asset as the drop channel already closed"); // todo: error handling (can just drop it here?)
    }
}
impl<'a, A: Asset> Future for &'a Ash<A> // non-reffing requires being able to consume an Arc
{
    type Output = OwnedAssetSnapshot<A>;

    // TODO: re-evaluate
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>
    {
        let guard = self.data_ptr();
        if guard.is_pending()
        {
            let mut locked = self.inner().header.ready_waker.lock();
            swap(&mut *locked, &mut Some(cx.waker().clone()));
            return Poll::Pending
        }
        return Poll::Ready(guard);
    }
}
impl<A: Asset> PartialEq for Ash<A>
{
    fn eq(&self, other: &Self) -> bool
    {
        self.inner == other.inner
    }
}
impl<A: Asset> PartialEq<AssetKey> for Ash<A> // let's hope there's never a collision
{
    fn eq(&self, key: &AssetKey) -> bool
    {
        self.key() == *key
    }
}

impl<A: Asset> Debug for Ash<A>
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        f.write_fmt(
            format_args!("{{ {:#?} {}, {} refs }}",
            self.key(),
            self.map(|snap| match snap
            {
                AssetState::Pending => "pending",
                AssetState::Unavailable(_) => "unavailable",
                AssetState::Available(_) => "available"
            }),
            self.ref_count(),
        ))
    }
}
