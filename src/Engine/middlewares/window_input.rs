#![warn(dead_code)]

use winit::event::{VirtualKeyCode, ModifiersState};

use crate::CoreTypes::TickCount;

const MAX_KEYCODE_ENTRIES: usize = 256;
const MAX_MOUSE_BUTTON_ENTRIES: usize = 5; /* TODO: don't hardcode */

#[derive(Default, Copy, Clone, PartialEq)]
pub enum InputState
{
    #[default]
    Off,
    JustOn, // off->on this frame
    JustOff, // on->off this frame
    On,
    // repeat?
}

#[derive(Copy, Clone)]
pub struct KeyState
{
    pub state: InputState,
    pub last_set_time: TickCount,
}
impl Default for KeyState
{
    fn default() -> Self {
        Self
        {
            state: InputState::Off,
            last_set_time: TickCount(0), // use time?
        }
    }
}

#[derive(Default, Copy, Clone)]
pub struct MouseButtonState
{
    pub state: InputState,
    pub last_set_time: TickCount,
}

#[derive(Copy, Clone, PartialEq)]
pub enum MouseButton
{
    Left    = 0,
    Middle  = 1,
    Right   = 2,
    X1      = 3,
    X2      = 4,
}

#[derive(Default, Copy, Clone)]
pub struct MouseState
{
    pub position: winit::dpi::PhysicalPosition<f32>,
    pub move_delta: winit::dpi::PhysicalPosition<f32>,

    pub buttons: [MouseButtonState; MAX_MOUSE_BUTTON_ENTRIES],
    pub wheel_delta: f32,
}
impl MouseState
{
    pub fn button(&self, button: MouseButton) -> MouseButtonState
    {
        self.buttons[button as usize]
    }
    pub fn is_button_down(&self, button: MouseButton) -> bool
    {
        match self.buttons[button as usize].state
        {
            InputState::JustOn|InputState::On => true,
            _ => false,
        }
    }
    pub fn is_button_up(&self, button: MouseButton) -> bool
    {
        match self.buttons[button as usize].state
        {
            InputState::JustOff|InputState::Off => true,
            _ => false,
        }
    }
}

pub struct KeyboardState
{
    pub keys: [KeyState; MAX_KEYCODE_ENTRIES],
    pub modifiers: ModifiersState,
}
impl KeyboardState
{
    pub fn key(&self, key_code: VirtualKeyCode) -> KeyState { self.keys[key_code as usize] }
    pub fn is_key_down(&self, key_code: VirtualKeyCode) -> bool
    {
        match self.keys[key_code as usize].state
        {
            InputState::JustOn|InputState::On => true,
            _ => false,
        }
    }
    pub fn is_key_up(&self, key_code: VirtualKeyCode) -> bool
    {
        match self.keys[key_code as usize].state
        {
            InputState::JustOff|InputState::Off => true,
            _ => false,
        }
    }
    pub fn set_key(&mut self, key_code: VirtualKeyCode, is_pressed: bool, )
    {
        self.keys[key_code as usize].state = match self.keys[key_code as usize].state
        {
            InputState::Off if is_pressed => InputState::JustOn,
            InputState::JustOn if is_pressed => InputState::On,
            InputState::JustOn if !is_pressed => InputState::JustOff,
            InputState::JustOff if !is_pressed => InputState::Off,
            InputState::JustOff if is_pressed => InputState::JustOn,
            InputState::On if !is_pressed => InputState::JustOff,
            no_change => no_change,
        }
    }
}
impl Default for KeyboardState
{
    fn default() -> Self { Self
    {
        keys: [KeyState::default(); MAX_KEYCODE_ENTRIES],
        modifiers: ModifiersState::empty(),
    }}
}

#[derive(Default)]
pub struct WindowInputState
{
    pub mouse_state: MouseState,
    pub keyboard_state: KeyboardState,
}