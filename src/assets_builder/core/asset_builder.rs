use game_3l14::engine::assets::AssetKeyBaseId;
use serde::{Deserialize, Serialize};
use std::io;
use std::io::{Seek, Write};
use super::*;

#[derive(Debug)]
pub enum BuildError
{
    InvalidSourcePath, // lies outside the sources root
    NoBuilderForSource,
    SourceIOError(io::Error),
    SourceMetaIOError(io::Error),
    SourceMetaSerializeError(ron::Error),
    InvalidInputData,
    TooManyDerivedIDs,
    OutputIOError(std::io::Error),
    OutputSerializeError(postcard::Error),
} // TODO: impl Error trait?

pub trait BuildOutputWrite: Write + Seek { }
impl<T: Write + Seek> BuildOutputWrite for T { }

pub trait AssetBuilder: 'static
{
    // A list of file extensions (omit . prefix) that this builder can read from
    fn supported_input_file_extensions(&self) -> &'static [&'static str];

    // Build the source data into one or more outputted assets
    fn build_assets(&self, input: SourceInput, outputs: &mut BuildOutputs) -> Result<(), BuildError>;
}

#[derive(Serialize, Deserialize)]
pub struct SourceMetadata
{
    pub base_id: AssetKeyBaseId,

    // key value pairs to pass into builder?
}
