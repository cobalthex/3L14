use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use rand::RngCore;
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

pub type AssetKeyDerivedId = u16; // only 15 bits are used
pub type AssetKeyBaseId = u128; // only 100 bits are used

// TODO: revisit endianness?

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AssetKey(u128);
impl AssetKey
{
    // ordered low-high
    const BASE_KEY_BITS: u8 = 100;
    const DERIVED_KEY_BITS: u8 = 15;
    const TEMP_FLAG_BITS: u8 = 1;
    const TYPE_ID_BITS: u8 = 12;

    const BASE_KEY_MAX: u128 = (1 << Self::BASE_KEY_BITS) - 1;
    const DERIVED_KEY_MAX: u16 = (1 << Self::DERIVED_KEY_BITS) - 1;
    const TEMP_FLAG_MAX: u8 = 1;
    const TYPE_ID_MAX: u16 = (1 << Self::TYPE_ID_BITS) - 1;

    const BASE_KEY_SHIFT: u8 = 0;
    const DERIVED_KEY_SHIFT: u8 = Self::BASE_KEY_SHIFT + Self::BASE_KEY_BITS;
    const TEMP_FLAG_SHIFT: u8 = Self::DERIVED_KEY_SHIFT + Self::DERIVED_KEY_BITS;
    const TYPE_ID_SHIFT: u8 = Self::TEMP_FLAG_SHIFT + Self::TEMP_FLAG_BITS;

    pub fn new(asset_type: AssetTypeId, is_temporary: bool, derived_id: AssetKeyDerivedId, base_id: AssetKeyBaseId) -> Self
    {
        const_assert!(
            (AssetKey::BASE_KEY_BITS + AssetKey::DERIVED_KEY_BITS + AssetKey::TEMP_FLAG_BITS + AssetKey::TYPE_ID_BITS) / 8
            == (size_of::<AssetKey>() as u8)
        );
        const_assert!(size_of::<AssetKey>() == 16);

        debug_assert!((asset_type as u16) < Self::TYPE_ID_MAX);
        debug_assert!((derived_id) < Self::DERIVED_KEY_MAX);
        debug_assert!((base_id) < Self::BASE_KEY_MAX);

        let mut u: u128 = (base_id & Self::BASE_KEY_MAX) << Self::BASE_KEY_SHIFT;
        u |= ((derived_id & Self::DERIVED_KEY_MAX) as u128) << Self::DERIVED_KEY_SHIFT;
        u |= (((is_temporary as u8) & Self::TEMP_FLAG_MAX) as u128) << Self::DERIVED_KEY_SHIFT;
        u |= (((asset_type as u16) & Self::TYPE_ID_MAX) as u128) << Self::TYPE_ID_SHIFT;
        Self(u)
    }

    pub fn generate_base_id() -> AssetKeyBaseId
    {
        let mut bytes = [0u8; size_of::<Self>()];
        rand::thread_rng().fill_bytes(&mut bytes[0..((Self::BASE_KEY_BITS / 8) as usize)]);
        unsafe { std::mem::transmute(bytes) }
    }

    pub fn asset_type(&self) -> AssetTypeId
    {
        unsafe { std::mem::transmute((self.0 >> Self::TYPE_ID_SHIFT) as u16 & Self::TYPE_ID_MAX) }
    }
    pub fn derived_id(&self) -> AssetKeyDerivedId
    {
        (self.0 >> Self::DERIVED_KEY_SHIFT) as u16 & Self::DERIVED_KEY_MAX
    }
    pub fn base_id(&self) -> AssetKeyBaseId
    {
        (self.0 >> Self::BASE_KEY_SHIFT) & Self::BASE_KEY_MAX
    }
    pub fn is_temporary(&self) -> bool
    {
        ((self.0 >> Self::TEMP_FLAG_SHIFT) as u8 & Self::TEMP_FLAG_MAX) == 1
    }

    pub fn as_file_name(&self) -> PathBuf
    {
        PathBuf::from(format!("{:032x}.ass", self.0))
    }

    pub fn as_meta_file_name(&self) -> PathBuf
    {
        PathBuf::from(format!("{:032x}.mass", self.0))
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
                f.write_fmt(format_args!("{:032x}", self.0)),
            false =>
                f.write_fmt(format_args!("⟨{:?}|{:024x}+{:04x}⟩",
                     self.asset_type(),
                     self.base_id(),
                     self.derived_id())),
        }
    }
}
impl From<AssetKey> for u128
{
    fn from(value: AssetKey) -> Self
    {
        value.0
    }
}
impl From<u128> for AssetKey
{
    fn from(u: u128) -> Self { Self(u) }
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
            false,
            0,
            0x111111111111111111111111);
        let k2 = AssetKey::new(
            AssetTypeId::Test1,
            false,
            0,
            0x111111111111111111111111);

        assert_eq!(k1, k2);
    }

    #[test]
    fn mismatched_asset_type()
    {
        let k1 = AssetKey::new(
            AssetTypeId::Test1,
            false,
            0,
            0x111111111111111111111111);
        let k2 = AssetKey::new(
            AssetTypeId::Test2,
            false,
            0,
            0x111111111111111111111111);

        assert_ne!(k1, k2);
    }

    #[test]
    fn mismatched_derived_id()
    {
        let k1 = AssetKey::new(
            AssetTypeId::Test1,
            false,
            0,
            0x111111111111111111111111);
        let k2 = AssetKey::new(
            AssetTypeId::Test1,
            false,
            1,
            0x111111111111111111111111);

        assert_ne!(k1, k2);
    }

    #[test]
    fn mismatched_base_id()
    {
        let k1 = AssetKey::new(
            AssetTypeId::Test1,
            false,
            0,
            0x111111111111111111111111);
        let k2 = AssetKey::new(
            AssetTypeId::Test1,
            false,
            0,
            0x222222222222222222222222);

        assert_ne!(k1, k2);
    }
}
