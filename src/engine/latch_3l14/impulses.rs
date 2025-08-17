use log::log;
use super::{ImpulseBlock, ImpulseOutletVisitor, PulsedOutlet, Scope, VarId, VarValue};

struct NoOp
{
    pub outlet: PulsedOutlet,
}
impl ImpulseBlock for NoOp
{
    fn pulse(&self, _scope: Scope, mut pulse_outlets: ImpulseOutletVisitor)
    {
        pulse_outlets.visit_pulsed(&self.outlet);
    }
}

struct DebugPrint
{
    pub message: String,
    // todo: format strings

    pub outlet: PulsedOutlet,
}
impl ImpulseBlock for DebugPrint
{
    fn pulse(&self, _scope: Scope, mut pulse_outlets: ImpulseOutletVisitor)
    {
        log::debug!("{}", self.message);
        pulse_outlets.visit_pulsed(&self.outlet);
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
    fn pulse(&self, _scope: Scope, mut visitor: ImpulseOutletVisitor)
    {
        // todo: set vars
        visitor.visit_pulsed(&self.outlet);
    }
}

