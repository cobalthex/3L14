use crate::engine::asset::AssetKey;
use base64::Engine;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Debug, Formatter};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct BuilderHash(pub u64);
impl Debug for BuilderHash
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { std::fmt::LowerHex::fmt(&self.0, f) }
}
// custom serialize/deserialize b/c TOML doesn't support u64
impl Serialize for BuilderHash
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>
    {
        let str = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(self.0.to_le_bytes());
        str.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for BuilderHash
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error>
    {
        let inp = String::deserialize(deserializer)?;
        let mut dec = [0u8; size_of::<u64>()];
        match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode_slice(inp, &mut dec)
        {
            Ok(_) => Ok(BuilderHash(u64::from_le_bytes(dec))),
            Err(e) => Err(D::Error::custom(e)),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetMetadata
{
    pub key: AssetKey,
    pub build_timestamp: chrono::DateTime<chrono::Utc>,
    pub source_path: PathBuf, // relative to the sources directory
    pub builder_hash: BuilderHash,
    pub format_hash: BuilderHash,
    pub dependencies: Box<[AssetKey]>,
}