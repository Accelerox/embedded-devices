//! HD44780-compatible 16×2 LCD with a PCF8574-compatible I²C backpack.
//!
//! Uses four-bit transfers and fixed delays, with no reads or busy polling.
//! The backpack maps P0 to RS, P1 to R/W, P2 to Enable, P3 to an active-high
//! backlight, and P4–P7 to D4–D7. Other backpack wirings are not supported.
//! See the [HD44780 instruction specification](https://cdn-shop.adafruit.com/datasheets/HD44780.pdf).
//!
//! `maybe_async_cfg` generates `HD44780PCF8574Sync` and `HD44780PCF8574Async`
//! for the `sync` and `async` features respectively. Construction performs no I/O;
//! call `init` before using LCD commands. After a failed or cancelled transfer,
//! call `init` again to restore nibble alignment and the default LCD state.
//!
//! A 5 V backpack may pull SDA and SCL up to 5 V. Use suitable voltage interfacing
//! with a 3.3 V host, such as an appropriate bidirectional I²C level shifter.
//! Successful communication does not establish electrical safety. The hardware
//! was verified with Python on a Raspberry Pi 5 at 100 kHz and address `0x3F`;
//! this does not constitute hardware validation of this Rust implementation.
//!
//! ## Synchronous usage
//!
//! ```rust
//! # #[cfg(feature = "sync")] mod example {
//! # fn display<I: embedded_hal::i2c::I2c, D: embedded_hal::delay::DelayNs>(i2c: I, delay: D)
//! # -> Result<(), embedded_devices::devices::hitachi::hd44780_pcf8574::Error<I::Error>> {
//! use embedded_devices::devices::hitachi::hd44780_pcf8574::HD44780PCF8574Sync;
//! let mut lcd = HD44780PCF8574Sync::new(i2c, delay, 0x3F).expect("valid address");
//! lcd.init()?;
//! lcd.write_bytes(b"Hello")?;
//! lcd.set_cursor(0, 1)?;
//! lcd.write_bytes(b"Second line")?;
//! // Eight rows of five meaningful bits define character slot zero.
//! lcd.set_cgram_address(0)?;
//! lcd.write_bytes(&[0x04, 0x0E, 0x15, 0x04, 0x04, 0x04, 0x04, 0])?;
//! lcd.set_cursor(15, 0)?;
//! lcd.write_bytes(&[0])?;
//! let (i2c, delay) = lcd.destroy();
//! # Ok(())
//! # }
//! # }
//! ```
//!
//! ## Asynchronous usage
//!
//! ```rust
//! # #[cfg(feature = "async")] mod example {
//! # async fn display<I: embedded_hal_async::i2c::I2c, D: embedded_hal_async::delay::DelayNs>(i2c: I, delay: D)
//! # -> Result<(), embedded_devices::devices::hitachi::hd44780_pcf8574::Error<I::Error>> {
//! use embedded_devices::devices::hitachi::hd44780_pcf8574::HD44780PCF8574Async;
//! let mut lcd = HD44780PCF8574Async::new(i2c, delay, 0x3F).expect("valid address");
//! lcd.init().await?;
//! lcd.write_bytes(b"Hello").await?;
//! lcd.set_cursor(0, 1).await?;
//! lcd.write_bytes(b"Second line").await?;
//! lcd.set_cgram_address(0).await?;
//! lcd.write_bytes(&[0x04, 0x0E, 0x15, 0x04, 0x04, 0x04, 0x04, 0]).await?;
//! lcd.set_cursor(15, 0).await?;
//! lcd.write_bytes(&[0]).await?;
//! let (i2c, delay) = lcd.destroy();
//! # Ok(())
//! # }
//! # }
//! ```

mod backpack;
mod commands;

/// Address-counter movement after writing a character or CGRAM row.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryDirection {
    /// Increase the address counter.
    Increment,
    /// Decrease the address counter.
    Decrement,
}

/// Direction of an explicit cursor or display shift.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftDirection {
    /// Shift left.
    Left,
    /// Shift right.
    Right,
}

/// Independent display, cursor, and blinking settings.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayControl {
    /// Show DDRAM contents without changing them.
    pub display_enabled: bool,
    /// Show the underline cursor.
    pub cursor_enabled: bool,
    /// Blink the character at the cursor position.
    pub blink_enabled: bool,
}

/// Invalid constructor configuration.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// Address exceeds the seven-bit range.
    #[error("invalid seven-bit address")]
    InvalidAddress,
}

/// Invalid externally supplied LCD coordinates or RAM addresses.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ArgumentError {
    /// Column must be less than 16.
    #[error("column must be less than 16")]
    InvalidColumn,
    /// Row must be less than two.
    #[error("row must be less than two")]
    InvalidRow,
    /// CGRAM address must be at most 0x3F.
    #[error("invalid CGRAM address")]
    InvalidCgramAddress,
    /// Two-line DDRAM addresses are 0x00–0x27 and 0x40–0x67.
    #[error("invalid DDRAM address")]
    InvalidDdramAddress,
}

/// Failure from a command accepting validated arguments.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error<E> {
    /// Underlying I²C transfer failed.
    #[error("I²C transfer failed: {0}")]
    I2c(#[from] E),
    /// Argument was rejected before any I²C transfer.
    #[error("invalid argument: {0}")]
    Argument(ArgumentError),
}

/// LCD driver owning its I²C bus and delay provider.
#[maybe_async_cfg::maybe(sync(feature = "sync"), async(feature = "async"))]
pub struct HD44780PCF8574<I, D> {
    interface: I,
    delay: D,
    address: u8,
    backlight: bool,
}

#[maybe_async_cfg::maybe(
    idents(hal(sync = "embedded_hal", async = "embedded_hal_async")),
    sync(feature = "sync"),
    async(feature = "async")
)]
impl<I: hal::i2c::I2c, D: hal::delay::DelayNs> HD44780PCF8574<I, D> {
    /// Creates a driver without hardware access, with backlight initially enabled.
    pub fn new(interface: I, delay: D, address: impl Into<u8>) -> Result<Self, ConfigError> {
        let address = address.into();
        if address > 0x7F {
            return Err(ConfigError::InvalidAddress);
        }
        Ok(Self {
            interface,
            delay,
            address,
            backlight: true,
        })
    }

    /// Returns the owned bus and delay without accessing hardware.
    pub fn destroy(self) -> (I, D) {
        (self.interface, self.delay)
    }

    /// Resets to four-bit, two-line mode, clears text, and enables the display.
    /// Sets incrementing entry mode without shifting, cursor or blink; preserves backlight.
    pub async fn init(&mut self) -> Result<(), I::Error> {
        self.interface
            .write(self.address, &[backpack::output(0, false, self.backlight)])
            .await?;
        self.delay.delay_ms(50).await;
        // Single nibbles recover from either the power-on state or a partial byte.
        for _ in 0..3 {
            self.write_nibble(3, false).await?;
            self.delay.delay_ms(5).await;
        }
        self.write_nibble(2, false).await?;
        for command in [commands::FUNCTION_SET, 0x08, commands::CLEAR, 0x06, 0x0C] {
            self.write_byte(command, false).await?;
        }
        Ok(())
    }

    /// Clears DDRAM, homes the display, and selects incrementing entry mode.
    /// The entry-mode shift setting is preserved by the LCD.
    pub async fn clear(&mut self) -> Result<(), I::Error> {
        self.write_byte(commands::CLEAR, false).await
    }

    /// Returns the cursor and display shift to their origin without clearing text.
    pub async fn home(&mut self) -> Result<(), I::Error> {
        self.write_byte(commands::HOME, false).await
    }

    /// Selects a DDRAM position: column 0–15 and row 0–1.
    /// Coordinates refer to the unshifted display; call `home` to undo display shifts.
    pub async fn set_cursor(&mut self, column: u8, row: u8) -> Result<(), Error<I::Error>> {
        if column >= 16 {
            return Err(Error::Argument(ArgumentError::InvalidColumn));
        }
        if row >= 2 {
            return Err(Error::Argument(ArgumentError::InvalidRow));
        }
        self.set_ddram_address(column + row * 0x40).await
    }

    /// Writes raw LCD codes to the selected RAM area, with no UTF-8 or newline handling.
    /// CGRAM accepts eight rows per 5×8 glyph; only each row's low five bits are displayed.
    /// Address movement and shifting follow the selected entry mode; no wrapping is added.
    pub async fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), I::Error> {
        for &byte in bytes {
            self.write_byte(byte, true).await?;
        }
        Ok(())
    }

    /// Changes the backlight, leaving Enable and R/W low.
    pub async fn set_backlight(&mut self, enabled: bool) -> Result<(), I::Error> {
        self.interface
            .write(self.address, &[backpack::output(0, false, enabled)])
            .await?;
        self.backlight = enabled;
        Ok(())
    }

    /// Selects address movement and automatic display shifting after DDRAM writes.
    /// Increment shifts the display left; decrement shifts it right. CGRAM writes never shift it.
    pub async fn set_entry_mode(&mut self, direction: EntryDirection, shift_display: bool) -> Result<(), I::Error> {
        self.write_byte(commands::entry_mode(direction, shift_display), false)
            .await
    }

    /// Sets display visibility, underline cursor, and character blinking independently.
    pub async fn set_display_control(&mut self, control: DisplayControl) -> Result<(), I::Error> {
        self.write_byte(commands::display_control(control), false).await
    }

    /// Moves the cursor and address counter without changing DDRAM contents.
    pub async fn shift_cursor(&mut self, direction: ShiftDirection) -> Result<(), I::Error> {
        self.write_byte(commands::shift(direction, false), false).await
    }

    /// Shifts both display rows without changing the address counter or DDRAM contents.
    pub async fn shift_display(&mut self, direction: ShiftDirection) -> Result<(), I::Error> {
        self.write_byte(commands::shift(direction, true), false).await
    }

    /// Selects CGRAM address 0x00–0x3F for subsequent raw character-pattern writes.
    /// Select DDRAM again before writing display text.
    pub async fn set_cgram_address(&mut self, address: u8) -> Result<(), Error<I::Error>> {
        if address > 0x3F {
            return Err(Error::Argument(ArgumentError::InvalidCgramAddress));
        }
        self.write_byte(commands::cgram_address(address), false).await?;
        Ok(())
    }

    /// Selects DDRAM address 0x00–0x27 or 0x40–0x67, including off-screen columns.
    pub async fn set_ddram_address(&mut self, address: u8) -> Result<(), Error<I::Error>> {
        if !matches!(address, 0x00..=0x27 | 0x40..=0x67) {
            return Err(Error::Argument(ArgumentError::InvalidDdramAddress));
        }
        self.write_byte(commands::ddram_address(address), false).await?;
        Ok(())
    }

    async fn write_byte(&mut self, byte: u8, character: bool) -> Result<(), I::Error> {
        self.write_nibble(byte >> 4, character).await?;
        self.write_nibble(byte & 0x0F, character).await?;
        // Conservative fixed timing also covers clear and return-home commands.
        self.delay.delay_ms(2).await;
        Ok(())
    }

    async fn write_nibble(&mut self, nibble: u8, character: bool) -> Result<(), I::Error> {
        let data = backpack::output(nibble, character, self.backlight);
        self.interface.write(self.address, &[data]).await?;
        self.interface.write(self.address, &[data | backpack::ENABLE]).await?;
        self.delay.delay_ms(1).await;
        self.interface.write(self.address, &[data]).await?;
        self.delay.delay_ms(1).await;
        Ok(())
    }
}

#[cfg(all(test, any(feature = "sync", feature = "async")))]
mod tests;
