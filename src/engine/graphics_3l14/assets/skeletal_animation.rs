use asset_3l14::TrivialAssetLifecycler;
use bitcode::{Decode, Encode};
use debug_3l14::debug_gui::DebugGui;
use egui::Ui;
use math_3l14::{DualQuat, Ratio, PackedTransform};
use proc_macros_3l14::asset;
use crate::assets::BoneId;

// todo: standardize
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Encode, Decode)]
pub struct AnimFrameNumber(pub u32);

#[asset]
#[derive(Encode, Decode)]
pub struct SkeletalAnimation
{
    pub sample_rate: Ratio<u32>, // todo: hard code into flags?
    // flags
    // TODO: sparse frames.
    pub frame_count: AnimFrameNumber,
    pub bones: Box<[BoneId]>, // sorted by ID
    pub poses: Box<[PackedTransform]>, // 2D array, # of bones (ordered by bone ID) * # of keyframes
}
impl SkeletalAnimation
{
    pub fn get_pose_for_frame(&self, frame: AnimFrameNumber) -> &[PackedTransform]
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