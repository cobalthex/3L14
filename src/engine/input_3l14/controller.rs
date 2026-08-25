use std::fmt::Debug;
use glam::Vec2;
use crate::InputReader;

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