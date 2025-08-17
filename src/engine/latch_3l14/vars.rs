use std::fmt::{Debug, Formatter};
use super::{BlockId, InstRunId, Instance};
use smallvec::SmallVec;

#[repr(u8)]
pub enum VarScope
{
    Local = 0,
    Shared = 1,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct VarId(u32);
impl VarId
{
    #[inline] #[must_use]
    pub fn scope(self) -> VarScope
    {
        unsafe { std::mem::transmute((self.0 >> (u32::BITS - 1)) as u8) }
    }
}
impl Debug for VarId
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self.scope()
        {
            VarScope::Local => f.write_fmt(format_args!("{{Local|{}}}", self.0)),
            VarScope::Shared => f.write_fmt(format_args!("{{Shared|{}}}", self.0)),
        }
    }
}

pub(super) type VarListener = (InstRunId, BlockId);

#[derive(PartialEq, Clone)]
pub struct Var
{
    // provider (name, inputs, get_val())
    // value
    // listeners (block refs)
    pub value: VarValue,
    pub(super) listeners: SmallVec<[VarListener; 2]>,
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

pub(super) type ScopeChanges = SmallVec<[VarChange; 4]>;

#[derive(Default)]
pub struct LocalScope
{
    vars: Box<[Var]>,
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
    pub var: VarId,
    pub target: VarListener,
    pub new_value: VarValue,
}


pub struct Scope<'s>
{
    pub(super) local_scope: &'s mut LocalScope,
    pub(super) local_changes: &'s mut ScopeChanges,
    pub(super) shared_scope: &'s SharedScope,
    pub(super) shared_changes: &'s mut ScopeChanges,
}
impl<'s> Scope<'s>
{
    pub fn get(&self, var_id: VarId) -> Option<VarValue>
    {
        let val = match var_id.scope()
        {
            VarScope::Local =>
            {
                self.local_scope.vars.get(var_id.0 as usize)
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
                let var = &mut self.local_scope.vars[var_id.0 as usize];
                var.value = value.clone();

                for listener in var.listeners.iter()
                {
                    self.local_changes.push(VarChange
                    {
                        var: var_id,
                        target: listener.clone(),
                        new_value: value.clone(),
                    });
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
