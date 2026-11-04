#![no_std]
#![doc = include_str!("../README.md")]

mod color;
#[cfg(feature = "esp-hal")]
mod controller;
mod encoder;

pub use color::{ColorFormat, RGB, RGBW, RgbOrder, RgbwOrder};
#[cfg(feature = "esp-hal")]
pub use controller::LedController;
pub use encoder::{Timing, WS2812B_TIMING, WS2812B_V5_TIMING, buffer_size, checked_buffer_size};

/// Driver errors, including the original RMT configuration and transmission errors.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    /// The pulse buffer cannot hold the requested frame.
    BufferSizeExceeded,
    /// A duration cannot be represented at the selected RMT frequency.
    InvalidTiming,
    /// RMT channel configuration failed.
    #[cfg(feature = "esp-hal")]
    Config(esp_hal::rmt::ConfigError),
    /// RMT transmission failed.
    #[cfg(feature = "esp-hal")]
    Transmission(esp_hal::rmt::Error),
}
#[cfg(feature = "esp-hal")]
impl From<esp_hal::rmt::ConfigError> for Error {
    fn from(err: esp_hal::rmt::ConfigError) -> Self {
        Self::Config(err)
    }
}
#[cfg(feature = "esp-hal")]
impl From<esp_hal::rmt::Error> for Error {
    fn from(err: esp_hal::rmt::Error) -> Self {
        Self::Transmission(err)
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BufferSizeExceeded => {
                f.write_str("pulse buffer too small or invalid pixel format")
            }
            Self::InvalidTiming => {
                f.write_str("timing cannot be represented at this RMT frequency")
            }
            #[cfg(feature = "esp-hal")]
            Self::Config(err) => write!(f, "RMT configuration failed: {err:?}"),
            #[cfg(feature = "esp-hal")]
            Self::Transmission(err) => write!(f, "RMT transmission failed: {err:?}"),
        }
    }
}

impl core::error::Error for Error {}
