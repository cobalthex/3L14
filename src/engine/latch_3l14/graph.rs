use std::fmt::{Debug, Formatter};
use smallvec::SmallVec;
use asset_3l14::Signal;
use crate::Scope;

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

// How the target block should behave on pulse
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum Inlet
{
    #[default]
    Pulse,
    PowerOff, // ignored by non-stateful blocks
}

// Where an outlet points to
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plug
{
    pub target: BlockId,
    pub inlet: Inlet,
}
impl Plug
{
    #[inline] #[must_use]
    pub fn new(target: BlockId, inlet: Inlet) -> Self
    {
        Self { target, inlet }
    }

    // TODO: better design
    #[inline] #[must_use]
    pub(crate) fn poison(self, poison: Inlet) -> Self
    {
        if let Inlet::PowerOff = poison
        {
            Self { target: self.target, inlet: poison }
        }
        else
        {
            self
        }
    }
}

// Outlets that pass-thru incoming pulses (but not power-offs)
#[derive(Default)]
pub struct PulsedOutlet
{
    pub plugs: Box<[Plug]>,
}
// Outlets that carry the parent signal and respond to power-offs
#[derive(Default)]
pub struct LatchingOutlet
{
    pub plugs: Box<[Plug]>,
}

pub trait Block  { }

pub(super) type OutletLinkList = SmallVec<[Plug; 2]>;

pub struct ImpulseOutletVisitor<'s>
{
    // TODO: move this back to a pass-by-(mut)ref
    pub(super) pulses: &'s mut OutletLinkList,
}
impl ImpulseOutletVisitor<'_>
{
    pub fn visit_pulsed(&mut self, outlet: &PulsedOutlet)
    {
        self.pulses.extend_from_slice(&outlet.plugs);
    }
}

// A block that can perform an action whenever they are pulsed
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
        self.pulses.extend_from_slice(&outlet.plugs);
    }
    pub fn visit_latching(&mut self, outlet: &LatchingOutlet)
    {
        self.latching.extend_from_slice(&outlet.plugs);
    }
}

// A block that can be powered on/off, performing an action upon on/off.
// Will turn off any downstream blocks when turned off
pub trait StateBlock
{
    // different names? activate/deactivate?
    fn power_on(&self, scope: &mut Scope);
    fn power_off(&self, scope: &mut Scope);

    // fn on_dependency_changed()

    fn visit_powered_outlets(&self, visitor: StateOutletVisitor);
}
impl Block for dyn StateBlock { }

// [standardized] template blocks?
//      in->out actions
//      in->powered

// A list of target blocks to pulse
pub type EntryPoints = Box<[BlockId]>;

pub struct Graph
{
    pub(super) auto_entries: EntryPoints,
    pub(super) signaled_entries: Box<[(Signal, EntryPoints)]>,
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
        assert!(block.is_impulse());
        assert!(!block.is_state());

        let block = BlockId::state(0);
        assert!(block.is_state());
        assert!(!block.is_impulse());
    }
}
