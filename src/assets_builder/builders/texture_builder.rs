use crate::asset_builder::{AssetBuilder, BuildError, BuildOutput, BuildOutputs, SourceInput, VersionStrings};

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

    fn build_assets(&self, input: SourceInput, output: &mut BuildOutputs) -> Result<(), BuildError>
    {
        output.write_output()
        
        Ok(())
        // if input.file_extension.eq(UniCase::new("png"))
        // {
        //     let png = png::Decoder::new(input);
        //     let mut png_reader = png.read_info()?;
        //     let mut png_buf = unsafe { alloc_slice_uninit(png_reader.output_buffer_size()).unwrap() }; // catch error?
        //     let png_info = png_reader.next_frame(&mut png_buf)?;
        // }
        //
        // Err(BuildError::InvalidInputData)
    }
}

struct TextureBuildOutput
{

}