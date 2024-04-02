#![allow(private_bounds)] // todo: https://github.com/rust-lang/rust/issues/115475

use std::any::TypeId;
use std::fmt::{Debug, Display};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use arc_swap::{ArcSwap, DefaultStrategy, Guard};
use unicase::UniCase;

pub mod texture;
pub mod material;

pub trait AssetPath: AsRef<str> + Hash + Display + Debug { }
impl<T> AssetPath for T where T: AsRef<str> + Hash + Display + Debug { }

#[derive(Debug, Hash, Clone)]
pub struct AssetKeyDesc<S: AssetPath>
{
    pub path: UniCase<S>,
    pub type_id: TypeId,
}

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd)]
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

pub trait Asset: Sync + Send
{
}

pub enum AssetLoadError
{
    NotFound,

    #[cfg(test)]
    TestEmpty,
}

pub enum AssetPayload<A: Asset>
{
    Pending,
    Available(A),
    Unavailable(AssetLoadError), // shouldn't be stored in-cache
}

// TODO: lifetime management?
pub struct AssetHandle<A: Asset>
{
    key: AssetKey,
    payload: ArcSwap<AssetPayload<A>>,
}
impl<A: Asset> AssetHandle<A>
{
    pub fn key(&self) -> AssetKey { self.key }
    pub fn payload(&self) -> Guard<Arc<AssetPayload<A>>, DefaultStrategy>
    {
        self.payload.load()
    }
    // downgrade?
}

pub struct AssetLoadRequest<'a>
{
    key_desc: AssetKeyDesc<&'a str>,
    // metadata: &'a AssetMetadata,
}

pub struct AssetLifecyclerStats
{
    pub active_assets: usize,
}

pub trait AssetLifecycler<A: Asset>
{
    fn get_or_create(&self, request: AssetLoadRequest) -> ArcSwap<AssetPayload<A>>; // Return a handle immediately, perform loads asynchronously if necessary
    // reload ?

    // todo: controlled destruction
    fn stats(&self) -> AssetLifecyclerStats;
}

// Internal trait used by the assets container to forward load requests
trait TypedAssetLoadProxy<A: Asset>
{
    fn load_internal(&self, request: AssetLoadRequest) -> ArcSwap<AssetPayload<A>>;
}

/// Define an asset container
/// each entry must be module::TypeName,
///   where each TypeName derives Asset
///   and has a corresponding module::TypeNameLifecycler which implements AssetLifecycler&lt;TypeName&gt;
/// e.g.
/// texture::Texture
/// texture::TextureLifecycler
/// impl AssetLifecycler&lt;texture::Texture&gt; for texture::TextureLifecycler
#[macro_export]
macro_rules! define_assets // todo: take any length asset type path, impl not ideal inside here
{
    ($($asset_module:ident::$asset_type:ident),* $(,)?) =>
    {
        paste::paste!
        {
            pub struct Assets
            {
                $(pub [<$asset_type:snake s>]: $asset_module::[<$asset_type Lifecycler>]),*
            }
            $(
            #[allow(private_bounds)]
            impl TypedAssetLoadProxy<$asset_module::$asset_type> for Assets
            {
                fn load_internal(&self, request: AssetLoadRequest) -> ArcSwap<AssetPayload<$asset_module::$asset_type>>
                {
                    self.[<$asset_type:snake s>].get_or_create(request)
                }
            }
            )*

            impl Assets
            {
                pub fn new($([<$asset_type:snake s>]: $asset_module::[<$asset_type Lifecycler>]),*) -> Self
                {
                    Self
                    {
                        $([<$asset_type:snake s>]),*
                    }
                }

                pub fn load<A: Asset + 'static, S: AssetPath>(&self, asset_path: UniCase<&S>)
                    -> AssetHandle<A>
                    where Self: TypedAssetLoadProxy<A>
                {
                    let key_desc = AssetKeyDesc
                    {
                        path: UniCase::unicode(asset_path.as_ref()),
                        type_id: TypeId::of::<A>(),
                    };
                    let asset_key = (&key_desc).into();

                    let load = AssetLoadRequest
                    {
                        key_desc,
                    };
                    let payload: ArcSwap<AssetPayload<A>> = TypedAssetLoadProxy::<A>::load_internal(self, load);

                    AssetHandle
                    {
                        key: asset_key,
                        payload,
                    }
                }
            }
        }
    };
}

#[cfg(test)]
mod tests
{
    use arc_swap::ArcSwapOption;
    use super::*;
    use unicase::UniCase;

    struct TestAsset
    {
        name: &'static str,
    }
    impl Asset for TestAsset { }

    #[derive(Default)]
    struct TestAssetLifecycler
    {
        pub factory: ArcSwapOption<&'static dyn Fn(AssetLoadRequest) -> AssetPayload<TestAsset>>,
    }
    impl AssetLifecycler<TestAsset> for TestAssetLifecycler
    {
        fn get_or_create(&self, request: AssetLoadRequest) -> ArcSwap<AssetPayload<TestAsset>>
        {
            ArcSwap::from_pointee(match self.factory.load().as_ref()
            {
                Some(f) => f(request),
                None => AssetPayload::Unavailable(AssetLoadError::TestEmpty),
            })
        }
    }

    define_assets![self::TestAsset];

    #[test]
    fn load_texture()
    {
        let assets = Assets::new(TestAssetLifecycler::default());
        let mut req: AssetHandle<TestAsset> = assets.load(UniCase::new(&"test"));

        match req.payload.load().as_ref()
        {
            AssetPayload::Unavailable(AssetLoadError::TestEmpty) => {},
            _ => panic!("Invalid result"),
        }

        assets.test_assets.factory.store(Some(Arc::new(&|_req|
        {
            AssetPayload::Pending
        })));

        req = assets.load(UniCase::new(&"test"));
        match req.payload.load().as_ref()
        {
            AssetPayload::Pending => {},
            _ => panic!("Asset not pending"),
        }

        assets.test_assets.factory.store(Some(Arc::new(&|_req|
            {
                AssetPayload::Available(TestAsset { name: "test asset"})
            })));

        req = assets.load(UniCase::new(&"test"));
        match req.payload.load().as_ref()
        {
            AssetPayload::Available(a) => assert_eq!(a.name, "test asset"),
            _ => panic!("Asset not available"),
        }
    }
}
