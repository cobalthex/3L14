use serde::de::{DeserializeOwned, Error};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Debug, Formatter};
use std::io::{Read, Write};
use std::path::PathBuf;
use base64::Engine;
use crate::{AssetKey, AssetKeySourceId};

// TODO: is this really better than just fs_read_to_string() and toml parse?
pub trait TomlRead: DeserializeOwned
{
    fn load(reader: &mut impl Read) -> Result<Self, Box<dyn std::error::Error>>
    {
        let mut buf = String::new();
        reader.read_to_string(&mut buf)?;
        Ok(toml::from_str(&buf)?)
    }
}
pub trait TomlWrite: Serialize
{
    fn save(&self, prettify: bool, writer: &mut impl Write) -> Result<(), Box<dyn std::error::Error>>
    {
        let toml = if prettify
        {
            toml::ser::to_string_pretty(self).expect("Failed to (pretty) serialize TOML")
        }
        else
        {
            toml::ser::to_string(self).expect("Failed to serialize TOML")
        };
        writer.write_all(toml.as_bytes())?;
        Ok(())
    }
}

// used only for scanning, field names (and ideally order) must match SourceMetadata
// not guaranteed to work with all serialization formats (TOML supported)
#[derive(Deserialize)]
pub struct SourceMetadataStub
{
    pub source_id: AssetKeySourceId,
}
impl TomlRead for SourceMetadataStub { }

#[derive(Serialize, Deserialize)]
pub struct SourceMetadata
{
    pub source_id: AssetKeySourceId,
    // is_dependent? (don't self build, omit source_id)
    pub build_config: toml::Value,
}
impl TomlRead for SourceMetadata { }
impl TomlWrite for SourceMetadata { }

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

// TODO: move ^ into asset builder and make v parseable w/out?

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetMetadata
{
    pub key: AssetKey,
    pub name: Option<String>,
    pub source_path: PathBuf, // relative to the sources directory
    pub build_timestamp: chrono::DateTime<chrono::Utc>,
    pub builder_hash: BuilderHash,
    pub format_hash: BuilderHash,
    pub dependencies: Box<[AssetKey]>,
}
impl TomlRead for AssetMetadata { }
impl TomlWrite for AssetMetadata { }
