use std::fmt::{Debug, Formatter};
use glam::{Vec3, Vec4, Vec4Swizzles};
use crate::math::{Facing, GetFacing, Sphere, WORLD_RIGHT, WORLD_UP};
use crate::utils::ShortTypeName;

#[derive(Copy, Clone, PartialEq)]
pub struct Plane(pub Vec4);
impl Plane
{
    // An 'invalid' plane with all zero values, primarily for 'fast' initialization
    pub const NULL: Plane = Plane(Vec4::new(0.0, 0.0, 0.0, 0.0));

    #[inline] #[must_use]
    pub const fn new(normal: Vec3, distance: f32) -> Self
    {
        Self(Vec4::new(normal.x, normal.y, normal.z, distance))
    }

    #[inline] #[must_use]
    pub const fn new_raw(x: f32, y: f32, z: f32, d: f32) -> Self { Self(Vec4::new(x, y, z, d)) }

    #[must_use]
    pub fn from_points(a: Vec3, b: Vec3, c: Vec3) -> Self
    {
        let ab = b - a;
        let ac = c - a;

        let cross = Vec3::cross(ab, ac);
        let norm = cross.normalize();
        let dist = -Vec3::dot(norm, a);
        Self(norm.extend(dist))
    }

    #[inline] #[must_use]
    pub fn normal(self) -> Vec3 { self.0.xyz() }
    #[inline] #[must_use]
    pub fn distance(self) -> f32 { self.0.w }

    #[inline] #[must_use]
    pub fn origin(self) -> Vec3 { self.0.xyz() * self.0.w }

    #[inline] #[must_use]
    pub fn flipped(self) -> Self
    {
        Self(Vec4::new(-self.0.x, -self.0.y, -self.0.z, self.0.w))
    } // negate the normal
    #[inline] #[must_use]
    pub fn negated_distance(self) -> Self
    {
        Self(Vec4::new(self.0.x, self.0.y, self.0.z, -self.0.w))
    } // negate the distance

    // TODO: dot(), transform()
    // intersects?

    pub fn normalize(&mut self)
    {
        let len = self.0.xyz().length_recip();
        self.0 *= len;
    }
    #[inline] #[must_use]
    pub fn normalized(self) -> Self
    {
        let len = self.0.xyz().length_recip();
        Self(self.0 * len)
    }

    #[inline] #[must_use]
    pub fn dot(self, other: Plane) -> f32
    {
        self.0.dot(other.0)
    }

    #[must_use]
    pub fn intersecting_point(a: Self, b: Self, c: Self) -> Option<Vec3>
    {
        let nab = a.normal().cross(b.normal());
        let nbc = b.normal().cross(c.normal());
        let nca = c.normal().cross(a.normal());

        let denom = (a.distance() * nbc) + (b.distance() * nca) + (c.distance() * nab);
        let recip = a.normal().dot(nbc);

        let result = denom / recip;
        (!result.is_nan()).then_some(result)
    }

    // Create a quad with the given half-extents on the plane, centered around the origin
    #[must_use]
    pub fn into_quad(self, half_width: f32, half_height: f32) -> [Vec3; 4]
    {
        let normal = self.normal();
        let tan = normal.cross(if normal == WORLD_UP { WORLD_RIGHT } else { WORLD_UP }); // todo: take in up param?
        let bitan = normal.cross(tan);
        let origin = self.origin();

        [
            origin + half_width * tan + half_height * bitan,
            origin + half_width * tan - half_height * bitan,
            origin - half_width * tan - half_height * bitan,
            origin - half_width * tan + half_height * bitan,
        ]
    }

// intersecting_line?
}
impl Debug for Plane
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        f.debug_struct(Self::short_type_name())
            .field("normal", &self.normal())
            .field("distance", &self.distance())
            .finish()
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
    fn get_facing(&self, other: Vec3) -> Facing
    {
        let d = self.normal().dot(other) - self.distance();
        if d > 0.0 { Facing::InFront }
        else if d == 0.0 { Facing::On }
        else { Facing::Behind }
    }
}
impl GetFacing<Sphere> for Plane
{
    fn get_facing(&self, other: Sphere) -> Facing
    {
        let d = self.normal().dot(other.center()) - self.distance();
        if d >= other.radius() { Facing::InFront }
        else if d >= -other.radius() { Facing::On }
        else { Facing::Behind }
    }
}

#[cfg(test)]
mod tests
{
    use approx::assert_relative_eq;
    use super::*;

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
    fn point_facing()
    {
        let plane = Plane::new(Vec3::new(1.0, 0.0, 0.0), 2.0);

        assert!(matches!(plane.get_facing(Vec3::new(5.0, 0.0, 0.0)), Facing::InFront));
        assert!(matches!(plane.get_facing(Vec3::new(2.0, 0.0, 0.0)), Facing::On));
        assert!(matches!(plane.get_facing(Vec3::new(2.0, 5.0, 0.0)), Facing::On));
        assert!(matches!(plane.get_facing(Vec3::new(0.0, 0.0, 0.0)), Facing::Behind));
    }

    #[test]
    fn sphere_facing()
    {
        let plane = Plane::new(Vec3::new(1.0, 0.0, 0.0), 2.0);
        assert!(matches!(plane.get_facing(Sphere::new(Vec3::new(5.0, 0.0, 0.0), 1.5)), Facing::InFront));
        assert!(matches!(plane.get_facing(Sphere::new(Vec3::new(2.0, 0.0, 0.0), 1.5)), Facing::On));
        assert!(matches!(plane.get_facing(Sphere::new(Vec3::new(1.0, 0.0, 0.0), 1.5)), Facing::On));
        assert!(matches!(plane.get_facing(Sphere::new(Vec3::new(-10.0, 0.0, 0.0), 1.5)), Facing::Behind));
    }

    #[test]
    fn three_points()
    {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        let c = Vec3::new(7.0, 8.0, -9.0);

        let plane = Plane::from_points(a, b, c);
        let recip_sqrt2 = 1.0 / 2.0_f32.sqrt();
        assert!(plane.normal().abs_diff_eq(Vec3::new(-recip_sqrt2, recip_sqrt2, 0.0), 1e-5));
        assert_relative_eq!(plane.distance(), -recip_sqrt2);
    }

    #[test]
    fn normalize()
    {
        let mut plane = Plane::new(Vec3::new(1.0, 4.0, 8.0), 3.0);
        let normed = plane.normalized();
        plane.normalize();
        assert_eq!(normed, plane);

        assert!(plane.normal().abs_diff_eq(Vec3::new(1.0 / 9.0, 4.0 / 9.0, 8.0 / 9.0), 1e-5));
        assert_relative_eq!(plane.distance(), 3.0 / 9.0);
    }

    #[test]
    fn point_intersection()
    {
        let pa = Plane::new(Vec3::new(1.0, 0.0, 0.0), 0.0);
        let pb = Plane::new(Vec3::new(0.0, 1.0, 0.0), 0.0);
        let pc = Plane::new(Vec3::new(0.0, 0.0, 1.0), 0.0);
        
        let intersection = Plane::intersecting_point(pa, pb, pc);
        assert_eq!(intersection, Some(Vec3::new(0.0, 0.0, 0.0)));
    }

    #[test]
    fn no_point_intersection()
    {
        let pa = Plane::new(Vec3::new(0.0, 1.0, 0.0), 0.0);
        let pb = Plane::new(Vec3::new(0.0, 1.0, 0.0), 0.0);
        let pc = Plane::new(Vec3::new(0.0, 1.0, 0.0), 0.0);

        let intersection = Plane::intersecting_point(pa, pb, pc);
        assert_eq!(intersection, None);
    }
}