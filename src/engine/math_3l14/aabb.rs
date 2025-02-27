use bitcode::{Decode, Encode};
use glam::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Encode, Decode)]
pub struct AABB
{
    pub min: Vec3,
    pub max: Vec3,
}
impl AABB
{
    pub const MAX_MIN: Self = Self { min: Vec3::MAX, max: Vec3::MIN };

    #[inline] #[must_use] pub const fn new(min: Vec3, max: Vec3) -> Self { Self { min, max } }

    #[inline] #[must_use] pub fn size(&self) -> Vec3 { self.max - self.min }
    #[inline] #[must_use] pub fn half(&self) -> Vec3 { (self.max - self.min) / 2.0 }
    #[inline] #[must_use]
    pub fn volume(&self) -> f32
    {
        let diff = self.max - self.min;
        diff.x * diff.y * diff.z
    }

    #[inline] #[must_use] pub fn center(&self) -> Vec3 { (self.min + self.max) / 2.0 }

    #[inline]
    pub fn union_with(&mut self, rhs: AABB)
    {
        self.min = self.min.min(rhs.min);
        self.max = self.max.max(rhs.max);
    }
}

// todo: proper shapes library?
