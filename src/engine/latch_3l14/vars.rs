use std::fmt::{Debug, Formatter};
use super::{BlockId, InstRunId, ContextfulLatchBlock};
use smallvec::SmallVec;
use nab_3l14::utils::alloc_slice::alloc_slice_default;
use crate::instance::LatchContextStorage;
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
    // TODO: remove?
    #[inline] #[must_use]
    pub fn new(id: u32, scope: VarScope) -> Self
    {
        Self((id as u32) | (scope as u32) << (u32::BITS - 1))
    }

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

#[derive(Debug, Default, PartialEq, Clone)]
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
    // use glam types?
    Vec2 { x: f32, y: f32 },
    Vec3 { x: f32, y: f32, z: f32 },
    Vec4 { x: f32, y: f32, z: f32, w: f32 },

    List(Vec<VarValue>), // TODO: this needs to not be copied
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
impl Debug for LocalScope
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        let mut dbg = f.debug_struct("LocalScope");
        for (i, var) in self.vars.iter().enumerate()
        {
            dbg.field(&format!("{}", i), var);
        }
        dbg.finish()
    }
}

#[derive(Default)]
pub struct SharedScope
{
    // TODO
}

#[derive(Debug)]
pub struct VarChange
{
    pub var: VarId,
    pub target: BlockRef,
    pub old_value: VarValue,
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

    pub(super) latch_context: *mut LatchContextStorage, // pointer to pointer, dirty dirty hax
}
impl<'s> Scope<'s>
{
    pub fn run_id(&self) -> InstRunId { self.run_id }
    pub fn block_id(&self) -> BlockId { self.block_id }
    pub fn get_block_ref(&self) -> BlockRef { BlockRef(self.run_id, self.block_id) }

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

                // TODO: actually make sure variable has changed

                let old_value = std::mem::replace(&mut var.value, value.clone());

                for listener in var.listeners.iter()
                {
                    self.local_changes.push(VarChange
                    {
                        var: var_id,
                        target: listener.clone(),
                        old_value: old_value.clone(),
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

    // Wake-up the calling block whenever this var changes. Returns the current value of the var
    pub fn subscribe(&mut self, var_id: VarId) -> VarValue
    {
        // statically restrict this?
        debug_assert!(self.block_id.is_latch());

        match var_id.scope()
        {
            VarScope::Local =>
            {
                let var = self.local_scope.vars.get_mut(var_id.value() as usize).expect("Invalid var ID");
                var.listeners.push(BlockRef(self.run_id, self.block_id));
                // assert unique?
                log::trace!("{:?} subscribed to {var:?}", self.block_id);
                var.value.clone()
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
                if let Some(idx) = var.listeners.iter().position(|l| *l == BlockRef(self.run_id, self.block_id))
                {
                    var.listeners.remove(idx);
                    log::trace!("{:?} unsubscribed from {var:?}", self.block_id);
                }
                // assert unique?
            }
            VarScope::Shared =>
            {
                todo!()
            }
        }
    }

    // Get runtime data (internally used by tracked latch blocks)
    #[inline] #[must_use]
    pub(super) fn unpack_context<L: ContextfulLatchBlock>(self) -> (&'s mut L::Context, Scope<'s>)
    {
        let Self
        {
            run_id,
            block_id,
            local_scope,
            local_changes,
            shared_scope,
            shared_changes,
            latch_context
        } = self;

        // TODO: method to call to create context?
        unsafe
        {
            if (*latch_context).is_none()
            {
                let outbox = Box::new(L::Context::default());
                *latch_context = Some(outbox);
            }
        }

        let unboxed =
        unsafe {
            let deref = &mut *latch_context;
            let inbox = deref.as_mut().unwrap();
            &mut *(inbox.as_mut() as *mut _ as *mut L::Context)
        };

        (
            unboxed,
            Self
            {
                run_id,
                block_id,
                local_scope,
                local_changes,
                shared_scope,
                shared_changes,
                latch_context: std::ptr::null_mut(), // dirty hax, but this should not be used again
            }
        )
    }
}


#[cfg(test)]
mod tests
{
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