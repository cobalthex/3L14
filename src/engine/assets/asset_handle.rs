use std::any::TypeId;
use std::fmt::{Debug, Display, Formatter, Write};
use std::future::Future;
use std::marker::PhantomData;
use std::mem::swap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::task::{Context, Poll, Waker};
use arc_swap::access::Access;
use arc_swap::ArcSwapOption;
use crossbeam::channel::Sender;
use parking_lot::Mutex;
use crate::engine::{DataPayload, ShortTypeName};

use super::*;

// this should always be converted to an AssetHandle<A> for any refcount ops
// note: be very careful when using these, there are no lifetime guarantees
#[derive(PartialEq, Debug)]
pub(super) struct UntypedHandleInner(usize);
impl UntypedHandleInner
{
    pub const NULL: UntypedHandleInner = UntypedHandleInner(0usize);

    pub fn new<A: Asset>(inner: *const AssetHandleInner<A>) -> Self { Self(inner as usize) }
    pub fn is_null(&self) -> bool { self.0 == 0usize }

    pub unsafe fn as_unchecked<A: Asset>(&self) -> &AssetHandleInner<A> { &*(self.0 as *const AssetHandleInner<A>) }
    pub unsafe fn into_ptr(self) -> *mut u8 { self.0 as *mut u8 }
}

#[derive(Debug, Clone, Copy)]
pub enum AssetLoadError
{
    Shutdown, // The asset system has been shutdown and no new assets can be loaded
    LifecyclerNotRegistered,
    InvalidFormat,
    IOError(std::io::ErrorKind),
    ParseError(u16), // The parse error is specific to each lifecycler -- TODO: strings in debug, ints in release?

    #[cfg(test)]
    Test(u16),
}

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

type _PayloadProjectionFn<A> = fn(&Option<Arc<AssetPayload<A>>>) -> &AssetPayload<A>;
pub type PayloadGuard<A> = arc_swap::access::MapGuard<
    arc_swap::Guard<Option<Arc<AssetPayload<A>>>, arc_swap::DefaultStrategy>,
    _PayloadProjectionFn<A>,
    Option<Arc<AssetPayload<A>>>,
    AssetPayload<A>>; // holy shit this is ugly; this *should* be safe;

pub(super) struct AssetHandleInner<A: Asset>
{
    pub ref_count: AtomicIsize, // TODO: isize?

    pub key: AssetKey,
    pub dropper: Sender<AssetLifecycleRequest>,

    // test only?
    pub ready_waker: Mutex<Option<Waker>>,

    pub payload: ArcSwapOption<AssetPayload<A>>, // will be None while pending -- could this be stored directly, and the implementers are required to handle swapping? (e.g. textures have an internal pointer) -- if this changes from an arc, internal allocations in AssetStorage may need to change
}
impl<A: Asset> AssetHandleInner<A>
{
    // Store a new payload and signal. Note: storing ::Pending will clear the pointer
    pub fn store_payload(&self, payload: AssetPayload<A>)
    {
        match payload
        {
            DataPayload::Pending => self.payload.store(None),
            _ => self.payload.store(Some(Arc::new(payload))),
        }

        let mut locked = self.ready_waker.lock();
        if let Some(waker) = locked.take()
        {
            waker.wake();
        }
    }

    #[inline]
    fn map_payload(payload: &Option<Arc<AssetPayload<A>>>) -> &AssetPayload<A>
    {
        match payload
        {
            None => &AssetPayload::Pending,
            Some(s) => s.as_ref()
        }
    }

    #[inline]
    pub fn payload(&self) -> PayloadGuard<A>
    {
        self.payload.map(Self::map_payload as _PayloadProjectionFn<A>).load()
    }

    #[inline]
    pub fn ref_count(&self) -> isize
    {
        self.ref_count.load(Ordering::Acquire)
    }
}

pub struct AssetHandle<A: Asset>
{
    pub(super) inner: *const AssetHandleInner<A>, // store v-table? (use box), would *maybe* allow calls to virtual methods (test?)
    pub(super) phantom: PhantomData<AssetHandleInner<A>>,
}
impl<A: Asset> AssetHandle<A>
{
    // creation managed by <Assets>

    #[inline]
    pub(super) fn inner(&self) -> &AssetHandleInner<A>
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
        self.inner().payload()
    }

    // The number of references to this asset
    #[inline]
    pub fn ref_count(&self) -> isize
    {
        self.inner().ref_count()
    }

    // Is this asset + all dependencies loaded
    #[inline]
    pub fn is_loaded_recursive(&self) -> bool
    {
        self.payload().is_loaded_recursive()
    }

    #[inline]
    pub(super) unsafe fn clone_from_untyped(untyped: &UntypedHandleInner) -> Self
    {
        let cloned = Self
        {
            inner: untyped.0 as *const AssetHandleInner<A>,
            phantom: Default::default(),
        };
        cloned.add_ref();
        cloned
    }

    #[inline]
    pub(super) unsafe fn attach_from_untyped(untyped: UntypedHandleInner) -> Self
    {
        Self
        {
            inner: untyped.0 as *const AssetHandleInner<A>,
            phantom: Default::default(),
        }
    }

    #[inline]
    pub(super) unsafe fn into_untyped(self) -> UntypedHandleInner
    {
        let untyped = UntypedHandleInner(self.inner as usize);
        std::mem::forget(self);
        untyped
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

        // move ownership into the request
        let inner_untyped =
        {
            let mut fill = std::ptr::null(); // reset to null for help w/ detecting bugs
            swap(&mut self.inner, &mut fill);
            fill
        };

        inner.dropper.send(AssetLifecycleRequest
        {
            asset_type: TypeId::of::<A>(),
            untyped_handle: UntypedHandleInner(inner_untyped as usize),
            kind: AssetLifecycleRequestKind::Drop,
        }).expect("Failed to drop asset as the drop channel already closed"); // todo: error handling
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