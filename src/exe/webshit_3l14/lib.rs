use wasm_bindgen::prelude::wasm_bindgen;
use latch_3l14::{Circuit, InstRunId, Instance, Runtime, SharedScope};

#[wasm_bindgen]
extern
{
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet()
{
    alert("Hello");
}

#[wasm_bindgen]
pub struct App
{
    inst_run_id: InstRunId,
    shared_scope: SharedScope,
    runtime: Runtime,

}
impl App
{
    #[must_use]
    pub fn new() -> Self
    {
        let graph = Circuit
        {
            auto_entries: Box::new([]),
            signaled_entries: Box::new([]),
            impulses: Box::new([]),
            latches: Box::new([]),
            num_local_vars: 0,
        };

        let mut runtime = Runtime::new();
        let inst_run_id = runtime.spawn(graph);

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
}