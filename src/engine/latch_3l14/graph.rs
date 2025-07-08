use nab_3l14::utils::ShortTypeName;
use std::fmt::{Debug, Formatter};
use smallvec::SmallVec;
use super::Scope;

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct BlockId(u32);
impl BlockId
{
    #[inline] #[must_use]
    pub const fn impulse(id: u32) -> Self
    {
        debug_assert!(id < (1 << 31));
        Self(id)
    }
    #[inline] #[must_use]
    pub const fn state(id: u32) -> Self
    {
        debug_assert!(id < (1 << 31));
        Self(id | (1 << 31))
    }

    #[inline] #[must_use]
    pub const fn is_impulse(self) -> bool
    {
        self.0 < (1 << 31)
    }
    #[inline] #[must_use]
    pub const fn is_state(self) -> bool
    {
        self.0 >= (1 << 31)
    }

    #[inline] #[must_use]
    pub(super) const fn value(self) -> u32
    {
        self.0 & ((1 << 31) - 1)
    }
}
impl Debug for BlockId
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        if self.is_state()
        {
            f.write_fmt(format_args!("[State|{}]", self.value()))
        }
        else
        {
            f.write_fmt(format_args!("[Impulse|{}]", self.value()))
        }
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub enum Inlet
{
    #[default]
    Pulse,
    PowerOff, // ignored by non-stateful blocks
}

#[derive(Debug, Clone, Copy)]
pub struct OutletLink
{
    pub block: BlockId,
    pub inlet: Inlet,
}
impl OutletLink
{
    // TODO: better design
    pub(crate) fn poison(self, poison: Inlet) -> Self
    {
        if let Inlet::PowerOff = poison
        {
            Self { block: self.block, inlet: poison }
        }
        else
        {
            self
        }
    }
}

#[derive(Default)]
pub struct PulsedOutlet
{
    pub links: Box<[OutletLink]>,
}
#[derive(Default)]
pub struct LatchingOutlet
{
    pub links: Box<[OutletLink]>,
}

pub trait Block  { }

pub(super) type OutletLinkList = SmallVec<[OutletLink; 2]>;

pub struct ImpulseOutletVisitor<'s>
{
    // TODO: move this back to a pass-by-(mut)ref
    pub(super) pulses: &'s mut OutletLinkList,
}
impl ImpulseOutletVisitor<'_>
{
    pub fn visit_pulsed(&mut self, outlet: &PulsedOutlet)
    {
        self.pulses.extend_from_slice(&outlet.links);
    }
}

pub trait ImpulseBlock
{
    fn pulse(&self, scope: &mut Scope);
    fn visit_outlets(&self, visitor: ImpulseOutletVisitor);
}
impl Block for dyn ImpulseBlock { }

pub struct StateOutletVisitor<'s>
{
    pub(super) pulses: &'s mut OutletLinkList,
    pub(super) latching: &'s mut OutletLinkList,
}
impl StateOutletVisitor<'_>
{
    pub fn visit_pulsed(&mut self, outlet: &PulsedOutlet)
    {
        self.pulses.extend_from_slice(&outlet.links);
    }
    pub fn visit_latching(&mut self, outlet: &LatchingOutlet)
    {
        self.latching.extend_from_slice(&outlet.links);
    }
}

pub trait StateBlock
{
    // different names? activate/deactivate?
    fn power_on(&self, scope: &mut Scope);
    fn power_off(&self, scope: &mut Scope);

    // fn on_dependency_changed()

    fn visit_powered_outlets(&self, visitor: StateOutletVisitor);
}
impl Block for dyn StateBlock { }

#[derive(Debug)]
pub enum EntryPoint
{
    Automatic,
    // event/message, parametric
}

pub struct EntryBlock
{
    pub kind: EntryPoint,
    // entry blocks are not 'stateful' and only 'pulse' blocks
    pub outlet: Box<[BlockId]>,
}

// [standardized] template blocks?
// in->out actions
// in->powered

pub struct Graph
{
    pub(super) entries: Box<[EntryBlock]>,
    pub(super) impulses: Box<[Box<dyn ImpulseBlock>]>,
    pub(super) states: Box<[Box<dyn StateBlock>]>,
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn block_id()
    {
        let block = BlockId::impulse(0);
        println!("{:?}", block);
        assert!(block.is_impulse());
        assert!(!block.is_state());

        let block = BlockId::state(0);
        println!("{:?}", block);
        assert!(block.is_state());
        assert!(!block.is_impulse());
    }
}
