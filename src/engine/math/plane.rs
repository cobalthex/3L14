use glam::{Vec3, Vec4, Vec4Swizzles};
use crate::engine::math::{Facing, GetFacing};

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Plane(pub Vec4);
impl Plane
{
    // An 'invalid' plane with all zero values, primarily for 'fast' initialization
    pub const NULL: Plane = Plane(Vec4::new(0.0, 0.0, 0.0, 0.0));

    #[inline] pub const fn new(normal: Vec3, distance: f32) -> Self
    {
        Self(Vec4::new(normal.x, normal.y, normal.z, distance))
    }
    pub fn from_points(a: Vec3, b: Vec3, c: Vec3) -> Self
    {
        let ab = b - a;
        let ac = c - a;

        let cross = Vec3::cross(ab, ac);
        let norm = cross.normalize();
        let dist = -Vec3::dot(norm, a);
        Self(Vec4::new(norm.x, norm.y, norm.z, dist))
    }

    #[inline] pub fn normal(self) -> Vec3 { self.0.xyz() }
    #[inline] pub fn distance(self) -> f32 { self.0.w }

    #[inline] pub fn flipped(self) -> Self
    {
        Self(Vec4::new(-self.0.x, -self.0.y, -self.0.z, self.0.w))
    }

    // TODO: dot(), transform()
    // intersects?

    pub fn normalize(&mut self)
    {
        let len = self.0.xyz().length_recip();
        self.0 *= len;
    }
    pub fn normalized(self) -> Self
    {
        let len = self.0.xyz().length_recip();
        Self(self.0 * len)
    }

    pub fn dot(self, other: Plane) -> f32
    {
        self.0.dot(other.0)
    }
}
impl Default for Plane
{
    // Defaults to pointing in +Z direction
    fn default() -> Self { Plane(Vec4::new(0.0, 0.0, 1.0, 1.0)) }
}
impl From<Vec4> for Plane
{
    fn from(value: Vec4) -> Self
    {
        Self(value)
    }
}
impl From<Plane> for Vec4
{
    fn from(value: Plane) -> Self
    {
        value.0
    }
}
impl GetFacing<Vec3> for Plane
{
    fn get_facing(&self, other: &Vec3) -> Facing
    {
        let dot = self.normal().dot(*other) - self.distance();
        if dot > 0.0 { Facing::InFront }
        else if dot == 0.0 { Facing::On }
        else { Facing::Behind }
    }
}

#[cfg(test)]
mod tests
{
    use approx::assert_relative_eq;
    use super::*;

    fn vec_approx_eq(a: Vec3, b: Vec3)
    {
        assert_relative_eq!(a.x, b.x);
        assert_relative_eq!(a.y, b.y);
        assert_relative_eq!(a.z, b.z);
    }

    #[test]
    fn basic()
    {
        let norm = Vec3::new(1.0, 2.0, 3.0);
        let dist = 3.0;

        let plane = Plane::new(norm, dist);

        assert_eq!(plane.normal(), norm);
        assert_eq!(plane.0.xyz(), norm);
        assert_eq!(plane.distance(), dist);
        assert_eq!(plane.0.w, dist);
    }

    #[test]
    fn three_points()
    {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        let c = Vec3::new(7.0, 8.0, -9.0);

        let plane = Plane::from_points(a, b, c);
        let recip_sqrt2 = 1.0 / 2.0_f32.sqrt();
        vec_approx_eq(plane.normal(), Vec3::new(-recip_sqrt2, recip_sqrt2, 0.0));
        assert_relative_eq!(plane.distance(), -recip_sqrt2);
    }

    #[test]
    fn normalize()
    {
        let mut plane = Plane::new(Vec3::new(1.0, 4.0, 8.0), 3.0);
        let normed = plane.normalized();
        plane.normalize();
        assert_eq!(normed, plane);

        vec_approx_eq(plane.normal(), Vec3::new(1.0 / 9.0, 4.0 / 9.0, 8.0 / 9.0));
        assert_relative_eq!(plane.distance(), 3.0 / 9.0);
    }
}