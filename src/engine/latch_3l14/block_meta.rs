use crate::Block;
use bitcode::DecodeOwned;
use std::error::Error;

pub trait BlockMeta: 'static
{
    const TYPE_NAME: &'static str;
    const BLOCK_NAME_HASH: u64; // a combination of crate name and type name
}

pub struct BlockRegistration
{
    pub name: &'static str,
    pub name_hash: u64,

    // variant for Latch vs Impulse?
    pub decode_fn: fn(&[u8]) -> Result<Box<dyn Block>, Box<dyn Error>>,
}
impl BlockRegistration
{
    pub const fn register<B: BlockMeta + Block + DecodeOwned>() -> Self
    {

        Self
        {
            name: B::TYPE_NAME,
            name_hash: B::BLOCK_NAME_HASH,

            decode_fn: |bytes: &[u8]| -> Result<Box<dyn Block>, Box<dyn Error>>
            {
                match bitcode::decode::<B>(bytes)
                {
                    Ok(t) => Ok(Box::new(t)),
                    Err(e) => Err(Box::new(e)),
                }
            },
        }
    }
}

// TODO: mutually exclusive
// trait BlockMeta<B: Block>
// {
//     const IS_LATCH: bool;
// }
// impl<I: ImpulseBlock> BlockMeta<I> for BlockRegistration
// {
//     const IS_LATCH: bool = false;
// }
// impl<L: LatchBlock> BlockMeta<L> for BlockRegistration
// {
//     const IS_LATCH: bool = true;
// }

::inventory::collect!(BlockRegistration);
