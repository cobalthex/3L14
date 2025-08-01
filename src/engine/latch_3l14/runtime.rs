use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use smallvec::SmallVec;
use crossbeam::channel::{unbounded, Receiver, Sender};
use crossbeam::queue::SegQueue;
use super::{Graph, Instance};
use asset_3l14::Signal;
use crate::instance::Event;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstRunId(u32);

pub struct Runtime
{
    instances: HashMap<InstRunId, Instance>, // TODO: thread-safe/lock free
    instance_id_counter: AtomicU32,
    signals: HashMap<Signal, SmallVec<[(InstRunId, usize); 4]>>,

    // shared scopes
    process_queue: SegQueue<InstRunId>,
}
impl Runtime
{
    #[must_use]
    pub fn new() -> Self
    {
        Self
        {
            instances: HashMap::new(),
            instance_id_counter: AtomicU32::new(1),
            signals: HashMap::new(),
            process_queue: SegQueue::new(),
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
        let inst = Instance::new(graph);
        inst.enqueue_event(Event::AutoEnter);
        self.instances.insert(inst_id, inst);

        for (slot, signal) in signals
        {
            self.signals.entry(signal).or_default().push((inst_id, slot));
        }

        self.process_queue.push(inst_id);
        Some(inst_id)
    }
    // terminate a running instance (async)
    pub fn stop(&mut self, instance_id: InstRunId)
    {
        let Some(inst) = self.instances.get(&instance_id) else { return; };
        inst.enqueue_event(Event::Exit);
        self.process_queue.push(instance_id);
    }

    // drain the event queue
    pub fn process_events(&mut self)
    {
        puffin::profile_function!();

        while let Ok(run_id) = self.process_queue.pop()
        {
            let Some(inst) = self.instances.get(&run_id) else { continue; };
            if inst.is_processing_events.compare_exchange(
                false,
                true,
                Ordering::Relaxed, // relaxed is ok here, b/c all jobs are queued from this one thread
                Ordering::Relaxed)
                .is_ok()
            {
                // enqueue job
            }
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


/*

var changes:
node/external var queues var change which then signals
vars should update/signal immediately -- don't signal to downstream states not yet activated

on_dependency_changed() -> [pulsing outlets]
    - optionally power-off self
*/
