use std::fmt::{Display, Formatter};
use crate::Affine3;
use approx::{assert_abs_diff_eq, assert_relative_eq, AbsDiffEq, RelativeEq};
use bitcode::{Decode, Encode};
use glam::{Mat4, Quat, Vec3};
use std::ops::{Add, Div, Mul, Neg};

#[derive(Debug, Default, PartialEq, Clone, Copy, Encode, Decode)]
pub struct DualQuat
{
    real: Quat, // also called primal
    dual: Quat,
}
impl DualQuat
{
    pub const IDENTITY: Self = Self { real: Quat::IDENTITY, dual: Quat::from_xyzw(0.0, 0.0, 0.0, 0.0) };

    #[inline] #[must_use]
    pub fn from_rot_trans(rotation: Quat, translation: Vec3) -> Self
    {
        let dual = Quat::from_vec4(translation.extend(0.0)) * 0.5 * rotation;
        Self { real: rotation, dual }
        // normalize?
    }

    #[inline] #[must_use]
    pub fn from_raw(real: Quat, dual: Quat) -> Self
    {
        Self { real, dual }
        // normalize?
    }

    #[inline] #[must_use] pub fn rotation(self) -> Quat { self.real }
    #[inline] #[must_use] pub fn translation(self) -> Vec3 { 2.0 * (self.dual * self.real.conjugate()).xyz() }

    #[inline]
    pub fn translate(&mut self, translation: Vec3)
    {
        let t_quat = Quat::from_vec4(translation.extend(0.0));
        self.dual = self.dual + (t_quat * 0.5) * self.real;
    }
    #[inline] #[must_use]
    pub fn translated(mut self, translation: Vec3) -> Self
    {
        self.translate(translation);
        self
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
    pub fn rotated(mut self, normalized_rotation: Quat) -> Self
    {
        self.rotate(normalized_rotation);
        self
    }

    // linearly lerp two dual-quaternions, and (simplified) normalize after
    // note: t is not clamped internally
    #[must_use]
    pub fn nlerp(self, mut rhs: Self, t: f32) -> Self
    {
        // take the shortest path
        let dot = self.real.dot(rhs.real);
        if dot < 0.0 { rhs = -rhs; }

        Self
        {
            real: self.real + (rhs.real - self.real) * t,
            dual: self.dual + (rhs.dual - self.dual) * t,
        }.simple_normalized()

        // let t_inv = 1.0 - t;
        // Self
        // {
        //     real: (self.real * t_inv) + (rhs.real * t),
        //     dual: (self.dual * t_inv) + (rhs.dual * t),
        // }.simple_normalized()
    }

    // TODO: Dual-quaternion linear blending -- https://arxiv.org/pdf/2303.13395

    // Calculate the screw linear interpolation between two dual quaternions
    #[must_use]
    pub fn sclerp(self, mut rhs: Self, t: f32) -> Self
    {
        // https://borodust.github.io/public/shared/paper_dual-quats.pdf

        // TODO: this seems broken, article above has some tests

        // take the shortest path
        let dot = self.real.dot(rhs.real);
        if dot < 0.0 { rhs = -rhs; }

        let diff = self.conjugate() * rhs;
        let vr = diff.real.xyz();
        let vd = diff.dual.xyz();
        let invr = 1.0 / vr.length();

        // screw params
        let mut angle = 2.0 * diff.real.w.acos();
        let mut pitch = -2.0 * diff.dual.w * invr;
        let direction = vr * invr;
        let moment = (vd - direction * pitch * diff.real.w * 0.5) * invr;

        // exp power
        angle *= t;
        pitch *= t;

        // convert back to dual quats
        let sin = (angle * 0.5).sin();
        let cos = (angle * 0.5).cos();

        let real = Quat::from_axis_angle(direction * sin, cos);
        let dual = Quat::from_axis_angle((sin * moment) + (pitch * 0.5 * cos * direction), -pitch * 0.5 * sin);

        self * Self::from_raw(real, dual)
    }

    // spherically lerp two dual-quaternions, and (true) normalize after
    #[must_use]
    pub fn slerp(self, rhs: Self, t: f32) -> Self
    {
        let translation = Vec3::lerp(self.translation(), rhs.translation(), t);
        let rotation = Quat::slerp(self.rotation(), rhs.rotation(), t);
        Self::from_rot_trans(rotation, translation).true_normalized()
    }

    #[inline] #[must_use]
    pub fn transform_vector3(&self, direction: Vec3) -> Vec3 { self.rotation() * direction }
    #[inline] #[must_use]
    pub fn transform_point3(&self, point: Vec3) -> Vec3 { self.rotation() * point + self.translation() }

    #[inline] #[must_use] pub fn simple_length(&self) -> f32 { self.real.length() }
    #[inline] #[must_use] pub fn simple_length_squared(&self) -> f32 { self.real.length_squared() }
    #[inline] #[must_use]
    pub fn true_length(self) -> f32
    {
        let len = self.real.length();
        len + (self.real.dot(self.dual) * (1.0 / len))
    }

    #[inline] #[must_use]
    pub fn simple_normalized(self) -> Self
    {
        let len_recip = 1.0 / self.simple_length();
        Self { real: self.real * len_recip, dual: self.dual * len_recip }
    }
    #[inline] #[must_use]
    pub fn true_normalized(self) -> Self
    {
        let real_len_recip = 1.0 / self.real.length();
        let real_norm = self.real * real_len_recip;
        Self
        {
            real: real_norm,
            dual: (self.dual * real_len_recip) - real_norm * (self.real.dot(self.dual) * (real_len_recip * real_len_recip)),
        }
    }
    // true normalization?

    #[inline] #[must_use]
    pub fn conjugate(self) -> Self
    {
        Self
        {
            real: self.real.conjugate(),
            dual: self.dual.conjugate(),
        }
    }
    #[inline] #[must_use]
    pub fn dual_number_conjugate(self) -> Self
    {
        Self
        {
            real: self.real,
            dual: -self.dual,
        }
    }
    #[inline] #[must_use]
    pub fn combined_conjugate(self) -> Self
    {
        Self
        {
            real: self.real.conjugate(),
            dual: -self.dual.conjugate(),
        }
    }

    #[inline] #[must_use]
    pub fn inverse(self) -> Self
    {
        let real_inv = self.real.conjugate() / self.real.length_squared();
        Self
        {
            real: real_inv,
            dual: -(self.dual * real_inv),
        }
    }

    #[inline] #[must_use]
    pub fn dot(self, other: Self) -> f32
    {
        self.real.dot(other.real) + self.real.dot(other.dual) + self.dual.dot(other.real)
    }
}

impl From<&Mat4> for DualQuat
{
    fn from(value: &Mat4) -> Self
    {
        let (_scale, rotation, translation) = value.to_scale_rotation_translation();
        // TODO: handle scale?
        Self::from_rot_trans(rotation, translation)
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
impl From<&DualQuat> for Affine3
{
    fn from(value: &DualQuat) -> Self
    {
        let rot = value.rotation();
        let trans = value.translation();
        Affine3::from_rotation_translation(rot, trans)
    }
}
impl From<DualQuat> for Affine3 { fn from(value: DualQuat) -> Self { Self::from(&value) } }

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
impl Neg for DualQuat
{
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::Output
        {
            real: -self.real,
            dual: -self.dual,
        }
    }
}

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
impl Display for DualQuat
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        f.write_fmt(format_args!("{{ Rotation: {}, Translation: {} }}", self.rotation(), self.translation()))
    }
}

#[cfg(test)]
mod tests
{
    use super::*;
    use approx::{assert_abs_diff_eq, assert_relative_eq, assert_relative_ne};

    #[test]
    fn create_extract()
    {
        let r = Quat::from_rotation_y(0.345);
        let t = Vec3::new(1.0, 2.0, 3.0);

        let dq = DualQuat::from_rot_trans(r, t);

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
        let dq = DualQuat::from_rot_trans(r, t);

        let test = Vec3::new(10.0, 3.032, 8.5);

        assert_relative_eq!(m.transform_vector3(test), dq.transform_vector3(test));
        assert_relative_eq!(m.transform_point3(test), dq.transform_point3(test));
    }

    #[test]
    fn translate()
    {
        let r = Quat::IDENTITY;
        let t = Vec3::new(1.0, 40.0, 3.0);
        let dq = DualQuat::from_rot_trans(r, t);

        let test = Vec3::new(10.0, 11.0, 12.0);

        assert_relative_eq!(DualQuat::from_rot_trans(r, t + test), dq.translated(test));
        // assert_relative_eq!(DualQuat::new(-r, -(t + test)), dq.translated(test)); // TODO
    }

    #[test]
    fn rotate()
    {
        let r = Quat::from_rotation_y(3.5);
        let t = Vec3::ZERO;
        let dq = DualQuat::from_rot_trans(r, t);

        let test = Quat::from_rotation_y(3.0);

        assert_relative_eq!(DualQuat::from_rot_trans(r * test, t), dq.rotated(test));
    }

    // TODO: to/from mat4
    // TODO: multiply, add, conjugate, length, inverse, dot
}