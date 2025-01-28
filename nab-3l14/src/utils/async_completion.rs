use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use parking_lot::Mutex;

#[derive(Default)]
struct AsyncCompletionInternal
{
    completed: bool,
    waker: Option<Waker>,
}
#[derive(Default, Clone)]
pub struct AsyncCompletion
{
    internal: Arc<Mutex<AsyncCompletionInternal>>,
}
impl AsyncCompletion
{
    pub fn complete(self)
    {
        let mut locked = self.internal.lock();
        locked.completed = true;
        if let Some(waker) = locked.waker.take()
        {
            waker.wake()
        }
    }
}
impl Future for AsyncCompletion
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>
    {
        let mut locked = self.internal.lock();
        match locked.completed
        {
            true => Poll::Ready(()),
            false =>
                {
                    locked.waker = Some(cx.waker().clone());
                    Poll::Pending
                },
        }
    }
}

#[cfg(test)]
mod tests
{
    use std::future::Future;
    use std::pin::Pin;
    use std::ptr;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    use super::AsyncCompletion;

    // taken from Waker::noop()
    pub fn make_waker() -> Waker
    {
        const VTABLE: RawWakerVTable = RawWakerVTable::new(
            // Cloning just returns a new no-op raw waker
            |_| RAW,
            // `wake` does nothing
            |_| {},
            // `wake_by_ref` does nothing
            |_| {},
            // Dropping does nothing as we don't allocate anything
            |_| {},
        );
        const RAW: RawWaker = RawWaker::new(ptr::null(), &VTABLE);

        unsafe { Waker::from_raw(RAW) }
    }

    #[test]
    fn complete()
    {
        let mut completion = AsyncCompletion::default();
        let waker = make_waker();

        assert_eq!(Future::poll(Pin::new(&mut completion), &mut Context::from_waker(&waker)), Poll::Pending);
        assert_eq!(Future::poll(Pin::new(&mut completion), &mut Context::from_waker(&waker)), Poll::Pending);
        assert_eq!(Future::poll(Pin::new(&mut completion), &mut Context::from_waker(&waker)), Poll::Pending);

        completion.clone().complete();
        assert_eq!(Future::poll(Pin::new(&mut completion), &mut Context::from_waker(&waker)), Poll::Ready(()));
    }
}