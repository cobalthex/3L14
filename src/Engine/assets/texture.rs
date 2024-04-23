use std::collections::HashMap;
use std::thread::Builder;
use arc_swap::ArcSwap;
use parking_lot::Mutex;
use super::*;

pub struct Texture<'a>
{
    gpu_handle: wgpu::Texture,
    desc: wgpu::TextureDescriptor<'a>,
}  
impl<'a> Texture<'a>
{
    pub fn gpu_handle(&self) -> &wgpu::Texture { &self.gpu_handle }
    pub fn desc(&self) -> &wgpu::TextureDescriptor { &self.desc }
}
impl<'a> Asset for Texture<'a>
{
}

pub type TexturePayloadArc<'a> = ArcSwap<AssetPayload<Texture<'a>>>;

pub struct TextureLifecycler<'a>
{
    device: &'a wgpu::Device,
    textures: Mutex<HashMap<AssetKey, TexturePayloadArc<'a>>>,
}
impl<'a> TextureLifecycler<'a>
{
    pub fn new(device: &'a wgpu::Device) -> Self
    {
        Self
        {
            device,
            textures: Default::default(),
        }
    }
}
impl<'a> AssetLifecycler<Texture<'a>> for TextureLifecycler<'a>
{
    fn get_or_create(&self, request: AssetLoadRequest<Texture>)
    {
        let mut locked = self.textures.lock();
        let entry = locked.entry(request.key).or_insert_with(||
        {
            todo!()
        });
        // todo: write output
    }

    // todo: create manual

    fn stats(&self) -> AssetLifecyclerStats
    {
        AssetLifecyclerStats
        {
            active_count: 0,
        }
    }
}