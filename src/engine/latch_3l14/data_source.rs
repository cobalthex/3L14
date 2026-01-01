use crate::VarValue;

pub trait DataSource
{
     fn get(&self) -> VarValue; // TODO: needs to be able to return containers
}

// TODO: how should these get constructed?
