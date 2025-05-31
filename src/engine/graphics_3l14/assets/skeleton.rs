use asset_3l14::TrivialAssetLifecycler;
use bitcode::{Decode, Encode};
use debug_3l14::debug_gui::DebugGui;
use egui::Ui;
use math_3l14::DualQuat;
use metrohash::MetroHash64;
use nab_3l14::hashing::hash64_to_32;
use proc_macros_3l14::asset;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};

pub const MAX_SKINNED_BONES: usize = 128; // TODO: share with HLSL

#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, Debug)]
pub struct BoneId(pub u32); // TODO: 32 bits should be sufficient
impl BoneId
{
    // names are case-sensitive
    pub fn from_name(value: &str) -> Self
    {
        let mut hasher = MetroHash64::new();
        value.hash(&mut hasher);
        let trunc = hash64_to_32(hasher.finish());
        Self(trunc)
    }
}

#[asset(debug_type = SkeletonDebugData)]
#[derive(Encode, Decode)]
pub struct Skeleton
{
    pub bone_ids: Box<[BoneId]>,
    pub parent_indices: Box<[i16]>, // bones are ordered with parents first (with the root at the front), negative indicates no parent
    pub bind_poses: Box<[DualQuat]>, // necessary?
    pub inverse_bind_poses: Box<[DualQuat]>,
}

#[derive(Encode, Decode)]
pub struct SkeletonDebugData
{
    pub bone_names: Box<[String]>,
}

#[derive(Default)]
pub struct SkeletonLifecycler
{
    display_bones: AtomicBool,
}
impl SkeletonLifecycler
{
    pub fn display_bones(&self) -> bool { self.display_bones.load(Ordering::Relaxed) }
}
impl TrivialAssetLifecycler for SkeletonLifecycler { type Asset = Skeleton; }
impl DebugGui for SkeletonLifecycler
{
    fn display_name(&self) -> &str { "Skeletons" }

    fn debug_gui(&self, ui: &mut Ui)
    {
        let mut display_bones = self.display_bones.load(Ordering::Acquire);
        ui.checkbox(&mut display_bones, "Display bones");
        self.display_bones.store(display_bones, Ordering::Release);
    }
}