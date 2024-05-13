use std::sync::atomic::{AtomicI64, Ordering};
use arc_swap::ArcSwap;
use png::DecodingError;
use wgpu::{Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor};
use wgpu::util::{DeviceExt, TextureDataOrder};
use crate::engine::alloc_slice::alloc_slice_uninit;
use crate::engine::graphics::Renderer;
use super::*;

pub struct Texture
{
    gpu_handle: wgpu::Texture,
    desc: wgpu::TextureDescriptor<'static>,
}  
impl Texture
{
    pub fn gpu_handle(&self) -> &wgpu::Texture { &self.gpu_handle }
    pub fn desc(&self) -> &wgpu::TextureDescriptor { &self.desc }

    pub fn total_device_bytes(&self) -> i64
    {
        let mut total_size = 0i64;
        for mip in 0..self.desc.mip_level_count
        {
            let size = self.desc.mip_level_size(mip).unwrap().physical_size(self.desc.format);
            let area = (size.width as i64) * (size.height as i64) * (size.depth_or_array_layers as i64);
            // TODO
            //total_size += area * self.desc.format.block_copy_size(Some(TextureAspect::All))
        }
        total_size
    }
    
    pub fn create_view(&self) -> wgpu::TextureView
    {
        self.gpu_handle.create_view(&TextureViewDescriptor
        {
            label: None,
            format: None,
            dimension: None,
            aspect: Default::default(),
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        })
    }
}
impl Asset for Texture
{
}

pub type TexturePayloadArc = ArcSwap<AssetPayload<Texture>>;

pub struct TextureLifecycler<'r>
{
    renderer: &'r Renderer<'r>,
    device_bytes: AtomicI64,
}
impl<'r> TextureLifecycler<'r>
{
    pub fn new(renderer: &'r Renderer<'r>) -> Self
    {
        Self
        {
            renderer,
            device_bytes: AtomicI64::new(0)
        }
    }

    fn try_import_png(&self, input: &mut dyn Read) -> Result<Texture, DecodingError>
    {
        let png = png::Decoder::new(input);
        let mut png_reader = png.read_info()?;
        let mut png_buf = unsafe { alloc_slice_uninit(png_reader.output_buffer_size()).unwrap() }; // catch error?
        let png_info = png_reader.next_frame(&mut png_buf)?;

        let dtor = TextureDescriptor
        {
            label: None, // TODO
            size: Extent3d
            {
                width: png_info.width,
                height: png_info.height,
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
            &png_buf[..png_info.buffer_size()]);

        let payload = Texture
        {
            gpu_handle: tex,
            desc: dtor,
        };

        let bytes = payload.total_device_bytes();
        self.device_bytes.fetch_add(bytes, Ordering::Relaxed); // relaxed ok here?

        Ok(payload)
    }
}
impl<'a> AssetLifecycler<Texture> for TextureLifecycler<'a>
{
    fn create_or_update(&self, mut request: AssetLoadRequest<Texture>)
    {
        // asset system handles lookups
        match self.try_import_png(request.input.as_mut())
        {
            Ok(tex) =>
            {
                request.finish(tex);
            }
            Err(e) =>
            {
                eprintln!("Failed to create texture: {e}");
                request.error(AssetLoadError::ParseError);
            }
        }
    }
    //
    // fn before_drop(&self, payload: AssetPayload<Texture<'a>>)
    // {
    //     if let AssetPayload::Available(tex) = payload
    //     {
    //         let bytes = tex.total_device_bytes();
    //         let old_val = self.device_bytes.fetch_sub(bytes, Ordering::AcqRel);
    //         debug_assert!((old_val - bytes) >= 0);
    //     }
    // }
}

impl<'r> DebugGui<'r> for TextureLifecycler<'r>
{
    fn name(&self) -> &'r str { "Textures" }

    fn debug_gui(&self, ui: &mut Ui)
    {
    }
}