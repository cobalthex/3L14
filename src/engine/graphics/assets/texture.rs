use crate::engine::graphics::Renderer;
use crate::format_bytes;
use bitcode::{Decode, Encode};
use egui::Ui;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use wgpu::util::{DeviceExt, TextureDataOrder};
use wgpu::{Extent3d, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor};

use crate::engine::assets::{Asset, AssetLifecycler, AssetLoadError, AssetLoadRequest, AssetPayload, AssetTypeId};
use crate::engine::graphics::debug_gui::DebugGui;

pub const MAX_MIP_COUNT: usize = 16;

#[repr(u8)]
#[derive(Encode, Decode)]
pub enum TextureFilePixelFormat
{
    // Uncompressed formats
    Rgba8 = 1,
    Rgba8Srgb = 2,
    R8 = 3,
    Rg8 = 4,

    // TODO: compressed formats (bc#)

}

#[derive(Encode, Decode)]
pub struct TextureFile
{
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mip_count: u8, // always <= MAX_MIP_COUNT
    pub mip_offsets: [usize; MAX_MIP_COUNT], // offsets into the payload (0 being the beginning of the smallest mip)
    pub pixel_format: TextureFilePixelFormat,
    // mips are organized from smallest (lowest quality) to largest (highest quality)
    // all mips are stored contiguously w/out gaps
}

pub struct Texture
{
    gpu_tex: wgpu::Texture,
    gpu_view: wgpu::TextureView,
    desc: wgpu::TextureDescriptor<'static>, // TODO: might be able to get this from the gpu_tex directly
}  
impl Texture
{
    pub fn gpu_handle(&self) -> &wgpu::Texture { &self.gpu_tex }
    pub fn desc(&self) -> &wgpu::TextureDescriptor { &self.desc }

    pub fn total_device_bytes(&self) -> i64
    {
        let mut total_size = 0i64;
        for mip in 0..self.desc.mip_level_count
        {
            let size = self.desc.mip_level_size(mip).unwrap().physical_size(self.desc.format);
            let area = (size.width as i64) * (size.height as i64) * (size.depth_or_array_layers as i64);
            let block_size = self.desc.format.block_copy_size(Some(TextureAspect::All));
            total_size += area * block_size.unwrap() as i64;
        }
        total_size
    }
}
impl Asset for Texture
{
    fn asset_type() -> AssetTypeId { AssetTypeId::Texture }
}

pub struct TextureLifecycler
{
    renderer: Arc<Renderer>,
    device_bytes: AtomicI64,
}
impl TextureLifecycler
{
    pub fn new(renderer: Arc<Renderer>) -> Self
    {
        Self
        {
            renderer,
            device_bytes: AtomicI64::new(0)
        }
    }

    fn create(&self, width: u32, height: u32, texels: &[u8]) -> Texture
    {
        let dtor = TextureDescriptor
        {
            label: None, // TODO
            size: Extent3d
            {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };

        let tex = self.renderer.device().create_texture_with_data(
            self.renderer.queue(),
            &dtor,
            TextureDataOrder::LayerMajor,
            texels);

        let view = tex.create_view(&TextureViewDescriptor
        {
            label: None,
            format: None,
            dimension: None,
            aspect: Default::default(),
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        let payload = Texture
        {
            gpu_tex: tex,
            gpu_view: view,
            desc: dtor,
        };

        let bytes = payload.total_device_bytes();
        self.device_bytes.fetch_add(bytes, Ordering::Relaxed); // relaxed ok here?

        payload
    }
}
impl AssetLifecycler for TextureLifecycler
{
    type Asset = Texture;
    fn load(&self, request: AssetLoadRequest) -> AssetPayload<Self::Asset>
    {
        // TODO
        AssetPayload::Unavailable(AssetLoadError::ParseError(0))
    }
}

impl<'a> DebugGui<'a> for TextureLifecycler
{
    fn name(&self) -> &'a str { "Textures" }

    fn debug_gui(&self, ui: &mut Ui)
    {
        ui.label(format!("Total device bytes: {:.1}", format_bytes!(self.device_bytes.load(Ordering::Relaxed))));
    }
}