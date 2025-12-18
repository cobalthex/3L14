use bitcode::{Decode, Encode};
use egui::Ui;
use std::error::Error;
use std::sync::atomic::{AtomicI64, Ordering};
use triomphe::Arc;
use wgpu::util::{DeviceExt, TextureDataOrder};
use wgpu::{Extent3d, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor};
use asset_3l14::{AssetLifecycler, AssetLoadRequest};
use debug_3l14::debug_gui::DebugGui;
use nab_3l14::format_binary;
use crate::{debug_label, Renderer};

pub const MAX_MIP_COUNT: usize = 16;

#[repr(u8)]
#[derive(Encode, Decode)]
pub enum TextureFilePixelFormat
{
    // Uncompressed formats
    R8 = 1,
    Rg8 = 2,
    Rgba8 = 3,
    Rgba8Srgb = 4,

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

#[proc_macros_3l14::asset]
pub struct Texture
{
    pub gpu_tex: wgpu::Texture,
    pub gpu_view: wgpu::TextureView,
}
impl Texture
{
    pub fn total_device_bytes(&self) -> i64
    {
        let mut total_size = 0i64;
        for mip in 0..self.gpu_tex.mip_level_count()
        {
            let size = self.gpu_tex.size()
                .mip_level_size(mip, self.gpu_tex.dimension())
                .physical_size(self.gpu_tex.format());

            let area = (size.width as i64) * (size.height as i64) * (size.depth_or_array_layers as i64);
            let block_size = self.gpu_tex.format().block_copy_size(Some(TextureAspect::All));

            total_size += area * block_size.unwrap() as i64;
        }
        total_size
    }
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
}
impl AssetLifecycler for TextureLifecycler
{
    type Asset = Texture;

    fn load(&self, mut request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>>
    {
        let tex_file: TextureFile = request.deserialize()?;

        let asskey = request.asset_key;
        let texel_bytes = request.read_to_end()?;
        let gpu_tex = self.renderer.device().create_texture_with_data(
            self.renderer.queue(),
            &TextureDescriptor
            {
                label: debug_label!(&format!("{:?}", asskey)),
                size: Extent3d
                {
                    width: tex_file.width,
                    height: tex_file.height,
                    depth_or_array_layers: tex_file.depth,
                },
                mip_level_count: tex_file.mip_count as u32,
                sample_count: 1,
                dimension:
                if tex_file.depth > 1 { TextureDimension::D3 }
                else if tex_file.height > 1 { TextureDimension::D2 }
                else { TextureDimension::D1 },
                format: match tex_file.pixel_format
                {
                    TextureFilePixelFormat::R8 => TextureFormat::R8Unorm,
                    TextureFilePixelFormat::Rg8 => TextureFormat::Rg8Unorm,
                    TextureFilePixelFormat::Rgba8 => TextureFormat::Rgba8Unorm,
                    TextureFilePixelFormat::Rgba8Srgb => TextureFormat::Rgba8UnormSrgb,
                },
                usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            TextureDataOrder::LayerMajor,
            texel_bytes);

        let view = gpu_tex.create_view(&TextureViewDescriptor
        {
            label: None,
            format: None,
            dimension: None,
            usage: None,
            aspect: Default::default(),
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        let tex = Texture
        {
            gpu_tex,
            gpu_view: view,
        };

        let bytes = tex.total_device_bytes();
        self.device_bytes.fetch_add(bytes, Ordering::Relaxed); // relaxed ok here?

        Ok(tex)
    }
}

impl DebugGui for TextureLifecycler
{
    fn display_name(&self) -> &str { "Textures" }

    fn debug_gui(&self, ui: &mut Ui)
    {
        ui.label(format!("Total device bytes: {:#.2}B", format_binary!(self.device_bytes.load(Ordering::Relaxed))));
    }
}