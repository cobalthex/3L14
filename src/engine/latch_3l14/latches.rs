use crate::vars::VarChange;
use super::{LatchingOutlet, PulsedOutlet, Scope, LatchBlock, LatchOutletVisitor, OnVarChangedResult, Inlet};

// A no-op, always-active after power-on latch
pub struct Latch
{
    // on_power?
    powered_outlet: LatchingOutlet,
}
impl LatchBlock for Latch
{
    fn power_on(&self, _scope: Scope, mut pulse_outlets: LatchOutletVisitor)
    {
        pulse_outlets.visit_latching(&self.powered_outlet, Inlet::Pulse);
    }
    fn power_off(&self, _scope: Scope) { }
    fn on_var_changed(&self, _change: VarChange, _scope: Scope , _pulse_outlets: LatchOutletVisitor) -> OnVarChangedResult
    {
        OnVarChangedResult::NoChange
    }
}

pub struct BoolSwitch
{
    pub test: bool, // TODO: expression

    on_true_outlet: PulsedOutlet,
    true_outlet: LatchingOutlet,

    on_false_outlet: PulsedOutlet,
    false_outlet: LatchingOutlet,

    powered_outlet: LatchingOutlet,
}
impl LatchBlock for BoolSwitch
{
    fn power_on(&self, _scope: Scope, mut pulse_outlets: LatchOutletVisitor)
    {
        /* TODO:
            dependency change enqueues power-off then power-on (if bool flipped)
         */
        if self.test
        {
            pulse_outlets.visit_pulsed(&self.on_true_outlet);
            pulse_outlets.visit_latching(&self.true_outlet, Inlet::Pulse);
            pulse_outlets.visit_latching(&self.powered_outlet, Inlet::Pulse);
        }
        else
        {
            pulse_outlets.visit_pulsed(&self.on_false_outlet);
            pulse_outlets.visit_latching(&self.false_outlet, Inlet::Pulse);
            pulse_outlets.visit_latching(&self.powered_outlet, Inlet::Pulse);
        }
    }

    fn power_off(&self, _scope: Scope)
    {
    }


    fn on_var_changed(&self, change: VarChange, scope: Scope, pulse_outlets: LatchOutletVisitor) -> OnVarChangedResult
    {
        todo!();
        OnVarChangedResult::NoChange
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