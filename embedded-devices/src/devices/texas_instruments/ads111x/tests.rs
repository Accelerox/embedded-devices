//! Observable transactions, state changes, and errors in both generated modes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
extern crate std;

use super::*;
use embedded_hal::i2c::Operation;
use std::{cell::Cell, collections::VecDeque, rc::Rc, vec, vec::Vec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fault {
    Bus,
    Gpio,
}
impl embedded_hal::i2c::Error for Fault {
    fn kind(&self) -> embedded_hal::i2c::ErrorKind {
        embedded_hal::i2c::ErrorKind::Other
    }
}
impl embedded_hal::digital::Error for Fault {
    fn kind(&self) -> embedded_hal::digital::ErrorKind {
        embedded_hal::digital::ErrorKind::Other
    }
}

struct Step {
    write: Vec<u8>,
    read: Option<Vec<u8>>,
    error: bool,
}
impl Step {
    fn write(bytes: &[u8]) -> Self {
        Self {
            write: bytes.into(),
            read: None,
            error: false,
        }
    }
    fn read(register: u8, value: u16) -> Self {
        Self {
            write: vec![register],
            read: Some(value.to_be_bytes().into()),
            error: false,
        }
    }
    fn fail(mut self) -> Self {
        self.error = true;
        self
    }
}
struct Bus {
    steps: VecDeque<Step>,
    address: u8,
}
impl Bus {
    fn new(steps: Vec<Step>) -> Self {
        Self {
            steps: steps.into(),
            address: 0x48,
        }
    }
    fn done(&self) {
        assert!(self.steps.is_empty(), "unused bus steps");
    }
    fn transfer(&mut self, address: u8, operations: &mut [Operation<'_>]) -> Result<(), Fault> {
        assert_eq!(address, self.address);
        let step = self.steps.pop_front().expect("unexpected bus access");
        match operations {
            [Operation::Write(bytes)] => {
                assert_eq!(*bytes, step.write);
                assert!(step.read.is_none());
            }
            [Operation::Write(bytes), Operation::Read(read)] => {
                assert_eq!(*bytes, step.write);
                read.copy_from_slice(&step.read.expect("expected read"));
            }
            _ => panic!("unexpected transaction"),
        }
        if step.error { Err(Fault::Bus) } else { Ok(()) }
    }
}
impl embedded_hal::i2c::ErrorType for Bus {
    type Error = Fault;
}
#[cfg(feature = "sync")]
impl embedded_hal::i2c::I2c for Bus {
    fn transaction(&mut self, address: u8, operations: &mut [Operation<'_>]) -> Result<(), Fault> {
        self.transfer(address, operations)
    }
}
#[cfg(feature = "async")]
impl embedded_hal_async::i2c::I2c for Bus {
    async fn transaction(&mut self, address: u8, operations: &mut [Operation<'_>]) -> Result<(), Fault> {
        self.transfer(address, operations)
    }
}

struct Pin {
    level: Rc<Cell<bool>>,
    transitions: VecDeque<bool>,
    error: bool,
}
impl Pin {
    fn new(high: bool) -> Self {
        Self {
            level: Rc::new(Cell::new(high)),
            transitions: VecDeque::new(),
            error: false,
        }
    }
    fn sample(&mut self) -> Result<bool, Fault> {
        if self.error {
            return Err(Fault::Gpio);
        }
        if let Some(level) = self.transitions.pop_front() {
            self.level.set(level);
        }
        Ok(self.level.get())
    }
}
impl embedded_hal::digital::ErrorType for Pin {
    type Error = Fault;
}
#[cfg(feature = "sync")]
impl embedded_hal::digital::InputPin for Pin {
    fn is_high(&mut self) -> Result<bool, Fault> {
        self.sample()
    }
    fn is_low(&mut self) -> Result<bool, Fault> {
        self.sample().map(|high| !high)
    }
}
#[cfg(feature = "async")]
impl Pin {
    async fn wait(&mut self, high: bool) -> Result<(), Fault> {
        core::future::poll_fn(|_| match self.sample() {
            Ok(level) if level == high => core::task::Poll::Ready(Ok(())),
            Ok(_) => core::task::Poll::Pending,
            Err(error) => core::task::Poll::Ready(Err(error)),
        })
        .await
    }
}
#[cfg(feature = "async")]
impl embedded_hal_async::digital::Wait for Pin {
    async fn wait_for_high(&mut self) -> Result<(), Fault> {
        self.wait(true).await
    }
    async fn wait_for_low(&mut self) -> Result<(), Fault> {
        self.wait(false).await
    }
    async fn wait_for_rising_edge(&mut self) -> Result<(), Fault> {
        panic!("unexpected edge wait")
    }
    async fn wait_for_falling_edge(&mut self) -> Result<(), Fault> {
        panic!("unexpected edge wait")
    }
    async fn wait_for_any_edge(&mut self) -> Result<(), Fault> {
        panic!("unexpected edge wait")
    }
}

#[cfg(feature = "async")]
fn complete(future: impl core::future::Future<Output = ()>) {
    use core::task::{Context, Poll, Waker};
    let mut future = core::pin::pin!(future);
    assert!(matches!(
        future.as_mut().poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(())
    ));
}

macro_rules! test_both {
    ($blocking:ident, $asynchronous:ident, $test_sync:ident, $test_async:ident) => {
        #[cfg(feature = "sync")]
        #[test]
        fn $test_sync() {
            $blocking();
        }
        #[cfg(feature = "async")]
        #[test]
        fn $test_async() {
            complete($asynchronous());
        }
    };
}

#[test]
fn address_pin_mapping() {
    assert_eq!(u8::from(address::Address::Gnd), 0x48);
    assert_eq!(u8::from(address::Address::Vdd), 0x49);
    assert_eq!(u8::from(address::Address::Sda), 0x4A);
    assert_eq!(u8::from(address::Address::Scl), 0x4B);
    assert_eq!(u8::from(address::Address::Custom(0x20)), 0x20);
}

#[cfg(feature = "texas_instruments-ads1113")]
#[maybe_async_cfg::maybe(idents(ADS1113, ADS1114, ADS1115), sync(feature = "sync"), async(feature = "async"))]
async fn check_fixed_range() {
    use super::ADS1113;
    let configuration = Configuration::default().with_data_rate(DataRate::Sps860);
    assert_eq!(configuration.bits(), 0x05E3);
    let mut adc = ADS1113::new(
        Bus::new(vec![
            Step::write(&[1, 0x85, 0xE3]),
            Step::read(1, 0x05E3),
            Step::read(1, 0x85E3),
            Step::read(0, 0x8000),
            Step::read(0, 0x7FFF),
            Step::read(0, 0),
            Step::read(1, 0xFFFF),
            Step::read(0, 0x4000),
        ]),
        address::Address::Gnd,
        configuration,
    )
    .unwrap();
    assert_eq!(adc.convert().await.unwrap().get::<volt>(), -2.048);
    assert_eq!(adc.read().await.unwrap().get::<volt>(), 2.048 * 32767.0 / 32768.0);
    assert_eq!(adc.read_raw().await.unwrap(), 0);
    // Reserved MUX/PGA/comparator bits cannot change ADS1113 interpretation.
    assert_eq!(adc.read_configuration().await.unwrap().bits(), 0x05E3);
    assert_eq!(adc.read().await.unwrap().get::<volt>(), 1.024);
    adc.destroy().done();
}
#[cfg(feature = "texas_instruments-ads1113")]
test_both!(
    check_fixed_range_sync,
    check_fixed_range_async,
    fixed_range_sync,
    fixed_range_async
);

#[cfg(feature = "texas_instruments-ads1114")]
#[maybe_async_cfg::maybe(idents(ADS1113, ADS1114, ADS1115), sync(feature = "sync"), async(feature = "async"))]
async fn check_fixed_inputs() {
    use super::ADS1114;
    let configuration = Configuration::default().with_gain(Gain::V4_096);
    assert_eq!(configuration.bits(), 0x0383);
    let mut adc = ADS1114::new(
        Bus::new(vec![
            Step::write(&[1, 0x83, 0x83]),
            Step::read(1, 0x8383),
            Step::read(0, 0x4000),
            Step::read(1, 0xF183),
            Step::read(0, 0x4000),
        ]),
        0x48,
        configuration,
    )
    .unwrap();
    assert_eq!(adc.convert().await.unwrap().get::<volt>(), 2.048);
    // ADS1114 ignores MUX bits while PGA remains meaningful.
    assert_eq!(adc.read_configuration().await.unwrap().bits(), 0x0183);
    assert_eq!(adc.read().await.unwrap().get::<volt>(), 3.072);
    adc.destroy().done();
    let mut adc = ADS1114::new(
        Bus::new(vec![
            Step::write(&[2, 0, 0]),
            Step::write(&[3, 0x80, 0]),
            Step::write(&[1, 0x03, 0x80]),
            Step::write(&[1, 0x83, 0x80]),
            Step::read(0, 0x4000),
        ]),
        0x48,
        configuration,
    )
    .unwrap()
    .with_alert_ready(Pin::new(false));
    adc.configure_ready().await.unwrap();
    assert_eq!(adc.convert_raw().await.unwrap(), 0x4000);
    adc.destroy().0.done();
}
#[cfg(feature = "texas_instruments-ads1114")]
test_both!(
    check_fixed_inputs_sync,
    check_fixed_inputs_async,
    fixed_inputs_sync,
    fixed_inputs_async
);

#[cfg(feature = "texas_instruments-ads1115")]
#[maybe_async_cfg::maybe(idents(ADS1113, ADS1114, ADS1115), sync(feature = "sync"), async(feature = "async"))]
async fn check_conversions() {
    use super::ADS1115;
    let configuration = Configuration::default().with_gain(Gain::V4_096);
    let mut adc = ADS1115::new(
        Bus::new(vec![
            Step::write(&[1, 0xC3, 0x83]),
            Step::read(1, 0x4383),
            Step::read(1, 0xC383),
            Step::read(0, 0x4000),
            Step::read(0, 0x8000),
            Step::read(0, 0x7FFF),
            Step::write(&[1, 0xF3, 0x83]),
            Step::read(1, 0xF383),
            Step::read(0, 0xFFFF),
        ]),
        0x48,
        configuration,
    )
    .unwrap();
    assert_eq!(adc.convert(InputChannel::Single0).await.unwrap().get::<volt>(), 2.048);
    assert_eq!(adc.read().await.unwrap().get::<volt>(), -4.096);
    assert_eq!(adc.read_raw().await.unwrap(), i16::MAX);
    assert_eq!(adc.convert_raw(InputChannel::Single3).await.unwrap(), -1);
    adc.destroy().done();

    let continuous = configuration
        .with_mode(Mode::Continuous)
        .with_input(InputChannel::Single1);
    let mut adc = ADS1115::new(
        Bus::new(vec![
            Step::write(&[1, 0x52, 0x83]), // Apply continuous configuration.
            Step::read(0, 0x4000),         // Immediate read does not inspect OS.
            Step::write(&[1, 0x53, 0x83]), // Stop continuous conversion.
            Step::read(1, 0x5383),
            Step::read(1, 0xD383), // Drain in-flight conversion.
            Step::write(&[1, 0xE3, 0x83]),
            Step::read(1, 0xE383),
            Step::read(0, 0x1234),
        ]),
        0x48,
        continuous,
    )
    .unwrap();
    adc.write_configuration(continuous).await.unwrap();
    assert_eq!(adc.read_raw().await.unwrap(), 0x4000);
    assert_eq!(adc.convert_raw(InputChannel::Single2).await.unwrap(), 0x1234);
    adc.destroy().done();
}
#[cfg(feature = "texas_instruments-ads1115")]
test_both!(
    check_conversions_sync,
    check_conversions_async,
    conversions_sync,
    conversions_async
);

#[cfg(feature = "texas_instruments-ads1115")]
#[maybe_async_cfg::maybe(idents(ADS1113, ADS1114, ADS1115), sync(feature = "sync"), async(feature = "async"))]
async fn check_configuration() {
    use super::ADS1115;
    for (input, high) in [
        (InputChannel::Diff01, 0x85),
        (InputChannel::Diff03, 0x95),
        (InputChannel::Diff13, 0xA5),
        (InputChannel::Diff23, 0xB5),
        (InputChannel::Single0, 0xC5),
        (InputChannel::Single1, 0xD5),
        (InputChannel::Single2, 0xE5),
        (InputChannel::Single3, 0xF5),
    ] {
        let mut adc = ADS1115::new(
            Bus::new(vec![Step::write(&[1, high, 0x83])]),
            0x48,
            Configuration::default(),
        )
        .unwrap();
        adc.start_single_conversion(input).await.unwrap();
        assert_eq!(adc.configuration.input(), input);
        adc.destroy().done();
    }
    // Read failures and OS-only status reads leave cached gain unchanged.
    let original = Configuration::default();
    let changed = original.with_gain(Gain::V4_096);
    let mut adc = ADS1115::new(
        Bus::new(vec![
            Step::write(&[1, 0x03, 0x83]).fail(),
            Step::read(0, 0x4000),
            Step::read(1, 0x8383).fail(),
            Step::read(1, 0x8383),
            Step::read(0, 0x4000),
            Step::write(&[1, 0xC5, 0x83]).fail(),
            Step::read(1, 0xE383),
            Step::read(0, 0x4000),
        ]),
        0x48,
        original,
    )
    .unwrap();
    assert_eq!(adc.write_configuration(changed).await, Err(Fault::Bus));
    assert_eq!(adc.configuration, original);
    assert_eq!(adc.read().await.unwrap().get::<volt>(), 1.024);
    assert_eq!(adc.read_configuration().await, Err(Fault::Bus));
    assert!(adc.is_idle().await.unwrap());
    assert_eq!(adc.configuration, original);
    assert_eq!(adc.read().await.unwrap().get::<volt>(), 1.024);
    assert_eq!(
        adc.start_single_conversion(InputChannel::Single0).await,
        Err(Fault::Bus)
    );
    assert_eq!(adc.configuration, original);
    assert_eq!(adc.read_configuration().await.unwrap().input(), InputChannel::Single2);
    assert_eq!(adc.read().await.unwrap().get::<volt>(), 2.048);
    adc.destroy().done();

    let mut bus = Bus::new(vec![Step::read(0, 0)]);
    bus.address = 0x20;
    let mut adc = ADS1115::new(bus, address::Address::Custom(0x20), original).unwrap();
    assert_eq!(adc.read_raw().await.unwrap(), 0);
    adc.destroy().done();
    let invalid = ADS1115::new(Bus::new(vec![]), 0x80, original);
    assert!(matches!(invalid, Err(ConfigError::InvalidAddress)));
    ADS1115::new(Bus::new(vec![]), 0x7F, original).unwrap().destroy().done();
}
#[cfg(feature = "texas_instruments-ads1115")]
test_both!(
    check_configuration_sync,
    check_configuration_async,
    configuration_sync,
    configuration_async
);

#[cfg(feature = "texas_instruments-ads1115")]
#[maybe_async_cfg::maybe(idents(ADS1113, ADS1114, ADS1115), sync(feature = "sync"), async(feature = "async"))]
async fn check_thresholds() {
    use super::ADS1115;
    for gain in [
        Gain::V6_144,
        Gain::V4_096,
        Gain::V2_048,
        Gain::V1_024,
        Gain::V0_512,
        Gain::V0_256,
    ] {
        let configuration = Configuration::default().with_gain(gain);
        let mut adc = ADS1115::new(
            Bus::new(vec![
                Step::write(&[2, 0x80, 0]),
                Step::write(&[3, 0x7F, 0xFF]),
                Step::write(&[2, 0, 1]),
                Step::write(&[3, 0xFF, 0xFF]),
                Step::read(0, 0x8000),
                Step::read(0, 0x7FFF),
            ]),
            0x48,
            configuration,
        )
        .unwrap();
        let fs = gain.full_scale_voltage();
        adc.set_alert_low(-fs).await.unwrap();
        adc.set_alert_high(fs * (32767.0 / 32768.0)).await.unwrap();
        adc.set_alert_low(fs * (0.5 / 32768.0)).await.unwrap();
        adc.set_alert_high(fs * (-0.5 / 32768.0)).await.unwrap();
        assert_eq!(adc.read().await.unwrap(), -fs);
        assert_eq!(adc.read().await.unwrap(), fs * (32767.0 / 32768.0));
        for voltage in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            fs.get::<volt>(),
            -fs.get::<volt>() - 0.001,
        ] {
            assert_eq!(
                adc.set_alert_low(ElectricPotential::new::<volt>(voltage)).await,
                Err(ThresholdError::InvalidVoltage)
            );
            assert_eq!(
                adc.set_alert_high(ElectricPotential::new::<volt>(voltage)).await,
                Err(ThresholdError::InvalidVoltage)
            );
        }
        adc.destroy().done();
    }
    let mut adc = ADS1115::new(
        Bus::new(vec![Step::write(&[3, 0, 0]).fail()]),
        0x48,
        Configuration::default(),
    )
    .unwrap();
    assert_eq!(
        adc.set_alert_high(ElectricPotential::new::<volt>(0.0)).await,
        Err(ThresholdError::I2c(Fault::Bus))
    );
    adc.destroy().done();
}
#[cfg(feature = "texas_instruments-ads1115")]
test_both!(
    check_thresholds_sync,
    check_thresholds_async,
    thresholds_sync,
    thresholds_async
);

#[cfg(feature = "texas_instruments-ads1115")]
#[maybe_async_cfg::maybe(idents(ADS1113, ADS1114, ADS1115), sync(feature = "sync"), async(feature = "async"))]
async fn check_ready_conversion() {
    use super::ADS1115;
    for polarity in [ComparatorPolarity::ActiveLow, ComparatorPolarity::ActiveHigh] {
        let configuration = Configuration::default().with_comparator_polarity(polarity);
        let asserted = polarity == ComparatorPolarity::ActiveHigh;
        let low = if asserted { 0x88 } else { 0x80 };
        let mut adc = ADS1115::new(
            Bus::new(vec![
                Step::write(&[2, 0, 0]),
                Step::write(&[3, 0x80, 0]),
                Step::write(&[1, 0x05, low]),
                Step::write(&[1, 0xC5, low]),
                Step::read(0, 0x4000),
            ]),
            0x48,
            configuration,
        )
        .unwrap()
        .with_alert_ready(Pin::new(asserted));
        assert_eq!(
            adc.convert_raw(InputChannel::Single0).await,
            Err(ReadyError::NotConfigured)
        );
        adc.configure_ready().await.unwrap();
        assert_eq!(adc.convert(InputChannel::Single0).await.unwrap().get::<volt>(), 1.024);
        let (bus, pin) = adc.destroy();
        bus.done();
        assert_eq!(pin.level.get(), asserted);
    }

    let configuration = Configuration::default()
        .with_input(InputChannel::Single3)
        .with_gain(Gain::V4_096)
        .with_data_rate(DataRate::Sps860)
        .with_comparator_mode(ComparatorMode::Window)
        .with_comparator_latch(ComparatorLatch::Latching)
        .with_comparator_polarity(ComparatorPolarity::ActiveHigh);
    let mut adc = ADS1115::new(
        Bus::new(vec![
            Step::write(&[2, 0, 0]),
            Step::write(&[3, 0x80, 0]),
            Step::write(&[1, 0x73, 0xFC]),
        ]),
        0x48,
        configuration,
    )
    .unwrap();
    adc.configure_ready().await.unwrap();
    assert_eq!(
        adc.configuration,
        configuration.with_comparator_queue(ComparatorQueue::AfterOne)
    );
    adc.destroy().done();

    for failed in 0..3 {
        let mut steps = vec![
            Step::write(&[2, 0, 0]),
            Step::write(&[3, 0x80, 0]),
            Step::write(&[1, 0x05, 0x80]),
        ];
        steps.truncate(failed + 1);
        steps[failed].error = true;
        let mut adc = ADS1115::new(Bus::new(steps), 0x48, Configuration::default())
            .unwrap()
            .with_alert_ready(Pin::new(false));
        assert_eq!(adc.configure_ready().await, Err(Fault::Bus));
        assert_eq!(adc.wait_conversion_ready().await, Err(ReadyError::NotConfigured));
        adc.destroy().0.done();
    }
}
#[cfg(feature = "texas_instruments-ads1115")]
test_both!(
    check_ready_conversion_sync,
    check_ready_conversion_async,
    ready_conversion_sync,
    ready_conversion_async
);

#[cfg(feature = "texas_instruments-ads1115")]
#[maybe_async_cfg::maybe(idents(ADS1113, ADS1114, ADS1115), sync(feature = "sync"), async(feature = "async"))]
async fn check_ready_invalidation() {
    use super::ADS1115;
    let configuration = Configuration::default();
    let enabled = configuration.with_comparator_queue(ComparatorQueue::AfterOne);
    let mut adc = ADS1115::new(
        Bus::new(vec![
            Step::write(&[2, 0, 0]),
            Step::write(&[3, 0x80, 0]),
            Step::write(&[1, 0x05, 0x80]),
            Step::write(&[1, 0x03, 0x80]),  // Gain-only update preserves readiness.
            Step::write(&[2, 0, 0]).fail(), // Failed threshold write invalidates readiness.
            Step::write(&[2, 0, 0]),
            Step::write(&[3, 0x80, 0]),
            Step::write(&[1, 0x03, 0x80]),
            Step::write(&[1, 0x03, 0x88]).fail(), // Failed polarity update invalidates readiness.
        ]),
        0x48,
        configuration,
    )
    .unwrap()
    .with_alert_ready(Pin::new(false));
    adc.configure_ready().await.unwrap();
    assert_eq!(
        adc.set_alert_low(ElectricPotential::new::<volt>(f64::NAN)).await,
        Err(ThresholdError::InvalidVoltage)
    );
    adc.wait_conversion_ready().await.unwrap();
    adc.write_configuration(enabled.with_gain(Gain::V4_096)).await.unwrap();
    adc.wait_conversion_ready().await.unwrap();
    assert_eq!(adc.set_alert_low_raw(0).await, Err(Fault::Bus));
    assert_eq!(adc.wait_conversion_ready().await, Err(ReadyError::NotConfigured));
    adc.configure_ready().await.unwrap();
    let new = adc
        .configuration
        .with_comparator_polarity(ComparatorPolarity::ActiveHigh);
    assert_eq!(adc.write_configuration(new).await, Err(Fault::Bus));
    assert_eq!(adc.wait_conversion_ready().await, Err(ReadyError::NotConfigured));
    adc.destroy().0.done();

    let mut adc = ADS1115::new(
        Bus::new(vec![
            Step::write(&[2, 0, 0]),
            Step::write(&[3, 0x80, 0]),
            Step::write(&[1, 0x05, 0x80]),
            Step::write(&[1, 0x04, 0x80]),
            Step::write(&[1, 0x05, 0x83]),
        ]),
        0x48,
        configuration,
    )
    .unwrap()
    .with_alert_ready(Pin::new(false));
    adc.configure_ready().await.unwrap();
    adc.write_configuration(enabled.with_mode(Mode::Continuous))
        .await
        .unwrap();
    assert_eq!(adc.wait_conversion_ready().await, Err(ReadyError::NotSingleShot));
    adc.write_configuration(configuration).await.unwrap();
    assert_eq!(adc.wait_conversion_ready().await, Err(ReadyError::NotConfigured));
    adc.destroy().0.done();
}
#[cfg(feature = "texas_instruments-ads1115")]
test_both!(
    check_ready_invalidation_sync,
    check_ready_invalidation_async,
    ready_invalidation_sync,
    ready_invalidation_async
);

#[cfg(feature = "texas_instruments-ads1115")]
#[maybe_async_cfg::maybe(idents(ADS1113, ADS1114, ADS1115), sync(feature = "sync"), async(feature = "async"))]
async fn check_errors_and_alerts() {
    use super::ADS1115;
    for failed in 0..3 {
        let mut steps = vec![Step::write(&[1, 0xC5, 0x83]), Step::read(1, 0xC583), Step::read(0, 0)];
        steps.truncate(failed + 1);
        steps[failed].error = true;
        let mut adc = ADS1115::new(Bus::new(steps), 0x48, Configuration::default()).unwrap();
        assert_eq!(adc.convert_raw(InputChannel::Single0).await, Err(Fault::Bus));
        adc.destroy().done();
    }
    for failed in 0..2 {
        let mut steps = vec![
            Step::write(&[2, 0, 0]),
            Step::write(&[3, 0x80, 0]),
            Step::write(&[1, 0x05, 0x80]),
            Step::write(&[1, 0xC5, 0x80]),
            Step::read(0, 0),
        ];
        steps.truncate(4 + failed);
        steps[3 + failed].error = true;
        let mut adc = ADS1115::new(Bus::new(steps), 0x48, Configuration::default())
            .unwrap()
            .with_alert_ready(Pin::new(false));
        adc.configure_ready().await.unwrap();
        assert_eq!(
            adc.convert_raw(InputChannel::Single0).await,
            Err(ReadyError::I2c(Fault::Bus))
        );
        adc.destroy().0.done();
    }
    let mut pin = Pin::new(false);
    pin.error = true;
    let mut adc = ADS1115::new(
        Bus::new(vec![
            Step::write(&[2, 0, 0]),
            Step::write(&[3, 0x80, 0]),
            Step::write(&[1, 0x05, 0x80]),
            Step::write(&[1, 0xC5, 0x80]),
        ]),
        0x48,
        Configuration::default(),
    )
    .unwrap()
    .with_alert_ready(pin);
    adc.configure_ready().await.unwrap();
    assert_eq!(
        adc.convert_raw(InputChannel::Single0).await,
        Err(ReadyError::Pin(Fault::Gpio))
    );
    adc.destroy().0.done();

    for polarity in [ComparatorPolarity::ActiveLow, ComparatorPolarity::ActiveHigh] {
        let active = polarity == ComparatorPolarity::ActiveHigh;
        let configuration = Configuration::default().with_comparator_polarity(polarity);
        let mut adc = ADS1115::new(Bus::new(vec![]), 0x48, configuration)
            .unwrap()
            .with_alert_ready(Pin::new(active));
        adc.wait_alert_asserted().await.unwrap();
        adc.ready.pin.level.set(!active);
        adc.wait_alert_cleared().await.unwrap();
        adc.ready.pin.error = true;
        assert_eq!(adc.wait_alert_asserted().await, Err(Fault::Gpio));
        adc.destroy().0.done();
    }
}
#[cfg(feature = "texas_instruments-ads1115")]
test_both!(
    check_errors_and_alerts_sync,
    check_errors_and_alerts_async,
    errors_and_alerts_sync,
    errors_and_alerts_async
);

#[cfg(all(feature = "async", feature = "texas_instruments-ads1115"))]
#[test]
fn gpio_waits_remain_pending_until_the_requested_level() {
    use core::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    let mut adc = ADS1115Async::new(
        Bus::new(vec![
            Step::write(&[2, 0, 0]),
            Step::write(&[3, 0x80, 0]),
            Step::write(&[1, 0x05, 0x80]),
        ]),
        0x48,
        Configuration::default(),
    )
    .unwrap()
    .with_alert_ready(Pin::new(true));
    complete(async {
        adc.configure_ready().await.unwrap();
    });
    let level = adc.ready.pin.level.clone();
    let mut context = Context::from_waker(Waker::noop());
    {
        let mut future = core::pin::pin!(adc.wait_conversion_ready());
        assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
        level.set(false);
        assert!(matches!(future.as_mut().poll(&mut context), Poll::Ready(Ok(()))));
    }
    {
        let mut future = core::pin::pin!(adc.wait_alert_cleared());
        assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
        level.set(true);
        assert!(matches!(future.as_mut().poll(&mut context), Poll::Ready(Ok(()))));
    }
    adc.destroy().0.done();
}

#[cfg(all(feature = "sync", feature = "texas_instruments-ads1115"))]
#[test]
fn blocking_pin_wait_polls_levels_without_i2c() {
    let mut pin = Pin::new(true);
    pin.transitions = [true, true, false].into();
    let mut adc = ADS1115Sync::new(
        Bus::new(vec![
            Step::write(&[2, 0, 0]),
            Step::write(&[3, 0x80, 0]),
            Step::write(&[1, 0x05, 0x80]),
        ]),
        0x48,
        Configuration::default(),
    )
    .unwrap()
    .with_alert_ready(pin);
    adc.configure_ready().unwrap();
    adc.wait_conversion_ready().unwrap();
    let (bus, pin) = adc.destroy();
    bus.done();
    assert!(pin.transitions.is_empty());
}
