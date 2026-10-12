//! Sealed readiness strategies used by the shared conversion workflow.

use super::{
    ReadyError,
    registers::{CONFIGURATION, ComparatorPolarity},
};

mod private {
    pub trait Sealed {}
}

/// Poll conversion status over I²C instead of owning a GPIO.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoReadyPin;
impl private::Sealed for NoReadyPin {}

/// An owned ALERT/RDY input; attach it with `with_alert_ready`.
#[derive(Debug)]
pub struct ReadyPin<P> {
    pub(super) pin: P,
}
impl<P> private::Sealed for ReadyPin<P> {}

/// Internal readiness abstraction; applications cannot implement it.
#[allow(async_fn_in_trait)]
#[maybe_async_cfg::maybe(
    idents(hal(sync = "embedded_hal", async = "embedded_hal_async")),
    sync(feature = "sync"),
    async(feature = "async")
)]
pub trait ReadySignal<I: hal::i2c::I2c>: private::Sealed {
    type Error;
    fn validate(&self, configured: bool, single_shot: bool) -> Result<(), Self::Error>;
    fn bus_error(error: I::Error) -> Self::Error;
    async fn wait(&mut self, interface: &mut I, address: u8, polarity: ComparatorPolarity) -> Result<(), Self::Error>;
}

#[maybe_async_cfg::maybe(
    idents(hal(sync = "embedded_hal", async = "embedded_hal_async"), ReadySignal),
    sync(feature = "sync"),
    async(feature = "async"),
    keep_self
)]
impl<I: hal::i2c::I2c> ReadySignal<I> for NoReadyPin {
    type Error = I::Error;
    fn validate(&self, _: bool, _: bool) -> Result<(), Self::Error> {
        Ok(())
    }
    fn bus_error(error: I::Error) -> Self::Error {
        error
    }
    async fn wait(&mut self, interface: &mut I, address: u8, _: ComparatorPolarity) -> Result<(), Self::Error> {
        loop {
            let mut bytes = [0; 2];
            interface.write_read(address, &[CONFIGURATION], &mut bytes).await?;
            if bytes[0] & 0x80 != 0 {
                return Ok(());
            }
        }
    }
}

#[cfg(feature = "sync")]
impl<I: embedded_hal::i2c::I2c, P: embedded_hal::digital::InputPin> ReadySignalSync<I> for ReadyPin<P> {
    type Error = ReadyError<I::Error, P::Error>;
    fn validate(&self, configured: bool, single_shot: bool) -> Result<(), Self::Error> {
        if !configured {
            return Err(ReadyError::NotConfigured);
        }
        if !single_shot {
            return Err(ReadyError::NotSingleShot);
        }
        Ok(())
    }
    fn bus_error(error: I::Error) -> Self::Error {
        ReadyError::I2c(error)
    }
    fn wait(&mut self, _: &mut I, _: u8, polarity: ComparatorPolarity) -> Result<(), Self::Error> {
        self.wait_level_sync(polarity).map_err(ReadyError::Pin)
    }
}

#[cfg(feature = "sync")]
impl<P: embedded_hal::digital::InputPin> ReadyPin<P> {
    pub(super) fn wait_level_sync(&mut self, polarity: ComparatorPolarity) -> Result<(), P::Error> {
        loop {
            let asserted = match polarity {
                ComparatorPolarity::ActiveLow => self.pin.is_low()?,
                ComparatorPolarity::ActiveHigh => self.pin.is_high()?,
            };
            if asserted {
                return Ok(());
            }
        }
    }
}

#[cfg(feature = "async")]
impl<I: embedded_hal_async::i2c::I2c, P: embedded_hal_async::digital::Wait> ReadySignalAsync<I> for ReadyPin<P> {
    type Error = ReadyError<I::Error, P::Error>;
    fn validate(&self, configured: bool, single_shot: bool) -> Result<(), Self::Error> {
        if !configured {
            return Err(ReadyError::NotConfigured);
        }
        if !single_shot {
            return Err(ReadyError::NotSingleShot);
        }
        Ok(())
    }
    fn bus_error(error: I::Error) -> Self::Error {
        ReadyError::I2c(error)
    }
    async fn wait(&mut self, _: &mut I, _: u8, polarity: ComparatorPolarity) -> Result<(), Self::Error> {
        self.wait_level_async(polarity).await.map_err(ReadyError::Pin)
    }
}

#[cfg(feature = "async")]
impl<P: embedded_hal_async::digital::Wait> ReadyPin<P> {
    pub(super) async fn wait_level_async(&mut self, polarity: ComparatorPolarity) -> Result<(), P::Error> {
        match polarity {
            ComparatorPolarity::ActiveLow => self.pin.wait_for_low().await,
            ComparatorPolarity::ActiveHigh => self.pin.wait_for_high().await,
        }
    }
}
