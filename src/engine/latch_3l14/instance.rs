use std::collections::HashMap;
use std::fmt::{Debug, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use crossbeam::queue::SegQueue;
use smallvec::SmallVec;
use nab_3l14::{debug_panic, Signal};
use super::*;

const MAX_VISIT_DEPTH: u32 = 100; // smaller number?

/* TODO: at build time:
- allow multiple entrypoints during design time but merge into one
- circuits that have no entrypoint
- latches with only power-off inlets
- guarantee block index order? (lower numbers guaranteed to be closer to root?)
- (currently) disable multiple links to a single inlet
    - possible design alt for 'all': blocks keep a count of number of links per inlet, runtime info tracks powered links
 */

#[derive(Debug, PartialEq)]
#[allow(dead_code)]
enum History
{
    InstancePowerOn,
    Signal(Signal),
    InstancePowerOff,
    Visit(BlockId), // todo: should inlet be tracked?
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
    pub runtime: Arc<Runtime>, // TODO: this could probably be a reference
}

#[derive(Debug)]
enum VisitAction
{
    Pulse(Inlet, Option<BlockId> /* parent latch */), // TODO: downstream latches should be stored statically in circuit
    ReEnter,
    VarChanged(VarChange),
}
#[derive(Debug)]
struct Visit
{
    block: BlockId,
    action: VisitAction,
}

pub struct Instance
{
    circuit: Circuit,
    scope: LocalScope,

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

            #[cfg(any(test, feature = "action_history"))]
            action_history: Vec::new(), // todo: timestamps
        }
    }

    #[inline] #[must_use]
    pub fn circuit(&self) -> &Circuit { &self.circuit }

    // power-on all the auto-entry blocks
    pub fn power_on(&mut self, context: RunContext)
    {
        self.push_action(History::InstancePowerOn);
        let auto_blocks: SmallVec<[BlockId; 8]> = SmallVec::from_slice(&self.circuit.auto_entries);
        self.visit(
            auto_blocks.iter().rev().map(|b| Visit { block: *b, action: VisitAction::Pulse(Inlet::Pulse, None) }),
            context);
    }

    // power-on the signaled entry blocks
    pub fn signal(&mut self, signal_slot: usize, context: RunContext)
    {
        let mut outlets = SmallVec::<[_; 2]>::new();
        self.push_action(History::Signal(self.circuit.signaled_entries[signal_slot].0));
        outlets.extend_from_slice(&self.circuit.signaled_entries[signal_slot].1);
        self.visit(
            outlets.iter().rev().map(|b| Visit { block: *b, action: VisitAction::Pulse(Inlet::Pulse, None) }),
            context);
    }

    // power off all blocks immediately
    pub fn power_off(&mut self, context: RunContext)
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
        powered_latches.sort_unstable(); // TODO: guarantee latch ordering? (reverse order too?)

        // latches that are already powered-off should no-op
        self.visit(
            powered_latches.iter().rev().map(|l| Visit { block: BlockId::latch(*l), action: VisitAction::Pulse(Inlet::PowerOff, None) }),
            context);
    }

    #[inline]
    pub fn re_enter(&mut self, block: BlockId, context: RunContext)
    {
        self.visit([Visit { block, action: VisitAction::ReEnter }], context);
    }

    // Visit a list of blocks and perform na
    fn visit(&mut self, visit_in_rev_order: impl IntoIterator<Item=Visit>, context: RunContext)
    {
        puffin::profile_function!();

        #[derive(Debug)]
        struct VisitBlock
        {
            visit: Visit,
            depth: u32,
        }

        // TODO: track cache misses on small vec
        let mut stack: SmallVec<[VisitBlock; 16]> = SmallVec::new();
        for visit in visit_in_rev_order
        {
            stack.push(VisitBlock { visit, depth: 0 });
        }

        let mut pulsed_plugs = PlugList::default();
        let mut latched_plugs = PlugList::default();

        // TODO: change propagation ordering needs to be well defined

        let mut local_changes = ScopeChanges::new();
        let mut shared_changes = ScopeChanges::new();

        macro_rules! scope { ($block_id:expr) =>
        {
            Scope
            {
                run_id: context.run_id,
                block_id: $block_id,
                local_scope: &mut self.scope,
                local_changes: &mut local_changes,
                shared_scope: context.shared_scope,
                shared_changes: &mut shared_changes
            }
        } }

        // macros not ideal here
        macro_rules! process_pulses { ($curr_depth:expr, $parent_latch:expr) =>
        {
            stack.reserve(pulsed_plugs.len() + latched_plugs.len());
            for pulse in pulsed_plugs.drain(..).rev()
            {
                // non-latching links
                stack.push(VisitBlock
                {
                    visit: Visit
                    {
                        block: pulse.block,
                        action: VisitAction::Pulse(pulse.inlet, None), // non-latching links ignore power-off propagation
                    },
                    depth: $curr_depth + 1
                });
            }
            for latch in latched_plugs.drain(..).rev()
            {
                stack.push(VisitBlock
                {
                    visit: Visit
                    {
                        block: latch.block,
                        action: VisitAction::Pulse(latch.inlet, Some($parent_latch)),
                    },
                    depth: $curr_depth + 1
                });
            }
        } }
        macro_rules! process_var_changes { ($curr_depth:expr) =>
        {
            for change in local_changes.drain(..).rev()
            {
                stack.push(VisitBlock
                {
                    visit: Visit
                    {
                        block: change.target.1,
                        action: VisitAction::VarChanged(change),
                    },
                    depth: $curr_depth + 1,
                })
            }
            // TODO: shared changes
        } }

        while let Some(VisitBlock { visit: test_visit, depth }) = stack.pop()
        {
            debug_assert!(depth < MAX_VISIT_DEPTH, "Maximum visit depth exceeded");

            self.push_action(History::Visit(test_visit.block));

            match test_visit.action
            {
                VisitAction::Pulse(inlet, parent_latch) =>
                {
                    if test_visit.block.is_impulse()
                    {
                        let impulse = self.circuit.impulses[test_visit.block.value() as usize].as_ref();
                        if let Inlet::Pulse = inlet
                        {
                            // stupid rust mutability rules
                            // ordering here to match latch ordering
                            #[cfg(any(test, feature = "action_history"))]
                            self.action_history.push(History::Pulse(test_visit.block));

                            impulse.pulse(
                                scope!(test_visit.block),
                                ImpulseActions
                                {
                                    pulse_outlets: &mut pulsed_plugs,
                                    runtime: context.runtime.clone(),
                                }
                            );

                            stack.reserve(pulsed_plugs.len());
                            for pulse in pulsed_plugs.drain(..).rev()
                            {
                                stack.push(VisitBlock
                                {
                                    visit: Visit
                                    {
                                        block: pulse.block,
                                        action: VisitAction::Pulse(pulse.inlet, parent_latch /* forward through */),
                                    },
                                    depth: depth + 1
                                });
                            }
                        }
                    }
                    else
                    {
                        let hydrated = self.hydrated_latches.entry(test_visit.block.value())
                            .or_insert_with(|| HydratedLatch { is_powered: false, powered_outlets: SmallVec::new(), });
                        match inlet
                        {
                            Inlet::Pulse =>
                            {
                                if hydrated.is_powered { continue; }
                                hydrated.is_powered = true;

                                self.push_action(History::PowerOn(test_visit.block));

                                // link the parent directly to this state for when powering-off
                                // todo: can downstream latches be stored statically?
                                if let Some(parent_latch) = parent_latch
                                {
                                    let hydrated_parent = self.hydrated_latches.get_mut(&parent_latch.value())
                                        .expect("Parent latch set but no hydrated state exists for it??");
                                    hydrated_parent.powered_outlets.push(test_visit.block);
                                }

                                let latch = self.circuit.latches[test_visit.block.value() as usize].as_ref();
                                latch.power_on(
                                    scope!(test_visit.block),
                                    LatchActions
                                    {
                                        pulse_plugs: &mut pulsed_plugs,
                                        latch_plugs: &mut latched_plugs,
                                        runtime: context.runtime.clone(),
                                    }
                                );

                                process_pulses!(depth, test_visit.block);
                                process_var_changes!(depth);
                            }
                            Inlet::PowerOff =>
                            {
                                if !hydrated.is_powered { continue; }

                                // todo: store downstream latches in circuit

                                if hydrated.powered_outlets.is_empty()
                                {
                                    hydrated.is_powered = false;

                                    self.push_action(History::PowerOff(test_visit.block));

                                    let latch = &self.circuit.latches[test_visit.block.value() as usize];
                                    latch.power_off(scope!(test_visit.block));

                                    process_var_changes!(depth);
                                }
                                else
                                {
                                    // re-visit this after powering-off all downstream latches
                                    stack.push(VisitBlock
                                    {
                                        visit: test_visit,
                                        depth: depth + 1,
                                    });

                                    for powered in hydrated.powered_outlets.drain(..).rev() // should this be rev order?
                                    {
                                        stack.push(VisitBlock
                                        {
                                            visit: Visit
                                            {
                                                block: powered,
                                                action: VisitAction::Pulse(Inlet::PowerOff, None),
                                            },
                                            depth: depth + 1,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                VisitAction::ReEnter =>
                {
                    // should be verified earlier
                    debug_assert!(test_visit.block.is_latch(), "Only latch blocks can be re-entered");
                    self.push_action(History::ReEnter(test_visit.block));

                    let latch = &self.circuit.latches[test_visit.block.value() as usize];
                    latch.re_enter(scope!(test_visit.block), LatchActions
                    {
                        pulse_plugs: &mut pulsed_plugs,
                        latch_plugs: &mut latched_plugs,
                        runtime: context.runtime.clone(),
                    });

                    process_pulses!(depth, test_visit.block);
                    process_var_changes!(depth);
                }
                VisitAction::VarChanged(change) =>
                {
                    // should be verified earlier
                    debug_assert!(test_visit.block.is_latch(), "Only latch blocks can be re-entered");
                    self.push_action(History::VarChanged(change.var, change.new_value.clone()));

                    let latch = &self.circuit.latches[test_visit.block.value() as usize];
                    latch.on_var_changed(change, scope!(test_visit.block), LatchActions
                    {
                        pulse_plugs: &mut pulsed_plugs,
                        latch_plugs: &mut latched_plugs,
                        runtime: context.runtime.clone(),
                    });

                    process_pulses!(depth, test_visit.block);
                    process_var_changes!(depth);
                }
            }
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

    #[must_use]
    pub fn as_graphviz(&self) -> String
    {
        let mut out_str = String::new();

        write!(out_str, "digraph {{\n  rankdir=LR\n  splines=ortho\n  bgcolor=transparent\n").unwrap(); // todo: name?

        let mut stack: SmallVec<[BlockId; 16]> = SmallVec::new();
        write!(out_str, "  \"auto_entry\" [label=\"Automatic entry\" class=\"entry\" shape=\"cds\"]\n").unwrap();
        for outlet in &self.circuit.auto_entries
        {
            stack.push(*outlet);
            write!(out_str, "  \"auto_entry\" -> \"{:?}\":IN [minlen=3]\n", outlet).unwrap();
        }

        while let Some(block) = stack.pop()
        {
            let block_name: &str = "";
            let mut pulses = VisitList::default();
            let mut latches = VisitList::default();

            if block.is_impulse()
            {
                let impulse = self.circuit.impulses[block.value() as usize].as_ref();

                impulse.inspect(BlockVisitor
                {
                    name: &block_name,
                    pulses: &mut pulses,
                    latches: &mut latches,
                });
                debug_assert!(latches.is_empty(), "Impulse blocks cannot have latches");

                write!(out_str, "  \"{:?}\" [class=impulse shape=record label=\"{{ <IN> ∿ | {}\\n\\N",
                    block,
                    block_name).unwrap();

                if !pulses.is_empty()
                {
                    out_str.push_str(" | { ");
                }

                for (i, outlet) in pulses.iter().enumerate()
                {
                    if i > 0 { out_str.push_str(" | "); }
                    write!(out_str, "<P{}> {}", i, outlet.0).unwrap();
                }

                if !pulses.is_empty()
                {
                    out_str.push_str(" }");
                }
                out_str.push_str(" }\"]\n");
            }
            else
            {
                let hydrated = self.hydrated_latches.get(&block.value());
                let latch = self.circuit.latches[block.value() as usize].as_ref();

                let is_powered = hydrated.map(|h| h.is_powered).unwrap_or(false);

                latch.inspect(BlockVisitor
                {
                    name: &block_name,
                    pulses: &mut pulses,
                    latches: &mut latches,
                });

                write!(out_str, "  \"{:?}\" [class=latch shape=record label=\"{{ {{ <IN> ∿ | <OFF> ◯ }} | {}\\n\\N",
                       block,
                       block_name).unwrap();

                if !pulses.is_empty() || !latches.is_empty()
                {
                    out_str.push_str(" | { ");
                }

                for (i, outlet) in pulses.iter().enumerate()
                {
                    if i > 0 { out_str.push_str(" | "); }
                    write!(out_str, "<P{}> {}", i, outlet.0).unwrap();
                }

                for (i, outlet) in latches.iter().enumerate()
                {
                    if i > 0 { out_str.push_str(" | "); }
                    write!(out_str, "<L{}> {}", i, outlet.0).unwrap();
                }

                if !pulses.is_empty() || !latches.is_empty()
                {
                    out_str.push_str(" }");
                }
                out_str.push_str(" }\"]\n");
            }

            for pulse in pulses.iter()
            {
                for (i, plug) in pulse.1.iter().enumerate()
                {
                    stack.push(plug.block);
                    write!(out_str, "  \"{:?}\":\"P{}\" -> \"{:?}\":\"{}\" [minlen=3 class=\"pulse-plug\"]\n",
                           block,
                           i,
                           plug.block,
                           "IN").unwrap();
                }
            }
            for latch in latches.iter()
            {
                for (i, plug) in latch.1.iter().enumerate()
                {
                    stack.push(plug.block);
                    write!(out_str, "  \"{:?}\":\"L{}\" -> \"{:?}\":\"{}\" [color=\"black:invis:black\" class=\"latch-plug\" minlen=3]\n",
                           block,
                           i,
                           plug.block,
                           "IN").unwrap();
                }
            }
        }

        write!(out_str, "}}").unwrap();
        out_str
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
    let run_cxt = RunContext
    {
        run_id: InstRunId::TEST,
        shared_scope,
        runtime: Runtime::new(),
    };
    run_cxt
}

#[cfg(test)]
mod traversal_tests
{
    use nab_3l14::utils::ShortTypeName;
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

        fn inspect(&self, mut visit: BlockVisitor)
        {
            visit.set_name(Self::short_type_name());
            visit.visit_pulses("Outlet", &self.outlet);
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

        fn inspect(&self, mut visit: BlockVisitor)
        {
            visit.set_name(Self::short_type_name());
            visit.visit_pulses("On True", &self.on_true_outlet);
            visit.visit_pulses("On True", &self.on_false_outlet);
            visit.visit_latches("True", &self.true_outlet);
            visit.visit_latches("False", &self.false_outlet);
            visit.visit_latches("Powered", &self.powered_outlet);
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
                        plugs: Box::new([Plug { block: BlockId::latch(1), inlet: Inlet::Pulse }]),
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
                        plugs: Box::new([Plug { block: BlockId::impulse(1), inlet: Inlet::Pulse }]),
                    },
                    false_outlet: LatchingOutlet
                    {
                        plugs: Box::new([Plug { block: BlockId::impulse(0), inlet: Inlet::Pulse }]),
                    },
                    true_outlet: LatchingOutlet
                    {
                        plugs: Box::new([Plug { block: BlockId::impulse(3), inlet: Inlet::Pulse }]),
                    },
                    .. Default::default()
                }),

                Box::new(TestLatch
                {
                    name: "Latch 1 (true)",
                    value: true,
                    true_outlet: LatchingOutlet
                    {
                        plugs: Box::new([Plug { block: BlockId::impulse(2), inlet: Inlet::Pulse }]),
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
            History::Visit(BlockId::latch(0)),
            History::PowerOn(BlockId::latch(0)),
            History::Visit(BlockId::impulse(0)),
            History::Pulse(BlockId::impulse(0)),
            History::Visit(BlockId::latch(1)),
            History::PowerOn(BlockId::latch(1)),
            History::Visit(BlockId::impulse(2)),
            History::Pulse(BlockId::impulse(2)),
            History::Visit(BlockId::impulse(1)),
            History::Pulse(BlockId::impulse(1)),
            History::Visit(BlockId::impulse(4)),
            History::Pulse(BlockId::impulse(4)),
        ]);

        instance.clear_action_history();

        instance.visit([Visit { block: BlockId::latch(0), action: VisitAction::Pulse(Inlet::PowerOff, None) }], run_cxt.clone());

        assert_eq!(instance.latch_has_power(0), false);
        assert_eq!(instance.latch_has_power(1), false);

        assert_eq!(instance.get_action_history(), &[
            History::Visit(BlockId::latch(0)), // Inlet::PowerOff
            History::Visit(BlockId::latch(1)), // Inlet::PowerOff
            History::PowerOff(BlockId::latch(1)),
            History::Visit(BlockId::latch(0)),
            History::PowerOff(BlockId::latch(0)),
        ]);

        instance.clear_action_history();
        instance.visit([Visit { block: BlockId::latch(1), action: VisitAction::Pulse(Inlet::Pulse, None) }], run_cxt.clone());
        assert!(instance.any_latches_powered());
        instance.power_off(run_cxt.clone());
        assert!(!instance.any_latches_powered());

        assert_eq!(instance.get_action_history(), &[
            History::Visit(BlockId::latch(1)),
            History::PowerOn(BlockId::latch(1)),
            History::Visit(BlockId::impulse(2)),
            History::Pulse(BlockId::impulse(2)),
            History::InstancePowerOff,
            History::Visit(BlockId::latch(1)), // Inlet::PowerOff
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
                        plugs: Box::new([Plug { block: BlockId::latch(0), inlet: Inlet::PowerOff }]),
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
                        plugs: Box::new([Plug { block: BlockId::impulse(0), inlet: Inlet::Pulse }]),
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
            History::Visit(BlockId::latch(0)),
            History::PowerOn(BlockId::latch(0)),
            History::Visit(BlockId::impulse(0)),
            History::Pulse(BlockId::impulse(0)),
            // powering off latch will go through impulse and back to itself
            History::Visit(BlockId::latch(0)), // Inlet::PowerOff
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
                        plugs: Box::new([Plug { block: BlockId::latch(1), inlet: Inlet::Pulse }]),
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

        instance.visit([Visit { block: BlockId::latch(0), action: VisitAction::Pulse(Inlet::PowerOff, None) }], run_cxt.clone());

        assert_eq!(instance.latch_has_power(0), false);
        assert_eq!(instance.latch_has_power(1), true);

        instance.power_off(run_cxt.clone());

        assert_eq!(instance.get_action_history(), &[
            History::InstancePowerOn,
            History::Visit(BlockId::latch(0)),
            History::PowerOn(BlockId::latch(0)),
            History::Visit(BlockId::latch(1)),
            History::PowerOn(BlockId::latch(1)),
            History::Visit(BlockId::latch(0)), // Inlet::PowerOff
            History::PowerOff(BlockId::latch(0)),
            History::InstancePowerOff,
            History::Visit(BlockId::latch(1)), // Inlet::PowerOff
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
            History::Visit(BlockId::impulse(0)),
            History::Pulse(BlockId::impulse(0)),
            History::Visit(BlockId::latch(0)),
            History::PowerOn(BlockId::latch(0)),
            History::InstancePowerOff,
            History::Visit(BlockId::latch(0)), // Inlet::PowerOff
            History::PowerOff(BlockId::latch(0)),
        ]);
    }

    // todo: multiple outlets pulse in defined order
    #[test]
    fn multiple_outlets()
    {
        let circuit = Circuit
        {
            auto_entries: Box::new([]),
            signaled_entries: Box::new([
                (Signal::test('i'), Box::new([BlockId::impulse(0)])),
                (Signal::test('l'), Box::new([BlockId::latch(0)])),
            ]),
            impulses: Box::new([
                Box::new(TestImpulse
                {
                    name: "IA",
                    outlet: PulsedOutlet
                    {
                        plugs: Box::new([
                            Plug { block: BlockId::impulse(1), inlet: Inlet::Pulse },
                            Plug { block: BlockId::impulse(2), inlet: Inlet::Pulse },
                            Plug { block: BlockId::impulse(3), inlet: Inlet::Pulse },
                        ]),
                    },
                }),
                Box::new(TestImpulse
                {
                    name: "IB",
                    outlet: PulsedOutlet::default(),
                }),
                Box::new(TestImpulse
                {
                    name: "IC",
                    outlet: PulsedOutlet::default(),
                }),
                Box::new(TestImpulse
                {
                    name: "ID",
                    outlet: PulsedOutlet::default(),
                }),
            ]),
            latches: Box::new([
                Box::new(TestLatch
                {
                    name: "LA",
                    powered_outlet: LatchingOutlet
                    {
                        plugs: Box::new([
                            Plug { block: BlockId::latch(1), inlet: Inlet::Pulse },
                            Plug { block: BlockId::latch(2), inlet: Inlet::Pulse },
                            Plug { block: BlockId::impulse(1), inlet: Inlet::Pulse },
                            Plug { block: BlockId::impulse(2), inlet: Inlet::Pulse },
                        ]),
                    },
                    .. Default::default()
                }),
                Box::new(TestLatch
                {
                    name: "LB",
                    .. Default::default()
                }),
                Box::new(TestLatch
                {
                    name: "LC",
                    .. Default::default()
                }),
            ]),
            num_local_vars: 0,
        };

        let mut instance = Instance::new(circuit);
        let shared_scope = SharedScope::default();
        let run_cxt = gen_run_cxt(&shared_scope);

        instance.signal(0, run_cxt.clone());
        assert_eq!(instance.get_action_history(), &[
            History::Signal(Signal::test('i')),
            History::Visit(BlockId::impulse(0)),
            History::Pulse(BlockId::impulse(0)),
            History::Visit(BlockId::impulse(1)),
            History::Pulse(BlockId::impulse(1)),
            History::Visit(BlockId::impulse(2)),
            History::Pulse(BlockId::impulse(2)),
            History::Visit(BlockId::impulse(3)),
            History::Pulse(BlockId::impulse(3)),
        ]);

        instance.clear_action_history();
        instance.signal(1, run_cxt.clone());
        assert_eq!(instance.get_action_history(), &[
            History::Signal(Signal::test('l')),
            History::Visit(BlockId::latch(0)),
            History::PowerOn(BlockId::latch(0)),
            History::Visit(BlockId::latch(1)),
            History::PowerOn(BlockId::latch(1)),
            History::Visit(BlockId::latch(2)),
            History::PowerOn(BlockId::latch(2)),
            History::Visit(BlockId::impulse(1)),
            History::Pulse(BlockId::impulse(1)),
            History::Visit(BlockId::impulse(2)),
            History::Pulse(BlockId::impulse(2)),
        ]);

        instance.clear_action_history();
        instance.power_off(run_cxt.clone());
        assert_eq!(instance.get_action_history(), &[
            History::InstancePowerOff,
            History::Visit(BlockId::latch(0)),
            History::Visit(BlockId::latch(1)),
            History::PowerOff(BlockId::latch(1)),
            History::Visit(BlockId::latch(2)),
            History::PowerOff(BlockId::latch(2)),
            History::Visit(BlockId::latch(0)),
            History::PowerOff(BlockId::latch(0)),
            History::Visit(BlockId::latch(1)),
            History::Visit(BlockId::latch(2)),
        ]);
    }
}

// TODO: re-entrance tests

#[cfg(test)]
mod var_tests
{
    use nab_3l14::utils::ShortTypeName;
    use super::*;

    struct TestImpulse;
    impl ImpulseBlock for TestImpulse
    {
        fn pulse(&self, _scope: Scope, _actions: ImpulseActions) { println!("pulsed TestImpulse"); }
        fn inspect(&self, mut visit: BlockVisitor) { }
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
        fn inspect(&self, visit_outlets: BlockVisitor) { }
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
            println!("read {:?} = {:?} -> {:?}", change.var, change.old_value, change.new_value);
            actions.pulse(&self.on_read);
        }
        fn inspect(&self, mut visit: BlockVisitor)
        {
            visit.set_name(Self::short_type_name());
            visit.visit_pulses("On Read", &self.on_read);
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
                        plugs: Box::new([Plug { block: BlockId::impulse(0), inlet: Inlet::Pulse }]),
                    },
                }),
            ]),

            num_local_vars: 1,
        };

        let mut instance = Instance::new(circuit);
        let shared_scope = SharedScope::default();
        let run_cxt = gen_run_cxt(&shared_scope);

        instance.power_on(run_cxt.clone());

        assert_eq!(instance.get_action_history(), &[
            History::InstancePowerOn,
            History::Visit(BlockId::latch(0)),
            History::PowerOn(BlockId::latch(0)),
            History::Visit(BlockId::latch(1)),
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
                        plugs: Box::new([Plug { block: BlockId::impulse(0), inlet: Inlet::Pulse }]),
                    },
                }),
            ]),

            num_local_vars: 1,
        };

        let mut instance = Instance::new(circuit);
        let shared_scope = SharedScope::default();
        let run_cxt = gen_run_cxt(&shared_scope);

        instance.power_on(run_cxt.clone());

        assert_eq!(instance.get_action_history(), &[
            History::InstancePowerOn,
            History::Visit(BlockId::latch(1)),
            History::PowerOn(BlockId::latch(1)),
            History::Visit(BlockId::latch(0)),
            History::PowerOn(BlockId::latch(0)),
            History::Visit(BlockId::latch(1)),
            History::VarChanged(VarId::test(0, VarScope::Local), VarValue::Bool(true)),
            History::Visit(BlockId::impulse(0)),
            History::Pulse(BlockId::impulse(0)),
        ]);

        instance.power_off(run_cxt.clone());
    }

    // TODO: change that kicks off another change
    // TODO: change that kicks off propagation (that kicks off another change?)
}