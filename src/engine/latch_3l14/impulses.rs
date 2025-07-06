use log::log;
use super::{ImpulseBlock, ImpulseOutletVisitor, PulsedOutlet, Scope, VarId, VarValue};

struct NoOp
{
    pub outlet: PulsedOutlet,
}
impl ImpulseBlock for NoOp
{
    fn pulse(&self, scope: &mut Scope) { }

    fn visit_outlets(&self, mut visitor: ImpulseOutletVisitor)
    {
        visitor.visit_pulsed(&self.outlet);
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
    fn pulse(&self, scope: &mut Scope)
    {
        log::debug!("{}", self.message);
    }

    fn visit_outlets(&self, mut visitor: ImpulseOutletVisitor)
    {
        visitor.visit_pulsed(&self.outlet);
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
    fn pulse(&self, scope: &mut Scope)
    {
        // TODO
    }

    fn visit_outlets(&self, mut visitor: ImpulseOutletVisitor)
    {
        visitor.visit_pulsed(&self.outlet);
    }
}

