use std::fmt::{Debug, Formatter};
use bitcode::{Decode, Encode};
use glam::{Mat4, Vec3, Vec4, Vec4Swizzles};
use crate::{CenterDistance, Intersection, Intersects, IsOnOrInside};
use nab_3l14::utils::ShortTypeName;
// todo: union { struct { center, radius }, simd }?

#[derive(Default, Clone, Copy, PartialEq, Encode, Decode)]
pub struct Sphere(pub Vec4);
impl Sphere
{
    pub const EMPTY: Self = Self(Vec4::ZERO);

    #[inline] #[must_use] pub fn new(center: Vec3, radius: f32) -> Self
    {
        Self(Vec4::new(center.x, center.y, center.z, radius))
    }

    #[inline] #[must_use] pub fn center(&self) -> Vec3 { self.0.xyz() }
    #[inline] #[must_use] pub fn radius(&self) -> f32 { self.0.w }
    #[inline] #[must_use] pub fn radius_squared(&self) -> f32 { self.0.w * self.0.w }

    #[inline] #[must_use]
    pub fn expanded(self, add_radius: f32) -> Self
    {
        Self(Vec4::new(self.0.x, self.0.y, self.0.z, self.0.w + add_radius))
    }

    #[must_use]
    pub fn transform(self, transform: &Mat4) -> Self
    {
        let center = transform.transform_point3(self.center());
        let scale = transform.x_axis.x.max(transform.y_axis.y).max(transform.z_axis.z);
        Self::new(center, self.radius() * scale)
    }

    // outer_distance/sq

    #[must_use]
    fn from_two_points(a: Vec3, b: Vec3) -> Self
    {
        Self::new((a + b) / 2.0, a.distance(b) / 2.0)
    }

    #[must_use]
    fn from_three_points(a: Vec3, b: Vec3, c: Vec3) -> Self
    {
        // https://en.wikipedia.org/wiki/Circumcircle

        let ab = b - a;
        let ac = c - a;

        let ab_len_sq = ab.length_squared();
        let ac_len_sq = ac.length_squared();

        let ab_x_ac = ab.cross(ac); // calculate the normal to the plane defined by the two lines

        // points are collinear, calculate from the two outermost points
        if ab_x_ac == Vec3::ZERO // todo: abs < epsilon?
        {
            let bc = c - b;
            let bc_len_sq = bc.length_squared();

            return
            {
                if ab_len_sq > ac_len_sq
                {
                    if bc_len_sq > ab_len_sq { Self::from_two_points(b, c) }
                    else { Self::from_two_points(a, b) }
                }
                else if bc_len_sq > ac_len_sq { Self::from_two_points(b, c) }
                else { Self::from_two_points(a, c) }
            };
        }

        let num = (ab_len_sq * ac - ac_len_sq * ab).cross(ab_x_ac);
        let den = 2.0 * ab_x_ac.length_squared();

        let center = a + (num / den);
        let radius = center.distance(a);

        Self::new(center, radius)
    }

    #[must_use]
    fn from_four_points(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> Self
    {
        // given 3x3 col matrix [ab ac ad]
        // calculate the adjugate matrix adj(M) = det(M)*inv(M) , also known as [(ac x ad), (ad x ab), (ab x ac)]
        // adj(M) = C^T (cofactor matrix)
        // numerator = adj * weights = adj(M) * [||ac||^2, ||ad||^2, ||ab||^2]^T
        // denominator = 2 * det(M)

        let ab = b - a;
        let ac = c - a;
        let ad = d - a;

        let ab_x_ac = ab.cross(ac);

        let det = ad.dot(ab_x_ac);
        // collinear or coplanar
        if det.abs() < f32::EPSILON
        {
            // TODO: collinear?

            // coplanar: find the pair of points that are farthest apart
            let mut max_dist = 0.0;
            let mut best_pair = (a, b);

            let points = [a, b, c, d];
            for i in 0..4
            {
                for j in (i + 1)..4
                {
                    let dist = Vec3::distance_squared (points[i], points[j]);
                    if dist > max_dist
                    {
                        max_dist = dist;
                        best_pair = (points[i], points[j]);
                    }
                }
            }
            return Self::new((best_pair.0 + best_pair.1) / 2.0, max_dist.sqrt() / 2.0);
        }

        let ad_x_ab = ad.cross(ab);
        let ac_x_ad = ac.cross(ad);

        let ab_len_sq = ab.length_squared();
        let ac_len_sq = ac.length_squared();
        let ad_len_sq = ad.length_squared();

        let d = ((ad_len_sq * ab_x_ac) + (ac_len_sq * ad_x_ab) + (ab_len_sq * ac_x_ad)) / (2.0 * det);
        Self::new(a + d, d.length())
    }

    // Calculate a bounding sphere from a set of points, using Welzl's algorithm
    // TODO: use EPOS algo (faster)
    #[must_use]
    pub fn from_points(points: &[Vec3]) -> Self
    {
        // todo: convert to iterative
        fn find_recursive(points: &[Vec3], mut remaining_points: Vec<usize>, boundary_points: &mut Vec<usize>) -> Sphere
        {
            // Dimensions + 1 boundary points
            if remaining_points.is_empty() || boundary_points.len() == 4
            {
                return match boundary_points.len()
                {
                    0 => Sphere(Vec4::ZERO),
                    1 => Sphere::new(points[boundary_points[0]], 0.0),
                    2 => Sphere::from_two_points(points[boundary_points[0]], points[boundary_points[1]]),
                    3 => Sphere::from_three_points(points[boundary_points[0]], points[boundary_points[1]], points[boundary_points[2]]),
                    4 => Sphere::from_four_points(points[boundary_points[0]], points[boundary_points[1]], points[boundary_points[2]], points[boundary_points[3]]),
                    _ => unreachable!("Too many points to create a trivial bounding sphere")
                };
            }

            let test_index = 0;// rand::random();
            let test_point = remaining_points.swap_remove(test_index);

            let smallest = find_recursive(points, remaining_points.clone(), boundary_points);

            if smallest.rhs_is_on_or_inside(points[test_point])
            {
                return smallest;
            }

            boundary_points.push(test_point);
            find_recursive(points, remaining_points, boundary_points)
        }

        let remaining_points = (0..points.len()).collect();
        let mut boundary_points = Vec::new();
        find_recursive(points, remaining_points, &mut boundary_points)
    }
}
impl Debug for Sphere
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        f.debug_struct(Self::short_type_name())
            .field("center", &self.center())
            .field("radius", &self.radius())
            .finish()
    }
}
impl From<Vec4> for Sphere
{
    fn from(value: Vec4) -> Self
    {
        Self(value)
    }
}
impl Intersects<Vec3> for Sphere
{
    fn get_intersection(&self, other: Vec3) -> Intersection
    {
        let dist = self.center().distance_squared(other);
        if dist <= self.radius_squared()
        {
            Intersection::FullyContained
        }
        else
        {
            Intersection::None
        }
    }
}
impl Intersects<Sphere> for Sphere
{
    fn get_intersection(&self, other: Sphere) -> Intersection
    {
        let dist = self.center().distance_squared(other.center());
        let rr = (self.radius() + other.radius()).powi(2);
        if dist <= rr // approx eq?
        {
            // TODO: fully contained
            Intersection::Overlapping
        }
        else
        {
            Intersection::None
        }
    }
}
impl IsOnOrInside<Vec3> for Sphere
{
    fn rhs_is_on_or_inside(&self, other: Vec3) -> bool
    {
        let dist = self.center().distance(other);
        dist <= self.radius()
    }
}
impl IsOnOrInside<Sphere> for Sphere
{
    fn rhs_is_on_or_inside(&self, other: Sphere) -> bool
    {
        let dist = self.center().distance_squared(other.center());
        let rr = (self.radius() + other.radius()).powi(2);
        dist <= rr
    }
}
impl CenterDistance<Sphere> for Sphere
{
    fn center_distance_sq(&self, other: Sphere) -> f32 { self.center().distance_squared(other.center()) }
}
impl CenterDistance<Vec3> for Sphere
{
    fn center_distance_sq(&self, other: Vec3) -> f32 { self.center().distance_squared(other) }
}
// add/sub assign?
impl std::ops::Add<Vec3> for Sphere
{
    type Output = Sphere;
    fn add(self, other: Vec3) -> Sphere
    {
        Sphere(self.0 + Vec4::from((other, 0.0)))
    }
}
impl std::ops::Sub<Vec3> for Sphere
{
    type Output = Sphere;
    fn sub(self, other: Vec3) -> Sphere
    {
        Sphere(self.0 - Vec4::from((other, 0.0)))
    }
}
impl std::ops::Add<Sphere> for Sphere
{
    type Output = Sphere;
    fn add(mut self, rhs: Sphere) -> Self::Output { self += rhs; self }
}
impl std::ops::AddAssign<Sphere> for Sphere
{
    // Note: Combining multiple spheres can become over-sized due to the 'greedy' nature of this algorithm (geometric iterative expansion)
    fn add_assign(&mut self, other: Sphere)
    {
        let dist = self.center().distance(other.center());
        if dist + other.radius() < self.radius()
        {
            return;
        }
        if dist + self.radius() < other.radius()
        {
            self.0 = other.0;
            return;
        }

        let new_radius = (dist + self.radius() + other.radius()) / 2.0;
        let new_center = self.center() + (other.center() - self.center()) * (new_radius - self.radius()) / dist;
        self.0 = Vec4::from((new_center, new_radius));
    }
}

#[cfg(test)]
mod tests
{
    use approx::assert_relative_eq;
    use super::*;

    #[test]
    fn basics()
    {
        let sphere = Sphere::new(Vec3::new(0.0, 2.0, 0.0), 5.0);
        assert_eq!(sphere.center(), Vec3::new(0.0, 2.0, 0.0));
        assert_eq!(sphere.radius(), 5.0);
        assert_eq!(sphere.radius_squared(), 5.0 * 5.0);
    }

    #[test]
    fn math()
    {
        let sphere_a = Sphere::new(Vec3::new(0.0, 2.0, 0.0), 5.0);
        assert_eq!(sphere_a.center(), Vec3::new(0.0, 2.0, 0.0));
        assert_eq!(sphere_a.radius(), 5.0);

        let sphere_b = sphere_a + Vec3::new(1.0, 0.0, 0.0);
        assert_eq!(sphere_b.center(), Vec3::new(1.0, 2.0, 0.0));
        assert_eq!(sphere_b.radius(), 5.0);

        let sphere_c = sphere_b - Vec3::new(4.0, 0.0, 0.0);
        assert_eq!(sphere_c.center(), Vec3::new(-3.0, 2.0, 0.0));
        assert_eq!(sphere_c.radius(), 5.0);

        let sphere_d = sphere_c.expanded(3.0);
        assert_eq!(sphere_d.center(), Vec3::new(-3.0, 2.0, 0.0));
        assert_eq!(sphere_d.radius(), 8.0);
    }

    #[test]
    fn expanding_sphere()
    {
        let mut sphere_a = Sphere::EMPTY;
        sphere_a += Sphere::new(Vec3::new(0.0, 2.0, 0.0), 5.0);
        assert_eq!(sphere_a.center(), Vec3::new(0.0, 2.0, 0.0));
        assert_eq!(sphere_a.radius(), 5.0);

        let mut sphere_b = Sphere::new(Vec3::new(0.0, 2.0, 0.0), 5.0);
        sphere_b += Sphere::new(Vec3::new(0.0, 5.0, 0.0), 2.0);
        assert_eq!(sphere_b.center(), Vec3::new(0.0, 2.0, 0.0));
        assert_eq!(sphere_b.radius(), 5.0);

        sphere_b += Sphere::new(Vec3::new(0.0, 7.0, 0.0), 2.0);
        assert_eq!(sphere_b.center(), Vec3::new(0.0, 3.0, 0.0));
        assert_eq!(sphere_b.radius(), 6.0);
    }

    #[test]
    fn point_intersections()
    {
        let sphere = Sphere::new(Vec3::new(0.0, 2.0, 0.0), 5.0);

        assert_eq!(sphere.get_intersection(Vec3::ZERO), Intersection::Overlapping);
        assert!(sphere.rhs_is_on_or_inside(Vec3::ZERO));

        assert_eq!(sphere.get_intersection(Vec3::new(0.0, 7.0, 0.0)), Intersection::Overlapping);
        assert!(sphere.rhs_is_on_or_inside(Vec3::new(0.0, 7.0, 0.0)));

        assert_eq!(sphere.get_intersection(Vec3::new(0.0, 10.0, 0.0)), Intersection::None);
        assert!(!sphere.rhs_is_on_or_inside(Vec3::new(0.0, 10.0, 0.0)));
    }

    #[test]
    fn sphere_intersections()
    {
        let sphere = Sphere::new(Vec3::new(0.0, 2.0, 0.0), 5.0);

        let test_a = Sphere::new(Vec3::new(0.0, 4.0, 0.0), 3.0);
        assert_eq!(sphere.get_intersection(test_a), Intersection::Overlapping);
        assert!(sphere.rhs_is_on_or_inside(test_a));

        let test_b = Sphere::new(Vec3::new(0.0, 10.0, 0.0), 3.0);
        assert_eq!(sphere.get_intersection(test_b), Intersection::Overlapping);
        assert!(sphere.rhs_is_on_or_inside(test_b));

        let test_c = Sphere::new(Vec3::new(0.0, 100.0, 0.0), 3.0);
        assert_eq!(sphere.get_intersection(test_c), Intersection::None);
        assert!(!sphere.rhs_is_on_or_inside(test_c));
    }

    mod from_points
    {
        use super::*;
        
        #[test]
        fn from_0_points()
        {
            let sphere = Sphere::from_points(&[]);
            assert_eq!(sphere.center(), Vec3::new(0.0, 0.0, 0.0));
            assert_eq!(sphere.radius(), 0.0);
        }

        #[test]
        fn from_1_point()
        {
            let points = [
                Vec3::new(0.0, 0.0, 0.0),
            ];
            let sphere = Sphere::from_points(&points);
            assert_eq!(sphere.center(), Vec3::new(0.0, 0.0, 0.0));
            assert_eq!(sphere.radius(), 0.0);
        }

        #[test]
        fn from_2_points()
        {
            let points = [
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(0.0, 4.0, 0.0),
            ];
            let sphere = Sphere::from_points(&points);
            assert_eq!(sphere.center(), Vec3::new(0.0, 2.0, 0.0));
            assert_eq!(sphere.radius(), 2.0);
        }

        #[test]
        fn from_3_points()
        {
            let points = [
                Vec3::new(0.0, 2.0, 0.0),
                Vec3::new(4.0, 0.0, 0.0),
                Vec3::new(-2.0, 0.0, 0.0),
            ];
            // for i in 0..9
            // {
            //     points.shuffle(&mut rand::thread_rng());
            //     println!("{:?} -- {:?}", points, Sphere::from_three_points(points[0], points[1], points[2]));
            // }

            let sphere = Sphere::from_points(&points);
            assert_eq!(sphere.center(), Vec3::new(1.0, -1.0, 0.0));
            assert_relative_eq!(sphere.radius(), 3.1622775);
        }

        #[test]
        fn from_4_points()
        {
            let points = [
                Vec3::new(0.0, -2.0, 0.0),
                Vec3::new(0.0, 2.0, -4.0),
                Vec3::new(4.0, 2.0, 0.0),
                Vec3::new(0.0, 2.0, 4.0),
            ];
            // for i in 0..12
            // {
            //     points.shuffle(&mut rand::thread_rng());
            //     println!("{:?}", Sphere::from_four_points(points[0], points[1], points[2], points[3]));
            // }

            let sphere = Sphere::from_points(&points);
            println!("{sphere:?}");
            assert_relative_eq!(sphere.radius(), 4.0);
            assert_eq!(sphere.center(), Vec3::new(0.0, 2.0, 0.0));
        }
    }
}