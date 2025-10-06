use std::error::Error;
use crate::blocks::PlugList;
use crate::vars::ScopeChanges;
use crate::{BlockId, ImpulseActions, ImpulseBlock, InstRunId, LatchActions, LatchBlock, LocalScope, Runtime, Scope, SharedScope};
use nab_3l14::Signal;
use std::fmt::Debug;
use std::sync::Arc;
use asset_3l14::{AssetLifecycler, AssetLoadRequest};
use debug_3l14::debug_gui::DebugGui;

#[proc_macros_3l14::asset]
pub struct Circuit
{
    // todo: make pub(crate)

    pub auto_entries: EntryPoints,
    pub signaled_entries: Box<[(Signal, EntryPoints)]>,
    pub impulses: Box<[Box<dyn ImpulseBlock + Send>]>,
    pub latches: Box<[Box<dyn LatchBlock + Send>]>,
    pub num_local_vars: u32,
}

pub type EntryPoints = Box<[BlockId]>;

struct CircuitLifecycler
{

}
impl AssetLifecycler for CircuitLifecycler
{
    type Asset = Circuit;

    fn load(&self, request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>> {
        todo!()
    }
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
        let mut _rs = None;
        impulse.pulse(Scope
        {
            run_id: InstRunId::TEST,
            block_id: BlockId::impulse(0),
            local_scope: &mut self.local_scope,
            local_changes: &mut self.local_changes,
            shared_scope: &self.shared_scope,
            shared_changes: &mut self.shared_changes,
            latch_context: &mut _rs,
        }, ImpulseActions
        {
            pulse_outlets: &mut self.pulse_outlets,
            runtime: self.runtime.clone(),
        });
    }

    pub fn latch(&mut self, latch: impl LatchBlock)
    {
        let mut _rs = None; // store?
        latch.power_on(Scope
        {
            run_id: InstRunId::TEST,
            block_id: BlockId::latch(0),
            local_scope: &mut self.local_scope,
            local_changes: &mut self.local_changes,
            shared_scope: &self.shared_scope,
            shared_changes: &mut self.shared_changes,
            latch_context: &mut _rs,
        }, LatchActions
        {
            pulse_plugs: &mut self.pulse_outlets,
            latch_plugs: &mut self.latch_outlets,
            runtime: self.runtime.clone(),
        });
    }

    pub fn unloatch(&mut self, latch: impl LatchBlock)
    {
        let mut _rs = None; // store?
        latch.power_off(Scope
        {
            run_id: InstRunId::TEST,
            block_id: BlockId::latch(0),
            local_scope: &mut self.local_scope,
            local_changes: &mut self.local_changes,
            shared_scope: &self.shared_scope,
            shared_changes: &mut self.shared_changes,
            latch_context: &mut _rs,
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
