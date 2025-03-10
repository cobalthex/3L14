use std::alloc::{Layout, LayoutError};
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::ptr;

#[derive(Debug)]
pub enum AllocError
{
    Layout(LayoutError),
    Alloc,
}
impl Display for AllocError
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { Debug::fmt(self, f) }
}
impl Error for AllocError { }

unsafe fn alloc_slice_internal<T>(n: usize) -> Result<(*mut u8, usize), AllocError>
{
    let layout = Layout::array::<T>(n).map_err(AllocError::Layout)?;
    let ptr = unsafe { std::alloc::alloc(layout) };
    if ptr.is_null()
    {
        return Err(AllocError::Alloc);
    }

    Ok((ptr, layout/*.pad_to_align()*/.size()))
}

/// # Safety
/// It is up to the caller to safely initialize the data before it is dropped
pub unsafe fn alloc_slice_uninit<T>(n: usize) -> Result<Box<[T]>, AllocError>
{
    let alloc = unsafe { alloc_slice_internal::<T>(n)? };

    // necessary? (allocator may do already)
    #[cfg(debug_assertions)]
    {
        const DEBUG_UNINIT_FILL_PATTERN: u8 = 0x89;
        unsafe { alloc.0.write_bytes(DEBUG_UNINIT_FILL_PATTERN, alloc.1) };
    }
    // should this pre-fill with T?

    Ok(unsafe { Box::from_raw(std::slice::from_raw_parts_mut(alloc.0.cast::<T>(), n)) })
}

pub fn alloc_slice_default<T: Default>(n: usize) -> Result<Box<[T]>, AllocError>
{
    unsafe
    {
        let alloc = alloc_slice_internal::<T>(n)?;
        let t_ptr: *mut T = alloc.0.cast();
        for i in 0..n
        {
            ptr::write(t_ptr.add(i), T::default());
        }
        Ok(Box::from_raw(std::slice::from_raw_parts_mut(t_ptr, n)))
    }
}

pub fn alloc_slice_copy<T: Copy>(n: usize, val: T) -> Result<Box<[T]>, AllocError>
{
    unsafe
    {
        let alloc = alloc_slice_internal::<T>(n)?;
        let t_ptr: *mut T = alloc.0.cast();
        for i in 0..n
        {
            ptr::write(t_ptr.add(i), val);
        }
        Ok(Box::from_raw(std::slice::from_raw_parts_mut(t_ptr, n)))
    }
}

pub fn alloc_slice_fn<T, F: Fn(usize) -> T>(n: usize, create_fn: F) -> Result<Box<[T]>, AllocError>
{
    unsafe
    {
        let alloc = alloc_slice_internal::<T>(n)?;
        let t_ptr: *mut T = alloc.0.cast();
        for i in 0..n
        {
            ptr::write(t_ptr.add(i), create_fn(i));
        }
        Ok(Box::from_raw(std::slice::from_raw_parts_mut(t_ptr, n)))
    }
}

pub fn alloc_u8_slice<T>(t: T) -> Result<Box<[u8]>, AllocError>
{
    unsafe
    {
        let alloc = alloc_slice_internal::<T>(1)?;
        let t_ptr: *mut T = alloc.0.cast();
        ptr::write(t_ptr, t);
        Ok(Box::from_raw(std::slice::from_raw_parts_mut(alloc.0, alloc.1)))
    }
}