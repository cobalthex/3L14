use nab_3l14::utils::ShortTypeName;
use crate::vars::VarChange;
use super::{LatchingOutlet, PulsedOutlet, Scope, LatchBlock, BlockVisitor, LatchActions, VarValue, VarId};

// A no-op, always-active after power-on latch
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

pub struct ConditionLatch
{
    pub condition: VarId, // TODO: expression

    pub on_true_outlet: PulsedOutlet,
    pub true_outlet: LatchingOutlet,

    pub on_false_outlet: PulsedOutlet,
    pub false_outlet: LatchingOutlet,

    pub powered_outlet: LatchingOutlet,
}
impl LatchBlock for ConditionLatch
{
    fn power_on(&self, mut scope: Scope, mut actions: LatchActions)
    {
        scope.subscribe(self.condition);

        /* TODO:
            dependency change enqueues power-off then power-on (if bool flipped)
         */
        if scope.get(self.condition).unwrap_or(VarValue::Bool(false)) == VarValue::Bool(true)
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

    fn power_off(&self, _scope: Scope)
    {
    }

    fn on_var_changed(&self, change: VarChange, scope: Scope, mut actions: LatchActions)
    {
        // TODO: evaluate condition
        // TODO: use runtime data here instead of old/new

        match change.new_value
        {
            VarValue::Bool(new) =>
            {
                let old = match change.old_value
                {
                    VarValue::Bool(v) => v,
                    _ => false,
                };
                if old == new
                {
                    return;
                }

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
        visit.visit_pulses("On True", &self.on_true_outlet);
        visit.visit_pulses("On True", &self.on_false_outlet);
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
    use super::*;

}