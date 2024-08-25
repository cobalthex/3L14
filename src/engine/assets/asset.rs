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


pub trait AssetPath: AsRef<str> + Hash + Display + Debug { }
impl<T> AssetPath for T where T: AsRef<str> + Hash + Display + Debug { }

#[derive(Debug, Hash, Clone)]
pub struct AssetKeyDesc<S: AssetPath>
{
    pub path: UniCase<S>,
    pub type_id: TypeId,
}

#[derive(Hash, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub struct AssetKey(u64);
impl<S: AssetPath> From<&AssetKeyDesc<S>> for AssetKey
{
    fn from(desc: &AssetKeyDesc<S>) -> Self
    {
        let mut hasher = DefaultHasher::default();
        desc.hash(&mut hasher);
        Self(hasher.finish())
    }
}
impl<S: AssetPath> From<AssetKeyDesc<S>> for AssetKey
{
    fn from(desc: AssetKeyDesc<S>) -> Self { Self::from(&desc) }
}
impl Debug for AssetKey
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        let sizeof = size_of::<Self>();
        f.write_fmt(format_args!("{:0width$x}", self.0, width = sizeof))
    }
}