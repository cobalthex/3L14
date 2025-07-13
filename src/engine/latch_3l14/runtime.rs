use std::collections::HashMap;
use smallvec::SmallVec;
use crossbeam::channel::{unbounded, Receiver, Sender};
use super::{Graph, Instance};
use asset_3l14::Signal;
use containers_3l14::{ObjectPool, ObjectPoolToken};

enum Event
{
    SpawnInstance(InstanceId),
    TerminateInstance(InstanceId),

    TriggerEntry(Signal)
}

pub type InstanceId = ObjectPoolToken<Instance>;

pub struct Runtime
{
    instances: ObjectPool<Instance>, // this is probably the wrong approach
    signals: HashMap<Signal, SmallVec<[InstanceId; 4]>>, // regular vec?

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
            instances: ObjectPool::default(), // object pool?
            signals: HashMap::new(),
            event_sender: send,
            event_queue: recv,
        }
    }

    // spawn a new instance of the specified graph (async)
    pub fn start(&mut self, graph: Graph)
    {
        // TODO: error handling
        let inst_tok = self.instances.take_token(|_| Instance::new(graph));
        let _ = self.event_sender.send(Event::SpawnInstance(inst_tok));
    }
    // terminate a running instance (async)
    pub fn stop(&mut self, instance_id: InstanceId)
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
                Event::SpawnInstance(mut instance) =>
                {
                    let mut inst = instance.hydrate(&self.instances);
                    inst.start();
                }
                Event::TerminateInstance(instance) =>
                {

                }
                Event::TriggerEntry(signal) =>
                {

                }
            }
        }
    }
}

impl Drop for Runtime
{
    fn drop(&mut self)
    {
        for instance in self.instances.iter_mut()
        {
            instance.terminate();
        }
        self.process_events();
    }
}