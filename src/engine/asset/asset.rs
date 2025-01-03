use crate::const_assert;
use crate::engine::asset::AssetTypeId;
use crate::engine::utils::format_width_hex_bytes;
use bitcode::{Decode, Encode};
use metrohash::MetroHash64;
use rand::RngCore;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Debug, Display, Formatter, Write};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use unicase::UniCase;
use std::cmp::PartialEq;

pub const ASSET_FILE_EXTENSION: UniCase<&'static str> = UniCase::unicode("ass");
pub const ASSET_META_FILE_EXTENSION: UniCase<&'static str> = UniCase::unicode("mass");

pub trait Asset: Sync + Send + 'static
{
    fn asset_type() -> AssetTypeId;

    // Have all dependencies of this asset been loaded? (always true if no dependencies)
    fn all_dependencies_loaded(&self) -> bool { true }
}
pub trait HasAssetDependencies
{
    fn asset_dependencies_loaded(&self) -> bool;
}

pub trait AssetPath: AsRef<str> + Hash + Display + Debug { }
impl<T> AssetPath for T where T: AsRef<str> + Hash + Display + Debug { }

type AssetKeyDerivedIdRepr = u16;
type AssetKeySynthHashRepr = u64;
type AssetKeySourceIdRepr = u64;
type AssetKeyRepr = u64;

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct AssetKeyDerivedId(AssetKeyDerivedIdRepr); // only 15 bits are used.
impl AssetKeyDerivedId
{
    #[cfg(test)]
    pub const fn test() -> Self { Self(0) }
}
// Used to generate new derived IDs, next returns the existing value and increments self
impl Iterator for AssetKeyDerivedId
{
    type Item = Self;
    fn next(&mut self) -> Option<Self>
    {
        let rval = *self;
        self.0.checked_add(1).map(|u| { self.0 = u; rval })
    }
}
impl Debug for AssetKeyDerivedId
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        f.write_fmt(format_args!("{:0width$x}", self.0, width = format_width_hex_bytes(AssetKey::DERIVED_ID_BITS)))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AssetKeySourceId(AssetKeySourceIdRepr); // only 100 bits are used.
impl AssetKeySourceId
{
    pub fn generate() -> Self
    {
        let mut bytes = [0u8; size_of::<Self>()];
        rand::thread_rng().fill_bytes(&mut bytes[0..((AssetKey::SOURCE_ID_BITS / 8) as usize)]);
        Self(AssetKeySourceIdRepr::from_le_bytes(bytes))
    }

    #[cfg(test)]
    pub const fn test(n: u8) -> Self
    {
        Self(n as AssetKeySourceIdRepr)
    }
}
// custom serialize/deserialize b/c TOML doesn't support u64
impl Serialize for AssetKeySourceId
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>
    {
        format!("{:0width$x}", self.0, width = format_width_hex_bytes(AssetKey::SOURCE_ID_BITS)).serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for AssetKeySourceId
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error>
    {
        let inp = String::deserialize(deserializer)?;
        match u64::from_str_radix(&inp, 16)
        {
            Ok(u) => Ok(Self(u)),
            Err(e) => Err(D::Error::custom(e)),
        }
    }
}
impl Debug for AssetKeySourceId
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        f.write_fmt(format_args!("{:026x}", self.0))
    }
}


#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AssetKeySynthHash(AssetKeySynthHashRepr);
impl AssetKeySynthHash
{
    pub fn generate(hashable: impl Hash) -> Self
    {
        let mut hasher = MetroHash64::new();
        hashable.hash(&mut hasher);
        let n = hasher.finish();
        Self(n & AssetKey::SYNTH_HASH_MAX)
    }

    #[cfg(test)]
    pub const fn test(n: AssetKeySynthHashRepr) -> Self
    {
        Self(n)
    }
}
impl Serialize for AssetKeySynthHash
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>
    {
        format!("{:0width$x}", self.0, width = format_width_hex_bytes(AssetKey::SYNTH_HASH_BITS)).serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for AssetKeySynthHash
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error>
    {
        let inp = String::deserialize(deserializer)?;
        match AssetKeySynthHashRepr::from_str_radix(&inp, 16)
        {
            Ok(u) => Ok(Self(u)),
            Err(e) => Err(D::Error::custom(e)),
        }
    }
}
impl Debug for AssetKeySynthHash
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        f.write_fmt(format_args!("{:0width$x}", self.0, width = format_width_hex_bytes(AssetKey::SYNTH_HASH_BITS)))
    }
}

// A unique identifier identifying a built asset
// It can either be 'synthetic' whereby the ID is a hash of its contents
// or it can be 'unique' where the ID is composed of a combined unique source and derived ID
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub struct AssetKey(AssetKeyRepr);
impl AssetKey
{
    const TOTAL_BITS: u8 = AssetKeyRepr::BITS as u8;
    
    // ordered high-low bits
    const ASSET_TYPE_BITS: u8 = 12;
    const SYNTH_FLAG_BITS: u8 = 1;
    const SYNTH_HASH_BITS: u8 = Self::TOTAL_BITS - Self::ASSET_TYPE_BITS - Self::SYNTH_FLAG_BITS;
    const DERIVED_ID_BITS: u8 = 15;
    const SOURCE_ID_BITS:  u8 = Self::TOTAL_BITS - Self::ASSET_TYPE_BITS - Self::SYNTH_FLAG_BITS - Self::DERIVED_ID_BITS;

    const SOURCE_KEY_MAX:  u64 = (1 << Self::SOURCE_ID_BITS) - 1;
    const DERIVED_KEY_MAX: u16 = (1 << Self::DERIVED_ID_BITS) - 1;
    const SYNTH_HASH_MAX:  u64 = (1 << Self::SYNTH_HASH_BITS) - 1;
    const SYNTH_FLAG_MAX:  u8  = (1 << Self::SYNTH_FLAG_BITS) - 1;
    const ASSET_TYPE_MAX:  u16 = (1 << Self::ASSET_TYPE_BITS) - 1;

    const SOURCE_KEY_SHIFT:  u8 = 0;
    const DERIVED_KEY_SHIFT: u8 = Self::SOURCE_KEY_SHIFT + Self::SOURCE_ID_BITS;
    const SYNTH_HASH_SHIFT:  u8 = 0;
    const SYNTH_FLAG_SHIFT:  u8 = Self::DERIVED_KEY_SHIFT + Self::DERIVED_ID_BITS;
    const ASSET_TYPE_SHIFT:  u8 = Self::SYNTH_FLAG_SHIFT + Self::SYNTH_FLAG_BITS;

    pub const fn unique(asset_type: AssetTypeId, derived_id: AssetKeyDerivedId, source_id: AssetKeySourceId) -> Self
    {
        const_assert!((AssetKey::TOTAL_BITS / 8) as usize == size_of::<AssetKey>());
        const_assert!(
            (AssetKey::SOURCE_ID_BITS + AssetKey::DERIVED_ID_BITS + AssetKey::SYNTH_FLAG_BITS + AssetKey::ASSET_TYPE_BITS)
            == (AssetKey::TOTAL_BITS)
        );

        debug_assert!((asset_type as u16) < Self::ASSET_TYPE_MAX);
        debug_assert!(derived_id.0 < Self::DERIVED_KEY_MAX);
        debug_assert!(source_id.0 < Self::SOURCE_KEY_MAX);

        let mut u: u64 = (source_id.0 & Self::SOURCE_KEY_MAX) << Self::SOURCE_KEY_SHIFT;
        u |= ((derived_id.0 & Self::DERIVED_KEY_MAX) as u64) << Self::DERIVED_KEY_SHIFT;
        // u |= ((0u8 & Self::SYNTH_FLAG_MAX) as u64) << Self::SYNTH_FLAG_SHIFT;
        u |= (((asset_type as u16) & Self::ASSET_TYPE_MAX) as u64) << Self::ASSET_TYPE_SHIFT;
        Self(u)
    }

    #[inline]
    pub const fn synthetic(asset_type: AssetTypeId, synth_hash: AssetKeySynthHash) -> Self
    {
        const_assert!(
            (AssetKey::SYNTH_HASH_BITS + AssetKey::SYNTH_FLAG_BITS + AssetKey::ASSET_TYPE_BITS)
            == (AssetKey::TOTAL_BITS)
        );

        debug_assert!((asset_type as u16) < Self::ASSET_TYPE_MAX);
        debug_assert!(synth_hash.0 < Self::SYNTH_HASH_MAX);

        let mut u: u64 = (synth_hash.0 & Self::SYNTH_HASH_MAX) << Self::SYNTH_HASH_SHIFT;
        u |= ((1u8 & Self::SYNTH_FLAG_MAX) as u64) << Self::SYNTH_FLAG_SHIFT;
        u |= (((asset_type as u16) & Self::ASSET_TYPE_MAX) as u64) << Self::ASSET_TYPE_SHIFT;
        Self(u)
    }

    #[inline]
    pub const fn asset_type(&self) -> AssetTypeId
    {
        unsafe { std::mem::transmute((self.0 >> Self::ASSET_TYPE_SHIFT) as u16 & Self::ASSET_TYPE_MAX) }
    }
    // Get the derived ID for this asset key, returns 0 if synthetic
    #[inline]
    pub const fn derived_id(&self) -> AssetKeyDerivedId
    {
        let u = (self.0 >> Self::DERIVED_KEY_SHIFT) as u16 & Self::DERIVED_KEY_MAX;
        AssetKeyDerivedId(u * !self.is_synthetic() as u16)
    }
    // Get the source ID for this asset key, returns 0 if synthetic
    #[inline]
    pub const fn source_id(&self) -> AssetKeySourceId
    {
        let u = (self.0 >> Self::SOURCE_KEY_SHIFT) & Self::SOURCE_KEY_MAX;
        AssetKeySourceId(u * !self.is_synthetic() as u64)
    }
    // Get the synthesized hash for this asset key, returns 0 if unique (not synthetic)
    #[inline]
    pub const fn synth_hash(&self) -> AssetKeySynthHash
    {
        // TODO: & with !is_synthetic?
        let u = (self.0 >> Self::SYNTH_HASH_SHIFT) & Self::SYNTH_HASH_MAX;
        AssetKeySynthHash(u * self.is_synthetic() as u64)
    }
    #[inline]
    pub const fn is_synthetic(&self) -> bool
    {
        ((self.0 >> Self::SYNTH_FLAG_SHIFT) as u8 & Self::SYNTH_FLAG_MAX) == 1
    }

    #[inline]
    pub fn as_file_name(&self) -> PathBuf
    {
        PathBuf::from(format!("{:0width$x}.{}",
            self.0,
            ASSET_FILE_EXTENSION,
            width = format_width_hex_bytes(AssetKey::TOTAL_BITS)))
    }
    #[inline]
    pub fn as_meta_file_name(&self) -> PathBuf
    {
        PathBuf::from(format!("{:0width$x}.{}",
            self.0,
            ASSET_META_FILE_EXTENSION,
            width = format_width_hex_bytes(AssetKey::TOTAL_BITS)))
    }
}
// custom serialize/deserialize b/c TOML doesn't support u64
impl Serialize for AssetKey
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>
    {
        format!("{:0width$x}", self.0, width = format_width_hex_bytes(AssetKey::TOTAL_BITS)).serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for AssetKey
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error>
    {
        match u64::from_str_radix(&String::deserialize(deserializer)?, 16)
        {
            Ok(u) => Ok(Self(u)),
            Err(e) => Err(D::Error::custom(e))
        }
    }
}

crate::const_assert!(size_of::<AssetTypeId>() == 2);
impl Debug for AssetKey
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match f.alternate()
        {
            true =>
                f.write_fmt(format_args!("⟨{:?}|{:0width$x}⟩",
                    self.asset_type(),
                    self.0,
                    width = format_width_hex_bytes(AssetKey::TOTAL_BITS))),
            false =>
                f.write_fmt(format_args!("⟨{:0width$x}⟩",
                    self.0,
                    width = format_width_hex_bytes(AssetKey::TOTAL_BITS))),
        }
    }
}
impl From<AssetKey> for AssetKeyRepr
{
    fn from(value: AssetKey) -> Self { value.0 }
}
impl From<AssetKeyRepr> for AssetKey
{
    fn from(u: AssetKeyRepr) -> Self { Self(u) }
}
impl From<[u8; size_of::<AssetKey>()]> for AssetKey
{
    fn from(value: [u8; size_of::<AssetKey>()]) -> Self { Self(u64::from_le_bytes(value)) }
}
impl From<AssetKey> for [u8; size_of::<AssetKey>()]
{
    fn from(value: AssetKey) -> Self { value.0.to_le_bytes() }
}

#[cfg(test)]
mod asset_key_tests
{
    use super::*;

    #[test]
    fn construct_unique()
    {
        let asset_type = AssetTypeId::Test1;
        let is_synthetic = false;
        let derived_id = AssetKeyDerivedId(0x33u16);
        let source_id = AssetKeySourceId(0x111111u64);
        let synth_hash = AssetKeySynthHash(0);

        let k = AssetKey::unique(asset_type, derived_id, source_id);
        assert_eq!(k.asset_type(), asset_type, "Asset type");
        assert_eq!(k.is_synthetic(), is_synthetic, "Is synthetic");
        assert_eq!(k.derived_id(), derived_id, "Derived ID");
        assert_eq!(k.source_id(), source_id, "Source ID");
        assert_eq!(k.synth_hash(), synth_hash, "Synth Hash");

        assert_eq!(0x0010033000111111u64, <AssetKeyRepr>::from(k));
    }

    #[test]
    fn construct_synthetic()
    {
        let asset_type = AssetTypeId::Test1;
        let is_synthetic = true;
        let derived_id = AssetKeyDerivedId(0);
        let source_id = AssetKeySourceId(0);
        let synth_hash = AssetKeySynthHash::test(0x123);

        let k = AssetKey::synthetic(asset_type, synth_hash);
        assert_eq!(k.asset_type(), asset_type, "Asset type");
        assert_eq!(k.is_synthetic(), is_synthetic, "Is synthetic");
        assert_eq!(k.derived_id(), derived_id, "Derived ID");
        assert_eq!(k.source_id(), source_id, "Source ID");
        assert_eq!(k.synth_hash(), synth_hash, "Synth Hash");

        assert_eq!(0x0018_0000_0000_0123, <AssetKeyRepr>::from(k));
    }

    #[test]
    fn source_id_generate_only_fills_bottom_bytes()
    {
        let bid = AssetKeySourceId::generate();
        assert_eq!(0u64, bid.0 >> AssetKey::SOURCE_ID_BITS);
    }

    #[test]
    fn same_asset_keys_match()
    {
        let k1 = AssetKey::unique(
            AssetTypeId::Test1,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x1_1111_1111));
        let k2 = AssetKey::unique(
            AssetTypeId::Test1,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x1_1111_1111));

        assert_eq!(k1, k2);
    }

    #[test]
    fn mismatched_asset_type()
    {
        let k1 = AssetKey::unique(
            AssetTypeId::Test1,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x1_1111_1111));
        let k2 = AssetKey::unique(
            AssetTypeId::Test2,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x1_1111_1111));

        assert_ne!(k1, k2);
    }

    #[test]
    fn mismatched_derived_id()
    {
        let k1 = AssetKey::unique(
            AssetTypeId::Test1,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x1_1111_1111));
        let k2 = AssetKey::unique(
            AssetTypeId::Test1,
            AssetKeyDerivedId(1),
            AssetKeySourceId(0x1_1111_1111));

        assert_ne!(k1, k2);
    }

    #[test]
    fn mismatched_source_id()
    {
        let k1 = AssetKey::unique(
            AssetTypeId::Test1,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x1_1111_1111));
        let k2 = AssetKey::unique(
            AssetTypeId::Test1,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x2_2222_2222));

        assert_ne!(k1, k2);
    }
}
