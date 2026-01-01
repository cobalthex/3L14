use std::fmt::{Debug, Formatter};
use glam::{Mat4, Vec3};
use crate::{Facing, GetFacing, Intersection, Intersects, IsOnOrInside, Plane, Sphere};
use nab_3l14::utils::ShortTypeName;

#[derive(Clone, PartialEq)]
pub struct Frustum
{
    pub planes: [Plane; 6], // ordered left, right, top, bottom, near, far
}
impl Frustum
{
    pub const NULL: Frustum = Frustum { planes: [Plane::NULL; 6] }; // an invalid frustum acting as a placeholder

    // if input is projection, planes are in view space
    // if view projection, planes are in world space
    // if model view projection, planes are in model space
    #[must_use]
    pub fn from_matrix(col_major_mtx: &Mat4) -> Self
    {
        let rows = col_major_mtx.transpose(); // glam stores in column-major
        let planes =
        [
            // not sure why all of these need to be mirrored...
            Plane::from(rows.w_axis + rows.x_axis).negated_distance().normalized(), // left
            Plane::from(rows.w_axis - rows.x_axis).negated_distance().normalized(), // right
            Plane::from(rows.w_axis - rows.y_axis).negated_distance().normalized(), // top
            Plane::from(rows.w_axis + rows.y_axis).negated_distance().normalized(), // bottom

            Plane::from(rows.z_axis).negated_distance().normalized(), // near
            Plane::from(rows.w_axis - rows.z_axis).negated_distance().normalized(), // far
        ];
        Self { planes }
    }

    #[inline] #[must_use] pub fn left(&self) -> Plane { self.planes[0] }
    #[inline] #[must_use] pub fn right(&self) -> Plane { self.planes[1] }
    #[inline] #[must_use] pub fn top(&self) -> Plane { self.planes[2] }
    #[inline] #[must_use] pub fn bottom(&self) -> Plane { self.planes[3] }
    #[inline] #[must_use] pub fn near(&self) -> Plane { self.planes[4] }
    #[inline] #[must_use] pub fn far(&self) -> Plane { self.planes[5] }

    #[must_use]
    pub fn get_corners(projected_mtx: &Mat4) -> [Vec3; 8]
    {
        // todo: wrong?
        let mtx = projected_mtx.inverse();
        [
            mtx.transform_vector3(Vec3::new(-1.0, -1.0, -1.0)), // near bottom left
            mtx.transform_vector3(Vec3::new( 1.0, -1.0, -1.0)), // near bottom right
            mtx.transform_vector3(Vec3::new(-1.0,  1.0, -1.0)), // near top left
            mtx.transform_vector3(Vec3::new( 1.0,  1.0, -1.0)), // near top right
            mtx.transform_vector3(Vec3::new(-1.0, -1.0,  1.0)), // far bottom left
            mtx.transform_vector3(Vec3::new( 1.0, -1.0,  1.0)), // far bottom right
            mtx.transform_vector3(Vec3::new(-1.0,  1.0,  1.0)), // far top left
            mtx.transform_vector3(Vec3::new( 1.0,  1.0,  1.0)), // far top right
        ]
    }
}
impl Debug for Frustum
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        f.debug_struct(Self::short_type_name())
            .field("left", &self.left())
            .field("right", &self.right())
            .field("top", &self.top())
            .field("bottom", &self.bottom())
            .field("near", &self.near())
            .field("far", &self.far())
            .finish()
    }
}
impl Intersects<Vec3> for Frustum
{
    fn get_intersection(&self, other: Vec3) -> Intersection
    {
        // TODO: radar approach (requires knowing camera point)
        // http://www.lighthouse3d.com/tutorials/view-frustum-culling/radar-approach-testing-points/

        let mut inside = true;
        for p in &self.planes
        {
            inside &= matches!(p.get_facing(other), Facing::Behind);
        }
        match inside
        {
            true => Intersection::Overlapping,
            false => Intersection::None,
        }
    }
}
impl IsOnOrInside<Sphere> for Frustum
{
    fn rhs_is_on_or_inside(&self, other: Sphere) -> bool
    {
        // TODO: simd
        for p in &self.planes
        {
            // planes point outward
            let z = p.get_facing(other);
            match z
            {
                Facing::Behind => { return false },
                Facing::On => {},
                Facing::InFront => {},
            }
        }
        true
    }
}

#[cfg(test)]
mod tests
{
    use glam::Vec3;
    use crate::Angle;
    use super::*;

    #[test]
    fn planes()
    {
        let projection = Mat4::perspective_lh(Angle::PI_OVER_TWO.to_radians(), 1.0, 1.0, 10.0);
        let view = Mat4::look_at_lh(
            Vec3::ZERO,
            Vec3::Z,
            Vec3::Y,
        );
        let view_projection = projection * view;
        let frustum = Frustum::from_matrix(&view_projection);
        let recip_sqrt2 = 1.0 / 2.0_f32.sqrt();

        // TODO: these values are wrong
        let expected_planes = [
            Plane::new(Vec3::new(recip_sqrt2, 0.0, recip_sqrt2), 0.0),
            Plane::new(Vec3::new(-recip_sqrt2, 0.0, recip_sqrt2), 0.0),
            Plane::new(Vec3::new(0.0, -recip_sqrt2, recip_sqrt2), 0.0),
            Plane::new(Vec3::new(0.0, recip_sqrt2, recip_sqrt2), 0.0),
            Plane::new(Vec3::new(0.0, 0.0, 1.0), 1.0),
            Plane::new(Vec3::new(0.0, 0.0, -1.0), -10.0), // TODO: This seems wrong
    ];

        for (i, plane) in frustum.planes.iter().enumerate() {
            let expected = &expected_planes[i];
            assert!(
                plane.normal().abs_diff_eq(expected.normal(), 1e-5),
                "Plane {} normal() mismatch: got {:?}, expected {:?}",
                i,
                plane.normal(),
                expected.normal()
            );
            assert!(
                (plane.distance() - expected.distance()).abs() < 1e-5,
                "Plane {} distance() mismatch: got {}, expected {}",
                i,
                plane.distance(),
                expected.distance()
            );
        }
    }

    #[test]
    fn sphere_inside()
    {
        let projection = Mat4::perspective_lh(Angle::PI_OVER_TWO.to_radians(), 16.0 / 9.0, 0.1, 100.0);
        let view = Mat4::look_at_lh(
            Vec3::new(0.0, 0.0, -10.0),
            Vec3::Z,
            Vec3::Y,
        );
        let view_projection = projection * view;
        let frustum = Frustum::from_matrix(&view_projection);

        let sphere = Sphere::new(Vec3::new(0.0, 0.0, 0.0), 1.0);

        println!("{:?}", frustum.planes);
        assert!(frustum.rhs_is_on_or_inside(sphere));
    }

    // TODO: corners
}