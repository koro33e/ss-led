use crate::ColorFormat;
#[cfg(any(feature = "esp-hal", test))]
use crate::Error;
#[cfg(any(feature = "esp-hal", test))]
use core::borrow::Borrow;

/// Pulse durations in nanoseconds. Custom timings must satisfy the LED's specification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Timing {
    pub t_0_high: u32,
    pub t_0_low: u32,
    pub t_1_high: u32,
    pub t_1_low: u32,
    pub t_reset: u32,
}
/// Default timing; the reset is 300 microseconds.
pub const WS2812B_TIMING: Timing = Timing {
    t_0_high: 400,
    t_0_low: 800,
    t_1_high: 850,
    t_1_low: 450,
    t_reset: 300_000,
};
/// Worldsemi WS2812B V5 timing. Select according to the actual LED revision.
pub const WS2812B_V5_TIMING: Timing = Timing {
    t_0_high: 300,
    t_0_low: 900,
    t_1_high: 650,
    t_1_low: 600,
    t_reset: 300_000,
};

/// Calculate the required number of pulse codes, returning None for invalid or overflowing sizes.
pub const fn checked_buffer_size<F: ColorFormat>(count: usize) -> Option<usize> {
    if F::CHANNELS == 0 {
        return None;
    }
    match F::CHANNELS.checked_mul(8) {
        Some(bits) => match count.checked_mul(bits) {
            Some(n) => n.checked_add(2),
            None => None,
        },
        None => None,
    }
}
/// Calculate the pulse buffer size. Panics if the format is invalid or the size overflows.
pub const fn buffer_size<F: ColorFormat>(count: usize) -> usize {
    match checked_buffer_size::<F>(count) {
        Some(n) => n,
        None => panic!("invalid pulse buffer size"),
    }
}

#[cfg(any(feature = "esp-hal", test))]
#[derive(Clone, Copy)]
// C::default() must be the transmission end marker.
pub(crate) struct Pulses<C> {
    zero: C,
    one: C,
    pub(crate) reset: C,
}
#[cfg(any(feature = "esp-hal", test))]
impl<C: Copy + Default> Pulses<C> {
    pub(crate) fn new(
        timing: Timing,
        frequency_hz: u32,
        code: impl Fn(bool, u16, bool, u16) -> C,
    ) -> Result<Self, Error> {
        // RMT stores each half-pulse length in 15 bits.
        const MAX_LEN: u64 = 0x7fff;
        let hz = u64::from(frequency_hz);
        let half = |ns: u32| -> Result<u16, Error> {
            let ticks = (u64::from(ns) * hz + 500_000_000) / 1_000_000_000;
            if ns == 0 || !(1..=MAX_LEN).contains(&ticks) {
                return Err(Error::InvalidTiming);
            }
            Ok(ticks as u16)
        };
        let zero = code(true, half(timing.t_0_high)?, false, half(timing.t_0_low)?);
        let one = code(true, half(timing.t_1_high)?, false, half(timing.t_1_low)?);
        let reset_ticks = (u64::from(timing.t_reset) * hz).div_ceil(1_000_000_000);
        if !(2..=2 * MAX_LEN).contains(&reset_ticks) {
            return Err(Error::InvalidTiming);
        }
        let a = reset_ticks / 2;
        let reset = code(false, a as u16, false, (reset_ticks - a) as u16);
        Ok(Self { zero, one, reset })
    }
    pub(crate) fn encode<F, I>(
        &self,
        format: F,
        pixels: I,
        buffer: &mut [C],
    ) -> Result<usize, Error>
    where
        F: ColorFormat,
        I: IntoIterator,
        I::Item: Borrow<F::Pixel>,
    {
        let capacity = buffer
            .len()
            .checked_sub(2)
            .ok_or(Error::BufferSizeExceeded)?;
        let bits = F::CHANNELS
            .checked_mul(8)
            .filter(|n| *n > 0)
            .ok_or(Error::BufferSizeExceeded)?;
        let mut idx: usize = 0;
        for pixel in pixels {
            let end = idx
                .checked_add(bits)
                .filter(|end| *end <= capacity)
                .ok_or(Error::BufferSizeExceeded)?;
            for channel in 0..F::CHANNELS {
                let byte = format.wire_byte(pixel.borrow(), channel);
                for bit in 0..8 {
                    buffer[idx] = if byte & (0x80 >> bit) != 0 {
                        self.one
                    } else {
                        self.zero
                    };
                    idx += 1;
                }
            }
            debug_assert_eq!(idx, end);
        }
        buffer[idx] = self.reset;
        buffer[idx + 1] = C::default();
        Ok(idx + 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RGB, RGBW, RgbOrder, RgbwOrder};

    // Keep pulse levels and durations observable without emulating the HAL.
    type Code = (bool, u16, bool, u16);

    fn pulses(timing: Timing, hz: u32) -> Result<Pulses<Code>, Error> {
        Pulses::new(timing, hz, |a, b, c, d| (a, b, c, d))
    }

    fn decode<const N: usize>(codes: &[Code], pulses: &Pulses<Code>) -> [u8; N] {
        assert_eq!(codes.len(), N * 8);
        core::array::from_fn(|i| {
            codes[i * 8..(i + 1) * 8].iter().fold(0, |byte, code| {
                assert!(*code == pulses.zero || *code == pulses.one);
                (byte << 1) | u8::from(*code == pulses.one)
            })
        })
    }

    #[derive(Clone, Copy)]
    struct Invalid<const CHANNELS: usize>;

    impl<const N: usize> ColorFormat for Invalid<N> {
        type Pixel = ();
        const CHANNELS: usize = N;

        fn wire_byte(&self, _: &(), _: usize) -> u8 {
            panic!("invalid formats must be rejected before reading pixels")
        }
    }

    #[test]
    fn buffer_sizes_include_reset_and_end_marker() {
        assert_eq!([0, 1, 2, 30].map(buffer_size::<RgbOrder>), [2, 26, 50, 722]);
        assert_eq!(
            [0, 1, 2, 30].map(buffer_size::<RgbwOrder>),
            [2, 34, 66, 962]
        );
    }

    #[test]
    fn invalid_and_overflowing_sizes_are_rejected() {
        assert_eq!(checked_buffer_size::<Invalid<0>>(0), None);
        assert_eq!(checked_buffer_size::<Invalid<{ usize::MAX }>>(0), None);
        assert_eq!(checked_buffer_size::<RgbOrder>(usize::MAX), None);
        assert_eq!(
            checked_buffer_size::<RgbOrder>((usize::MAX - 2) / 24 + 1),
            None
        );
        assert!(checked_buffer_size::<RgbOrder>((usize::MAX - 2) / 24).is_some());
    }

    #[test]
    #[should_panic(expected = "invalid pulse buffer size")]
    fn infallible_size_panics_on_overflow() {
        buffer_size::<RgbOrder>(usize::MAX);
    }

    #[test]
    fn default_and_v5_pulse_durations() {
        let p = pulses(WS2812B_TIMING, 80_000_000).unwrap();
        assert_eq!(p.zero, (true, 32, false, 64));
        assert_eq!(p.one, (true, 68, false, 36));
        assert_eq!(p.reset, (false, 12000, false, 12000));
        let p = pulses(WS2812B_V5_TIMING, 80_000_000).unwrap();
        assert_eq!(p.zero, (true, 24, false, 72));
        assert_eq!(p.one, (true, 52, false, 48));
    }

    #[test]
    fn data_rounds_to_nearest_tick_and_reset_rounds_up() {
        let timing = Timing {
            t_0_high: 18,
            t_0_low: 19,
            t_1_high: 31,
            t_1_low: 32,
            t_reset: 26,
        };
        let p = pulses(timing, 80_000_000).unwrap();
        assert_eq!(p.zero, (true, 1, false, 2));
        assert_eq!(p.one, (true, 2, false, 3));
        assert_eq!(p.reset, (false, 1, false, 2));
    }

    #[test]
    fn unrepresentable_timings_are_rejected() {
        assert!(matches!(
            pulses(WS2812B_TIMING, 0),
            Err(Error::InvalidTiming)
        ));
        for field in 0..5 {
            for invalid in [0, 1, u32::MAX] {
                let mut t = WS2812B_TIMING;
                match field {
                    0 => t.t_0_high = invalid,
                    1 => t.t_0_low = invalid,
                    2 => t.t_1_high = invalid,
                    3 => t.t_1_low = invalid,
                    _ => t.t_reset = invalid,
                }
                assert!(matches!(pulses(t, 80_000_000), Err(Error::InvalidTiming)));
            }
        }
    }

    #[test]
    fn reset_fits_both_fifteen_bit_halves() {
        let mut t = WS2812B_TIMING;
        t.t_reset = 819_175;
        assert_eq!(
            pulses(t, 80_000_000).unwrap().reset,
            (false, 32767, false, 32767)
        );
        t.t_reset += 1;
        assert!(matches!(pulses(t, 80_000_000), Err(Error::InvalidTiming)));
        t.t_reset = 13;
        assert_eq!(pulses(t, 80_000_000).unwrap().reset, (false, 1, false, 1));
        t.t_reset = 12;
        assert!(matches!(pulses(t, 80_000_000), Err(Error::InvalidTiming)));
    }

    #[test]
    fn encodes_every_byte_most_significant_bit_first() {
        let p = pulses(WS2812B_TIMING, 80_000_000).unwrap();
        let mut buffer = [Code::default(); buffer_size::<RgbOrder>(1)];
        for byte in 0..=255 {
            assert_eq!(
                p.encode(RgbOrder::RGB, [RGB::new(byte, byte, byte)], &mut buffer),
                Ok(26)
            );
            assert_eq!(decode::<3>(&buffer[..24], &p), [byte; 3]);
            assert_eq!(buffer[24], p.reset);
            assert_eq!(buffer[25], Code::default());
        }
    }

    #[test]
    fn encodes_borrowed_rgb_and_owned_rgbw_in_wire_order() {
        let p = pulses(WS2812B_TIMING, 80_000_000).unwrap();
        let frame = [RGB::new(0x80, 0x01, 0xa5), RGB::new(0x12, 0x34, 0x56)];
        let mut buffer = [Code::default(); buffer_size::<RgbOrder>(2)];
        assert_eq!(
            p.encode(RgbOrder::GRB, frame.as_slice(), &mut buffer),
            Ok(50)
        );
        assert_eq!(
            decode::<6>(&buffer[..48], &p),
            [0x01, 0x80, 0xa5, 0x34, 0x12, 0x56]
        );
        let mut buffer = [Code::default(); buffer_size::<RgbwOrder>(1)];
        assert_eq!(
            p.encode(RgbwOrder::GRBW, [RGBW::new(1, 2, 3, 4)], &mut buffer),
            Ok(34)
        );
        assert_eq!(decode::<4>(&buffer[..32], &p), [2, 1, 3, 4]);
        assert_eq!(buffer[32], p.reset);
        assert_eq!(buffer[33], Code::default());
    }

    #[test]
    fn empty_frame_only_latches_and_leaves_unused_buffer_untouched() {
        let p = pulses(WS2812B_TIMING, 80_000_000).unwrap();
        let sentinel = (true, 1, true, 1);
        let mut buffer = [sentinel; 4];
        assert_eq!(
            p.encode(RgbOrder::RGB, core::iter::empty::<RGB>(), &mut buffer),
            Ok(2)
        );
        assert_eq!(buffer, [p.reset, Code::default(), sentinel, sentinel]);
    }

    #[test]
    fn insufficient_capacity_and_invalid_formats_return_errors() {
        let p = pulses(WS2812B_TIMING, 80_000_000).unwrap();
        let mut buffer = [Code::default(); 50];
        for size in [0, 1, 2, 25] {
            assert_eq!(
                p.encode(RgbOrder::RGB, [RGB::RED], &mut buffer[..size]),
                Err(Error::BufferSizeExceeded)
            );
        }
        // Even an unbounded input must stop at the buffer boundary.
        assert_eq!(
            p.encode(RgbOrder::RGB, core::iter::repeat(RGB::RED), &mut buffer),
            Err(Error::BufferSizeExceeded)
        );
        assert_eq!(
            p.encode(Invalid::<0>, [()], &mut buffer),
            Err(Error::BufferSizeExceeded)
        );
        assert_eq!(
            p.encode(Invalid::<{ usize::MAX }>, [()], &mut buffer),
            Err(Error::BufferSizeExceeded)
        );
        // Reusing the buffer after an encoding error remains valid.
        assert_eq!(p.encode(RgbOrder::RGB, [RGB::BLUE; 2], &mut buffer), Ok(50));
    }
}
