use super::*;
use crossbeam::channel::Sender;
use parking_lot::Mutex;
use std::alloc::Layout;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::marker::PhantomData;
use std::mem::swap;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use nab_3l14::utils::ShortTypeName;
// TODO: name Ash<> ?

#[derive(Debug)]
#[repr(u16)]
pub enum AssetLoadError
{
    Shutdown = 1, // The asset system has been shutdown and no new asset can be loaded
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
pub enum AssetPayload<Asset>
{
    // unloaded?
    Pending,
    Unavailable(AssetLoadError),
    Available(Arc<Asset>),
}
impl<A: Asset> AssetPayload<A>
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

    pub fn unwrap(&self) -> Arc<A>
    {
        match self
        {
            AssetPayload::Available(a) => a.clone(),
            _ => panic!("Asset of type {:?} is not available", A::asset_type()),
        }
    }
}

// The inner pointer to each asset handle. This should generally not be used by itself b/c it does not have any RAII for ref-counting
pub(super) struct AssetHandleInner
{
    ref_count: AtomicIsize,

    key: AssetKey,
    dropper: Sender<AssetLifecycleRequest>,
    //
    // #[cfg(feature = "asset_names")]
    // name: Arc<str>, // stores optional

    // test only?
    ready_waker: Mutex<Option<Waker>>,

    is_reloading: AtomicBool, // cleared before payload is set

    payload: AtomicUsize, // if high bit is 1, stores Arc<Asset>

    #[cfg(feature = "asset_debug_data")]
    debug_data: AtomicUsize,
}

impl AssetHandleInner
{
    // if high bit is set, it is storing a pointer
    const PAYLOAD_PTR_MASK: usize = 1 << (usize::BITS - 1);

    pub fn store_payload<A: Asset>(&self, payload: AssetPayload<A>)
    {
        debug_assert_eq!(A::asset_type(), self.key.asset_type());
        // (debug) store a TypeId to verify templates match?

        #[cfg(feature = "debug_asset_lifetimes")]
        log::debug!("{:?} storing new payload", self.key());

        // can maybe get some inspiration from
        // https://vorner.github.io/2020/09/03/performance-cheating.html

        let new_val = match payload
        {
            AssetPayload::Pending => 0,
            AssetPayload::Unavailable(err) => err as usize,
            AssetPayload::Available(arc) =>
            {
                let raw = Arc::into_raw(arc);
                debug_assert_eq!((raw as usize) & Self::PAYLOAD_PTR_MASK, 0); // rethink entire design if this ever fails :)
                (raw as usize) | Self::PAYLOAD_PTR_MASK
            },
        };

        let old_val = self.payload.swap(new_val, Ordering::AcqRel); // seqcst?

        if (old_val & Self::PAYLOAD_PTR_MASK) != 0
        {
            // drop the old arc
            let _arc = unsafe { Arc::from_raw((old_val & !Self::PAYLOAD_PTR_MASK) as *const A) };
        }

        // ideally this entire function should be atomic (RwLock the whole thing?)
        self.is_reloading.store(false, Ordering::Release); // could this just be is_loading() ?

        let mut locked = self.ready_waker.lock();
        if let Some(waker) = locked.take()
        {
            waker.wake();
        }
    }

    #[inline] #[must_use]
    pub fn key(&self) -> AssetKey
    {
        self.key
    }

    #[inline] #[must_use]
    pub fn asset_type(&self) -> AssetTypeId
    {
        self.key.asset_type()
    }

    #[inline] #[must_use]
    pub fn payload<A: Asset>(&self) -> AssetPayload<A> // TODO: this should return a non-cloneable refcount ptr (Guard obj)
    {
        debug_assert_eq!(A::asset_type(), self.asset_type());
        let ptr = self.payload.load(Ordering::Acquire);

        match ptr
        {
            0 => AssetPayload::Pending,
            p if (ptr & Self::PAYLOAD_PTR_MASK) != 0 =>
            {
                let arc = unsafe { Arc::from_raw((p & !Self::PAYLOAD_PTR_MASK) as *const A) };
                let avail = AssetPayload::Available(arc.clone());
                std::mem::forget(arc);
                avail
            },
            e=> AssetPayload::Unavailable(unsafe { std::mem::transmute(e as u16) }),
        }
    }

    #[cfg(feature = "asset_debug_data")]
    #[inline] #[must_use]
    pub fn debug_data<A: AssetDebugData>(&self) -> Option<Arc<A>>
    {
        let ptr = self.debug_data.load(Ordering::Acquire);
        match ptr
        {
            0 => None,
            p =>
            {
                let arc = unsafe { Arc::from_raw(p as *const A) };
                let debug_data = arc.clone();
                std::mem::forget(arc);
                Some(debug_data)
            }
        }
    }

    #[inline] #[must_use]
    pub fn ref_count(&self) -> isize
    {
        self.ref_count.load(Ordering::Acquire)
    }
}

#[derive(PartialEq, Eq)]
pub(super) struct UntypedAssetHandle(*const AssetHandleInner); // Internal only (does no ref-counting, use AssetHandle<A> - must never be null
impl UntypedAssetHandle
{
    pub fn alloc<A: Asset>(key: AssetKey, dropper: Sender<AssetLifecycleRequest>) -> Self
    {
        let inner = AssetHandleInner
        {
            ref_count: AtomicIsize::new(0), // this must be incremented by the caller
            key,
            dropper,
            ready_waker: Mutex::new(None),
            is_reloading: AtomicBool::new(false),
            payload: AtomicUsize::new(0), // pending
            #[cfg(feature = "asset_debug_data")]
            debug_data: AtomicUsize::new(0),
        };

        #[cfg(feature = "debug_asset_lifetimes")]
        log::debug!("{key:?} alloc");

        let layout = Layout::for_value(&inner);
        Self(unsafe
        {
            let alloc: *mut AssetHandleInner = std::alloc::alloc(layout).cast();
            std::ptr::write(alloc, inner);
            alloc as *const AssetHandleInner
        })
    }

    pub unsafe fn dealloc<A: Asset>(self)
    {
        debug_assert!(!self.0.is_null());
        debug_assert_eq!(A::asset_type(), unsafe { &*self.0 }.key.asset_type());

        unsafe { &*self.0 }.store_payload::<A>(AssetPayload::Pending); // clears the stored payload

        #[cfg(feature = "debug_asset_lifetimes")]
        log::debug!("{:?} de-alloc asset handle", unsafe { &*self.0 }.key);

        let layout = Layout::for_value(unsafe { &*self.0 });
        unsafe { std::alloc::dealloc(self.0 as *mut u8, layout) };
    }
}
impl AsRef<AssetHandleInner> for UntypedAssetHandle
{
    fn as_ref(&self) -> &AssetHandleInner
    {
        unsafe { &*self.0 }
    }
}
unsafe impl Sync for UntypedAssetHandle { }
unsafe impl Send for UntypedAssetHandle { }

// A more convenient alias for AssetHandle<A>
pub type Ash<A> = AssetHandle<A>;

// A hot-reloadable handle to an asset.
// Do not store references to the internal payload longer than necessary
#[repr(transparent)]
pub struct AssetHandle<A: Asset>
{
    pub(super) inner: *const AssetHandleInner, // store v-table? (use box), would *maybe* allow calls to virtual methods (test?)
    pub(super) phantom: PhantomData<A>,
}
impl<A: Asset> AssetHandle<A>
{
    pub(super) unsafe fn into_inner(self) -> UntypedAssetHandle
    {
        let untyped = UntypedAssetHandle(self.inner);
        std::mem::forget(self);
        untyped
    }

    pub(super) unsafe fn clone_from(untyped_handle: &UntypedAssetHandle) -> Self
    {
        let handle = Self
        {
            inner: untyped_handle.0,
            phantom: PhantomData::default(),
        };
        handle.debug_assert_type();
        handle.add_ref();
        handle
    }

    pub(super) unsafe fn attach_from(untyped_handle: UntypedAssetHandle) -> Self
    {
        let handle = Self
        {
            inner: untyped_handle.0,
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

    #[inline]
    pub(super) fn inner(&self) -> &AssetHandleInner
    {
        unsafe { &*self.inner }
    }

    // The key uniquely identifying this asset
    #[inline]
    pub fn key(&self) -> AssetKey
    {
        self.inner().key
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

    // Retrieve the current payload, holds a strong reference for as long as the guard exists
    #[inline]
    pub fn payload(&self) -> AssetPayload<A> { self.inner().payload() }

    // The number of references to this asset
    #[inline]
    pub fn ref_count(&self) -> isize
    {
        self.inner().ref_count()
    }
    
    // // Is this asset + all dependencies loaded
    #[inline]
    pub fn is_loaded_recursive(&self) -> bool
    {
        self.payload().is_loaded_recursive()
    }

    #[inline]
    pub(super) fn add_ref(&self) // add a ref (will cause a leak if misused)
    {
        // see Arc::clone() for details on ordering requirements
        let old_refs = self.inner().ref_count.fetch_add(1, Ordering::Acquire);
        debug_assert_ne!(old_refs, isize::MAX);

        #[cfg(feature = "debug_asset_lifetimes")]
        log::debug!("{self:?} increment ref to {}", old_refs + 1);
    }

    pub(super) fn store_payload(&self, payload: AssetPayload<A>)
    {
        let inner = unsafe { &*self.inner };
        inner.store_payload(payload);
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
            inner: self.inner,
            phantom: self.phantom,
        }
    }
}
impl<A: Asset> Drop for AssetHandle<A>
{
    fn drop(&mut self)
    {
        // see Arc::drop() for details on ordering requirements
        let inner = unsafe { &*self.inner };
        let old_refs = inner.ref_count.fetch_sub(1, Ordering::Release);

        #[cfg(feature = "debug_asset_lifetimes")]
        log::debug!("{self:?} decrement ref to {}", old_refs - 1);

        if old_refs != 1
        {
            debug_assert!(old_refs > 0);
            return;
        }

        #[cfg(feature = "debug_asset_lifetimes")]
        log::debug!("{:?} dropping", self.key());

        inner.dropper.send(AssetLifecycleRequest::Drop(UntypedAssetHandle(self.inner))).expect("Failed to drop asset as the drop channel already closed"); // todo: error handling (can just drop it here?)
    }
}
impl<A: Asset> Future for &AssetHandle<A> // non-reffing requires being able to consume an Arc
{
    type Output = AssetPayload<A>;

    // TODO: re-evaluate
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>
    {
        let inner = self.inner();

        if inner.is_reloading.load(Ordering::Acquire)
        {
            let mut locked = inner.ready_waker.lock();
            swap(&mut *locked, &mut Some(cx.waker().clone()));
            return Poll::Pending;
        }

        let p = self.payload();
        match p
        {
            AssetPayload::Pending =>
            {
                let mut locked = inner.ready_waker.lock();
                swap(&mut *locked, &mut Some(cx.waker().clone()));
                Poll::Pending
            },
            AssetPayload::Available(_) | AssetPayload::Unavailable(_) => Poll::Ready(p),
        }
    }
}
impl<A: Asset> PartialEq for AssetHandle<A>
{
    fn eq(&self, other: &Self) -> bool
    {
        self.inner == other.inner
    }
}
impl<A: Asset> PartialEq<AssetKey> for AssetHandle<A> // let's hope there's never a collision
{
    fn eq(&self, key: &AssetKey) -> bool
    {
        self.key() == *key
    }
}

impl<A: Asset> Debug for AssetHandle<A>
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match f.alternate()
        {
            true => f.write_fmt(format_args!("{}:{:?}", A::short_type_name(), self.key())),
            false => self.key().fmt(f)
        }
    }
}