pub mod async_completion;

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


// fuck rust iterators
// use std::fmt::{Debug, Display, Formatter, Result};
//
// pub trait FmtJoined<'a, Iter: Iterator>
// {
//     fn fmt_join(self, separator: &'a str) -> FmtJoiner<'a, Iter>;
// }
//
// pub struct FmtJoiner<'a, Iter: Iterator>
// {
//     separator: &'a str,
//     iterable: &'a Iter,
// }
//
// impl<'a, T: Iterator> FmtJoined<'a, T> for &'a T
// {
//     #[inline]
//     fn fmt_join(self, separator: &'a str) -> FmtJoiner<'a, T>
//     {
//         FmtJoiner { separator, iterable: &self }
//     }
// }
//
// impl<'a, Iter> Display for &mut FmtJoiner<'a, Iter>
//     where Iter: Iterator,
//           Iter::Item: Display
// {
//     fn fmt(&self, f: &mut Formatter<'_>) -> Result
//     {
//         let mut first = true;
//         for i in self.iterable
//         {
//             match first
//             {
//                 true => { first = false; }
//                 false => { f.write_str(self.separator)?; }
//             };
//             Display::fmt(&i, f)?;
//         }
//         Ok(())
//     }
// }
//
// impl<'a, Iter> Debug for FmtJoiner<'a, Iter>
//     where Iter: Iterator,
//           Iter::Item: Debug
// {
//     fn fmt(&self, f: &mut Formatter<'_>) -> Result
//     {
//         let mut first = true;
//         let mut iter = self.iterable.into_iter();
//         while let Some(i) = iter.next()
//         {
//             match first
//             {
//                 true => { first = false; }
//                 false => { f.write_str(self.separator)?; }
//             };
//             Debug::fmt(&i, f)?;
//         }
//         Ok(())
//     }
// }