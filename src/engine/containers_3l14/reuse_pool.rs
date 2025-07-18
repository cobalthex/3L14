use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::AtomicU32;
use crossbeam::queue::SegQueue;
use crossbeam::sync::ShardedLock;

type PoolEntryIndex = u16; // bottom bit is entry index, rest of bits are bucket index; u16 allows for 64k entries
pub const OBJECT_POOL_BUCKET_ENTRY_BITS: PoolEntryIndex = 6;
const OBJECT_POOL_BUCKET_ENTRY_COUNT: PoolEntryIndex = 1 << OBJECT_POOL_BUCKET_ENTRY_BITS;

#[derive(Default)]
struct Buckets<T>
{
    count: PoolEntryIndex, // total created across all buckets
    buckets: Vec<Box<[MaybeUninit<T>; OBJECT_POOL_BUCKET_ENTRY_COUNT as usize]>>,
}

pub struct ReusePool<T>
{
    free: SegQueue<PoolEntryIndex>, // ArrayQueue w/ max 2 or 3 buckets of free space? overflow buckets get deleted?
    buckets: ShardedLock<Buckets<T>>, // TODO: grow only lock-free storage?
    uid: u32,
}
impl<T> Default for ReusePool<T>
{
    fn default() -> Self
    {
        let uid = Self::UID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        debug_assert!(uid != 0, "ObjectPool UID counter overflowed");

        Self
        {
            free: Default::default(),
            buckets: ShardedLock::new(Buckets
            {
                count: 0,
                buckets: Vec::new(),
            }),
            uid,
        }
    }
}
impl<T> ReusePool<T>
{
    const UID_COUNTER: AtomicU32 = AtomicU32::new(1);

    #[inline] #[must_use]
    pub fn free_count(&self) -> usize { self.free.len() }
    #[inline] #[must_use]
    pub fn total_count(&self) -> usize { self.buckets.read().unwrap().count as usize }

    // returns the first entry, ready for use; or none if a failure happened
    fn extend(&self, create_entry_fn: impl Fn(usize) -> T) -> Option<PoolEntryIndex>
    {
        let mut locked = self.buckets.write().unwrap();
        let count = locked.count;
        if count == PoolEntryIndex::MAX
        {
            return None;
        }

        // todo: push 8 or so entries to the free list at a time

        let index;
        let bucket_local = count & (OBJECT_POOL_BUCKET_ENTRY_COUNT - 1);
        if count > 0 && bucket_local < OBJECT_POOL_BUCKET_ENTRY_COUNT
        {
            locked.buckets[(count >> OBJECT_POOL_BUCKET_ENTRY_BITS) as usize][bucket_local as usize]
                .write((create_entry_fn)(count as usize));
            index = count;
        }
        else
        {
            let mut new_bucket = Box::new([const { MaybeUninit::zeroed() }; OBJECT_POOL_BUCKET_ENTRY_COUNT as usize]);
            new_bucket[0].write((create_entry_fn)(count as usize));
            index = (locked.buckets.len() << OBJECT_POOL_BUCKET_ENTRY_BITS) as PoolEntryIndex;
            locked.buckets.push(new_bucket);
        }
        locked.count += 1;

        Some(index)
    }

    // Take an 'owned' token that doesn't require holding a ref to this pool
    // Note: this will need to be returned or the pool will panic on drop
    #[inline]
    pub fn take_token(&self, create_entry_fn: impl Fn(usize) -> T) -> ObjectPoolToken<T>
    {
        let index = match self.free.pop()
        {
            Some(i) => i,
            None =>
            {
                // this can end up making extra entries if someone frees while this is extending, but that is hopefully rare
                self.extend(&create_entry_fn).expect("Failed to extend object pool") // more graceful error handling?
            }
        };

        ObjectPoolToken
        {
            pool_uid: self.uid,
            entry: index,
            _phantom: PhantomData
        }
    }
    pub fn return_token(&self, token: ObjectPoolToken<T>)
    {
        debug_assert_eq!(token.pool_uid, self.uid);
        self.free.push(token.entry);
    }

    pub fn take(&self, create_entry_fn: impl Fn(usize) -> T) -> ObjectPoolEntryGuard<T>
    {
        let index = match self.free.pop()
        {
            Some(i) => i,
            None =>
            {
                // this can end up making extra entries if someone frees while this is extending, but that is hopefully rare
                self.extend(&create_entry_fn).expect("Failed to extend object pool") // more graceful error handling?
            }
        };

        let locked = self.buckets.read().unwrap();
        let entry = &locked.buckets[(index >> OBJECT_POOL_BUCKET_ENTRY_BITS) as usize][(index & (OBJECT_POOL_BUCKET_ENTRY_COUNT - 1)) as usize];

        ObjectPoolEntryGuard
        {
            pool: &self,
            entry: unsafe { &mut *(entry.as_ptr() as *mut T) }, // this is 'safe' b/c only this guard can refer to this entry
            index,
        }
    }
}
impl<T> Drop for ReusePool<T>
{
    fn drop(&mut self)
    {
        let mut locked = self.buckets.write().unwrap();
        let mut count = locked.count;
        for bucket in &mut locked.buckets
        {
            let n = count.min(OBJECT_POOL_BUCKET_ENTRY_COUNT);
            for i in 0..n
            {
                unsafe { bucket[i as usize].assume_init_drop() };
            }
            count -= n;
        }
    }
}

#[must_use]
pub struct ObjectPoolEntryGuard<'p, T>
{
    pool: &'p ReusePool<T>,
    entry: &'p mut T,
    index: PoolEntryIndex,
}
impl<'p, T> ObjectPoolEntryGuard<'p, T>
{
    #[inline]
    pub fn to_token(self) -> ObjectPoolToken<T>
    {
        let token = ObjectPoolToken
        {
            pool_uid: self.pool.uid,
            entry: self.index,
            _phantom: PhantomData,
        };
        std::mem::forget(self);
        token
    }
}
impl<'p, T> Deref for ObjectPoolEntryGuard<'p, T>
{
    type Target = T;
    fn deref(&self) -> &T { &self.entry }
}
impl<'p, T> DerefMut for ObjectPoolEntryGuard<'p, T>
{
    fn deref_mut(&mut self) -> &mut T { &mut self.entry }
}
impl<'p, T> Drop for ObjectPoolEntryGuard<'p, T>
{
    fn drop(&mut self)
    {
        self.pool.free.push(self.index);
    }
}

#[must_use]
pub struct ObjectPoolToken<T>
{
    pool_uid: u32,
    entry: PoolEntryIndex,
    // todo: generation
    _phantom: PhantomData<T>,
}
impl<T> ObjectPoolToken<T>
{
    pub fn hydrate(&mut self, pool: &ReusePool<T>) -> &mut T
    {
        // store pointer?
        debug_assert_eq!(self.pool_uid, pool.uid);
        let locked = pool.buckets.read().unwrap();
        let entry = &locked.buckets[(self.entry >> OBJECT_POOL_BUCKET_ENTRY_BITS) as usize][(self.entry & (OBJECT_POOL_BUCKET_ENTRY_COUNT - 1)) as usize];
        unsafe { &mut *(entry.as_ptr().cast_mut()) }
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    struct TestEntry(pub usize);

    #[test]
    fn test()
    {
        let pool = ReusePool::default();
        assert_eq!(pool.total_count(), 0);
        assert_eq!(pool.free_count(), 0);

        {
            let entry = pool.take(TestEntry);
            assert_eq!(entry.0, 0);
            assert_eq!(pool.total_count(), 1);
            assert_eq!(pool.free_count(), 0);
        }

        assert_eq!(pool.total_count(), 1);
        assert_eq!(pool.free_count(), 1);

        {
            let entry = pool.take(TestEntry);
            assert_eq!(entry.0, 0);
            {
                let entry2 = pool.take(TestEntry);
                assert_eq!(entry2.0, 1);
                assert_eq!(pool.total_count(), 2);
                assert_eq!(pool.free_count(), 0);
            }

            assert_eq!(pool.total_count(), 2);
            assert_eq!(pool.free_count(), 1);
        }

        assert_eq!(pool.total_count(), 2);
        assert_eq!(pool.free_count(), 2);

        {
            let mut t = pool.take_token(TestEntry);

            assert_eq!(pool.total_count(), 2);
            assert_eq!(pool.free_count(), 1);

            let hydr = t.hydrate(&pool);
            assert_eq!(hydr.0, 1);

            pool.return_token(t);
            assert_eq!(pool.total_count(), 2);
            assert_eq!(pool.free_count(), 2);
        }
    }

    // TODO: create POOL_BUCKET_ENTRY_MAX + 1 entries
}