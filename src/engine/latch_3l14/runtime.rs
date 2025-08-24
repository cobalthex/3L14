use std::collections::HashMap;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU32, Ordering};
use crossbeam::channel::{Receiver, Sender};
use smallvec::SmallVec;
use crossbeam::queue::SegQueue;
use super::{BlockId, Circuit, Instance, VarId, VarValue};
use nab_3l14::Signal;
use crate::instance::Event;
use crate::{Scope, SharedScope};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstRunId(u32);
impl InstRunId
{
    #[cfg(test)]
    pub(crate) const TEST: Self = Self(1);
}

pub(super) type BlockRef = (InstRunId, BlockId);

enum Action
{
    Spawn
    {
        circuit: Circuit,
        parent: Option<BlockRef>, // is blockID always necessary? (notify code-backed locations?)
    },
    PowerOn(InstRunId),
    PowerOff(InstRunId),
    Signal(Signal),
    #[cfg(debug_assertions)]
    DebugSetVar(InstRunId, VarId, VarValue),
}

struct RunningInstance
{
    instance: UnsafeCell<Instance>,
    parent: Option<BlockRef>,
}

pub struct Runtime
{
    instances: HashMap<InstRunId, RunningInstance>, // TODO: thread-safe/lock free (dashmap?)
    instance_id_counter: AtomicU32,
    signals: HashMap<Signal, SmallVec<[(InstRunId, usize); 4]>>,

    // shared scopes

    action_queue_send: Sender<Action>,
    action_queue_recv: Receiver<Action>,
}
impl Runtime
{
    #[must_use]
    pub fn new() -> Self
    {
        let (sender, receiver) = crossbeam::channel::unbounded();

        Self
        {
            instances: HashMap::new(),
            instance_id_counter: AtomicU32::new(1),
            signals: HashMap::new(),
            action_queue_send: sender,
            action_queue_recv: receiver,
        }
    }

    // spawn a new instance of the specified circuit (async)
    pub fn spawn(&mut self, circuit: Circuit, parent: Option<BlockRef>) -> InstRunId
    {
        let signals: SmallVec<[_; 8]> = circuit.signaled_entries
            .iter().enumerate()
            .map(|(i, signal)| (i, signal.0))
            .collect();

        // maybe can generate ID from token in the future (need generation probably)
        let inst_id = InstRunId(self.instance_id_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        let instance = Instance::new(circuit);
        self.instances.insert(inst_id,RunningInstance
        {
            instance: UnsafeCell::new(instance),
            parent,
        });

        // TODO: parent

        // todo: error handling
        let _ = self.action_queue_send.send(Action::PowerOn(inst_id));

        for (slot, signal) in signals
        {
            self.signals.entry(signal).or_default().push((inst_id, slot));
        }

        inst_id
    }

    // terminate a running instance (async)
    pub fn stop(&mut self, instance_id: InstRunId)
    {
        let Some(inst) = self.get_instance(instance_id) else { return; };

        unsafe { &* inst.instance.get() }.enqueue_event(Event::Exit);
        // todo: error handling
        let _ = self.action_queue_send.send(Action::PowerOff(instance_id));
    }

    // send a signal, waking one or more instances (async)
    pub fn signal(&mut self, signal: Signal)
    {
        // todo: error handling
        let _ = self.action_queue_send.send(Action::Signal(signal));
    }

    #[inline] #[must_use]
    fn get_instance(&self, instance_id: InstRunId) -> Option<&RunningInstance>
    {
        unsafe { &* self.instances.get(&instance_id) }
    }

    // manually set a variable's value (does propagate)
    #[cfg(debug_assertions)]
    pub fn debug_set_var(&self, run_id: InstRunId, var_id: VarId, value: VarValue)
    {
        let Some(inst) = self.instances.get(&run_id) else { return; };

        // todo: error handling?
        let _ = self.action_queue_send.send(Action::DebugSetVar(run_id, var_id, value));
    }

    // drain the event queue
    pub fn process_events(&mut self)
    {
        puffin::profile_function!();

        fn process_events_instance(
            inst_run_id: InstRunId,
            inst_run: &mut RunningInstance,
            shared_scope: &mut SharedScope)
        {
            let inst_mut = unsafe { &mut* inst_run.instance.get() };
            if !inst_mut.is_processing_events.swap(true, Ordering::AcqRel)
            {
                // TODO: enqueue job
                inst_mut.process_events(&shared_scope, inst_run_id);
            }
        }

        // todo: Error handling?
        'queue_recv:
        while let Ok(action) = self.action_queue_recv.recv()
        {
            match action
            {
                Action::Spawn { circuit: circuit, parent } =>
                {
                    let _ = self.spawn(circuit, parent);
                    continue 'queue_recv;
                }
                Action::PowerOn(run_id) =>
                {
                    (run_id,self.get_instance(run_id)
                        .expect("There should never be a power-off before power-on"))
                }
                Action::PowerOff(run_id) =>
                {
                    let Some(ir) = self.get_instance(run_id) else { continue; };
                    (run_id, ir)
                    // TODO: signal parent
                }
                Action::Signal(signal) =>
                {
                    let Some(insts) = self.signals.get(&signal) else { return; };
                    todo!();
                }
                Action::DebugSetVar(run_id, var, value) =>
                {
todo!();
                }
            };
        }

        // signal to threads they can sleep then wait for all jobs to finish?
    }
}

impl Drop for Runtime
{
    fn drop(&mut self)
    {
        // TODO: shutdown all instances
        self.process_events();
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

        let mut runtime = Runtime::new();
        runtime.spawn(circuit, None);
    }
}


/*

var changes:
node/external var queues var change which then signals
vars should update/signal immediately -- don't signal to downstream latches not yet powered on

on_dependency_changed() -> [pulsing outlets]
    - optionally power-off self
*/
