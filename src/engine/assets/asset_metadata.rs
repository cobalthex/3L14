use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::engine::assets::AssetKey;

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetMetadata
{
    pub key: AssetKey,
    pub build_timestamp: u64, // unix timestamp in milliseconds
    pub source_path: PathBuf, // relative to the sources directory
    pub dependencies: Box<[AssetKey]>,
}