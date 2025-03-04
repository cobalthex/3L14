use std::ops::{Add, Mul};
use bitcode::{Decode, Encode};
use glam::{Mat4, Quat, Vec3};

#[derive(Debug, Default, PartialEq, Clone, Copy, Encode, Decode)]
pub struct DualQuat
{
    real: Quat, // also called primal
    dual: Quat,
}
impl DualQuat
{
    pub const IDENTITY: Self = Self { real: Quat::IDENTITY, dual: Quat::IDENTITY };

    #[inline] #[must_use]
    pub fn new(rotation: Quat, translation: Vec3) -> Self
    {
        let dual = Quat::from_vec4(translation.extend(0.0)) * 0.5 * rotation;
        Self { real: rotation, dual }
        // normalize?
    }

    #[inline] #[must_use] pub fn rotation(&self) -> Quat { self.real }
    #[inline] #[must_use] pub fn translation(&self) -> Vec3 { 2.0 * (self.dual * self.real.conjugate()).xyz() }

    #[inline] #[must_use] pub fn simple_length(&self) -> f32 { self.real.length() }
    #[inline] #[must_use] pub fn simple_length_squared(&self) -> f32 { self.real.length_squared() }
    #[inline] #[must_use]
    pub fn true_length(&self) -> f32
    {
        let len = self.real.length();
        len + (self.real.dot(self.dual) / len)
    }

    #[inline] #[must_use]
    pub fn simple_normalized(&self) -> Self
    {
        let len = self.simple_length();
        Self { real: self.real / len, dual: self.dual / len }
    }
    #[inline] #[must_use]
    pub fn true_normalized(&self) -> Self
    {
        let real_len = self.real.length();
        let real_norm = self.real / real_len;
        Self
        {
            real: real_norm,
            dual: (self.dual / real_len) - real_norm * (self.real.dot(self.dual) / (real_len * real_len)),
        }
    }
    // true normalization?

    #[inline] #[must_use]
    pub fn conjugate(&self) -> Self
    {
        Self
        {
            real: self.real.conjugate(),
            dual: self.dual.conjugate(),
        }
    }
    #[inline] #[must_use]
    pub fn dual_number_conjugate(&self) -> Self
    {
        Self
        {
            real: self.real,
            dual: -self.dual,
        }
    }
    #[inline] #[must_use]
    pub fn combined_conjugate(&self) -> Self
    {
        Self
        {
            real: self.real.conjugate(),
            dual: -self.dual.conjugate(),
        }
    }

    #[inline] #[must_use]
    pub fn inverse(&self) -> Self
    {
        let real_inv = self.real.conjugate() / self.real.length_squared();
        Self
        {
            real: real_inv,
            dual: -(self.dual * real_inv),
        }
    }
}
impl From<&Mat4> for DualQuat
{
    fn from(value: &Mat4) -> Self
    {
        let (_, rotation, translation) = value.to_scale_rotation_translation();
        Self::new(rotation, translation)
    }
}

impl Mul<DualQuat> for DualQuat
{
    type Output = DualQuat;

    fn mul(self, rhs: DualQuat) -> Self::Output
    {
        Self::Output
        {
            real: self.real * rhs.real,
            dual: self.real * rhs.dual + self.dual * rhs.real,
        }
    }
}
impl Mul<f32> for DualQuat
{
    type Output = DualQuat;

    fn mul(self, scalar: f32) -> Self::Output
    {
        Self::Output
        {
            real: self.real * scalar,
            dual: self.real * scalar,
        }
    }
}
impl Add<DualQuat> for DualQuat
{
    type Output = DualQuat;

    fn add(self, rhs: DualQuat) -> Self::Output
    {
        Self::Output
        {
            real: self.real + rhs.real,
            dual: self.dual + rhs.dual,
        }
    }
}

#[cfg(test)]
mod tests
{
    use approx::assert_relative_eq;
    use super::*;

    #[test]
    pub fn create_extract()
    {
        let r = Quat::from_rotation_y(0.345);
        let t = Vec3::new(1.0, 2.0, 3.0);

        let dq = DualQuat::new(r, t);

        assert_relative_eq!(dq.rotation(), r);
        assert_relative_eq!(dq.translation(), t);
    }

    // TODO: multiply, add, conjugate, length, normalization, inverse
}