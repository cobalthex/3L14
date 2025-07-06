use std::marker::PhantomData;
use smallvec::SmallVec;
use super::Scope;

// This can probably go into reflection info
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum BlockId
{
    Impulse(u32),
    State(u32),
}
// ensure size of BlockId?

#[derive(Clone, Copy, Default)]
pub enum Inlet
{
    #[default]
    Pulse,
    PowerOff, // ignored by non-stateful blocks
}

#[derive(Clone, Copy)]
pub struct OutletLink
{
    pub block: BlockId,
    pub inlet: Inlet,
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

pub trait Block { }

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
