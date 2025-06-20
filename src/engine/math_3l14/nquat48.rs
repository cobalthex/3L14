use std::fmt::{Binary, Debug, Formatter};
use std::mem::MaybeUninit;
use bitcode::{Decode, Encode};
use glam::Quat;
use nab_3l14::const_assert;

// NQuat32:
//  can be done by duplicating this, and setting repr to u/i8
//  rust generics won't allow consts on generic types, so would prob need to be a macro
//  will get shit precision

// Can also do NQuat64 (with u/i32) and bits_per_cmp = 20

type URepr = u16;
type IRepr = i16;
const _BITS_PER_COMPONENT: u32 = size_of::<URepr>() as u32 * 8 - 1;
const _REPR_BYTES: usize = 3 * _BITS_PER_COMPONENT.div_ceil(8) as usize;

const_assert!(size_of::<URepr>() == size_of::<IRepr>());
const_assert!(size_of::<URepr>() * 8 >= _BITS_PER_COMPONENT as usize);
const_assert!(size_of::<IRepr>() * 8 >= _BITS_PER_COMPONENT as usize);
const_assert!(size_of::<u64>() * 8 >= _BITS_PER_COMPONENT as usize * 3);

const_assert!(cfg!(target_endian = "little")); // may work on other endians?

#[inline] #[must_use]
fn f32_to_fixed(f: f32) -> URepr
{
    // -1 is not a fraction due to extra value when negative
    let as_int = (f.clamp(-1.0, NQuat48::MAX_FRAC) * NQuat48::MAX_PRECISION).round_ties_even() as IRepr;
    (as_int as URepr) >> 1
}

#[inline] #[must_use]
fn f32_from_fixed(i: URepr) -> f32
{
    let i = (i << 1) as IRepr;
    i as f32 / NQuat48::MAX_PRECISION
}

// Store a normalized quaternion in 48 bits
// This loses precision but is generally sufficient for animations
// bits 0-14, 15-29, 30-44 are smaller three of x,y,z,w
// bits 45,46 are which component is missing
// bit 47 is the sign of the omitted value
// todo: use bit 47 to increase precision of one value?
#[derive(Clone, Copy, PartialEq, Encode, Decode)]
pub struct NQuat48([u8; _REPR_BYTES]);
impl NQuat48
{
    pub const MAX_PRECISION: f32 = (1 << (_BITS_PER_COMPONENT - 1)) as f32;
    pub const MAX_FRAC: f32 = 1.0 - (1.0 / Self::MAX_PRECISION);
}
impl Default for NQuat48
{
    fn default() -> Self { Self([0u8; _REPR_BYTES]) }
}
impl From<Quat> for NQuat48
{
    fn from(value: Quat) -> Self
    {
        debug_assert!(value.is_normalized());

        let vals = value.to_array();
        let mut max = 0;
        for i in 0..vals.len()
        {
            if vals[i].abs() > vals[max].abs()
            {
                max = i;
            }
        }

        let mut bits = vals[max].is_sign_negative() as u64;
        bits = (bits << 2) | (max & 0b11) as u64;
        for i in 0..vals.len()
        {
            let n = 3 - i;
            if n == max { continue }
            bits = (bits << _BITS_PER_COMPONENT) | (f32_to_fixed(vals[n]) as u64);
        }

        unsafe
        {
            let mut final_bits = MaybeUninit::<[u8; _REPR_BYTES]>::uninit();
            std::ptr::copy_nonoverlapping(bits.to_le_bytes().as_ptr(), final_bits.as_mut_ptr() as *mut u8, _REPR_BYTES);
            Self(final_bits.assume_init())
        }
    }
}
impl From<NQuat48> for Quat
{
    fn from(value: NQuat48) -> Self
    {
        let mut bits = 0u64;
        unsafe
        {
            std::ptr::copy_nonoverlapping(value.0.as_ptr(), &mut bits as *mut u64 as *mut u8, _REPR_BYTES);
        }
        let mut vals = [0.0; 4];
        let max = ((bits >> (_BITS_PER_COMPONENT * 3)) & 0b11) as usize;
        let mut sum_sq = 0.0;
        for i in 0..4
        {
            if i == max { continue }
            vals[i] = f32_from_fixed((bits & ((1 << _BITS_PER_COMPONENT) - 1)) as URepr);
            sum_sq += vals[i] * vals[i];
            bits >>= _BITS_PER_COMPONENT;
        }

        vals[max] = (1.0 - sum_sq).sqrt();
        if (bits >> 2) > 0 { vals[max] = -vals[max]; } // this is likely not needed for animations (nlerp/etc will align)

        Quat::from_array(vals)
    }
}
impl Debug for NQuat48
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        let mut bits = 0u64;
        unsafe
        {
            std::ptr::copy_nonoverlapping(self.0.as_ptr(), &mut bits as *mut u64 as *mut u8, 6);
        }

        const COMPONENT_BITS: u32 = _BITS_PER_COMPONENT * 3;
        let max = ((bits >> COMPONENT_BITS) & 0b11) as usize;
        f.write_fmt(format_args!("M:{}{}", max, if (bits >> (COMPONENT_BITS + 2)) > 0 { "[-]" } else { "[+]" }))?;
        for i in 0..4
        {
            if i == max { continue }
            let val = f32_from_fixed((bits & ((1 << _BITS_PER_COMPONENT) - 1)) as URepr);
            f.write_fmt(format_args!(" {}", val))?;
            bits >>= _BITS_PER_COMPONENT;
        }

        Ok(())
    }
}
impl Binary for NQuat48
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        let mut bits = 0u64;
        unsafe
        {
            std::ptr::copy_nonoverlapping(self.0.as_ptr(), &mut bits as *mut u64 as *mut u8, 6);
        }


        const TOTAL_BITS: u32 = _BITS_PER_COMPONENT * 3 + 3;
        bits = bits.reverse_bits() >> (u64::BITS - TOTAL_BITS);
        let mut str = [' ' as u8; TOTAL_BITS as usize + 4];
        let mut next = 0;
        let lens = [1, 2, _BITS_PER_COMPONENT, _BITS_PER_COMPONENT, _BITS_PER_COMPONENT];
        for l in lens
        {
            for _i in 0..l { str[next] = if (bits & 1) == 1 { '1' as u8 } else { '0' as u8 }; bits >>= 1; next += 1; }
            next += 1;
        }

        f.write_str(unsafe { std::str::from_utf8_unchecked(&str) })
    }
}

#[cfg(test)]
mod tests
{
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    #[should_panic]
    fn not_normalized()
    {
        let q = Quat::from_xyzw(10.0, 10.0, 10.0, 10.0);
        let q48 =  NQuat48::from(q);
    }

    #[test]
    #[should_panic]
    fn zero()
    {
        let q = Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
        let q48 =  NQuat48::from(q);
    }

    #[test]
    fn default()
    {
        let q48 =  NQuat48::default();
        let q = Quat::from(q48);
        assert_eq!(q, Quat::from_xyzw(1.0, 0.0, 0.0, 0.0));
    }

    macro_rules! q48_tests
    {
        ($($name:ident: $test_quat:expr),*$(,)?) =>
        { $(
                #[test]
                fn $name()
                {
                    let in_q = Quat::from($test_quat).normalize();
                    let test_48q = NQuat48::from(in_q);
                    println!("Input: {in_q}\n N48Q: {test_48q:?}\n       {test_48q:b}");

                    let out_q =  Quat::from(test_48q);
                    println!("Recon: {out_q}");
                    assert_relative_eq!(in_q, out_q, epsilon = 1e-5);
                }
        )* }
    }
    q48_tests!
    {
        qrx: Quat::from_rotation_x(std::f32::consts::FRAC_PI_4),
        qry: Quat::from_rotation_y(std::f32::consts::FRAC_PI_4),
        qrz: Quat::from_rotation_z(std::f32::consts::FRAC_PI_4),

        x1: Quat::from_xyzw(1.0, 0.0, 0.0, 0.0),
        y1: Quat::from_xyzw(0.0, 1.0, 0.0, 0.0),
        z1: Quat::from_xyzw(0.0, 0.0, 1.0, 0.0),
        w1: Quat::from_xyzw(0.0, 0.0, 0.0, 1.0),

        nx1: Quat::from_xyzw(-1.0, 0.0, 0.0, 0.0),
        ny1: Quat::from_xyzw(0.0, -1.0, 0.0, 0.0),
        nz1: Quat::from_xyzw(0.0, 0.0, -1.0, 0.0),
        nw1: Quat::from_xyzw(0.0, 0.0, 0.0, -1.0),
    }
}