use super::*;
use erased_serde::{Deserializer, Serialize};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::error::Error;
use std::io::{Seek, Write};

pub trait BuildOutputWrite: Write + Seek { }
impl<T: Write + Seek> BuildOutputWrite for T { }

pub type VersionStrings = &'static [&'static [u8]];

pub trait AssetBuilderMeta
{
    // A list of file extensions (omit . prefix) that this builder can read from
    fn supported_input_file_extensions() -> &'static [&'static str];

    // Returns a list of binary strings, one per version, used to generate a hash versioning the builder
    fn builder_version() -> VersionStrings;
    // Returns a list of binary strings, one per version, used to generate a hash versioning the format of the outputted asset data
    fn format_version() -> VersionStrings;
}

pub(super) trait ErasedAssetBuilder // virtual base trait?
{
    // Build the source data into one or more outputted assets
    fn build_assets(&self, input: SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>;
}

pub trait AssetBuilder: 'static
{
    type Config: Default + Serialize + DeserializeOwned;

    // Build the source data into one or more outputted assets
    fn build_assets(&self, config: Self::Config, input: SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>;
}
impl<C: Default + Serialize + DeserializeOwned> ErasedAssetBuilder for dyn AssetBuilder<Config=C>
{
    fn build_assets(&self, mut input: SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        let config = match &mut input.raw_config
        {
            None => C::default(),
            Some(rc) => erased_serde::deserialize(rc.as_mut())?,
        };
        AssetBuilder::build_assets(self, config, input, outputs)
    }
}