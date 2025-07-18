use std::collections::HashMap;
use super::{BlockId, InstRunId};

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct VarId(u32); // two 2 bits define scope?

#[derive(PartialEq, Clone)]
pub struct Var
{
    // provider (name, inputs, get_val())
    // value
    // listeners (block refs)
    pub value: VarValue,
    pub listeners: Box<[(InstRunId, BlockId)]>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum VarValue
{
    Null, // remove?
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
    vars: HashMap<VarId, Var>,
    // stacked vars
}
impl LocalScope
{
    pub fn set_var(&mut self, var_id: VarId, var: VarValue)
    {
        let entry = self.vars.entry(var_id).or_insert(Var { value: VarValue::Null, listeners: Box::new([]) });
        entry.value = var;
        // TODO: notify listeners
    }

    #[inline] #[must_use]
    pub fn get_var(&self, var_id: VarId) -> Option<VarValue>
    {
        self.vars.get(&var_id).map(|v| v.value.clone())
    }
}

#[derive(Default)]
pub struct SharedScope
{
    // TODO
}

pub struct Scope
{
    // TODO: local + shared scopes
    // shared scope is arc (or just ref)?
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn set_get()
    {
        const VID_A: VarId = VarId(0);
        const VID_B: VarId = VarId(1);

        let mut l = LocalScope::default();
        l.set_var(VID_A, VarValue::Int(1));

        assert_eq!(l.get_var(VID_A), Some(VarValue::Int(1)));
    }
}