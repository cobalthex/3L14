use serde::de::{DeserializeOwned, Error};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Debug, Formatter};
use std::io::{Read, Write};
use std::path::PathBuf;
use base64::Engine;
use crate::{AssetKey, AssetKeySourceId};

// TODO: move to somewhere more central?
#[derive(Debug)]
pub enum MetaFileError
{
    NotAFile,
    FileReadError(std::io::Error),
    FileWriteError(std::io::Error),
    TomlReadError(toml::de::Error),
    TomlWriteError(toml::ser::Error),
}

// TODO: is this really better than just fs_read_to_string() and toml parse?
pub trait TomlRead: DeserializeOwned
{
    fn load(reader: &mut impl Read) -> Result<Self, MetaFileError>
    {
        let mut buf = String::new();
        reader.read_to_string(&mut buf).map_err(MetaFileError::FileReadError)?;
        toml::from_str(&buf).map_err(MetaFileError::TomlReadError)
    }
}
pub trait TomlWrite: Serialize
{
    fn save(&self, prettify: bool, writer: &mut impl Write) -> Result<(), MetaFileError>
    {
        let toml = if prettify
        {
            toml::ser::to_string_pretty(self).map_err(MetaFileError::TomlWriteError)?
        }
        else
        {
            toml::ser::to_string(self).map_err(MetaFileError::TomlWriteError)?
        };
        writer.write_all(toml.as_bytes()).map_err(MetaFileError::FileWriteError)
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

#[derive(Debug, Serialize, Deserialize)]
pub struct SourceMetadata
{
    pub source_id: AssetKeySourceId,
    pub version_hash: VersionHash,
    // is_dependent? (don't self build, omit source_id)
    pub build_config: toml::Value, // default to empty table?
    pub notes: Option<String>, // author/user provided notes
}
impl TomlRead for SourceMetadata { }
impl TomlWrite for SourceMetadata { }

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct VersionHash(pub u64);
impl Debug for VersionHash
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { std::fmt::LowerHex::fmt(&self.0, f) }
}
// custom serialize/deserialize b/c TOML doesn't support u64
impl Serialize for VersionHash
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>
    {
        let str = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(self.0.to_le_bytes());
        str.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for VersionHash
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error>
    {
        let inp = String::deserialize(deserializer)?;
        let mut dec = [0u8; size_of::<u64>()];
        match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode_slice(inp, &mut dec)
        {
            Ok(_) => Ok(VersionHash(u64::from_le_bytes(dec))),
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
    pub version_hash: VersionHash,
    pub dependencies: Box<[AssetKey]>,
}
impl TomlRead for AssetMetadata { }
impl TomlWrite for AssetMetadata { }
