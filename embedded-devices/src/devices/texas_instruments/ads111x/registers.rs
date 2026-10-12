//! Typed configuration and register encoding, independent of transport.

use super::models::{HasComparator, HasPga, Model};
use core::marker::PhantomData;
use uom::si::{electric_potential::volt, f64::ElectricPotential};

pub(super) const CONVERSION: u8 = 0;
pub(super) const CONFIGURATION: u8 = 1;
pub(super) const LOW_THRESHOLD: u8 = 2;
pub(super) const HIGH_THRESHOLD: u8 = 3;

/// ADS1115 positive and negative input selection.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InputChannel {
    /// AIN0 relative to AIN1.
    Diff01 = 0,
    /// AIN0 relative to AIN3.
    Diff03 = 1,
    /// AIN1 relative to AIN3.
    Diff13 = 2,
    /// AIN2 relative to AIN3.
    Diff23 = 3,
    /// AIN0 relative to GND.
    Single0 = 4,
    /// AIN1 relative to GND.
    Single1 = 5,
    /// AIN2 relative to GND.
    Single2 = 6,
    /// AIN3 relative to GND.
    Single3 = 7,
}

impl InputChannel {
    fn from_bits(bits: u16) -> Self {
        match bits {
            0 => Self::Diff01,
            1 => Self::Diff03,
            2 => Self::Diff13,
            3 => Self::Diff23,
            4 => Self::Single0,
            5 => Self::Single1,
            6 => Self::Single2,
            _ => Self::Single3,
        }
    }
}

/// Programmable full-scale range; input pin limits still apply.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Gain {
    /// ±6.144 V.
    V6_144 = 0,
    /// ±4.096 V.
    V4_096 = 1,
    /// ±2.048 V.
    V2_048 = 2,
    /// ±1.024 V.
    V1_024 = 3,
    /// ±0.512 V.
    V0_512 = 4,
    /// ±0.256 V.
    V0_256 = 5,
}

impl Gain {
    fn from_bits(bits: u16) -> Self {
        match bits {
            0 => Self::V6_144,
            1 => Self::V4_096,
            2 => Self::V2_048,
            3 => Self::V1_024,
            4 => Self::V0_512,
            _ => Self::V0_256,
        }
    }
}

/// ADC operating mode.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Mode {
    /// Repeated conversions.
    Continuous = 0,
    /// Single conversion followed by power-down.
    SingleShot = 1,
}

impl Mode {
    fn from_bits(bits: u16) -> Self {
        match bits {
            0 => Self::Continuous,
            _ => Self::SingleShot,
        }
    }
}

/// Nominal conversions per second.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DataRate {
    /// 8 samples per second.
    Sps8 = 0,
    /// 16 samples per second.
    Sps16 = 1,
    /// 32 samples per second.
    Sps32 = 2,
    /// 64 samples per second.
    Sps64 = 3,
    /// 128 samples per second.
    Sps128 = 4,
    /// 250 samples per second.
    Sps250 = 5,
    /// 475 samples per second.
    Sps475 = 6,
    /// 860 samples per second.
    Sps860 = 7,
}

impl DataRate {
    fn from_bits(bits: u16) -> Self {
        match bits {
            0 => Self::Sps8,
            1 => Self::Sps16,
            2 => Self::Sps32,
            3 => Self::Sps64,
            4 => Self::Sps128,
            5 => Self::Sps250,
            6 => Self::Sps475,
            _ => Self::Sps860,
        }
    }
}

/// Comparator threshold interpretation.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ComparatorMode {
    /// High-threshold assertion with low-threshold hysteresis.
    Traditional = 0,
    /// Assert outside the low/high threshold window.
    Window = 1,
}

impl ComparatorMode {
    fn from_bits(bits: u16) -> Self {
        match bits {
            0 => Self::Traditional,
            _ => Self::Window,
        }
    }
}

/// ALERT/RDY asserted level.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ComparatorPolarity {
    /// Assert low (requires a pull-up).
    ActiveLow = 0,
    /// Assert high (requires a pull-up).
    ActiveHigh = 1,
}

impl ComparatorPolarity {
    fn from_bits(bits: u16) -> Self {
        match bits {
            0 => Self::ActiveLow,
            _ => Self::ActiveHigh,
        }
    }
}

/// Comparator latch behavior.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ComparatorLatch {
    /// Automatically clear when the comparison condition clears.
    NonLatching = 0,
    /// Clear by reading conversion data or SMBus alert response.
    Latching = 1,
}

impl ComparatorLatch {
    fn from_bits(bits: u16) -> Self {
        match bits {
            0 => Self::NonLatching,
            _ => Self::Latching,
        }
    }
}

/// Number of consecutive conversions before asserting ALERT.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ComparatorQueue {
    /// Assert after one conversion.
    AfterOne = 0,
    /// Assert after two conversions.
    AfterTwo = 1,
    /// Assert after four conversions.
    AfterFour = 2,
    /// Disable comparator and conversion-ready output.
    Disabled = 3,
}

impl ComparatorQueue {
    fn from_bits(bits: u16) -> Self {
        match bits {
            0 => Self::AfterOne,
            1 => Self::AfterTwo,
            2 => Self::AfterFour,
            _ => Self::Disabled,
        }
    }
}

impl Gain {
    /// Nominal positive full-scale voltage, independent of the supply voltage.
    pub fn full_scale_voltage(self) -> ElectricPotential {
        ElectricPotential::new::<volt>(self.full_scale_volts())
    }

    pub(super) fn full_scale_volts(self) -> f64 {
        match self {
            Self::V6_144 => 6.144,
            Self::V4_096 => 4.096,
            Self::V2_048 => 2.048,
            Self::V1_024 => 1.024,
            Self::V0_512 => 0.512,
            Self::V0_256 => 0.256,
        }
    }

    pub(super) fn counts(self, voltage: ElectricPotential) -> Option<i16> {
        let volts = voltage.get::<volt>();
        let scale = self.full_scale_volts();
        if !volts.is_finite() || volts < -scale || volts > f64::from(i16::MAX) * scale / 32768.0 {
            return None;
        }
        let counts = volts * 32768.0 / scale;
        // Round ties away from zero without a libm dependency.
        Some((counts + if counts >= 0.0 { 0.5 } else { -0.5 }) as i16)
    }
}

/// Writable settings supported by model `M`; defaults match power-on settings.
/// Conversion-start and idle status are deliberately separate from this value.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Configuration<M: Model> {
    pub(super) input: InputChannel,
    pub(super) gain: Gain,
    pub(super) mode: Mode,
    data_rate: DataRate,
    comparator_mode: ComparatorMode,
    pub(super) comparator_polarity: ComparatorPolarity,
    comparator_latch: ComparatorLatch,
    pub(super) comparator_queue: ComparatorQueue,
    model: PhantomData<M>,
}

impl<M: Model> Default for Configuration<M> {
    fn default() -> Self {
        Self {
            input: InputChannel::Diff01,
            gain: Gain::V2_048,
            mode: Mode::SingleShot,
            data_rate: DataRate::Sps128,
            comparator_mode: ComparatorMode::Traditional,
            comparator_polarity: ComparatorPolarity::ActiveLow,
            comparator_latch: ComparatorLatch::NonLatching,
            comparator_queue: ComparatorQueue::Disabled,
            model: PhantomData,
        }
    }
}

impl<M: Model> Configuration<M> {
    /// Select continuous conversion or single-shot power-down.
    pub fn with_mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }
    /// Current operating mode.
    pub fn mode(&self) -> Mode {
        self.mode
    }
    /// Select the nominal conversion rate.
    pub fn with_data_rate(mut self, data_rate: DataRate) -> Self {
        self.data_rate = data_rate;
        self
    }
    /// Current nominal conversion rate.
    pub fn data_rate(&self) -> DataRate {
        self.data_rate
    }

    pub(super) fn bits(&self) -> u16 {
        ((self.input as u16) << 12)
            | ((self.gain as u16) << 9)
            | ((self.mode as u16) << 8)
            | ((self.data_rate as u16) << 5)
            | ((self.comparator_mode as u16) << 4)
            | ((self.comparator_polarity as u16) << 3)
            | ((self.comparator_latch as u16) << 2)
            | self.comparator_queue as u16
    }

    pub(super) fn from_bits(bits: u16) -> Self {
        // Reserved model-specific fields retain their documented reset values.
        Self {
            input: if M::HAS_MUX {
                InputChannel::from_bits((bits >> 12) & 7)
            } else {
                InputChannel::Diff01
            },
            gain: if M::HAS_PGA {
                Gain::from_bits((bits >> 9) & 7)
            } else {
                Gain::V2_048
            },
            mode: Mode::from_bits((bits >> 8) & 1),
            data_rate: DataRate::from_bits((bits >> 5) & 7),
            comparator_mode: if M::HAS_COMPARATOR {
                ComparatorMode::from_bits((bits >> 4) & 1)
            } else {
                ComparatorMode::Traditional
            },
            comparator_polarity: if M::HAS_COMPARATOR {
                ComparatorPolarity::from_bits((bits >> 3) & 1)
            } else {
                ComparatorPolarity::ActiveLow
            },
            comparator_latch: if M::HAS_COMPARATOR {
                ComparatorLatch::from_bits((bits >> 2) & 1)
            } else {
                ComparatorLatch::NonLatching
            },
            comparator_queue: if M::HAS_COMPARATOR {
                ComparatorQueue::from_bits(bits & 3)
            } else {
                ComparatorQueue::Disabled
            },
            model: PhantomData,
        }
    }

    pub(super) fn preserves_ready(&self, other: &Self) -> bool {
        self.comparator_queue != ComparatorQueue::Disabled
            && other.comparator_queue != ComparatorQueue::Disabled
            && self.comparator_polarity == other.comparator_polarity
    }
}

impl<M: HasPga> Configuration<M> {
    /// Select the full-scale range without changing physical input limits.
    pub fn with_gain(mut self, gain: Gain) -> Self {
        self.gain = gain;
        self
    }
    /// Current full-scale range.
    pub fn gain(&self) -> Gain {
        self.gain
    }
}

impl<M: HasComparator> Configuration<M> {
    /// Select comparator threshold interpretation.
    pub fn with_comparator_mode(mut self, value: ComparatorMode) -> Self {
        self.comparator_mode = value;
        self
    }
    /// Current comparator threshold interpretation.
    pub fn comparator_mode(&self) -> ComparatorMode {
        self.comparator_mode
    }
    /// Select comparator asserted output level.
    pub fn with_comparator_polarity(mut self, value: ComparatorPolarity) -> Self {
        self.comparator_polarity = value;
        self
    }
    /// Current comparator asserted output level.
    pub fn comparator_polarity(&self) -> ComparatorPolarity {
        self.comparator_polarity
    }
    /// Select comparator latching behavior.
    pub fn with_comparator_latch(mut self, value: ComparatorLatch) -> Self {
        self.comparator_latch = value;
        self
    }
    /// Current comparator latching behavior.
    pub fn comparator_latch(&self) -> ComparatorLatch {
        self.comparator_latch
    }
    /// Select comparator consecutive-sample count or disable the output.
    pub fn with_comparator_queue(mut self, value: ComparatorQueue) -> Self {
        self.comparator_queue = value;
        self
    }
    /// Current comparator consecutive-sample count or disable the output.
    pub fn comparator_queue(&self) -> ComparatorQueue {
        self.comparator_queue
    }
}

#[cfg(feature = "texas_instruments-ads1115")]
impl Configuration<super::models::ADS1115> {
    /// Select the positive and negative inputs.
    pub fn with_input(mut self, input: InputChannel) -> Self {
        self.input = input;
        self
    }
    /// Current input selection.
    pub fn input(&self) -> InputChannel {
        self.input
    }
}
