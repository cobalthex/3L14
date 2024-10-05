use super::*;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
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

pub(super) type ErasedBuildConfig = toml::Value; // leaky abstraction

pub(super) trait ErasedAssetBuilder // virtual base trait?
{
    // Build the source data into one or more outputted assets
    fn build_assets(&self, erased_config: ErasedBuildConfig, input: SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>;

    fn default_config(&self) -> ErasedBuildConfig;
}

pub trait AssetBuildConfig: Default + Serialize + DeserializeOwned { }
impl<T: Default + Serialize + DeserializeOwned> AssetBuildConfig for T { }

pub trait AssetBuilder
{
    type BuildConfig: AssetBuildConfig;

    // Build the source data into one or more outputted assets
    fn build_assets(&self, config: Self::BuildConfig, input: SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>;
}
impl<AB: AssetBuilder<BuildConfig=impl AssetBuildConfig>> ErasedAssetBuilder for AB
{
    fn build_assets(&self, erased_config: ErasedBuildConfig, input: SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        let config = erased_config.try_into()?;
        AssetBuilder::build_assets(self, config, input, outputs)
    }

    fn default_config(&self) -> ErasedBuildConfig
    {
        ErasedBuildConfig::try_from(AB::BuildConfig::default()).unwrap()
    }
}
