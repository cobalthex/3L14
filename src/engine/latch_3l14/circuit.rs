use std::fmt::{Debug, Formatter};
use crossbeam::channel::Sender;
use smallvec::{ExtendFromSlice, SmallVec};
use nab_3l14::Signal;
use crate::Scope;
use crate::vars::VarChange;
use crate::runtime::Action as RuntimeAction;

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

pub struct ImpulseOutletVisitor<'i>
{
    pub(super) pulses: &'i mut PlugList,
}
impl ImpulseOutletVisitor<'_>
{
    pub fn visit_pulsed(&mut self, outlet: &PulsedOutlet)
    {
        self.pulses.extend_from_slice(&outlet.plugs);
    }
}

pub struct ImpulseActions<'i>
{
    pub(super) pulse_outlets: &'i mut PlugList,
    pub(super) action_sender: Sender<RuntimeAction>,
    // scope?
}
impl ImpulseActions<'_>
{
    #[inline]
    pub fn pulse(&mut self, outlet: &PulsedOutlet) { self.pulse_outlets.extend_from_slice(&outlet.plugs); }
    #[inline]
    pub fn enqueue_action(&mut self, action: RuntimeAction) { let _ = self.action_sender.send(action); } // TODO: error handling
}

// A block that can perform an action whenever they are pulsed
pub trait ImpulseBlock
{
    // Called when this block is pulsed
    fn pulse(&self, scope: Scope, actions: ImpulseActions);
    // iterate through all outlets in this block (Primarily used by diagnostics/etc)
    fn visit_all_outlets(&self, visitor: ImpulseOutletVisitor);
}
impl Block for dyn ImpulseBlock { }

pub struct LatchOutletVisitor<'l>
{
    pub(super) pulses: &'l mut PlugList,
    pub(super) latches: &'l mut PlugList,
}
impl LatchOutletVisitor<'_>
{
    #[inline]
    pub fn visit_pulsed(&mut self, outlet: &PulsedOutlet)
    {
        self.pulses.extend_from_slice(&outlet.plugs);
    }
    #[inline]
    pub fn visit_latching(&mut self, outlet: &LatchingOutlet)
    {
        self.latches.extend_from_slice(&outlet.plugs);
    }
}

// TODO: ability to send actions to runtime

pub struct LatchActions<'l>
{
    pub(super) pulse_outlets: &'l mut PlugList,
    pub(super) latch_outlets: &'l mut PlugList,
    pub(super) action_sender: Sender<RuntimeAction>,
    // scope?
}
impl LatchActions<'_>
{
    #[inline]
    pub fn pulse(&mut self, outlet: &PulsedOutlet) { self.pulse_outlets.extend_from_slice(&outlet.plugs); }
    #[inline]
    pub fn latch(&mut self, outlet: &LatchingOutlet) { self.latch_outlets.extend_from_slice(&outlet.plugs); }
    #[inline]
    pub fn enqueue_action(&mut self, action: RuntimeAction) { let _ = self.action_sender.send(action); } // TODO: error handling
}

// A block that can be powered on/off, performing an action upon on/off.
// Will turn off any downstream blocks when turned off
pub trait LatchBlock
{
    // Called when this latch gets powered-on
    fn power_on(&self, scope: Scope, actions: LatchActions);
    // Called as this latch is being powered off
    fn power_off(&self, scope: Scope);
    // Re-enter an already powered-on latch.
    // This is typically used for code-backed wake-ups
    fn re_enter(&self, _scope: Scope, _actions: LatchActions) { }
    // Called when a variable this block is listening to, changes
    fn on_var_changed(&self, _change: VarChange, _scope: Scope, _actions: LatchActions) { }
    // iterate through all outlets in this block (Primarily used by diagnostics/etc)
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
