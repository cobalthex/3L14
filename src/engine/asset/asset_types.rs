// All the supported asset types
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u16)]
pub enum AssetTypeId
{
    Invalid = 0,

    #[cfg(test)]
    Test1 = 1,
    #[cfg(test)]
    Test2 = 2,
    
    Untyped = 3, // non-descript, untyped data

    Texture = 4,

    Shader = 5,
    RenderMaterial = 6,

    Model = 7,
    Scene = 8,
}
