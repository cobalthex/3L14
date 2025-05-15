use asset_3l14::{AssetDebugData, AssetLifecycler, TrivialAssetLifecycler};
use bitcode::{Decode, Encode};
use debug_3l14::debug_gui::DebugGui;
use egui::Ui;
use math_3l14::DualQuat;
use metrohash::MetroHash64;
use proc_macros_3l14::Asset;
use std::hash::{Hash, Hasher};
use nab_3l14::hashing::hash64_to_32;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
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

#[derive(Encode, Decode)]
pub struct BoneRelation
{
    pub id: BoneId,
    pub parent_index: u32,
}

#[derive(Asset, Encode, Decode)]
pub struct Skeleton
{
    pub bones: Box<[BoneRelation]>, // bones are ordered with root first in DFS order
    pub bind_poses: Box<[DualQuat]>,
    pub inv_bind_poses: Box<[DualQuat]>, // only store one of these?
}

impl AssetDebugData for Skeleton
{
    type DebugData = SkeletonDebugData;
}

#[derive(Encode, Decode)]
pub struct SkeletonDebugData
{
    pub bone_names: Box<[String]>,
}

pub struct SkeletonLifecycler;
impl TrivialAssetLifecycler for SkeletonLifecycler { type Asset = Skeleton; }
impl DebugGui for SkeletonLifecycler
{
    fn display_name(&self) -> &str { "Skeletons" }

    fn debug_gui(&self, ui: &mut Ui)
    {
        // TODO
    }
}