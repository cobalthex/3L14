use bitcode::{Decode, Encode};
use glam::{Mat4, Vec3, Vec3A};
use crate::{Facing, GetFacing, Intersection, Intersects, Plane, Sphere};

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

    #[inline] #[must_use] pub const fn empty() -> Self { Self { min: Vec3::ZERO, max: Vec3::ZERO } }
    #[inline] #[must_use] pub const fn new(min: Vec3, max: Vec3) -> Self { Self { min, max } }
    #[inline] #[must_use] pub const fn around_point(centroid: Vec3, half_size: Vec3) -> Self
    {
        Self
        {
            min: Vec3::new(centroid.x - half_size.x,centroid.y - half_size.y, centroid.z - half_size.z),
            max: Vec3::new(centroid.x + half_size.x,centroid.y + half_size.y, centroid.z + half_size.z),
        }
    }
    // Create an AABB around two unordered corners
    #[inline] #[must_use]
    pub fn from_points(a: Vec3, b: Vec3) -> Self
    {
        Self
        {
            min: Vec3::min(a, b),
            max: Vec3::max(a, b)
        }
    }

    #[inline] #[must_use] pub fn size(self) -> Vec3 { self.max - self.min }
    #[inline] #[must_use] pub fn half_size(self) -> Vec3 { (self.max - self.min) * 0.5 }
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

    #[inline] #[must_use] pub fn centroid(self) -> Vec3 { (self.min + self.max) * 0.5 }

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

    pub fn scale(&mut self, amount_frac: f32)
    {
        let centroid = self.centroid();
        let half = self.half_size() * amount_frac;
        self.min = centroid - half;
        self.max = centroid + half;
    }
    #[must_use]
    pub fn scaled(self, amount_frac: f32) -> Self
    {
        let centroid = self.centroid();
        let half = self.half_size() * amount_frac;
        Self
        {
            min: centroid - half,
            max: centroid + half,
        }
    }

    // Transform this AABB by a transform matrix. Do not use this with perspective transforms
    pub fn affine_transform(&mut self, transform: Mat4)
    {
        let centroid = self.centroid();
        let half = self.half_size();
        let new_center = transform.transform_point3(centroid);
        let new_half
            = transform.x_axis.abs() * half.y
            + transform.y_axis.abs() * half.y
            + transform.z_axis.abs() * half.z;
        self.min = new_center - new_half.truncate();
        self.max = new_center + new_half.truncate();
    }

    // Transform this AABB by a transform matrix, allowing for perspective transforms
    fn transform_slow(&mut self, transform: Mat4)
    {
        let mut new_min = Vec3A::splat(f32::INFINITY);
        let mut new_max = Vec3A::splat(f32::NEG_INFINITY);

        for &x in &[self.min.x, self.max.x]
        {
            for &y in &[self.min.y, self.max.y]
            {
                for &z in &[self.min.z, self.max.z]
                {
                    let corner = Vec3A::new(x, y, z);
                    let transformed = transform * corner.extend(1.0);

                    let point = Vec3A::new(
                        transformed.x,
                        transformed.y,
                        transformed.z,
                    );

                    new_min = new_min.min(point);
                    new_max = new_max.max(point);
                }
            }
        }

        self.min = new_min.into();
        self.max = new_max.into();
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

    /// Classification of a box against a single plane using the p-vertex trick.
    /// Returns signed distance of the near-face-to-plane extents:
    ///   > 0: fully inside (positive half-space)
    ///   < 0: fully outside
    ///     =: overlap otherwise (straddling)
    /// and radius, the projected half-extent onto the plane normal
    #[inline] #[must_use]
    pub fn test_plane(&self, plane: Plane) -> (f32, f32)
    {
        // todo: get_facing() instead ?
        let signed_dist = plane.signed_distance_to(Vec3A::from(self.centroid()));
        let radius = Vec3A::from(self.half_size()).dot(plane.normal().abs());
        (signed_dist, radius)
    }
}
impl Intersects<AABB> for AABB
{
    fn get_intersection(&self, _other: AABB) -> Intersection
    {
        todo!()
    }
}
impl Intersects<Sphere> for AABB
{
    fn get_intersection(&self, _other: Sphere) -> Intersection
    {
        //let min_t = f32::INFINITY;
        //let max_t = f32::NEG_INFINITY;
        todo!()
    }
}
impl GetFacing<Plane> for AABB
{
    // TODO: test
    fn get_facing(&self, other: Plane) -> Facing
    {
        let (dist, radius) = self.test_plane(other);
        if dist + radius < 0.0
        {
            Facing::Behind
        }
        else if dist - radius >= 0.0
        {
            Facing::InFront
        }
        else
        {
            Facing::On
        }
    }
}
// TODO: raycast

// todo: proper shapes library?

#[cfg(test)]
mod tests
{
    use std::assert_matches;
    use glam::Quat;
    use super::*;

    #[test]
    fn empty()
    {
        let aabb = AABB::default();
        assert_eq!(aabb.size(), Vec3::ZERO);
        assert_eq!(aabb.centroid(), Vec3::ZERO);
        assert_eq!(aabb.volume(), 0.0);
        assert_eq!(aabb.surface_area(), 0.0);
    }

    #[test]
    fn sizes()
    {
        let aabb = AABB::new(Vec3::splat(-2.0), Vec3::splat(2.0));
        assert_eq!(aabb.size(), Vec3::splat(4.0));
        assert_eq!(aabb.centroid(), Vec3::ZERO);
        assert_eq!(aabb.volume(), 4.0f32.powi(3));
        assert_eq!(aabb.surface_area(), 4.0 * 4.0 * 6.0);

        let aabb = AABB::around_point(Vec3::splat(0.0), Vec3::splat(2.0));
        assert_eq!(aabb.size(), Vec3::splat(4.0));
        assert_eq!(aabb.centroid(), Vec3::ZERO);
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
    fn get_facing()
    {
        let p = Plane::from_point_normal(Vec3A::ZERO, Vec3A::new(0.0, 1.0, 0.0));

        let a = AABB::around_point(Vec3::new(0.0, 4.0, 0.0), Vec3::splat(3.0));
        assert_matches!(a.get_facing(p), Facing::InFront);

        let a = AABB::around_point(Vec3::new(0.0, 0.0, 0.0), Vec3::splat(3.0));
        assert_matches!(a.get_facing(p), Facing::On);

        let a = AABB::around_point(Vec3::new(0.0, -4.0, 0.0), Vec3::splat(3.0));
        assert_matches!(a.get_facing(p), Facing::Behind);
    }

    #[test]
    fn test_plane()
    {
        let p = Plane::from_point_normal(Vec3A::ZERO, Vec3A::new(0.0, 1.0, 0.0));

        let a = AABB::around_point(Vec3::new(0.0, 4.0, 0.0), Vec3::splat(3.0));
        assert_matches!(a.test_plane(p), (4.0, 3.0));

        let a = AABB::around_point(Vec3::new(0.0, 0.0, 0.0), Vec3::splat(3.0));
        assert_matches!(a.test_plane(p), (0.0, 3.0));

        let a = AABB::around_point(Vec3::new(0.0, -4.0, 0.0), Vec3::splat(3.0));
        assert_matches!(a.test_plane(p), (-4.0, 3.0));
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

    #[test]
    fn affine_transform()
    {
        let aabb = AABB::new(
            Vec3::new(-1.0, -2.0, -0.5),
            Vec3::new(2.0, 1.0, 3.0)
        );

        let transform = Mat4::from_scale_rotation_translation(
            Vec3::new(2.0, 0.5, 1.5),
            Quat::from_rotation_z(0.7),
            Vec3::new(10.0, -4.0, 3.0),
        );

        let mut optimized = aabb.clone();
        optimized.affine_transform(transform);
        let mut reference = aabb.clone();
        reference.transform_slow(transform);

        let epsilon = 1e-5;

        assert!(
            (optimized.min - reference.min).abs().max_element() < epsilon,
            "minimum mismatch: optimized={:?}, reference={:?}",
            optimized.min,
            reference.min
        );

        assert!(
            (optimized.max - reference.max).abs().max_element() < epsilon,
            "maximum mismatch: optimized={:?}, reference={:?}",
            optimized.max,
            reference.max
        );
    }
}