use std::fmt::{Debug, Formatter};
use super::{BlockId, InstRunId, Instance};
use smallvec::SmallVec;
use nab_3l14::utils::alloc_slice::alloc_slice_default;
use crate::runtime::BlockRef;

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
    #[cfg(test)]
    #[inline] #[must_use]
    pub fn test(id: u8, scope: VarScope) -> Self
    {
        Self((id as u32) | (scope as u32) << (u32::BITS - 1))
    }

    #[inline] #[must_use]
    pub fn scope(self) -> VarScope
    {
        unsafe { std::mem::transmute((self.0 >> (u32::BITS - 1)) as u8) }
    }

    #[inline] #[must_use]
    fn value(self) -> u32
    {
        self.0 & ((1 << (u32::BITS - 1)) - 1)
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

#[derive(Default, PartialEq, Clone)]
pub struct Var
{
    // TODO: should vars be fixed types? (would be tricky w/ shared vars)
    pub value: VarValue,
    pub(super) listeners: SmallVec<[BlockRef; 2]>,
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

pub struct LocalScope
{
    vars: Box<[Var]>,
    // var ID->u32 mapping
    // stacked vars
}
impl LocalScope
{
    #[inline] #[must_use]
    pub(super) fn new(count: u32) -> Self
    {
        Self
        {
            vars: alloc_slice_default(count as usize)
        }
    }
}

#[derive(Default)]
pub struct SharedScope
{
    // TODO
}

pub struct VarChange
{
    pub var: VarId,
    pub target: BlockRef,
    pub new_value: VarValue,
}

pub struct Scope<'s>
{
    pub(super) run_id: InstRunId,
    pub(super) block_id: BlockId,

    pub(super) local_scope: &'s mut LocalScope,
    pub(super) local_changes: &'s mut ScopeChanges,
    pub(super) shared_scope: &'s SharedScope,
    pub(super) shared_changes: &'s mut ScopeChanges,
}
impl<'s> Scope<'s>
{
    #[must_use]
    pub fn get(&self, var_id: VarId) -> Option<VarValue>
    {
        let val = match var_id.scope()
        {
            VarScope::Local =>
            {
                self.local_scope.vars.get(var_id.value() as usize)
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

    // TODO: automate sub/unsub in latches?

    pub fn subscribe(&mut self, var_id: VarId)
    {
        // statically restrict this?
        debug_assert!(self.block_id.is_latch());

        match var_id.scope()
        {
            VarScope::Local =>
            {
                let var = self.local_scope.vars.get_mut(var_id.value() as usize).expect("Invalid var ID");
                var.listeners.push((self.run_id, self.block_id));
                // assert unique?
            }
            VarScope::Shared =>
            {
                todo!()
            }
        }
    }

    pub fn unsubscribe(&mut self, var_id: VarId)
    {
        debug_assert!(self.block_id.is_latch());

        match var_id.scope()
        {
            VarScope::Local =>
            {
                let var = self.local_scope.vars.get_mut(var_id.value() as usize).expect("Invalid var ID");
                if let Some(idx) = var.listeners.iter().position(|l| *l == (self.run_id, self.block_id))
                {
                    var.listeners.remove(idx);
                }
                // assert unique?
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


/* TODO

- code-backed var values:
  - code can push changes (live values)
- expressions

 */