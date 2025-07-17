use std::collections::HashMap;
use super::{BlockId, InstRunId};

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct VarId(u32); // two 2 bits define scope?

#[derive(Debug, PartialEq, Clone, Hash)]
pub struct Var
{
    // provider (name, inputs, get_val())
    // value
    // listeners (block refs)
    pub listeners: Box<[(InstRunId, BlockId)]>,
}

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

}

#[derive(Default)]
pub struct SharedScope
{
    // TODO
}