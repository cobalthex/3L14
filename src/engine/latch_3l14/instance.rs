use super::*;
use nab_3l14::{append_file, debug_panic, Signal};
use smallvec::SmallVec;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter, Write};
use std::sync::Arc;
use nab_3l14::utils::ShortTypeName;
use crate::latches::Latch;

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
pub(super) enum History
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

pub(super) type LatchContextStorage = Option<Box<dyn Debug + Send>>;

#[derive(Debug)]
struct HydratedLatch
{
    is_powered: bool, // flags?
    powered_outlets: SmallVec<[BlockId; 4]>, // maybe powered
    latch_context: LatchContextStorage,
}

#[derive(Clone)] // not ideal pubicly
pub struct RunContext<'r>
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

    #[inline] #[must_use]
    pub fn local_scope(&self) -> &LocalScope { &self.scope }

    // power-on all the auto-entry blocks
    pub fn power_on(&mut self, context: RunContext)
    {
        #[cfg(any(test, feature = "action_history"))]
        self.action_history.push(History::InstancePowerOn);

        let auto_blocks: SmallVec<[BlockId; 8]> = SmallVec::from_slice(&self.circuit.auto_entries);
        self.visit(
            auto_blocks.iter().rev().map(|b| Visit { block: *b, action: VisitAction::Pulse(Inlet::Pulse, None) }),
            context);
    }

    // power-on the signaled entry blocks
    pub fn signal(&mut self, signal_slot: usize, context: RunContext)
    {
        #[cfg(any(test, feature = "action_history"))]
        self.action_history.push(History::Signal(self.circuit.signaled_entries[signal_slot].0));

        let mut outlets = SmallVec::<[_; 2]>::new();
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
        #[cfg(any(test, feature = "action_history"))]
        self.action_history.push(History::InstancePowerOff);

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

        while let Some(VisitBlock { visit: test_visit, depth }) = stack.pop()
        {
            debug_assert!(depth < MAX_VISIT_DEPTH, "Maximum visit depth exceeded");

            macro_rules! scope { ($runtime_state:expr) =>
            {
                Scope
                {
                    run_id: context.run_id,
                    block_id: test_visit.block,
                    local_scope: &mut self.scope,
                    local_changes: &mut local_changes,
                    shared_scope: context.shared_scope,
                    shared_changes: &mut shared_changes,
                    latch_context: unsafe { $runtime_state as *mut _ },
                }
            } }
            macro_rules! process_pulses { ($option_parent_latch:expr) =>
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
                            // only propagate upstream latch pulses through impulses (downstream latches 'reset' propagation)
                            action: VisitAction::Pulse(pulse.inlet, if test_visit.block.is_latch() { None } else { $option_parent_latch }),
                        },
                        depth: depth + 1
                    });
                }
                for latch in latched_plugs.drain(..).rev()
                {
                    stack.push(VisitBlock
                    {
                        visit: Visit
                        {
                            block: latch.block,
                            action: VisitAction::Pulse(latch.inlet, $option_parent_latch),
                        },
                        depth: depth + 1
                    });
                }
            } }
            macro_rules! process_var_changes { () =>
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
                        depth: depth + 1,
                    })
                }
                // TODO: shared changes
            } }

            #[cfg(any(test, feature = "action_history"))]
            self.action_history.push(History::Visit(test_visit.block));

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
                                scope!(std::ptr::null_mut()), // impulses cannot use tracked data
                                ImpulseActions
                                {
                                    pulse_outlets: &mut pulsed_plugs,
                                    runtime: context.runtime.clone(),
                                }
                            );

                            process_pulses!(parent_latch); // pass-thru parent latch
                            process_var_changes!();
                        }
                        }
                    else
                    {
                        let hydrated = self.hydrated_latches.entry(test_visit.block.value())
                            .or_insert_with(|| HydratedLatch
                            {
                                is_powered: false,
                                powered_outlets: SmallVec::new(),
                                latch_context: None,
                            });
                        match inlet
                        {
                            Inlet::Pulse =>
                            {
                                if hydrated.is_powered { continue; }
                                hydrated.is_powered = true;

                                #[cfg(any(test, feature = "action_history"))]
                                self.action_history.push(History::PowerOn(test_visit.block));

                                let latch = self.circuit.latches[test_visit.block.value() as usize].as_ref();

                                latch.power_on(
                                    scope!(&mut hydrated.latch_context),
                                    LatchActions
                                    {
                                        pulse_plugs: &mut pulsed_plugs,
                                        latch_plugs: &mut latched_plugs,
                                        runtime: context.runtime.clone(),
                                    }
                                );

                                // link the parent directly to this state for when powering-off
                                // todo: can downstream latches be stored statically?
                                if let Some(parent_latch) = parent_latch
                                {
                                    append_file!("D:\\latch_test.txt", "%%% {parent_latch:?}");
                                    let hydrated_parent = self.hydrated_latches.get_mut(&parent_latch.value())
                                        .expect("Parent latch set but no hydrated state exists for it??");
                                    hydrated_parent.powered_outlets.push(test_visit.block);
                                }

                                process_pulses!(Some(test_visit.block));
                                process_var_changes!();
                            }
                            Inlet::PowerOff =>
                            {
                                if !hydrated.is_powered { continue; }

                                // todo: store downstream latches in circuit

                                if hydrated.powered_outlets.is_empty()
                                {
                                    hydrated.is_powered = false;

                                    #[cfg(any(test, feature = "action_history"))]
                                    self.action_history.push(History::PowerOff(test_visit.block));

                                    let latch = &self.circuit.latches[test_visit.block.value() as usize];
                                    latch.power_off(scope!(&mut hydrated.latch_context));

                                    process_var_changes!();
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

                    #[cfg(any(test, feature = "action_history"))]
                    self.action_history.push(History::ReEnter(test_visit.block));

                    // only powered latches can be re-entered
                    let Some(hydrated) = self.hydrated_latches.get_mut(&test_visit.block.value())
                        else { debug_panic!("No hydrated state for latch"); return; };

                    if !hydrated.is_powered
                    {
                        continue;
                    }

                    let latch = &self.circuit.latches[test_visit.block.value() as usize];
                    latch.re_enter(scope!(&mut hydrated.latch_context),
                                   LatchActions
                        {
                            pulse_plugs: &mut pulsed_plugs,
                            latch_plugs: &mut latched_plugs,
                            runtime: context.runtime.clone(),
                        });

                    process_pulses!(Some(test_visit.block));
                    process_var_changes!();
                }
                VisitAction::VarChanged(change) =>
                {
                    // should be verified earlier
                    debug_assert!(test_visit.block.is_latch(), "Only latch blocks can be re-entered");
                    #[cfg(any(test, feature = "action_history"))]
                    self.action_history.push(History::VarChanged(change.var, change.new_value.clone()));

                    let Some(hydrated) = self.hydrated_latches.get_mut(&test_visit.block.value())
                        else { debug_panic!("No hydrated state for latch"); return; };

                    if !hydrated.is_powered
                    {
                        continue;
                    }

                    let latch = &self.circuit.latches[test_visit.block.value() as usize];
                    latch.on_var_changed(change, scope!(&mut hydrated.latch_context),
                                         LatchActions
                    {
                        pulse_plugs: &mut pulsed_plugs,
                        latch_plugs: &mut latched_plugs,
                        runtime: context.runtime.clone(),
                    });

                    process_pulses!(Some(test_visit.block));
                    process_var_changes!();
                }
            }
        }
    }

    #[inline]
    pub(super) fn clear_action_history(&mut self)
    {
        #[cfg(any(test, feature = "action_history"))]
        self.action_history.clear();
    }
    #[inline] #[must_use]
    pub(super) fn get_action_history(&self) -> &[History]
    {
        #[cfg(any(test, feature = "action_history"))]
        { &self.action_history }
        #[cfg(not(any(test, feature = "action_history")))]
        { &[] }
    }

    #[inline] #[must_use]
    pub fn is_latch_powered(&self, latch: u32) -> bool
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
        write!(out_str, "  \"auto_entry\" [label=\"Automatic entry  \" class=\"entry\" shape=\"cds\"]\n").unwrap();
        for outlet in &self.circuit.auto_entries
        {
            stack.push(*outlet);
            write!(out_str, "  \"auto_entry\" -> \"{:?}\":\"IN\"\n", outlet).unwrap();
        }

        for (i, (signal, entries)) in self.circuit.signaled_entries.iter().enumerate()
        {
            write!(out_str, "  \"signal_{}\" [label=\" {:?}  \" class=\"signal entry\" shape=\"rpromoter\"]\n", i, signal).unwrap();
            for entry in entries
            {
                stack.push(*entry);
                write!(out_str, "  \"signal_{}\" -> \"{:?}\":\"IN\"\n", i, entry).unwrap();
            }
        }

        fn escape_string(input: &str) -> String
        {
            let mut result = String::with_capacity(input.len() * 2); // Pre-allocate for worst case
            for ch in input.chars()
            {
                match ch
                {
                    '{' | '}' | '|' | '"' | '<' | '>' =>
                    {
                        result.push('\\');
                        result.push(ch);
                    }
                    _ => result.push(ch),
                }
            }
            result
        }

        for i in 0..self.circuit.impulses.len()
        {
            stack.push(BlockId::impulse(i as u32));
        }
        for i in 0..self.circuit.latches.len()
        {
            stack.push(BlockId::latch(i as u32));
        }

        // todo: rework to not need stack
        while let Some(block) = stack.pop()
        {
            let mut block_name: &str = "";
            let mut annotation = String::new();
            let mut pulses = VisitList::default();
            let mut latches = VisitList::default();

            if block.is_impulse()
            {
                let impulse = self.circuit.impulses[block.value() as usize].as_ref();

                impulse.inspect(BlockVisitor
                {
                    name: &mut block_name,
                    annotation: &mut annotation,
                    pulses: &mut pulses,
                    latches: &mut latches,
                });
                debug_assert!(latches.is_empty(), "Impulse blocks cannot have latches");

                write!(out_str, "  \"{:?}\" [class=impulse shape=record style=rounded label=\"{{ <IN> ∿ | {}",
                    block,
                    if annotation.is_empty() { block_name } else { &format!("{{ {block_name} | {} }}", escape_string(&annotation)) }).unwrap();

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
                    name: &mut block_name,
                    annotation: &mut annotation,
                    pulses: &mut pulses,
                    latches: &mut latches,
                });

                write!(out_str, "  \"{:?}\" [class=\"{} latch\" shape=record label=\"{{ {{ <IN> ∿ | <OFF> ◯ }} | {}",
                       block,
                       if is_powered { "powered" } else { "" },
                       if annotation.is_empty() { block_name } else { &format!("{{ {block_name} | {} }}", escape_string(&annotation)) }).unwrap();

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
                    if !pulses.is_empty() || i > 0 { out_str.push_str(" | "); }
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
                    // stack.push(plug.block);
                    write!(out_str, "  \"{:?}\":\"P{}\" -> \"{:?}\":\"{}\" [class=\"pulse-plug\"]\n",
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
                    // stack.push(plug.block);
                    write!(out_str, "  \"{:?}\":\"L{}\" -> \"{:?}\":\"{}\" [color=\"black:invis:black\" class=\"latch-plug\"]\n",
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
fn gen_run_cxt(shared_scope: &SharedScope) -> RunContext<'_>
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
    use super::*;
    use nab_3l14::utils::ShortTypeName;

    #[derive(Default)]
    struct TestImpulse
    {
        #[allow(dead_code)]
        name: &'static str,
        outlet: PulsedOutlet,
    }
    impl ImpulseBlock for TestImpulse
    {
        fn pulse(&self, _scope: Scope, mut actions: ImpulseActions)
        {
            actions.pulse(&self.outlet);
        }

        fn inspect(&self, mut visit: BlockVisitor)
        {
            visit.set_name(Self::short_type_name());
            visit.visit_pulses("Outlet", &self.outlet);
        }
    }

    #[derive(Default, Debug)]
    struct TestLatchContext
    {
        test: usize,
    }

    #[derive(Default)]
    struct TestLatch
    {
        #[allow(dead_code)]
        name: &'static str,
        value: bool,

        on_true_outlet: PulsedOutlet,
        true_outlet: LatchingOutlet,

        on_false_outlet: PulsedOutlet,
        false_outlet: LatchingOutlet,

        powered_outlet: LatchingOutlet,
    }
    impl ContextfulLatchBlock for TestLatch
    {
        type Context = TestLatchContext;
        fn power_on(&self, c: &mut Self::Context, _scope: Scope, mut actions: LatchActions)
        {
            c.test = 5; // testing contextful latch state pointer arithmetic

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

        fn power_off(&self, c: &mut Self::Context, _scope: Scope)
        {
            assert_eq!(c.test, 5);
            c.test = 0;
        }

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

        assert_eq!(instance.is_latch_powered(0), false);
        assert_eq!(instance.is_latch_powered(1), false);

        instance.power_on(run_cxt.clone());

        assert_eq!(instance.is_latch_powered(0), true);
        assert_eq!(instance.is_latch_powered(1), true);

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

        append_file!("D:\\latch_test.txt", "!!! {:#?}", instance.hydrated_latches);
        instance.visit([Visit { block: BlockId::latch(0), action: VisitAction::Pulse(Inlet::PowerOff, None) }], run_cxt.clone());

        append_file!("D:\\latch_test.txt", "$$$ {:#?}", instance.hydrated_latches);
        assert_eq!(instance.is_latch_powered(0), false);
        assert_eq!(instance.is_latch_powered(1), false);

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

        assert_eq!(instance.is_latch_powered(0), false);

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

        assert_eq!(instance.is_latch_powered(0), true);
        assert_eq!(instance.is_latch_powered(1), true);

        instance.visit([Visit { block: BlockId::latch(0), action: VisitAction::Pulse(Inlet::PowerOff, None) }], run_cxt.clone());

        assert_eq!(instance.is_latch_powered(0), false);
        assert_eq!(instance.is_latch_powered(1), true);

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

        assert_eq!(instance.is_latch_powered(0), false);
        assert_eq!(instance.get_action_history(), &[
            History::InstancePowerOn,
        ]);
        instance.clear_action_history();

        instance.signal(0, run_cxt.clone());
        assert_eq!(instance.is_latch_powered(0), true);

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

    #[test]
    #[should_panic]
    fn recursion_assertion()
    {
        let circuit = Circuit
        {
            auto_entries: Box::new([BlockId::impulse(0)]),
            signaled_entries: Box::new([]),
            impulses: Box::new([
                Box::new(TestImpulse
                {
                    name: "first",
                    outlet: PulsedOutlet
                    {
                        plugs: Box::new([Plug::new(BlockId::impulse(1), Inlet::Pulse)]),
                    },
                }),
                Box::new(TestImpulse
                {
                    name: "second",
                    outlet: PulsedOutlet
                    {
                        plugs: Box::new([Plug::new(BlockId::impulse(0), Inlet::Pulse)]),
                    }
                })
            ]),
            latches: Box::new([]),
            num_local_vars: 0,
        };
        
        let mut instance = Instance::new(circuit);
        let shared_scope = SharedScope::default();
        let run_cxt = gen_run_cxt(&shared_scope);
        instance.power_on(run_cxt.clone());
    }
}

// TODO: re-entrance tests

#[cfg(test)]
mod var_tests
{
    use super::*;
    use nab_3l14::utils::ShortTypeName;

    struct TestImpulse
    {
        pub var: VarId,
    }
    impl ImpulseBlock for TestImpulse
    {
        fn pulse(&self, scope: Scope, _actions: ImpulseActions) { println!("Var {:?} = {:?}", self.var, scope.get(self.var)); }
        fn inspect(&self, _visit: BlockVisitor) { }
    }

    struct WriteLatch
    {
        pub var: VarId,
        pub to_value: bool,
    }
    impl LatchBlock for WriteLatch
    {
        fn power_on(&self, mut scope: Scope, _actions: LatchActions)
        {
            println!("powered on WriteLatch");
            scope.set(self.var, VarValue::Bool(self.to_value));
        }
        fn power_off(&self, _scope: Scope) { }
        fn inspect(&self, _visit: BlockVisitor) { }
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
                Box::new(TestImpulse { var: VarId::test(0, VarScope::Local) }),
            ]),

            latches: Box::new([
                Box::new(WriteLatch
                {
                    var: VarId::test(0, VarScope::Local),
                    to_value: true,
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
                BlockId::latch(0),
                BlockId::impulse(0),
                BlockId::latch(1),
                BlockId::latch(2),
            ]),

            signaled_entries: Box::new([]),

            impulses: Box::new([
                Box::new(TestImpulse { var: VarId::test(0, VarScope::Local) }),
            ]),

            latches: Box::new([
                Box::new(WriteLatch
                {
                    var: VarId::test(0, VarScope::Local),
                    to_value: false,
                }),
                Box::new(ReadLatch
                {
                    var: VarId::test(0, VarScope::Local),
                    on_read: PulsedOutlet
                    {
                        plugs: Box::new([Plug { block: BlockId::impulse(0), inlet: Inlet::Pulse }]),
                    },
                }),
                Box::new(WriteLatch
                {
                    var: VarId::test(0, VarScope::Local),
                    to_value: true,
                }),
            ]),

            num_local_vars: 1,
        };

        let mut instance = Instance::new(circuit);
        let shared_scope = SharedScope::default();
        let run_cxt = gen_run_cxt(&shared_scope);

        instance.power_on(run_cxt.clone());

        println!("{:?}", instance.get_action_history());
        assert_eq!(instance.get_action_history(), &[
            History::InstancePowerOn,
            History::Visit(BlockId::latch(0)),
            History::PowerOn(BlockId::latch(0)),
            History::Visit(BlockId::impulse(0)),
            History::Pulse(BlockId::impulse(0)),
            History::Visit(BlockId::latch(1)),
            History::PowerOn(BlockId::latch(1)),
            History::Visit(BlockId::latch(2)),
            History::PowerOn(BlockId::latch(2)),
            History::Visit(BlockId::latch(1)),
            History::VarChanged(VarId::test(0, VarScope::Local), VarValue::Bool(true)),
            History::Visit(BlockId::impulse(0)),
            History::Pulse(BlockId::impulse(0)),
        ]);

        instance.power_off(run_cxt.clone());
    }

    // TODO: change that kicks off another change
    // TODO: change that kicks off propagation (that kicks off another change?)
    // TODO: verify pulses also propagate
}