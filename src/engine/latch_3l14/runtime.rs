use super::{BlockId, Circuit, Instance};
use crate::{RunContext, SharedScope};
use crossbeam::queue::SegQueue;
use dashmap::DashMap;
use nab_3l14::Signal;
use parking_lot::Mutex;
use smallvec::SmallVec;
use std::fmt::{Debug, Formatter};
use std::sync::atomic::AtomicU32;
use triomphe::Arc;

/* TODO
- ability to set initial scope
- scope and state serialization/deserialization
 */

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct InstRunId(u32);
impl InstRunId
{
    pub(super) const TEST: Self = Self(1);
}

#[derive(PartialEq, Eq, Clone)]
pub struct BlockRef(pub(super) InstRunId, pub(super) BlockId);
impl Debug for BlockRef
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        f.write_fmt(format_args!("Ⅱ{}{:?}", self.0.0, self.1))
    }
}

enum InstanceAction
{
    PowerOn,
    Signal(u32),
    PowerOff,
    ReEnter(BlockId),
}

struct RunningInstance
{
    instance: Mutex<Instance>,
    pending_actions: SegQueue<InstanceAction>,
    parent: Option<BlockRef>,
}

pub struct Runtime
{
    instances: DashMap<InstRunId, RunningInstance>,
    instance_id_counter: AtomicU32,
    signals: DashMap<Signal, SmallVec<[(InstRunId, u32); 4]>>,

    // shared scopes
}
impl Runtime
{
    #[must_use]
    pub fn new() -> Arc<Self>
    {
        Arc::new(Self
        {
            instances: DashMap::new(),
            instance_id_counter: AtomicU32::new(1),
            signals: DashMap::new(),
        })
    }

    #[must_use]
    pub fn dump_graphviz(&self, inst_run_id: InstRunId) -> String
    {
        let inst = self.instances.get(&inst_run_id).expect("Instance not found");
        inst.instance.lock().as_graphviz()
    }

    // Get a log of all actions taken. returns an empty string if feature(action_history) is not enabled
    #[must_use]
    pub fn dump_action_history(&self, inst_run_id: InstRunId, clear: bool) -> String
    {
        let inst = self.instances.get(&inst_run_id).expect("Instance not found");
        let mut locked = inst.instance.lock();
        let mut history = String::new();
        for hist in locked.get_action_history()
        {
            history.push_str(&format!("{:?}\n", hist));
        }
        if clear
        {
            locked.clear_action_history();
        }
        history
    }

    #[must_use]
    pub fn dump_scope(&self, inst_run_id: InstRunId) -> String
    {
        let inst = self.instances.get(&inst_run_id).expect("Instance not found");
        format!("{:#?}", inst.instance.lock().local_scope())
    }

    // spawn a new instance of the specified circuit (async)
    pub fn spawn(runtime: &Arc<Self>, circuit: Arc<Circuit>, parent: Option<BlockRef>) -> InstRunId
    {
        let signals: SmallVec<[_; 8]> = circuit.signaled_entries
            .iter().enumerate()
            .map(|(i, signal)| (i as u32, signal.0))
            .collect();

        // maybe can generate ID from token in the future (need generation probably)
        let inst_id = InstRunId(runtime.instance_id_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        let instance = Instance::new(circuit);

        let pending_actions = SegQueue::new();
        pending_actions.push(InstanceAction::PowerOn);

        let new_inst = runtime.instances.entry(inst_id).insert(RunningInstance
        {
            instance: Mutex::new(instance),
            pending_actions,
            parent,
        });

        // todo: error handling

        for (slot, signal) in signals
        {
            runtime.signals.entry(signal).or_default().push((inst_id, slot));
        }

        Self::run_instance(runtime.clone(), inst_id, &new_inst);

        inst_id
    }

    // Power-off and remove a running instance
    pub fn destroy(runtime: &Arc<Self>, run_id: InstRunId)
    {
        puffin::profile_function!();

        let Some((_, running_inst)) = runtime.instances.remove(&run_id) else { return; };

        // it is ok if signals go to instances that don't exist anymore
        // TODO: this can supposedly deadlock if also iterating elsewhere
        for mut sig in runtime.signals.iter_mut()
        {
            sig.retain(|(inst, _)| *inst != run_id);
        }

        running_inst.pending_actions.push(InstanceAction::PowerOff);
        Self::run_instance(runtime.clone(), run_id, &running_inst); // TODO: once threaded, this will need to take ownership of running_inst

        // this must run after ^ finishes
        if let Some(parent) = running_inst.parent
        {
            Self::re_enter(runtime, parent);
        }
    }

    // Power-off a running instance. It can be restarted via signals
    pub fn power_off(runtime: &Arc<Self>, run_id: InstRunId)
    {
        let running_inst = runtime.instances.get(&run_id)
            .expect("There should never be a power-off before power-on");
        running_inst.pending_actions.push(InstanceAction::PowerOff);
        Self::run_instance(runtime.clone(), run_id, &running_inst);

        // TODO: wake up parent (instance should probably send this?)
    }

    // Emit a signal and wake up all listening circuits
    pub fn signal(runtime: &Arc<Self>, signal: Signal)
    {
        let Some(signals) = runtime.signals.get(&signal) else { return; };
        for (run_id, slot) in signals.iter()
        {
            let Some(running_inst) = runtime.instances.get(run_id) else { continue; };
            running_inst.pending_actions.push(InstanceAction::Signal(*slot));
            Self::run_instance(runtime.clone(), *run_id, &running_inst);
        }
    }

    // Re-enter a powered block (in a specific circuit). Used by both super-circuits and code-backed listeners
    pub fn re_enter(runtime: &Arc<Self>, block_ref: BlockRef)
    {
        // re-enter reason?

        let running_inst = runtime.instances.get(&block_ref.0)
            .expect("There should never be a power-off before power-on");
        running_inst.pending_actions.push(InstanceAction::ReEnter(block_ref.1));
        Self::run_instance(runtime.clone(), block_ref.0, &running_inst);
    }

    // drain the action queue for a running instance
    fn run_instance(runtime: Arc<Self>, run_id: InstRunId, instance: &RunningInstance) // better name?
    {
        puffin::profile_function!();

        let Some(mut inst_mut) = instance.instance.try_lock() else { return; };

        let shared_scope = SharedScope::default(); // TODO
        let context = RunContext
        {
            run_id,
            shared_scope: &shared_scope,
            runtime,
        };

        while let Some(action) = instance.pending_actions.pop()
        {
            match action
            {
                InstanceAction::PowerOn => inst_mut.power_on(context.clone()),
                InstanceAction::PowerOff => inst_mut.power_off(context.clone()),
                InstanceAction::Signal(slot) => inst_mut.signal(slot as usize, context.clone()),
                InstanceAction::ReEnter(block_id) => inst_mut.re_enter(block_id, context.clone()),
            }
        }

        // TODO: auto-destroy if no way to re-enter?
        // if !inst_mut.any_latches_powered() &&
        //     inst_mut.circuit().signaled_entries.is_empty()
        // {
        // }
    }
}

impl Drop for Runtime
{
    fn drop(&mut self)
    {
        // TODO: shutdown all instances
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn basic()
    {
        let circuit = Circuit
        {
            auto_entries: Box::new([]),
            signaled_entries: Box::new([]),
            impulses: Box::new([]),
            latches: Box::new([]),
            num_local_vars: 0,
        };

        Runtime::spawn(&Runtime::new(), Arc::new(circuit), None);
    }
}

// TODO: make sure subcircuits work