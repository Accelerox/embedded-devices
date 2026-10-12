//! ADS1113, ADS1114, and ADS1115 signed 16-bit I²C ADCs.
//!
//! Adapted from embedded-ads111x by Maciej Skrzypek (MIT OR Apache-2.0),
//! including its conversion, comparator and ALERT/RDY support from
//! [wintermute101/ads111x](https://github.com/wintermute101/ads111x).
//! See the [TI datasheet](https://www.ti.com/lit/ds/symlink/ads1115.pdf).
//!
//! `maybe_async_cfg` generates Sync and Async driver types; both features may be enabled.
//! Construction performs no I/O. `convert_raw` and `convert` start and wait for a fresh
//! single-shot result. `read_raw` and `read` return the existing conversion register.
//! Apply `write_configuration` before immediate reads with a supplied configuration.
//!
//! ADS1113/1114 measure AIN0 minus AIN1. ADS1113 always uses ±2.048 V;
//! ADS1114/1115 offer programmable gain and comparator/ALERT outputs. Only ADS1115
//! has selectable inputs. Full-scale range is independent of VDD: physical inputs
//! must still satisfy the datasheet's voltage limits. No supply-voltage limits are enforced.
//!
//! After failed or cancelled writes, reapply the intended configuration and, if used,
//! conversion-ready setup. External resets or register changes are not tracked. Immediate
//! reads after gain/input changes can contain an older sample; allow a new conversion to
//! complete before interpreting it with the new gain. Voltage scaling is nominal, not calibrated.
//!
//! Without a pin, waits tightly poll I²C with no delay or timeout; async polling yields only
//! when a transaction suspends. Use separate start/status/read calls for application timing.
//! ALERT/RDY is open-drain and requires a pull-up compatible with the host's GPIO voltage.
//! Ready-pin conversion waits use single-shot levels; continuous mode emits short pulses
//! and is not supported by these level-based conversion waits.

//! ## Blocking conversion (ADS1115)
//!
//! ```rust
//! # #[cfg(all(feature = "sync", feature = "texas_instruments-ads1115"))] mod example {
//! # fn sample<I: embedded_hal::i2c::I2c>(i2c: I) -> Result<(), I::Error> {
//! use embedded_devices::devices::texas_instruments::ads111x::{
//!     ADS1115Sync, Configuration, DataRate, Gain, InputChannel, address::Address,
//! };
//! let configuration = Configuration::default()
//!     .with_gain(Gain::V4_096).with_data_rate(DataRate::Sps128);
//! let mut adc = ADS1115Sync::new(i2c, Address::Gnd, configuration).expect("valid address");
//! let voltage = adc.convert(InputChannel::Single0)?;
//! let counts = adc.convert_raw(InputChannel::Diff01)?;
//! let i2c = adc.destroy();
//! # Ok(())
//! # }
//! # }
//! ```
//!
//! ADS1113 and ADS1114 use the same constructor but `convert()` and `convert_raw()`
//! have no channel argument, because their inputs are fixed.
//!
//! ## Continuous reads (ADS1114)
//!
//! ```rust
//! # #[cfg(all(feature = "sync", feature = "texas_instruments-ads1114"))] mod example {
//! # fn sample<I: embedded_hal::i2c::I2c, D: embedded_hal::delay::DelayNs>(i2c: I, delay: &mut D)
//! # -> Result<(), I::Error> {
//! use embedded_devices::devices::texas_instruments::ads111x::{ADS1114Sync, Configuration, Mode};
//! let configuration = Configuration::default().with_mode(Mode::Continuous);
//! let mut adc = ADS1114Sync::new(i2c, 0x48, configuration).expect("valid address");
//! adc.write_configuration(configuration)?;
//! // Application timing: let a conversion finish at the default 128 SPS rate.
//! delay.delay_ms(20);
//! let voltage = adc.read()?;
//! let counts = adc.read_raw()?;
//! # Ok(())
//! # }
//! # }
//! ```
//!
//! ## Asynchronous ALERT/RDY conversion (ADS1115)
//!
//! ```rust
//! # #[cfg(all(feature = "async", feature = "texas_instruments-ads1115"))] mod example {
//! # async fn sample<I: embedded_hal_async::i2c::I2c, P: embedded_hal_async::digital::Wait>(i2c: I, pin: P)
//! # -> Result<(), embedded_devices::devices::texas_instruments::ads111x::ReadyError<I::Error, P::Error>> {
//! use embedded_devices::devices::texas_instruments::ads111x::{
//!     ADS1115Async, Configuration, InputChannel, ReadyError,
//! };
//! let mut adc = ADS1115Async::new(i2c, 0x48, Configuration::default())
//!     .expect("valid address").with_alert_ready(pin);
//! adc.configure_ready().await.map_err(ReadyError::I2c)?;
//! let voltage = adc.convert(InputChannel::Single0).await?;
//! let counts = adc.convert_raw(InputChannel::Single1).await?;
//! let (i2c, pin) = adc.destroy();
//! # Ok(())
//! # }
//! # }
//! ```
//!
//! Pinless async drivers return the underlying bus error directly. Pin-enabled conversions
//! distinguish bus errors, GPIO errors, and missing readiness setup. Attaching a pin alone
//! does not modify the chip; `configure_ready` replaces both comparator thresholds.
//!
//! ## Comparator threshold setup (ADS1114)
//!
//! ```rust
//! # #[cfg(all(feature = "sync", feature = "texas_instruments-ads1114"))] mod example {
//! # fn setup<I: embedded_hal::i2c::I2c, P: embedded_hal::digital::InputPin>(i2c: I, pin: P)
//! # -> Result<(), embedded_devices::devices::texas_instruments::ads111x::ReadyError<I::Error, P::Error>> {
//! use embedded_devices::devices::texas_instruments::ads111x::{
//!     ADS1114Sync, Configuration, ComparatorMode, ComparatorQueue, Gain, Mode, ReadyError,
//! };
//! let configuration = Configuration::default().with_gain(Gain::V4_096)
//!     .with_mode(Mode::Continuous).with_comparator_mode(ComparatorMode::Window)
//!     .with_comparator_queue(ComparatorQueue::AfterOne);
//! let mut adc = ADS1114Sync::new(i2c, 0x48, configuration)
//!     .expect("valid address").with_alert_ready(pin);
//! // At ±4.096 V full scale these counts represent 1 V and 3 V.
//! adc.set_alert_low_raw(8_000).map_err(ReadyError::I2c)?;
//! adc.set_alert_high_raw(24_000).map_err(ReadyError::I2c)?;
//! adc.write_configuration(configuration).map_err(ReadyError::I2c)?;
//! adc.wait_alert_asserted().map_err(ReadyError::Pin)?;
//! # Ok(())
//! # }
//! # }
//! ```
//!
//! Voltage threshold setters accept `ElectricPotential`, round to the nearest signed count,
//! and reject non-finite or unrepresentable values. Raw setters allow arbitrary `i16` counts.
//! Changing gain changes the effective voltage of already-written thresholds. Comparator
//! waits observe pin levels, do not start conversions, and do not clear a latched alert.
//!
//! ## Unsupported model settings are absent
//!
//! ```compile_fail
//! use embedded_devices::devices::texas_instruments::ads111x::{Configuration, Gain, models::ADS1113};
//! let configuration = Configuration::<ADS1113>::default().with_gain(Gain::V4_096);
//! ```
//!
//! ```compile_fail
//! use embedded_devices::devices::texas_instruments::ads111x::{Configuration, InputChannel, models::ADS1114};
//! let configuration = Configuration::<ADS1114>::default().with_input(InputChannel::Single0);
//! ```
//!
//! ```compile_fail
//! use embedded_devices::devices::texas_instruments::ads111x::{ADS1113Sync, Configuration};
//! fn attach<I: embedded_hal::i2c::I2c, P>(i2c: I, pin: P) {
//!     let adc = ADS1113Sync::new(i2c, 0x48, Configuration::default()).expect("valid address");
//!     let adc = adc.with_alert_ready(pin);
//! }
//! ```

use models::{HasComparator, Model};
use registers::{CONFIGURATION, CONVERSION, HIGH_THRESHOLD, LOW_THRESHOLD};
use uom::si::{electric_potential::volt, f64::ElectricPotential};

pub mod address;
pub mod models;
#[doc(hidden)]
pub mod ready;
pub mod registers;

pub use ready::{NoReadyPin, ReadyPin};
pub use registers::{
    ComparatorLatch, ComparatorMode, ComparatorPolarity, ComparatorQueue, Configuration, DataRate, Gain, InputChannel,
    Mode,
};

/// Invalid constructor configuration.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// Address exceeds the seven-bit range.
    #[error("invalid seven-bit address")]
    InvalidAddress,
}

/// Errors from voltage threshold setters.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ThresholdError<E> {
    /// Voltage is non-finite or cannot be represented with the selected gain.
    #[error("voltage outside representable range")]
    InvalidVoltage,
    /// I²C transfer failed.
    #[error("I²C transfer failed: {0}")]
    I2c(E),
}

/// Failures from conversion workflows using an attached ALERT/RDY input.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ReadyError<I, P> {
    /// I²C transfer failed.
    #[error("I²C transfer failed: {0}")]
    I2c(I),
    /// Reading or awaiting the input failed.
    #[error("GPIO operation failed: {0}")]
    Pin(P),
    /// Conversion-ready setup has not completed successfully.
    #[error("conversion-ready output not configured")]
    NotConfigured,
    /// Level-based readiness cannot identify samples in continuous mode.
    #[error("conversion-ready wait requires single-shot mode")]
    NotSingleShot,
}

/// Shared driver; use the model-specific ADS1113/1114/1115 aliases.
#[maybe_async_cfg::maybe(sync(feature = "sync"), async(feature = "async"))]
pub struct ADS111x<I, M: Model, R = NoReadyPin> {
    interface: I,
    address: u8,
    configuration: Configuration<M>,
    ready: R,
    ready_configured: bool,
}

#[maybe_async_cfg::maybe(
    idents(hal(sync = "embedded_hal", async = "embedded_hal_async")),
    sync(feature = "sync"),
    async(feature = "async")
)]
impl<I: hal::i2c::I2c, M: Model> ADS111x<I, M> {
    /// Creates a driver without hardware access; conversions apply the supplied settings.
    pub fn new(interface: I, address: impl Into<u8>, configuration: Configuration<M>) -> Result<Self, ConfigError> {
        let address = address.into();
        if address > 0x7F {
            return Err(ConfigError::InvalidAddress);
        }
        Ok(Self {
            interface,
            address,
            configuration,
            ready: NoReadyPin,
            ready_configured: false,
        })
    }

    /// Returns the owned bus without accessing hardware.
    pub fn destroy(self) -> I {
        self.interface
    }
}

#[maybe_async_cfg::maybe(
    idents(hal(sync = "embedded_hal", async = "embedded_hal_async")),
    sync(feature = "sync"),
    async(feature = "async")
)]
impl<I: hal::i2c::I2c, M: HasComparator> ADS111x<I, M> {
    /// Attaches an input without hardware access; call `configure_ready` before ready waits.
    pub fn with_alert_ready<P>(self, pin: P) -> ADS111x<I, M, ReadyPin<P>> {
        ADS111x {
            interface: self.interface,
            address: self.address,
            configuration: self.configuration,
            ready: ReadyPin { pin },
            ready_configured: self.ready_configured,
        }
    }
}

#[maybe_async_cfg::maybe(
    idents(hal(sync = "embedded_hal", async = "embedded_hal_async")),
    sync(feature = "sync"),
    async(feature = "async")
)]
impl<I: hal::i2c::I2c, M: Model, P> ADS111x<I, M, ReadyPin<P>> {
    /// Returns the owned bus and input without accessing hardware.
    pub fn destroy(self) -> (I, P) {
        (self.interface, self.ready.pin)
    }
}

#[maybe_async_cfg::maybe(
    idents(hal(sync = "embedded_hal", async = "embedded_hal_async")),
    sync(feature = "sync"),
    async(feature = "async")
)]
impl<I: hal::i2c::I2c, M: Model, R> ADS111x<I, M, R> {
    /// Applies writable settings; commits the cache only after the write succeeds.
    /// Does not start a single-shot conversion. Continuous mode starts automatically.
    pub async fn write_configuration(&mut self, configuration: Configuration<M>) -> Result<(), I::Error> {
        if !self.configuration.preserves_ready(&configuration) {
            self.ready_configured = false;
        }
        self.write_register(CONFIGURATION, configuration.bits()).await?;
        self.configuration = configuration;
        Ok(())
    }

    /// Reads writable settings and synchronizes the cache; ignores the OS status bit.
    pub async fn read_configuration(&mut self) -> Result<Configuration<M>, I::Error> {
        let configuration = Configuration::from_bits(self.read_register(CONFIGURATION).await?);
        if !self.configuration.preserves_ready(&configuration) {
            self.ready_configured = false;
        }
        self.configuration = configuration;
        Ok(configuration)
    }

    /// Checks OS once over I²C: true means idle, even before any conversion.
    /// This does not detect individual samples in continuous mode.
    pub async fn is_idle(&mut self) -> Result<bool, I::Error> {
        Ok(self.read_register(CONFIGURATION).await? & 0x8000 != 0)
    }

    /// Reads the existing signed conversion register without starting or waiting.
    /// Reading conversion data also clears a latched comparator alert.
    pub async fn read_raw(&mut self) -> Result<i16, I::Error> {
        Ok(self.read_register(CONVERSION).await? as i16)
    }

    /// Reads the existing conversion, scaled by the cached gain (fixed on ADS1113).
    pub async fn read(&mut self) -> Result<ElectricPotential, I::Error> {
        let raw = self.read_raw().await?;
        Ok(self.scale(raw))
    }

    fn scale(&self, raw: i16) -> ElectricPotential {
        ElectricPotential::new::<volt>(f64::from(raw) * self.configuration.gain.full_scale_volts() / 32768.0)
    }

    async fn start(&mut self, mut configuration: Configuration<M>) -> Result<(), I::Error> {
        configuration.mode = Mode::SingleShot;
        self.write_register(CONFIGURATION, configuration.bits() | 0x8000)
            .await?;
        self.configuration = configuration;
        Ok(())
    }

    async fn read_register(&mut self, register: u8) -> Result<u16, I::Error> {
        let mut bytes = [0; 2];
        // Combined transaction preserves the repeated START between pointer and data.
        self.interface.write_read(self.address, &[register], &mut bytes).await?;
        Ok(u16::from_be_bytes(bytes))
    }

    async fn write_register(&mut self, register: u8, value: u16) -> Result<(), I::Error> {
        let bytes = value.to_be_bytes();
        self.interface
            .write(self.address, &[register, bytes[0], bytes[1]])
            .await
    }
}

#[maybe_async_cfg::maybe(
    idents(hal(sync = "embedded_hal", async = "embedded_hal_async"), ReadySignal),
    sync(feature = "sync"),
    async(feature = "async")
)]
impl<I: hal::i2c::I2c, M: Model, R: ready::ReadySignal<I>> ADS111x<I, M, R> {
    /// Waits for single-shot completion through the pin, or tightly polls OS without a pin.
    /// Pin waits require configured readiness. No timeout is imposed.
    pub async fn wait_conversion_ready(&mut self) -> Result<(), R::Error> {
        self.ready
            .validate(self.ready_configured, self.configuration.mode == Mode::SingleShot)?;
        self.ready
            .wait(
                &mut self.interface,
                self.address,
                self.configuration.comparator_polarity,
            )
            .await
    }

    async fn convert_raw_with(&mut self, configuration: Configuration<M>) -> Result<i16, R::Error> {
        self.ready.validate(self.ready_configured, true)?;
        // A start request is ignored during conversion: drain continuous mode before starting anew.
        if self.configuration.mode == Mode::Continuous {
            self.write_configuration(self.configuration.with_mode(Mode::SingleShot))
                .await
                .map_err(R::bus_error)?;
            while !self.is_idle().await.map_err(R::bus_error)? {}
        }
        self.start(configuration).await.map_err(R::bus_error)?;
        self.wait_conversion_ready().await?;
        self.read_raw().await.map_err(R::bus_error)
    }
}

#[maybe_async_cfg::maybe(
    idents(hal(sync = "embedded_hal", async = "embedded_hal_async")),
    sync(feature = "sync"),
    async(feature = "async")
)]
impl<I: hal::i2c::I2c, M: HasComparator, R> ADS111x<I, M, R> {
    /// Configures ALERT/RDY with low=0, high=-32768 and queue=one; preserves polarity.
    /// Replaces comparator thresholds. Setup is valid only after all three writes succeed.
    pub async fn configure_ready(&mut self) -> Result<(), I::Error> {
        self.ready_configured = false;
        self.set_alert_low_raw(0).await?;
        self.set_alert_high_raw(i16::MIN).await?;
        let configuration = self.configuration.with_comparator_queue(ComparatorQueue::AfterOne);
        self.write_configuration(configuration).await?;
        self.ready_configured = true;
        Ok(())
    }

    /// Writes the low threshold in signed counts, invalidating conversion-ready setup.
    pub async fn set_alert_low_raw(&mut self, threshold: i16) -> Result<(), I::Error> {
        self.ready_configured = false;
        self.write_register(LOW_THRESHOLD, threshold as u16).await
    }

    /// Writes the high threshold in signed counts, invalidating conversion-ready setup.
    pub async fn set_alert_high_raw(&mut self, threshold: i16) -> Result<(), I::Error> {
        self.ready_configured = false;
        self.write_register(HIGH_THRESHOLD, threshold as u16).await
    }

    /// Writes a finite, representable low voltage threshold, rounded to the nearest count.
    /// Ties round away from zero. Gain changes later alter its effective voltage.
    pub async fn set_alert_low(&mut self, voltage: ElectricPotential) -> Result<(), ThresholdError<I::Error>> {
        let threshold = self
            .configuration
            .gain
            .counts(voltage)
            .ok_or(ThresholdError::InvalidVoltage)?;
        self.set_alert_low_raw(threshold).await.map_err(ThresholdError::I2c)
    }

    /// Writes a high voltage threshold using the same range and rounding as `set_alert_low`.
    /// Invalid voltages leave hardware and readiness setup untouched.
    pub async fn set_alert_high(&mut self, voltage: ElectricPotential) -> Result<(), ThresholdError<I::Error>> {
        let threshold = self
            .configuration
            .gain
            .counts(voltage)
            .ok_or(ThresholdError::InvalidVoltage)?;
        self.set_alert_high_raw(threshold).await.map_err(ThresholdError::I2c)
    }
}

#[cfg(feature = "sync")]
impl<I: embedded_hal::i2c::I2c, M: HasComparator, P: embedded_hal::digital::InputPin> ADS111xSync<I, M, ReadyPin<P>> {
    /// Waits until comparator ALERT is asserted; no I²C access or automatic setup.
    /// Requires valid thresholds, an enabled comparator and ready signaling disabled.
    pub fn wait_alert_asserted(&mut self) -> Result<(), P::Error> {
        self.ready.wait_level_sync(self.configuration.comparator_polarity)
    }

    /// Waits until comparator ALERT clears; does not clear a latched alert itself.
    /// Latched alerts require a conversion-register read or SMBus alert response.
    pub fn wait_alert_cleared(&mut self) -> Result<(), P::Error> {
        let polarity = match self.configuration.comparator_polarity {
            ComparatorPolarity::ActiveLow => ComparatorPolarity::ActiveHigh,
            ComparatorPolarity::ActiveHigh => ComparatorPolarity::ActiveLow,
        };
        self.ready.wait_level_sync(polarity)
    }
}

#[cfg(feature = "async")]
impl<I: embedded_hal_async::i2c::I2c, M: HasComparator, P: embedded_hal_async::digital::Wait>
    ADS111xAsync<I, M, ReadyPin<P>>
{
    /// Waits until comparator ALERT is asserted; no I²C access or automatic setup.
    /// Requires valid thresholds, an enabled comparator and ready signaling disabled.
    pub async fn wait_alert_asserted(&mut self) -> Result<(), P::Error> {
        self.ready
            .wait_level_async(self.configuration.comparator_polarity)
            .await
    }

    /// Waits until comparator ALERT clears; does not clear a latched alert itself.
    /// Latched alerts require a conversion-register read or SMBus alert response.
    pub async fn wait_alert_cleared(&mut self) -> Result<(), P::Error> {
        let polarity = match self.configuration.comparator_polarity {
            ComparatorPolarity::ActiveLow => ComparatorPolarity::ActiveHigh,
            ComparatorPolarity::ActiveHigh => ComparatorPolarity::ActiveLow,
        };
        self.ready.wait_level_async(polarity).await
    }
}

/// ADS1113 blocking driver.
#[cfg(all(feature = "sync", feature = "texas_instruments-ads1113"))]
pub type ADS1113Sync<I, R = NoReadyPin> = ADS111xSync<I, models::ADS1113, R>;

/// ADS1113 asynchronous driver.
#[cfg(all(feature = "async", feature = "texas_instruments-ads1113"))]
pub type ADS1113Async<I, R = NoReadyPin> = ADS111xAsync<I, models::ADS1113, R>;

#[cfg(feature = "texas_instruments-ads1113")]
#[maybe_async_cfg::maybe(
    idents(hal(sync = "embedded_hal", async = "embedded_hal_async")),
    sync(feature = "sync"),
    async(feature = "async")
)]
impl<I: hal::i2c::I2c, R> ADS111x<I, models::ADS1113, R> {
    /// Selects single-shot mode and starts a conversion without waiting.
    /// The ADC must be idle: a start request during conversion is ignored.
    pub async fn start_single_conversion(&mut self) -> Result<(), I::Error> {
        self.start(self.configuration).await
    }
}

#[cfg(feature = "texas_instruments-ads1113")]
#[maybe_async_cfg::maybe(
    idents(hal(sync = "embedded_hal", async = "embedded_hal_async"), ReadySignal),
    sync(feature = "sync"),
    async(feature = "async")
)]
impl<I: hal::i2c::I2c, R: ready::ReadySignal<I>> ADS111x<I, models::ADS1113, R> {
    /// Starts a fresh single-shot conversion, waits, and returns signed counts.
    /// Do not overlap a previously started single-shot conversion. Continuous mode is drained first.
    pub async fn convert_raw(&mut self) -> Result<i16, R::Error> {
        self.convert_raw_with(self.configuration).await
    }

    /// Starts a fresh single-shot conversion, waits, and returns nominal voltage.
    pub async fn convert(&mut self) -> Result<ElectricPotential, R::Error> {
        let raw = self.convert_raw().await?;
        Ok(self.scale(raw))
    }
}

/// ADS1114 blocking driver.
#[cfg(all(feature = "sync", feature = "texas_instruments-ads1114"))]
pub type ADS1114Sync<I, R = NoReadyPin> = ADS111xSync<I, models::ADS1114, R>;

/// ADS1114 asynchronous driver.
#[cfg(all(feature = "async", feature = "texas_instruments-ads1114"))]
pub type ADS1114Async<I, R = NoReadyPin> = ADS111xAsync<I, models::ADS1114, R>;

#[cfg(feature = "texas_instruments-ads1114")]
#[maybe_async_cfg::maybe(
    idents(hal(sync = "embedded_hal", async = "embedded_hal_async")),
    sync(feature = "sync"),
    async(feature = "async")
)]
impl<I: hal::i2c::I2c, R> ADS111x<I, models::ADS1114, R> {
    /// Selects single-shot mode and starts a conversion without waiting.
    /// The ADC must be idle: a start request during conversion is ignored.
    pub async fn start_single_conversion(&mut self) -> Result<(), I::Error> {
        self.start(self.configuration).await
    }
}

#[cfg(feature = "texas_instruments-ads1114")]
#[maybe_async_cfg::maybe(
    idents(hal(sync = "embedded_hal", async = "embedded_hal_async"), ReadySignal),
    sync(feature = "sync"),
    async(feature = "async")
)]
impl<I: hal::i2c::I2c, R: ready::ReadySignal<I>> ADS111x<I, models::ADS1114, R> {
    /// Starts a fresh single-shot conversion, waits, and returns signed counts.
    /// Do not overlap a previously started single-shot conversion. Continuous mode is drained first.
    pub async fn convert_raw(&mut self) -> Result<i16, R::Error> {
        self.convert_raw_with(self.configuration).await
    }

    /// Starts a fresh single-shot conversion, waits, and returns nominal voltage.
    pub async fn convert(&mut self) -> Result<ElectricPotential, R::Error> {
        let raw = self.convert_raw().await?;
        Ok(self.scale(raw))
    }
}

/// ADS1115 blocking driver.
#[cfg(all(feature = "sync", feature = "texas_instruments-ads1115"))]
pub type ADS1115Sync<I, R = NoReadyPin> = ADS111xSync<I, models::ADS1115, R>;

/// ADS1115 asynchronous driver.
#[cfg(all(feature = "async", feature = "texas_instruments-ads1115"))]
pub type ADS1115Async<I, R = NoReadyPin> = ADS111xAsync<I, models::ADS1115, R>;

#[cfg(feature = "texas_instruments-ads1115")]
#[maybe_async_cfg::maybe(
    idents(hal(sync = "embedded_hal", async = "embedded_hal_async")),
    sync(feature = "sync"),
    async(feature = "async")
)]
impl<I: hal::i2c::I2c, R> ADS111x<I, models::ADS1115, R> {
    /// Selects single-shot mode and starts a conversion without waiting.
    /// The ADC must be idle: a start request during conversion is ignored.
    pub async fn start_single_conversion(&mut self, channel: InputChannel) -> Result<(), I::Error> {
        self.start(self.configuration.with_input(channel)).await
    }
}

#[cfg(feature = "texas_instruments-ads1115")]
#[maybe_async_cfg::maybe(
    idents(hal(sync = "embedded_hal", async = "embedded_hal_async"), ReadySignal),
    sync(feature = "sync"),
    async(feature = "async")
)]
impl<I: hal::i2c::I2c, R: ready::ReadySignal<I>> ADS111x<I, models::ADS1115, R> {
    /// Starts a fresh single-shot conversion, waits, and returns signed counts.
    /// Do not overlap a previously started single-shot conversion. Continuous mode is drained first.
    pub async fn convert_raw(&mut self, channel: InputChannel) -> Result<i16, R::Error> {
        self.convert_raw_with(self.configuration.with_input(channel)).await
    }

    /// Starts a fresh single-shot conversion, waits, and returns nominal voltage.
    pub async fn convert(&mut self, channel: InputChannel) -> Result<ElectricPotential, R::Error> {
        let raw = self.convert_raw(channel).await?;
        Ok(self.scale(raw))
    }
}

#[cfg(all(test, any(feature = "sync", feature = "async")))]
mod tests;
