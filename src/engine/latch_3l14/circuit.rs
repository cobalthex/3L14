use std::fmt::{Debug, Formatter};
use smallvec::SmallVec;
use nab_3l14::Signal;
use crate::Scope;
use crate::vars::VarChange;

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
    pub const fn latch(id: u32) -> Self
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
    pub const fn is_latch(self) -> bool
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
        if self.is_latch()
        {
            f.write_fmt(format_args!("[Latch|{}]", self.value()))
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
    PowerOff, // ignored by non-latching blocks
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

pub trait Block
{
    fn name(&self) -> &'static str { "TODO?" }
}

pub(super) type PlugList = SmallVec<[Plug; 2]>; // todo: re-eval count

pub struct ImpulseOutletVisitor<'s>
{
    pub(super) pulses: &'s mut PlugList,
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
    fn pulse(&self, scope: Scope, pulse_outlets: ImpulseOutletVisitor);

    fn visit_all_outlets(&self, visitor: ImpulseOutletVisitor);
}
impl Block for dyn ImpulseBlock { }

pub struct LatchOutletVisitor<'s>
{
    pub(super) pulses: &'s mut PlugList,
    pub(super) latches: &'s mut PlugList,
}
impl LatchOutletVisitor<'_>
{
    pub fn visit_pulsed(&mut self, outlet: &PulsedOutlet)
    {
        self.pulses.extend_from_slice(&outlet.plugs);
    }
    // visit a latching outlet. Inlet can be used to override the outlet (specific use cases)
    pub fn visit_latching(&mut self, outlet: &LatchingOutlet)
    {
        self.latches.reserve(outlet.plugs.len());
        for plug in outlet.plugs.iter()
        {
            self.latches.push(*plug);
        }
    }
}

pub enum OnVarChangedResult
{
    NoChange,
    PowerOff,
}

// A block that can be powered on/off, performing an action upon on/off.
// Will turn off any downstream blocks when turned off
pub trait LatchBlock
{
    fn power_on(&self, scope: Scope, pulse_outlets: LatchOutletVisitor);
    fn power_off(&self, scope: Scope);

    fn on_var_changed(&self, change: VarChange, scope: Scope, pulse_outlets: LatchOutletVisitor) -> OnVarChangedResult;

    fn visit_all_outlets(&self, visitor: LatchOutletVisitor);
}
impl Block for dyn LatchBlock { }

pub type EntryPoints = Box<[BlockId]>;

pub struct Circuit
{
    // todo: make pub(crate)

    pub auto_entries: EntryPoints,
    pub signaled_entries: Box<[(Signal, EntryPoints)]>,
    pub impulses: Box<[Box<dyn ImpulseBlock>]>,
    pub latches: Box<[Box<dyn LatchBlock>]>,
    pub num_local_vars: u32,
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
        assert!(!block.is_latch());

        let block = BlockId::latch(0);
        assert!(block.is_latch());
        assert!(!block.is_impulse());
    }
}
