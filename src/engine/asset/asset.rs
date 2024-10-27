use crate::const_assert;
use crate::engine::asset::AssetTypeId;
use bitcode::{Decode, Encode};
use rand::RngCore;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Debug, Display, Formatter, Write};
use std::hash::Hash;
use std::path::PathBuf;
use metrohash::MetroHash128;
use unicase::UniCase;

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

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct AssetKeyDerivedId(u16); // only 15 bits are used.
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
        f.write_fmt(format_args!("{:04x}", self.0))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AssetKeySourceId(u128); // only 100 bits are used.
impl AssetKeySourceId
{
    pub fn generate() -> Self
    {
        let mut bytes = [0u8; size_of::<Self>()];
        rand::thread_rng().fill_bytes(&mut bytes[0..((AssetKey::SOURCE_KEY_BITS / 8) as usize)]);
        Self(u128::from_le_bytes(bytes))
    }

    #[cfg(test)]
    pub const fn test(n: u8) -> Self
    {
        Self(n as u128)
    }
}
// custom serialize/deserialize b/c TOML doesn't support u128
impl Serialize for AssetKeySourceId
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>
    {
        format!("{:026x}", self.0).serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for AssetKeySourceId
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error>
    {
        let inp = String::deserialize(deserializer)?;
        match u128::from_str_radix(&inp, 16)
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
pub struct AssetKeySynthHash(u128);
impl AssetKeySynthHash
{
    pub fn generate(hashable: impl Hash) -> Self
    {
        let mut hasher = MetroHash128::new();
        hashable.hash(&mut hasher);
        let (low, high) = hasher.finish128();
        let n = (low as u128) | ((high as u128) << 64);
        Self(n & AssetKey::SYNTH_HASH_MAX)
    }

    #[cfg(test)]
    pub const fn test(n: u128) -> Self
    {
        Self(n)
    }
}
// custom serialize/deserialize b/c TOML doesn't support u128
impl Serialize for AssetKeySynthHash
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>
    {
        format!("{:030x}", self.0).serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for AssetKeySynthHash
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error>
    {
        let inp = String::deserialize(deserializer)?;
        match u128::from_str_radix(&inp, 16)
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
        f.write_fmt(format_args!("{:030x}", self.0))
    }
}

// A unique identifier identifying a built asset
// It can either be 'synthetic' whereby the ID is a hash of its contents
// or it can be 'unique' where the ID is composed of a combined unique source and derived ID
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub struct AssetKey(u128);
impl AssetKey
{
    // ordered low-high bits
    const SOURCE_KEY_BITS: u8 = 100;
    const DERIVED_KEY_BITS: u8 = 15;
    const SYNTH_HASH_BITS: u8 = Self::SOURCE_KEY_BITS + Self::DERIVED_KEY_BITS;
    const SYNTH_FLAG_BITS: u8 = 1;
    const ASSET_TYPE_BITS: u8 = 12;

    const SOURCE_KEY_MAX: u128 = (1 << Self::SOURCE_KEY_BITS) - 1;
    const DERIVED_KEY_MAX: u16 = (1 << Self::DERIVED_KEY_BITS) - 1;
    const SYNTH_HASH_MAX: u128 = (1 << Self::SYNTH_HASH_BITS) - 1;
    const SYNTH_FLAG_MAX: u8 = (1 << Self::SYNTH_FLAG_BITS) - 1;
    const ASSET_TYPE_MAX: u16 = (1 << Self::ASSET_TYPE_BITS) - 1;

    const SOURCE_KEY_SHIFT: u8 = 0;
    const DERIVED_KEY_SHIFT: u8 = Self::SOURCE_KEY_SHIFT + Self::SOURCE_KEY_BITS;
    const SYNTH_HASH_SHIFT: u8 = 0;
    const SYNTH_FLAG_SHIFT: u8 = Self::DERIVED_KEY_SHIFT + Self::DERIVED_KEY_BITS;
    const ASSET_TYPE_SHIFT: u8 = Self::SYNTH_FLAG_SHIFT + Self::SYNTH_FLAG_BITS;

    pub const fn unique(asset_type: AssetTypeId, derived_id: AssetKeyDerivedId, source_id: AssetKeySourceId) -> Self
    {
        const_assert!(
            (AssetKey::SOURCE_KEY_BITS + AssetKey::DERIVED_KEY_BITS + AssetKey::SYNTH_FLAG_BITS + AssetKey::ASSET_TYPE_BITS) / 8
            == (size_of::<AssetKey>() as u8)
        );
        const_assert!(size_of::<AssetKey>() == 16);

        debug_assert!((asset_type as u16) < Self::ASSET_TYPE_MAX);
        debug_assert!(derived_id.0 < Self::DERIVED_KEY_MAX);
        debug_assert!(source_id.0 < Self::SOURCE_KEY_MAX);

        let mut u: u128 = (source_id.0 & Self::SOURCE_KEY_MAX) << Self::SOURCE_KEY_SHIFT;
        u |= ((derived_id.0 & Self::DERIVED_KEY_MAX) as u128) << Self::DERIVED_KEY_SHIFT;
        // u |= ((0u8 & Self::SYNTH_FLAG_MAX) as u128) << Self::SYNTH_FLAG_SHIFT;
        u |= (((asset_type as u16) & Self::ASSET_TYPE_MAX) as u128) << Self::ASSET_TYPE_SHIFT;
        Self(u)
    }

    #[inline]
    pub const fn synthetic(asset_type: AssetTypeId, synth_hash: AssetKeySynthHash) -> Self
    {
        const_assert!(
            (AssetKey::SYNTH_HASH_BITS + AssetKey::SYNTH_FLAG_BITS + AssetKey::ASSET_TYPE_BITS) / 8
            == (size_of::<AssetKey>() as u8)
        );
        const_assert!(size_of::<AssetKey>() == 16);

        debug_assert!((asset_type as u16) < Self::ASSET_TYPE_MAX);
        debug_assert!(synth_hash.0 < Self::SYNTH_HASH_MAX);

        let mut u: u128 = (synth_hash.0 & Self::SYNTH_HASH_MAX) << Self::SYNTH_HASH_SHIFT;
        u |= ((1u8 & Self::SYNTH_FLAG_MAX) as u128) << Self::SYNTH_FLAG_SHIFT;
        u |= (((asset_type as u16) & Self::ASSET_TYPE_MAX) as u128) << Self::ASSET_TYPE_SHIFT;
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
        AssetKeySourceId(u * !self.is_synthetic() as u128)
    }
    // Get the synthesized hash for this asset key, returns 0 if unique (not synthetic)
    #[inline]
    pub const fn synth_hash(&self) -> AssetKeySynthHash
    {
        // TODO: & with !is_synthetic?
        let u = (self.0 >> Self::SYNTH_HASH_SHIFT) & Self::SYNTH_HASH_MAX;
        AssetKeySynthHash(u * self.is_synthetic() as u128)
    }
    #[inline]
    pub const fn is_synthetic(&self) -> bool
    {
        ((self.0 >> Self::SYNTH_FLAG_SHIFT) as u8 & Self::SYNTH_FLAG_MAX) == 1
    }

    #[inline]
    pub fn as_file_name(&self) -> PathBuf
    {
        PathBuf::from(format!("{:032x}.{}", self.0, ASSET_FILE_EXTENSION))
    }
    #[inline]
    pub fn as_meta_file_name(&self) -> PathBuf
    {
        PathBuf::from(format!("{:032x}.{}", self.0, ASSET_META_FILE_EXTENSION))
    }
}
// custom serialize/deserialize b/c TOML doesn't support u128
impl Serialize for AssetKey
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>
    {
        format!("{:032x}", self.0).serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for AssetKey
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error>
    {
        match u128::from_str_radix(&String::deserialize(deserializer)?, 16)
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
                f.write_fmt(format_args!("⟨{:?}|{:032x}⟩",
                     self.asset_type(),
                     self.0)),
            false =>
                f.write_fmt(format_args!("⟨{:032x}⟩", self.0)),
        }
    }
}
impl From<AssetKey> for u128
{
    fn from(value: AssetKey) -> Self { value.0 }
}
impl From<u128> for AssetKey
{
    fn from(u: u128) -> Self { Self(u) }
}
impl From<[u8; 16]> for AssetKey
{
    fn from(value: [u8; 16]) -> Self { Self(u128::from_le_bytes(value)) }
}
impl From<AssetKey> for [u8; 16]
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
        let source_id = AssetKeySourceId(0x111111u128);
        let synth_hash = AssetKeySynthHash(0);

        let k = AssetKey::unique(asset_type, derived_id, source_id);
        assert_eq!(k.asset_type(), asset_type, "Asset type");
        assert_eq!(k.is_synthetic(), is_synthetic, "Is synthetic");
        assert_eq!(k.derived_id(), derived_id, "Derived ID");
        assert_eq!(k.source_id(), source_id, "Source ID");
        assert_eq!(k.synth_hash(), synth_hash, "Synth Hash");

        assert_eq!(0x00100330000000000000000000111111u128, k.into());
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

        assert_eq!(0x00180000000000000000000000000123u128, k.into());
    }

    #[test]
    fn source_id_generate_only_fills_bottom_bytes()
    {
        let bid = AssetKeySourceId::generate();
        assert_eq!(0u128, bid.0 >> AssetKey::SOURCE_KEY_BITS);
    }

    #[test]
    fn same_asset_keys_match()
    {
        let k1 = AssetKey::unique(
            AssetTypeId::Test1,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x111111111111111111111111));
        let k2 = AssetKey::unique(
            AssetTypeId::Test1,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x111111111111111111111111));

        assert_eq!(k1, k2);
    }

    #[test]
    fn mismatched_asset_type()
    {
        let k1 = AssetKey::unique(
            AssetTypeId::Test1,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x111111111111111111111111));
        let k2 = AssetKey::unique(
            AssetTypeId::Test2,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x111111111111111111111111));

        assert_ne!(k1, k2);
    }

    #[test]
    fn mismatched_derived_id()
    {
        let k1 = AssetKey::unique(
            AssetTypeId::Test1,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x111111111111111111111111));
        let k2 = AssetKey::unique(
            AssetTypeId::Test1,
            AssetKeyDerivedId(1),
            AssetKeySourceId(0x111111111111111111111111));

        assert_ne!(k1, k2);
    }

    #[test]
    fn mismatched_source_id()
    {
        let k1 = AssetKey::unique(
            AssetTypeId::Test1,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x111111111111111111111111));
        let k2 = AssetKey::unique(
            AssetTypeId::Test1,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x222222222222222222222222));

        assert_ne!(k1, k2);
    }
}
