use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use triomphe::Arc;
use smallvec::SmallVec;
use crate::{Runtime, Scope, VarChange};

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum BlockKind
{
    Impulse = 0,
    Latch = 1,
}

#[derive(Hash, PartialEq, Eq, Clone, Copy, Encode, Decode)]
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
    pub const fn kind(self) -> BlockKind
    {
        unsafe { std::mem::transmute((self.0 >> 31) as u8) }
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
        f.write_fmt(format_args!("[{:?}|{}]", self.kind(), self.value()))
    }
}

pub trait Block: Debug + Send
{
}

// A block that can perform an action whenever they are pulsed
pub trait ImpulseBlock: Block
{
    // Called when this block is pulsed
    fn pulse(&self, scope: Scope, actions: ImpulseActions);

    // Return some basic information about this block, useful for diagnostics/etc
    fn inspect(&self, visit: BlockVisitor);
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

pub struct LatchActions<'l>
{
    pub(super) pulse_plugs: &'l mut PlugList,
    pub(super) latch_plugs: &'l mut PlugList,
    pub runtime: Arc<Runtime>, // should this go into scope?
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
pub trait LatchBlock: Block
{
    // Called when this latch gets powered-on
    fn power_on(&self, scope: Scope, actions: LatchActions);
    // Called as this latch is being powered off
    fn power_off(&self, scope: Scope);
    // Re-enter an already powered-on latch.
    // This is typically used by code-backed wake-ups
    fn re_enter(&self, _scope: Scope, _actions: LatchActions) { }
    // Called when a variable this block is listening to, changes
    fn on_var_changed(&self, _change: VarChange, _scope: Scope, _actions: LatchActions) { }

    // Return some basic information about this block, useful for diagnostics/etc
    fn inspect(&self, visit: BlockVisitor);
}

// A latch block that also maintains an internal (runtime-only) state
pub trait ContextfulLatchBlock: Block // better name?
{
    type Context: Default + Debug + Send + 'static;

    // Called when this latch gets powered-on
    fn power_on(&self, context: &mut Self::Context, scope: Scope, actions: LatchActions);
    // Called as this latch is being powered off
    fn power_off(&self, context: &mut Self::Context, scope: Scope);
    // Re-enter an already powered-on latch.
    // This is typically used by code-backed wake-ups
    fn re_enter(&self, _context: &mut Self::Context, _scope: Scope, _actions: LatchActions) { }
    // Called when a variable this block is listening to, changes
    fn on_var_changed(&self, _context: &mut Self::Context, _change: VarChange, _scope: Scope, _actions: LatchActions) { }

    fn inspect(&self, visit: BlockVisitor);
}
impl<L: ContextfulLatchBlock> LatchBlock for L
{
    #[inline]
    fn power_on(&self, scope: Scope, actions: LatchActions)
    {
        let (context, scope) = scope.unpack_context::<L>();
        L::power_on(self, context, scope, actions)
    }

    #[inline]
    fn power_off(&self, scope: Scope)
    {
        let (context, scope) = scope.unpack_context::<L>();
        L::power_off(self, context, scope)
    }

    #[inline]
    fn re_enter(&self, scope: Scope, actions: LatchActions)
    {
        let (context, scope) = scope.unpack_context::<L>();
        L::re_enter(self, context, scope, actions)
    }

    #[inline]
    fn on_var_changed(&self, change: VarChange, scope: Scope, actions: LatchActions)
    {
        let (context, scope) = scope.unpack_context::<L>();
        L::on_var_changed(self, context, change, scope, actions)
    }

    #[inline]
    fn inspect(&self, visit: BlockVisitor)
    {
        L::inspect(self, visit)
    }
}

// How the target block should behave on pulse
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Inlet
{
    #[default]
    Pulse,
    PowerOff, // ignored by non-latching blocks
}

// Where an outlet points to
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
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
}

// Outlets that pass-thru incoming pulses (but not power-offs)
#[derive(Default, Debug, Encode, Decode)]
pub struct PulsedOutlet
{
    pub plugs: Box<[Plug]>,
}

// Outlets that carry the parent signal and respond to power-offs
// note: Latchin
#[derive(Default, Debug, Encode, Decode)]
pub struct LatchingOutlet
{
    pub plugs: Box<[Plug]>,
}

pub(super) type VisitList<'b> = SmallVec<[(&'b str, SmallVec<[Plug; 2]>); 4]>;

// todo: convert to return object rather than mut?
pub struct BlockVisitor<'b, 'v>
{
    pub(super) name: &'v mut &'b str,
    pub(super) annotation: &'v mut String,
    pub(super) pulses: &'v mut VisitList<'b>,
    pub(super) latches: &'v mut VisitList<'b>,
}
impl<'b> BlockVisitor<'b, '_>
{
    // Define a colloquial name for this block (usually the type name)
    pub fn set_name(&mut self, name: &'b str)
    {
        *self.name = name.as_ref();
    }

    // Optionally provide a comment/note/context for this block
    pub fn annotate(&mut self, annotation: impl AsRef<str>)
    {
        self.annotation.push_str(annotation.as_ref());
    }

    pub fn visit_pulses(&mut self, outlet_name: &'b str, outlet: &PulsedOutlet)
    {
        self.pulses.push((outlet_name.as_ref(), SmallVec::from_slice(&outlet.plugs)));
    }
    pub fn visit_latches(&mut self, outlet_name: &'b str, outlet: &LatchingOutlet)
    {
        self.latches.push((outlet_name.as_ref(), SmallVec::from_slice(&outlet.plugs)));
    }
}
pub(super) type PlugList = SmallVec<[Plug; 2]>;


// The intermediate format of a block that is used for deserializing
pub struct BlockDefz<'p>
{
    pulsed_outlets: HashMap<&'p str, PulsedOutlet>,
    latching_outlets: HashMap<&'p str, LatchingOutlet>,
    // fields: HashMap<&'p str, toml::Value>,
}
