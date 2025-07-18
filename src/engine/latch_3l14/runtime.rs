use std::collections::HashMap;
use std::sync::atomic::AtomicU32;
use smallvec::SmallVec;
use crossbeam::channel::{unbounded, Receiver, SendError, Sender};
use super::{Graph, Instance};
use asset_3l14::Signal;
use containers_3l14::{ReusePool, ObjectPoolEntryGuard, ObjectPoolToken};

enum Event
{
    SpawnInstance(InstRunId),
    TerminateInstance(InstRunId),

    TriggerEntry(Signal)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstRunId(u32);

pub struct Runtime
{
    instances: HashMap<InstRunId, Instance>, // TODO: thread-safe/lock free
    instance_id_counter: AtomicU32,
    signals: HashMap<Signal, SmallVec<[(InstRunId, usize); 4]>>,

    event_sender: Sender<Event>,
    event_queue: Receiver<Event>,

    // shared scopes

}
impl Runtime
{
    #[must_use]
    pub fn new() -> Self
    {
        let (send, recv) = unbounded();

        Self
        {
            instances: HashMap::new(),
            instance_id_counter: AtomicU32::new(1),
            signals: HashMap::new(),
            event_sender: send,
            event_queue: recv,
        }
    }

    // spawn a new instance of the specified graph (async)
    pub fn spawn(&mut self, graph: Graph) -> Option<InstRunId>
    {
        let signals: SmallVec<[_; 8]> = graph.signaled_entries
            .iter().enumerate()
            .map(|(i, signal)| (i, signal.0))
            .collect();

        // maybe can generate ID from token in the future (need generation probably)
        let inst_id = InstRunId(self.instance_id_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        self.instances.insert(inst_id, Instance::new(graph));

        for (slot, signal) in signals
        {
            self.signals.entry(signal).or_default().push((inst_id, slot));
        }

        match self.event_sender.send(Event::SpawnInstance(inst_id))
        {
            Ok(_) => Some(inst_id),
            Err(_) => None, // todo: real error handling
        }
    }
    // terminate a running instance (async)
    pub fn stop(&mut self, instance_id: InstRunId)
    {
        // TODO: error handling
        let _ = self.event_sender.send(Event::TerminateInstance(instance_id));
    }

    // drain the event queue
    pub fn process_events(&mut self)
    {
        puffin::profile_function!();

        // approach A:
        // create thread pool and assign instances to particular threads
        // forward events to thread local queues
        // + can process events efficiently w/out locking
        // - creates dedicated threads that could be heavy/hog cores

        // approach B:
        // create jobs, each job owns an instance for the duration of the job
        // jobs exist only to perform down-stream effects of that one job
        // shared scope data gets queued, all other side effects run locally
        // + fill in work space gaps more nicely, more atomic
        // - how
        // perhaps:
        // jobs spun up for side effects, each job owns an instance until the job exits
        // all events go to that job while active


        while let Ok(event) = self.event_queue.try_recv()
        {
            match event
            {
                Event::SpawnInstance(mut instance_id) =>
                {
                    let instance = &mut self.instances.get_mut(&instance_id).expect("Failed to find just-created instance");
                    instance.power_on();
                }
                Event::TerminateInstance(instance_id) =>
                {
                    let Some(mut removed) = self.instances.remove(&instance_id) else { continue; };
                    removed.power_off();

                    for (i, signal) in removed.graph().signaled_entries.iter().enumerate()
                    {
                        let signals = self.signals.get_mut(&signal.0).expect("Signals weren't hooked up for just-removed instance");
                        signals.retain(|(id, _)| *id != instance_id);
                    }
                }
                Event::TriggerEntry(signal) =>
                {
                    let Some(instances) = self.signals.get(&signal) else { continue; };
                    for (instance_id, slot) in instances.iter()
                    {
                        // TODO: guarantee atomicity when removing instance?
                        let Some(instance) =self.instances.get_mut(&instance_id) else { continue; };
                        instance.signal(*slot);
                    }
                }
            }
        }
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
        let graph = Graph
        {
            auto_entries: Box::new([]),
            signaled_entries: Box::new([]),
            impulses: Box::new([]),
            states: Box::new([]),
        };

        let mut runtime = Runtime::new();
        runtime.spawn(graph);
    }
}