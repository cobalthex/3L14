use std::fmt::{Debug, Display};
use std::future::Future;
use std::marker::PhantomData;
use std::mem::swap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Waker};
use arc_swap::access::Access;
use arc_swap::ArcSwapOption;
use crossbeam::channel::Sender;
use parking_lot::Mutex;
use crate::engine::DataPayload;

use super::*;

pub(super) type UntypedHandleInner = usize;

#[derive(Debug)]
pub enum AssetLoadError
{
    NotFound = 100,

    InvalidFormat = 200,

    ParseError = 300,

    #[cfg(test)]
    TestEmpty = 100000000,
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
    pub ref_count: AtomicUsize,

    pub key: AssetKey,
    pub payload: ArcSwapOption<AssetPayload<A>>, // will be None while pending
    pub dropper: Sender<AssetLifecycleJob>,

    // necessary?
    pub ready_waker: Mutex<Option<Waker>>,
}
impl<A: Asset> AssetHandleInner<A>
{
    pub(super) fn signal_waker(&self)
    {
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
    pub fn ref_count(&self) -> usize
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
    pub fn ref_count(&self) -> usize
    {
        self.inner().ref_count()
    }

    // Is this asset + all dependencies loaded
    #[inline]
    pub fn is_loaded_recursive(&self) -> bool
    {
        self.payload().is_loaded_recursive()
    }

    // Create a handle from an untyped inner handle
    #[inline]
    pub(super) unsafe fn clone_from_untyped(untyped: UntypedHandleInner) -> Self
    {
        let cloned = Self
        {
            inner: untyped as *const AssetHandleInner<A>,
            phantom: Default::default(),
        };
        cloned.add_ref();
        cloned
    }

    #[inline]
    pub(super) fn add_ref(&self) // add a ref (will cause a leak if misused)
    {
        // see Arc::clone() for details on ordering requirements
        let _old_refs = self.inner().ref_count.fetch_add(1, Ordering::Acquire);
    }

    pub(super) fn maybe_drop(untyped: UntypedHandleInner) -> Option<AssetKey>
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

        inner.dropper.send(AssetLifecycleJob::Drop(AssetDropJob { handle: (inner_untyped as UntypedHandleInner), drop_fn: Self::maybe_drop })).expect("Failed to drop asset as the drop channel already closed"); // todo: error handling
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
