use std::fmt::{Display, Formatter};
use std::ops::{Neg, Rem};

use bitcode::{Decode, Encode};
use glam::FloatExt;

// Angle, stored as radians internally
#[derive(Debug, Default, Copy, Clone, PartialEq, PartialOrd, Encode, Decode)]
pub struct Angle(f32);
impl Angle
{
    pub const ZERO: Self = Self::from_radians(0.0);
    pub const PI: Self = Self::from_radians(std::f32::consts::PI);
    pub const TWO_PI: Self = Self::from_radians(std::f32::consts::TAU);
    pub const PI_OVER_TWO: Self = Self::from_radians(std::f32::consts::FRAC_PI_2);
    pub const PI_OVER_FOUR: Self = Self::from_radians(std::f32::consts::FRAC_PI_4);

    #[inline] #[must_use]
    pub const fn from_radians(radians: f32) -> Self { Self(radians) }
    #[inline] #[must_use]
    pub const fn from_degrees(degrees: f32) -> Self { Self(degrees.to_radians()) }

    #[inline] #[must_use]
    pub const fn to_radians(self) -> f32 { self.0 }
    #[inline] #[must_use]
    pub const fn to_degrees(self) -> f32 { self.0.to_degrees() }

    // Lerp the angle, correctly handling wrapping behavior
    #[must_use]
    pub fn lerp(self, to: Self, t: f32) -> Self
    {
        let diff = (to.0 - self.0) % Self::TWO_PI.0;
        let dist = ((2.0 * diff) % Self::TWO_PI.0) - diff;
        Self(self.0 + dist * t)
        // TODO: add tests
    }

    // modifies the value to be between -PI and PI
    #[inline]
    pub fn normalize(&mut self) -> &Self
    {
        self.0 %= std::f32::consts::PI;
        self
    }
}
// Display the angle, by default as radians, alternatively as degrees
impl Display for Angle
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        if f.alternate()
        {
            f.write_fmt(format_args!("{:.1}deg", self.to_degrees()))
        }
        else
        {
            f.write_fmt(format_args!("{:.1}rad", self.to_radians()))
        }
    }
}
impl Neg for Angle
{
    type Output = Self;
    fn neg(self) -> Self { Self(-self.0) }
}
impl Rem for Angle
{
    type Output = Self;
    fn rem(self, rhs: Self) -> Self { Self(Rem::rem(self.0, rhs.0)) }
}
// todo: math ops

#[cfg(test)]
mod tests
{
    use approx::assert_ulps_eq;
    use super::*;

    #[test]
    fn ctors()
    {
        assert_eq!(Angle::from_degrees(0.0), Angle::ZERO);
        assert_eq!(Angle::from_degrees(180.0), Angle::PI);
        assert_eq!(Angle::from_radians(std::f32::consts::PI), Angle::from_degrees(180.0));

        println!("{}", Angle::from_degrees(25.0));
        println!("{:#}", Angle::from_degrees(25.0));
    }

    #[test]
    fn normalize()
    {
        assert_eq!(*Angle::from_radians(0.0).normalize(), Angle::ZERO);
        assert_eq!(*Angle::from_radians(1.0).normalize(), Angle::from_radians(1.0));
        assert_eq!(*Angle::from_radians(-1.0).normalize(), Angle::from_radians(-1.0));
        assert_ulps_eq!(Angle::from_radians(std::f32::consts::PI + std::f32::consts::FRAC_PI_2).normalize().0, std::f32::consts::FRAC_PI_2);
        assert_ulps_eq!(Angle::from_radians(-std::f32::consts::PI - std::f32::consts::FRAC_PI_2).normalize().0, -std::f32::consts::FRAC_PI_2);
    }
}
// proc macros for 123deg or 3.45rad? or 123deg2rad or 3.45rad2deg
