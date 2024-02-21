use glam::{Mat4, Vec3};

pub const WORLD_RIGHT: Vec3 = Vec3 { x: 1.0, y: 0.0, z: 0.0 };
pub const WORLD_UP: Vec3 = Vec3 { x: 0.0, y: 1.0, z: 0.0 };
pub const WORLD_FORWARD: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 1.0 }; // into screen

pub struct Camera
{
    position: Vec3,
    forward: Vec3,
    up: Vec3,
}

#[repr(packed)]
pub struct CameraUniform
{
    pub proj_view: Mat4,
}
