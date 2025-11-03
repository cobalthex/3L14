use crate::{Block, BlockKind, ImpulseBlock, LatchBlock, LatchingOutlet, PulsedOutlet};
use std::collections::HashMap;
use unicase::UniCase;

type Des<'de> = dyn erased_serde::Deserializer<'de> + 'de;
// The intermediate format of a block that is used for deserializing
pub struct HydrateBlock<'de>
{
    pub pulsed_outlets: HashMap<UniCase<&'de str>, PulsedOutlet>,
    pub latching_outlets: HashMap<UniCase<&'de str>, LatchingOutlet>,
    pub fields: HashMap<UniCase<&'de str>, Box<Des<'de>>>,
}

pub struct BlockMeta
{
    pub type_name: &'static str,
    pub type_name_hash: u64,
    pub hydrate_fn: fn(&mut HydrateBlock) -> Result<Box<dyn Block>, erased_serde::Error>,
    pub kind: BlockKind,
}
::inventory::collect!(BlockMeta);

// Not the cleanest way to verify this, but hard without specialization
pub trait CannotImplBothBlockTypes { }
impl<B: ImpulseBlock + LatchBlock> CannotImplBothBlockTypes for B { }