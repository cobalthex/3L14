// Truncate a 64-bit value (e.g. from hashing) to a 32-bit one with reasonable mixing
pub fn hash64_to_32(value: u64) -> u32
{
    let low = (value & (u32::MAX as u64)) as u32;
    let high = (value >> u32::BITS) as u32;
    let mixed = low ^ high.rotate_left(5); // simple rotation + xor
    mixed
}