use proc_macros_3l14::Flags;

// TODO: go back to intel_tex_2? (ISPC is deprecated)

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
#[derive(Flags)]
pub enum ChannelMapping
{
    Red   = 0b0001,
    Green = 0b0010,
    Blue  = 0b0100,
    Alpha = 0b1000,
}

pub struct TextureInput
{
    pub texels: (), // todo
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
                todo!()
            }
            CompressionQuality::Lossless =>
            {
                todo!()
            }
        }
    }
}