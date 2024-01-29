use std::fmt::{Debug, Formatter, Write};
use std::intrinsics::transmute;
use std::mem::MaybeUninit;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not, Sub, SubAssign};
use std::slice::Iter;
use std::time::Instant;
use chrono::format::Colons::Maybe;
use glam::{BVec2, IVec2, Vec2};
use sdl2::{Sdl, event::Event};
use sdl2::keyboard::Mod;
use sdl2::mouse::MouseUtil;

pub type KeyCode = sdl2::keyboard::Keycode;
pub type ScanCode = sdl2::keyboard::Scancode;
pub type MouseButton = sdl2::mouse::MouseButton;

// todo: access control

#[derive(Debug, Default)]
pub struct Input
{
    mouse: MouseState,
    keyboard: KeyboardState,
}

impl Input
{
    pub fn mouse(&self) -> &MouseState { &self.mouse }
    pub fn keyboard(&self) -> &KeyboardState { &self.keyboard }

    pub fn pre_update(&mut self)
    {
        self.keyboard.pre_update();
        self.mouse.pre_update();
    }

    pub fn handle_event(&mut self, event: Event, time: Instant)
    {
        match event
        {
            Event::KeyDown { keycode, scancode, keymod, .. } =>
            {
                if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD)
                {
                    self.keyboard.mods |= KeyMods::CTRL;
                }
                if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD)
                {
                    self.keyboard.mods |= KeyMods::SHIFT;
                }
                if keymod.intersects(Mod::LALTMOD | Mod::RALTMOD)
                {
                    self.keyboard.mods |= KeyMods::ALT;
                }

                if let Some(key) = keycode
                {
                    if self.keyboard.get_key_down(key).is_none()
                    {
                        self.keyboard.pressed_keys.push(KeyState
                        {
                            key_code: key,
                            scan_code: scancode.unwrap_or(unsafe { transmute(0) }),
                            state: ButtonState::JustOn,
                            set_time: time,
                        });
                    }
                }
            }
            Event::KeyUp { keycode, keymod, .. } =>
            {
                if keymod.intersection(Mod::LCTRLMOD | Mod::RCTRLMOD).is_empty()
                {
                    self.keyboard.mods -= KeyMods::CTRL;
                }
                if keymod.intersection(Mod::LSHIFTMOD | Mod::RSHIFTMOD).is_empty()
                {
                    self.keyboard.mods -= KeyMods::SHIFT;
                }
                if keymod.intersection(Mod::LALTMOD | Mod::RALTMOD).is_empty()
                {
                    self.keyboard.mods -= KeyMods::ALT;
                }

                match keycode
                {
                    Some(key) =>
                    {
                        if let Some(keystate) = self.keyboard.get_key_down_mut(key)
                        {
                            keystate.state = ButtonState::JustOff
                        }
                    }
                    None => panic!("invalid on->off state transition"), // todo
                }
            }

            // Event::TextEditing { .. } => {}
            // Event::TextInput { .. } => {}

            Event::MouseMotion { x, y, xrel, yrel, .. } =>
            {
                self.mouse.position = IVec2::new(x, y);
                self.mouse.delta = IVec2::new(xrel, yrel);
            }
            Event::MouseButtonDown { mouse_btn, .. } => // double click?
            {
                self.mouse.curr_buttons.0 |= MouseButtons::from(mouse_btn).0;
                self.mouse.button_set_times[mouse_btn as usize - 1] = MaybeUninit::new(time);
            }
            Event::MouseButtonUp { mouse_btn, .. } =>
            {
                self.mouse.curr_buttons.0 &= !MouseButtons::from(mouse_btn).0;
            }
            Event::MouseWheel { x, y, .. } =>
            {
                self.mouse.wheel += IVec2::new(x, y);
            }

            // Event::JoyAxisMotion { .. } => {}
            // Event::JoyBallMotion { .. } => {}
            // Event::JoyHatMotion { .. } => {}
            // Event::JoyButtonDown { .. } => {}
            // Event::JoyButtonUp { .. } => {}
            // Event::JoyDeviceAdded { .. } => {}
            // Event::JoyDeviceRemoved { .. } => {}
            //
            // Event::ControllerAxisMotion { .. } => {}
            // Event::ControllerButtonDown { .. } => {}
            // Event::ControllerButtonUp { .. } => {}
            // Event::ControllerDeviceAdded { .. } => {}
            // Event::ControllerDeviceRemoved { .. } => {}
            // Event::ControllerDeviceRemapped { .. } => {}
            // Event::ControllerSensorUpdated { .. } => {}
            //
            // Event::FingerDown { .. } => {}
            // Event::FingerUp { .. } => {}
            // Event::FingerMotion { .. } => {}
            //
            // Event::DollarGesture { .. } => {}
            // Event::DollarRecord { .. } => {}
            // Event::MultiGesture { .. } => {}

            _ => {}
        }
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub enum ButtonState
{
    #[default]
    Off,
    JustOn, // off->on this frame
    JustOff, // on->off this frame
    On,
    // repeat?
}
impl ButtonState
{
    pub fn set(&mut self, is_on: bool)
    {
        *self = match *self
        {
            ButtonState::Off if is_on => ButtonState::JustOn,
            ButtonState::JustOn if is_on => ButtonState::On,
            ButtonState::JustOn if !is_on => ButtonState::JustOff,
            ButtonState::JustOff if !is_on => ButtonState::Off,
            ButtonState::JustOff if is_on => ButtonState::JustOn,
            ButtonState::On if !is_on => ButtonState::JustOff,
            _ => panic!("Unsupported state transition from {:?} towards {:?}", *self, if is_on { ButtonState::On } else { ButtonState::Off }),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct KeyState
{
    pub key_code: KeyCode,
    pub scan_code: ScanCode,
    pub state: ButtonState,
    pub set_time: Instant,
}

#[derive(PartialEq, Eq, Clone, Copy, Hash)]
pub struct KeyMods(u8);
impl KeyMods
{
    pub const NONE:  KeyMods = KeyMods(0b0000);
    pub const CTRL:  KeyMods = KeyMods(0b0001);
    pub const SHIFT: KeyMods = KeyMods(0b0010);
    pub const ALT:   KeyMods = KeyMods(0b0100);
}
impl Default for KeyMods
{
    fn default() -> Self { Self::NONE }
}
impl BitOr for KeyMods
{
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output { Self(self.0 | rhs.0) }
}
impl BitOrAssign for KeyMods
{
    fn bitor_assign(&mut self, rhs: Self) { *self = *self | rhs; }
}
impl BitAnd for KeyMods
{
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output { Self(self.0 & rhs.0) }
}
impl BitAndAssign for KeyMods
{
    fn bitand_assign(&mut self, rhs: Self) { *self = *self & rhs; }
}
impl Not for KeyMods
{
    type Output = Self;
    fn not(self) -> Self::Output { Self(!self.0) }
}
impl Sub for KeyMods
{
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output { Self(self.0 & (!rhs.0)) }
}
impl SubAssign for KeyMods
{
    fn sub_assign(&mut self, rhs: Self) { *self = *self - rhs; }
}
impl Into<bool> for KeyMods
{
    fn into(self) -> bool { self.0 != 0 }
}
impl Debug for KeyMods
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        let mut any = false;
        if (*self & Self::CTRL).into()
        {
            let _ = f.write_str("CTRL");
            any = true;
        }
        if (*self & Self::SHIFT).into()
        {
            if any { f.write_char('|'); }
            let _ = f.write_str("SHIFT");
            any = true;
        }
        if (*self & Self::ALT).into()
        {
            if any { f.write_char('|'); }
            let _ = f.write_str("ALT");
            any = true;
        }
        if !any
        {
            f.write_str("NONE");
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct KeyboardState
{
    mods: KeyMods,
    pressed_keys: Vec<KeyState>,
}
impl KeyboardState
{
    fn pre_update(&mut self)
    {
        self.pressed_keys.retain_mut(|k|
        {
            match k.state
            {
                ButtonState::JustOn =>
                    {
                        k.state = ButtonState::On;
                        true
                    },
                ButtonState::On => true,

                ButtonState::Off => panic!("Key in off state but still marked as pressed"),
                ButtonState::JustOff => false
            }
        });
    }

    pub fn iter_pressed_keys(&self) -> Iter<KeyState>
    {
        self.pressed_keys.iter()
    }

    fn get_key_down_mut(&mut self, key_code: KeyCode) -> Option<&mut KeyState>
    {
        self.pressed_keys.iter_mut().find(|p| p.key_code == key_code)
    }
    pub fn get_key_down(&self, key_code: KeyCode) -> Option<&KeyState>
    {
        self.pressed_keys.iter().find(|p| p.key_code == key_code)
    }

    pub fn get_keymods(&self) -> KeyMods { self.mods }
    pub fn has_keymod(&self, mods: KeyMods) -> bool { (self.mods & mods).into() }
}
impl Default for KeyboardState
{
    fn default() -> Self
    {
        Self
        {
            pressed_keys: Vec::with_capacity(8),
            mods: KeyMods::NONE,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct MouseButtonState
{
    pub state: ButtonState,
    pub set_time: Instant,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct MouseButtons(u8);
impl MouseButtons
{
    pub const NONE:   MouseButtons = MouseButtons(0b00000);
    pub const LEFT:   MouseButtons = MouseButtons(0b00001);
    pub const MIDDLE: MouseButtons = MouseButtons(0b00010);
    pub const RIGHT:  MouseButtons = MouseButtons(0b00100);
    pub const X1:     MouseButtons = MouseButtons(0b01000);
    pub const X2:     MouseButtons = MouseButtons(0b10000);

    // todo: make/use a proper flags macro?

    // todo: custom debug
}
impl Default for MouseButtons
{
    fn default() -> Self { Self::NONE }
}
impl From<MouseButton> for MouseButtons
{
    fn from(button: MouseButton) -> Self
    {
        match button
        {
            MouseButton::Left => Self::LEFT,
            MouseButton::Middle => Self::MIDDLE,
            MouseButton::Right => Self::RIGHT,
            MouseButton::X1 => Self::X1,
            MouseButton::X2 => Self::X2,

            _ => Self::NONE,
        }
    }
}
impl Debug for MouseButtons
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        let mut any = false;
        if (self.0 & Self::LEFT.0) != 0
        {
            let _ = f.write_str("LEFT");
            any = true;
        }
        if (self.0 & Self::MIDDLE.0) != 0
        {
            if any { f.write_char('|'); }
            let _ = f.write_str("MIDDLE");
            any = true;
        }
        if (self.0 & Self::RIGHT.0) != 0
        {
            if any { f.write_char('|'); }
            let _ = f.write_str("RIGHT");
            any = true;
        }
        if (self.0 & Self::X1.0) != 0
        {
            if any { f.write_char('|'); }
            let _ = f.write_str("X1");
            any = true;
        }
        if (self.0 & Self::X2.0) != 0
        {
            if any { f.write_char('|'); }
            let _ = f.write_str("X2");
            any = true;
        }
        if !any
        {
            f.write_str("NONE");
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct MouseState
{
    pub position: IVec2,
    pub delta: IVec2,
    pub wheel: IVec2,

    prev_buttons: MouseButtons,
    curr_buttons: MouseButtons,
    button_set_times: [MaybeUninit<Instant>; 5], // can probably use maybe unused
}
impl MouseState
{
    fn pre_update(&mut self)
    {
        self.delta = IVec2::ZERO;
        self.prev_buttons = self.curr_buttons;
    }

    // is this faster than updating this in pre-update?
    pub fn get_button_down(&self, button: MouseButton) -> Option<MouseButtonState>
    {
        let mapped: MouseButtons = button.into();
        let was_pressed = (self.prev_buttons.0 & mapped.0) != 0;
        let is_pressed = (self.curr_buttons.0 & mapped.0) != 0;

        if !was_pressed && !is_pressed
        {
            return None;
        }

        let state: ButtonState;
        if was_pressed && is_pressed { state = ButtonState::On }
        else if !was_pressed { state = ButtonState::JustOn }
        else if !is_pressed { state = ButtonState::JustOff }
        else { todo!() }

        Some(MouseButtonState
        {
            state,
            set_time: unsafe { self.button_set_times[(button as usize) - 1].assume_init() },
        })
    }
}
impl Default for MouseState
{
    fn default() -> Self
    {
        Self
        {
            position: Default::default(),
            delta: Default::default(),
            wheel: Default::default(),
            prev_buttons: Default::default(),
            curr_buttons: Default::default(),
            button_set_times: [MaybeUninit::zeroed(); 5],
        }
    }
}