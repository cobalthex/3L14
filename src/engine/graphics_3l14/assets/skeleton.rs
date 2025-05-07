use asset_3l14::{AssetDebugData, AssetLifecycler, TrivialAssetLifecycler};
use bitcode::{Decode, Encode};
use debug_3l14::debug_gui::DebugGui;
use egui::Ui;
use math_3l14::DualQuat;
use metrohash::MetroHash64;
use proc_macros_3l14::Asset;
use std::hash::{Hash, Hasher};

#[derive(Encode, Decode, PartialOrd, PartialEq)]
pub struct BoneId(pub u64);
impl BoneId
{
    // names are case-sensitive
    pub fn from_name(value: &str) -> Self
    {
        let mut hasher = MetroHash64::new();
        value.hash(&mut hasher);
        Self(hasher.finish())
    }
}

#[derive(Asset, Encode, Decode)]
pub struct Skeleton
{
    // bone IDs?
    pub inv_bind_pose: Box<[DualQuat]>, // ordered by numerically sorted bone ID hash
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
    fn name(&self) -> &str { "Skeletons" }

    fn debug_gui(&self, ui: &mut Ui)
    {
        // TODO
    }
}