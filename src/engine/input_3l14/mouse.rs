use std::fmt::{Debug, Formatter};
use std::time::Instant;
use glam::IVec2;
use nab_3l14::ToggleState;
use nab_3l14::utils::NoOpDebug;
use super::{ButtonState, InputReader};

pub type MouseButton = sdl2::mouse::MouseButton;

#[derive(Debug, Default, Copy, Clone)]
pub struct MouseButtonState
{
    pub state: ButtonState,
    pub set_time: Option<Instant>,
}

const MAX_MOUSE_BUTTON_STATES: usize = 5;

#[derive(Debug)]
pub struct MouseState
{
    pub position: IVec2,
    pub position_delta: IVec2,
    pub wheel: IVec2,
    pub wheel_delta: IVec2,

    sdl_mouse: NoOpDebug<sdl2::mouse::MouseUtil>,
    pub(super) buttons: [MouseButtonState; MAX_MOUSE_BUTTON_STATES], // L, M, R, X1, X2
}
impl MouseState
{
    pub(super) fn new(sdl_mouse_util: sdl2::mouse::MouseUtil) -> Self
    {
        Self
        {
            position: IVec2::default(),
            position_delta: IVec2::default(),
            wheel: IVec2::default(),
            wheel_delta: IVec2::default(),
            sdl_mouse: NoOpDebug(sdl_mouse_util),
            buttons: [MouseButtonState::default(); MAX_MOUSE_BUTTON_STATES],
        }
    }

    // is this faster than updating this in pre-update?
    pub fn get_button(&self, button: MouseButton) -> MouseButtonState
    {
        self.buttons[(button as usize) - 1]
    }

    pub fn set_capture(&self, state: ToggleState)
    {
        // todo: capture stack?

        let new_state = match state
        {
            ToggleState::Off => false,
            ToggleState::On => true,
            ToggleState::Toggle => !self.is_captured()
        };
        self.sdl_mouse.0.show_cursor(!new_state);
        self.sdl_mouse.0.set_relative_mouse_mode(new_state);
    }
    pub fn is_captured(&self) -> bool
    {
        self.sdl_mouse.0.relative_mouse_mode()
    }
}
impl InputReader for MouseState
{
    fn pre_update(&mut self)
    {
        self.position_delta = IVec2::ZERO;
        self.wheel_delta = IVec2::ZERO;

        for button in self.buttons.iter_mut()
        {
            match button.state
            {
                ButtonState::JustOn =>
                {
                    button.state = ButtonState::On;
                }
                ButtonState::JustOff =>
                {
                    button.state = ButtonState::Off;
                    button.set_time = None;
                }

                _ => {}
            }
        }
    }
}