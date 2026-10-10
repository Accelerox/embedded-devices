//! Fixed PCF8574 backpack wiring, independent of LCD instructions.

pub(super) const ENABLE: u8 = 0x04;

pub(super) fn output(nibble: u8, character: bool, backlight: bool) -> u8 {
    // R/W stays low because transfers use fixed delays instead of reading the LCD.
    (nibble << 4) | u8::from(character) | (u8::from(backlight) << 3)
}
