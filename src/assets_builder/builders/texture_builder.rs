use crate::core::{AssetBuilder, BuildOutputs, SourceInput, VersionStrings};
use game_3l14::engine::alloc_slice::alloc_slice_uninit;
use game_3l14::engine::assets::AssetTypeId;
use game_3l14::engine::graphics::assets::{TextureFile, TextureFilePixelFormat, MAX_MIP_COUNT};
use image::{ColorType, EncodableLayout, GenericImageView};
use std::error::Error;
use std::io::{BufReader, Write};
use unicase::UniCase;

pub struct TextureBuilder;
impl AssetBuilder for TextureBuilder
{
    fn supported_input_file_extensions(&self) -> &'static [&'static str]
    {
        &["dds", "png"]
    }

    fn builder_version(&self) -> VersionStrings
    {
        &[
            b"Initial"
        ]
    }

    fn format_version(&self) -> VersionStrings
    {
        &[
            b"Initial"
        ]
    }

    fn build_assets(&self, mut input: SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        let mut output = outputs.add_output(AssetTypeId::Texture)?;

        if input.file_extension() == &UniCase::new("png")
        {
            let img = image::ImageReader::new(BufReader::new(input)).with_guessed_format()?.decode()?;

            // TODO: mipmaps
            let tex_file = TextureFile
            {
                width: img.width(),
                height: img.height(),
                depth: 1,
                mip_count: 1,
                mip_offsets: [0; MAX_MIP_COUNT],
                pixel_format: match img.color()
                {
                    ColorType::L8 => TextureFilePixelFormat::R8,
                    ColorType::La8 => TextureFilePixelFormat::Rg8,
                    ColorType::Rgb8 => TextureFilePixelFormat::Rgb8,
                    ColorType::Rgba8 => TextureFilePixelFormat::Rgba8,
                    ColorType::L16 => TextureFilePixelFormat::R8,
                    ColorType::La16 => TextureFilePixelFormat::Rg8,
                    ColorType::Rgb16 => TextureFilePixelFormat::Rgb8,
                    ColorType::Rgba16 => TextureFilePixelFormat::Rgba8,
                    ColorType::Rgb32F => TextureFilePixelFormat::Rgb8,
                    ColorType::Rgba32F => TextureFilePixelFormat::Rgba8,
                    _ => { todo!("Unknown pixel format") } // todo: non fatal error
                },
            };
            output.serialize(&tex_file)?;

            match tex_file.pixel_format
            {
                TextureFilePixelFormat::R8 => output.write_all(img.into_luma8().as_bytes())?,
                TextureFilePixelFormat::Rg8 => output.write_all(img.into_luma_alpha8().as_bytes())?,
                TextureFilePixelFormat::Rgb8 => output.write_all(img.into_rgb8().as_bytes())?,
                TextureFilePixelFormat::Rgba8 => output.write_all(img.into_rgba8().as_bytes())?,
                TextureFilePixelFormat::Rgba8Srgb => output.write_all(img.into_rgba8().as_bytes())?, // TODO
            }
        }

        output.finish()?;

        Ok(())
        // if input.file_extension.eq(UniCase::new("png"))
        // {;
        // }
        //
        // Err(BuildError::InvalidInputData)
    }
}
