use crate::assets_builder::{BuildOutputs, SourceInput};
use game_3l14::engine::assets::AssetKeyBaseId;
use serde::{Deserialize, Serialize};
use std::io::{Seek, Write};

#[derive(Debug)]
pub enum BuildError
{
    InvalidSourcePath, // lies outside of the sources root
    NoBuilderForSource,
    SourceIOError(std::io::Error),
    SourceMetaIOError(std::io::Error),
    SourceMetaSerializeError(ron::Error),
    InvalidInputData,
    TooManyDerivedIDs,
    OutputIOError(std::io::Error),
    OutputSerializeError(postcard::Error),
}

pub trait BuildOutputWrite: Write + Seek { }
impl<T: Write + Seek> BuildOutputWrite for T { }

// A list of binary strings, one for each version, to be hashed together
pub type VersionStrings = &'static [&'static [u8]];

pub trait AssetBuilder: 'static
{
    // A list of file extensions (omit . prefix) that this builder can read from
    fn supported_input_file_extensions(&self) -> &'static [&'static str];

    // Returns a list of binary strings, one per version, used to generate a hash versioning the builder
    fn builder_version(&self) -> VersionStrings;
    // Returns a list of binary strings, one per version, used to generate a hash versioning the format of the outputted asset data
    fn format_version(&self) -> VersionStrings;

    // TODO: stream/iter build outputs?

    // Build the source data into one or more outputted assets
    fn build_assets(&self, input: SourceInput, outputs: &mut BuildOutputs) -> Result<(), BuildError>;
}

#[derive(Serialize, Deserialize)]
pub struct SourceMetadata
{
    pub base_id: AssetKeyBaseId,

    // key value pairs to pass into builder?
}
