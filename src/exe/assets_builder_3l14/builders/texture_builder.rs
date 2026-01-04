use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::io::{BufReader, Write};
use enumflags2::bitflags;
use image::{ColorType, DynamicImage, GenericImageView, ImageReader, ImageResult};
use serde::{Deserialize, Serialize};
use asset_3l14::AssetTypeId;
use graphics_3l14::assets::{TextureFile, TextureFilePixelFormat};
use crate::core::{AssetBuilder, AssetBuilderMeta, BuildOutputs, SourceInput, VersionBuilder};

// TODO: go back to intel_tex_2? (ISPC is deprecated)

#[derive(Default, Serialize, Deserialize)]
pub enum CompressionQuality
{
    Low,
    #[default]
    Standard,
    HighDetail,
    Lossless,
}

#[derive(Default, Serialize, Deserialize)]
pub enum TextureUsage
{
    Background,
    #[default]
    Model,
    NormalMap,
    Detail,
    UI,
    Text,
}

#[bitflags]
#[repr(u8)]
#[derive(Copy, Clone)]
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

#[derive(Default, Serialize, Deserialize)]
pub struct TextureBuildConfig
{
    pub quality: CompressionQuality,
    pub usage: TextureUsage,
}

pub struct TextureBuilder;
impl TextureBuilder
{
    pub fn new() -> Self
    {
        Self
    }
}
impl AssetBuilderMeta for TextureBuilder
{
    fn supported_input_file_extensions() -> &'static [&'static str]
    {
        // TODO: more input formats
        &["png"]
    }

    fn builder_version(vb: &mut VersionBuilder)
    {
        vb.push(b"Texture builder - initial");
        vb.push_prehashed(1);
    }
}
impl AssetBuilder for TextureBuilder
{
    type BuildConfig = TextureBuildConfig;

    fn build_assets(&self, config: Self::BuildConfig, input: &mut SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        let reader = ImageReader::new(BufReader::new(input))
        .with_guessed_format()?;
        let image = reader.decode()?;

        let format = match image.color()
        {
            ColorType::Rgba8 => TextureFilePixelFormat::Rgba8,
            // TODO: do as_image_rgba8?
            _ => return Err(Box::new(TextureBuilderError::UnsupportedPixelFormat)),
        };

        outputs.add_output(AssetTypeId::Texture, |output|
        {
            output.serialize(&TextureFile
            {
                width: image.width(),
                height: image.height(),
                depth: 1,
                mip_count: 1,
                mip_offsets: [0; _],
                pixel_format: TextureFilePixelFormat::Rgba8,
            })?;
            
            output.write_all(image.as_bytes())?;
            
            Ok(())
        })?;

        Ok(())
    }
}

#[derive(Debug)]
enum TextureBuilderError
{
    UnsupportedPixelFormat,
}
impl Display for TextureBuilderError
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { Debug::fmt(self, f) }
}
impl Error for TextureBuilderError { }