use glam::{Mat4, Quat, Vec3};
use crate::engine::world::{WORLD_FORWARD, WORLD_RIGHT, WORLD_UP};

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
    pub fn forward(&self) -> Vec3 { self.rotation * super::WORLD_FORWARD }
    pub fn backward(&self) -> Vec3 { self.rotation * -super::WORLD_FORWARD }
    pub fn right(&self) -> Vec3 { self.rotation * super::WORLD_RIGHT }
    pub fn left(&self) -> Vec3 { self.rotation * -super::WORLD_RIGHT }
    pub fn up(&self) -> Vec3 { self.rotation * super::WORLD_UP }
    pub fn down(&self) -> Vec3 { self.rotation * -super::WORLD_UP }

    // Apply an in-place rotation to this transform
    pub fn turn(&mut self, yaw: f32, pitch: f32, roll: f32)
    {
        let yaw_quat = Quat::from_axis_angle(WORLD_UP, yaw);
        let pitch_quat = Quat::from_axis_angle(WORLD_RIGHT, pitch);
        let roll_quat = Quat::from_axis_angle(WORLD_FORWARD, roll);
        // LH ordering
        self.rotation = Quat::normalize(yaw_quat * self.rotation * pitch_quat * roll_quat);
    }

    pub fn to_view(&self) -> Mat4
    {
        let rotation = Mat4::from_quat(self.rotation.inverse());
        let translation = Mat4::from_translation(-self.position);
        rotation * translation
    }

    pub fn to_world(&self) -> Mat4
    {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
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
    fn from(t: Transform) -> Self { t.to_world() }
}
impl From<Mat4> for Transform
{
    fn from(m: Mat4) -> Self
    {
        let (scale, rotation, position) = m.to_scale_rotation_translation();
        Transform { position, rotation, scale }
    }
}

#[derive(Default)]
#[repr(C, align(256))]
pub struct TransformUniform
{
    pub world: Mat4,
}
impl From<Transform> for TransformUniform
{
    fn from(transform: Transform) -> Self
    {
        Self
        {
            world: transform.into()
        }
    }
}