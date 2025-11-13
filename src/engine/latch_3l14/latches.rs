use nab_3l14::utils::ShortTypeName;
use proc_macros_3l14::CircuitBlock;
use crate::vars::VarChange;
use super::{LatchingOutlet, PulsedOutlet, Scope, LatchBlock, BlockVisitor, LatchActions, VarValue, VarId, ContextfulLatchBlock};

// A no-op, always-active after power-on latch
#[derive(CircuitBlock, Debug)]
pub struct Latch
{
    // on_power?
    pub powered_outlet: LatchingOutlet,
}
impl LatchBlock for Latch
{
    fn power_on(&self, _scope: Scope, mut actions: LatchActions)
    {
        actions.latch(&self.powered_outlet);
    }
    fn power_off(&self, _scope: Scope) { }

    fn inspect(&self, mut visit: BlockVisitor)
    {
        visit.set_name(Self::short_type_name());
        visit.visit_latches("Powered", &self.powered_outlet);
    }
}

#[derive(CircuitBlock, Debug)]
pub struct ConditionLatch
{
    pub condition: VarId, // TODO: expression

    pub on_true_outlet: PulsedOutlet,
    pub true_outlet: LatchingOutlet,

    pub on_false_outlet: PulsedOutlet,
    pub false_outlet: LatchingOutlet,

    pub powered_outlet: LatchingOutlet,
}
#[derive(Debug, Default)]
pub struct ConditionLatchContext
{
    known_value: bool,
}
impl ContextfulLatchBlock for ConditionLatch
{
    type Context = ConditionLatchContext;

    fn power_on(&self, context: &mut Self::Context, mut scope: Scope, mut actions: LatchActions)
    {
        let curr_val = scope.subscribe(self.condition);

        context.known_value = match curr_val
        {
            VarValue::Bool(v) => v,
            _ => false,
        };

        if context.known_value
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

    fn power_off(&self, _context: &mut Self::Context, mut scope: Scope)
    {
        scope.unsubscribe(self.condition);
    }

    fn on_var_changed(&self, context: &mut Self::Context, change: VarChange, _scope: Scope, mut actions: LatchActions)
    {
        // TODO: evaluate condition
        // TODO: use runtime data here instead of old/new

        match change.new_value
        {
            VarValue::Bool(new) =>
            {
                if context.known_value == new
                {
                    return;
                }

                context.known_value = new;
                if new
                {
                    actions.unlatch(&self.false_outlet);
                    actions.pulse(&self.on_true_outlet);
                    actions.latch(&self.true_outlet);
                }
                else
                {
                    actions.unlatch(&self.true_outlet);
                    actions.pulse(&self.on_false_outlet);
                    actions.latch(&self.false_outlet);
                }
            }
            // todo: better error handling?
            _ => { log::warn!("Invalid var value for condition latch: {:?}", change.new_value); return; }
        };
    }

    fn inspect(&self, mut visit: BlockVisitor)
    {
        visit.set_name(Self::short_type_name());
        visit.annotate(&format!("👂 {:?}", self.condition));
        visit.visit_pulses("On True", &self.on_true_outlet);
        visit.visit_pulses("On False", &self.on_false_outlet);
        visit.visit_latches("True", &self.true_outlet);
        visit.visit_latches("False", &self.false_outlet);
        visit.visit_latches("Powered", &self.powered_outlet);
    }
}

// stack var
// pulser
// wait
// timeout

#[cfg(test)]
mod tests
{
}