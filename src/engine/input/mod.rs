use std::fmt::{Debug, Formatter, Write};
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not, Sub, SubAssign};
use std::slice::Iter;
use std::time::Instant;
use egui::{Pos2, RawInput, Ui};
use glam::IVec2;
use sdl2::event::Event;
use sdl2::keyboard::Mod;
use crate::engine::ToggleState;

pub type KeyCode = sdl2::keyboard::Keycode;
pub type ScanCode = sdl2::keyboard::Scancode;
pub type MouseButton = sdl2::mouse::MouseButton;

// todo: access control

#[derive(Debug)]
pub struct Input
{
    mouse: MouseState,
    keyboard: KeyboardState,
}

impl Input
{
    pub fn new(sdl: &sdl2::Sdl) -> Self
    {
        Self
        {
            mouse: MouseState::new(sdl.mouse()),
            keyboard: KeyboardState::default(),
        }
    }

    pub fn mouse(&self) -> &MouseState { &self.mouse }
    pub fn keyboard(&self) -> &KeyboardState { &self.keyboard }

    pub fn pre_update(&mut self)
    {
        puffin::profile_function!();
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
                    if self.keyboard.get_key(key).is_none()
                    {
                        self.keyboard.pressed_keys.push(KeyState
                        {
                            key_code: key,
                            scan_code: scancode.unwrap_or(unsafe { std::mem::transmute(0) }),
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
                        if let Some(keystate) = self.keyboard.get_key_mut(key)
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
                self.mouse.position_delta = IVec2::new(xrel, yrel);
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
            }
            Event::MouseWheel { x, y, .. } =>
            {
                // precise x/y?
                self.mouse.wheel += IVec2::new(x, y);
                self.mouse.wheel_delta = IVec2::new(x, y);
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
impl From<&Input> for RawInput
{
    fn from(input: &Input) -> Self
    {
        let mut ri = Self::default();
        ri.modifiers.ctrl = input.keyboard.has_keymod(KeyMods::CTRL);
        ri.modifiers.shift = input.keyboard.has_keymod(KeyMods::SHIFT);
        ri.modifiers.alt = input.keyboard.has_keymod(KeyMods::ALT);
        // todo: iterate keys

        // todo: this should be scaled by zoom apparently
        let mouse_pos = Pos2
        {
            x: input.mouse.position.x as f32,
            y: input.mouse.position.y as f32,
        };

        ri.events.push(egui::Event::PointerMoved(mouse_pos));

        ri.events.push(egui::Event::MouseWheel
        {
            delta: egui::Vec2
            {
                x: input.mouse.wheel.x as f32,
                y: input.mouse.wheel.y as f32
            },
            unit: egui::MouseWheelUnit::Point,
            modifiers: ri.modifiers,
        });

        for i in 0..input.mouse.buttons.len()
        {
            let pressed = match input.mouse.buttons[i].state
            {
                ButtonState::JustOn|ButtonState::On => true,
                ButtonState::JustOff => false,
                ButtonState::Off => continue,
            };

            ri.events.push(egui::Event::PointerButton
            {
                pos: mouse_pos,
                button: match i
                {
                    0 => egui::PointerButton::Primary,
                    1 => egui::PointerButton::Middle,
                    2 => egui::PointerButton::Secondary,
                    3 => egui::PointerButton::Extra1,
                    4 => egui::PointerButton::Extra2,
                    _ => panic!("Unknown pointer button")
                },
                pressed,
                modifiers: ri.modifiers,
            })
        }

        // todo: keyboard events
        // todo: other events

        ri
    }
}

impl<'n> super::graphics::debug_gui::DebugGui<'n> for Input
{
    fn name(&self) -> &'n str { "Input state" }
    fn debug_gui(&self, ui: &mut Ui)
    {
        ui.horizontal_top(|hui|
        {
            hui.collapsing("Keyboard", |kbui|
            {
                kbui.set_min_width(120.0);
                kbui.label(format!("Mods: {:?}", self.keyboard.mods));
                let mut any = false;
                for state in self.keyboard.pressed_keys.iter()
                {
                    any = true;
                    kbui.label(format!("{:?}: {:?}", state.key_code, state.state));
                }
                if !any
                {
                    kbui.label("(No keys pressed)");
                }
            });

            hui.collapsing("Mouse", |mui|
            {
                mui.set_min_width(200.0);
                mui.label(format!("Pos: {:?} - Delta: {:?}", self.mouse.position.to_array(), self.mouse.position_delta.to_array()));
                mui.label(format!("Wheel: {:?} - Delta: {:?}", self.mouse.wheel.to_array(), self.mouse.wheel_delta.to_array()));
                mui.label(format!("LB: {:?}", self.mouse.get_button(MouseButton::Left).state));
                mui.label(format!("MB: {:?}", self.mouse.get_button(MouseButton::Middle).state));
                mui.label(format!("RB: {:?}", self.mouse.get_button(MouseButton::Right).state));
                mui.label(format!("X1: {:?}", self.mouse.get_button(MouseButton::X1).state));
                mui.label(format!("X2: {:?}", self.mouse.get_button(MouseButton::X2).state));
            });
        });
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
impl From<KeyMods> for bool
{
    fn from(km: KeyMods) -> Self { km.0 != 0 }
}
impl Debug for KeyMods
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        let mut any = false;
        if (*self & Self::CTRL).into()
        {
            f.write_str("CTRL")?;
            any = true;
        }
        if (*self & Self::SHIFT).into()
        {
            if any { f.write_char('|')?; }
            f.write_str("SHIFT")?;
            any = true;
        }
        if (*self & Self::ALT).into()
        {
            if any { f.write_char('|')?; }
            f.write_str("ALT")?;
            any = true;
        }
        if !any
        {
            f.write_str("NONE")?;
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

    fn get_key_mut(&mut self, key_code: KeyCode) -> Option<&mut KeyState>
    {
        self.pressed_keys.iter_mut().find(|p| p.key_code == key_code)
    }
    pub fn get_key(&self, key_code: KeyCode) -> Option<&KeyState>
    {
        self.pressed_keys.iter().find(|p| p.key_code == key_code)
    }
    pub fn is_down(&self, key_code: KeyCode) -> bool
    {
        matches!(self.get_key(key_code), Some(KeyState { state: ButtonState::On | ButtonState::JustOn, .. }))
    }
    pub fn is_press(&self, key_code: KeyCode) -> bool
    {
        matches!(self.get_key(key_code), Some(KeyState { state: ButtonState::JustOn, .. }))
    }
    pub fn is_click(&self, key_code: KeyCode) -> bool
    {
        matches!(self.get_key(key_code), Some(KeyState { state: ButtonState::JustOff, .. }))
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

    pub fn is_on(&self) -> bool
    {
        match *self
        {
            ButtonState::Off => false,
            ButtonState::JustOff => false,
            ButtonState::JustOn => true,
            ButtonState::On => true,
        }
    }
}

#[derive(Debug, Default, Copy, Clone)]
pub struct MouseButtonState
{
    pub state: ButtonState,
    pub set_time: Option<Instant>,
}

const MAX_MOUSE_BUTTON_STATES: usize = 5;

struct SdlMouseUtil(sdl2::mouse::MouseUtil);
impl Debug for SdlMouseUtil
{
    fn fmt(&self, _f: &mut Formatter<'_>) -> std::fmt::Result { Ok(()) }
}

#[derive(Debug)]
pub struct MouseState
{
    pub position: IVec2,
    pub position_delta: IVec2,
    pub wheel: IVec2,
    pub wheel_delta: IVec2,

    sdl_mouse: SdlMouseUtil,
    buttons: [MouseButtonState; MAX_MOUSE_BUTTON_STATES], // L, M, R, X1, X2
}
impl MouseState
{
    fn new(sdl_mouse_util: sdl2::mouse::MouseUtil) -> Self
    {
        Self
        {
            position: IVec2::default(),
            position_delta: IVec2::default(),
            wheel: IVec2::default(),
            wheel_delta: IVec2::default(),
            sdl_mouse: SdlMouseUtil(sdl_mouse_util),
            buttons: [MouseButtonState::default(); MAX_MOUSE_BUTTON_STATES],
        }
    }

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
