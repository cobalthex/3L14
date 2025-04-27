use std::ops::{Add, Div, Mul, Neg};
use approx::{AbsDiffEq, RelativeEq};
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

    #[inline]
    pub fn translate(&mut self, translation: Vec3)
    {
        let t_quat = Quat::from_vec4(translation.extend(0.0));
        self.dual = self.dual + (t_quat * 0.5) * self.real;
    }
    #[inline] #[must_use]
    pub fn translated(&self, translation: Vec3) -> Self
    {
        let mut dq = *self;
        dq.translate(translation);
        dq
    }

    // fast rotate 90deg (fast conj calc), fast rotate 180deg (r * d,q * r)

    #[inline]
    pub fn rotate(&mut self, normalized_rotation: Quat)
    {
        let conj = normalized_rotation.conjugate();
        self.real = normalized_rotation * self.real * conj;
        self.dual = normalized_rotation * self.dual * conj;
    }
    #[inline] #[must_use]
    pub fn rotated(&self, normalized_rotation: Quat) -> Self
    {
        let mut dq = *self;
        dq.rotate(normalized_rotation);
        dq
    }

    #[inline] #[must_use]
    pub fn transform_vector3(&self, direction: Vec3) -> Vec3 { self.rotation() * direction }
    #[inline] #[must_use]
    pub fn transform_point3(&self, point: Vec3) -> Vec3 { self.rotation() * point + self.translation() }

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

    #[inline] #[must_use]
    pub fn dot(&self, other: &Self) -> f32
    {
        self.real.dot(other.real) + self.real.dot(other.dual) + self.dual.dot(other.real)
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
impl From<Mat4> for DualQuat { fn from(value: Mat4) -> Self { Self::from(&value) } }
impl From<&DualQuat> for Mat4
{
    fn from(value: &DualQuat) -> Self
    {
        let rot = value.rotation();
        let trans = value.translation();
        Mat4::from_rotation_translation(rot, trans)
    }
}
impl From<DualQuat> for Mat4 { fn from(value: DualQuat) -> Self { Self::from(&value) } }

impl Mul<DualQuat> for DualQuat
{
    type Output = Self;

    fn mul(self, rhs: DualQuat) -> Self::Output
    {
        Self::Output
        {
            real: self.real * rhs.real,
            dual: self.real * rhs.dual + self.dual * rhs.real,
        }
    }
}
impl Div<DualQuat> for DualQuat
{
    type Output = Self;

    // TODO: verify
    fn div(self, rhs: DualQuat) -> Self::Output
    {
        let den = (rhs.real * rhs.real).inverse();
        Self::Output
        {
            real: (self.real * rhs.real) * den,
            dual: (rhs.real * self.dual - self.real * rhs.dual) * den,
        }
    }
}
impl Mul<f32> for DualQuat
{
    type Output = Self;

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
    type Output = Self;

    fn add(self, rhs: DualQuat) -> Self::Output
    {
        Self::Output
        {
            real: self.real + rhs.real,
            dual: self.dual + rhs.dual,
        }
    }
}
// Neg (-real, -dual) does exist, but represents the same transform, so pointless

impl AbsDiffEq for DualQuat
{
    type Epsilon = f32;
    fn default_epsilon() -> Self::Epsilon { Quat::default_epsilon() }
    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool
    {
        // assumes normalized
        self.real.abs_diff_eq(other.real, epsilon) &&
        self.dual.abs_diff_eq(other.dual, epsilon)
    }
}
impl RelativeEq for DualQuat
{
    fn default_max_relative() -> Self::Epsilon { Quat::default_max_relative() }
    fn relative_eq(&self, other: &Self, epsilon: Self::Epsilon, max_relative: Self::Epsilon) -> bool
    {
        // assumes normalized
        self.real.relative_eq(&other.real, epsilon, max_relative) &&
        self.dual.relative_eq(&other.dual, epsilon, max_relative)
    }
}

#[cfg(test)]
mod tests
{
    use approx::{assert_abs_diff_eq, assert_relative_eq, assert_relative_ne};
    use super::*;

    #[test]
    fn create_extract()
    {
        let r = Quat::from_rotation_y(0.345);
        let t = Vec3::new(1.0, 2.0, 3.0);

        let dq = DualQuat::new(r, t);

        assert_relative_eq!(dq.rotation(), r);
        assert_relative_eq!(dq.translation(), t);
    }

    #[test]
    fn simple_normalize()
    {
        let dq = DualQuat
        {
            real: Quat::from_xyzw(1.0, 2.0, 3.0, 4.0),
            dual: Quat::from_xyzw(0.5, 1.0, 1.5, 2.0),
        };

        assert_relative_eq!(dq.simple_length(), dq.real.length());
        assert_relative_ne!(dq.true_length(), dq.real.length());

        let dq_n = dq.simple_normalized();
        assert_relative_eq!(dq_n.simple_length(), 1.0);
        assert_relative_ne!(dq_n.true_length(), 1.0);
    }

    #[test]
    fn true_normalize()
    {
        let dq = DualQuat
        {
            real: Quat::from_xyzw(1.0, 2.0, 3.0, 4.0),
            dual: Quat::from_xyzw(0.5, 1.0, 1.5, 2.0),
        };

        assert_relative_eq!(dq.true_length(), 8.215838); // determine mathematically?

        let dq_n = dq.true_normalized();
        assert_relative_eq!(dq_n.simple_length(), 1.0);
        assert_relative_eq!(dq_n.true_length(), 1.0);
    }

    #[test]
    fn transforms()
    {
        let r = Quat::from_rotation_y(1.345);
        let t = Vec3::new(1.0, 40.0, 3.0);

        let m = Mat4::from_rotation_translation(r, t);
        let dq = DualQuat::new(r, t);

        let test = Vec3::new(10.0, 3.032, 8.5);

        assert_relative_eq!(m.transform_vector3(test), dq.transform_vector3(test));
        assert_relative_eq!(m.transform_point3(test), dq.transform_point3(test));
    }

    #[test]
    fn translate()
    {
        let r = Quat::IDENTITY;
        let t = Vec3::new(1.0, 40.0, 3.0);
        let dq = DualQuat::new(r, t);

        let test = Vec3::new(10.0, 11.0, 12.0);

        assert_relative_eq!(DualQuat::new(r, t + test), dq.translated(test));
        // assert_relative_eq!(DualQuat::new(-r, -(t + test)), dq.translated(test)); // TODO
    }

    #[test]
    fn rotate()
    {
        let r = Quat::from_rotation_y(3.5);
        let t = Vec3::ZERO;
        let dq = DualQuat::new(r, t);

        let test = Quat::from_rotation_y(3.0);

        // TODO
    }

    // TODO: to/from mat4
    // TODO: multiply, add, conjugate, length, inverse, dot
}