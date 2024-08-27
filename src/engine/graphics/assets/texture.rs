use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use arc_swap::ArcSwap;
use egui::Ui;
use png::DecodingError;
use wgpu::{Extent3d, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor};
use wgpu::util::{DeviceExt, TextureDataOrder};
use crate::engine::alloc_slice::alloc_slice_uninit;
use crate::engine::graphics::Renderer;
use crate::format_bytes;

use crate::engine::assets::{Asset, AssetLifecycler, AssetLoadRequest, AssetPayload};
use crate::engine::graphics::debug_gui::DebugGui;

pub struct Texture
{
    gpu_tex: wgpu::Texture,
    gpu_view: wgpu::TextureView,
    desc: wgpu::TextureDescriptor<'static>,
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
impl Asset for Texture { }

pub type TexturePayloadArc = ArcSwap<AssetPayload<Texture>>;

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

    fn try_import_png(&self, input: &mut dyn Read) -> Result<Texture, DecodingError>
    {
        let png = png::Decoder::new(input);
        let mut png_reader = png.read_info()?;
        let mut png_buf = unsafe { alloc_slice_uninit(png_reader.output_buffer_size()).unwrap() }; // catch error?
        let png_info = png_reader.next_frame(&mut png_buf)?;

        let tex = self.create(png_info.width, png_info.height, &png_buf[..png_info.buffer_size()]);
        Ok(tex)
    }
}
impl AssetLifecycler for TextureLifecycler
{
    type Asset = Texture;
    fn load(&self, request: AssetLoadRequest) -> AssetPayload<Self::Asset>
    {
        // TESTING
        let gltf_texture = unsafe
        {
            let raw: *mut crate::engine::graphics::GltfTexture = &*request.input as *const _ as *mut _;
            &*raw
        };
        let tex = self.create(gltf_texture.width, gltf_texture.height, gltf_texture.texel_data.as_slice());

        AssetPayload::Available(tex)
        // asset system handles lookups
        // match self.try_import_png(request.input.as_mut())
        // {
        //     Ok(tex) =>
        //     {
        //         request.finish(tex);
        //     }
        //     Err(e) =>
        //     {
        //         eprintln!("Failed to create texture: {e}");
        //         request.error(AssetLoadError::ParseError);
        //     }
        // }
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

impl<'a> DebugGui<'a> for TextureLifecycler
{
    fn name(&self) -> &'a str { "Textures" }

    fn debug_gui(&self, ui: &mut Ui)
    {
        ui.label(format!("Total device bytes: {:.1}", format_bytes!(self.device_bytes.load(Ordering::Relaxed))));
    }
}