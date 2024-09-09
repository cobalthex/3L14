use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::engine::assets::AssetKey;

pub type BuilderHash = u64;

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetMetadata
{
    pub key: AssetKey,
    pub build_timestamp: u64, // unix timestamp in milliseconds
    pub builder_hash: BuilderHash, // taken from the builder
    pub format_hash: BuilderHash, // taken from the builder
    pub source_path: PathBuf, // relative to the sources directory
}