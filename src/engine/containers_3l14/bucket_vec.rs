use std::ops::{Index, IndexMut};
use std::mem::MaybeUninit;

pub struct BucketVec<T>
{
    buckets: Vec<Box<[MaybeUninit<T>]>>,
    bucket_size: usize,
    len: usize,
}
impl<T> BucketVec<T>
{
    #[inline] #[must_use]
    pub fn new(bucket_size: usize) -> Self
    {
        Self
        {
            buckets: Vec::new(),
            bucket_size,
            len: 0,
        }
    }

    #[inline] #[must_use]
    pub fn len(&self) -> usize { self.len }

    #[inline] #[must_use]
    pub fn num_buckets(&self) -> usize { self.buckets.len() }

    pub fn push(&mut self, val: T)
    {
        let rel_idx = self.len % self.bucket_size;
        if rel_idx == 0
        {
            self.buckets.push(Box::new_uninit_slice(self.bucket_size));
        }

        let bucket_idx = self.buckets.len() - 1;
        self.buckets[bucket_idx][rel_idx].write(val);
        self.len += 1;
    }
}
impl<T: Default> BucketVec<T>
{
    // Will shrink if necessary
    pub fn resize_with_default(&mut self, new_size: usize)
    {
        if new_size <= self.len
        {
            // trim elements past the new end
            for i in new_size..self.len
            {
                let bucket_idx = i / self.bucket_size;
                let rel_idx = i % self.bucket_size;
                unsafe { self.buckets[bucket_idx][rel_idx].assume_init_drop(); }
            }
        }
        else
         {
            let mut new_count = new_size - self.len();

            let rel_start = self.len() % self.bucket_size;
            let last_bucket = self.len() / self.bucket_size;
            if rel_start > 0
            {
                let remaining = new_count.min(self.bucket_size - rel_start);
                // fill in existing bucket
                for i in 0..remaining
                {
                    self.buckets[last_bucket][rel_start + i] = MaybeUninit::new(T::default());
                }
                new_count -= remaining;
            }

            // grow to new size
            let new_buckets = new_count.div_ceil(self.bucket_size);
            for b in 0..new_buckets
            {
                self.buckets.push(Box::new_uninit_slice(self.bucket_size));
                for i in 0..new_count.min(self.bucket_size)
                {
                    self.buckets[last_bucket + b][i].write(T::default());
                }
                new_count -= self.bucket_size;
            }
        }
        self.len = new_size;
    }
}
impl<T> Index<usize> for BucketVec<T>
{
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output
    {
        if index >= self.len { panic!("{index} is out of bounds (0-{})", self.len()) };

        let bucket_idx = index / self.bucket_size;
        let rel_idx = index % self.bucket_size;
        unsafe { self.buckets[bucket_idx][rel_idx].assume_init_ref() }
    }
}
impl<T> IndexMut<usize> for BucketVec<T>
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output
    {
        if index >= self.len { panic!("{index} is out of bounds (0-{})", self.len()) };

        let bucket_idx = index / self.bucket_size;
        let rel_idx = index % self.bucket_size;
        unsafe { self.buckets[bucket_idx][rel_idx].assume_init_mut() }
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn basic_creation()
    {
        let mut vec = BucketVec::new(2);
        assert_eq!(0, vec.len());
        assert_eq!(0, vec.num_buckets());

        vec.push(1024u32);
        assert_eq!(1, vec.len());
        assert_eq!(1, vec.num_buckets());
        assert_eq!(1024u32, vec[0]);

        vec.push(1337u32);
        assert_eq!(2, vec.len());
        assert_eq!(1, vec.num_buckets());
        assert_eq!(1024u32, vec[0]);
        assert_eq!(1337u32, vec[1]);

        vec.push(9001u32);
        assert_eq!(3, vec.len());
        assert_eq!(2, vec.num_buckets());
        assert_eq!(1024u32, vec[0]);
        assert_eq!(1337u32, vec[1]);
        assert_eq!(9001u32, vec[2]);

        vec[2] = 12345u32;
        assert_eq!(1024u32, vec[0]);
        assert_eq!(1337u32, vec[1]);
        assert_eq!(12345u32, vec[2]);
    }

    #[test]
    pub fn resize()
    {
        let mut vec = BucketVec::<u32>::new(2);
        assert_eq!(0, vec.len());
        assert_eq!(0, vec.num_buckets());

        vec.resize_with_default(12);
        assert_eq!(12, vec.len());
        assert_eq!(6, vec.num_buckets());
    }
}