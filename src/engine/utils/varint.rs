// based on EBML (Matroska)'s varint impl

pub fn encode(n: u64) -> (u8, [u8;8])
{
    let len = bytes_needed(n);
    let mut b = (n << (64 - 8 * len)).to_be_bytes();
    b[0] |= 1 << (8 - len); // prepend the length bits
    (len as u8, b)
}

// #[feature(generic_const_exprs)]
// #[feature(const_option)]
// pub const fn encode_const<const N: u64>() -> [u8; bytes_needed(N) as usize]
// {
//     let mut b = *N.to_be_bytes().last_chunk().unwrap();
//     b[0] |= 1 << (8 - b.len());
//     b
// }

// how many bytes are needed to encode n
#[inline]
pub const fn bytes_needed(n: u64) -> u8
{
    // in theory this if could optimized by jumping on zero flag after bsr
    // (or just store zero in al before bsr and perform the rest of the ops)
    if n == 0 { return 1; }
    (n.ilog2() / 7) as u8 + 1
}

// how many more bytes to read after this byte
#[inline]
pub const fn more_length(prefix: u8) -> u8
{
    prefix.leading_zeros() as u8
}

pub fn decode(bytes: &[u8]) -> u64
{
    let more = more_length(bytes[0]);

    let mut be_bytes = [0u8; 8];
    for i in 0..=(more as usize) // this should be inlinable
    {
        be_bytes[i] = bytes[i]
    }
    //be_bytes[0] &= ((1 << (7 - more)) - 1); // masks out all the top bits
    be_bytes[0] &= !(1 << (7 - more));

    let n = u64::from_be_bytes(be_bytes);
    n >> (8 * (7 - more))
}

#[cfg(test)]
mod tests
{
    use super::*;

    fn test(val: u64, expected_len: u8)
    {
        let encoded = encode(val);
        let decode_len = more_length(encoded.1[0]) + 1;
        let decoded = decode(&encoded.1[0..(encoded.0 as usize)]);

        assert_eq!(val, decoded);
        assert_eq!(decode_len, expected_len);
    }

    #[test]
    fn encode_decode()
    {
        for i in 0..8
        {
            test((1 << (i * 7)) - 1, i.max(1));
        }
    }
}