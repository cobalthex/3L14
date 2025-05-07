use math_3l14::Affine3;
use nab_3l14::timing::FSeconds;
use crate::assets::{SkeletalAnimation, Skeleton};

pub enum LoopBehavior
{
    Loop,
    StopAtLastFrame,
}

pub struct AnimationFrame<'s>
{
    pub time: FSeconds,
    pub loop_behavior: LoopBehavior,
    pub skeleton: &'s Skeleton,
    pub animation: &'s SkeletalAnimation,
}

pub struct Pose<'p>(&'p [Affine3]);
impl Pose<'_>
{
    pub fn from_animation(frame: AnimationFrame<'_>) -> Self
    {
        todo!()
    }
}