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

    Geometry = 4,
    Skeleton = 5,
    Texture = 6,
    Material = 7,
    Shader = 8,
    Model = 9,
    SkeletalAnimation = 10,

    Scene = 11,
    
    // ComputePipeline

    // Surface -- physics
}
