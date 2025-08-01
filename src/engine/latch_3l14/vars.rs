use std::collections::HashMap;
use super::{BlockId, InstRunId};
use smallvec::SmallVec;

#[repr(u8)]
pub enum VarScope
{
    Local = 0,
    Shared = 1,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct VarId(u32); // two 2 bits define scope?
impl VarId
{
    #[inline] #[must_use]
    pub fn scope(self) -> VarScope
    {
        unsafe { std::mem::transmute((self.0 >> (u32::BITS - 1)) as u8) }
    }
}

#[derive(PartialEq, Clone)]
pub struct Var
{
    // provider (name, inputs, get_val())
    // value
    // listeners (block refs)
    pub value: VarValue,
    pub listeners: SmallVec<[(InstRunId, BlockId); 2]>,
}

#[derive(Default, Debug, PartialEq, Clone)]
pub enum VarValue
{
    #[default]
    Null,
    Bool(bool),
    Int(i32),
    Float(f32),
    // Vec2, Vec3, Vec4
    // Entity
    // Asset?
    // Array
    // Map
}

#[derive(Default)]
pub struct LocalScope
{
    vars: Box<[VarValue]>,
    // var ID->u32 mapping
    // stacked vars
}

#[derive(Default)]
pub struct SharedScope
{
    // TODO
}

pub(crate) struct VarChange
{
    var: VarId,
    // todo save out ref to vars directly?
    new_value: VarValue,
}

pub struct Scope<'s>
{
    local_scope: &'s mut LocalScope,
    shared_scope: &'s SharedScope,

    changes: SmallVec<[VarChange; 2]>,
}
impl Scope<'_>
{
    pub fn get(&self, var_id: VarId) -> Option<VarValue>
    {
        let val = match var_id.scope()
        {
            VarScope::Local =>
            {
                self.local_scope.vars.get(&var_id.0)
            }
            VarScope::Shared =>
            {
                todo!()
            }
        };

        val.map(|v| v.value.clone())
    }

    pub fn set(&mut self, var_id: VarId, value: VarValue)
    {
        match var_id.scope()
        {
            VarScope::Local =>
            {
                let var = self.local_scope.vars.entry(var_id.0)
                    .or_default();
                var.value = value;

                for (inst, listener) in var.listeners
                {

                }
            }
            VarScope::Shared =>
            {
                todo!()
            }
        }
    }
}


#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
     fn set_get()
    {
    }
}