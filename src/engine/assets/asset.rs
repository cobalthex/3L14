use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use crate::const_assert;
use crate::engine::assets::AssetTypeId;

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

pub type AssetKeyDerivedId = u16;
pub type AssetKeyBaseId = [u8; 12]; // newtype?

#[repr(packed)]
#[derive(Clone, Copy, PartialOrd, Eq, Ord)]
pub struct AssetKey
{
    asset_type: AssetTypeId, // 2 B
    derived_id: AssetKeyDerivedId, // an asset builder can output multiple assets for a single source, each source must have a unique derived_id
    base_id: AssetKeyBaseId, // a unique ID generated from a single source file
}
impl AssetKey
{
    pub fn new(asset_type: AssetTypeId, derived_id: AssetKeyDerivedId, base_id: AssetKeyBaseId) -> Self
    {
        const_assert!(size_of::<AssetKey>() == 16);

        Self
        {
            asset_type,
            derived_id,
            base_id,
        }
    }

    pub fn asset_type(&self) -> AssetTypeId { self.asset_type }
    pub fn derived_id(&self) -> u16 { self.derived_id }
    pub fn base_id(&self) -> u128
    {
        let mut bid = [0u8; 16];
        bid[..12].copy_from_slice(&self.base_id);
        u128::from_le_bytes(bid)
    }

    pub fn as_file_name(&self) -> PathBuf
    {
        PathBuf::from(format!("{:02x}{:02x}{:012x}.booga", self.asset_type() as u16, self.derived_id(), self.base_id()))
    }
}
impl PartialEq for AssetKey
{
    fn eq(&self, other: &Self) -> bool
    {
        Into::<u128>::into(*self) == Into::<u128>::into(*other)
    }
}
impl Hash for AssetKey
{
    fn hash<H: Hasher>(&self, state: &mut H) 
    {
        state.write_u128((*self).into())
    }
}

crate::const_assert!(size_of::<AssetTypeId>() == 2);
impl Debug for AssetKey
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        let ty = self.asset_type;
        let did = self.derived_id;

        f.write_fmt(format_args!("⟨{:?}|{:012x}+{:02x}⟩",
                    ty,
                    self.base_id(),
                    did))
    }
}
impl Into<u128> for AssetKey
{
    fn into(self) -> u128
    {
        unsafe { <u128>::from_le_bytes(std::mem::transmute::<_, [u8; 16]>(self)) }
    }
}
impl From<u128> for AssetKey
{
    fn from(u: u128) -> Self
    {
        unsafe { std::mem::transmute::<_, Self>(<u128>::to_le_bytes(u)) }
    }
}
impl Serialize for AssetKey
{
    // ideally this should assert little endian
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>
    {
        serializer.serialize_u128((*self).into())
    }
}
impl<'de> Deserialize<'de> for AssetKey
{
    // ideally this should assert little endian
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error>
    {
        u128::deserialize(deserializer).map(|d| d.into())
    }
}

#[cfg(test)]
mod asset_key_tests
{
    use crate::engine::assets::{AssetKey, AssetTypeId};

    #[test]
    fn same_asset_keys_match()
    {
        let k1 = AssetKey::new(
            AssetTypeId::Test1,
            0,
            [1,1,1,1,1,1,1,1,1,1,1,1]);
        let k2 = AssetKey::new(
            AssetTypeId::Test1,
            0,
            [1,1,1,1,1,1,1,1,1,1,1,1]);

        assert_eq!(k1, k2);
    }

    #[test]
    fn mismatched_asset_type()
    {
        let k1 = AssetKey::new(
            AssetTypeId::Test1,
            0,
            [1,1,1,1,1,1,1,1,1,1,1,1]);
        let k2 = AssetKey::new(
            AssetTypeId::Test2,
            0,
            [1,1,1,1,1,1,1,1,1,1,1,1]);

        assert_ne!(k1, k2);
    }

    #[test]
    fn mismatched_derived_id()
    {
        let k1 = AssetKey::new(
            AssetTypeId::Test1,
            0,
            [1,1,1,1,1,1,1,1,1,1,1,1]);
        let k2 = AssetKey::new(
            AssetTypeId::Test1,
            1,
            [1,1,1,1,1,1,1,1,1,1,1,1]);

        assert_ne!(k1, k2);
    }

    #[test]
    fn mismatched_base_id()
    {
        let k1 = AssetKey::new(
            AssetTypeId::Test1,
            0,
            [1,1,1,1,1,1,1,1,1,1,1,1]);
        let k2 = AssetKey::new(
            AssetTypeId::Test1,
            0,
            [2,2,2,2,2,2,2,2,2,2,2,2]);

        assert_ne!(k1, k2);
    }
}
