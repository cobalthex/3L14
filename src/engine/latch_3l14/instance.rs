use std::collections::HashMap;
use std::fmt::Debug;
use smallvec::SmallVec;
use super::*;

#[derive(Debug)]
struct HydratedState
{
    is_powered: bool, // flags?
}

pub struct Instance
{
    graph: Graph,
    scope: Scope,

    hydrated_states: HashMap<u32, HydratedState>,

    // TODO: action history
}
impl Instance
{
    #[must_use]
    pub fn new(graph: Graph) -> Self
    {
        puffin::profile_function!();

        let mut new_inst = Self
        {
            graph,
            scope: Scope::default(),
            hydrated_states: HashMap::default(),
        };

        let mut auto_blocks: SmallVec<[BlockId; 8]> = SmallVec::new();
        for entry in &new_inst.graph.entries
        {
            let EntryPoint::Automatic = entry.kind else { continue; };
            auto_blocks.extend_from_slice(&entry.outlet);
        }
        for link in auto_blocks
        {
            new_inst.pulse(link);
        }

        new_inst
    }

    const MAX_PULSE_DEPTH: u32 = 100; // smaller number?

    // TODO: make not pub(crate)
    fn pulse(&mut self, block: BlockId)
    {
        puffin::profile_function!();

        // TODO: track cache misses on small vec

        let mut stack: SmallVec<[(OutletLink, u32); 8]> = smallvec![(OutletLink { block, inlet: Inlet::Pulse }, 0)];

        let mut pulsed_links = OutletLinkList::default();
        let mut latching_links = OutletLinkList::default();

        while let Some((link, depth)) = stack.pop()
        {
            debug_assert!(depth < Self::MAX_PULSE_DEPTH, "Maximum pulse depth exceeded");
            match link.block
            {
                BlockId::Impulse(id) =>
                {
                    let impulse = self.graph.impulses[id as usize].as_ref();
                    // if let Inlet::Pulse = link.inlet (only necessary if merging pulse() and power_off()
                    {
                        impulse.pulse(&mut self.scope);
                    }
                    impulse.visit_outlets(ImpulseOutletVisitor { pulses: &mut pulsed_links });
                }
                BlockId::State(id) =>
                {
                    let mut hydrated = self.hydrated_states.entry(id)
                        .or_insert_with(|| HydratedState { is_powered: false });
                    match link.inlet
                    {
                        Inlet::Pulse =>
                        {
                            if hydrated.is_powered { continue; }

                            hydrated.is_powered = true;
                            let state = self.graph.states[id as usize].as_ref();
                            state.power_on(&mut self.scope);
                            state.visit_powered_outlets(StateOutletVisitor { pulses: &mut pulsed_links, latching: &mut latching_links });
                        }
                        Inlet::PowerOff =>
                        {
                            if !hydrated.is_powered { continue; }

                            // TODO: should this go in reverse order?
                            hydrated.is_powered = false;
                            let state = self.graph.states[id as usize].as_ref();
                            state.power_off(&mut self.scope);
                            state.visit_powered_outlets(StateOutletVisitor { pulses: &mut pulsed_links, latching: &mut latching_links });
                        }
                    }
                }
            };

            for pulse in &pulsed_links
            {
                stack.push((*pulse, depth + 1));
            }
            for latch in &latching_links
            {
                stack.push((*latch, depth + 1));
            }

            pulsed_links.clear();
            latching_links.clear();
        }
    }

    fn power_off(&mut self, block: BlockId)
    {
        puffin::profile_function!();

        // TODO: track cache misses on small vec

        let mut stack: SmallVec<[(BlockId, u32); 8]> = smallvec![(block, 0)];
        let mut powered_states: SmallVec<[u32; 16]> = SmallVec::new();

        let mut pulsed_links = OutletLinkList::default();
        let mut latching_links = OutletLinkList::default();

        while let Some((top_block, depth)) = stack.pop()
        {
            debug_assert!(depth < Self::MAX_PULSE_DEPTH, "Maximum pulse depth exceeded");
            match top_block
            {
                BlockId::Impulse(id) =>
                {
                    let impulse = self.graph.impulses[id as usize].as_ref();
                    impulse.visit_outlets(ImpulseOutletVisitor { pulses: &mut pulsed_links });

                    // impulses themselves can't latch but can be in a chain between states
                    for pulse in &pulsed_links
                    {
                        stack.push((pulse.block, depth + 1));
                    }
                    pulsed_links.clear();
                }
                BlockId::State(id) =>
                {
                    let Some(mut hydrated) = self.hydrated_states.get_mut(&id) else { continue; };
                    if !hydrated.is_powered { continue; }

                    hydrated.is_powered = false;
                    let state = self.graph.states[id as usize].as_ref();
                    state.visit_powered_outlets(StateOutletVisitor { pulses: &mut pulsed_links, latching: &mut latching_links });
                    powered_states.push(id);

                    // only latched outputs carry power-off
                    for latch in &latching_links
                    {
                        stack.push((latch.block, depth + 1));
                    }
                    pulsed_links.clear();
                    latching_links.clear();
                }
            };
        }

        // post-order traversal to shut-off
        for powered in powered_states.iter().rev()
        {
            let state = self.graph.states[*powered as usize].as_ref();
            state.power_off(&mut self.scope);
        }
    }

    #[must_use]
    pub fn state_has_power(&self, state: u32) -> bool
    {
        // assert if not state?
        debug_assert!((state as usize) < self.graph.states.len());
        self.hydrated_states.get(&state).map_or(false, |state| state.is_powered)
    }
}
impl Debug for Instance
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
    {
        f.debug_struct("Instance")
            .field("hydrated states", &self.hydrated_states)
            // TODO
            .finish()
    }
}
impl Drop for Instance
{
    fn drop(&mut self)
    {
        let mut links: SmallVec<[BlockId; 8]> = SmallVec::new();
        for entry in &self.graph.entries
        {
            links.extend_from_slice(&entry.outlet);
        }
        for link in links
        {
            self.power_off(link);
        }

        // TODO: non-latching links will be missed here
        // they will need to be manually located

        debug_assert!(self.hydrated_states.iter().all(|(_, state)| !state.is_powered));
    }
}

#[derive(Default)]
pub struct Scope
{
    vars: HashMap<VarId, Var>,
    // stacked vars
}

#[cfg(test)]
mod tests
{
    use super::*;

    /* TODO tests:
    - recursive power-ons
    - recursive power-offs
    - power-off inlets
    - power-on and power-off carry through inter-chained impulses
    - power-off does not carry through non latching state output
    - all states are powered off upon shutdown
    - states can't be double powered-on
    - states can't be double powered-off
     */

    #[derive(Default)]
    struct TestImpulse
    {
        name: &'static str,
        outlet: PulsedOutlet,
    }
    impl ImpulseBlock for TestImpulse
    {
        fn pulse(&self, scope: &mut Scope)
        {
            println!("Pulsed TestImpulse {}", self.name);
        }

        fn visit_outlets(&self, mut visitor: ImpulseOutletVisitor)
        {
            visitor.visit_pulsed(&self.outlet);
        }
    }

    #[derive(Default)]
    struct TestState
    {
        name: &'static str,
        value: bool,

        on_true_outlet: PulsedOutlet,
        true_outlet: LatchingOutlet,

        on_false_outlet: PulsedOutlet,
        false_outlet: LatchingOutlet,

        any_outlet: LatchingOutlet,
    }
    impl StateBlock for TestState
    {
        fn power_on(&self, scope: &mut Scope)
        {
            println!("Powered on TestState {}", self.name);

            /* TODO:
                dependency change enqueues power-off then power-on (if bool flipped)
             */
        }

        fn power_off(&self, scope: &mut Scope)
        {
            println!("Powered off TestState {}", self.name);
        }

        fn visit_powered_outlets(&self, mut visitor: StateOutletVisitor)
        {
            if self.value
            {
                visitor.visit_pulsed(&self.on_true_outlet);
                visitor.visit_latching(&self.true_outlet);
                visitor.visit_latching(&self.any_outlet);
            }
            else
            {
                visitor.visit_pulsed(&self.on_false_outlet);
                visitor.visit_latching(&self.false_outlet);
                visitor.visit_latching(&self.any_outlet);
            }
        }
    }

    #[test]
    fn basic()
    {
        let graph = Graph
        {
            entries: Box::new(
            [
                EntryBlock
                {
                    kind: EntryPoint::Automatic,
                    outlet: Box::new([BlockId::State(0)]),
                },
            ]),

            impulses: Box::new(
            [
                Box::new(TestImpulse
                {
                    name: "Impulse 0",
                    outlet: PulsedOutlet
                    {
                        links: Box::new([OutletLink { block: BlockId::State(1), inlet: Inlet::Pulse }]),
                    },
                }),
                Box::new(TestImpulse
                {
                    name: "Impulse 1",
                    outlet: Default::default(),
                }),
                Box::new(TestImpulse
                {
                    name: "Impulse 2",
                    outlet: Default::default(),
                }),
                Box::new(TestImpulse
                {
                    name: "Impulse 3",
                    outlet: Default::default(),
                }),
            ]),

            states: Box::new(
            [
                Box::new(TestState
                {
                    name: "State 0 (false)",
                    value: false,
                    on_false_outlet: PulsedOutlet
                    {
                        links: Box::new([OutletLink { block: BlockId::State(0), inlet: Inlet::Pulse }]),
                    },
                    false_outlet: LatchingOutlet
                    {
                        links: Box::new([OutletLink { block: BlockId::State(1), inlet: Inlet::Pulse }]),
                    },
                    true_outlet: LatchingOutlet
                    {
                        links: Box::new([OutletLink { block: BlockId::Impulse(3), inlet: Inlet::Pulse }]),
                    },
                    .. Default::default()
                }),

                Box::new(TestState
                {
                    name: "State 1 (true)",
                    value: true,
                    true_outlet: LatchingOutlet
                    {
                        links: Box::new([OutletLink { block: BlockId::Impulse(2), inlet: Inlet::Pulse }]),
                    },
                    .. Default::default()
                }),
            ]),
        };

        let mut instance = Instance::new(graph);
        println!("\n{:#?}\n", instance);

        assert_eq!(instance.state_has_power(0), true);
        assert_eq!(instance.state_has_power(0), true);

        // TODO: create and verify ordering in action log
    }
}