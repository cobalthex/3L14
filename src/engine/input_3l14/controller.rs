use std::fmt::{Debug, Formatter};
use glam::Vec2;
use nab_3l14::utils::NoOpDebug;
use crate::{ButtonState, InputReader};

#[derive(Debug, Default)]
pub struct ControllerState
{
    connected: bool,
    buttons: u32,

    l_thumb: Vec2,
    r_thumb: Vec2,

    l_trigger: f32,
    r_trigger: f32,
}
impl ControllerState
{
}
impl InputReader for ControllerState
{
    fn pre_update(&mut self)
    {

    }
}