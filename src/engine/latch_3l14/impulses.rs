use bitcode::Decode;
use super::{BlockVisitor, ImpulseActions, ImpulseBlock, PulsedOutlet, Runtime, Scope, VarId, VarValue};
use nab_3l14::utils::ShortTypeName;
use nab_3l14::Signal;
use proc_macros_3l14::CircuitBlock;

pub struct NoOp
{
    pub outlet: PulsedOutlet,
}
impl ImpulseBlock for NoOp
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

#[derive(CircuitBlock, Decode)]
pub struct DebugPrint
{
    pub message: String,
    // todo: format strings

    pub outlet: PulsedOutlet,
}
impl ImpulseBlock for DebugPrint
{
    fn pulse(&self, _scope: Scope, mut actions: ImpulseActions)
    {
        log::debug!("LATCH> {}", self.message);
        actions.pulse(&self.outlet);
    }

    fn inspect(&self, mut visit: BlockVisitor)
    {
        visit.set_name(Self::short_type_name());
        visit.visit_pulses("Outlet", &self.outlet);
    }
}

pub struct SetVars
{
    // TODO: multiple vars
    pub var: VarId,
    pub to_value: VarValue, // expression?

    pub outlet: PulsedOutlet,
}
impl ImpulseBlock for SetVars
{
    fn pulse(&self, mut scope: Scope, mut actions: ImpulseActions)
    {
        scope.set(self.var, self.to_value.clone());
        actions.pulse(&self.outlet);
    }

    fn inspect(&self, mut visit: BlockVisitor)
    {
        visit.set_name(Self::short_type_name());
        visit.annotate(&format!("{:?} := {:?}", self.var, self.to_value));
        visit.visit_pulses("Outlet", &self.outlet);
    }
}

pub struct EmitSignal
{
    pub signal: Signal,
    pub outlet: PulsedOutlet,
}
impl ImpulseBlock for EmitSignal
{
    fn pulse(&self, _scope: Scope, mut actions: ImpulseActions)
    {
        Runtime::signal(&actions.runtime, self.signal);
        actions.pulse(&self.outlet);
    }

    fn inspect(&self, mut visit: BlockVisitor)
    {
        visit.set_name(Self::short_type_name());
        visit.visit_pulses("Outlet", &self.outlet);
    }
}

pub struct PowerOff;
impl ImpulseBlock for PowerOff
{
    fn pulse(&self, scope: Scope, actions: ImpulseActions)
    {
        // directly send to instance?
        Runtime::power_off(&actions.runtime, scope.run_id());
    }

    fn inspect(&self, mut visit: BlockVisitor)
    {
        visit.set_name(Self::short_type_name());
    }
}

#[cfg(test)]
mod tests
{
    use super::*;
    use crate::{BlockId, Inlet, Plug, TestContext};

    #[test]
    fn no_op()
    {
        let noop = NoOp
        {
            outlet: PulsedOutlet
            {
                plugs: Box::new([Plug { block: BlockId::impulse(1), inlet: Inlet::Pulse }]),
            },
        };

        // TODO: test inspect()

        let mut tc = TestContext::default();
        tc.pulse(noop);
        assert_eq!(tc.pulse_outlets.as_slice(), &[Plug::new(BlockId::impulse(1), Inlet::Pulse)]);
    }

    #[test]
    fn debug_print()
    {
        let debug_print = DebugPrint
        {
            message: "Hello, world!".to_string(),
            outlet: PulsedOutlet
            {
                plugs: Box::new([Plug { block: BlockId::impulse(1), inlet: Inlet::Pulse }]),
            },
        };

        // TODO: test inspect()
        
        let mut tc = TestContext::default();
        tc.pulse(debug_print);
        assert_eq!(tc.pulse_outlets.as_slice(), &[Plug::new(BlockId::impulse(1), Inlet::Pulse)]);
    }
    
    // todo: set vars
    
    #[test]
    fn emit_signal()
    {
        let emit_signal = EmitSignal
        {
            signal: Signal::test('a'),
            outlet: PulsedOutlet
            {
                plugs: Box::new([Plug { block: BlockId::impulse(1), inlet: Inlet::Pulse }]),
            },
        };

        // TODO: test inspect()

        let mut tc = TestContext::default();
        tc.pulse(emit_signal);
        assert_eq!(tc.pulse_outlets.as_slice(), &[Plug::new(BlockId::impulse(1), Inlet::Pulse)]);
        // TODO: check for signal sent
        // assert_eq!(sig, Signal::test('a'));
    }
}
