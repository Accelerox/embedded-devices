#[cfg(any(
    feature = "texas_instruments-ads1113",
    feature = "texas_instruments-ads1114",
    feature = "texas_instruments-ads1115"
))]
pub mod ads111x;
#[cfg(feature = "texas_instruments-ads7830")]
pub mod ads7830;
#[cfg(feature = "texas_instruments-ina219")]
pub mod ina219;
#[cfg(feature = "texas_instruments-ina226")]
pub mod ina226;
#[cfg(feature = "texas_instruments-ina228")]
pub mod ina228;
#[cfg(feature = "texas_instruments-tmp102")]
pub mod tmp102;
#[cfg(feature = "texas_instruments-tmp117")]
pub mod tmp117;
