use std::fmt::{Debug, Formatter};
use bitcode::Encode;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use crate::Angle;

#[derive(Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct YawPitchRoll(Vec3);
impl YawPitchRoll
{
    #[inline] #[must_use]
    pub fn new(yaw: Angle, pitch: Angle, roll: Angle) -> Self
    {
        Self(Vec3::new(yaw.to_radians(), pitch.to_radians(), roll.to_radians()))
    }

    #[inline] #[must_use]
    pub fn yaw(&self) -> Angle { Angle::from_radians(self.0.x) }
    #[inline] #[must_use]
    pub fn pitch(&self) -> Angle { Angle::from_radians(self.0.y) }
    #[inline] #[must_use]
    pub fn roll(&self) -> Angle { Angle::from_radians(self.0.z) }
}
impl Debug for YawPitchRoll
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        f.debug_struct("YawPitchRoll")
            .field("yaw", &self.yaw())
            .field("pitch", &self.pitch())
            .field("roll", &self.roll())
            .finish()
    }
}
