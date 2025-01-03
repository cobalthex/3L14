use glam::Mat4;
use crate::engine::math::Plane;

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
        let projection = Mat4::perspective_lh(Radians::PI_OVER_TWO.0, 1.0, 1.0, 10.0);
        let view = Mat4::look_at_lh(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::Y,
        );

        let view_projection = projection * view;

        // Extract frustum planes
        let frustum = Frustum::new(&view_projection);
        println!("{:?}", frustum);

        // Verify plane normals and distances
        // Expected results are calculated manually for this simple setup
        let expected_planes = [
            Plane::new(Vec3::new(1.0, 0.0, 0.0), 1.0),
            Plane::new(Vec3::new(-1.0, 0.0, 0.0), 1.0),
            Plane::new(Vec3::new(0.0, 1.0, 0.0), 1.0),
            Plane::new(Vec3::new(0.0, -1.0, 0.0), 1.0),
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