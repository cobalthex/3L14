use std::collections::HashMap;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU32, Ordering};
use crossbeam::channel::{Receiver, Sender};
use smallvec::SmallVec;
use crossbeam::queue::SegQueue;
use super::{BlockId, Circuit, Instance, VarId, VarValue};
use nab_3l14::Signal;
use crate::instance::Action as InstanceAction;
use crate::{RunContext, SharedScope};
use crate::VarScope::Shared;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstRunId(u32);
impl InstRunId
{
    #[cfg(test)]
    pub(crate) const TEST: Self = Self(1);
}

pub type BlockRef = (InstRunId, BlockId);

pub enum Action
{
    Spawn // spawn a new instance
    {
        circuit: Circuit,
        parent: Option<BlockRef>, // is blockID always necessary? (notify code-backed locations?)
    },
    PowerOn(InstRunId), // power-on a new instance  -- TODO: This should be private
    PowerOff(InstRunId), // terminate a running instance
    Signal(Signal), // power-on any graphs listening to this signal
    #[cfg(debug_assertions)]
    DebugSetVar(InstRunId, VarId, VarValue), // for testing only, set a variable value
}

struct RunningInstance
{
    instance: UnsafeCell<Instance>,
    parent: Option<BlockRef>,
}
impl RunningInstance
{
    fn inst_mut(&self) -> &mut Instance
    {
        unsafe { &mut* self.instance.get() }
    }
}

pub struct Runtime
{
    instances: HashMap<InstRunId, RunningInstance>, // TODO: thread-safe/lock free (dashmap?)
    instance_id_counter: AtomicU32,
    signals: HashMap<Signal, SmallVec<[(InstRunId, u32); 4]>>,

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
            .map(|(i, signal)| (i as u32, signal.0))
            .collect();

        // maybe can generate ID from token in the future (need generation probably)
        let inst_id = InstRunId(self.instance_id_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        let instance = Instance::new(circuit);
        self.instances.insert(inst_id,RunningInstance
        {
            instance: UnsafeCell::new(instance),
            parent,
        });

        // todo: error handling
        let _ = self.action_queue_send.send(Action::PowerOn(inst_id));

        for (slot, signal) in signals
        {
            self.signals.entry(signal).or_default().push((inst_id, slot));
        }

        inst_id
    }

    fn request_action(&mut self, action: Action)
    {
        // TODO: error handling
        let _ = self.action_queue_send.send(action);
    }

    // drain the event queue
    pub fn process_actions(&mut self)
    {
        puffin::profile_function!();

        fn process_instance_actions(
            instance: &mut Instance, // mutability is verified in this function
            run_context: RunContext)
        {
            if !instance.is_processing_actions.swap(true, Ordering::AcqRel)
            {
                // TODO: enqueue job
                instance.process_actions(run_context);
            }
        }

        // todo: Error handling?
        'queue_recv:
        while let Ok(action) = self.action_queue_recv.recv()
        {
            match action
            {
                Action::Spawn { circuit, parent } =>
                {
                    let _ = self.spawn(circuit, parent);
                    continue 'queue_recv;
                }
                Action::PowerOn(run_id) =>
                {
                    let running_inst = self.instances.get(&run_id)
                        .expect("There should never be a power-off before power-on");

                    let shared_scope = SharedScope::default();
                    let inst = running_inst.inst_mut();
                    inst.enqueue_action(InstanceAction::PowerOn);
                    process_instance_actions(inst, RunContext
                    {
                        run_id,
                        shared_scope: &shared_scope,
                        action_sender: self.action_queue_send.clone(),
                    });
                }
                Action::PowerOff(run_id) =>
                {
                    let running_inst = self.instances.get(&run_id)
                        .expect("There should never be a power-off before power-on");

                    let shared_scope = SharedScope::default();
                    let inst = running_inst.inst_mut();
                    inst.enqueue_action(InstanceAction::PowerOff);
                    process_instance_actions(inst, RunContext
                    {
                        run_id,
                        shared_scope: &shared_scope,
                        action_sender: self.action_queue_send.clone(),
                    });
                    
                    // TODO: wake up parent
                }
                Action::Signal(signal) =>
                {
                    let Some(signals) = self.signals.get(&signal) else { continue 'queue_recv; };
                    for (run_id, slot) in signals
                    {
                        let Some(running_inst) = self.instances.get(&run_id) else { continue; };

                        let shared_scope = SharedScope::default();
                        let inst = running_inst.inst_mut();
                        inst.enqueue_action(InstanceAction::Signal(*slot as u32));
                        process_instance_actions(inst, RunContext
                        {
                            run_id: *run_id,
                            shared_scope: &shared_scope,
                            action_sender: self.action_queue_send.clone(),
                        });
                    }
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
        self.process_actions();
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
