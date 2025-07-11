use crossbeam::channel::{unbounded, Receiver, Sender};
use super::Instance;

enum Event
{
    SpawnInstance(/* graph, owner */),
    TerminateInstance(InstanceId),

    TriggerEntry(/*entry trigger*/)
}

pub struct InstanceId(u32);

pub struct Runtime
{
    instances: Vec<Instance>,

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
            instances: Vec::new(),
            event_sender: send,
            event_queue: recv,
        }
    }

    pub fn spawn_instance(&mut self)
    {
        // TODO: error handling
        let _ = self.event_sender.send(Event::SpawnInstance());
    }
    pub fn terminate_instance(&mut self, instance_id: InstanceId)
    {
        let _ = self.event_sender.send(Event::TerminateInstance(instance_id));
    }

    pub fn process_events(&mut self)
    {
        puffin::profile_function!();

        while let Ok(event) = self.event_queue.try_recv()
        {
            match event
            {
                Event::SpawnInstance() => {}
                Event::TerminateInstance(_) => {}
                Event::TriggerEntry() => {}
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