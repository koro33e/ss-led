use crate::encoder::Pulses;
use crate::{ColorFormat, Error, Timing, WS2812B_TIMING};
use core::borrow::Borrow;
use esp_hal::{
    Async,
    gpio::{Level, interconnect::PeripheralOutput},
    rmt::{Channel, PulseCode, Tx, TxChannelConfig, TxChannelCreator},
    time::Rate,
};

/// Async RMT LED controller using an exclusively borrowed pulse buffer and one hardware RAM block.
/// Long frames require timely async refill; verify waveform continuity under system load.
pub struct LedController<'d, 'b, F: ColorFormat> {
    channel: Channel<'d, Async, Tx>,
    format: F,
    pulses: Pulses<PulseCode>,
    buffer: &'b mut [PulseCode],
}
impl<'d, 'b, F: ColorFormat> LedController<'d, 'b, F> {
    /// Create a controller using the default timing. `freq` must be this RMT's `frequency()`.
    pub fn new<Ch, P>(
        channel: Ch,
        pin: P,
        freq: Rate,
        format: F,
        buffer: &'b mut [PulseCode],
    ) -> Result<Self, Error>
    where
        Ch: TxChannelCreator<'d, Async>,
        P: PeripheralOutput<'d>,
    {
        Self::with_timing(channel, pin, freq, format, buffer, WS2812B_TIMING)
    }
    /// Create a controller with custom timing. `freq` must be this RMT's `frequency()`.
    pub fn with_timing<Ch, P>(
        channel: Ch,
        pin: P,
        freq: Rate,
        format: F,
        buffer: &'b mut [PulseCode],
        timing: Timing,
    ) -> Result<Self, Error>
    where
        Ch: TxChannelCreator<'d, Async>,
        P: PeripheralOutput<'d>,
    {
        if buffer.len() < 2 {
            return Err(Error::BufferSizeExceeded);
        }
        let pulses = Pulses::new(timing, freq.as_hz(), |level1, length1, level2, length2| {
            PulseCode::new(level1.into(), length1, level2.into(), length2)
        })?;
        let config = TxChannelConfig::default()
            .with_clk_divider(1)
            .with_memsize(1)
            .with_idle_output_level(Level::Low)
            .with_idle_output(true)
            .with_carrier_modulation(false);
        let channel = channel.configure_tx(&config)?.with_pin(pin);
        Ok(Self {
            channel,
            format,
            pulses,
            buffer,
        })
    }
    /// Encode and send a frame. After cancellation or transmission failure, successfully
    /// call `reset` before sending the next frame, then resend the complete desired frame.
    pub async fn write<I>(&mut self, pixels: I) -> Result<(), Error>
    where
        I: IntoIterator,
        I::Item: Borrow<F::Pixel>,
    {
        let len = self.pulses.encode(self.format, pixels, self.buffer)?;
        self.channel
            .transmit(&self.buffer[..len])
            .await
            .map_err(Into::into)
    }
    /// Send one pixel; this does not fill all connected LEDs.
    pub async fn write_pixel(&mut self, pixel: F::Pixel) -> Result<(), Error> {
        self.write(core::iter::once(pixel)).await
    }
    /// Send only a latch pulse; this does not clear the LEDs.
    pub async fn reset(&mut self) -> Result<(), Error> {
        let pulses = [self.pulses.reset, PulseCode::end_marker()];
        self.channel.transmit(&pulses).await.map_err(Into::into)
    }
}
