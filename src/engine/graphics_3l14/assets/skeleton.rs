use std::error::Error;
use bitcode::{Decode, Encode};
use egui::Ui;
use asset_3l14::{AssetDebugData, AssetLifecycler, AssetLoadRequest};
use debug_3l14::debug_gui::DebugGui;
use math_3l14::DualQuat;
use proc_macros_3l14::Asset;

#[derive(Asset, Encode, Decode)]
pub struct Skeleton
{
    pub inv_bind_pose: Box<[DualQuat]>, // ordered by numerically sorted bone ID hash
}

impl AssetDebugData for Skeleton
{
    type DebugData = SkeletonDebugData;
}

pub struct SkeletonDebugData
{

}

pub struct SkeletonLifecycler;
impl AssetLifecycler for SkeletonLifecycler
{
    type Asset = Skeleton;

    fn load(&self, mut request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>>
    {
        request.deserialize::<Skeleton>()
    }
}
impl DebugGui for SkeletonLifecycler
{
    fn name(&self) -> &str { "Skeletons" }

    fn debug_gui(&self, ui: &mut Ui)
    {
        // TODO
    }
}