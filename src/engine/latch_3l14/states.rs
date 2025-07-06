use super::{LatchingOutlet, PulsedOutlet, Scope, StateBlock, StateOutletVisitor};

// A no-op, always-active after activate state
pub struct Latch
{
    // on_power?
    powered_outlet: LatchingOutlet,
}
impl StateBlock for Latch
{
    fn power_on(&self, scope: &mut Scope) { }

    fn power_off(&self, scope: &mut Scope) { }

    fn visit_powered_outlets(&self, mut visitor: StateOutletVisitor)
    {
        visitor.visit_latching(&self.powered_outlet);
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
impl StateBlock for BoolSwitch
{
    fn power_on(&self, scope: &mut Scope)
    {
        /* TODO:
            dependency change enqueues power-off then power-on (if bool flipped)
         */
    }

    fn power_off(&self, scope: &mut Scope)
    {
    }

    fn visit_powered_outlets(&self, mut visitor: StateOutletVisitor)
    {
        if self.test
        {
            visitor.visit_pulsed(&self.on_true_outlet);
            visitor.visit_latching(&self.true_outlet);
            visitor.visit_latching(&self.powered_outlet);
        }
        else
        {
            visitor.visit_pulsed(&self.on_false_outlet);
            visitor.visit_latching(&self.false_outlet);
            visitor.visit_latching(&self.powered_outlet);
        }
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    pub fn test_latch()
    {

    }
}