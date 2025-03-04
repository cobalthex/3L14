use std::fmt::{Debug, Formatter, Write};
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not, Sub, SubAssign};
use std::slice::Iter;
use std::time::Instant;
use crate::{ButtonState, InputReader};

pub type KeyCode = sdl2::keyboard::Keycode;
pub type ScanCode = sdl2::keyboard::Scancode;

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
    pub(super) mods: KeyMods,
    pub(super) pressed_keys: Vec<KeyState>,
}
impl KeyboardState
{
    pub fn iter_pressed_keys(&self) -> Iter<KeyState>
    {
        self.pressed_keys.iter()
    }

    pub(super) fn get_key_mut(&mut self, key_code: KeyCode) -> Option<&mut KeyState>
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
impl InputReader for KeyboardState
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
}