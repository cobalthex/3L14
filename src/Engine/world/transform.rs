use glam::{Mat4, Quat, Vec3};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Transform
{
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}
impl Transform
{
    pub fn look_to(&mut self, direction: Vec3)
    {
        
    }
    
    // TODO: test
    pub fn forward(&self) -> Vec3 { self.rotation * super::WORLD_FORWARD }
    pub fn backward(&self) -> Vec3 { self.rotation * -super::WORLD_FORWARD }
    pub fn up(&self) -> Vec3 { self.rotation * super::WORLD_UP }
    pub fn down(&self) -> Vec3 { self.rotation * -super::WORLD_UP }
    pub fn right(&self) -> Vec3 { self.rotation * super::WORLD_RIGHT }
    pub fn left(&self) -> Vec3 { self.rotation * -super::WORLD_RIGHT }
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
    fn from(t: Transform) -> Self
    {
        Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.position)
    }
}
impl From<Mat4> for Transform
{
    fn from(m: Mat4) -> Self
    {
        let (scale, rotation, position) = m.to_scale_rotation_translation();
        Transform { position, rotation, scale }
    }
}