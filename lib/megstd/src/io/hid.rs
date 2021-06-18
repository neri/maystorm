//! Human Interface Device Manager

use bitflags::*;

/// Keyboard usage as defined by the HID specification.
#[repr(transparent)]
#[derive(Debug, Copy, Clone, Default, PartialEq, PartialOrd, Eq, Ord)]
pub struct Usage(pub u8);

impl Usage {
    pub const NONE: Usage = Usage(0);
    pub const ERR_ROLL_OVER: Usage = Usage(1);
    pub const ERR_POST_FAIL: Usage = Usage(2);
    pub const ERR_UNDEFINED: Usage = Usage(3);

    pub const KEY_A: Usage = Usage(0x04);
    pub const KEY_B: Usage = Usage(0x05);
    pub const KEY_C: Usage = Usage(0x06);
    pub const KEY_D: Usage = Usage(0x07);
    pub const KEY_E: Usage = Usage(0x08);
    pub const KEY_F: Usage = Usage(0x09);
    pub const KEY_G: Usage = Usage(0x0A);
    pub const KEY_H: Usage = Usage(0x0B);
    pub const KEY_I: Usage = Usage(0x0C);
    pub const KEY_J: Usage = Usage(0x0D);
    pub const KEY_K: Usage = Usage(0x0E);
    pub const KEY_L: Usage = Usage(0x0F);
    pub const KEY_M: Usage = Usage(0x10);
    pub const KEY_N: Usage = Usage(0x11);
    pub const KEY_O: Usage = Usage(0x12);
    pub const KEY_P: Usage = Usage(0x13);
    pub const KEY_Q: Usage = Usage(0x14);
    pub const KEY_R: Usage = Usage(0x15);
    pub const KEY_S: Usage = Usage(0x16);
    pub const KEY_T: Usage = Usage(0x17);
    pub const KEY_U: Usage = Usage(0x18);
    pub const KEY_V: Usage = Usage(0x19);
    pub const KEY_W: Usage = Usage(0x1A);
    pub const KEY_X: Usage = Usage(0x1B);
    pub const KEY_Y: Usage = Usage(0x1C);
    pub const KEY_Z: Usage = Usage(0x1D);
    pub const KEY_1: Usage = Usage(0x1E);
    pub const KEY_2: Usage = Usage(0x1F);
    pub const KEY_3: Usage = Usage(0x20);
    pub const KEY_4: Usage = Usage(0x21);
    pub const KEY_5: Usage = Usage(0x22);
    pub const KEY_6: Usage = Usage(0x23);
    pub const KEY_7: Usage = Usage(0x24);
    pub const KEY_8: Usage = Usage(0x25);
    pub const KEY_9: Usage = Usage(0x26);
    pub const KEY_0: Usage = Usage(0x27);
    pub const KEY_ENTER: Usage = Usage(0x28);
    pub const KEY_ESCAPE: Usage = Usage(0x29);
    pub const KEY_BASKSPACE: Usage = Usage(0x2A);
    pub const KEY_TAB: Usage = Usage(0x2B);
    pub const KEY_SPACE: Usage = Usage(0x2C);

    pub const KEY_F1: Usage = Usage(0x3A);
    pub const KEY_F2: Usage = Usage(0x3B);
    pub const KEY_F3: Usage = Usage(0x3C);
    pub const KEY_F4: Usage = Usage(0x3D);
    pub const KEY_F5: Usage = Usage(0x3E);
    pub const KEY_F6: Usage = Usage(0x3F);
    pub const KEY_F7: Usage = Usage(0x40);
    pub const KEY_F8: Usage = Usage(0x41);
    pub const KEY_F9: Usage = Usage(0x42);
    pub const KEY_F10: Usage = Usage(0x43);
    pub const KEY_F11: Usage = Usage(0x44);
    pub const KEY_F12: Usage = Usage(0x45);
    pub const DELETE: Usage = Usage(0x4C);
    pub const KEY_RIGHT_ARROW: Usage = Usage(0x4F);
    pub const KEY_LEFT_ARROW: Usage = Usage(0x50);
    pub const KEY_DOWN_ARROW: Usage = Usage(0x51);
    pub const KEY_UP_ARROW: Usage = Usage(0x52);
    pub const KEY_NUM_LOCK: Usage = Usage(0x53);

    pub const NUMPAD_1: Usage = Usage(0x59);
    pub const NUMPAD_2: Usage = Usage(0x5A);
    pub const NUMPAD_3: Usage = Usage(0x5B);
    pub const NUMPAD_4: Usage = Usage(0x5C);
    pub const NUMPAD_5: Usage = Usage(0x5D);
    pub const NUMPAD_6: Usage = Usage(0x5E);
    pub const NUMPAD_7: Usage = Usage(0x5F);
    pub const NUMPAD_8: Usage = Usage(0x60);
    pub const NUMPAD_9: Usage = Usage(0x61);
    pub const NUMPAD_0: Usage = Usage(0x62);

    pub const INTERNATIONAL_1: Usage = Usage(0x87);
    pub const INTERNATIONAL_2: Usage = Usage(0x88);
    pub const INTERNATIONAL_3: Usage = Usage(0x89);
    pub const INTERNATIONAL_4: Usage = Usage(0x8A);
    pub const INTERNATIONAL_5: Usage = Usage(0x8B);
    pub const INTERNATIONAL_6: Usage = Usage(0x8C);
    pub const INTERNATIONAL_7: Usage = Usage(0x8D);
    pub const INTERNATIONAL_8: Usage = Usage(0x8E);
    pub const INTERNATIONAL_9: Usage = Usage(0x8F);

    pub const ALPHABET_MIN: Usage = Usage(0x04);
    pub const ALPHABET_MAX: Usage = Usage(0x1D);
    pub const NUMBER_MIN: Usage = Usage(0x1E);
    pub const NUMBER_MAX: Usage = Usage(0x27);
    pub const NON_ALPHABET_MIN: Usage = Usage(0x28);
    pub const NON_ALPHABET_MAX: Usage = Usage(0x38);
    pub const NUMPAD_MIN: Usage = Usage(0x54);
    pub const NUMPAD_MAX: Usage = Usage(0x63);
    pub const MOD_MIN: Usage = Usage(0xE0);
    pub const MOD_MAX: Usage = Usage(0xE7);
}

bitflags! {
    /// Modifier keys as defined by the HID specification.
    pub struct Modifier: u8 {
        const LEFT_CTRL     = 0b0000_0001;
        const LEFT_SHIFT    = 0b0000_0010;
        const LEFT_ALT      = 0b0000_0100;
        const LEFT_GUI      = 0b0000_1000;
        const RIGHT_CTRL    = 0b0001_0000;
        const RIGHT_SHIFT   = 0b0010_0000;
        const RIGHT_ALT     = 0b0100_0000;
        const RIGHT_GUI     = 0b1000_0000;
    }
}

impl Modifier {
    #[inline]
    pub const fn has_shift(self) -> bool {
        self.contains(Self::LEFT_SHIFT) | self.contains(Self::RIGHT_SHIFT)
    }

    #[inline]
    pub const fn has_ctrl(self) -> bool {
        self.contains(Self::LEFT_CTRL) | self.contains(Self::RIGHT_CTRL)
    }

    #[inline]
    pub const fn has_alt(self) -> bool {
        self.contains(Self::LEFT_ALT) | self.contains(Self::RIGHT_ALT)
    }
}

impl Default for Modifier {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MouseReport<T>
where
    T: Into<isize> + Copy,
{
    pub buttons: MouseButton,
    pub x: T,
    pub y: T,
}

impl<T> MouseReport<T>
where
    T: Into<isize> + Copy,
{
    /// Returns the mouse report in a canonical format.
    #[inline]
    pub fn normalize(self) -> MouseReport<isize> {
        MouseReport {
            buttons: self.buttons,
            x: self.x.into(),
            y: self.y.into(),
        }
    }
}

bitflags! {
    /// Mouse buttons as defined by the HID specification.
    pub struct MouseButton: u8 {
        const LEFT      = 0b0000_0001;
        const RIGHT     = 0b0000_0010;
        const MIDDLE    = 0b0000_0100;
        const BUTTON4   = 0b0000_1000;
        const BUTTON5   = 0b0001_0000;
        const BUTTON6   = 0b0010_0000;
        const BUTTON7   = 0b0100_0000;
        const BUTTON8   = 0b1000_0000;
    }
}

impl Default for MouseButton {
    fn default() -> Self {
        Self::empty()
    }
}
