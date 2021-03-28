// Human Interface Device Manager

use crate::window::*;
use crate::*;
use alloc::boxed::Box;
use bitflags::*;
use core::num::*;
use megstd::drawing::*;

const INVALID_UNICHAR: char = '\u{FEFF}';

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Default, PartialEq, PartialOrd, Eq, Ord)]
pub struct Usage(pub u8);

#[allow(dead_code)]
impl Usage {
    pub const NONE: Usage = Usage(0);
    pub const ALPHABET_A: Usage = Usage(0x04);
    pub const ALPHABET_Z: Usage = Usage(0x1D);
    pub const NUMBER_MIN: Usage = Usage(0x1E);
    pub const NUMBER_MAX: Usage = Usage(0x27);
    pub const NON_ALPHABET_MIN: Usage = Usage(0x28);
    pub const NON_ALPHABET_MAX: Usage = Usage(0x38);
    pub const DELETE: Usage = Usage(0x4C);
    pub const NUMPAD_MIN: Usage = Usage(0x54);
    pub const NUMPAD_MAX: Usage = Usage(0x63);
    pub const INTERNATIONAL_1: Usage = Usage(0x87);
    pub const INTERNATIONAL_3: Usage = Usage(0x89);
    pub const MOD_MIN: Usage = Usage(0xE0);
    pub const MOD_MAX: Usage = Usage(0xE7);
}

bitflags! {
    pub struct Modifier: u8 {
        const LCTRL = 0b0000_0001;
        const LSHIFT = 0b0000_0010;
        const LALT = 0b0000_0100;
        const LGUI = 0b0000_1000;
        const RCTRL = 0b0001_0000;
        const RSHIFT = 0b0010_0000;
        const RALT = 0b0100_0000;
        const RGUI = 0b1000_0000;
    }
}

impl Modifier {
    #[inline]
    pub fn has_shift(self) -> bool {
        (self.bits & (Self::LSHIFT.bits | Self::RSHIFT.bits)) != 0
    }
    #[inline]
    pub fn has_ctrl(self) -> bool {
        (self.bits & (Self::LCTRL.bits | Self::RCTRL.bits)) != 0
    }
    #[inline]
    pub fn has_alt(self) -> bool {
        (self.bits & (Self::LALT.bits | Self::RALT.bits)) != 0
    }
}

impl Default for Modifier {
    fn default() -> Self {
        Self::empty()
    }
}

bitflags! {
    pub struct KeyEventFlags: u8 {
        const BREAK = 0b1000_0000;
    }
}

/// USB HID BIOS Keyboard Raw Report
#[repr(C, packed)]
#[derive(Debug, Copy, Clone, Default)]
pub struct KeyReportRaw {
    pub modifier: Modifier,
    _reserved_1: u8,
    pub keydata: [Usage; 6],
}

#[derive(Debug)]
pub struct KeyboardState {
    pub current: KeyReportRaw,
    pub prev: KeyReportRaw,
}

impl KeyboardState {
    pub fn process_key_report(&mut self, report: KeyReportRaw) {
        let modifier = report.modifier;
        self.prev = self.current;
        self.current = report;
        for usage in &self.prev.keydata {
            let usage = *usage;
            if usage != Usage::NONE
                && usage < Usage::MOD_MIN
                && usage > Usage::MOD_MAX
                && !self.current.keydata.contains(&usage)
            {
                KeyEvent::new(usage, modifier, KeyEventFlags::BREAK).post();
            }
        }
        for usage in &self.current.keydata {
            let usage = *usage;
            if usage != Usage::NONE
                && usage < Usage::MOD_MIN
                && usage > Usage::MOD_MAX
                && !self.prev.keydata.contains(&usage)
            {
                KeyEvent::new(usage, modifier, KeyEventFlags::empty()).post();
            }
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct KeyEvent(NonZeroU32);

impl KeyEvent {
    pub const fn new(usage: Usage, modifier: Modifier, flags: KeyEventFlags) -> Self {
        unsafe {
            Self(NonZeroU32::new_unchecked(
                usage.0 as u32 | ((modifier.bits as u32) << 16) | ((flags.bits as u32) << 24),
            ))
        }
    }

    pub fn into_char(self) -> char {
        HidManager::key_event_to_char(self)
    }

    pub const fn usage(self) -> Usage {
        Usage((self.0.get() & 0xFF) as u8)
    }

    pub const fn modifier(self) -> Modifier {
        unsafe { Modifier::from_bits_unchecked(((self.0.get() >> 16) & 0xFF) as u8) }
    }

    pub const fn flags(self) -> KeyEventFlags {
        unsafe { KeyEventFlags::from_bits_unchecked(((self.0.get() >> 24) & 0xFF) as u8) }
    }

    pub fn key_data(self) -> Option<Self> {
        if self.usage() != Usage::NONE && !self.flags().contains(KeyEventFlags::BREAK) {
            Some(self)
        } else {
            None
        }
    }

    pub fn post(self) {
        WindowManager::post_key_event(self);
    }
}

impl Into<char> for KeyEvent {
    fn into(self) -> char {
        self.into_char()
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

/// USB HID BIOS Mouse Raw Report
pub type MouseReportRaw = MouseReport<i8>;

impl<T> MouseReport<T>
where
    T: Into<isize> + Copy,
{
    pub fn normalize(self) -> MouseReport<isize> {
        MouseReport {
            buttons: self.buttons,
            x: self.x.into(),
            y: self.y.into(),
        }
    }
}

bitflags! {
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

#[derive(Debug, Copy, Clone, Default)]
pub struct MouseState {
    pub current_buttons: MouseButton,
    pub prev_buttons: MouseButton,
    pub x: isize,
    pub y: isize,
}

impl MouseState {
    pub const fn empty() -> Self {
        Self {
            current_buttons: MouseButton::empty(),
            prev_buttons: MouseButton::empty(),
            x: 0,
            y: 0,
        }
    }

    pub fn process_mouse_report<T>(&mut self, report: MouseReport<T>)
    where
        T: Into<isize> + Copy,
    {
        self.prev_buttons = self.current_buttons;
        self.current_buttons = report.buttons;
        self.x += report.x.into();
        self.y += report.y.into();
        WindowManager::post_mouse_event(self);
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct MouseEvent {
    pub x: i16,
    pub y: i16,
    pub buttons: MouseButton,
    pub event_buttons: MouseButton,
}

impl MouseEvent {
    pub const fn new(point: Point, buttons: MouseButton, event_buttons: MouseButton) -> Self {
        Self {
            x: point.x as i16,
            y: point.y as i16,
            buttons,
            event_buttons,
        }
    }

    pub const fn point(&self) -> Point {
        Point {
            x: self.x as isize,
            y: self.y as isize,
        }
    }

    pub const fn buttons(&self) -> MouseButton {
        self.buttons
    }

    pub const fn event_buttons(&self) -> MouseButton {
        self.event_buttons
    }
}

pub struct HidManager {
    _phantom: (),
}

static mut HID_MANAGER: Option<Box<HidManager>> = None;

impl HidManager {
    pub(crate) fn init() {
        unsafe {
            HID_MANAGER = Some(Box::new(HidManager::new()));
        }
    }

    fn new() -> Self {
        HidManager { _phantom: () }
    }

    #[allow(dead_code)]
    fn shared() -> &'static HidManager {
        unsafe { HID_MANAGER.as_ref().unwrap() }
    }

    fn key_event_to_char(event: KeyEvent) -> char {
        if event.flags().contains(KeyEventFlags::BREAK) || event.usage() == Usage::NONE {
            '\0'
        } else {
            Self::usage_to_char_109(event.usage(), event.modifier())
        }
    }

    fn usage_to_char_109(usage: Usage, modifier: Modifier) -> char {
        let mut uni: char = INVALID_UNICHAR;

        if usage >= Usage::ALPHABET_A && usage <= Usage::ALPHABET_Z {
            uni = (usage.0 - Usage::ALPHABET_A.0 + 0x61) as char;
        } else if usage >= Usage::NUMBER_MIN && usage <= Usage::NON_ALPHABET_MAX {
            uni = USAGE_TO_CHAR_NON_ALPLABET_109[(usage.0 - Usage::NUMBER_MIN.0) as usize];
            if uni > ' ' && uni < '\x40' && uni != '0' && modifier.has_shift() {
                uni = (uni as u8 ^ 0x10) as char;
            }
        } else if usage == Usage::DELETE {
            uni = '\x7F';
        } else if usage >= Usage::NUMPAD_MIN && usage <= Usage::NUMPAD_MAX {
            uni = USAGE_TO_CHAR_NUMPAD[(usage.0 - Usage::NUMPAD_MIN.0) as usize];
        } else if usage == Usage::INTERNATIONAL_3 {
            // '\|'
            uni = '\\';
        }

        if uni >= '\x40' && uni < '\x7F' {
            if modifier.has_ctrl() {
                uni = (uni as u8 & 0x1F) as char;
            } else if modifier.has_shift() {
                uni = (uni as u8 ^ 0x20) as char;
            }
        }

        if usage == Usage::INTERNATIONAL_1 {
            if modifier.has_shift() {
                uni = '_';
            } else {
                uni = '\\';
            }
        }

        uni
    }
}

// Non Alphabet
static USAGE_TO_CHAR_NON_ALPLABET_109: [char; 27] = [
    '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '\x0D', '\x1B', '\x08', '\x09', ' ', '-',
    '^', '@', '[', ']', ']', ';', ':', '`', ',', '.', '/',
];

// Numpads
static USAGE_TO_CHAR_NUMPAD: [char; 16] = [
    '/', '*', '-', '+', '\x0D', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '.',
];
