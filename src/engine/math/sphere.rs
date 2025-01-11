use bitcode::{Decode, Encode};
use glam::{Mat3, Mat4, Vec3, Vec4, Vec4Swizzles};
use rand::seq::SliceRandom;
use crate::engine::math::{Intersection, Intersects, IsOnOrInside, Plane};
// todo: union { struct { center, radius }, simd }?

#[derive(Default, Debug, Clone, Copy, PartialEq, Encode, Decode)]
pub struct Sphere(pub Vec4);
impl Sphere
{
    pub const ZERO: Self = Self(Vec4::ZERO);
    pub fn new(center: Vec3, radius: f32) -> Self
    {
        Self(Vec4::new(center.x, center.y, center.z, radius))
    }

    pub fn center(&self) -> Vec3 { self.0.xyz() }
    pub fn radius(&self) -> f32 { self.0.w }
    pub fn radius_sq(&self) -> f32 { self.0.w * self.0.w }

    pub fn expanded(self, add_radius: f32) -> Self
    {
        Self(Vec4::new(self.0.x, self.0.y, self.0.z, self.0.w + add_radius))
    }

    // Calculate a bounding sphere from a set of points, using Welzl's algorithm
    pub fn from_points(points: &[Vec3]) -> Self
    {
        fn from_two_points(a: Vec3, b: Vec3) -> Sphere
        {
            Sphere::new((a + b) / 2.0, a.distance(b) / 2.0)
        }

        fn from_three_points(a: Vec3, b: Vec3, c: Vec3) -> Sphere
        {
            // https://en.wikipedia.org/wiki/Circumcircle

            let u = a - c;
            let v = b - c;

            let uu = u.length_squared();
            let vv = v.length_squared();

            // let uxv2 = uu * vv - u.dot(v).powi(2);
            let uxv = u.cross(v);

            // points are collinear, calculate from the two outermost points
            if uxv == Vec3::ZERO
            {
                let w = a - b;
                let ul = u.length_squared();
                let vl = v.length_squared();
                let wl = w.length_squared();

                return
                    if ul > vl
                    {
                        if wl > ul { from_two_points(a, b) }
                        else { from_two_points(a, c) }
                    }
                    else if wl > vl { from_two_points(a, b) }
                    else { from_two_points(b, c) }
            }


            let center = (uu * v - vv * u).cross(uxv) / (2.0 * uxv.length_squared());
            // let radius = (uu.sqrt() * vv.sqrt() * (u - v).length()) / (2.0 * uxv.length());
            let radius = center.distance(a);

            // dot product version works in all(?) dimensions
            // let udv = u.dot(v);
            // let uuvv = uu * vv;
            //
            // let n = (uuvv * (u + v)) - (udv * (uu * v + vv * u));
            // let d = (uuvv - udv * udv);
            //
            // let center = (n / (2.0 * d)) + c;
            // let radius = (uu.sqrt() * vv.sqrt() * (u - v).length()) / (2.0 * d.sqrt());

            Sphere::new(center, radius)
        }

        fn from_four_points(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> Sphere
        {
            let u = b - a;
            let v = c - a;
            let w = d - a;

            let matrix = Mat3
            {
                x_axis: u,
                y_axis: v,
                z_axis: w,
            };
            let det = matrix.determinant();
            if det == 0.0
            {
                todo!("Points are coplanar");
            }

            let recip2Vol = 0.5 / det;
            let center = a + recip2Vol * matrix.mul_vec3(Vec3::new(u.length_squared(), v.length_squared(), w.length_squared()));

            //Once we know the center, the radius is clearly the distance to any vertex
            let radius = (center - a).length();
            Sphere::new(center, radius)
        }

        // todo: convert to stack impl
        fn find_recursive(points: &[Vec3], remaining_points: &mut Vec<usize>, boundary_points: &mut Vec<usize>) -> Sphere
        {
            // Dimensions + 1 boundary points
            if remaining_points.is_empty() || boundary_points.len() == 4
            {
                return match boundary_points.len()
                {
                    0 => Sphere(Vec4::ZERO),
                    1 => Sphere::new(points[boundary_points[0]], 0.0),
                    2 => from_two_points(points[boundary_points[0]], points[boundary_points[1]]),
                    3 => from_three_points(points[boundary_points[0]], points[boundary_points[1]], points[boundary_points[2]]),
                    4 => from_four_points(points[boundary_points[0]], points[boundary_points[1]], points[boundary_points[2]], points[boundary_points[3]]),
                    _ => unreachable!("Too many points to create bounding sphere?")
                }
            }

            let test_index = 0; // up front
            let test_point = remaining_points.swap_remove(test_index);

            let smallest = find_recursive(points, remaining_points, boundary_points);

            if let Intersection::Overlapping = smallest.get_intersection(points[test_point])
            {
                return smallest;
            }

            boundary_points.push(test_point);
            find_recursive(points, remaining_points, boundary_points)
        }

        let mut remaining_points: Vec::<usize> = (0..points.len()).collect();
        remaining_points.shuffle(&mut rand::thread_rng());
        let mut boundary_points = Vec::new();
        find_recursive(points, &mut remaining_points, &mut boundary_points)
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
        if dist == self.radius_sq() // approx eq?
        {
            Intersection::EdgesTouching
        }
        else if dist < self.radius_sq()
        {
            Intersection::Overlapping
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
        if dist == rr // approx eq?
        {
            Intersection::EdgesTouching
        }
        else if dist < rr
        {
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
    fn is_on_or_inside(&self, other: Vec3) -> bool
    {
        let dist = self.center().distance(other);
        dist <= self.radius()
    }
}
impl IsOnOrInside<Sphere> for Sphere
{
    fn is_on_or_inside(&self, other: Sphere) -> bool
    {
        let dist = self.center().distance_squared(other.center());
        let rr = (self.radius() + other.radius()).powi(2);
        dist <= rr
    }
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
        let dist_sq = self.center().distance_squared(other.center());
        if dist_sq <= (self.radius() - other.radius()).powi(2)
        {
            return;
        }

        let dist = dist_sq.sqrt();
        let new_radius = (dist + self.radius() + other.radius()) / 2.0;
        let new_center = self.center() + ((new_radius - self.radius()) / dist) * (other.center() - self.center());
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
        assert_eq!(sphere.radius_sq(), 5.0 * 5.0);
    }

    #[test]
    fn math()
    {
        let sphere = Sphere::new(Vec3::new(0.0, 2.0, 0.0), 5.0);
        assert_eq!(sphere.center(), Vec3::new(0.0, 2.0, 0.0));
        assert_eq!(sphere.radius(), 5.0);

        let sphere = sphere + Vec3::new(1.0, 0.0, 0.0);
        assert_eq!(sphere.center(), Vec3::new(1.0, 2.0, 0.0));
        assert_eq!(sphere.radius(), 5.0);

        let sphere = sphere - Vec3::new(4.0, 0.0, 0.0);
        assert_eq!(sphere.center(), Vec3::new(-3.0, 2.0, 0.0));
        assert_eq!(sphere.radius(), 5.0);

        let sphere = sphere.expanded(3.0);
        assert_eq!(sphere.center(), Vec3::new(-3.0, 2.0, 0.0));
        assert_eq!(sphere.radius(), 8.0);
    }

    #[test]
    fn expanding_sphere()
    {
        let mut sphere = Sphere::new(Vec3::new(0.0, 2.0, 0.0), 5.0);
        sphere += Sphere::new(Vec3::new(0.0, 5.0, 0.0), 2.0);
        assert_eq!(sphere.center(), Vec3::new(0.0, 2.0, 0.0));
        assert_eq!(sphere.radius(), 5.0);

        sphere += Sphere::new(Vec3::new(0.0, 7.0, 0.0), 2.0);
        assert_eq!(sphere.center(), Vec3::new(0.0, 3.0, 0.0));
        assert_eq!(sphere.radius(), 6.0);
    }

    #[test]
    fn point_intersections()
    {
        let sphere = Sphere::new(Vec3::new(0.0, 2.0, 0.0), 5.0);

        assert_eq!(sphere.get_intersection(Vec3::ZERO), Intersection::Overlapping);
        assert!(sphere.is_on_or_inside(Vec3::ZERO));

        assert_eq!(sphere.get_intersection(Vec3::new(0.0, 7.0, 0.0)), Intersection::EdgesTouching);
        assert!(sphere.is_on_or_inside(Vec3::new(0.0, 7.0, 0.0)));

        assert_eq!(sphere.get_intersection(Vec3::new(0.0, 10.0, 0.0)), Intersection::None);
        assert!(!sphere.is_on_or_inside(Vec3::new(0.0, 10.0, 0.0)));
    }

    #[test]
    fn sphere_intersections()
    {
        let sphere = Sphere::new(Vec3::new(0.0, 2.0, 0.0), 5.0);

        let test = Sphere::new(Vec3::new(0.0, 4.0, 0.0), 3.0);
        assert_eq!(sphere.get_intersection(test), Intersection::Overlapping);
        assert!(sphere.is_on_or_inside(test));

        let test = Sphere::new(Vec3::new(0.0, 10.0, 0.0), 3.0);
        assert_eq!(sphere.get_intersection(test), Intersection::EdgesTouching);
        assert!(sphere.is_on_or_inside(test));

        let test = Sphere::new(Vec3::new(0.0, 100.0,0.0), 3.0);
        assert_eq!(sphere.get_intersection(test), Intersection::None);
        assert!(!sphere.is_on_or_inside(test));
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
            let sphere = Sphere::from_points(&[
                Vec3::new(0.0, 0.0, 0.0),
            ]);
            assert_eq!(sphere.center(), Vec3::new(0.0, 0.0, 0.0));
            assert_eq!(sphere.radius(), 0.0);
        }

        #[test]
        fn from_2_points()
        {
            let sphere = Sphere::from_points(&[
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(0.0, 4.0, 0.0),
            ]);
            assert_eq!(sphere.center(), Vec3::new(0.0, 2.0, 0.0));
            assert_eq!(sphere.radius(), 2.0);
        }

        #[test]
        fn from_3_points()
        {
            let sphere = Sphere::from_points(&[
                Vec3::new(-2.0, 0.0, 0.0),
                Vec3::new(0.0, 2.0, 0.0),
                Vec3::new(4.0, 0.0, 0.0),
            ]);
            assert_eq!(sphere.center(), Vec3::new(1.0, -1.0, 0.0));
            assert_relative_eq!(sphere.radius(), 3.1622775);
        }

        #[test]
        fn from_4_points()
        {
            let sphere = Sphere::from_points(&[
                Vec3::new(0.0, -2.0, 0.0),
                Vec3::new(0.0, 2.0, -4.0),
                Vec3::new(4.0, 2.0, 0.0),
                Vec3::new(0.0, 2.0, 4.0),
            ]);
            assert_eq!(sphere.center(), Vec3::new(0.0, 0.0, 2.0));
            assert_relative_eq!(sphere.radius(), 5.39);
        }
    }
}