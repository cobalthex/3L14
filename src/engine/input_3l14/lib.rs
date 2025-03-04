mod button;

mod controller;
mod keyboard;
mod mouse;

mod input;

pub use button::*;
pub use controller::*;
pub use keyboard::*;
pub use mouse::*;

pub use input::*;

trait InputReader
{
    fn pre_update(&mut self);
}