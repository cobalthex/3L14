use std::sync::{Arc, Mutex};
use std::time::Duration;
use dashmap::DashMap;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;
use latch_3l14::{BlockId, BlockVisitor, Circuit, ImpulseBlock, Inlet, InstRunId, LatchActions, LatchBlock, LatchingOutlet, Plug, PulsedOutlet, Runtime, Scope, SharedScope, VarId, VarScope, VarValue};
use latch_3l14::impulses::{DebugPrint, NoOp, SetVars};
use latch_3l14::latches::{ConditionLatch, Latch};
use nab_3l14::Signal;
use nab_3l14::utils::ShortTypeName;

#[wasm_bindgen]
pub fn run_app() -> App
{
    let _ = console_log::init_with_level(log::Level::Debug);
    App::new()
}

static S_LOG: Mutex<String> = Mutex::new(String::new());

#[derive(Debug)]
enum LogType
{
    String(String),
    Var(VarId),
}
struct LogPrint
{
    log: LogType,
    outlet: PulsedOutlet,
}
impl ImpulseBlock for LogPrint
{
    fn pulse(&self, scope: latch_3l14::Scope, mut actions: latch_3l14::ImpulseActions)
    {
        // else panic?
        match S_LOG.lock()
        {
            Ok(mut s_log) =>
            {
                match &self.log
                {
                    LogType::String(s) =>
                    {
                        s_log.push_str("> \"");
                        s_log.push_str(&s);
                        s_log.push_str("\"\n");

                    }
                    LogType::Var(v) =>
                    {
                        s_log.push_str(&format!("> {v:?} = {:?}\n", scope.get(*v)));
                    }
                }
            }
            Err(e) => panic!("Failed to lock log: {}", e),
        }
        actions.pulse(&self.outlet);
    }

    fn inspect(&self, mut visit: BlockVisitor)
    {
        visit.set_name(Self::short_type_name());
        visit.annotate(format!("\"{:?}\"", self.log));
        visit.visit_pulses("Outlet", &self.outlet);
    }
}

fn defer<TFn: Fn() + Send + 'static>(duration: Duration, f: TFn)
{
    #[wasm_bindgen]
    extern "C"
    {
        #[wasm_bindgen(js_name = setTimeout)]
        fn set_timeout(closure: &Closure<dyn Fn()>, time: u32) -> i32;
    }

    let id = Box::new(0);
    let closure = Closure::new(f);
    set_timeout(&closure, duration.as_millis() as u32);
    closure.forget();
}

struct Wait
{
    duration: Duration,
    waiting_outlet: LatchingOutlet,
    finished_outlet: LatchingOutlet,
}
impl LatchBlock for Wait
{
    fn power_on(&self, scope: Scope, mut actions: LatchActions)
    {
        actions.latch(&self.waiting_outlet);
        let block_ref = scope.get_block_ref();
        let rt = actions.runtime.clone();
        defer(self.duration, move ||
        {
            log::debug!("Wait finished");
            rt.re_enter(block_ref.clone());
        });
    }

    fn power_off(&self, _scope: Scope)
    {
    }

    fn re_enter(&self, _scope: Scope, mut actions: LatchActions)
    {
        actions.unlatch(&self.waiting_outlet);
        actions.latch(&self.finished_outlet);
    }

    fn inspect(&self, mut visit: BlockVisitor)
    {
        visit.set_name(Self::short_type_name());
        visit.visit_latches("Waiting", &self.waiting_outlet);
        visit.visit_latches("Finished", &self.finished_outlet);
    }
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
            signaled_entries: Box::new([
                (Signal::test('a'), Box::new([BlockId::impulse(4)])),
                (Signal::test('b'), Box::new([BlockId::latch(0)])),
                (Signal::test('c'), Box::new([BlockId::impulse(6)])),
            ]),
            impulses: Box::new([
                Box::new(LogPrint
                {
                    log: LogType::String("-> false".to_string()),
                    outlet: PulsedOutlet
                    {
                        plugs: Box::new([Plug::new(BlockId::impulse(2), Inlet::Pulse)]),
                    },
                }),
                Box::new(LogPrint
                {
                    log: LogType::String("-> true".to_string()),
                    outlet: Default::default(),
                }),
                Box::new(LogPrint
                {
                    log: LogType::Var(VarId::new(0, VarScope::Local)),
                    outlet: PulsedOutlet
                    {
                        plugs: Box::new([Plug::new(BlockId::impulse(4), Inlet::Pulse)]),
                    },
                }),
                Box::new(LogPrint
                {
                    log: LogType::String("You clicked signal!".to_string()),
                    outlet: Default::default(),
                }),
                Box::new(SetVars
                {
                    var: VarId::new(0, VarScope::Local),
                    to_value: VarValue::Bool(true),
                    outlet: PulsedOutlet
                    {
                        plugs: Box::new([Plug::new(BlockId::impulse(5), Inlet::Pulse)]),
                    },
                }),
                Box::new(LogPrint
                {
                    log: LogType::Var(VarId::new(0, VarScope::Local)),
                    outlet: Default::default(),
                }),
                Box::new(NoOp
                {
                    outlet: PulsedOutlet
                    {
                        plugs: Box::new([
                            Plug::new(BlockId::latch(0), Inlet::PowerOff),
                        ]),
                    },
                }),
                Box::new(DebugPrint
                {
                    message: "DEFERRED".to_string(),
                    outlet: Default::default(),
                })
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
                    powered_outlet: LatchingOutlet
                    {
                        plugs: Box::new([Plug::new(BlockId::latch(2), Inlet::Pulse)]),
                    }
                }),
                Box::new(Wait
                {
                    duration: Duration::from_secs(3),
                    waiting_outlet: Default::default(),
                    finished_outlet: LatchingOutlet
                    {
                        plugs: Box::new([Plug::new(BlockId::impulse(7), Inlet::Pulse)]),
                    },
                })
            ]),
            num_local_vars: 1,
        };

        let runtime = Runtime::new();
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

    pub fn signal(&self)
    {
        self.runtime.signal(Signal::test('a'));
    }

    pub fn signal_on(&self)
    {
        self.runtime.signal(Signal::test('b'));
    }

    pub fn signal_off(&self)
    {
        self.runtime.signal(Signal::test('c'));
    }

    #[must_use]
    pub fn as_graphviz(&self) -> String
    {
        self.runtime.dump_graphviz(self.inst_run_id)
    }

    #[must_use]
    pub fn get_action_history(&self) -> String
    {
        self.runtime.dump_action_history(self.inst_run_id, true)
    }

    #[must_use]
    pub fn get_scope(&self) -> String
    {
        self.runtime.dump_scope(self.inst_run_id)
    }

    #[must_use]
    pub fn get_log(&self) -> String
    {
        match S_LOG.lock()
        {
            Ok(mut s_log) =>
            {
                let log = s_log.clone();
                s_log.clear();
                log
            },
            Err(_) => String::new(),
        }
    }
}