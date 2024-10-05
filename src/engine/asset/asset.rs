use crate::const_assert;
use crate::engine::asset::AssetTypeId;
use bitcode::{Decode, Encode};
use rand::RngCore;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Debug, Display, Formatter};
use std::hash::Hash;
use std::path::PathBuf;
use serde::de::Error;

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
#[cfg_attr(test, derive(Debug))] // custom debug taking bits into account?
pub struct AssetKeyDerivedId(u16); // only 15 bits are used.
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

#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))] // custom debug taking bits into account?
pub struct AssetKeySourceId(u128); // only 100 bits are used.
impl AssetKeySourceId
{
    pub fn generate() -> Self
    {
        let mut bytes = [0u8; size_of::<Self>()];
        rand::thread_rng().fill_bytes(&mut bytes[0..((AssetKey::SOURCE_KEY_BITS / 8) as usize)]);
        Self(u128::from_le_bytes(bytes))
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub struct AssetKey(u128);
impl AssetKey
{
    // ordered low-high bits
    const SOURCE_KEY_BITS: u8 = 100;
    const DERIVED_KEY_BITS: u8 = 15;
    const TEMP_FLAG_BITS: u8 = 1;
    const ASSET_TYPE_BITS: u8 = 12;

    const SOURCE_KEY_MAX: u128 = (1 << Self::SOURCE_KEY_BITS) - 1;
    const DERIVED_KEY_MAX: u16 = (1 << Self::DERIVED_KEY_BITS) - 1;
    const TEMP_FLAG_MAX: u8 = (1 << Self::TEMP_FLAG_BITS) - 1;
    const ASSET_TYPE_MAX: u16 = (1 << Self::ASSET_TYPE_BITS) - 1;

    const SOURCE_KEY_SHIFT: u8 = 0;
    const DERIVED_KEY_SHIFT: u8 = Self::SOURCE_KEY_SHIFT + Self::SOURCE_KEY_BITS;
    const TEMP_FLAG_SHIFT: u8 = Self::DERIVED_KEY_SHIFT + Self::DERIVED_KEY_BITS;
    const ASSET_TYPE_SHIFT: u8 = Self::TEMP_FLAG_SHIFT + Self::TEMP_FLAG_BITS;

    pub const fn new(asset_type: AssetTypeId, is_temporary: bool, derived_id: AssetKeyDerivedId, source_id: AssetKeySourceId) -> Self
    {
        const_assert!(
            (AssetKey::SOURCE_KEY_BITS + AssetKey::DERIVED_KEY_BITS + AssetKey::TEMP_FLAG_BITS + AssetKey::ASSET_TYPE_BITS) / 8
            == (size_of::<AssetKey>() as u8)
        );
        const_assert!(size_of::<AssetKey>() == 16);

        debug_assert!((asset_type as u16) < Self::ASSET_TYPE_MAX);
        debug_assert!(derived_id.0 < Self::DERIVED_KEY_MAX);
        debug_assert!(source_id.0 < Self::SOURCE_KEY_MAX);

        let mut u: u128 = (source_id.0 & Self::SOURCE_KEY_MAX) << Self::SOURCE_KEY_SHIFT;
        u |= ((derived_id.0 & Self::DERIVED_KEY_MAX) as u128) << Self::DERIVED_KEY_SHIFT;
        u |= (((is_temporary as u8) & Self::TEMP_FLAG_MAX) as u128) << Self::TEMP_FLAG_SHIFT;
        u |= (((asset_type as u16) & Self::ASSET_TYPE_MAX) as u128) << Self::ASSET_TYPE_SHIFT;
        Self(u)
    }

    #[inline]
    pub const fn asset_type(&self) -> AssetTypeId
    {
        unsafe { std::mem::transmute((self.0 >> Self::ASSET_TYPE_SHIFT) as u16 & Self::ASSET_TYPE_MAX) }
    }
    #[inline]
    pub const fn derived_id(&self) -> AssetKeyDerivedId
    {
        AssetKeyDerivedId((self.0 >> Self::DERIVED_KEY_SHIFT) as u16 & Self::DERIVED_KEY_MAX)
    }
    #[inline]
    pub const fn source_id(&self) -> AssetKeySourceId
    {
        AssetKeySourceId((self.0 >> Self::SOURCE_KEY_SHIFT) & Self::SOURCE_KEY_MAX)
    }
    #[inline]
    pub const fn is_temporary(&self) -> bool
    {
        ((self.0 >> Self::TEMP_FLAG_SHIFT) as u8 & Self::TEMP_FLAG_MAX) == 1
    }

    #[inline]
    pub fn as_file_name(&self) -> PathBuf
    {
        PathBuf::from(format!("{:032x}.ass", self.0))
    }
    #[inline]
    pub fn as_meta_file_name(&self) -> PathBuf
    {
        PathBuf::from(format!("{:032x}.mass", self.0))
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
                f.write_fmt(format_args!("⟨{:?}|{:026x}+{:04x}⟩",
                     self.asset_type(),
                     self.source_id().0,
                     self.derived_id().0)),
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
    fn construct_destruct()
    {
        let asset_type = AssetTypeId::Test1;
        let is_temporary = true; // TODO: broken
        let derived_id = AssetKeyDerivedId(0x33u16);
        let source_id = AssetKeySourceId(0x111111u128);

        let k = AssetKey::new(asset_type, is_temporary, derived_id, source_id);
        assert_eq!(k.asset_type(), asset_type, "Asset type");
        assert_eq!(k.is_temporary(), is_temporary, "Is temporary");
        assert_eq!(k.derived_id(), derived_id, "Derived ID");
        assert_eq!(k.source_id(), source_id, "Source ID");

        assert_eq!(0x00180330000000000000000000111111u128, k.into());
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
        let k1 = AssetKey::new(
            AssetTypeId::Test1,
            false,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x111111111111111111111111));
        let k2 = AssetKey::new(
            AssetTypeId::Test1,
            false,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x111111111111111111111111));

        assert_eq!(k1, k2);
    }

    #[test]
    fn mismatched_asset_type()
    {
        let k1 = AssetKey::new(
            AssetTypeId::Test1,
            false,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x111111111111111111111111));
        let k2 = AssetKey::new(
            AssetTypeId::Test2,
            false,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x111111111111111111111111));

        assert_ne!(k1, k2);
    }

    #[test]
    fn mismatched_derived_id()
    {
        let k1 = AssetKey::new(
            AssetTypeId::Test1,
            false,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x111111111111111111111111));
        let k2 = AssetKey::new(
            AssetTypeId::Test1,
            false,
            AssetKeyDerivedId(1),
            AssetKeySourceId(0x111111111111111111111111));

        assert_ne!(k1, k2);
    }

    #[test]
    fn mismatched_source_id()
    {
        let k1 = AssetKey::new(
            AssetTypeId::Test1,
            false,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x111111111111111111111111));
        let k2 = AssetKey::new(
            AssetTypeId::Test1,
            false,
            AssetKeyDerivedId(0),
            AssetKeySourceId(0x222222222222222222222222));

        assert_ne!(k1, k2);
    }
}
