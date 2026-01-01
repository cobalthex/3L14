use super::*;
use asset_3l14::BuilderHash;
use metrohash::MetroHash64;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::error::Error;
use std::hash::Hasher;
use std::io::{Seek, Write};

pub trait BuildOutputWrite: Write + Seek { }
impl<T: Write + Seek> BuildOutputWrite for T { }

pub trait AssetBuilderMeta
{
    // A list of file extensions (omit . prefix) that this builder can read from
    fn supported_input_file_extensions() -> &'static [&'static str];

    // Returns a list of binary strings, one per version, used to generate a hash versioning the builder
    fn builder_version(vb: &mut VersionBuilder);
    // Returns a list of binary strings, one per version, used to generate a hash versioning the format of the outputted asset data
    fn format_version(vb: &mut VersionBuilder);
}

pub(super) type ErasedBuildConfig = toml::Value; // leaky abstraction

pub(super) trait ErasedAssetBuilder // virtual base trait?
{
    // Build the source data into one or more outputted assets
    fn build_assets(&self, erased_config: ErasedBuildConfig, input: &mut SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>;

    fn default_config(&self) -> ErasedBuildConfig;
}

pub trait AssetBuildConfig: Default + Serialize + DeserializeOwned { }
impl<T: Default + Serialize + DeserializeOwned> AssetBuildConfig for T { }

pub trait AssetBuilder
{
    type BuildConfig: AssetBuildConfig;

    // Build the source data into one or more outputted assets
    fn build_assets(&self, config: Self::BuildConfig, input: &mut SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>;
}
impl<AB: AssetBuilder<BuildConfig=impl AssetBuildConfig>> ErasedAssetBuilder for AB
{
    fn build_assets(&self, erased_config: ErasedBuildConfig, input: &mut SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        let config = erased_config.try_into()?;
        AssetBuilder::build_assets(self, config, input, outputs)
    }

    fn default_config(&self) -> ErasedBuildConfig
    {
        ErasedBuildConfig::try_from(AB::BuildConfig::default()).unwrap()
    }
}

pub struct VersionBuilder(MetroHash64, usize);
impl VersionBuilder
{
    #[inline] #[must_use]
    pub(super) fn new(seed: u64) -> Self
    {
        Self(MetroHash64::with_seed(seed), 0)
    }
    #[inline] #[must_use]
    pub(super) fn build(self) -> BuilderHash
    {
        BuilderHash(self.build_raw())
    }
    #[inline] #[must_use]
    pub(super) fn build_raw(self) -> u64
    {
        assert_ne!(self.1, 0, "There must be at least one entry to build a version hash from");
        self.0.finish()
    }

    pub fn push(&mut self, bstr: &[u8])
    {
        self.1 += 1;
        self.0.write(bstr);
    }
    pub fn append(&mut self, bstrs: &[&[u8]])
    {
        self.1 += bstrs.len();
        bstrs.iter().for_each(|s| { self.0.write(s); });
    }
}
