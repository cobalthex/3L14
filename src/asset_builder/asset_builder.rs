use std::hash::Hasher;
use std::io::{Read, Seek};
use unicase::UniCase;

pub enum AssetBuildError
{
    InvalidInputData,
}

pub struct BuiltAsset
{
    // dependencies
    // output data
}

pub trait SourceInputRead: Read + Seek { }
impl<T: Read + Seek> SourceInputRead for T { }

pub struct SourceInput
{
    asset_path: UniCase<String>,
    input: Box<dyn SourceInputRead>,
}

pub trait AssetBuilder
{
    // Returns a unique hash to version the output of this particular builder
    fn format_hash<H: Hasher>(hasher: &mut H);
    // Returns a unique hash to version this builder, even if the output format has not changed
    fn builder_hash<H: Hasher>(hasher: &mut H);

    fn build_asset(&mut self, input: SourceInput) -> Result<BuiltAsset, AssetBuildError>;
}