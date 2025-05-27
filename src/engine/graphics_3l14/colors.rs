use std::ops::BitXor;
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use crate::colors;

// todo: HDR support?
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct Rgba
{
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}
impl Rgba
{
    #[inline] #[must_use] pub const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self { Self { red, green, blue, alpha } }
    #[inline] #[must_use] pub const fn new_f32(red: f32, green: f32, blue: f32, alpha: f32) -> Self
    {
        Self { red: (red * 255.0) as u8, green: (green * 255.0) as u8, blue: (blue * 255.0) as u8, alpha: (alpha * 255.0) as u8 }
    }
    #[inline] #[must_use] pub const fn gray(lightness: u8, a: u8) -> Self { Self { red: lightness, green: lightness, blue: lightness, alpha: a } }
    #[inline] #[must_use] pub const fn gray_f32(lightness: f32, a: f32) -> Self { let lu = (lightness * 255.0) as u8; Self { red: lu, green: lu, blue: lu, alpha: (a * 255.0) as u8 } }

    #[inline] #[must_use]
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

        Rgba { red: f(self.red), green: f(self.green), blue: f(self.blue), alpha: self.alpha }
    }

    #[inline] #[must_use]
    pub fn to_bgra(self) -> Self
    {
        Self::new(self.blue, self.green, self.red, self.alpha)
    }

    // Calculate RGBa from HSLa. hue should be in degrees from 0-360, saturation and lightness from 0-1
    // may not work if outside of these ranges
    #[must_use]
    pub fn from_hsla(hue: f32, saturation: f32, lightness: f32, alpha: f32) -> Self
    {
        // https://en.wikipedia.org/wiki/HSL_and_HSV#HSL_to_RGB_alternative
        let q = saturation * lightness.max(1.0 - lightness); // a
        let f = |n|
        {
            let k = (n + hue / 30.0) % 12.0;
            let t1 = f32::min(k - 3.0, 9.0 - k);
            let t2 = f32::min(t1, 1.0);
            let t = f32::max(t2, -1.0);
            lightness - q * t
        };

        Rgba::new_f32(f(0.0), f(8.0), f(4.0), alpha)
    }

    #[must_use]
    pub fn from_hsva(hue: f32, saturation: f32, value: f32, alpha: f32) -> Self
    {
        let f = |n|
        {
            let k = (n + hue / 60.0) % 6.0;
            let t1 = f32::min(k, 4.0 - k);
            let t2 = f32::min(t1, 1.0);
            value - (value * saturation * f32::max(0.0, t2))
        };

        Rgba::new_f32(f(5.0), f(3.0), f(1.0), alpha)
    }

    // rgb <-> yuv
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
            red: ((rgba >> 0) & 0xff) as u8,
            green: ((rgba >> 8) & 0xff) as u8,
            blue: ((rgba >> 16) & 0xff) as u8,
            alpha: ((rgba >> 24) & 0xff) as u8,
        }
    }
}
impl From<Rgba> for u32
{
    fn from(color: Rgba) -> Self
    {
        ((color.red as u32) << 0) +
        ((color.green as u32) << 8) +
        ((color.blue as u32) << 16) +
        ((color.alpha as u32) << 24)
    }
}
impl From<[u8;4]> for Rgba
{
    fn from(rgba: [u8;4]) -> Self
    {
        Rgba
        {
            red: rgba[0],
            green: rgba[1],
            blue: rgba[2],
            alpha: rgba[3],
        }
    }
}
impl From<Rgba> for [u8;4]
{
    fn from(color: Rgba) -> Self
    {
        [ color.red, color.green, color.blue, color.alpha]
    }
}
impl From<[f32;4]> for Rgba
{
    fn from(rgba: [f32;4]) -> Self
    {
        Rgba
        {
            red: (rgba[0] * 255.0) as u8,
            green: (rgba[1] * 255.0) as u8,
            blue: (rgba[2] * 255.0) as u8,
            alpha: (rgba[3] * 255.0) as u8,
        }
    }
}
impl From<Rgba> for [f32;4]
{
    fn from(color: Rgba) -> Self
    {
        [
            (color.red as f32) / 255.0,
            (color.green as f32) / 255.0,
            (color.blue as f32) / 255.0,
            (color.alpha as f32) / 255.0,
        ]
    }
}
impl From<wgpu::Color> for Rgba
{
    fn from(color: wgpu::Color) -> Self
    {
        Rgba
        {
            red: (color.r * 255.0) as u8,
            green: (color.g * 255.0) as u8,
            blue: (color.b * 255.0) as u8,
            alpha: (color.a * 255.0) as u8,
        }
    }
}
impl From<Rgba> for wgpu::Color
{
    fn from(color: Rgba) -> Self
    {
        Self
        {
            r: (color.red as f64) / 255.0,
            g: (color.green as f64) / 255.0,
            b: (color.blue as f64) / 255.0,
            a: (color.alpha as f64) / 255.0,
        }
    }
}
impl BitXor for Rgba
{
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self::Output
    {
        Self
        {
            red: self.red ^ rhs.red,
            green: self.green ^ rhs.green,
            blue: self.blue ^ rhs.blue,
            alpha: self.alpha ^ rhs.alpha,
        }
    }
}

pub const TRANSPARENT_BLACK: Rgba = Rgba { red: 0, green: 0, blue: 0, alpha: 0 };
pub const TRANSPARENT_WHITE: Rgba = Rgba { red: 255, green: 255, blue: 255, alpha: 0 };
pub const BLACK: Rgba = Rgba { red: 0, green: 0, blue: 0, alpha: 255 };
pub const WHITE: Rgba = Rgba { red: 255, green: 255, blue: 255, alpha: 255 };
pub const GRAY: Rgba = Rgba { red: 144, green: 144, blue: 144, alpha: 255 };
pub const RED: Rgba = Rgba { red: 255, green: 0, blue: 0, alpha: 255 };
pub const YELLOW: Rgba = Rgba { red: 255, green: 255, blue: 0, alpha: 255 };
pub const GREEN: Rgba = Rgba { red: 0, green: 255, blue: 0, alpha: 255 };
pub const CYAN: Rgba = Rgba { red: 0, green: 255, blue: 255, alpha: 255 };
pub const BLUE: Rgba = Rgba { red: 0, green: 0, blue: 255, alpha: 255 };
pub const MAGENTA: Rgba = Rgba { red: 255, green: 0, blue: 255, alpha: 255 };
pub const CORNFLOWER_BLUE: Rgba = Rgba { red: 100, green: 149, blue: 237, alpha: 255 };
pub const GOOD_PURPLE: Rgba = Rgba { red: 64, green: 72, blue: 255, alpha: 255 };
pub const BAD_RED: Rgba = Rgba { red: 102, green: 6, blue: 32, alpha: 255 };
pub const TOMATO: Rgba = Rgba { red: 255, green: 99, blue: 71, alpha: 255 };
pub const TURQUOISE: Rgba = Rgba { red: 64, green: 224, blue: 208, alpha: 255 };
pub const CHARTREUSE: Rgba = Rgba { red: 127, green: 255, blue: 0, alpha: 255 };
pub const ORANGE: Rgba = Rgba { red: 255, green: 165, blue: 0, alpha: 255 };
pub const LIME: Rgba = Rgba { red: 200, green: 255, blue: 0, alpha: 255 };