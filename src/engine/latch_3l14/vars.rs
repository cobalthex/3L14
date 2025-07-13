use std::collections::HashMap;
use super::BlockId;

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct VarId(usize);

#[derive(Debug, PartialEq, Clone, Hash)]
pub struct Var
{
    // provider (name, inputs, get_val())
    // value
    // listeners (block refs)
    pub listeners: Box<[BlockId]>,
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
