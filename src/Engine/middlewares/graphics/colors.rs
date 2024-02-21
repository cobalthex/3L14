// todo: HDR support?
#[repr(packed)]
#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct Color
{
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}
impl Color
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

        Color { r: f(self.r), g: f(self.g), b: f(self.b), a: self.a }
    }
}

impl From<u32> for Color
{
    fn from(rgba: u32) -> Self
    {
        Color
        {
            r: ((rgba >> 24) & 0xff) as u8,
            g: ((rgba >> 16) & 0xff) as u8,
            b: ((rgba >> 8) & 0xff) as u8,
            a: (rgba & 0xff) as u8,
        }
    }
}
impl From<Color> for u32
{
    fn from(color: Color) -> Self
    {
        ((color.r as u32) << 24) +
        ((color.g as u32) << 16) +
        ((color.b as u32) << 8) +
        (color.a as u32)
    }
}

impl From<[f32;4]> for Color
{
    fn from(rgba: [f32;4]) -> Self
    {
        Color
        {
            r: (rgba[0] * 255.0) as u8,
            g: (rgba[1] * 255.0) as u8,
            b: (rgba[2] * 255.0) as u8,
            a: (rgba[3] * 255.0) as u8,
        }
    }
}
impl From<Color> for [f32;4]
{
    fn from(color: Color) -> Self
    {
        [
            (color.r as f32) / 255.0,
            (color.g as f32) / 255.0,
            (color.b as f32) / 255.0,
            (color.a as f32) / 255.0,
        ]
    }
}
impl From<wgpu::Color> for Color
{
    fn from(color: wgpu::Color) -> Self
    {
        Color
        {
            r: (color.r * 255.0) as u8,
            g: (color.g * 255.0) as u8,
            b: (color.b * 255.0) as u8,
            a: (color.a * 255.0) as u8,
        }
    }
}
impl From<Color> for wgpu::Color
{
    fn from(color: Color) -> Self
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


pub const TRANSPARENT_BLACK: Color = Color { r: 0, g: 0, b: 0, a: 0 };
pub const BLACK: Color = Color { r: 0, g: 0, b: 0, a: 255 };
pub const WHITE: Color = Color { r: 255, g: 255, b: 255, a: 255 };
pub const CORNFLOWER_BLUE: Color = Color { r: 100, g: 149, b: 237, a: 255 };
pub const GOOD_PURPLE: Color = Color { r: 64, g: 72, b: 255, a: 255 };
