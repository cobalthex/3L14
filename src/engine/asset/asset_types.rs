// All the supported asset types
#[repr(u16)]
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetTypeId
{
    Invalid = 0,

    #[cfg(debug_assertions)]
    Test1 = 1,
    #[cfg(debug_assertions)]
    Test2 = 2,
    
    Untyped = 3, // non-descript, untyped data

    Texture = 4,

    Shader = 5,
    RenderMaterial = 6,

    Model = 7,
    Scene = 8,
}
