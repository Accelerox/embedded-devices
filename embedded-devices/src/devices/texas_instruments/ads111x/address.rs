//! Address selection through the ADS111x ADDR pin.

/// ADDR wiring or a translated seven-bit address.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Address {
    /// ADDR connected to GND: 0x48.
    Gnd,
    /// ADDR connected to VDD: 0x49.
    Vdd,
    /// ADDR connected to SDA: 0x4A.
    Sda,
    /// ADDR connected to SCL: 0x4B.
    Scl,
    /// Address supplied by an I²C translator.
    Custom(u8),
}

impl From<Address> for u8 {
    fn from(address: Address) -> Self {
        match address {
            Address::Gnd => 0x48,
            Address::Vdd => 0x49,
            Address::Sda => 0x4A,
            Address::Scl => 0x4B,
            Address::Custom(address) => address,
        }
    }
}
