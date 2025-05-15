use glam::{Mat4, Quat, Vec3};
use crate::DualQuat;
// TODO: Vec3A?

pub const WORLD_RIGHT: Vec3 = Vec3 { x: 1.0, y: 0.0, z: 0.0 };
pub const WORLD_UP: Vec3 = Vec3 { x: 0.0, y: 1.0, z: 0.0 };
pub const WORLD_FORWARD: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 1.0 }; // into screen

#[derive(Debug, PartialEq, Clone)]
pub struct Transform
{
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}
impl Default for Transform
{
    fn default() -> Self { Self
    {
        position: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    }}
}
impl Transform
{
    #[inline] #[must_use] pub fn forward(&self) -> Vec3
    {
        Vec3::new(
            2.0 * (self.rotation.x * self.rotation.z + self.rotation.w * self.rotation.y),
            2.0 * (self.rotation.y * self.rotation.z - self.rotation.w * self.rotation.x),
            1.0 - 2.0 * (self.rotation.x * self.rotation.x + self.rotation.y * self.rotation.y),
        )
    }
    #[inline] #[must_use] pub fn backward(&self) -> Vec3 { -self.forward() }
    #[inline] #[must_use] pub fn right(&self) -> Vec3
    {
        Vec3::new(
            1.0 - 2.0 * (self.rotation.y * self.rotation.y + self.rotation.z * self.rotation.z),
            2.0 * (self.rotation.x * self.rotation.y + self.rotation.w * self.rotation.z),
            2.0 * (self.rotation.x * self.rotation.z - self.rotation.w * self.rotation.y),
        )
    }
    #[inline] #[must_use] pub fn left(&self) -> Vec3 { -self.right() }
    #[inline] #[must_use] pub fn up(&self) -> Vec3
    {
        Vec3::new(
            2.0 * (self.rotation.x * self.rotation.y - self.rotation.w * self.rotation.z),
            1.0 - 2.0 * (self.rotation.x * self.rotation.x + self.rotation.z * self.rotation.z),
            2.0 * (self.rotation.y * self.rotation.z + self.rotation.w * self.rotation.x),
        )
    }
    #[inline] #[must_use] pub fn down(&self) -> Vec3 { -self.up() }

    // Apply an in-place rotation to this transform
    pub fn rotate(&mut self, yaw: f32, pitch: f32, roll: f32)
    {
        // todo: yaw,pitch only version?
        let yaw_quat = Quat::from_axis_angle(WORLD_UP, yaw);
        let pitch_quat = Quat::from_axis_angle(WORLD_RIGHT, pitch);
        let roll_quat = Quat::from_axis_angle(WORLD_FORWARD, roll);
        // LH ordering
        self.rotation = Quat::normalize(yaw_quat * self.rotation * pitch_quat * roll_quat);
    }

    #[inline] #[must_use]
    pub fn inverse(&self) -> Transform
    {
        Self
        {
            position: -self.position,
            rotation: self.rotation.inverse(),
            scale: self.scale.recip(),
        }
    }

    #[inline] #[must_use]
    pub fn to_view_mtx(&self) -> Mat4
    {
        let rotation = Mat4::from_quat(self.rotation.inverse());
        let translation = Mat4::from_translation(-self.position);
        // let scale = Mat4::from_scale(self.scale.recip());
        /*scale * */ rotation * translation
    }

    #[inline] #[must_use]
    pub fn to_world_mtx(&self) -> Mat4 { Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position) }

    // Ignores scale
    #[inline] #[must_use]
    pub fn to_dual_quat(&self) -> DualQuat
    {
        DualQuat::from_rot_trans(self.rotation, self.position)
    }
}

impl From<(Vec3, Quat, Vec3)> for Transform
{
    fn from((position, rotation, scale): (Vec3, Quat, Vec3)) -> Self
    {
        Transform { position, rotation, scale }
    }
}
impl From<Transform> for Mat4
{
    fn from(t: Transform) -> Self { t.to_world_mtx() }
}
impl From<Mat4> for Transform
{
    fn from(m: Mat4) -> Self
    {
        let (scale, rotation, position) = m.to_scale_rotation_translation();
        Transform { position, rotation, scale }
    }
}

// TODO: move
#[derive(Default, Clone, Copy)]
#[repr(C, align(16))]
pub struct StaticGeoUniform
{
    pub world: Mat4,
}
impl From<Transform> for StaticGeoUniform
{
    fn from(transform: Transform) -> Self
    {
        Self
        {
            world: transform.to_world_mtx()
        }
    }
}

#[cfg(test)]
mod tests
{
    use approx::{assert_abs_diff_eq};
    use super::*;

    #[test]
    fn direction_vectors()
    {
        let transform = Transform
        {
            position: Vec3::ZERO,
            rotation: Quat::from_axis_angle(Vec3::new(1.0, 2.0, 0.6).normalize(), 1.0),
            scale: Vec3::ONE,
        };

        let forward = transform.rotation * WORLD_FORWARD;
        assert_abs_diff_eq!(forward, transform.forward());
        let right = transform.rotation * WORLD_RIGHT;
        assert_abs_diff_eq!(right, transform.right());
        let up = transform.rotation * WORLD_UP;
        assert_abs_diff_eq!(up, transform.up());

        let backward = transform.rotation * -WORLD_FORWARD;
        assert_abs_diff_eq!(backward, transform.backward());
        let left = transform.rotation * -WORLD_RIGHT;
        assert_abs_diff_eq!(left, transform.left());
        let down = transform.rotation * -WORLD_UP;
        assert!(down.abs_diff_eq(transform.down(), f32::EPSILON));
    }

    // TODO: matrix ops
}