use glam::{Vec3};

pub mod camera;
pub use camera::*;
pub mod transform;
pub use transform::*;

pub const WORLD_RIGHT: Vec3 = Vec3 { x: 1.0, y: 0.0, z: 0.0 };
pub const WORLD_UP: Vec3 = Vec3 { x: 0.0, y: 1.0, z: 0.0 };
pub const WORLD_FORWARD: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 1.0 }; // into screen
