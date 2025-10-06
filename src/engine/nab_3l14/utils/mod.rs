pub mod bytes_traits;

use std::fmt::{Debug, Display, Formatter};
use std::hash::Hasher;
use metrohash::MetroHash64;
pub use bytes_traits::*;

pub mod async_completion;
pub mod alloc_slice;
pub mod array;
pub mod varint;
pub mod inline_hash;

// How many bytes to print for a maximum bit width
pub const fn format_width_hex_bytes(max_bits: u8) -> usize
{
    (1 + (max_bits - 1) / 4) as usize
}

pub struct FormatBinary
{
    pub bytes: f64
}
#[allow(non_upper_case_globals)]
impl FormatBinary
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
impl Display for FormatBinary
{
    #[allow(clippy::identity_op)]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        // todo: support decimals?

        // larger sizes not likely to ever be used
        let div =
        {
            if self.bytes > Self::Ti
            {
                (self.bytes / Self::Ti, "Ti")
            }
            else if self.bytes > Self::Gi
            {
                (self.bytes / Self::Gi, "Gi")
            }
            else if self.bytes > Self::Mi
            {
                (self.bytes / Self::Mi, "Mi")
            }
            else if self.bytes > Self::Ki
            {
                (self.bytes / Self::Ki, "Ki")
            }
            else
            {
                (self.bytes, "")
            }
        };
        Display::fmt(&div.0, f)?;
        if f.alternate() { f.write_str(" ")?; }
        f.write_str(div.1)
    }
}
#[macro_export]
macro_rules! format_binary
{
    ($val:expr) => { $crate::utils::FormatBinary { bytes: $val as f64 } };
}
#[cfg(test)]
mod format_binary_tests
{
    use super::*;

    #[test]
    fn values()
    {
        assert_eq!("123", format!("{}", format_binary!(123.0)));
        assert_eq!("123Ki", format!("{}", format_binary!(123.0 * FormatBinary::Ki)));
        assert_eq!("123Mi", format!("{}", format_binary!(123.0 * FormatBinary::Mi)));
        assert_eq!("123Gi", format!("{}", format_binary!(123.0 * FormatBinary::Gi)));
        assert_eq!("123Ti", format!("{}", format_binary!(123.0 * FormatBinary::Ti)));
    }

    #[test]
    fn decimals()
    {
        assert_eq!("123.50Mi", format!("{:.2}", format_binary!(123.0 * FormatBinary::Mi + (FormatBinary::Mi / 2.0))));
        // assert_eq!("123.0MiB", format!("{:.1}", format_bytes!(123.0 * FormatBinary::Mi))); // ?
    }
}

pub trait ShortTypeName
{
    fn short_type_name() -> &'static str;
}
impl<T> ShortTypeName for [T]
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

pub struct NoOpFmtDebug<T>(pub T);
impl<T> Debug for NoOpFmtDebug<T>
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { f.write_str(T::short_type_name()) }
}
impl<T> AsRef<T> for NoOpFmtDebug<T>
{
    fn as_ref(&self) -> &T { &self.0 }
}
impl<T> AsMut<T> for NoOpFmtDebug<T>
{
    fn as_mut(&mut self) -> &mut T { &mut self.0 }
}

pub fn hash_bstrings(seed: u64, bstrings: &[&[u8]]) -> u64
{
    let mut hasher = MetroHash64::with_seed(seed);
    bstrings.iter().for_each(|s| { hasher.write(s); });
    hasher.finish()
}