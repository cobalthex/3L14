use crate::vars::{ScopeChanges, VarChange};
use crate::{InstRunId, LocalScope, Runtime, Scope, SharedScope};
use nab_3l14::Signal;
use smallvec::SmallVec;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

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
    pub block: BlockId,
    pub inlet: Inlet,
}
impl Plug
{
    #[inline] #[must_use]
    pub fn new(target: BlockId, inlet: Inlet) -> Self
    {
        Self { block: target, inlet }
    }

    // TODO: better design
    #[inline] #[must_use]
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

pub(super) type PlugList = SmallVec<[Plug; 2]>; // todo: re-eval count (visitor probably needs more than actions)

pub struct ImpulseOutletVisitor<'i>
{
    pub(super) pulses: &'i mut PlugList,
}
impl<'i> ImpulseOutletVisitor<'i>
{
    #[inline]
    pub fn visit_pulsed(&mut self, outlet: &PulsedOutlet)
    {
        self.pulses.extend_from_slice(&outlet.plugs);
    }
}

pub struct ImpulseActions<'i>
{
    pub(super) pulse_outlets: &'i mut PlugList,
    pub runtime: Arc<Runtime>,
    // scope?
}
impl ImpulseActions<'_>
{
    #[inline]
    pub fn pulse(&mut self, outlet: &PulsedOutlet) { self.pulse_outlets.extend_from_slice(&outlet.plugs); }
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
impl<'l> LatchOutletVisitor<'l>
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
    pub(super) pulse_plugs: &'l mut PlugList,
    pub(super) latch_plugs: &'l mut PlugList,
    pub runtime: Arc<Runtime>,
    // scope?
}
impl LatchActions<'_>
{
    #[inline]
    pub fn pulse(&mut self, outlet: &PulsedOutlet) { self.pulse_plugs.extend_from_slice(&outlet.plugs); }
    // Attempt to power-on an outlet
    #[inline]
    pub fn latch(&mut self, outlet: &LatchingOutlet) { self.latch_plugs.extend_from_slice(&outlet.plugs); }
    // Attempt to power-off an outlet
    pub fn unlatch(&mut self, outlet: &LatchingOutlet)
    {
        for plug in outlet.plugs.iter()
        {
            self.latch_plugs.push(Plug::new(plug.block, Inlet::PowerOff));
        }
    }
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

// A helper struct that contains all of the types required to test circuit blocks
pub struct TestContext
{
    pub local_scope: LocalScope,
    pub local_changes: ScopeChanges,
    pub shared_scope: SharedScope,
    pub shared_changes: ScopeChanges,

    pub pulse_outlets: PlugList,
    pub latch_outlets: PlugList,
    pub runtime: Arc<Runtime>,
}
impl Default for TestContext
{
    fn default() -> Self
    {
        Self
        {
            local_scope: LocalScope::new(2), // todo: re-eval?
            local_changes: Default::default(),
            shared_scope: Default::default(),
            shared_changes: Default::default(),

            pulse_outlets: Default::default(),
            latch_outlets: Default::default(),
            runtime: Runtime::new(),
        }
    }
}
impl TestContext
{
    pub fn pulse(&mut self, impulse: impl ImpulseBlock)
    {
        impulse.pulse(Scope
        {
            run_id: InstRunId::TEST,
            block_id: BlockId::impulse(0),
            local_scope: &mut self.local_scope,
            local_changes: &mut self.local_changes,
            shared_scope: &self.shared_scope,
            shared_changes: &mut self.shared_changes,
        }, ImpulseActions
        {
            pulse_outlets: &mut self.pulse_outlets,
            runtime: self.runtime.clone(),
        });
    }

    pub fn latch(&mut self, latch: impl LatchBlock)
    {
        latch.power_on(Scope
        {
            run_id: InstRunId::TEST,
            block_id: BlockId::latch(0),
            local_scope: &mut self.local_scope,
            local_changes: &mut self.local_changes,
            shared_scope: &self.shared_scope,
            shared_changes: &mut self.shared_changes,
        }, LatchActions
        {
            pulse_plugs: &mut self.pulse_outlets,
            latch_plugs: &mut self.latch_outlets,
            runtime: self.runtime.clone(),
        });
    }

    pub fn unloatch(&mut self, latch: impl LatchBlock)
    {
        latch.power_off(Scope
        {
            run_id: InstRunId::TEST,
            block_id: BlockId::latch(0),
            local_scope: &mut self.local_scope,
            local_changes: &mut self.local_changes,
            shared_scope: &self.shared_scope,
            shared_changes: &mut self.shared_changes,
        });
    }

    // todo: var change, re-enter
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
