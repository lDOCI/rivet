//! Pixel layouts found in Linux framebuffers, as Slint render targets.

use slint::platform::software_renderer::{PremultipliedRgbaColor, TargetPixel};

/// Blend a premultiplied source over one 8-bit destination channel.
#[inline]
fn over(dst: u8, src: u8, inv_alpha: u16) -> u8 {
    ((dst as u16 * inv_alpha / 255) as u8).saturating_add(src)
}

/// 32 bpp, memory order B, G, R, X — `XRGB8888` on little-endian (most common).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Bgrx8888 {
    pub b: u8,
    pub g: u8,
    pub r: u8,
    pub x: u8,
}

impl TargetPixel for Bgrx8888 {
    fn blend(&mut self, c: PremultipliedRgbaColor) {
        let a = (u8::MAX - c.alpha) as u16;
        self.r = over(self.r, c.red, a);
        self.g = over(self.g, c.green, a);
        self.b = over(self.b, c.blue, a);
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            b,
            g,
            r,
            x: u8::MAX,
        }
    }
}

/// 32 bpp, memory order R, G, B, X — `XBGR8888` on little-endian.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Rgbx8888 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub x: u8,
}

impl TargetPixel for Rgbx8888 {
    fn blend(&mut self, c: PremultipliedRgbaColor) {
        let a = (u8::MAX - c.alpha) as u16;
        self.r = over(self.r, c.red, a);
        self.g = over(self.g, c.green, a);
        self.b = over(self.b, c.blue, a);
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r,
            g,
            b,
            x: u8::MAX,
        }
    }
}

/// Framebuffer layouts we can draw into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Bgrx8888,
    Rgbx8888,
    Rgb565,
}

impl PixelFormat {
    /// Detect from `fb_var_screeninfo` bits per pixel and channel offsets.
    pub fn detect(bits_per_pixel: u32, red_offset: u32, blue_offset: u32) -> Option<Self> {
        match (bits_per_pixel, red_offset, blue_offset) {
            (32, 16, 0) => Some(Self::Bgrx8888),
            (32, 0, 16) => Some(Self::Rgbx8888),
            (16, 11, 0) => Some(Self::Rgb565),
            _ => None,
        }
    }

    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Rgb565 => 2,
            Self::Bgrx8888 | Self::Rgbx8888 => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_formats() {
        assert_eq!(PixelFormat::detect(32, 16, 0), Some(PixelFormat::Bgrx8888));
        assert_eq!(PixelFormat::detect(32, 0, 16), Some(PixelFormat::Rgbx8888));
        assert_eq!(PixelFormat::detect(16, 11, 0), Some(PixelFormat::Rgb565));
        assert_eq!(PixelFormat::detect(24, 16, 0), None);
    }

    #[test]
    fn bgrx_memory_layout() {
        let p = Bgrx8888::from_rgb(1, 2, 3);
        let bytes: [u8; 4] = unsafe { std::mem::transmute(p) };
        assert_eq!(bytes, [3, 2, 1, 255]);
    }

    #[test]
    fn blend_half_white_over_black() {
        let mut p = Bgrx8888::from_rgb(0, 0, 0);
        p.blend(PremultipliedRgbaColor {
            red: 128,
            green: 128,
            blue: 128,
            alpha: 128,
        });
        assert_eq!((p.r, p.g, p.b), (128, 128, 128));
    }

    #[test]
    fn opaque_blend_replaces() {
        let mut p = Rgbx8888::from_rgb(10, 20, 30);
        p.blend(PremultipliedRgbaColor {
            red: 200,
            green: 100,
            blue: 50,
            alpha: 255,
        });
        assert_eq!((p.r, p.g, p.b), (200, 100, 50));
    }
}
