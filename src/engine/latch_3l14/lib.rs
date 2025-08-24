use std::collections::HashMap;
use std::fmt::Debug;
use std::u32::MAX;
use smallvec::{smallvec, SmallVec};

mod circuit;
pub use circuit::*;

mod instance;
pub use instance::*;

mod runtime;
pub use runtime::*;

mod vars;
pub use vars::*;

pub mod latches;
pub mod impulses;
