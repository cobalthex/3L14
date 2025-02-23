use std::fmt::{Display, Formatter};
use std::ops::{Neg, Rem};

// TODO: do these make sense?

#[derive(Debug, Default, Copy, Clone, PartialEq, PartialOrd)]
pub struct Radians(pub f32);
impl Radians
{
    pub const ZERO: Radians = Radians(0.0);
    pub const PI: Radians = Radians(std::f32::consts::PI);
    pub const TWO_PI: Radians = Radians(std::f32::consts::TAU);
    pub const PI_OVER_TWO: Radians = Radians(std::f32::consts::FRAC_PI_2);
    pub const PI_OVER_FOUR: Radians = Radians(std::f32::consts::FRAC_PI_4);

    // modifies the value to be between -PI and PI
    pub fn normalize(&mut self) -> &Self
    {
        self.0 %= std::f32::consts::PI;
        self
    }

    #[inline] #[must_use] pub const fn to_degrees_f32(self) -> f32 { self.0.to_degrees() }
    #[inline] #[must_use] pub const fn to_degrees(self) -> Degrees { Degrees(self.to_degrees_f32()) }
}
impl Display for Radians
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        Display::fmt(&self.0, f)?;
        f.write_str("rad")
    }
}
impl From<Degrees> for Radians
{
    fn from(degrees: Degrees) -> Self { degrees.to_radians() }
}
impl Neg for Radians
{
    type Output = Self;
    fn neg(self) -> Self { Self(-self.0) }
}
impl Rem for Radians
{
    type Output = Self;
    fn rem(self, rhs: Self) -> Self { Self(Rem::rem(self.0, rhs.0)) }
}
// todo: math ops

#[cfg(test)]
mod radians_tests
{
    use approx::assert_ulps_eq;
    use super::*;

    #[test]
    fn deg_to_rad()
    {
        assert_eq!(Radians::from(Degrees(0.0)), Radians::ZERO);
        assert_eq!(Radians::from(Degrees(180.0)), Radians::PI);
    }

    #[test]
    fn normalize()
    {
        assert_eq!(*Radians(0.0).normalize(), Radians::ZERO);
        assert_eq!(*Radians(1.0).normalize(), Radians(1.0));
        assert_eq!(*Radians(-1.0).normalize(), Radians(-1.0));
        assert_ulps_eq!(Radians(std::f32::consts::PI + std::f32::consts::FRAC_PI_2).normalize().0, std::f32::consts::FRAC_PI_2);
        assert_ulps_eq!(Radians(-std::f32::consts::PI - std::f32::consts::FRAC_PI_2).normalize().0, -std::f32::consts::FRAC_PI_2);
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, PartialOrd)]
pub struct Degrees(pub f32);
impl Degrees
{
    pub const ZERO: Degrees = Degrees(0.0);

    // modifies the value to be between -PI and PI
    pub fn normalize(&mut self) -> &Self
    {
        self.0 %= 180.0;
        self
    }

    #[inline] #[must_use] pub const fn to_radians_f32(self) -> f32 { self.0.to_radians() }
    #[inline] #[must_use] pub const fn to_radians(self) -> Radians { Radians(self.to_radians_f32()) }
}
impl From<Radians> for Degrees
{
    // const version?
    fn from(radians: Radians) -> Self { radians.to_degrees() }
}
// todo: math ops
impl Display for Degrees
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        Display::fmt(&self.0, f)?;
        f.write_str("°")
    }
}
impl Neg for Degrees
{
    type Output = Self;
    fn neg(self) -> Self { Self(-self.0) }
}
impl Rem for Degrees
{
    type Output = Self;
    fn rem(self, rhs: Self) -> Self { Self(Rem::rem(self.0, rhs.0)) }
}

#[cfg(test)]
mod degrees_tests
{
    use approx::assert_ulps_eq;
    use super::*;

    #[test]
    fn rad_to_deg()
    {
        assert_eq!(Degrees::from(Radians(0.0)), Degrees::ZERO);
        assert_eq!(Degrees::from(Radians::PI), Degrees(180.0));
    }

    #[test]
    fn normalize()
    {
        assert_eq!(*Degrees(0.0).normalize(), Degrees::ZERO);
        assert_eq!(*Degrees(1.0).normalize(), Degrees(1.0));
        assert_eq!(*Degrees(-1.0).normalize(), Degrees(-1.0));
        assert_ulps_eq!(Degrees(270.0).normalize().0, 90.0);
        assert_ulps_eq!(Degrees(-270.0).normalize().0, -90.0);
    }
}
// proc macros for 123deg or 3.45rad? or 123deg2rad or 3.45rad2deg