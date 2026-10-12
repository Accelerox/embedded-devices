//! Sealed model markers and capabilities for the shared driver.

mod private {
    pub trait Sealed {}
}

/// A supported ADS111x model; downstream implementations are prohibited.
pub trait Model: private::Sealed + Copy + core::fmt::Debug + PartialEq {
    /// Whether the model has an input multiplexer.
    #[doc(hidden)]
    const HAS_MUX: bool;
    /// Whether the model has programmable gain.
    #[doc(hidden)]
    const HAS_PGA: bool;
    /// Whether the model has a comparator and ALERT/RDY output.
    #[doc(hidden)]
    const HAS_COMPARATOR: bool;
}

/// Models with programmable gain: ADS1114 and ADS1115.
pub trait HasPga: Model {}
/// Models with comparator and ALERT/RDY: ADS1114 and ADS1115.
pub trait HasComparator: HasPga {}

/// ADS1113 hardware capabilities.
#[cfg(feature = "texas_instruments-ads1113")]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ADS1113;

#[cfg(feature = "texas_instruments-ads1113")]
impl private::Sealed for ADS1113 {}
#[cfg(feature = "texas_instruments-ads1113")]
impl Model for ADS1113 {
    const HAS_MUX: bool = false;
    const HAS_PGA: bool = false;
    const HAS_COMPARATOR: bool = false;
}

/// ADS1114 hardware capabilities.
#[cfg(feature = "texas_instruments-ads1114")]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ADS1114;

#[cfg(feature = "texas_instruments-ads1114")]
impl private::Sealed for ADS1114 {}
#[cfg(feature = "texas_instruments-ads1114")]
impl Model for ADS1114 {
    const HAS_MUX: bool = false;
    const HAS_PGA: bool = true;
    const HAS_COMPARATOR: bool = true;
}
#[cfg(feature = "texas_instruments-ads1114")]
impl HasPga for ADS1114 {}
#[cfg(feature = "texas_instruments-ads1114")]
impl HasComparator for ADS1114 {}

/// ADS1115 hardware capabilities.
#[cfg(feature = "texas_instruments-ads1115")]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ADS1115;

#[cfg(feature = "texas_instruments-ads1115")]
impl private::Sealed for ADS1115 {}
#[cfg(feature = "texas_instruments-ads1115")]
impl Model for ADS1115 {
    const HAS_MUX: bool = true;
    const HAS_PGA: bool = true;
    const HAS_COMPARATOR: bool = true;
}
#[cfg(feature = "texas_instruments-ads1115")]
impl HasPga for ADS1115 {}
#[cfg(feature = "texas_instruments-ads1115")]
impl HasComparator for ADS1115 {}
