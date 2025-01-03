pub trait AsU8Slice<'a>
{
    unsafe fn as_u8_slice(&'a self) -> &'a [u8];
}
impl<'a, T> AsU8Slice<'a> for Vec<T>
{
    unsafe fn as_u8_slice(&'a self) -> &'a [u8]
    {
        std::slice::from_raw_parts(self.as_ptr() as *const u8, self.len() * size_of::<T>())
    }
}
impl<'a, T> AsU8Slice<'a> for &'a [T]
{
    unsafe fn as_u8_slice(&'a self) -> &'a [u8]
    {
        std::slice::from_raw_parts(self.as_ptr() as *const u8, size_of_val(*self))
    }
}
impl<'a, T> AsU8Slice<'a> for [T]
{
    unsafe fn as_u8_slice(&self) -> &'a [u8]
    {
        std::slice::from_raw_parts(self.as_ptr() as *const u8, size_of_val(self))
    }
}
pub const unsafe fn as_u8_array<T>(t: &T) -> &[u8]
{
    std::slice::from_raw_parts(t as *const T as *const u8, size_of::<T>())
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
    use crate::engine::utils::{AsU8Slice, IntoU8Box};

    #[test]
    fn u8_slice()
    {
        let u16s = Box::<[u16]>::from([0, 1, 2, 3, 4, 5, 6, 7]);
        let u16s_size = size_of_val(&u16s);
        let u8s = unsafe { u16s.as_u8_slice() };
        let u8s_size = size_of_val(u8s);
        assert_eq!(u16s_size, u8s_size);
    }

    #[test]
    fn u8_box()
    {
        let u16s = Box::<[u16]>::from([0, 1, 2, 3, 4, 5, 6, 7]);
        let u16s_size = size_of_val(&u16s);
        let u8s = unsafe { u16s.into_u8_box() };
        let u8s_size = size_of_val(&u8s);
        assert_eq!(u16s_size, u8s_size);
    }
}

pub const unsafe fn as_typed_slice<T>(u8_slice: &[u8]) -> &[T]
{
    std::slice::from_raw_parts(u8_slice.as_ptr() as *const T, u8_slice.len() / size_of::<T>())
}
// TODO: const
pub unsafe fn as_typed_slice_mut<T>(u8_slice: &mut [u8]) -> &mut [T]
{
    // TODO: broken
    let align = align_of::<T>();
    let len = u8_slice.len()  / size_of::<T>();
    let p = u8_slice.as_mut_ptr() as *mut T;
    std::slice::from_raw_parts_mut(p, len)
}
pub unsafe fn as_typed_array<T, const N: usize>(u8_slice: &[u8]) -> &[T; N]
{
    unsafe { &*(u8_slice as *const [u8] as *const [T; N]) }
}
pub unsafe fn as_typed_array_mut<T, const N: usize>(u8_slice: &mut [u8]) -> &mut [T; N]
{
    unsafe { &mut *(u8_slice as *mut [u8] as *mut [T; N]) }
}

pub fn write_slice_index<T>(u8_slice: &mut [u8], index: usize, value: T)
{
    unsafe { as_typed_slice_mut(u8_slice)[index] = value; }
}
pub fn write_array_index<T, const N: usize>(u8_slice: &mut [u8], index: usize, value: T)
{
    unsafe { as_typed_array_mut::<_, N>(u8_slice)[index] = value; }
}