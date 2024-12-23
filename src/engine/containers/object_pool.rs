use std::cmp::min;
use std::mem::MaybeUninit;
use std::ops::Deref;
use crossbeam::queue::SegQueue;
use crossbeam::sync::ShardedLock;
use crate::engine::alloc_slice::alloc_slice_fn;

type PoolEntryIndex = u16; // bottom bit is entry index, rest of bits are bucket index; u16 allows for 64k entries
pub const OBJECT_POOL_BUCKET_ENTRY_BITS: PoolEntryIndex = 6;
const OBJECT_POOL_BUCKET_ENTRY_COUNT: PoolEntryIndex = 1 << OBJECT_POOL_BUCKET_ENTRY_BITS;

struct Buckets<T>
{
    count: PoolEntryIndex, // total created across all buckets
    buckets: Vec<Box<[MaybeUninit<T>; OBJECT_POOL_BUCKET_ENTRY_COUNT as usize]>>,
}

// TODO: separate version for passing the ctor as part of take?

pub struct ObjectPool<T>
{
    free: SegQueue<PoolEntryIndex>, // ArrayQueue w/ max 2 or 3 buckets of free space? overflow buckets get deleted?
    buckets: ShardedLock<Buckets<T>>,
    create_entry_fn: Box<dyn Fn(usize) -> T>,
}
impl<T> ObjectPool<T>
{
    pub fn new(create_entry_fn: impl Fn(usize) -> T + 'static) -> Self
    {
        Self
        {
            free: Default::default(),
            buckets: ShardedLock::new(Buckets
            {
                count: 0,
                buckets: Vec::new(),
            }),
            create_entry_fn: Box::new(create_entry_fn),
        }
    }

    pub fn free_count(&self) -> usize { self.free.len() }
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
            let mut new_bucket = Box::new([const { MaybeUninit::uninit() }; OBJECT_POOL_BUCKET_ENTRY_COUNT as usize]);
            new_bucket[0].write((create_entry_fn)(count as usize));
            index = (locked.buckets.len() << OBJECT_POOL_BUCKET_ENTRY_BITS) as PoolEntryIndex;
            locked.buckets.push(new_bucket);
        }

        locked.count += 1;
        Some(index)
    }

    #[inline]
    pub fn take(&self) -> ObjectPoolEntryGuard<T>
    {
        self.take_construct(&self.create_entry_fn)
    }

    pub fn take_construct(&self, create_entry_fn: impl Fn(usize) -> T) -> ObjectPoolEntryGuard<T>
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
            entry: unsafe { &*(entry.assume_init_ref() as *const T) }, // this is 'safe' b/c only this guard can refer to this entry
            index,
        }
    }
}
impl<T> Drop for ObjectPool<T>
{
    fn drop(&mut self)
    {
        let mut locked = self.buckets.write().unwrap();
        let mut count = locked.count;
        for bucket in &mut locked.buckets
        {
            let n = min(count, OBJECT_POOL_BUCKET_ENTRY_COUNT);
            for i in 0..n
            {
                unsafe { bucket[i as usize].assume_init_drop() };
            }
            count -= n;
        }
    }
}

pub struct ObjectPoolEntryGuard<'p, T>
{
    pool: &'p ObjectPool<T>,
    entry: &'p T,
    index: PoolEntryIndex,
}
impl<'p, T> Deref for ObjectPoolEntryGuard<'p, T>
{
    type Target = T;
    fn deref(&self) -> &T { &self.entry }
}
impl<'p, T> Drop for ObjectPoolEntryGuard<'p, T>
{
    fn drop(&mut self)
    {
        self.pool.free.push(self.index);
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test()
    {
        let pool = ObjectPool::new(|i| i);
        assert_eq!(pool.total_count(), 0);
        assert_eq!(pool.free_count(), 0);

        {
            let n = pool.take();
            assert_eq!(n.index, 0);
            assert_eq!(pool.total_count(), 1);
            assert_eq!(pool.free_count(), 0);
        }

        assert_eq!(pool.total_count(), 1);
        assert_eq!(pool.free_count(), 1);

        {
            let n = pool.take();
            assert_eq!(n.index, 0);
            {
                let m = pool.take();
                assert_eq!(m.index, 1);
                assert_eq!(pool.total_count(), 2);
                assert_eq!(pool.free_count(), 0);
            }

            assert_eq!(pool.total_count(), 2);
            assert_eq!(pool.free_count(), 1);
        }

        assert_eq!(pool.total_count(), 2);
        assert_eq!(pool.free_count(), 2);
    }

    // TODO: create POOL_BUCKET_ENTRY_MAX + 1 entries
}