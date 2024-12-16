use glam::{Mat4, Quat, Vec3};
use crate::engine::world::{WORLD_FORWARD, WORLD_RIGHT, WORLD_UP};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewMtx(pub Mat4);
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldMtx(pub Mat4);

// TODO: Vec3A?

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
    pub fn rotate(&mut self, yaw: f32, pitch: f32, roll: f32)
    {
        // todo: yaw,pitch only version?
        let yaw_quat = Quat::from_axis_angle(WORLD_UP, yaw);
        let pitch_quat = Quat::from_axis_angle(WORLD_RIGHT, pitch);
        let roll_quat = Quat::from_axis_angle(WORLD_FORWARD, roll);
        // LH ordering
        self.rotation = Quat::normalize(yaw_quat * self.rotation * pitch_quat * roll_quat);
    }

    pub fn to_view(&self) -> ViewMtx
    {
        let rotation = Mat4::from_quat(self.rotation.inverse());
        let translation = Mat4::from_translation(-self.position);
        ViewMtx(rotation * translation)
    }

    pub fn to_world(&self) -> WorldMtx
    {
        WorldMtx(Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position))
    }
}

impl From<(Vec3, Quat, Vec3)> for Transform
{
    fn from((position, rotation, scale): (Vec3, Quat, Vec3)) -> Self
    {
        Transform { position, rotation, scale }
    }
}
impl From<Transform> for WorldMtx
{
    fn from(t: Transform) -> Self { t.to_world() }
}
impl From<WorldMtx> for Transform
{
    fn from(m: WorldMtx) -> Self
    {
        let (scale, rotation, position) = m.0.to_scale_rotation_translation();
        Transform { position, rotation, scale }
    }
}

#[derive(Default, Clone, Copy)]
#[repr(C, align(16))]
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
            world: transform.to_world().0
        }
    }
}