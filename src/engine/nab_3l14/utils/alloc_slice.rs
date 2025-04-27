use std::alloc::{Layout, LayoutError};
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::ptr;
use proc_macros_3l14::FancyEnum;
use crate::app::{fatal_error, FatalError, FatalErrorCode};

#[derive(FancyEnum, Debug)]
pub enum AllocError
{
    Layout(LayoutError),
    Alloc,
}
impl FatalErrorCode for AllocError
{
    fn error_code(&self) -> u16 { self.variant_index() as u16 }
}
impl Display for AllocError
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { Debug::fmt(self, f) }
}
impl Error for AllocError { }

unsafe fn alloc_slice_internal<T>(n: usize) -> (*mut u8, usize)
{
    let layout = match Layout::array::<T>(n)
    {
        Ok(layout) => layout,
        Err(err) =>
        {
            fatal_error(FatalError::Memory, AllocError::Layout(err));
        }
    };

    let ptr = unsafe { std::alloc::alloc(layout) };
    if ptr.is_null()
    {
        fatal_error(FatalError::Memory, AllocError::Alloc);
    }

    (ptr, layout/*.pad_to_align()*/.size())
}

/// # Safety
/// It is up to the caller to safely initialize the data before it is dropped
pub unsafe fn alloc_slice_uninit<T>(n: usize) -> Box<[T]>
{
    let alloc = unsafe { alloc_slice_internal::<T>(n) };

    // necessary? (allocator may do already)
    #[cfg(debug_assertions)]
    {
        const DEBUG_UNINIT_FILL_PATTERN: u8 = 0x89;
        unsafe { alloc.0.write_bytes(DEBUG_UNINIT_FILL_PATTERN, alloc.1) };
    }
    // should this pre-fill with T?

    unsafe { Box::from_raw(std::slice::from_raw_parts_mut(alloc.0.cast::<T>(), n)) }
}

pub fn alloc_slice_default<T: Default>(n: usize) -> Box<[T]>
{
    unsafe
    {
        let alloc = alloc_slice_internal::<T>(n);
        let t_ptr: *mut T = alloc.0.cast();
        for i in 0..n
        {
            ptr::write(t_ptr.add(i), T::default());
        }
        Box::from_raw(std::slice::from_raw_parts_mut(t_ptr, n))
    }
}

pub fn alloc_slice_copy<T: Copy>(n: usize, val: T) -> Box<[T]>
{
    unsafe
    {
        let alloc = alloc_slice_internal::<T>(n);
        let t_ptr: *mut T = alloc.0.cast();
        for i in 0..n
        {
            ptr::write(t_ptr.add(i), val);
        }
        Box::from_raw(std::slice::from_raw_parts_mut(t_ptr, n))
    }
}

pub fn alloc_slice_fn<T, F: Fn(usize) -> T>(n: usize, create_fn: F) -> Box<[T]>
{
    unsafe
    {
        let alloc = alloc_slice_internal::<T>(n);
        let t_ptr: *mut T = alloc.0.cast();
        for i in 0..n
        {
            ptr::write(t_ptr.add(i), create_fn(i));
        }
        Box::from_raw(std::slice::from_raw_parts_mut(t_ptr, n))
    }
}

pub fn alloc_u8_slice<T>(t: T) -> Box<[u8]>
{
    unsafe
    {
        let alloc = alloc_slice_internal::<T>(1);
        let t_ptr: *mut T = alloc.0.cast();
        ptr::write(t_ptr, t);
        Box::from_raw(std::slice::from_raw_parts_mut(alloc.0, alloc.1))
    }
}