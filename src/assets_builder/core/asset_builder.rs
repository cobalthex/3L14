use std::error::Error;
use game_3l14::engine::assets::AssetKeyBaseId;
use serde::{Deserialize, Serialize};
use std::io::{Seek, Write};
use super::*;

pub trait BuildOutputWrite: Write + Seek { }
impl<T: Write + Seek> BuildOutputWrite for T { }

pub type VersionStrings = &'static [&'static [u8]];

pub trait AssetBuilder: 'static
{
    // A list of file extensions (omit . prefix) that this builder can read from
    fn supported_input_file_extensions(&self) -> &'static [&'static str];

    // Returns a list of binary strings, one per version, used to generate a hash versioning the builder
    fn builder_version(&self) -> VersionStrings;
    // Returns a list of binary strings, one per version, used to generate a hash versioning the format of the outputted asset data
    fn format_version(&self) -> VersionStrings;

    // Build the source data into one or more outputted assets
    fn build_assets(&self, input: SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>;
}

#[derive(Serialize, Deserialize)]
pub struct SourceMetadata
{
    pub base_id: AssetKeyBaseId,

    // key value pairs to pass into builder?
}
