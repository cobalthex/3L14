pub trait AsIterator<'i>
{
    type Item;
    type AsIter: Iterator<Item = Self::Item>;

    fn as_iter(&'i self) -> Self::AsIter;
}

pub trait AsU8Slice<'a>
{
    unsafe fn as_u8_slice(&'a self) -> &'a [u8];
}
impl<'a, T> AsU8Slice<'a> for Vec<T>
{
    unsafe fn as_u8_slice(&'a self) -> &'a [u8]
    {
        std::slice::from_raw_parts(self.as_ptr() as *const u8, self.len() * std::mem::size_of::<T>())
    }
}
impl<'a, T> AsU8Slice<'a> for &'a [T]
{
    unsafe fn as_u8_slice(&'a self) -> &'a [u8]
    {
        std::slice::from_raw_parts(self.as_ptr() as *const u8, std::mem::size_of_val(*self))
    }
}
impl<'a, T> AsU8Slice<'a> for [T]
{
    unsafe fn as_u8_slice(&self) -> &'a [u8]
    {
        std::slice::from_raw_parts(self.as_ptr() as *const u8, std::mem::size_of_val(self))
    }
}