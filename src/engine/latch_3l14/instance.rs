use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, Ordering};
use crossbeam::queue::SegQueue;
use smallvec::SmallVec;
use asset_3l14::Signal;
use nab_3l14::debug_panic;
use super::*;

const MAX_PULSE_DEPTH: u32 = 100; // smaller number?

/* TODO: at build time:
- allow multiple entrypoints during design time but merge into one
- graphs that have no entrypoint
- latches with only power-off inlets
- guarantee block index order? (lower numbers guaranteed to be closer to root?)
- (currently) disable multiple links to a single inlet
    - possible design alt for 'all': blocks keep a count of number of links per inlet, runtime info tracks powered links
 */

pub(crate) enum Event
{
    // rename entry/exit to match metaphor?
    AutoEnter,
    SignalEntry(u32),
    Exit,
    VarChanged(VarChange),
}

#[derive(Debug, PartialEq)]
#[allow(dead_code)]
pub(crate) enum Action
{
    AutoEntry,
    SignaledEntry(Signal),
    Exit,
    Visit(Plug),
    Pulse(BlockId),
    PowerOn(BlockId),
    PowerOff(BlockId),
    VarChanged(VarId, VarValue)
}

#[derive(Debug)]
struct HydratedLatch
{
    is_powered: bool, // flags?
    powered_outlets: SmallVec<[BlockId; 4]>, // maybe powered
}

pub struct Instance
{
    graph: Graph,
    pub(super) scope: LocalScope,

    // todo: perhaps organize these to be on their own cache line
    event_queue: SegQueue<Event>,
    pub(super) is_processing_events: AtomicBool, // leaky abstraction

    hydrated_latches: HashMap<u32, HydratedLatch>,

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
            scope: LocalScope::default(),
            hydrated_latches: HashMap::default(),
            event_queue: SegQueue::new(),
            is_processing_events: AtomicBool::new(false),

            #[cfg(any(test, feature = "action_history"))]
            action_history: Vec::new(),
        }
    }

    #[inline] #[must_use]
    pub fn graph(&self) -> &Graph { &self.graph }

    #[inline]
    pub(crate) fn enqueue_event(&self, event: Event)
    {
        self.event_queue.push(event);
    }

    pub(crate) fn process_events(&mut self, shared_scope: &SharedScope)
    {
        loop
        {
            while let Some(event) = self.event_queue.pop()
            {
                match event
                {
                    Event::AutoEnter => self.power_on(shared_scope),
                    Event::SignalEntry(slot) => self.signal(slot as usize, shared_scope),
                    Event::Exit => self.power_off(shared_scope),
                    Event::VarChanged(change) => self.on_var_changed(change, shared_scope),
                }
            }

            self.is_processing_events.store(false, Ordering::Release);

            if self.event_queue.is_empty()
            {
                break;
            }
            // todo: verify this is sound
            if !self.is_processing_events.swap(true, Ordering::Acquire)
            {
                break;
            }
        }
    }

    // power-on all the auto-entry blocks
    fn power_on(&mut self, shared_scope: &SharedScope)
    {
        self.push_action(Action::AutoEntry);
        let auto_blocks: SmallVec<[BlockId; 8]> = SmallVec::from_slice(&self.graph.auto_entries);
        for block in auto_blocks
        {
            self.pulse(Plug { target: block, inlet: Inlet::Pulse }, shared_scope);
        }
    }

    // power-on the signaled entry blocks
    fn signal(&mut self, signal_slot: usize, shared_scope: &SharedScope)
    {
        let mut outlets = SmallVec::<[_; 2]>::new();
        self.push_action(Action::SignaledEntry(self.graph.signaled_entries[signal_slot].0));
        outlets.extend_from_slice(&self.graph.signaled_entries[signal_slot].1);
        for block in outlets
        {
            self.pulse(Plug { target: block, inlet: Inlet::Pulse }, shared_scope);
        }
    }

    // power off all blocks immediately
    fn power_off(&mut self, shared_scope: &SharedScope)
    {
        puffin::profile_function!();

        // free memory?
        self.push_action(Action::Exit);

        // iter all powered latches and power-off
        let mut powered_latches = Vec::new();
        for (id, latch) in &self.hydrated_latches
        {
            if latch.is_powered
            {
                powered_latches.push(*id);
            }
        }
        powered_latches.sort_unstable(); // TODO: guarantee latch ordering?

        // latches that are already powered-off should no-op
        for ps in powered_latches
        {
            self.pulse(Plug { target: BlockId::latch(ps), inlet: Inlet::PowerOff }, shared_scope);
        }
    }

    // react to a variable change
    fn on_var_changed(&mut self, change: VarChange, shared_scope: &SharedScope)
    {
        puffin::profile_function!();

        let (_, block) = change.target;
        if block.is_latch()
        {
            self.push_action(Action::VarChanged(change.var, change.new_value.clone()));
            let Some(hydrated) = self.hydrated_latches.get(&block.value()) else { return; };
            if !hydrated.is_powered { return; };

            let mut pulsed_plugs = PlugList::default();
            let mut latching_plugs = PlugList::default();

            let mut local_changes = ScopeChanges::new();
            let mut shared_changes = ScopeChanges::new();

            self.graph.latches[block.value() as usize].on_var_changed(change, Scope
            {
                local_scope: &mut self.scope,
                local_changes: &mut local_changes,
                shared_scope,
                shared_changes: &mut shared_changes
            },
            LatchOutletVisitor
            {
                pulses: &mut pulsed_plugs,
                latching: &mut latching_plugs,
            });

            // TODO: further propagate any more changes

            // todo: pulse all plugs
        }
        else
        {
            debug_panic!("Dependent block must be a latch");
        }
    }

    // TODO: pass in array of plugs
    // pulse nodes down the graph starting at plug
    fn pulse(&mut self, plug: Plug, shared_scope: &SharedScope)
    {
        puffin::profile_function!();

        // TODO: track cache misses on small vec

        struct VisitPlug
        {
            plug: Plug,
            parent_latch: Option<BlockId>,
            depth: u32,
        }

        let mut stack: SmallVec<[VisitPlug; 8]> = smallvec![VisitPlug { plug, parent_latch: None, depth: 0 }];

        let mut pulsed_plugs = PlugList::default();
        let mut latching_plugs = PlugList::default();

        let mut powering_off_latches: SmallVec<[u32; 16]> = SmallVec::new();

        // TODO: change propagation ordering needs to be well defined

        let mut local_changes = ScopeChanges::new();
        let mut shared_changes = ScopeChanges::new();
        macro_rules! scope { () => { Scope
        {
            local_scope: &mut self.scope,
            local_changes: &mut local_changes,
            shared_scope,
            shared_changes: &mut shared_changes
        } } }

        while let Some(VisitPlug { plug: test_plug, parent_latch, depth }) = stack.pop()
        {
            debug_assert!(depth < MAX_PULSE_DEPTH, "Maximum pulse depth exceeded");

            self.push_action(Action::Visit(test_plug));

            if test_plug.target.is_impulse()
            {
                let impulse = self.graph.impulses[test_plug.target.value() as usize].as_ref();
                if let Inlet::Pulse = test_plug.inlet
                {
                    impulse.pulse(
                        scope!(),
                        ImpulseOutletVisitor
                        {
                            pulses: &mut pulsed_plugs,
                        }
                    );

                    stack.reserve(pulsed_plugs.len());
                    for pulse in pulsed_plugs.drain(..)
                    {
                        stack.push(VisitPlug
                        {
                            plug: pulse,
                            parent_latch, // forward through
                            depth: depth + 1
                        });
                    }

                    // stupid rust mutability rules
                    // ordering here to match latch ordering
                    #[cfg(any(test, feature = "action_history"))]
                    self.action_history.push(Action::Pulse(test_plug.target));
                }
            }
            else
            {
                let hydrated = self.hydrated_latches.entry(test_plug.target.value())
                    .or_insert_with(|| HydratedLatch { is_powered: false, powered_outlets: SmallVec::new(), });
                match test_plug.inlet
                {
                    Inlet::Pulse =>
                    {
                        if hydrated.is_powered { continue; }
                        hydrated.is_powered = true;

                        // link the parent directly to this state for when powering-off
                        // todo: can downstream latches be stored statically?
                        if let Some(parent_latch) = parent_latch
                        {
                            let hydrated_parent = self.hydrated_latches.get_mut(&parent_latch.value())
                                .expect("Parent latch set but no hydrated state exists for it??");
                            hydrated_parent.powered_outlets.push(test_plug.target);
                        }

                        let latch = self.graph.latches[test_plug.target.value() as usize].as_ref();
                        latch.power_on(
                            scope!(),
                            LatchOutletVisitor
                            {
                                pulses: &mut pulsed_plugs,
                                latching: &mut latching_plugs,
                            }
                        );

                        stack.reserve(pulsed_plugs.len() + latching_plugs.len());

                        for pulse in pulsed_plugs.drain(..)
                        {
                            // non-latching links
                            stack.push(VisitPlug
                            {
                                plug: pulse,
                                parent_latch: None, // non-latching links ignore power-off propagation
                                depth: depth + 1
                            });
                        }
                        for latch in latching_plugs.drain(..)
                        {
                            stack.push(VisitPlug
                            {
                                plug: latch,
                                parent_latch: Some(test_plug.target),
                                depth: depth + 1
                            });
                        }

                        self.push_action(Action::PowerOn(test_plug.target));
                    }
                    Inlet::PowerOff =>
                    {
                        if !hydrated.is_powered { continue; }

                        // possible optimization: track if there's any downstream latches
                        // don't traverse if no latches

                        hydrated.is_powered = false;

                        // power-off is deferred to below
                        powering_off_latches.reserve(hydrated.powered_outlets.len() + 1);

                        powering_off_latches.push(test_plug.target.value());
                        for powered in hydrated.powered_outlets.drain(..)
                        {
                            // powering_off_latches.push(powered.value());
                            stack.push(VisitPlug
                            {
                                plug: Plug::new(powered, Inlet::PowerOff),
                                parent_latch: Some(test_plug.target),
                                depth: depth + 1
                            });
                        }
                    }
                }
            };

            for change in local_changes.drain(..)
            {
                self.event_queue.push(Event::VarChanged(change))
            }

            for change in shared_changes.drain(..)
            {
                // TODO
            }
        }

        // post-order traversal to shut-off
        for powered in powering_off_latches.iter().rev()
        {
            let latch = self.graph.latches[*powered as usize].as_ref();
            latch.power_off(scope!());
            self.push_action(Action::PowerOff(BlockId::latch(*powered)));
        }
    }

    #[inline] #[allow(unused_variables)]
    fn push_action(&mut self, action: Action)
    {
        #[cfg(any(test, feature = "action_history"))]
        self.action_history.push(action);
    }
    #[inline]
    fn clear_action_history(&mut self)
    {
        #[cfg(any(test, feature = "action_history"))]
        self.action_history.clear();
    }
    #[inline] #[must_use]
    fn get_action_history(&self) -> &[Action]
    {
        #[cfg(any(test, feature = "action_history"))]
        { &self.action_history }
        #[cfg(not(any(test, feature = "action_history")))]
        { &[] }
    }

    #[inline] #[must_use]
    pub fn latch_has_power(&self, latch: u32) -> bool
    {
        debug_assert!((latch as usize) < self.graph.latches.len());
        self.hydrated_latches.get(&latch).map_or(false, |latch| latch.is_powered)
    }

    #[inline] #[must_use]
    pub fn any_latches_powered(&self) -> bool
    {
        self.hydrated_latches.iter().any(|(_, latch)| latch.is_powered)
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

        let mut pulsed_plugs = PlugList::default();
        let mut latching_plugs = PlugList::default();

        // TODO: broken

        while let Some(block) = stack.pop()
        {
            if block.is_impulse()
            {
                let impulse = self.graph.impulses[block.value() as usize].as_ref();
                writer.write_fmt(format_args!("  \"{:?}\" [label=\"{}\\n\\N\" shape=\"box\"]\n",
                                              block,
                                              "impulse"))?; // TODO: type name
                // TODO: visit outlets
            }
            else
            {
                let hydrated = self.hydrated_latches.get(&block.value());
                let latch = self.graph.latches[block.value() as usize].as_ref();

                let is_powered = hydrated.map(|h| h.is_powered).unwrap_or(false);
                writer.write_fmt(format_args!("  \"{:?}\" [label=\"{}\\n\\N\" shape=\"{}\"]\n",
                                              block,
                                              "latch", // TODO: type name
                                              if is_powered { "doubleoctagon" } else { "octagon" }))?;

                // TODO: visit outlets
            }

            for pulse in &pulsed_plugs
            {
                stack.push(pulse.target);
                writer.write_fmt(format_args!("  \"{:?}\" -> \"{:?}\" [minlen=3 taillabel=\"{}\" headlabel=\"{:?}\"]\n",
                                              block,
                                              pulse.target,
                                              "Pulsed (TODO NAME)",
                                              pulse.inlet))?;
            }
            for latch in &latching_plugs
            {
                stack.push(latch.target);
                writer.write_fmt(format_args!("  \"{:?}\" -> \"{:?}\" [color=\"black:invis:black\" minlen=3 taillabel=\"{}\" headlabel=\"{:?}\"]\n",
                                              block,
                                              latch.target,
                                              "Latching (TODO NAME)",
                                              latch.inlet))?;
            }

            pulsed_plugs.clear();
            latching_plugs.clear();
        }

        writer.write_fmt(format_args!("}}"))
    }
}
impl Debug for Instance
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
    {
        let mut s = f.debug_struct("Instance");
        s.field("hydrated latchess", &self.hydrated_latches);

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
        debug_assert!(!self.any_latches_powered(), "Instance still has powered latches after termination");
    }
}


#[cfg(test)]
mod traversal_tests
{
    use super::*;

    #[derive(Default)]
    struct TestImpulse
    {
        name: &'static str,
        outlet: PulsedOutlet,
    }
    impl ImpulseBlock for TestImpulse
    {
        fn pulse(&self, scope: Scope, mut pulse_outlets: ImpulseOutletVisitor)
        {
            pulse_outlets.visit_pulsed(&self.outlet);
        }
    }

    #[derive(Default)]
    struct TestLatch
    {
        name: &'static str,
        value: bool,

        on_true_outlet: PulsedOutlet,
        true_outlet: LatchingOutlet,

        on_false_outlet: PulsedOutlet,
        false_outlet: LatchingOutlet,

        any_outlet: LatchingOutlet,
    }
    impl LatchBlock for TestLatch
    {
        fn power_on(&self, _scope: Scope, mut pulse_outlets: LatchOutletVisitor)
        {
            if self.value
            {
                pulse_outlets.visit_pulsed(&self.on_true_outlet);
                pulse_outlets.visit_latching(&self.true_outlet, Inlet::Pulse);
                pulse_outlets.visit_latching(&self.any_outlet, Inlet::Pulse);
            }
            else
            {
                pulse_outlets.visit_pulsed(&self.on_false_outlet);
                pulse_outlets.visit_latching(&self.false_outlet, Inlet::Pulse);
                pulse_outlets.visit_latching(&self.any_outlet, Inlet::Pulse);
            }
        }

        fn power_off(&self, _scope: Scope)
        {
        }

        fn on_var_changed(&self, change: VarChange, _scope: Scope, pulse_outlets: LatchOutletVisitor) -> OnVarChangedResult
        {
            OnVarChangedResult::NoChange
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

        // TODO: add more depth
        // L -> I -> I -> I -> L
        // L -> i -> I -> I -> I
        // L -> L -> L -> L -> I

        let graph = Graph
        {
            auto_entries: Box::new(
            [
                BlockId::latch(0),
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
                        plugs: Box::new([Plug { target: BlockId::latch(1), inlet: Inlet::Pulse }]),
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

            latches: Box::new(
            [
                Box::new(TestLatch
                {
                    name: "Latch 0 (false)",
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

                Box::new(TestLatch
                {
                    name: "Latch 1 (true)",
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

        let shared_scope = SharedScope::default();

        assert_eq!(instance.latch_has_power(0), false);
        assert_eq!(instance.latch_has_power(1), false);

        instance.power_on(&shared_scope);

        assert_eq!(instance.latch_has_power(0), true);
        assert_eq!(instance.latch_has_power(1), true);

        assert_eq!(instance.get_action_history(), &[
            Action::AutoEntry,
            Action::Visit(Plug::new(BlockId::latch(0), Inlet::Pulse)),
            Action::PowerOn(BlockId::latch(0)),
            Action::Visit(Plug::new(BlockId::impulse(0), Inlet::Pulse)),
            Action::Pulse(BlockId::impulse(0)),
            Action::Visit(Plug::new(BlockId::latch(1), Inlet::Pulse)),
            Action::PowerOn(BlockId::latch(1)),
            Action::Visit(Plug::new(BlockId::impulse(2), Inlet::Pulse)),
            Action::Pulse(BlockId::impulse(2)),
            Action::Visit(Plug::new(BlockId::impulse(1), Inlet::Pulse)),
            Action::Pulse(BlockId::impulse(1)),
            Action::Visit(Plug::new(BlockId::impulse(4), Inlet::Pulse)),
            Action::Pulse(BlockId::impulse(4)),
        ]);

        instance.clear_action_history();

        instance.pulse(Plug { target: BlockId::latch(0), inlet: Inlet::PowerOff }, &shared_scope);

        assert_eq!(instance.latch_has_power(0), false);
        assert_eq!(instance.latch_has_power(1), false);

        assert_eq!(instance.get_action_history(), &[
            Action::Visit(Plug::new(BlockId::latch(0), Inlet::PowerOff)),
            Action::Visit(Plug::new(BlockId::latch(1), Inlet::PowerOff)),
            Action::PowerOff(BlockId::latch(1)),
            Action::PowerOff(BlockId::latch(0)),
        ]);

        instance.clear_action_history();
        instance.pulse(Plug { target: BlockId::latch(1), inlet: Inlet::Pulse }, &shared_scope);
        assert!(instance.any_latches_powered());
        instance.power_off(&shared_scope);
        assert!(!instance.any_latches_powered());

        assert_eq!(instance.get_action_history(), &[
            Action::Visit(Plug::new(BlockId::latch(1), Inlet::Pulse)),
            Action::PowerOn(BlockId::latch(1)),
            Action::Visit(Plug::new(BlockId::impulse(2), Inlet::Pulse)),
            Action::Pulse(BlockId::impulse(2)),
            Action::Exit,
            Action::Visit(Plug::new(BlockId::latch(1), Inlet::PowerOff)),
            Action::PowerOff(BlockId::latch(1)),
        ]);
    }

    #[test]
    fn power_off_inlet()
    {
        let graph = Graph
        {
            auto_entries: Box::new(
            [
                BlockId::latch(0),
            ]),

            signaled_entries: Default::default(),

            impulses: Box::new(
            [
                Box::new(TestImpulse
                {
                    name: "Impulse 0",
                    outlet: PulsedOutlet
                    {
                        plugs: Box::new([Plug { target: BlockId::latch(0), inlet: Inlet::PowerOff }]),
                    },
                }),
            ]),

            latches: Box::new(
            [
                Box::new(TestLatch
                {
                    name: "Latch 0 (false)",
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

        let shared_scope = SharedScope::default();

        instance.power_on(&shared_scope);

        assert_eq!(instance.latch_has_power(0), false);

        assert_eq!(instance.get_action_history(), &[
            Action::AutoEntry,
            Action::Visit(Plug::new(BlockId::latch(0), Inlet::Pulse)),
            Action::PowerOn(BlockId::latch(0)),
            Action::Visit(Plug::new(BlockId::impulse(0), Inlet::Pulse)),
            Action::Pulse(BlockId::impulse(0)),
            // powering off latch will go through impulse and back to itself
            Action::Visit(Plug::new(BlockId::latch(0), Inlet::PowerOff)),
            Action::PowerOff(BlockId::latch(0)),
        ]);
    }

    #[test]
    fn non_latching_outlet()
    {
        let graph = Graph
        {
            auto_entries: Box::new(
            [
                BlockId::latch(0),
            ]),

            signaled_entries: Default::default(),

            impulses: Box::new([]),

            latches: Box::new(
            [
                Box::new(TestLatch
                {
                    name: "Latch 0 (false)",
                    value: false,
                    on_false_outlet: PulsedOutlet
                    {
                        plugs: Box::new([Plug { target: BlockId::latch(1), inlet: Inlet::Pulse }]),
                    },
                    .. Default::default()
                }),
                Box::new(TestLatch
                {
                    name: "Latch 1 (false)",
                    value: false,
                    .. Default::default()
                }),
            ]),
        };

        let mut instance = Instance::new(graph);

        let shared_scope = SharedScope::default();

        instance.power_on(&shared_scope);

        assert_eq!(instance.latch_has_power(0), true);
        assert_eq!(instance.latch_has_power(1), true);

        instance.pulse(Plug { target: BlockId::latch(0), inlet: Inlet::PowerOff }, &shared_scope);

        assert_eq!(instance.latch_has_power(0), false);
        assert_eq!(instance.latch_has_power(1), true);

        instance.power_off(&shared_scope);

        assert_eq!(instance.get_action_history(), &[
            Action::AutoEntry,
            Action::Visit(Plug::new(BlockId::latch(0), Inlet::Pulse)),
            Action::PowerOn(BlockId::latch(0)),
            Action::Visit(Plug::new(BlockId::latch(1), Inlet::Pulse)),
            Action::PowerOn(BlockId::latch(1)),
            Action::Visit(Plug::new(BlockId::latch(0), Inlet::PowerOff)),
            Action::PowerOff(BlockId::latch(0)),
            Action::Exit,
            Action::Visit(Plug::new(BlockId::latch(1), Inlet::PowerOff)),
            Action::PowerOff(BlockId::latch(1)),
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
                    (Signal::test('a'), Box::new([BlockId::impulse(0), BlockId::latch(0)])),
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

            latches: Box::new(
            [
                Box::new(TestLatch
                {
                    name: "Latch 0 (false)",
                    value: false,
                    .. Default::default()
                }),
            ]),
        };

        let mut instance = Instance::new(graph);

        let shared_scope = SharedScope::default();

        instance.power_on(&shared_scope);

        assert_eq!(instance.latch_has_power(0), false);
        assert_eq!(instance.get_action_history(), &[
            Action::AutoEntry,
        ]);
        instance.clear_action_history();

        instance.signal(0, &shared_scope);
        assert_eq!(instance.latch_has_power(0), true);

        instance.power_off(&shared_scope);

        assert_eq!(instance.get_action_history(), &[
            Action::SignaledEntry(Signal::test('a')),
            Action::Visit(Plug::new(BlockId::impulse(0), Inlet::Pulse)),
            Action::Pulse(BlockId::impulse(0)),
            Action::Visit(Plug::new(BlockId::latch(0), Inlet::Pulse)),
            Action::PowerOn(BlockId::latch(0)),
            Action::Exit,
            Action::Visit(Plug::new(BlockId::latch(0), Inlet::PowerOff)),
            Action::PowerOff(BlockId::latch(0)),
        ]);
    }
}

#[cfg(test)]
mod var_tests
{
    use super::*;

    struct TestImpulse;
    impl ImpulseBlock for TestImpulse
    {
        fn pulse(&self, _scope: Scope, _outlet_visitor: ImpulseOutletVisitor) { println!("pulsed TestImpulse"); }
    }

    struct WriteLatch;
    impl LatchBlock for WriteLatch
    {
        fn power_on(&self, _scope: Scope, _outlet_visitor: LatchOutletVisitor) { println!("powered on WriteLatch"); }
        fn power_off(&self, _scope: Scope) { }
        fn on_var_changed(&self, _change: VarChange, _scope: Scope, _outlet_visitor: LatchOutletVisitor) -> OnVarChangedResult
        {
            OnVarChangedResult::NoChange
        }
    }

    struct ReadLatch
    {
        pub on_read: PulsedOutlet,
    }
    impl LatchBlock for ReadLatch
    {
        fn power_on(&self, _scope: Scope, _outlet_visitor: LatchOutletVisitor) { println!("powered on ReadLatch"); }
        fn power_off(&self, _scope: Scope) { }
        fn on_var_changed(&self, change: VarChange, _scope: Scope, mut outlet_visitor: LatchOutletVisitor) -> OnVarChangedResult
        {
            println!("read {:?} {:?}", change.var, change.new_value);
            outlet_visitor.visit_pulsed(&self.on_read);
            OnVarChangedResult::NoChange
        }
    }

    #[test]
    fn basic()
    {
        let graph = Graph
        {
            auto_entries: Box::new([]),
            signaled_entries: Box::new([]),
            impulses: Box::new([]),
            latches: Box::new([]),
        };
    }
}