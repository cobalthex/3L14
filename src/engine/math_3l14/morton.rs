use std::hash::Hash;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Morton(pub u64);
impl Morton
{
    pub const MAX_PRECISION: usize = 21; // all values will be clamped to this number of bits
    const PRECISION_MASK: u64 = (1 << Self::MAX_PRECISION) - 1;

    pub const fn encode(x: u32, y: u32, z: u32) -> Self
    {
        let x = Self::spread_bits(x);
        let y = Self::spread_bits(y);
        let z = Self::spread_bits(z);
        Self(x | (y << 1) | (z << 2))
    }

    pub const fn decode(self) -> (u32, u32, u32)
    {
        let x = Self::compact_bits(self.0);
        let y = Self::compact_bits(self.0 >> 1);
        let z = Self::compact_bits(self.0 >> 2);
        (x, y, z)
    }

    const fn spread_bits(mut v: u32) -> u64
    {
        let mut v = v as u64 & Self::PRECISION_MASK;
        v = (v | (v << 32)) & 0x001F00000000FFFF; // split high/low
        v = (v | (v << 16)) & 0x001F0000FF0000FF; // subdivide further
        v = (v | (v << 8))  & 0x100F00F00F00F00F; // split by nibble
        v = (v | (v << 4))  & 0x10C30C30C30C30C3; // 2-bit group selection
        v = (v | (v << 2))  & 0x1249249249249249; // per-bit interleaving
        v
    }

    const fn compact_bits(mut v: u64) -> u32
    {
        v &= 0x1249249249249249;
        v = (v | (v >> 2))  & 0x10C30C30C30C30C3;
        v = (v | (v >> 4))  & 0x100F00F00F00F00F;
        v = (v | (v >> 8))  & 0x001F0000FF0000FF;
        v = (v | (v >> 16)) & 0x001F00000000FFFF;
        v = (v | (v >> 32)) & Self::PRECISION_MASK;
        v as u32
    }

    // can also generate LUT (2^21 entries; 16MiB)
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn a()
    {
        let vals = (1, 2, 3);
        let morton = Morton::encode(vals.0, vals.1, vals.2);
        assert_eq!(Morton::decode(morton), vals);
    }

    #[test]
    fn b()
    {
        let vals = (0x7, 0xfff, 0xfffff);
        let morton = Morton::encode(vals.0, vals.1, vals.2);
        assert_eq!(Morton::decode(morton), vals);
    }
}