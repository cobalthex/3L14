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