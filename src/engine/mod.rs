pub mod core_types;
pub use core_types::*;

pub mod state_logic;
pub use state_logic::*;

pub mod math;
pub use math::*;

pub mod utils;
pub use utils::*;

pub mod asset;
// pub mod assets2;
pub mod graphics;
pub mod input;
pub mod timing;
pub mod windows;
pub mod world;
mod runtime;
// rust's module system sucks
