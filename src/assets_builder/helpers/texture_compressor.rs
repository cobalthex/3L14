pub use intel_tex::RgbaSurface;
use proc_macros_3l14::Flags;

pub enum CompressionQuality
{
    Low,
    Standard,
    HighDetail,
    Lossless,
}

pub enum TextureType
{
    Background,
    Model,
    NormalMap,
    Detail,
    UI,
    Text,
}

#[repr(u8)]
#[derive(Flags, Clone, Copy, PartialEq)]
pub enum ChannelMapping
{
    Red   = 0b0001,
    Green = 0b0010,
    Blue  = 0b0100,
    Alpha = 0b1000,
}

pub struct TextureInput<'t>
{
    pub texels: RgbaSurface<'t>,
    pub channels: ChannelMapping,
}

pub struct TextureCompressor;
impl TextureCompressor
{
    pub fn guess_quality(&self, input: &TextureInput) -> CompressionQuality
    {

        todo!()
    }

    pub fn compress(&self, input: &TextureInput, quality: CompressionQuality) -> Vec<u8>
    {
        match quality
        {
            CompressionQuality::Low =>
            {
                todo!()
            }
            CompressionQuality::Standard =>
            {
                todo!()
            }
            CompressionQuality::HighDetail =>
            {
                let settings = match input.channels.has_flag(ChannelMapping::Alpha)
                {
                    true => intel_tex::bc7::alpha_basic_settings(),
                    false => intel_tex::bc7::opaque_basic_settings(),
                };
                intel_tex::bc7::compress_blocks(todo!(), &input.texels)
            }
            CompressionQuality::Lossless =>
            {
                todo!()
            }
        }
    }
}