use std::collections::HashMap;
use std::fmt::Debug;
use crossbeam::channel::{unbounded, Receiver, Sender};
use smallvec::SmallVec;
use asset_3l14::Signal;
use super::*;

/* TODO: at build time:
- allow multiple entrypoints during design time but merge into one
- graphs that have no entrypoint
- states with only power-off inlets
- guarantee block index order? (lower numbers guaranteed to be closer to root?)
- (currently) disable multiple links to a single inlet
    - possible design alt for 'all': blocks keep a count of number of links per inlet, runtime info tracks powered links
 */

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum Action
{
    AutoEntry,
    SignaledEntry(Signal),
    // Exit
    Stop,
    Visit(Plug),
    Pulse(BlockId),
    PowerOn(BlockId),
    PowerOff(BlockId),
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

    #[cfg(any(test, feature = "action_history"))]
    action_history: Vec<Action>, // ring buffer?
}
impl Instance
{
    #[inline] #[must_use]
    pub fn new(graph: Graph) -> Self
    {
        Self
        {
            graph,
            scope: Scope::default(),
            hydrated_states: HashMap::default(),

            #[cfg(any(test, feature = "action_history"))]
            action_history: Vec::new(),
        }
    }

    #[inline] #[must_use]
    pub fn graph(&self) -> &Graph { &self.graph }

    pub(crate) fn power_on(&mut self)
    {
        self.push_action(Action::AutoEntry);
        let auto_blocks: SmallVec<[BlockId; 8]> = SmallVec::from_slice(&self.graph.auto_entries);
        for block in auto_blocks
        {
            self.pulse(Plug { target: block, inlet: Inlet::Pulse });
        }
    }

    pub(crate) fn signal(&mut self, signal_slot: usize)
    {
        let mut outlets = SmallVec::<[_; 2]>::new();
        self.push_action(Action::SignaledEntry(self.graph.signaled_entries[signal_slot].0));
        outlets.extend_from_slice(&self.graph.signaled_entries[signal_slot].1);
        for block in outlets
        {
            self.pulse(Plug { target: block, inlet: Inlet::Pulse });
        }
    }

    #[inline] #[allow(unused_variables)]
    fn push_action(&mut self, action: Action)
    {
        #[cfg(any(test, feature = "action_history"))]
        self.action_history.push(action);
    }
    #[inline]
    pub(crate) fn clear_action_history(&mut self)
    {
        #[cfg(any(test, feature = "action_history"))]
        self.action_history.clear();
    }
    #[inline] #[must_use]
    pub(crate) fn get_action_history(&self) -> &[Action]
    {
        #[cfg(any(test, feature = "action_history"))]
        { &self.action_history }
        #[cfg(not(any(test, feature = "action_history")))]
        { &[] }
    }

    const MAX_PULSE_DEPTH: u32 = 100; // smaller number?

    fn pulse(&mut self, plug: Plug)
    {
        puffin::profile_function!();

        // TODO: track cache misses on small vec

        let mut stack: SmallVec<[(Plug, u32); 8]> = smallvec![(plug, 0)];

        let mut pulsed_links = OutletLinkList::default();
        let mut latching_links = OutletLinkList::default();

        let mut powering_off_states: SmallVec<[u32; 16]> = SmallVec::new();

        while let Some((tp, depth)) = stack.pop()
        {
            debug_assert!(depth < Self::MAX_PULSE_DEPTH, "Maximum pulse depth exceeded");

            self.push_action(Action::Visit(tp));

            if tp.target.is_impulse()
            {
                let impulse = self.graph.impulses[tp.target.value() as usize].as_ref();
                if let Inlet::Pulse = tp.inlet
                {
                    impulse.pulse(&mut self.scope);

                    // stupid rust mutability rules
                    // ordering here to match state ordering
                    #[cfg(any(test, feature = "action_history"))]
                    self.action_history.push(Action::Pulse(tp.target));
                }
                impulse.visit_outlets(ImpulseOutletVisitor { pulses: &mut pulsed_links });
            }
            else
            {
                let hydrated = self.hydrated_states.entry(tp.target.value())
                    .or_insert_with(|| HydratedState { is_powered: false });
                match tp.inlet
                {
                    Inlet::Pulse =>
                    {
                        if hydrated.is_powered { continue; }

                        hydrated.is_powered = true;

                        let state = self.graph.states[tp.target.value() as usize].as_ref();
                        state.power_on(&mut self.scope);
                        state.visit_powered_outlets(StateOutletVisitor { pulses: &mut pulsed_links, latching: &mut latching_links });

                        self.push_action(Action::PowerOn(tp.target));
                    }
                    Inlet::PowerOff =>
                    {
                        if !hydrated.is_powered { continue; }

                        // possible optimization: track if there's any downstream states
                        // don't traverse if no states

                        hydrated.is_powered = false;
                        powering_off_states.push(tp.target.value());

                        let state = self.graph.states[tp.target.value() as usize].as_ref();
                        state.visit_powered_outlets(StateOutletVisitor { pulses: &mut pulsed_links, latching: &mut latching_links });
                        pulsed_links.clear(); // only latching links respect power-offs
                    }
                }
            };

            for pulse in &pulsed_links
            {
                stack.push((pulse.poison(tp.inlet), depth + 1));
            }
            for latch in &latching_links
            {
                stack.push((latch.poison(tp.inlet), depth + 1));
            }

            pulsed_links.clear();
            latching_links.clear();
        }

        // post-order traversal to shut-off
        for powered in powering_off_states.iter().rev()
        {
            let state = self.graph.states[*powered as usize].as_ref();
            state.power_off(&mut self.scope);
            self.push_action(Action::PowerOff(BlockId::state(*powered)));
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
        writer.write_fmt(format_args!("  \"auto_entry\" [label=\"Automatic entry\" shape=\"box\"]\n",))?;
        for outlet in &self.graph.auto_entries
        {
            stack.push(*outlet);
            writer.write_fmt(format_args!("  \"auto_entry\" -> \"{:?}\" [minlen=3]\n",
                outlet))?;
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
                stack.push(pulse.target);
                writer.write_fmt(format_args!("  \"{:?}\" -> \"{:?}\" [minlen=3 taillabel=\"{}\" headlabel=\"{:?}\"]\n",
                                              block,
                                              pulse.target,
                                              "Pulsed (TODO NAME)",
                                              pulse.inlet))?;
            }
            for latch in &latching_links
            {
                stack.push(latch.target);
                writer.write_fmt(format_args!("  \"{:?}\" -> \"{:?}\" [color=\"black:invis:black\" minlen=3 taillabel=\"{}\" headlabel=\"{:?}\"]\n",
                                              block,
                                              latch.target,
                                              "Latching (TODO NAME)",
                                              latch.inlet))?;
            }

            pulsed_links.clear();
            latching_links.clear();
        }

        writer.write_fmt(format_args!("}}"))
    }

    // power off all blocks immediately
    pub fn power_off(&mut self)
    {
        puffin::profile_function!();

        // free memory?
        self.push_action(Action::Stop);

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
            self.pulse(Plug { target: BlockId::state(ps), inlet: Inlet::PowerOff });
        }
    }
}
impl Debug for Instance
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
    {
        let mut s = f.debug_struct("Instance");
        s.field("hydrated states", &self.hydrated_states);

        #[cfg(any(test, feature = "action_history"))]
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
        debug_assert!(!self.any_states_powered(), "Instance still has powered states after termination");
    }
}

#[cfg(test)]
mod tests
{
    use crate::instance::Action::SignaledEntry;
    use super::*;

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
            /* TODO:
                dependency change enqueues power-off then power-on (if bool flipped)
             */
        }

        fn power_off(&self, _scope: &mut Scope)
        {
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
    fn combo_traversal()
    {
        /* tests:
            - nested traversal (through an impulse)
            - multiple outlets take the correct one
            - power-on and power-off
            - multiple automatic entries
        */

        let graph = Graph
        {
            auto_entries: Box::new(
            [
                BlockId::state(0),
                BlockId::impulse(4),
            ]),

            signaled_entries: Default::default(),

            impulses: Box::new(
            [
                Box::new(TestImpulse
                {
                    name: "Impulse 0",
                    outlet: PulsedOutlet
                    {
                        plugs: Box::new([Plug { target: BlockId::state(1), inlet: Inlet::Pulse }]),
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
                Box::new(TestImpulse
                {
                    name: "Impulse 4",
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
                        plugs: Box::new([Plug { target: BlockId::impulse(1), inlet: Inlet::Pulse }]),
                    },
                    false_outlet: LatchingOutlet
                    {
                        plugs: Box::new([Plug { target: BlockId::impulse(0), inlet: Inlet::Pulse }]),
                    },
                    true_outlet: LatchingOutlet
                    {
                        plugs: Box::new([Plug { target: BlockId::impulse(3), inlet: Inlet::Pulse }]),
                    },
                    .. Default::default()
                }),

                Box::new(TestState
                {
                    name: "State 1 (true)",
                    value: true,
                    true_outlet: LatchingOutlet
                    {
                        plugs: Box::new([Plug { target: BlockId::impulse(2), inlet: Inlet::Pulse }]),
                    },
                    .. Default::default()
                }),
            ]),
        };

        let mut instance = Instance::new(graph);
        
        assert_eq!(instance.state_has_power(0), false);
        assert_eq!(instance.state_has_power(1), false);
        
        instance.power_on();

        assert_eq!(instance.state_has_power(0), true);
        assert_eq!(instance.state_has_power(1), true);

        assert_eq!(instance.get_action_history(), &[
            Action::AutoEntry,
            Action::Visit(Plug::new(BlockId::state(0), Inlet::Pulse)),
            Action::PowerOn(BlockId::state(0)),
            Action::Visit(Plug::new(BlockId::impulse(0), Inlet::Pulse)),
            Action::Pulse(BlockId::impulse(0)),
            Action::Visit(Plug::new(BlockId::state(1), Inlet::Pulse)),
            Action::PowerOn(BlockId::state(1)),
            Action::Visit(Plug::new(BlockId::impulse(2), Inlet::Pulse)),
            Action::Pulse(BlockId::impulse(2)),
            Action::Visit(Plug::new(BlockId::impulse(1), Inlet::Pulse)),
            Action::Pulse(BlockId::impulse(1)),
            Action::Visit(Plug::new(BlockId::impulse(4), Inlet::Pulse)),
            Action::Pulse(BlockId::impulse(4)),
        ]);

        instance.clear_action_history();
        instance.pulse(Plug { target: BlockId::state(0), inlet: Inlet::PowerOff });

        assert_eq!(instance.state_has_power(0), false);
        assert_eq!(instance.state_has_power(1), false);

        assert_eq!(instance.get_action_history(), &[
            Action::Visit(Plug::new(BlockId::state(0), Inlet::PowerOff)),
            Action::Visit(Plug::new(BlockId::impulse(0), Inlet::PowerOff)),
            Action::Visit(Plug::new(BlockId::state(1), Inlet::PowerOff)),
            Action::Visit(Plug::new(BlockId::impulse(2), Inlet::PowerOff)),
            Action::PowerOff(BlockId::state(1)),
            Action::PowerOff(BlockId::state(0)),
        ]);

        instance.clear_action_history();
        instance.pulse(Plug { target: BlockId::state(1), inlet: Inlet::Pulse });
        assert!(instance.any_states_powered());
        instance.power_off();
        assert!(!instance.any_states_powered());

        assert_eq!(instance.get_action_history(), &[
            Action::Visit(Plug::new(BlockId::state(1), Inlet::Pulse)),
            Action::PowerOn(BlockId::state(1)),
            Action::Visit(Plug::new(BlockId::impulse(2), Inlet::Pulse)),
            Action::Pulse(BlockId::impulse(2)),
            Action::Stop,
            Action::Visit(Plug::new(BlockId::state(1), Inlet::PowerOff)),
            Action::Visit(Plug::new(BlockId::impulse(2), Inlet::PowerOff)),
            Action::PowerOff(BlockId::state(1)),
        ]);
    }

    #[test]
    fn power_off_inlet()
    {
        let graph = Graph
        {
            auto_entries: Box::new(
            [
                BlockId::state(0),
            ]),

            signaled_entries: Default::default(),

            impulses: Box::new(
            [
                Box::new(TestImpulse
                {
                    name: "Impulse 0",
                    outlet: PulsedOutlet
                    {
                        plugs: Box::new([Plug { target: BlockId::state(0), inlet: Inlet::PowerOff }]),
                    },
                }),
            ]),

            states: Box::new(
            [
                Box::new(TestState
                {
                    name: "State 0 (false)",
                    value: false,
                    false_outlet: LatchingOutlet
                    {
                        plugs: Box::new([Plug { target: BlockId::impulse(0), inlet: Inlet::Pulse }]),
                    },
                    .. Default::default()
                }),
            ]),
        };

        let mut instance = Instance::new(graph);
        instance.power_on();

        assert_eq!(instance.state_has_power(0), false);

        assert_eq!(instance.get_action_history(), &[
            Action::AutoEntry,
            Action::Visit(Plug::new(BlockId::state(0), Inlet::Pulse)),
            Action::PowerOn(BlockId::state(0)),
            Action::Visit(Plug::new(BlockId::impulse(0), Inlet::Pulse)),
            Action::Pulse(BlockId::impulse(0)),
            // powering off state will go through impulse and back to itself
            Action::Visit(Plug::new(BlockId::state(0), Inlet::PowerOff)),
            Action::Visit(Plug::new(BlockId::impulse(0), Inlet::PowerOff)),
            Action::Visit(Plug::new(BlockId::state(0), Inlet::PowerOff)),
            Action::PowerOff(BlockId::state(0)),
        ]);
    }

    #[test]
    fn non_latching_outlet()
    {
        let graph = Graph
        {
            auto_entries: Box::new(
            [
                BlockId::state(0),
            ]),

            signaled_entries: Default::default(),

            impulses: Box::new([]),

            states: Box::new(
            [
                Box::new(TestState
                {
                    name: "State 0 (false)",
                    value: false,
                    on_false_outlet: PulsedOutlet
                    {
                        plugs: Box::new([Plug { target: BlockId::state(1), inlet: Inlet::Pulse }]),
                    },
                    .. Default::default()
                }),
                Box::new(TestState
                {
                    name: "State 1 (false)",
                    value: false,
                    .. Default::default()
                }),
            ]),
        };

        let mut instance = Instance::new(graph);
        instance.power_on();

        assert_eq!(instance.state_has_power(0), true);
        assert_eq!(instance.state_has_power(1), true);

        instance.pulse(Plug { target: BlockId::state(0), inlet: Inlet::PowerOff });

        assert_eq!(instance.state_has_power(0), false);
        assert_eq!(instance.state_has_power(1), true);

        assert_eq!(instance.get_action_history(), &[
            Action::AutoEntry,
            Action::Visit(Plug::new(BlockId::state(0), Inlet::Pulse)),
            Action::PowerOn(BlockId::state(0)),
            Action::Visit(Plug::new(BlockId::state(1), Inlet::Pulse)),
            Action::PowerOn(BlockId::state(1)),
            Action::Visit(Plug::new(BlockId::state(0), Inlet::PowerOff)),
            Action::PowerOff(BlockId::state(0)),
        ]);
    }

    #[test]
    fn signaled_entry()
    {
        let graph = Graph
        {
            auto_entries: Box::new([]),
            signaled_entries: Box::new(
                [
                    (Signal::test('a'), Box::new([BlockId::impulse(0), BlockId::state(0)])),
                    (Signal::test('b'), Box::new([BlockId::impulse(1)])),
            ]),

            impulses: Box::new(
                [
                    Box::new(TestImpulse
                    {
                        name: "Impulse 0",
                        outlet: Default::default(),
                    }),
                    Box::new(TestImpulse
                    {
                        name: "Impulse 1",
                        outlet: Default::default(),
                    }),
                ]),

            states: Box::new(
            [
                Box::new(TestState
                {
                    name: "State 0 (false)",
                    value: false,
                    .. Default::default()
                }),
            ]),
        };

        let mut instance = Instance::new(graph);
        instance.power_on();

        assert_eq!(instance.state_has_power(0), false);
        assert_eq!(instance.get_action_history(), &[
            Action::AutoEntry,
        ]);
        instance.clear_action_history();

        instance.signal(0);

        assert_eq!(instance.state_has_power(0), true);
        assert_eq!(instance.get_action_history(), &[
            Action::SignaledEntry(Signal::test('a')),
            Action::Visit(Plug::new(BlockId::impulse(0), Inlet::Pulse)),
            Action::Pulse(BlockId::impulse(0)),
            Action::Visit(Plug::new(BlockId::state(0), Inlet::Pulse)),
            Action::PowerOn(BlockId::state(0)),
        ]);
    }
}
