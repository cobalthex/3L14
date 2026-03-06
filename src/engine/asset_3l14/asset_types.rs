use proc_macros_3l14::FancyEnum;

// All the supported asset types
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FancyEnum)]
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
    TextureMips = 7,
    Material = 8,
    Shader = 9,
    Model = 10,
    SkeletalAnimation = 11,

    Scene = 12,

    Circuit = 13,

    // ComputePipeline

    // Surface -- physics
}
