use std::fmt::{Debug, Formatter};
use glam::{Mat4, Vec3};
use crate::math::{Facing, GetFacing, Intersection, Intersects, Plane};
use crate::utils::ShortTypeName;

#[derive(Clone, PartialEq)]
pub struct Frustum
{
    planes: [Plane; 6],
}
impl Frustum
{
    pub const NULL: Frustum = Frustum { planes: [Plane::NULL; 6] }; // an invalid frustum acting as a placeholder

    // if input is projection, planes are in view space
    // if view projection, planes are in world space
    // if model view projection, planes are in model space
    pub fn new(col_major_mtx: &Mat4) -> Self
    {
        let rows = col_major_mtx.transpose(); // glam stores in column-major
        let planes =
        [
            Plane::from(rows.w_axis + rows.x_axis).normalized(), // left
            Plane::from(rows.w_axis - rows.x_axis).normalized(), // right
            Plane::from(rows.w_axis - rows.y_axis).normalized(), // top
            Plane::from(rows.w_axis + rows.y_axis).normalized(), // bottom

            // these seem to work, but feels wrong
            Plane::from(rows.z_axis).mirrored().normalized(), // near
            Plane::from(rows.w_axis - rows.z_axis).flipped().normalized(), // far
        ];
        Self { planes }
    }

    #[inline] pub fn left(&self) -> Plane { self.planes[0] }
    #[inline] pub fn right(&self) -> Plane { self.planes[1] }
    #[inline] pub fn top(&self) -> Plane { self.planes[2] }
    #[inline] pub fn bottom(&self) -> Plane { self.planes[3] }
    #[inline] pub fn near(&self) -> Plane { self.planes[4] }
    #[inline] pub fn far(&self) -> Plane { self.planes[5] }

    pub fn get_corners(projected_mtx: Mat4) -> [Vec3; 8]
    {
        [
            projected_mtx.project_point3(Vec3::new(-1.0, -1.0, -1.0)), // near bottom left
            projected_mtx.project_point3(Vec3::new( 1.0, -1.0, -1.0)), // near bottom right
            projected_mtx.project_point3(Vec3::new(-1.0,  1.0, -1.0)), // near top left
            projected_mtx.project_point3(Vec3::new( 1.0,  1.0, -1.0)), // near top right
            projected_mtx.project_point3(Vec3::new(-1.0, -1.0,  1.0)), // far bottom left
            projected_mtx.project_point3(Vec3::new( 1.0, -1.0,  1.0)), // far bottom right
            projected_mtx.project_point3(Vec3::new(-1.0,  1.0,  1.0)), // far top left
            projected_mtx.project_point3(Vec3::new( 1.0,  1.0,  1.0)), // far top right
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
        let mut inside = true;
        for p in &self.planes
        {
             inside &= ! matches!(p.get_facing(other), Facing::Behind);
        }
        match inside
        {
            true => Intersection::Overlapping,
            false => Intersection::None,
        }
    }
}

#[cfg(test)]
mod tests
{
    use glam::Vec3;
    use crate::math::Radians;
    use super::*;

    #[test]
    fn planes()
    {
        // todo: use Camera?

        let projection = Mat4::perspective_lh(Radians::PI_OVER_TWO.0, 1.0, 1.0, 10.0);
        let view = Mat4::look_at_lh(
            Vec3::ZERO,
            Vec3::Z,
            Vec3::Y,
        );
        
        let view_projection = projection * view;

        let frustum = Frustum::new(&view_projection);
        println!("{}\n{:?}", view_projection, frustum);

        let recip_sqrt2 = 1.0 / 2.0_f32.sqrt();

        // TODO: these values are wrong
        let expected_planes = [
            Plane::new(Vec3::new(recip_sqrt2, 0.0, recip_sqrt2), 0.0),
            Plane::new(Vec3::new(-recip_sqrt2, 0.0, recip_sqrt2), 0.0),
            Plane::new(Vec3::new(0.0, -recip_sqrt2, recip_sqrt2), 0.0),
            Plane::new(Vec3::new(0.0, recip_sqrt2, recip_sqrt2), 0.0),
            Plane::new(Vec3::new(0.0, 0.0, 1.0), 1.0),
            Plane::new(Vec3::new(0.0, 0.0, 1.0), 10.0),
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

    // TODO: corners
}