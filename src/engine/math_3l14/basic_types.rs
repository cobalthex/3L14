use glam::{Mat3, Quat, Vec3};

#[repr(C, packed)]
pub struct Affine3
{
    pub matrix3: Mat3,
    pub translation: Vec3,
}
impl Affine3
{
    pub fn from_rotation_translation(rotation: Quat, translation: Vec3) -> Self
    {
        Self
        {
            matrix3: Mat3::from_quat(rotation),
            translation,
        }
    }
    
    // add scale?
}