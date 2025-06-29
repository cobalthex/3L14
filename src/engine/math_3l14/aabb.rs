use bitcode::{Decode, Encode};
use glam::Vec3;
use crate::{Intersection, Intersects, Sphere};

#[derive(Default, Debug, Clone, Copy, PartialEq, Encode, Decode)]
pub struct AABB
{
    pub min: Vec3,
    pub max: Vec3,
}
impl AABB
{
    // convert to functions?
    pub const MIN_MAX: Self = Self { min: Vec3::MIN, max: Vec3::MAX }; // for 'universe' queries
    pub const MAX_MIN: Self = Self { min: Vec3::MAX, max: Vec3::MIN }; // for finding min volume

    #[inline] #[must_use] pub const fn new(min: Vec3, max: Vec3) -> Self { Self { min, max } }
    #[inline] #[must_use] pub const fn empty() -> Self { Self { min: Vec3::ZERO, max: Vec3::ZERO } }

    #[inline] #[must_use] pub fn size(self) -> Vec3 { self.max - self.min }
    #[inline] #[must_use] pub fn half(self) -> Vec3 { (self.max - self.min) / 2.0 }
    #[inline] #[must_use]
    pub fn volume(self) -> f32
    {
        let size = self.size();
        size.x * size.y * size.z
    }

    #[inline] #[must_use]
    pub fn surface_area(self) -> f32
    {
        let size = self.size();
        return 2.0 * (size.x * size.y + size.y * size.z + size.z * size.x);
    }

    #[inline] #[must_use] pub fn center(self) -> Vec3 { (self.min + self.max) / 2.0 }

    #[inline] #[must_use]
    pub fn max_axis(self) -> f32
    {
        let size = self.size();
        size.x.max(size.y.max(size.z))
    }

    #[inline]
    pub fn union_with(&mut self, other: Self)
    {
        *self = self.unioned_with(other);
    }

    // better name?
    #[inline] #[must_use]
    pub fn unioned_with(self, rhs: Self) -> Self
    {
        Self
        {
            min: self.min.min(rhs.min),
            max: self.max.max(rhs.max),
        }
    }
    
    pub fn scale(self, amount_frac: f32) -> Self
    {
        let scaled = (self.size() * amount_frac) / 2.0;
        Self
        {
            min: self.min - scaled,
            max: self.max + scaled,
        }
    }

    #[must_use]
    pub fn fully_contains(self, rhs: Self) -> bool
    {
        self.min.cmple(rhs.min).all() &&
        self.max.cmpge(rhs.max).all()
    }

    #[must_use]
    pub fn overlaps(self, rhs: Self) -> bool
    {
        self.min.cmple(rhs.max).all() &&
        self.max.cmpge(rhs.min).all()
    }
}
impl Intersects<AABB> for AABB
{
    fn get_intersection(&self, other: AABB) -> Intersection
    {
        todo!()
    }
}
// todo: proper shapes library?

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn empty()
    {
        let aabb = AABB::default();
        assert_eq!(aabb.size(), Vec3::ZERO);
        assert_eq!(aabb.center(), Vec3::ZERO);
        assert_eq!(aabb.volume(), 0.0);
        assert_eq!(aabb.surface_area(), 0.0);
    }

    #[test]
    fn sizes()
    {
        let aabb = AABB::new(Vec3::splat(-2.0), Vec3::splat(2.0));
        assert_eq!(aabb.size(), Vec3::splat(4.0));
        assert_eq!(aabb.center(), Vec3::ZERO);
        assert_eq!(aabb.volume(), 4.0f32.powi(3));
        assert_eq!(aabb.surface_area(), 4.0 * 4.0 * 6.0);
    }

    #[test]
    fn max_axis()
    {
        let aabb = AABB::new(Vec3::ZERO, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(aabb.max_axis(), 3.0);
    }
    
    #[test]
    fn union()
    {
        let a = AABB::new(Vec3::ZERO, Vec3::new(1.0, 5.0, 3.0));
        let b = AABB::new(Vec3::ONE, Vec3::new(2.0, 3.0, 4.0));
        
        assert_eq!(a.unioned_with(b), AABB::new(Vec3::ZERO, Vec3::new(2.0, 5.0, 4.0)));

        let mut c = AABB::empty();
        c.union_with(a);
        assert_eq!(c, a);
    }

    #[test]
    fn fully_contains()
    {
        let inner = AABB::new(Vec3::ONE, Vec3::splat(3.0));
        let outer = AABB::new(Vec3::ZERO, Vec3::splat(4.0));
        assert!(outer.fully_contains(inner));
        assert!(!inner.fully_contains(outer));

        // touching edges
        let inner = outer;
        assert!(outer.fully_contains(inner));
        assert!(inner.fully_contains(outer));

        // overlap
        let inner = AABB::new(Vec3::ONE, Vec3::splat(5.0));
        assert!(!outer.fully_contains(inner));
        assert!(!inner.fully_contains(outer));

        // no overlap
        let inner = AABB::new(Vec3::splat(10.0), Vec3::splat(15.0));
        assert!(!outer.fully_contains(inner));
        assert!(!inner.fully_contains(outer));
    }

    #[test]
    fn overlaps()
    {
        let a = AABB::new(Vec3::ONE, Vec3::splat(3.0));
        let b = AABB::new(Vec3::ZERO, Vec3::splat(4.0));
        assert!(a.overlaps(b));
        assert!(b.overlaps(a));

        // partial overlap
        let a = AABB::new(Vec3::ONE, Vec3::splat(5.0));
        assert!(a.overlaps(b));
        assert!(b.overlaps(a));

        // touching edges
        let b = a;
        assert!(a.overlaps(b));
        assert!(b.overlaps(a));

        // no overlap
        let b = AABB::new(Vec3::splat(10.0), Vec3::splat(15.0));
        assert!(!a.overlaps(b));
        assert!(!b.overlaps(a));
    }

    #[test]
    fn asdf()
    {
        // testing some stuff here

        let a = AABB::new(Vec3::splat(1.0), Vec3::splat(4.0));
        let b = AABB::new(Vec3::splat(5.0), Vec3::splat(6.0));

        let c = AABB::new(Vec3::splat(3.0), Vec3::splat(4.0));
        let d = AABB::new(Vec3::splat(1.0), Vec3::splat(16.0));

        let da = a.max - a.min; let sa = a.min + a.max;
        let db = b.max - b.min; let sb = b.min + b.max;
        let dc = c.max - c.min; let sc = c.min + c.max;
        let dd = d.max - d.min; let sd = d.min + d.max;

        println!("{a:?} - {} {} {}", da, sa, sa / da);
        println!("{b:?} - {} {} {}", db, sb, db / sb);
        println!("{c:?} - {} {} {}", dc, sc, dc / sc);
        println!("{d:?} - {} {} {}", dd, sd, dd / sd);
    }
}