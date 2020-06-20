// Human Interface Devices

use super::graphics::*;
use crate::kernel::sync::queue::*;
use crate::*;
use alloc::boxed::Box;
use bitflags::*;
use core::cmp;
use core::num::*;
use core::sync::atomic::*;

const INVALID_UNICHAR: char = '\u{FEFF}';

pub struct HidKeyboard {
    pub state: KeyboardState,
}

pub struct HidMouse {
    pub state: MouseState,
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct Usage(pub u8);

#[allow(dead_code)]
impl Usage {
    pub const NULL: Usage = Usage(0);
    pub const ALPHABET_MIN: Usage = Usage(0x04);
    pub const ALPHABET_MAX: Usage = Usage(0x1D);
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

        const SHIFT = Self::LSHIFT.bits | Self::RSHIFT.bits;
        const CTRL = Self::LCTRL.bits | Self::RCTRL.bits;
        const ALT = Self::LALT.bits | Self::RALT.bits;
    }
}

impl Modifier {
    pub fn is_shift(&self) -> bool {
        (self.bits & Self::SHIFT.bits) != 0
    }
    pub fn is_ctrl(&self) -> bool {
        (self.bits & Self::CTRL.bits) != 0
    }
    pub fn is_alt(&self) -> bool {
        (self.bits & Self::ALT.bits) != 0
    }
}

/// USB HID BIOS Keyboard Raw Report
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct KeyReportRaw {
    pub modifier: Modifier,
    _reserved_1: u8,
    pub keydata: [Usage; 6],
}

#[derive(Debug, Copy, Clone)]
pub struct KeyEvent {
    pub usage: Usage,
    pub modifier: Modifier,
}

impl KeyEvent {
    pub const fn new(usage: Usage, modifier: Modifier) -> Self {
        Self {
            usage: usage,
            modifier: modifier,
        }
    }
}

impl From<NonZeroUsize> for KeyEvent {
    fn from(value: NonZeroUsize) -> Self {
        let value = value.get();
        Self {
            usage: Usage((value & 0xFF) as u8),
            modifier: unsafe { Modifier::from_bits_unchecked(((value >> 16) & 0xFF) as u8) },
        }
    }
}

impl Into<NonZeroUsize> for KeyEvent {
    fn into(self) -> NonZeroUsize {
        unsafe {
            NonZeroUsize::new_unchecked(
                self.usage.0 as usize | ((self.modifier.bits as usize) << 16),
            )
        }
    }
}

#[derive(Debug)]
pub struct KeyboardState {
    pub current: KeyReportRaw,
    pub prev: KeyReportRaw,
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
    pub fn normalize(&self) -> MouseReport<isize> {
        MouseReport {
            buttons: self.buttons,
            x: self.x.into(),
            y: self.y.into(),
        }
    }
}

bitflags! {
    pub struct MouseButton: u8 {
        const LEFT = 0b0000_0001;
        const RIGHT = 0b0000_0010;
        const MIDDLE = 0b0000_0100;
        const BUTTON4 = 0b0000_1000;
        const BUTTON5 = 0b0001_0000;
        const BUTTON6 = 0b0010_0000;
        const BUTTON7 = 0b0100_0000;
        const BUTTON8 = 0b1000_0000;
    }
}

pub struct MouseState {
    pub buttons: MouseButton,
    pub prev: MouseButton,
    pub x: isize,
    pub y: isize,
}

pub struct HidManager {
    pointer_x: AtomicIsize,
    pointer_y: AtomicIsize,
    key_buf: Box<AtomicLinkedQueue<KeyEvent>>,
}

static mut HID_MANAGER: Option<Box<HidManager>> = None;

impl HidManager {
    pub(crate) fn init() {
        unsafe {
            HID_MANAGER = Some(Box::new(HidManager::new()));
        }
    }

    fn new() -> Self {
        HidManager {
            // lock: Spinlock::new(),
            pointer_x: AtomicIsize::new(256),
            pointer_y: AtomicIsize::new(256),
            key_buf: AtomicLinkedQueue::with_capacity(256),
        }
    }

    fn update_coord(coord: &AtomicIsize, movement: isize, min_value: isize, max_value: isize) {
        let mut value = coord.load(Ordering::Relaxed);
        loop {
            let new_value = cmp::min(cmp::max(value + movement, min_value), max_value);
            if value == new_value {
                break;
            }
            match coord.compare_exchange(value, new_value, Ordering::SeqCst, Ordering::Relaxed) {
                Ok(_) => break,
                Err(actual) => value = actual,
            }
        }
    }

    pub fn pointer() -> Point<isize> {
        let shared = unsafe { HID_MANAGER.as_ref().unwrap() };
        Point::new(
            shared.pointer_x.load(Ordering::Relaxed),
            shared.pointer_y.load(Ordering::Relaxed),
        )
    }

    fn update_pointer(report: MouseReport<isize>) {
        let fb = stdout().fb();

        let shared = unsafe { HID_MANAGER.as_ref().unwrap() };
        Self::update_coord(&shared.pointer_x, report.x, 0, fb.size().width - 1);
        Self::update_coord(&shared.pointer_y, report.y, 0, fb.size().height - 1);

        let pointer = Self::pointer();
        let r = 2;
        let pad = 4;
        let rect_outer = Rect::new(
            pointer.x - r - pad,
            pointer.y - r - pad,
            (r + pad) * 2,
            (r + pad) * 2,
        );
        let rect_inner = Rect::new(pointer.x - r, pointer.y - r, r * 2, r * 2);

        fb.fill_rect(rect_outer, Color::from(0x000000));
        fb.fill_rect(rect_inner, Color::from(0xFFFFFF));
    }

    pub fn process_mouse_report<T>(report: MouseReport<T>)
    where
        T: Into<isize> + Copy,
    {
        let report = report.normalize();
        Self::update_pointer(report);
    }

    pub fn send_key_event(v: KeyEvent) {
        let shared = unsafe { HID_MANAGER.as_ref().unwrap() };
        let _ = shared.key_buf.enqueue(v);
    }

    pub fn get_key() -> Option<KeyEvent> {
        let shared = unsafe { HID_MANAGER.as_ref().unwrap() };
        shared.key_buf.dequeue()
    }

    pub fn usage_to_char_109(usage: Usage, modifier: Modifier) -> char {
        let mut uni: char = INVALID_UNICHAR;

        if usage >= Usage::ALPHABET_MIN && usage <= Usage::ALPHABET_MAX {
            uni = (usage.0 - Usage::ALPHABET_MIN.0 + 0x61) as char;
        } else if usage >= Usage::NUMBER_MIN && usage <= Usage::NON_ALPHABET_MAX {
            uni = USAGE_TO_CHAR_NON_ALPLABET_109[(usage.0 - Usage::NUMBER_MIN.0) as usize];
            if uni > ' ' && uni < '\x40' && uni != '0' && modifier.is_shift() {
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
            if modifier.is_ctrl() {
                uni = (uni as u8 & 0x1F) as char;
            } else if modifier.is_shift() {
                uni = (uni as u8 ^ 0x20) as char;
            }
        }

        if usage == Usage::INTERNATIONAL_1 {
            if modifier.is_shift() {
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
    '1',
    '2',
    '3',
    '4',
    '5',
    '6',
    '7',
    '8',
    '9',
    '0',
    '\x0D',
    '\x1B',
    '\x08',
    '\x09',
    ' ',
    '-',
    '^',
    '@',
    '[',
    ']',
    INVALID_UNICHAR,
    ';',
    ':',
    '`',
    ',',
    '.',
    '/',
];

// Numpads
static USAGE_TO_CHAR_NUMPAD: [char; 16] = [
    '/', '*', '-', '+', '\x0D', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '.',
];
