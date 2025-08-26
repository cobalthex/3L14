use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, Ordering};
use crossbeam::channel::Sender;
use crossbeam::queue::SegQueue;
use smallvec::SmallVec;
use nab_3l14::{debug_panic, Signal};
use crate::runtime::Action as RuntimeAction;
use super::*;

const MAX_PULSE_DEPTH: u32 = 100; // smaller number?

/* TODO: at build time:
- allow multiple entrypoints during design time but merge into one
- circuits that have no entrypoint
- latches with only power-off inlets
- guarantee block index order? (lower numbers guaranteed to be closer to root?)
- (currently) disable multiple links to a single inlet
    - possible design alt for 'all': blocks keep a count of number of links per inlet, runtime info tracks powered links
 */

pub(crate) enum Action
{
    PowerOn,
    Signal(u32),
    PowerOff,
    ReEnter(BlockId),
    VarChanged(VarChange),
}

#[derive(Debug, PartialEq)]
#[allow(dead_code)]
enum History
{
    InstancePowerOn, // todo: better name?
    Signal(Signal),
    InstancePowerOff, // todo: better name?
    Visit(Plug),
    Pulse(BlockId),
    PowerOn(BlockId),
    PowerOff(BlockId),
    ReEnter(BlockId),
    VarChanged(VarId, VarValue)
}

#[derive(Debug)]
struct HydratedLatch
{
    is_powered: bool, // flags?
    powered_outlets: SmallVec<[BlockId; 4]>, // maybe powered
}

#[derive(Clone)]
pub(super) struct RunContext<'r>
{
    pub run_id: InstRunId,
    pub shared_scope: &'r SharedScope,
    pub action_sender: Sender<RuntimeAction>,
}

pub struct Instance
{
    circuit: Circuit,
    pub(super) scope: LocalScope,

    // todo: perhaps organize these to be on their own cache line
    action_queue: SegQueue<Action>,
    pub(super) is_processing_actions: AtomicBool, // leaky abstraction

    hydrated_latches: HashMap<u32, HydratedLatch>, // array?

    #[cfg(any(test, feature = "action_history"))]
    action_history: Vec<History>, // ring buffer?

}
impl Instance
{
    #[inline] #[must_use]
    pub fn new(circuit: Circuit) -> Self
    {
        Self
        {
            scope: LocalScope::new(circuit.num_local_vars),
            circuit,
            hydrated_latches: HashMap::default(),
            action_queue: SegQueue::new(),
            is_processing_actions: AtomicBool::new(false),

            #[cfg(any(test, feature = "action_history"))]
            action_history: Vec::new(),
        }
    }

    #[inline] #[must_use]
    pub fn circuit(&self) -> &Circuit { &self.circuit }

    #[inline]
    pub(crate) fn enqueue_action(&self, action: Action)
    {
        self.action_queue.push(action);
    }

    pub(crate) fn process_actions(&mut self, context: RunContext)
    {
        // TODO: detect 'infinite' loops?
        // TODO: set time limit
        loop
        {
            while let Some(action) = self.action_queue.pop()
            {
                match action
                {
                    Action::PowerOn => self.power_on(context.clone()),
                    Action::Signal(slot) => self.signal(slot as usize, context.clone()),
                    Action::PowerOff => self.power_off(context.clone()),
                    Action::ReEnter(block_id) => self.re_enter(block_id, context.clone()),
                    Action::VarChanged(change) => self.on_var_changed(change, context.clone()),
                }
            }

            self.is_processing_actions.store(false, Ordering::Release);

            if self.action_queue.is_empty()
            {
                break;
            }
            // todo: verify this is sound
            if !self.is_processing_actions.swap(true, Ordering::Acquire)
            {
                break;
            }
        }
    }

    // power-on all the auto-entry blocks
    fn power_on(&mut self, context: RunContext)
    {
        self.push_action(History::InstancePowerOn);
        let auto_blocks: SmallVec<[BlockId; 8]> = SmallVec::from_slice(&self.circuit.auto_entries);
        self.pulse(
            auto_blocks.iter().rev().map(|b| Plug { target: *b, inlet: Inlet::Pulse }),
            context);
    }

    // power-on the signaled entry blocks
    fn signal(&mut self, signal_slot: usize, context: RunContext)
    {
        let mut outlets = SmallVec::<[_; 2]>::new();
        self.push_action(History::Signal(self.circuit.signaled_entries[signal_slot].0));
        outlets.extend_from_slice(&self.circuit.signaled_entries[signal_slot].1);
        self.pulse(
            outlets.iter().rev().map(|b| Plug { target: *b, inlet: Inlet::Pulse }),
            context);
    }

    // power off all blocks immediately
    fn power_off(&mut self, context: RunContext)
    {
        puffin::profile_function!();

        // free memory?
        self.push_action(History::InstancePowerOff);

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
        self.pulse(
            powered_latches.iter().rev().map(|l| Plug { target: BlockId::latch(*l), inlet: Inlet::PowerOff }),
            context);
    }

    // re-enter a powered latch
    fn re_enter(&mut self, block_id: BlockId, context: RunContext)
    {
        puffin::profile_function!();

        assert!(block_id.is_latch());

        // TODO

        self.push_action(History::ReEnter(block_id));

    }

    // react to a variable change
    fn on_var_changed(&mut self, change: VarChange, context: RunContext)
    {
        puffin::profile_function!();

        let (change_run_id, block_id) = change.target;
        debug_assert!(change_run_id == context.run_id); // todo: remove once proven
        assert!(block_id.is_latch());

        self.push_action(History::VarChanged(change.var, change.new_value.clone()));
        let Some(hydrated) = self.hydrated_latches.get(&block_id.value()) else { return; };
        if !hydrated.is_powered { return; };

        let mut pulsed_plugs = PlugList::default();
        let mut latching_plugs = PlugList::default();

        let mut local_changes = ScopeChanges::new();
        let mut shared_changes = ScopeChanges::new();

        // todo: handle result
        let todo_result = self.circuit.latches[block_id.value() as usize].on_var_changed(change, Scope
        {
            run_id: context.run_id,
            block_id,
            local_scope: &mut self.scope,
            local_changes: &mut local_changes,
            shared_scope: context.shared_scope,
            shared_changes: &mut shared_changes
        },
        LatchActions
        {
            pulse_outlets: &mut pulsed_plugs,
            latch_outlets: &mut latching_plugs,
            action_sender: context.action_sender.clone(),
        });

        let plugs =
            pulsed_plugs.iter().rev().map(|p| Plug { target: p.target, inlet: Inlet::Pulse })
            .chain(latching_plugs.iter().rev().map(|p| Plug { target: p.target, inlet: Inlet::Pulse }));
        self.pulse(plugs, context);

        // should these evaluate immediately?
        for change in local_changes.drain(..)
        {
            self.enqueue_action(Action::VarChanged(change));
        }
        for change in shared_changes.drain(..)
        {
            todo!()
        }
    }

    // TODO: pass in array of plugs
    // pulse nodes down the circuit starting at the given plugs (which should be passed in in reverse order)
    fn pulse(&mut self, plugs_in_rev_order: impl IntoIterator<Item=Plug>, context: RunContext)
    {
        puffin::profile_function!();

        // TODO: track cache misses on small vec

        struct VisitPlug
        {
            plug: Plug,
            parent_latch: Option<BlockId>,
            depth: u32,
        }

        // todo: maybe split out the construction of this to avoid the monomorphization of `plugs`
        let mut stack: SmallVec<[VisitPlug; 8]> = SmallVec::new();
        for plug in plugs_in_rev_order
        {
            stack.push(VisitPlug { plug, parent_latch: None, depth: 0 });
        }

        let mut pulsed_plugs = PlugList::default();
        let mut latching_plugs = PlugList::default();

        let mut powering_off_latches: SmallVec<[u32; 16]> = SmallVec::new();

        // TODO: change propagation ordering needs to be well defined

        let mut local_changes = ScopeChanges::new();
        let mut shared_changes = ScopeChanges::new();
        macro_rules! scope { ($block_id:expr) => { Scope
        {
            run_id: context.run_id,
            block_id: $block_id,
            local_scope: &mut self.scope,
            local_changes: &mut local_changes,
            shared_scope: context.shared_scope,
            shared_changes: &mut shared_changes
        } } }

        while let Some(VisitPlug { plug: test_plug, parent_latch, depth }) = stack.pop()
        {
            debug_assert!(depth < MAX_PULSE_DEPTH, "Maximum pulse depth exceeded");

            self.push_action(History::Visit(test_plug));

            if test_plug.target.is_impulse()
            {
                let impulse = self.circuit.impulses[test_plug.target.value() as usize].as_ref();
                if let Inlet::Pulse = test_plug.inlet
                {
                    // TODO: use
                    let pulse_result = impulse.pulse(
                        scope!(test_plug.target),
                        ImpulseActions
                        {
                            pulse_outlets: &mut pulsed_plugs,
                            action_sender: context.action_sender.clone(),
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
                    self.action_history.push(History::Pulse(test_plug.target));
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

                        let latch = self.circuit.latches[test_plug.target.value() as usize].as_ref();
                        // TODO: handle result
                        let latch_result = latch.power_on(
                            scope!(test_plug.target),
                            LatchActions
                            {
                                pulse_outlets: &mut pulsed_plugs,
                                latch_outlets: &mut latching_plugs,
                                action_sender: context.action_sender.clone(),
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

                        self.push_action(History::PowerOn(test_plug.target));
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
                self.action_queue.push(Action::VarChanged(change))
            }

            for change in shared_changes.drain(..)
            {
                // TODO
            }
        }

        // post-order traversal to shut-off
        for powered in powering_off_latches.iter().rev()
        {
            let blk = BlockId::latch(*powered);
            let latch = self.circuit.latches[*powered as usize].as_ref();
            latch.power_off(scope!(blk));
            self.push_action(History::PowerOff(blk));
        }
    }

    #[inline] #[allow(unused_variables)]
    fn push_action(&mut self, action: History)
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
    fn get_action_history(&self) -> &[History]
    {
        #[cfg(any(test, feature = "action_history"))]
        { &self.action_history }
        #[cfg(not(any(test, feature = "action_history")))]
        { &[] }
    }

    #[inline] #[must_use]
    pub fn latch_has_power(&self, latch: u32) -> bool
    {
        debug_assert!((latch as usize) < self.circuit.latches.len());
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
        for outlet in &self.circuit.auto_entries
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
                let impulse = self.circuit.impulses[block.value() as usize].as_ref();
                writer.write_fmt(format_args!("  \"{:?}\" [label=\"{}\\n\\N\" shape=\"box\"]\n",
                    block,
                    "impulse"))?; // TODO: type name

                impulse.visit_all_outlets(ImpulseOutletVisitor
                {
                    pulses: &mut pulsed_plugs,
                });
            }
            else
            {
                let hydrated = self.hydrated_latches.get(&block.value());
                let latch = self.circuit.latches[block.value() as usize].as_ref();

                let is_powered = hydrated.map(|h| h.is_powered).unwrap_or(false);
                writer.write_fmt(format_args!("  \"{:?}\" [label=\"{}\\n\\N\" shape=\"{}\"]\n",
                    block,
                    "latch", // TODO: type name
                    if is_powered { "doubleoctagon" } else { "octagon" }))?;

                latch.visit_all_outlets(LatchOutletVisitor
                {
                    pulses: &mut pulsed_plugs,
                    latches: &mut latching_plugs,
                })
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
fn gen_run_cxt(shared_scope: &SharedScope) -> RunContext
{
    let (send, _) = crossbeam::channel::bounded(0);
    let run_cxt = RunContext
    {
        run_id: InstRunId::TEST,
        action_sender: send,
        shared_scope,
    };
    run_cxt
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
        fn pulse(&self, scope: Scope, mut actions: ImpulseActions)
        {
            actions.pulse(&self.outlet);
        }

        fn visit_all_outlets(&self, mut visitor: ImpulseOutletVisitor)
        {
            visitor.visit_pulsed(&self.outlet);
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

        powered_outlet: LatchingOutlet,
    }
    impl LatchBlock for TestLatch
    {
        fn power_on(&self, _scope: Scope, mut actions: LatchActions)
        {
            if self.value
            {
                actions.pulse(&self.on_true_outlet);
                actions.latch(&self.true_outlet);
                actions.latch(&self.powered_outlet);
            }
            else
            {
                actions.pulse(&self.on_false_outlet);
                actions.latch(&self.false_outlet);
                actions.latch(&self.powered_outlet);
            }
        }

        fn power_off(&self, _scope: Scope) { }

        fn visit_all_outlets(&self, mut visitor: LatchOutletVisitor)
        {
            visitor.visit_pulsed(&self.on_true_outlet);
            visitor.visit_pulsed(&self.on_false_outlet);
            visitor.visit_latching(&self.true_outlet);
            visitor.visit_latching(&self.false_outlet);
            visitor.visit_latching(&self.powered_outlet);
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

        let circuit = Circuit
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

            num_local_vars: 0,
        };

        let mut instance = Instance::new(circuit);
        let shared_scope = SharedScope::default();
        let run_cxt = gen_run_cxt(&shared_scope);

        assert_eq!(instance.latch_has_power(0), false);
        assert_eq!(instance.latch_has_power(1), false);

        instance.power_on(run_cxt.clone());

        assert_eq!(instance.latch_has_power(0), true);
        assert_eq!(instance.latch_has_power(1), true);

        assert_eq!(instance.get_action_history(), &[
            History::InstancePowerOn,
            History::Visit(Plug::new(BlockId::latch(0), Inlet::Pulse)),
            History::PowerOn(BlockId::latch(0)),
            History::Visit(Plug::new(BlockId::impulse(0), Inlet::Pulse)),
            History::Pulse(BlockId::impulse(0)),
            History::Visit(Plug::new(BlockId::latch(1), Inlet::Pulse)),
            History::PowerOn(BlockId::latch(1)),
            History::Visit(Plug::new(BlockId::impulse(2), Inlet::Pulse)),
            History::Pulse(BlockId::impulse(2)),
            History::Visit(Plug::new(BlockId::impulse(1), Inlet::Pulse)),
            History::Pulse(BlockId::impulse(1)),
            History::Visit(Plug::new(BlockId::impulse(4), Inlet::Pulse)),
            History::Pulse(BlockId::impulse(4)),
        ]);

        instance.clear_action_history();

        instance.pulse([Plug { target: BlockId::latch(0), inlet: Inlet::PowerOff }], run_cxt.clone());

        assert_eq!(instance.latch_has_power(0), false);
        assert_eq!(instance.latch_has_power(1), false);

        assert_eq!(instance.get_action_history(), &[
            History::Visit(Plug::new(BlockId::latch(0), Inlet::PowerOff)),
            History::Visit(Plug::new(BlockId::latch(1), Inlet::PowerOff)),
            History::PowerOff(BlockId::latch(1)),
            History::PowerOff(BlockId::latch(0)),
        ]);

        instance.clear_action_history();
        instance.pulse([Plug { target: BlockId::latch(1), inlet: Inlet::Pulse }], run_cxt.clone());
        assert!(instance.any_latches_powered());
        instance.power_off(run_cxt.clone());
        assert!(!instance.any_latches_powered());

        assert_eq!(instance.get_action_history(), &[
            History::Visit(Plug::new(BlockId::latch(1), Inlet::Pulse)),
            History::PowerOn(BlockId::latch(1)),
            History::Visit(Plug::new(BlockId::impulse(2), Inlet::Pulse)),
            History::Pulse(BlockId::impulse(2)),
            History::InstancePowerOff,
            History::Visit(Plug::new(BlockId::latch(1), Inlet::PowerOff)),
            History::PowerOff(BlockId::latch(1)),
        ]);
    }

    #[test]
    fn power_off_inlet()
    {
        let circuit = Circuit
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

            num_local_vars: 0,
        };

        let mut instance = Instance::new(circuit);
        let shared_scope = SharedScope::default();
        let run_cxt = gen_run_cxt(&shared_scope);

        instance.power_on(run_cxt.clone());

        assert_eq!(instance.latch_has_power(0), false);

        assert_eq!(instance.get_action_history(), &[
            History::InstancePowerOn,
            History::Visit(Plug::new(BlockId::latch(0), Inlet::Pulse)),
            History::PowerOn(BlockId::latch(0)),
            History::Visit(Plug::new(BlockId::impulse(0), Inlet::Pulse)),
            History::Pulse(BlockId::impulse(0)),
            // powering off latch will go through impulse and back to itself
            History::Visit(Plug::new(BlockId::latch(0), Inlet::PowerOff)),
            History::PowerOff(BlockId::latch(0)),
        ]);
    }

    #[test]
    fn non_latching_outlet()
    {
        let circuit = Circuit
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

            num_local_vars: 0,
        };

        let mut instance = Instance::new(circuit);
        let shared_scope = SharedScope::default();
        let run_cxt = gen_run_cxt(&shared_scope);

        instance.power_on(run_cxt.clone());

        assert_eq!(instance.latch_has_power(0), true);
        assert_eq!(instance.latch_has_power(1), true);

        instance.pulse([Plug { target: BlockId::latch(0), inlet: Inlet::PowerOff }], run_cxt.clone());

        assert_eq!(instance.latch_has_power(0), false);
        assert_eq!(instance.latch_has_power(1), true);

        instance.power_off(run_cxt.clone());

        assert_eq!(instance.get_action_history(), &[
            History::InstancePowerOn,
            History::Visit(Plug::new(BlockId::latch(0), Inlet::Pulse)),
            History::PowerOn(BlockId::latch(0)),
            History::Visit(Plug::new(BlockId::latch(1), Inlet::Pulse)),
            History::PowerOn(BlockId::latch(1)),
            History::Visit(Plug::new(BlockId::latch(0), Inlet::PowerOff)),
            History::PowerOff(BlockId::latch(0)),
            History::InstancePowerOff,
            History::Visit(Plug::new(BlockId::latch(1), Inlet::PowerOff)),
            History::PowerOff(BlockId::latch(1)),
        ]);
    }

    #[test]
    fn signaled_entry()
    {
        let circuit = Circuit
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

            num_local_vars: 0,
        };

        let mut instance = Instance::new(circuit);
        let shared_scope = SharedScope::default();
        let run_cxt = gen_run_cxt(&shared_scope);

        instance.power_on(run_cxt.clone());

        assert_eq!(instance.latch_has_power(0), false);
        assert_eq!(instance.get_action_history(), &[
            History::InstancePowerOn,
        ]);
        instance.clear_action_history();

        instance.signal(0, run_cxt.clone());
        assert_eq!(instance.latch_has_power(0), true);

        instance.power_off(run_cxt.clone());

        assert_eq!(instance.get_action_history(), &[
            History::Signal(Signal::test('a')),
            History::Visit(Plug::new(BlockId::impulse(0), Inlet::Pulse)),
            History::Pulse(BlockId::impulse(0)),
            History::Visit(Plug::new(BlockId::latch(0), Inlet::Pulse)),
            History::PowerOn(BlockId::latch(0)),
            History::InstancePowerOff,
            History::Visit(Plug::new(BlockId::latch(0), Inlet::PowerOff)),
            History::PowerOff(BlockId::latch(0)),
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
        fn pulse(&self, _scope: Scope, _actions: ImpulseActions) { println!("pulsed TestImpulse"); }
        fn visit_all_outlets(&self, _visitor: ImpulseOutletVisitor) { }
    }

    struct WriteLatch
    {
        pub var: VarId,
    }
    impl LatchBlock for WriteLatch
    {
        fn power_on(&self, mut scope: Scope, _actions: LatchActions)
        {
            println!("powered on WriteLatch");
            scope.set(self.var, VarValue::Bool(true));
        }
        fn power_off(&self, _scope: Scope) { }
        fn visit_all_outlets(&self, _visitor: LatchOutletVisitor) { }
    }

    struct ReadLatch
    {
        pub var: VarId,
        pub on_read: PulsedOutlet,
    }
    impl LatchBlock for ReadLatch
    {
        fn power_on(&self, mut scope: Scope, _actions: LatchActions)
        {
            scope.subscribe(self.var);
            println!("powered on ReadLatch");
        }
        fn power_off(&self, mut scope: Scope)
        {
            scope.unsubscribe(self.var);
        }
        fn on_var_changed(&self, change: VarChange, _scope: Scope, mut actions: LatchActions)
        {
            println!("read {:?} = {:?}", change.var, change.new_value);
            actions.pulse(&self.on_read);
        }
        fn visit_all_outlets(&self, mut visitor: LatchOutletVisitor)
        {
            visitor.visit_pulsed(&self.on_read);
        }
    }

    #[test]
    fn no_propagation_while_powered_off()
    {
        let circuit = Circuit
        {
            auto_entries: Box::new([
                BlockId::latch(0),
                BlockId::latch(1),
            ]),

            signaled_entries: Box::new([]),

            impulses: Box::new([
                Box::new(TestImpulse),
            ]),

            latches: Box::new([
                Box::new(WriteLatch
                {
                    var: VarId::test(0, VarScope::Local),
                }),
                Box::new(ReadLatch
                {
                    var: VarId::test(0, VarScope::Local),
                    on_read: PulsedOutlet
                    {
                        plugs: Box::new([Plug { target: BlockId::impulse(0), inlet: Inlet::Pulse }]),
                    },
                }),
            ]),

            num_local_vars: 1,
        };

        let mut instance = Instance::new(circuit);
        let shared_scope = SharedScope::default();
        let run_cxt = gen_run_cxt(&shared_scope);

        instance.power_on(run_cxt.clone());
        instance.process_actions(run_cxt.clone());

        assert_eq!(instance.get_action_history(), &[
            History::InstancePowerOn,
            History::Visit(Plug::new(BlockId::latch(0), Inlet::Pulse)),
            History::PowerOn(BlockId::latch(0)),
            History::Visit(Plug::new(BlockId::latch(1), Inlet::Pulse)),
            History::PowerOn(BlockId::latch(1)),
        ]);

        instance.clear_action_history();
        instance.power_off(run_cxt.clone());
    }

    #[test]
    fn var_propagation()
    {
        let circuit = Circuit
        {
            auto_entries: Box::new([
                BlockId::latch(1),
                BlockId::latch(0),
            ]),

            signaled_entries: Box::new([]),

            impulses: Box::new([
                Box::new(TestImpulse),
            ]),

            latches: Box::new([
                Box::new(WriteLatch
                {
                    var: VarId::test(0, VarScope::Local),
                }),
                Box::new(ReadLatch
                {
                    var: VarId::test(0, VarScope::Local),
                    on_read: PulsedOutlet
                    {
                        plugs: Box::new([Plug { target: BlockId::impulse(0), inlet: Inlet::Pulse }]),
                    },
                }),
            ]),

            num_local_vars: 1,
        };

        let mut instance = Instance::new(circuit);
        let shared_scope = SharedScope::default();
        let run_cxt = gen_run_cxt(&shared_scope);

        instance.power_on(run_cxt.clone());
        instance.process_actions(run_cxt.clone());

        assert_eq!(instance.get_action_history(), &[
            History::InstancePowerOn,
            History::Visit(Plug::new(BlockId::latch(1), Inlet::Pulse)),
            History::PowerOn(BlockId::latch(1)),
            History::Visit(Plug::new(BlockId::latch(0), Inlet::Pulse)),
            History::PowerOn(BlockId::latch(0)),
            History::VarChanged(VarId::test(0, VarScope::Local), VarValue::Bool(true)),
            History::Visit(Plug::new(BlockId::impulse(0), Inlet::Pulse)),
            History::Pulse(BlockId::impulse(0)),
        ]);

        instance.power_off(run_cxt.clone());
    }
}