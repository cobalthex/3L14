use glam::{Mat4, Quat, Vec3};

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
    // TODO: test
    pub fn forward(&self) -> Vec3 { self.rotation * super::WORLD_FORWARD }
    pub fn backward(&self) -> Vec3 { self.rotation * -super::WORLD_FORWARD }
    pub fn up(&self) -> Vec3 { self.rotation * super::WORLD_UP }
    pub fn down(&self) -> Vec3 { self.rotation * -super::WORLD_UP }
    pub fn right(&self) -> Vec3 { self.rotation * super::WORLD_RIGHT }
    pub fn left(&self) -> Vec3 { self.rotation * -super::WORLD_RIGHT }

    pub fn to_view(&self) -> Mat4
    {
        //let silly = Quat::from_xyzw(self.rotation.x, self.rotation.y, -self.rotation.z, -self.rotation.w);
        //let rotation = Mat4::from_quat(silly);
        let rotation = Mat4::from_quat(self.rotation);
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