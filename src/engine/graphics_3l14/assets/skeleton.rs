use crate::{debug_label, Renderer};
use asset_3l14::{AssetLifecycler, AssetLoadRequest};
use bitcode::{Decode, Encode};
use debug_3l14::debug_gui::DebugGui;
use egui::Ui;
use proc_macros_3l14::Asset;
use std::sync::Arc;
use wgpu::{Buffer, BufferUsages};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use crate::assets::GeometryFile;

#[derive(Encode, Decode)]
pub struct SkeletonFile
{
    // contains DualQuats
    pub inverse_bind_transforms: Box<[u8]>,
}

#[derive(Asset)]
pub struct Skeleton
{
    // contains DualQuats
    pub inverse_bind_transforms: Buffer,
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
        let sf = request.deserialize::<SkeletonFile>()?;

        let skeleton = self.renderer.device().create_buffer_init(&BufferInitDescriptor
        {
            label: debug_label!(format!("{:?} skeleton", request.asset_key).as_str()),
            contents: sf.inverse_bind_transforms.as_ref(),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        
        Ok(Skeleton { inverse_bind_transforms: skeleton })
    }
}
impl DebugGui for SkeletonLifecycler
{
    fn name(&self) -> &str { "Skeletons" }

    fn debug_gui(&self, ui: &mut Ui)
    {
        ui.label("TODO");
    }
}