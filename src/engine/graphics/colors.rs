use bitcode::{Decode, Encode};
use crate::engine::graphics::colors;

// todo: HDR support?
#[repr(packed)]
#[derive(Debug, Copy, Clone, PartialEq, Encode, Decode)]
pub struct Rgba
{
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}
impl Rgba
{
    pub fn new(r: u8, b: u8, g: u8, a: u8) -> Self { Self { r, g, b, a } }

    pub fn to_srgb(self) -> Self
    {
        let f = |xu: u8|
        {
            let x = xu as f32 / 255.0;
            if x > 0.04045
            {
                (((x + 0.055) / 1.055).powf(2.4) * 255.0) as u8
            }
            else
            {
                ((x / 12.92) * 255.0) as u8
            }
        };

        Rgba { r: f(self.r), g: f(self.g), b: f(self.b), a: self.a }
    }
}
impl Default for Rgba
{
    fn default() -> Self { colors::WHITE } 
}
impl From<u32> for Rgba
{
    fn from(rgba: u32) -> Self
    {
        Rgba
        {
            r: ((rgba >> 24) & 0xff) as u8,
            g: ((rgba >> 16) & 0xff) as u8,
            b: ((rgba >> 8) & 0xff) as u8,
            a: (rgba & 0xff) as u8,
        }
    }
}
impl From<Rgba> for u32
{
    fn from(color: Rgba) -> Self
    {
        ((color.r as u32) << 24) +
        ((color.g as u32) << 16) +
        ((color.b as u32) << 8) +
        (color.a as u32)
    }
}
impl From<[u8;4]> for Rgba
{
    fn from(rgba: [u8;4]) -> Self
    {
        Rgba
        {
            r: rgba[0],
            g: rgba[1],
            b: rgba[2],
            a: rgba[3],
        }
    }
}
impl From<Rgba> for [u8;4]
{
    fn from(color: Rgba) -> Self
    {
        [ color.r, color.g, color.b, color.a ]
    }
}
impl From<[f32;4]> for Rgba
{
    fn from(rgba: [f32;4]) -> Self
    {
        Rgba
        {
            r: (rgba[0] * 255.0) as u8,
            g: (rgba[1] * 255.0) as u8,
            b: (rgba[2] * 255.0) as u8,
            a: (rgba[3] * 255.0) as u8,
        }
    }
}
impl From<Rgba> for [f32;4]
{
    fn from(color: Rgba) -> Self
    {
        [
            (color.r as f32) / 255.0,
            (color.g as f32) / 255.0,
            (color.b as f32) / 255.0,
            (color.a as f32) / 255.0,
        ]
    }
}
impl From<wgpu::Color> for Rgba
{
    fn from(color: wgpu::Color) -> Self
    {
        Rgba
        {
            r: (color.r * 255.0) as u8,
            g: (color.g * 255.0) as u8,
            b: (color.b * 255.0) as u8,
            a: (color.a * 255.0) as u8,
        }
    }
}
impl From<Rgba> for wgpu::Color
{
    fn from(color: Rgba) -> Self
    {
        Self
        {
            r: (color.r as f64) / 255.0,
            g: (color.g as f64) / 255.0,
            b: (color.b as f64) / 255.0,
            a: (color.a as f64) / 255.0,
        }
    }
}

pub const TRANSPARENT_BLACK: Rgba = Rgba { r: 0, g: 0, b: 0, a: 0 };
pub const BLACK: Rgba = Rgba { r: 0, g: 0, b: 0, a: 255 };
pub const WHITE: Rgba = Rgba { r: 255, g: 255, b: 255, a: 255 };
pub const GRAY: Rgba = Rgba { r: 144, g: 144, b: 144, a: 255 };
pub const CORNFLOWER_BLUE: Rgba = Rgba { r: 100, g: 149, b: 237, a: 255 };
pub const GOOD_PURPLE: Rgba = Rgba { r: 64, g: 72, b: 255, a: 255 };
pub const BAD_RED: Rgba = Rgba { r: 102, g: 6, b: 32, a: 255 };