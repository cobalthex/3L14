use approx::RelativeEq;
use glam::Vec3;
use crate::{DualQuat, WORLD_FORWARD};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray
{
    pub origin: Vec3,
    pub direction: Vec3,
}
impl Ray
{
    // assumes normalized direction
    #[inline] #[must_use]
    pub fn new(origin: Vec3, direction: Vec3) -> Self { Self { origin, direction } }
}
impl From<DualQuat> for Ray
{
    fn from(dq: DualQuat) -> Self
    {
        Self
        {
            origin: dq.translation(),
            direction: dq.rotation() * WORLD_FORWARD,
        }
    }
}