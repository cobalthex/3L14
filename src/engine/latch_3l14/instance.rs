use std::collections::HashMap;
use std::fmt::Debug;
use smallvec::SmallVec;
use super::*;

/* TODO: at build time:
- eliminate entries that point nowhere
- graphs that have no entrypoint
- states with only power-off inlets
- guarantee block index order? (lower numbers guaranteed to be closer to root?)
- (currently) disable multiple links to a single inlet
    - possible design alt for 'all': blocks keep a count of number of links per inlet, runtime info tracks powered links
 */

#[derive(Debug)]
enum Action
{
    Entry(EntryPoint, u32),
    // Exit
    Terminate,
    Pulse(OutletLink),
    PowerOn(BlockId),
    Poweroff(BlockId),
}

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

    #[cfg(feature = "action_history")]
    action_history: Vec<Action>, // ring buffer?
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
            #[cfg(feature = "action_history")]
            action_history: Vec::new(),
        };

        let mut auto_blocks: SmallVec<[(BlockId, u32); 8]> = SmallVec::new();
        for (i, entry) in new_inst.graph.entries.iter().enumerate()
        {
            let EntryPoint::Automatic = entry.kind else { continue; };
            for outlet in &entry.outlet
            {
                auto_blocks.push((*outlet, i as u32));
            }
        }
        for (block, i) in auto_blocks
        {
            new_inst.push_action(Action::Entry(EntryPoint::Automatic, i));
            new_inst.pulse(OutletLink { block, inlet: Inlet::Pulse });
        }

        new_inst
    }

    #[inline]
    fn push_action(&mut self, action: Action)
    {
        #[cfg(feature = "action_history")]
        self.action_history.push(action);
    }
    #[inline]
    fn clear_action_history(&mut self)
    {
        #[cfg(feature = "action_history")]
        self.action_history.clear();
    }

    const MAX_PULSE_DEPTH: u32 = 100; // smaller number?

    fn pulse(&mut self, block: OutletLink)
    {
        puffin::profile_function!();

        // TODO: track cache misses on small vec

        let mut stack: SmallVec<[(OutletLink, u32); 8]> = smallvec![(block, 0)];

        let mut pulsed_links = OutletLinkList::default();
        let mut latching_links = OutletLinkList::default();

        let mut powering_off_states: SmallVec<[u32; 16]> = SmallVec::new();

        while let Some((link, depth)) = stack.pop()
        {
            debug_assert!(depth < Self::MAX_PULSE_DEPTH, "Maximum pulse depth exceeded");

            self.push_action(Action::Pulse(link));

            if link.block.is_impulse()
            {
                let impulse = self.graph.impulses[link.block.value() as usize].as_ref();
                if let Inlet::Pulse = link.inlet
                {
                    impulse.pulse(&mut self.scope);
                }
                impulse.visit_outlets(ImpulseOutletVisitor { pulses: &mut pulsed_links });
            }
            else
            {
                let hydrated = self.hydrated_states.entry(link.block.value())
                    .or_insert_with(|| HydratedState { is_powered: false });
                match link.inlet
                {
                    Inlet::Pulse =>
                    {
                        if hydrated.is_powered { continue; }

                        hydrated.is_powered = true;
                        self.push_action(Action::PowerOn(link.block));

                        let state = self.graph.states[link.block.value() as usize].as_ref();
                        state.power_on(&mut self.scope);
                        state.visit_powered_outlets(StateOutletVisitor { pulses: &mut pulsed_links, latching: &mut latching_links });
                    }
                    Inlet::PowerOff =>
                    {
                        if !hydrated.is_powered { continue; }

                        hydrated.is_powered = false;
                        powering_off_states.push(link.block.value());

                        let state = self.graph.states[link.block.value() as usize].as_ref();
                        state.visit_powered_outlets(StateOutletVisitor { pulses: &mut pulsed_links, latching: &mut latching_links });
                        pulsed_links.clear(); // only latching links respect power-offs
                    }
                }
            };

            for pulse in &pulsed_links
            {
                stack.push((pulse.poison(link.inlet), depth + 1));
            }
            for latch in &latching_links
            {
                stack.push((latch.poison(link.inlet), depth + 1));
            }

            pulsed_links.clear();
            latching_links.clear();

            // post-order traversal to shut-off
            for powered in powering_off_states.iter().rev()
            {
                let state = self.graph.states[*powered as usize].as_ref();
                state.power_off(&mut self.scope);
            }
        }
    }

    #[inline] #[must_use]
    pub fn state_has_power(&self, state: u32) -> bool
    {
        debug_assert!((state as usize) < self.graph.states.len());
        self.hydrated_states.get(&state).map_or(false, |state| state.is_powered)
    }

    #[inline] #[must_use]
    pub fn any_states_powered(&self) -> bool
    {
        self.hydrated_states.iter().any(|(_, state)| state.is_powered)
    }

    pub fn as_graphviz(&self, mut writer: impl std::io::Write) -> std::io::Result<()>
    {
        writer.write_fmt(format_args!("digraph {{\n  rankdir=LR\n  splines=ortho\n"))?; // name?

        let mut stack: SmallVec<[BlockId; 16]> = SmallVec::new();
        for entry in &self.graph.entries
        {
            stack.extend_from_slice(&entry.outlet);
        }

        let mut pulsed_links = OutletLinkList::default();
        let mut latching_links = OutletLinkList::default();

        while let Some(block) = stack.pop()
        {
            if block.is_impulse()
            {
                let impulse = self.graph.impulses[block.value() as usize].as_ref();
                writer.write_fmt(format_args!("  \"{:?}\" [label=\"{}\\n\\N\" shape=\"box\"]\n",
                    block,
                    "impulse"))?; // TODO: type name

                impulse.visit_outlets(ImpulseOutletVisitor { pulses: &mut pulsed_links });
            }
            else
            {
                let hydrated = self.hydrated_states.get(&block.value());
                let state = self.graph.states[block.value() as usize].as_ref();

                let is_powered = hydrated.map(|h| h.is_powered).unwrap_or(false);
                writer.write_fmt(format_args!("  \"{:?}\" [label=\"{}\\n\\N\" shape=\"{}\"]\n",
                    block,
                    "state", // TODO: type name
                    if is_powered { "doubleoctagon" } else { "octagon" }))?;

                state.visit_powered_outlets(StateOutletVisitor { pulses: &mut pulsed_links, latching: &mut latching_links });
            }

            for pulse in &pulsed_links
            {
                stack.push(pulse.block);
                writer.write_fmt(format_args!("  \"{:?}\" -> \"{:?}\" [minlen=3 taillabel=\"{}\" headlabel=\"{:?}\"]\n",
                    block,
                    pulse.block,
                    "Pulsed (TODO NAME)",
                    pulse.inlet))?;
            }
            for latch in &latching_links
            {
                stack.push(latch.block);
                writer.write_fmt(format_args!("  \"{:?}\" -> \"{:?}\" [color=\"black:invis:black\" minlen=3 taillabel=\"{}\" headlabel=\"{:?}\"]\n",
                    block,
                    latch.block,
                    "Latching (TODO NAME)",
                    latch.inlet))?;
            }

            pulsed_links.clear();
            latching_links.clear();
        }

        writer.write_fmt(format_args!("}}"))
    }
}
impl Debug for Instance
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
    {
        let mut s = f.debug_struct("Instance");
        s.field("hydrated states", &self.hydrated_states);

        #[cfg(feature = "action_history")]
        {
            s.field("action history", &self.action_history);
        }

        s.finish()
    }
}
impl Drop for Instance
{
    fn drop(&mut self)
    {
        self.push_action(Action::Terminate);

        // iter all powered states and power-off
        let mut powered_states = Vec::new();
        for (id, state) in &self.hydrated_states
        {
            if state.is_powered
            {
                powered_states.push(*id);
            }
        }
        powered_states.sort_unstable(); // TODO: guarantee state ordering?

        // states that are already powered-off should no-op
        for ps in powered_states
        {
            self.pulse(OutletLink { block: BlockId::state(ps), inlet: Inlet::PowerOff });
        }

        println!("!!! {self:#?}");

        // TODO: remove once more sure of design
        debug_assert!(!self.any_states_powered(), "Instance still has powered states after termination");
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
    - power-off (and pulse) inlets
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
                    outlet: Box::new([BlockId::state(0)]),
                },
            ]),

            impulses: Box::new(
            [
                Box::new(TestImpulse
                {
                    name: "Impulse 0",
                    outlet: PulsedOutlet
                    {
                        links: Box::new([OutletLink { block: BlockId::state(1), inlet: Inlet::Pulse }]),
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
                        links: Box::new([OutletLink { block: BlockId::impulse(1), inlet: Inlet::Pulse }]),
                    },
                    false_outlet: LatchingOutlet
                    {
                        links: Box::new([OutletLink { block: BlockId::impulse(0), inlet: Inlet::Pulse }]),
                    },
                    true_outlet: LatchingOutlet
                    {
                        links: Box::new([OutletLink { block: BlockId::impulse(3), inlet: Inlet::Pulse }]),
                    },
                    .. Default::default()
                }),

                Box::new(TestState
                {
                    name: "State 1 (true)",
                    value: true,
                    true_outlet: LatchingOutlet
                    {
                        links: Box::new([OutletLink { block: BlockId::impulse(2), inlet: Inlet::Pulse }]),
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

        let mut s = Vec::new();
        let _ = instance.as_graphviz(&mut s);
        println!("{}\n ", String::from_utf8(s).unwrap());

        instance.clear_action_history();
        instance.pulse(OutletLink { block: BlockId::state(0), inlet: Inlet::PowerOff });
        println!("\n{:#?}\n", instance);
    }
}