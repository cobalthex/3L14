use crate::{ImpulseBlock, LatchBlock, LatchingOutlet, PulsedOutlet};
use nab_3l14::utils::ShortTypeName;
use std::collections::HashMap;
use unicase::UniCase;

trait WhichBlock
{
    type Out: ?Sized;
}
struct BlockKindMapper<const BLOCK_KIND: u8>;
// values must match BlockKind::* values
impl WhichBlock for BlockKindMapper<0> { type Out = dyn ImpulseBlock; }
impl WhichBlock for BlockKindMapper<1> { type Out = dyn LatchBlock; }

type Des<'de> = dyn erased_serde::Deserializer<'de> + 'de;
// The intermediate format of a block that is used for deserializing
pub struct HydrateBlock<'de>
{
    pub pulsed_outlets: HashMap<UniCase<&'de str>, PulsedOutlet>,
    pub latching_outlets: HashMap<UniCase<&'de str>, LatchingOutlet>,
    pub fields: HashMap<UniCase<&'de str>, Box<Des<'de>>>,
}

pub struct BlockMeta<const BLOCK_KIND: u8>
    where BlockKindMapper<BLOCK_KIND>: WhichBlock
{
    pub type_name: &'static str,
    pub type_name_hash: u64,
    pub hydrate_fn: fn(&mut HydrateBlock) -> Result<Box<<BlockKindMapper<BLOCK_KIND> as WhichBlock>::Out>, erased_serde::Error>,
}
::inventory::collect!(BlockMeta<0>);
::inventory::collect!(BlockMeta<1>);

struct NeedsDefault<const B: bool>;
trait ConditionalDefault<T>  { fn cond_default() -> T; }
impl<T: Default> ConditionalDefault<T> for NeedsDefault<true>
{
    fn cond_default() -> T { T::default() }
}
impl<T> ConditionalDefault<T> for NeedsDefault<false>
{
    fn cond_default() -> T
    {
        panic!("{:?} does not implement Default", Self::short_type_name());
    }
}
#[inline(always)]
pub fn default_if<T, const B: bool>() -> T
    where NeedsDefault<B>: ConditionalDefault<T>
{
    <NeedsDefault<B> as ConditionalDefault<T>>::cond_default()
}