//! Check observable bus and timing behavior in both generated variants.
#![allow(clippy::expect_used, clippy::panic)]

extern crate std;

use super::{ArgumentError, ConfigError, DisplayControl, EntryDirection, Error, ShiftDirection};
use embedded_hal::i2c::{ErrorKind, Operation};
use std::{cell::RefCell, rc::Rc, vec::Vec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Write(u8, u8),
    Delay(u32),
}

type Events = Rc<RefCell<Vec<Event>>>;

struct Bus {
    events: Events,
    writes: usize,
    fail_at: Option<usize>,
}

impl embedded_hal::i2c::ErrorType for Bus {
    type Error = ErrorKind;
}

impl Bus {
    fn transfer(&mut self, address: u8, operations: &mut [Operation<'_>]) -> Result<(), ErrorKind> {
        let [Operation::Write(bytes)] = operations else {
            panic!("expected one write");
        };
        let [byte] = **bytes else {
            panic!("expected raw byte");
        };
        self.events.borrow_mut().push(Event::Write(address, byte));
        let fail = self.fail_at == Some(self.writes);
        self.writes += 1;
        if fail { Err(ErrorKind::Other) } else { Ok(()) }
    }
}

#[cfg(feature = "sync")]
impl embedded_hal::i2c::I2c for Bus {
    fn transaction(&mut self, address: u8, operations: &mut [Operation<'_>]) -> Result<(), ErrorKind> {
        self.transfer(address, operations)
    }
}

#[cfg(feature = "async")]
impl embedded_hal_async::i2c::I2c for Bus {
    async fn transaction(&mut self, address: u8, operations: &mut [Operation<'_>]) -> Result<(), ErrorKind> {
        self.transfer(address, operations)
    }
}

struct Delay(Events);

#[cfg(feature = "sync")]
impl embedded_hal::delay::DelayNs for Delay {
    fn delay_ns(&mut self, ns: u32) {
        self.0.borrow_mut().push(Event::Delay(ns));
    }
}

#[cfg(feature = "async")]
impl embedded_hal_async::delay::DelayNs for Delay {
    async fn delay_ns(&mut self, ns: u32) {
        self.0.borrow_mut().push(Event::Delay(ns));
    }
}

fn hardware(fail_at: Option<usize>) -> (Bus, Delay, Events) {
    let events = Rc::new(RefCell::new(Vec::new()));
    (
        Bus {
            events: events.clone(),
            writes: 0,
            fail_at,
        },
        Delay(events.clone()),
        events,
    )
}

// Expected output levels are supplied explicitly, independently of driver encoders.
fn nibble(expected: &mut Vec<Event>, low: u8, high: u8) {
    expected.extend([
        Event::Write(0x3F, low),
        Event::Write(0x3F, high),
        Event::Delay(1_000_000),
        Event::Write(0x3F, low),
        Event::Delay(1_000_000),
    ]);
}

fn byte(expected: &mut Vec<Event>, levels: [u8; 4]) {
    nibble(expected, levels[0], levels[1]);
    nibble(expected, levels[2], levels[3]);
    expected.push(Event::Delay(2_000_000));
}

#[maybe_async_cfg::maybe(idents(HD44780PCF8574), sync(feature = "sync"), async(feature = "async"))]
async fn exercise() {
    use super::HD44780PCF8574;

    let (bus, delay, events) = hardware(None);
    let invalid = HD44780PCF8574::new(bus, delay, 0x80);
    assert!(matches!(invalid, Err(ConfigError::InvalidAddress)));
    assert!(events.borrow().is_empty());
    let (bus, delay, events) = hardware(None);
    let mut lcd = HD44780PCF8574::new(bus, delay, 0x3F).expect("valid address");
    assert!(events.borrow().is_empty());
    lcd.init().await.expect("initialization");
    let mut expected = Vec::new();
    expected.extend([Event::Write(0x3F, 0x08), Event::Delay(50_000_000)]);
    for _ in 0..3 {
        nibble(&mut expected, 0x38, 0x3C);
        expected.push(Event::Delay(5_000_000));
    }
    nibble(&mut expected, 0x28, 0x2C);
    for levels in [
        [0x28, 0x2C, 0x88, 0x8C], // Function set: 0x28.
        [0x08, 0x0C, 0x88, 0x8C], // Display off: 0x08.
        [0x08, 0x0C, 0x18, 0x1C], // Clear: 0x01.
        [0x08, 0x0C, 0x68, 0x6C], // Entry mode: 0x06.
        [0x08, 0x0C, 0xC8, 0xCC], // Display on: 0x0C.
    ] {
        byte(&mut expected, levels);
    }
    assert_eq!(*events.borrow(), expected);

    events.borrow_mut().clear();
    expected.clear();
    lcd.set_cursor(15, 1).await.expect("cursor");
    byte(&mut expected, [0xC8, 0xCC, 0xF8, 0xFC]);
    lcd.write_bytes(&[0xA5, 0x00, 0xFF]).await.expect("raw data");
    for levels in [
        [0xA9, 0xAD, 0x59, 0x5D],
        [0x09, 0x0D, 0x09, 0x0D],
        [0xF9, 0xFD, 0xF9, 0xFD],
    ] {
        byte(&mut expected, levels);
    }
    lcd.clear().await.expect("clear");
    byte(&mut expected, [0x08, 0x0C, 0x18, 0x1C]);
    lcd.home().await.expect("home");
    byte(&mut expected, [0x08, 0x0C, 0x28, 0x2C]);
    lcd.set_cgram_address(0x3F).await.expect("CGRAM");
    byte(&mut expected, [0x78, 0x7C, 0xF8, 0xFC]);
    lcd.set_ddram_address(0x27).await.expect("off-screen DDRAM");
    byte(&mut expected, [0xA8, 0xAC, 0x78, 0x7C]);
    lcd.set_ddram_address(0x67).await.expect("off-screen DDRAM");
    byte(&mut expected, [0xE8, 0xEC, 0x78, 0x7C]);
    assert_eq!(*events.borrow(), expected);

    events.borrow_mut().clear();
    assert_eq!(
        lcd.set_cursor(16, 0).await,
        Err(Error::Argument(ArgumentError::InvalidColumn))
    );
    assert_eq!(
        lcd.set_cursor(0, 2).await,
        Err(Error::Argument(ArgumentError::InvalidRow))
    );
    assert_eq!(
        lcd.set_cgram_address(0x40).await,
        Err(Error::Argument(ArgumentError::InvalidCgramAddress))
    );
    for address in [0x28, 0x3F, 0x68, 0x80, 0xFF] {
        assert_eq!(
            lcd.set_ddram_address(address).await,
            Err(Error::Argument(ArgumentError::InvalidDdramAddress))
        );
    }
    lcd.write_bytes(&[]).await.expect("empty write");
    assert!(events.borrow().is_empty());

    // Exercise all entry, display-control, and shift combinations.
    expected.clear();
    for (direction, shift, low) in [
        (EntryDirection::Decrement, false, 0x48),
        (EntryDirection::Decrement, true, 0x58),
        (EntryDirection::Increment, false, 0x68),
        (EntryDirection::Increment, true, 0x78),
    ] {
        lcd.set_entry_mode(direction, shift).await.expect("entry mode");
        byte(&mut expected, [0x08, 0x0C, low, low | 4]);
    }
    for bits in 0..8 {
        lcd.set_display_control(DisplayControl {
            display_enabled: bits & 4 != 0,
            cursor_enabled: bits & 2 != 0,
            blink_enabled: bits & 1 != 0,
        })
        .await
        .expect("display control");
        let low = ((8 + bits) << 4) | 8;
        byte(&mut expected, [0x08, 0x0C, low, low | 4]);
    }
    lcd.shift_cursor(ShiftDirection::Left).await.expect("cursor left");
    byte(&mut expected, [0x18, 0x1C, 0x08, 0x0C]);
    lcd.shift_cursor(ShiftDirection::Right).await.expect("cursor right");
    byte(&mut expected, [0x18, 0x1C, 0x48, 0x4C]);
    lcd.shift_display(ShiftDirection::Left).await.expect("display left");
    byte(&mut expected, [0x18, 0x1C, 0x88, 0x8C]);
    lcd.shift_display(ShiftDirection::Right).await.expect("display right");
    byte(&mut expected, [0x18, 0x1C, 0xC8, 0xCC]);
    assert_eq!(*events.borrow(), expected);

    events.borrow_mut().clear();
    expected.clear();
    lcd.set_backlight(false).await.expect("backlight off");
    expected.push(Event::Write(0x3F, 0));
    lcd.write_bytes(b"A").await.expect("backlight preserved");
    byte(&mut expected, [0x41, 0x45, 0x11, 0x15]);
    lcd.home().await.expect("backlight preserved");
    byte(&mut expected, [0x00, 0x04, 0x20, 0x24]);
    lcd.set_backlight(true).await.expect("backlight on");
    expected.push(Event::Write(0x3F, 8));
    assert_eq!(*events.borrow(), expected);
    let (bus, delay) = lcd.destroy();
    assert!(Rc::ptr_eq(&bus.events, &events));
    assert!(Rc::ptr_eq(&delay.0, &events));

    // Every transfer position in initialization must stop immediately on bus failure.
    for fail_at in 0..43 {
        let (bus, delay, events) = hardware(Some(fail_at));
        let mut lcd = HD44780PCF8574::new(bus, delay, 0x3F).expect("valid address");
        assert_eq!(lcd.init().await, Err(ErrorKind::Other));
        assert!(matches!(events.borrow().last(), Some(Event::Write(_, _))));
        assert_eq!(lcd.destroy().0.writes, fail_at + 1);
    }
    for fail_at in 0..6 {
        let (bus, delay, _) = hardware(Some(fail_at));
        let mut lcd = HD44780PCF8574::new(bus, delay, 0x3F).expect("valid address");
        assert_eq!(lcd.set_cursor(0, 0).await, Err(Error::I2c(ErrorKind::Other)));
        assert_eq!(lcd.destroy().0.writes, fail_at + 1);
    }

    // Failed backlight writes keep the previous requested setting for recovery.
    let (bus, delay, events) = hardware(Some(0));
    let mut lcd = HD44780PCF8574::new(bus, delay, 0x3F).expect("valid address");
    assert_eq!(lcd.set_backlight(false).await, Err(ErrorKind::Other));
    events.borrow_mut().clear();
    lcd.init().await.expect("recover");
    assert_eq!(events.borrow()[0], Event::Write(0x3F, 8));
    lcd.set_backlight(false).await.expect("backlight off");
    events.borrow_mut().clear();
    lcd.init().await.expect("reinitialize");
    assert_eq!(events.borrow()[0], Event::Write(0x3F, 0));
    assert!(events.borrow().iter().all(|event| match event {
        Event::Write(_, byte) => byte & 0x0A == 0,
        Event::Delay(_) => true,
    }));
}

#[cfg(feature = "sync")]
#[test]
fn sync_protocol() {
    exercise_sync();
}

#[cfg(feature = "async")]
#[test]
fn async_protocol() {
    use core::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    let mut future = core::pin::pin!(exercise_async());
    // These mocks are immediately ready; a pending future indicates a test bug.
    assert!(matches!(
        future.as_mut().poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(())
    ));
}
