use bitcode::{Decode, Encode};
use glam::Vec3;
use math_3l14::Angle;
use serde::{Deserialize, Serialize};

#[derive(Debug, Encode, Decode, Serialize, Deserialize)]
pub enum Light
{
    Point(Vec3),
    Directional(Vec3),
    Spot
    {
        angle: Angle,
        range: f32,
    },
    // rect/disc area lights
}
