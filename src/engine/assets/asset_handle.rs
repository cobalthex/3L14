use std::alloc::Layout;
use std::any::{Any, TypeId};
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::marker::PhantomData;
use std::mem::swap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::task::{Context, Poll, Waker};
use arc_swap::access::Access;
use arc_swap::{ArcSwap, ArcSwapAny, ArcSwapOption};
use crossbeam::channel::Sender;
use parking_lot::Mutex;
use crate::engine::{DataPayload, ShortTypeName};

use super::*;

// tbh, after change, this could probably just be stored as an Arc

#[derive(Debug, Clone, Copy)]
pub enum AssetLoadError
{
    Shutdown, // The asset system has been shutdown and no new assets can be loaded
    MismatchedAssetType, // asset key does not match handle type
    LifecyclerNotRegistered,
    InvalidFormat,
    IOError(std::io::ErrorKind),
    ParseError(u16), // The parse error is specific to each lifecycler -- TODO: strings in debug, ints in release?

    #[cfg(test)]
    Test(u16),
}
impl Display for AssetLoadError
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { Debug::fmt(self, f) }
}
impl Error for AssetLoadError { }

pub type AssetPayload<Asset> = DataPayload<Asset, AssetLoadError>;
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
}

type _PayloadProjectionFn<A: Asset> = fn(&Option<Arc<()>>) -> &AssetPayload<A>;
// type PayloadGuard<A> = arc_swap::access::MapGuard<
//     arc_swap::Guard<Option<Arc<AssetPayload<A>>>, arc_swap::DefaultStrategy>,
//     _PayloadProjectionFn<A>,
//     Option<Arc<AssetPayload<A>>>,
//     AssetPayload<A>>; // holy shit this is ugly; this *should* be safe;
type PayloadGuard<'a, A> = &'a AssetPayload<A>; // TODO

// The inner pointer to each asset handle. This should generally not be used by itself b/c it does not have any RAII for ref-counting
pub(super) struct AssetHandleInner
{
    ref_count: AtomicIsize,

    key: AssetKey,
    dropper: Sender<AssetLifecycleRequest>,

    // test only?
    ready_waker: Mutex<Option<Waker>>,

    generation: AtomicU32, // updated <after> storing a payload
    is_reloading: AtomicBool, // cleared before payload is set

    payload: *const (), // Will be null while the handle is pending, the typed value is a raw inner to an Arc<A: Asset>
}
impl AssetHandleInner
{
    // Store a new payload and signal. Note: storing ::Pending will clear the pointer
    pub fn store_payload<A: Asset>(&self, payload: AssetPayload<A>)
    {
        debug_assert_eq!(A::asset_type(), self.key.asset_type());

        todo!()
        // match payload
        // {
        //     DataPayload::Pending => self.payload.store(None),
        //     _ =>
        //     {
        //         let arc = Arc::into_raw(Arc::new(payload));
        //         self.payload.store(Some(unsafe { Arc::from_raw(arc as *const ()) }))
        //     },
        // }
        // self.is_reloading.store(false, Ordering::Release); // could this just be is_loading() ?
        //
        // self.generation.fetch_add(1, Ordering::AcqRel); // TODO: verify ordering
        //
        // let mut locked = self.ready_waker.lock();
        // if let Some(waker) = locked.take()
        // {
        //     waker.wake();
        // }
    }
    
    #[inline]
    pub fn key(&self) -> AssetKey
    {
        self.key
    }
    
    #[inline]
    pub fn asset_type(&self) -> AssetTypeId
    {
        self.key.asset_type()
    }

    #[inline]
    pub fn payload<A: Asset>(&self) -> &Arc<AssetPayload<A>>
    {
        if self.payload.is_null() { todo!() }

        let arc = unsafe { Arc::from_raw(self.payload as *const AssetPayload<A>) };
        todo!()
    }

    #[inline]
    pub fn ref_count(&self) -> isize
    {
        self.ref_count.load(Ordering::Acquire)
    }

    #[inline]
    pub fn generation(&self) -> u32 { self.generation.load(Ordering::Acquire) }
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
            generation: AtomicU32::new(0),
            is_reloading: AtomicBool::new(false),
            payload: std::ptr::null(), // pending
        };
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
        debug_assert_eq!(A::asset_type(), (&*self.0).key.asset_type());

        (&*self.0).store_payload::<A>(AssetPayload::Pending); // clears the stored payload

        let layout = Layout::for_value(&*self.0);
        std::alloc::dealloc(self.0 as *mut u8, layout);
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

    // Retrieve the current payload, holds a strong reference for as long as the guard exists
    #[inline]
    pub fn payload(&self) -> PayloadGuard<A>
    {
        todo!()
        // self.inner().payload()
    }

    // The number of references to this asset
    #[inline]
    pub fn ref_count(&self) -> isize
    {
        self.inner().ref_count()
    }

    #[inline]
    pub fn generation(&self) -> u32 { self.inner().generation() }
    //
    // // Is this asset + all dependencies loaded
    #[inline]
    pub fn is_loaded_recursive(&self) -> bool
    {
        todo!()
        // self.payload().is_loaded_recursive()
    }

    #[inline]
    pub(super) fn add_ref(&self) // add a ref (will cause a leak if misused)
    {
        // see Arc::clone() for details on ordering requirements
        let old_refs = self.inner().ref_count.fetch_add(1, Ordering::Acquire);
        debug_assert_ne!(old_refs, isize::MAX);

        // eprintln!("{self:?} inc ref to {}", old_refs + 1);
    }

    pub(super) fn store_payload(&self, payload: AssetPayload<A>)
    {
        let inner = unsafe { &*self.inner };
        if let DataPayload::Unavailable(err) = &payload
        {
            eprintln!("Error loading asset {self:?}: {err:?}");
        }
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

        // eprintln!("{self:?} dec ref to {}", old_refs - 1);

        if old_refs != 1
        {
            debug_assert!(old_refs > 0);
            return;
        }

        inner.dropper.send(AssetLifecycleRequest::Drop(UntypedAssetHandle(self.inner))).expect("Failed to drop asset as the drop channel already closed"); // todo: error handling (can just drop it here?)
    }
}
impl<A: Asset> Future for &AssetHandle<A> // non-reffing requires being able to consume an Arc
{
    type Output = Arc<AssetPayload<A>>;

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

        todo!()
        // match *self.payload()
        // {
        //     AssetPayload::Pending =>
        //     {
        //         let mut locked = inner.ready_waker.lock();
        //         swap(&mut *locked, &mut Some(cx.waker().clone()));
        //         Poll::Pending
        //     },
        //     AssetPayload::Available(_) | AssetPayload::Unavailable(_) => Poll::Ready(inner.payload.load_full().unwrap()),
        // }
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
        f.write_fmt(format_args!("⟨{:?}|{}⟩", self.key(), A::short_type_name()))
    }
}