use std::sync::Arc;
use bitcode::{Decode, Encode};
use egui::Ui;
use asset_3l14::{AssetLifecycler, AssetLoadRequest};
use debug_3l14::debug_gui::DebugGui;
use proc_macros_3l14::Asset;
use crate::assets::{Geometry, GeometryLifecycler};
use crate::Renderer;

#[derive(Encode, Decode)]
pub struct SkeletonFile
{
    pub joints: Box<[u8]>,
    pub weights: Box<[u8]>,
}

#[derive(Asset)]
pub struct Skeleton
{

}

pub struct SkeletonLifecycler
{
    pub renderer: Arc<Renderer>,
}
impl SkeletonLifecycler
{
    pub fn new(renderer: Arc<Renderer>) -> Self
    {
        Self { renderer }
    }
}
impl AssetLifecycler for SkeletonLifecycler
{
    type Asset = Skeleton;

    fn load(&self, mut request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn std::error::Error>>
    {
        todo!()
    }
}
impl DebugGui for SkeletonLifecycler
{
    fn name(&self) -> &str
    {
        todo!()
    }

    fn debug_gui(&self, ui: &mut Ui)
    {
        todo!()
    }
}