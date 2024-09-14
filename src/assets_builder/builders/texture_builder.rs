use crate::core::{AssetBuilder, BuildOutputs, SourceInput, VersionStrings};
use game_3l14::engine::alloc_slice::alloc_slice_uninit;
use game_3l14::engine::assets::AssetTypeId;
use game_3l14::engine::graphics::assets::{TextureFile, TextureFilePixelFormat, MAX_MIP_COUNT};
use png::ColorType;
use std::error::Error;
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
            let png = png::Decoder::new(&mut input);
            let mut png_reader = png.read_info()?;
            let mut png_buf = unsafe { alloc_slice_uninit(png_reader.output_buffer_size()).unwrap() }; // catch error?

            // atlas frames?
            let png_info = png_reader.next_frame(&mut png_buf)?;

            let tex_file = TextureFile
            {
                width: png_info.width,
                height: png_info.height,
                depth: 1,
                mip_count: 1, // mipmap gen?
                mip_offsets: [0; MAX_MIP_COUNT],
                pixel_format: match png_info.color_type
                {
                    ColorType::Grayscale => TextureFilePixelFormat::R8,
                    _ => TextureFilePixelFormat::Rgba8,
                    // ColorType::Rgb => {}
                    // ColorType::Indexed => {}
                    // ColorType::GrayscaleAlpha => {}
                    // ColorType::Rgba => TextureFilePixelFormat::Rgba8,
                },
            };

            output.serialize(&tex_file)?;
            std::io::copy(&mut input, &mut output)?;
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
