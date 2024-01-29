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
                let button = &mut self.mouse.buttons[(mouse_btn as usize) - 1];
                button.state = ButtonState::JustOn;
                button.set_time = Some(time);
            }
            Event::MouseButtonUp { mouse_btn, .. } =>
            {
                let button = &mut self.mouse.buttons[(mouse_btn as usize) - 1];
                button.state = ButtonState::JustOff;
                button.set_time = None;
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

#[derive(Debug, Default, Copy, Clone)]
pub struct MouseButtonState
{
    pub state: ButtonState,
    pub set_time: Option<Instant>,
}

#[derive(Debug, Default)]
pub struct MouseState
{
    pub position: IVec2,
    pub delta: IVec2,
    pub wheel: IVec2,

    buttons: [MouseButtonState; 5], // L, M, R, X1, X2
}
impl MouseState
{
    fn pre_update(&mut self)
    {
        self.delta = IVec2::ZERO;

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
                }

                _ => {}
            }
        }
    }

    // is this faster than updating this in pre-update?
    pub fn get_button_down(&self, button: MouseButton) -> Option<MouseButtonState>
    {
        todo!()
    }
}