pub mod common_traits;

use std::fmt::{Display, Formatter};
pub use common_traits::*;

pub mod async_completion;
pub mod alloc_slice;
pub mod varint;

pub struct FormatBytes
{
    pub bytes: f64
}
#[allow(non_upper_case_globals)]
impl FormatBytes
{
    pub const Ki: f64 = 1.0 * 1024.0; // Kibi (Ki)
    pub const Mi: f64 = Self::Ki * 1024.0; // Mebi (Mi)
    pub const Gi: f64 = Self::Mi * 1024.0; // Gibi (Gi)
    pub const Ti: f64 = Self::Gi * 1024.0; // Tebi (Ti)
    // Pebi (Pi)
    // Exbi (Ei)
    // Zebi (Zi)
    // Yobi (Yi)
}
impl Display for FormatBytes
{
    #[allow(clippy::identity_op)]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        // todo: support decimals?

        // larger sizes not likely to ever be used
        if self.bytes > Self::Ti
        {
            f.write_fmt(format_args!("{}TiB", self.bytes / Self::Ti))
        }
        else if self.bytes > Self::Gi
        {
            f.write_fmt(format_args!("{}GiB", self.bytes / Self::Gi))
        }
        else if self.bytes > Self::Mi
        {
            f.write_fmt(format_args!("{}MiB", self.bytes / Self::Mi))
        }
        else if self.bytes > Self::Ki
        {
            f.write_fmt(format_args!("{}KiB", self.bytes / Self::Ki))
        }
        else
        {
            f.write_fmt(format_args!("{}B", self.bytes))
        }
    }
}
#[macro_export]
macro_rules! format_bytes
{
    ($val:expr) => { $crate::engine::utils::FormatBytes { bytes: $val as f64 } };
}

#[cfg(test)]
mod format_bytes_tests
{
    use super::*;

    #[test]
    fn values()
    {
        assert_eq!("123B", format!("{}", format_bytes!(123.0)));
        assert_eq!("123KiB", format!("{}", format_bytes!(123.0 * FormatBytes::Ki)));
        assert_eq!("123MiB", format!("{}", format_bytes!(123.0 * FormatBytes::Mi)));
        assert_eq!("123GiB", format!("{}", format_bytes!(123.0 * FormatBytes::Gi)));
        assert_eq!("123TiB", format!("{}", format_bytes!(123.0 * FormatBytes::Ti)));
    }

    #[test]
    fn decimals()
    {
        assert_eq!("123.5MiB", format!("{:.1}", format_bytes!(123.0 * FormatBytes::Mi + (FormatBytes::Mi / 2.0))));
        // assert_eq!("123.0MiB", format!("{:.1}", format_bytes!(123.0 * FormatBytes::Mi))); // ?
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