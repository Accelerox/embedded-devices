//! HD44780 instruction encoding for a four-bit, two-line, 5×8 display.

use super::{DisplayControl, EntryDirection, ShiftDirection};

pub(super) const CLEAR: u8 = 0x01;
pub(super) const HOME: u8 = 0x02;
pub(super) const FUNCTION_SET: u8 = 0x28;

pub(super) fn entry_mode(direction: EntryDirection, shift_display: bool) -> u8 {
    0x04 | ((matches!(direction, EntryDirection::Increment) as u8) << 1) | u8::from(shift_display)
}

pub(super) fn display_control(control: DisplayControl) -> u8 {
    0x08 | (u8::from(control.display_enabled) << 2)
        | (u8::from(control.cursor_enabled) << 1)
        | u8::from(control.blink_enabled)
}

pub(super) fn shift(direction: ShiftDirection, display: bool) -> u8 {
    0x10 | (u8::from(display) << 3) | ((matches!(direction, ShiftDirection::Right) as u8) << 2)
}

pub(super) fn cgram_address(address: u8) -> u8 {
    0x40 | address
}

pub(super) fn ddram_address(address: u8) -> u8 {
    0x80 | address
}
