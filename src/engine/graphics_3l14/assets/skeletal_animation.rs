use asset_3l14::TrivialAssetLifecycler;
use bitcode::{Decode, Encode};
use debug_3l14::debug_gui::DebugGui;
use egui::Ui;
use math_3l14::{DualQuat, Ratio};
use proc_macros_3l14::Asset;
use crate::assets::BoneId;

// todo: standardize
#[derive(Copy, Clone, PartialEq, Eq, Encode, Decode)]
pub struct AnimFrameNumber(pub u32);

#[derive(Asset, Encode, Decode)]
pub struct SkeletalAnimation
{
    pub sample_rate: Ratio<u32>,
    pub frame_count: AnimFrameNumber,
    pub bones: Box<[BoneId]>, // sorted by ID
    pub poses: Box<[DualQuat]>, // 2D array, # of bones (ordered by bone ID) * # of keyframes
}
impl SkeletalAnimation
{
    pub fn get_pose_for_frame(&self, frame: AnimFrameNumber) -> &[DualQuat]
    {
        let bone_count = self.bones.len();
        let start = frame.0 as usize * bone_count;
        &self.poses[start..start + bone_count]
    }
}

pub struct SkeletalAnimationLifecycler;
impl TrivialAssetLifecycler for SkeletalAnimationLifecycler { type Asset = SkeletalAnimation; }
impl DebugGui for SkeletalAnimationLifecycler
{
    fn display_name(&self) -> &str { "Skeletal animation" }

    fn debug_gui(&self, ui: &mut Ui)
    {
        // TODO
    }
}