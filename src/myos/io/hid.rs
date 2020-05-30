// Human Interface Devices

use super::graphics::*;
use crate::*;
use alloc::boxed::Box;
use alloc::vec::*;
use bitflags::*;
use core::cmp;
use core::sync::atomic::*;
// use core::ptr::NonNull;

const INVALID_UNICHAR: char = '\u{FEFF}';

pub enum HumanInterfaceDevice {
    Keyboard(HidKeyboard),
    Mouse(HidMouse),
}

pub struct HidKeyboard {
    pub state: KeyboardState,
}

pub struct HidMouse {
    pub buttons: MouseButton,
}

pub struct HidGeneric {
    pub class_id: u32,
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

#[derive(Debug)]
pub struct KeyboardState {
    pub current: KeyReportRaw,
    pub prev: KeyReportRaw,
}

/// USB HID BIOS Mouse Raw Report
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct MouseReportRaw {
    pub buttons: MouseButton,
    pub x: i8,
    pub y: i8,
}

#[derive(Debug, Copy, Clone)]
pub struct MouseReport<T>
where
    T: Into<isize>,
{
    pub buttons: MouseButton,
    pub x: T,
    pub y: T,
}

impl<T> From<MouseReportRaw> for MouseReport<T>
where
    T: Into<isize> + From<i8>,
{
    fn from(report: MouseReportRaw) -> Self {
        Self {
            buttons: report.buttons,
            x: T::from(report.x),
            y: T::from(report.y),
        }
    }
}

bitflags! {
    pub struct MouseButton: u8 {
        const LEFT = 0b0000_0001;
        const RIGHT = 0b0000_0010;
        const MIDDLE = 0b0000_0100;
    }
}

pub struct HidManager {
    devices: Vec<Box<HumanInterfaceDevice>>,
    lock: Spinlock,
    pointer_x: AtomicIsize,
    pointer_y: AtomicIsize,
}

static mut HID_MANAGER: HidManager = HidManager::new();

impl HidManager {
    const fn new() -> Self {
        HidManager {
            devices: Vec::new(),
            lock: Spinlock::new(),
            pointer_x: AtomicIsize::new(256),
            pointer_y: AtomicIsize::new(256),
        }
    }

    pub fn add(new_device: Box<HumanInterfaceDevice>) -> Result<(), ()> {
        unsafe { &HID_MANAGER }.lock.synchronized(|| {
            let manager = unsafe { &mut HID_MANAGER };
            manager.devices.push(new_device);
            Ok(())
        })
    }

    fn update_coord(coord: &AtomicIsize, displacement: isize, max_value: isize) {
        let mut value = coord.load(Ordering::Relaxed);
        loop {
            let new_value = cmp::min(cmp::max(value + displacement, 0), max_value);
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
        let shared = unsafe { &HID_MANAGER };
        Point::new(
            shared.pointer_x.load(Ordering::Relaxed),
            shared.pointer_y.load(Ordering::Relaxed),
        )
    }

    pub fn update_pointer<T>(report: MouseReport<T>)
    where
        T: Into<isize>,
    {
        let fb = stdout().fb();

        let shared = unsafe { &HID_MANAGER };
        Self::update_coord(&shared.pointer_x, report.x.into(), fb.size().width - 1);
        Self::update_coord(&shared.pointer_y, report.y.into(), fb.size().height - 1);

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
        T: Into<isize>,
    {
        Self::update_pointer(report)
    }

    pub fn usage_to_char_109(usage: Usage, modifier: Modifier) -> char {
        let mut uni: char = INVALID_UNICHAR;

        if usage >= Usage::ALPHABET_MIN && usage <= Usage::ALPHABET_MAX {
            uni = (usage.0 - Usage::ALPHABET_MIN.0 + 0x61) as char;
        } else if usage >= Usage::NUMBER_MIN && usage <= Usage::NON_ALPHABET_MAX {
            uni = USAGE_TO_CHAR_NON_ALPLABET[(usage.0 - Usage::NUMBER_MIN.0) as usize];
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
static USAGE_TO_CHAR_NON_ALPLABET: [char; 27] = [
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
