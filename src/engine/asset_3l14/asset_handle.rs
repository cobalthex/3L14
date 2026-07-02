use super::*;
use crossbeam::channel::Sender;
use parking_lot::Mutex;
use std::alloc::Layout;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::hint::unreachable_unchecked;
use std::marker::PhantomData;
use std::mem::swap;
use std::ops::Deref;
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering};
use std::task::{Context, Poll, Waker};
use arc_swap::{ArcSwap, Guard};
use futures::FutureExt;
use triomphe::Arc;

// There is a lot of shenaigans in here to safely do type-erasure

#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum AssetLoadError
{
    Shutdown = 1, // The asset system has been shutdown and no new asset can be loaded
    Canceled, // load request was canceled
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

pub type AssetRefCnt<A> = arc_swap::ArcSwapAny<Option<triomphe::Arc<A>>>;

// A strong reference to the asset data that can be dereferenced to access
// Invariant: the arc is always available, and ptr points to the available data
// This exists to improve ergonomics over a straight Arc
#[must_use]
pub struct AssetView<A: Asset>
{
    // TODO: use guard here?
    arc: Arc<AssetData<A>>, // TODO: Hopefully when offset_of[enum]! is stabilized, that can be used directly into this arc
    ptr: NonNull<A>,
}
impl<A: Asset> AssetView<A>
{
    #[cfg(test)]
    pub fn new(asset: A) -> Self
    {
        let arc = Arc::new(AssetData::Available(asset));
        let ptr = if let AssetData::Available(a) = &*arc { NonNull::from_ref(a) } else { unsafe { unreachable_unchecked() } };
        Self
        {
            ptr,
            arc,
        }
    }
}
impl<A: Asset> Deref for AssetView<A>
{
    type Target = A;
    fn deref(&self) -> &Self::Target
    {
        debug_assert!(matches!(self.arc.deref(), AssetData::Available(_)));
        unsafe { self.ptr.as_ref() }
    }
}
impl<A: Asset> AsRef<A> for AssetView<A>
{
    fn as_ref(&self) -> &A
    {
        debug_assert!(matches!(self.arc.deref(), AssetData::Available(_)));
        unsafe { self.ptr.as_ref() }
    }
}
impl<A: Asset> Clone for AssetView<A>
{
    fn clone(&self) -> Self
    {
        debug_assert!(matches!(self.arc.deref(), AssetData::Available(_)));
        Self { arc: self.arc.clone(), ptr: self.ptr }
    }
}
impl<A: Debug + Asset> Debug for AssetView<A>
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { self.deref().fmt(f) }
}

#[must_use]
#[derive(Debug)]
pub enum AssetSnapshot<A: Asset>
{
    Pending,
    Unavailable(AssetLoadError),
    Available(AssetView<A>),
}
impl<A: Asset> AssetSnapshot<A>
{
    pub fn unwrap(self) -> AssetView<A>
    {
        match self
        {
            AssetSnapshot::Pending => panic!("AssetSnapshot::unwrap() called on pending asset"),
            AssetSnapshot::Unavailable(err) => panic!("AssetSnapshot::unwrap() called on unavailable asset: {:?}", err),
            AssetSnapshot::Available(view) => view,
        }
    }
}

// non-generic members of AssetHandleInner
#[repr(C)]
pub(super) struct AshInnerHeader
{
    pub ref_count: AtomicIsize,

    pub key: AssetKey,
    pub dropper: Sender<AssetLifecycleRequest>,
    //
    // #[cfg(feature = "asset_names")]
    // name: Arc<str>, // stores optional

    // test only?
    pub ready_waker: Mutex<Option<Waker>>,

    // todo: re-evaluate
    pub is_reloading: AtomicBool, // cleared before payload is set
}
impl AshInnerHeader
{
    #[inline] #[must_use]
    pub fn ref_count(&self) -> isize { self.ref_count.load(Ordering::Acquire) }

    // This is a bit hacky, but allows for fully type erased drops
    pub fn enqueue_drop(&self)
    {
        let old_refs = self.ref_count.fetch_sub(1, Ordering::Release);

        #[cfg(feature = "debug_asset_lifetimes")]
        log::debug!("{:?} decrement ref to {}", self.key, old_refs - 1);

        if old_refs != 1
        {
            debug_assert!(old_refs > 0);
            return;
        }

        #[cfg(feature = "debug_asset_lifetimes")]
        log::debug!("{:?} dropping", self.key());

        let erased = ErasedAsh(unsafe { self as *const _ as *const () }); // this is only safe as header is the first member, and AshInner is repr(C)
        self.dropper.send(AssetLifecycleRequest::Drop(erased))
            .expect("Failed to drop asset as the drop channel already closed"); // todo: error handling (can just drop it here?)
    }
}

// The inner pointer to each asset handle. This should generally not be used by itself b/c it does not have any RAII for ref-counting
#[repr(C)]
pub(super) struct AshInner<A: Asset>
{
    header: AshInnerHeader, // this must always be the first field

    data: AssetRefCnt<AssetData<A>>,

    #[cfg(feature = "asset_debug_data")]
    debug_data: AssetRefCnt<A::DebugData>, // if 0, no debug data is stored, else stores Arc<AssetDebugData>
}

impl<A: Asset> AshInner<A>
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
pub(super) struct ErasedAsh(*const ()); // Internal only; does no ref-counting, use AssetHandle<A> - must never be null
impl ErasedAsh
{
    #[must_use]
    pub fn alloc<A: Asset>(key: AssetKey, dropper: Sender<AssetLifecycleRequest>) -> Self
    {
        let inner = AshInner
        {
            header: AshInnerHeader
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
            let alloc: *mut AshInner<A> = std::alloc::alloc(layout).cast();
            std::ptr::write(alloc, inner);
            alloc as *const ()
        })
    }

    pub unsafe fn dealloc<A: Asset>(self)
    {
        debug_assert!(!self.0.is_null());
        debug_assert_eq!(A::asset_type(), unsafe { &*(self.0 as *const AshInner<A>) }.header.key.asset_type());

        unsafe { &*(self.0 as *const AshInner<A>) }.store_data(None); // clears the stored payload

        #[cfg(feature = "debug_asset_lifetimes")]
        log::debug!("{:?} de-alloc asset handle", unsafe { &*self.0 }.key);

        let layout = Layout::for_value(unsafe { &*self.0 });
        unsafe { std::alloc::dealloc(self.0 as *mut u8, layout) };
    }

    #[inline] #[must_use]
    pub fn header(&self) -> &AshInnerHeader
    {
        unsafe { &*(self.0 as *const AshInnerHeader) }
    }
}
impl<A: Asset> AsRef<AshInner<A>> for ErasedAsh
{
    fn as_ref(&self) -> &AshInner<A>
    {
        unsafe { &*(self.0 as *const AshInner<A>) }
    }
}
unsafe impl Sync for ErasedAsh { }
unsafe impl Send for ErasedAsh { }

// A hot-reloadable handle to an asset.
// Do not store references to the internal payload longer than necessary
#[repr(transparent)]
pub struct Ash<A: Asset>
{
    pub(super) inner: *const AshInner<A>,
    pub(super) phantom: PhantomData<A>,
}
impl<A: Asset> Ash<A>
{
    #[must_use]
    pub(super) unsafe fn into_inner(self) -> ErasedAsh
    {
        let untyped = ErasedAsh(self.inner as *const ());
        std::mem::forget(self);
        untyped
    }

    #[must_use]
    pub(super) unsafe fn clone_from(untyped_handle: &ErasedAsh) -> Self
    {
        let handle = Self
        {
            inner: untyped_handle.0 as *const AshInner<A>,
            phantom: PhantomData::default(),
        };
        handle.debug_assert_type();
        handle.add_ref();
        handle
    }

    #[must_use]
    pub(super) unsafe fn attach_from(untyped_handle: ErasedAsh) -> Self
    {
        let handle = Self
        {
            inner: untyped_handle.0 as *const AshInner<A>,
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
    pub(super) fn inner(&self) -> &AshInner<A> { unsafe { &*self.inner } }

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

    #[inline] #[must_use]
    pub fn is_pending(&self) -> bool { return self.inner().data.load().is_none(); }

    // TODO: probably need a 'fast' quick-look at data (visit?), since taking an arc in data() is going to be slower

    // Retrieve a full snapshot of the asset state, with a full, owned pointer to the data if available
    #[inline]
    pub fn data(&self) -> AssetSnapshot<A>
    {
        if let Some(arc) = Guard::into_inner(self.inner().data.load())
        {
            match &*arc
            {
                AssetData::Unavailable(err) => AssetSnapshot::Unavailable(*err),
                AssetData::Available(asset) => AssetSnapshot::Available(AssetView { ptr: NonNull::from_ref(asset), arc }),
            }
        }
        else { AssetSnapshot::Pending }
    }

    // Retrieve optional debug data for this asset. Returns none if the asset_debug_data feature is disabled
    #[inline] #[must_use]
    pub fn debug_data(&self) -> Option<Arc<A::DebugData>> // TODO: return guard
    {
        #[cfg(feature = "asset_debug_data")]
        return self.inner().debug_data.load_full();
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
            AssetData::Available(asset) => asset.all_dependencies_loaded(),
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
        self.inner().header.enqueue_drop();
    }
}
impl<'a, A: Asset> Future for &'a Ash<A> // non-reffing requires being able to consume an Arc
{
    type Output = AssetSnapshot<A>;

    // TODO: re-evaluate
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>
    {
        if let Some(arc) = Guard::into_inner(self.inner().data.load())
        {
            match &*arc
            {
                AssetData::Unavailable(err) => Poll::Ready(AssetSnapshot::Unavailable(*err)),
                AssetData::Available(asset) => Poll::Ready(AssetSnapshot::Available(AssetView { ptr: NonNull::from_ref(asset), arc })),
            }
        }
        else
        {
            let mut locked = self.inner().header.ready_waker.lock();
            swap(&mut *locked, &mut Some(cx.waker().clone()));
            return Poll::Pending
        }
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
            match self.data()
            {
                AssetSnapshot::Pending => "pending",
                AssetSnapshot::Unavailable(_) => "unavailable",
                AssetSnapshot::Available(_) => "available"
            },
            self.ref_count(),
        ))
    }
}
