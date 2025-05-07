use super::BoneId;
use asset_3l14::TrivialAssetLifecycler;
use bitcode::{Decode, Encode};
use debug_3l14::debug_gui::DebugGui;
use egui::Ui;
use math_3l14::{DualQuat, Ratio};
use proc_macros_3l14::Asset;

#[derive(Encode, Decode)]
pub struct SkeletalAnimationTrack
{
    pub bone_index: u32,
    pub poses: Box<[DualQuat]>,
}

#[derive(Asset, Encode, Decode)]
pub struct SkeletalAnimation
{
    pub skel_hash: u64, // TODO: newtype
    pub sample_rate: Ratio<u32>,
    pub tracks: Box<[SkeletalAnimationTrack]>, // 2D array, # of bones * # of keyframes
}

pub struct SkeletalAnimationLifecycler;
impl TrivialAssetLifecycler for SkeletalAnimationLifecycler { type Asset = SkeletalAnimation; }
impl DebugGui for SkeletalAnimationLifecycler
{
    fn name(&self) -> &str { "Skeletal animation" }

    fn debug_gui(&self, ui: &mut Ui)
    {
        // TODO
    }
}