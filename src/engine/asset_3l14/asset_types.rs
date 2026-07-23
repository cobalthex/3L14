use proc_macros_3l14::FancyEnum;

// All the supported (runtime) asset types
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FancyEnum)]
#[repr(u16)]
pub enum AssetTypeId
{
    Invalid = 0x0,

    #[cfg(test)]
    Test1 = 0x1,
    #[cfg(test)]
    Test2 = 0x2,

    Untyped = 0x3, // non-descript, untyped data

    Geometry = 0x4,
    Skeleton = 0x5,
    Texture = 0x6,
    TextureMips = 0x7,
    Material = 0x8,
    Shader = 0x9,
    Model = 0xa,
    Look = 0xb,
    SkeletalAnimation = 0xc,

    Circuit = 0xd,

    Scene = 0xe,
    SceneChunk = 0xf,

    // ComputePipeline

    // Surface -- physics
}

// assert asset type max value < ASSET_TYPE_BITS * 8
