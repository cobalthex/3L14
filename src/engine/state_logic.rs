use crate::define_runtime_id_u32;

define_runtime_id_u32!(NodeId);

#[derive(Debug, Clone, PartialEq)]
pub struct PlugId(&'static str); // todo: not a string


pub struct Node
{
    id: NodeId,
    pub inputs: Vec<PlugId>,
}
impl Node
{
    fn id(&self) -> &NodeId { &self.id }
}

pub struct Graph
{
    entry_points: Vec<()>, // TODO
}
impl Graph
{
    fn new() -> Self { Self
    {
        entry_points: Vec::new(),
    }}

    pub fn run(&self) -> Instance { Instance::new(&self) }
}

pub struct Instance<'g>
{
    graph: &'g Graph,
}

impl<'g> Instance<'g>
{
    fn new(graph: &'g Graph) -> Self { Self
    {
        graph: graph,
    }}
}

struct StateConditional
{
    prev_value: Option<bool>,
    pub predicate: fn() -> bool,
}
impl StateConditional
{

}

struct ActionDebugPrint
{
    pub message: String,
}
impl ActionDebugPrint
{

}



/*

node structs are systems in the graph ECS
nodes can be pulsed, which can then activate/deactivate, perform actions,
todo: figure out graph logic
*/