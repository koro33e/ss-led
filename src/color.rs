/// Defines the pixel type and its byte order on the wire.
///
/// Implementations must have `CHANNELS > 0` and handle every index in
/// `0..CHANNELS`. The encoder only calls `wire_byte` in that range.
pub trait ColorFormat: Copy {
    type Pixel;
    const CHANNELS: usize;
    fn wire_byte(&self, pixel: &Self::Pixel, index: usize) -> u8;
}

/// 3-channel RGB color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct RGB {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RGB {
    pub const BLACK: Self = Self::new(0, 0, 0);
    pub const RED: Self = Self::new(255, 0, 0);
    pub const GREEN: Self = Self::new(0, 255, 0);
    pub const BLUE: Self = Self::new(0, 0, 255);
    pub const WHITE: Self = Self::new(255, 255, 255);
    pub const YELLOW: Self = Self::new(255, 255, 0);
    pub const CYAN: Self = Self::new(0, 255, 255);
    pub const MAGENTA: Self = Self::new(255, 0, 255);

    #[inline]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Scale brightness by `factor` (0 = off, 255 = full).
    #[inline]
    pub const fn dim(self, factor: u8) -> Self {
        Self {
            r: (self.r as u16 * factor as u16 / 255) as u8,
            g: (self.g as u16 * factor as u16 / 255) as u8,
            b: (self.b as u16 * factor as u16 / 255) as u8,
        }
    }
}

impl From<[u8; 3]> for RGB {
    #[inline]
    fn from([r, g, b]: [u8; 3]) -> Self {
        Self { r, g, b }
    }
}

impl From<(u8, u8, u8)> for RGB {
    #[inline]
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Self { r, g, b }
    }
}

/// 4-channel RGBW color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct RGBW {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub w: u8,
}

impl RGBW {
    #[inline]
    pub const fn new(r: u8, g: u8, b: u8, w: u8) -> Self {
        Self { r, g, b, w }
    }
}

/// An RGB wire channel order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct RgbOrder([usize; 3]);
impl RgbOrder {
    pub const RGB: Self = Self([0, 1, 2]);
    pub const RBG: Self = Self([0, 2, 1]);
    pub const GRB: Self = Self([1, 0, 2]);
    pub const GBR: Self = Self([1, 2, 0]);
    pub const BRG: Self = Self([2, 0, 1]);
    pub const BGR: Self = Self([2, 1, 0]);
}
impl ColorFormat for RgbOrder {
    type Pixel = RGB;
    const CHANNELS: usize = 3;
    fn wire_byte(&self, p: &RGB, index: usize) -> u8 {
        [p.r, p.g, p.b][self.0[index]]
    }
}
/// An RGBW wire channel order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct RgbwOrder([usize; 4]);
impl RgbwOrder {
    pub const RGBW: Self = Self([0, 1, 2, 3]);
    pub const GRBW: Self = Self([1, 0, 2, 3]);
}
impl ColorFormat for RgbwOrder {
    type Pixel = RGBW;
    const CHANNELS: usize = 4;
    fn wire_byte(&self, p: &RGBW, index: usize) -> u8 {
        [p.r, p.g, p.b, p.w][self.0[index]]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rgb_wire_orders() {
        let p = RGB::new(0x12, 0x34, 0x56);
        for (order, expected) in [
            (RgbOrder::RGB, [0x12, 0x34, 0x56]),
            (RgbOrder::RBG, [0x12, 0x56, 0x34]),
            (RgbOrder::GRB, [0x34, 0x12, 0x56]),
            (RgbOrder::GBR, [0x34, 0x56, 0x12]),
            (RgbOrder::BRG, [0x56, 0x12, 0x34]),
            (RgbOrder::BGR, [0x56, 0x34, 0x12]),
        ] {
            assert_eq!(
                core::array::from_fn::<_, 3, _>(|i| order.wire_byte(&p, i)),
                expected
            );
        }
    }

    #[test]
    fn rgbw_wire_orders() {
        let p = RGBW::new(1, 2, 3, 4);
        for (order, expected) in [
            (RgbwOrder::RGBW, [1, 2, 3, 4]),
            (RgbwOrder::GRBW, [2, 1, 3, 4]),
        ] {
            assert_eq!(
                core::array::from_fn::<_, 4, _>(|i| order.wire_byte(&p, i)),
                expected
            );
        }
    }

    #[test]
    fn brightness_scales_all_channels_without_overflow() {
        let color = RGB::new(255, 128, 1);
        assert_eq!(color.dim(0), RGB::BLACK);
        assert_eq!(color.dim(255), color);
        assert_eq!(color.dim(128), RGB::new(128, 64, 0));
        for factor in 0..=255 {
            assert_eq!(RGB::WHITE.dim(factor), RGB::new(factor, factor, factor));
        }
    }

    #[test]
    fn conversions_and_defaults() {
        assert_eq!(RGB::from([1, 2, 3]), RGB::new(1, 2, 3));
        assert_eq!(RGB::from((1, 2, 3)), RGB::new(1, 2, 3));
        assert_eq!(RGB::default(), RGB::BLACK);
        assert_eq!(RGBW::default(), RGBW::new(0, 0, 0, 0));
    }
}
