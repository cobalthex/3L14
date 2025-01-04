use glam::{Mat4, Vec3};
use crate::engine::math::{Facing, GetFacing, Intersection, Intersects, Plane};

#[derive(Debug)]
pub struct Frustum
{
    planes: [Plane; 6],
}
impl Frustum
{
    pub fn new(proj_view: &Mat4) -> Self
    {
        // note: these are not implicitly normalized

        let rows = proj_view.transpose(); // glam stores in column-major
        let planes =
        [
            (rows.w_axis + rows.x_axis).into(),
            (rows.w_axis - rows.x_axis).into(),
            (rows.w_axis + rows.y_axis).into(),
            (rows.w_axis - rows.y_axis).into(),
            (rows.w_axis + rows.z_axis).into(),
            (rows.w_axis - rows.z_axis).into(),
        ];
        Self { planes }
    }

    #[inline] pub fn left(&self) -> Plane { self.planes[0] }
    #[inline] pub fn right(&self) -> Plane { self.planes[1] }
    #[inline] pub fn top(&self) -> Plane { self.planes[2] }
    #[inline] pub fn bottom(&self) -> Plane { self.planes[3] }
    #[inline] pub fn near(&self) -> Plane { self.planes[4] }
    #[inline] pub fn far(&self) -> Plane { self.planes[5] }

    pub fn normalize(&mut self)
    {
        for p in &mut self.planes
        {
            p.normalize()
        }
    }
}
impl Intersects<Vec3> for Frustum
{
    fn intersects(&self, other: &Vec3) -> Intersection
    {
        let mut inside = true;
        for p in &self.planes
        {
             inside &= ! matches!(p.get_facing(other), Facing::Behind);
        }
        match inside
        {
            true => Intersection::Contained,
            false => Intersection::None,
        }
    }
}

#[cfg(test)]
mod tests
{
    use glam::Vec3;
    use crate::engine::math::Radians;
    use super::*;

    #[test]
    fn planes()
    {
        // todo: use Camera?

        let projection = Mat4::perspective_lh(Radians::PI_OVER_TWO.0, 1.0, 1.0, 10.0);
        let view = Mat4::look_at_lh(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::Y,
        );
        
        let view_projection = projection * view;

        let frustum = Frustum::new(&view_projection);
        println!("{:?}", frustum);

        let recip_sqrt2 = 1.0 / 2.0_f32.sqrt();

        // TODO: these values are wrong
        let expected_planes = [
            Plane::new(Vec3::new(recip_sqrt2, 0.0, recip_sqrt2), 1.0),
            Plane::new(Vec3::new(-recip_sqrt2, 0.0, recip_sqrt2), 1.0),
            Plane::new(Vec3::new(0.0, recip_sqrt2, recip_sqrt2), 1.0),
            Plane::new(Vec3::new(0.0, -recip_sqrt2, recip_sqrt2), 1.0),
            Plane::new(Vec3::new(0.0, 0.0, 1.0), 1.0),
            Plane::new(Vec3::new(0.0, 0.0, -1.0), 10.0),
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
}