use std::any::TypeId;
use std::fmt::{Debug, Display, Formatter, LowerHex};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::mem::size_of;
use unicase::UniCase;

pub trait Asset: Sync + Send + 'static
{
    // Have all dependencies of this asset been loaded? (always true if no dependencies)
    fn all_dependencies_loaded(&self) -> bool { true }
}
pub trait HasAssetDependencies
{
    fn asset_dependencies_loaded(&self) -> bool;
}

pub trait AssetPath: AsRef<str> + Hash + Display + Debug { }
impl<T> AssetPath for T where T: AsRef<str> + Hash + Display + Debug { }

#[derive(Debug, Hash, Clone)]
pub struct AssetKeyDesc<'a>
{
    pub path: &'a UniCase<String>,
    pub type_id: TypeId,
}

#[derive(Hash, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub struct AssetKey(u64);
impl<'a> From<&AssetKeyDesc<'a>> for AssetKey
{
    fn from(desc: &AssetKeyDesc<'a>) -> Self
    {
        let mut hasher = DefaultHasher::default();
        desc.hash(&mut hasher);
        Self(hasher.finish())
    }
}
impl<'a> From<AssetKeyDesc<'a>> for AssetKey
{
    fn from(desc: AssetKeyDesc<'a>) -> Self { Self::from(&desc) }
}
impl Debug for AssetKey
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        let sizeof = size_of::<Self>();
        f.write_fmt(format_args!("{:0width$x}", self.0, width = sizeof))
    }
}