use std::sync::Arc;
use wasm_bindgen::prelude::wasm_bindgen;
use latch_3l14::{BlockId, Circuit, Inlet, InstRunId, Instance, LatchingOutlet, Plug, PulsedOutlet, Runtime, SharedScope, VarId, VarScope};
use latch_3l14::impulses::DebugPrint;
use latch_3l14::latches::{ConditionLatch, Latch};

#[wasm_bindgen]
extern
{
}

#[wasm_bindgen]
pub fn run_app() -> App
{
    App::new()
}

#[wasm_bindgen]
pub struct App
{
    inst_run_id: InstRunId,
    shared_scope: SharedScope,
    runtime: Arc<Runtime>,

}
#[wasm_bindgen]
impl App
{
    #[must_use]
    pub fn new() -> Self
    {
        let graph = Circuit
        {
            auto_entries: Box::new([BlockId::latch(0)]),
            signaled_entries: Box::new([]),
            impulses: Box::new([
                Box::new(DebugPrint
                {
                    message: "false".to_string(),
                    outlet: Default::default(),
                }),
                Box::new(DebugPrint
                {
                    message: "true".to_string(),
                    outlet: Default::default(),
                }),
            ]),
            latches: Box::new([
                Box::new(Latch
                {
                    powered_outlet: LatchingOutlet
                    {
                        plugs: Box::new([Plug::new(BlockId::latch(1), Inlet::Pulse)]),
                    },
                }),
                Box::new(ConditionLatch
                {
                    condition: VarId::new(0, VarScope::Local),
                    on_true_outlet: PulsedOutlet
                    {
                        plugs: Box::new([Plug::new(BlockId::impulse(1), Inlet::Pulse)]),
                    },
                    true_outlet: Default::default(),
                    on_false_outlet: PulsedOutlet
                    {
                        plugs: Box::new([Plug::new(BlockId::impulse(0), Inlet::Pulse)]),
                    },
                    false_outlet: Default::default(),
                    powered_outlet: Default::default(),
                }),
            ]),
            num_local_vars: 1,
        };

        let mut runtime = Runtime::new();
        let inst_run_id = runtime.spawn(graph, None);

        Self
        {
            inst_run_id,
            shared_scope: SharedScope::default(),
            runtime,
        }
    }

    pub fn flip_switch(&mut self)
    {
        
    }

    pub fn as_graphviz(&self) -> String
    {
        self.runtime.dump_graphviz(self.inst_run_id)
    }
}