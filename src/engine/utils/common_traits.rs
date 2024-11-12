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
pub const unsafe fn as_u8_array<T>(t: &T) -> &[u8]
{
    std::slice::from_raw_parts(t as *const T as *const u8, std::mem::size_of::<T>())
}

pub trait IntoU8Box
{
    unsafe fn into_u8_box(self) -> Box<[u8]>;
}
impl<T> IntoU8Box for Box<[T]>
{
    unsafe fn into_u8_box(self) -> Box<[u8]> { Box::from_raw(Box::into_raw(self) as *mut [u8]) }
}
impl<T> IntoU8Box for Vec<T>
{
    unsafe fn into_u8_box(self) -> Box<[u8]> { Box::from_raw(Box::into_raw(self.into_boxed_slice()) as *mut [u8]) }
}

#[cfg(test)]
mod tests
{
    use crate::engine::{AsU8Slice, IntoU8Box};

    #[test]
    fn u8_slice()
    {
        let u16s = Box::<[u16]>::from([0, 1, 2, 3, 4, 5, 6, 7]);
        let u16s_size = std::mem::size_of_val(&u16s);
        let u8s = unsafe { u16s.as_u8_slice() };
        let u8s_size = std::mem::size_of_val(u8s);
        assert_eq!(u16s_size, u8s_size);
    }

    #[test]
    fn u8_box()
    {
        let u16s = Box::<[u16]>::from([0, 1, 2, 3, 4, 5, 6, 7]);
        let u16s_size = std::mem::size_of_val(&u16s);
        let u8s = unsafe { u16s.into_u8_box() };
        let u8s_size = std::mem::size_of_val(&u8s);
        assert_eq!(u16s_size, u8s_size);
    }
}

pub trait ShortTypeName
{
    fn short_type_name() -> &'static str;
}
impl<T> ShortTypeName for T
{
    #[inline]
    fn short_type_name() -> &'static str
    {
        let type_name = std::any::type_name::<T>();
        match type_name.rfind(':')
        {
            None => type_name,
            Some(i) => &type_name[(i + 1)..]
        }
    }
}
