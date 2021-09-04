//! Human Interface Device Manager

use crate::sync::atomicflags::AtomicBitflags;
use crate::sync::RwLock;
use crate::ui::window::*;
use crate::*;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use bitflags::*;
use core::cell::UnsafeCell;
use core::num::*;
use core::sync::atomic::{AtomicUsize, Ordering};
use megstd::drawing::*;
use megstd::io::hid::*;

const INVALID_UNICHAR: char = '\u{FEFF}';

bitflags! {
    pub struct KeyEventFlags: u8 {
        const BREAK = 0b1000_0000;
    }
}

/// USB HID BOOT Keyboard Raw Report
#[repr(C, packed)]
#[derive(Debug, Copy, Clone, Default)]
pub struct KeyReportRaw {
    pub modifier: Modifier,
    _reserved_1: u8,
    pub keydata: [Usage; 6],
}

impl KeyReportRaw {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            modifier: Modifier::empty(),
            _reserved_1: 0,
            keydata: [Usage::NONE; 6],
        }
    }
}

#[derive(Debug)]
pub struct KeyboardState {
    pub current: KeyReportRaw,
    pub prev: KeyReportRaw,
}

impl KeyboardState {
    #[inline]
    pub const fn new() -> Self {
        Self {
            current: KeyReportRaw::empty(),
            prev: KeyReportRaw::empty(),
        }
    }

    pub fn process_key_report(&mut self, report: KeyReportRaw) {
        self.prev = self.current;
        self.current = report;
        for modifier in Usage::MOD_MIN.0..Usage::MOD_MAX.0 {
            let bit = 1u8 << (modifier - Usage::MOD_MIN.0);
            if (self.current.modifier.bits() & bit) == 0 && (self.prev.modifier.bits() & bit) != 0 {
                KeyEvent::new(Usage(modifier), Modifier::empty(), KeyEventFlags::BREAK).post();
            }
        }
        for modifier in Usage::MOD_MIN.0..Usage::MOD_MAX.0 {
            let bit = 1u8 << (modifier - Usage::MOD_MIN.0);
            if (self.current.modifier.bits() & bit) != 0 && (self.prev.modifier.bits() & bit) == 0 {
                KeyEvent::new(Usage(modifier), Modifier::empty(), KeyEventFlags::empty()).post();
            }
        }
        for usage in &self.prev.keydata {
            let usage = *usage;
            if usage != Usage::NONE && !self.current.keydata.contains(&usage) {
                KeyEvent::new(usage, Modifier::empty(), KeyEventFlags::BREAK).post();
            }
        }
        for usage in &self.current.keydata {
            let usage = *usage;
            if usage != Usage::NONE && !self.prev.keydata.contains(&usage) {
                KeyEvent::new(usage, Modifier::empty(), KeyEventFlags::empty()).post();
            }
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct KeyEvent(pub NonZeroU32);

impl KeyEvent {
    #[inline]
    pub const fn new(usage: Usage, modifier: Modifier, flags: KeyEventFlags) -> Self {
        unsafe {
            Self(NonZeroU32::new_unchecked(
                usage.0 as u32 | ((modifier.bits() as u32) << 16) | ((flags.bits() as u32) << 24),
            ))
        }
    }

    #[inline]
    pub fn into_char(self) -> char {
        HidManager::key_event_to_char(self)
    }

    #[inline]
    pub const fn usage(self) -> Usage {
        Usage(self.0.get() as u8)
    }

    #[inline]
    pub const fn modifier(self) -> Modifier {
        Modifier::from_bits_truncate(((self.0.get() >> 16) & 0xFF) as u8)
    }

    #[inline]
    pub const fn flags(self) -> KeyEventFlags {
        unsafe { KeyEventFlags::from_bits_unchecked(((self.0.get() >> 24) & 0xFF) as u8) }
    }

    #[inline]
    pub fn is_make(&self) -> bool {
        !self.is_break()
    }

    #[inline]
    pub fn is_break(&self) -> bool {
        self.flags().contains(KeyEventFlags::BREAK)
    }

    /// Returns the data for which a valid key was pressed. Otherwise, it is None.
    #[inline]
    pub fn key_data(self) -> Option<Self> {
        if self.usage() != Usage::NONE
            && !(self.usage() >= Usage::MOD_MIN && self.usage() <= Usage::MOD_MAX)
            && !self.is_break()
        {
            Some(self)
        } else {
            None
        }
    }

    #[inline]
    pub fn post(self) {
        HidManager::post_key_event(self);
    }
}

impl Into<char> for KeyEvent {
    #[inline]
    fn into(self) -> char {
        self.into_char()
    }
}

/// USB HID BOOT Mouse Raw Report
pub type MouseReportRaw = MouseReport<i8>;

#[derive(Debug, Copy, Clone, Default)]
pub struct MouseState {
    pub current_buttons: MouseButton,
    pub prev_buttons: MouseButton,
    pub x: isize,
    pub y: isize,
}

impl MouseState {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            current_buttons: MouseButton::empty(),
            prev_buttons: MouseButton::empty(),
            x: 0,
            y: 0,
        }
    }

    #[inline]
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
    #[inline]
    pub const fn new(point: Point, buttons: MouseButton, event_buttons: MouseButton) -> Self {
        Self {
            x: point.x as i16,
            y: point.y as i16,
            buttons,
            event_buttons,
        }
    }

    #[inline]
    pub const fn point(&self) -> Point {
        Point {
            x: self.x as isize,
            y: self.y as isize,
        }
    }

    #[inline]
    pub const fn buttons(&self) -> MouseButton {
        self.buttons
    }

    #[inline]
    pub const fn event_buttons(&self) -> MouseButton {
        self.event_buttons
    }
}

/// HidManager relays between human interface devices and the window event subsystem.
///
/// Keyboard scancodes will be converted to the Usage specified by the USB-HID specification on all platforms.
pub struct HidManager {
    key_modifier: AtomicBitflags<Modifier>,
    simulated_game_input: RwLock<GameInput>,
    game_inputs: RwLock<BTreeMap<GameInputHandle, Arc<RwLock<GameInput>>>>,
    current_game_inputs: RwLock<Option<GameInputHandle>>,
}

static mut HID_MANAGER: UnsafeCell<HidManager> = UnsafeCell::new(HidManager::new());

impl HidManager {
    #[inline]
    const fn new() -> Self {
        HidManager {
            key_modifier: AtomicBitflags::empty(),
            simulated_game_input: RwLock::new(GameInput::empty()),
            game_inputs: RwLock::new(BTreeMap::new()),
            current_game_inputs: RwLock::new(None),
        }
    }

    #[inline]
    pub unsafe fn init() {
        //
    }

    #[inline]
    fn shared<'a>() -> &'a HidManager {
        unsafe { &*HID_MANAGER.get() }
    }

    fn post_key_event(event: KeyEvent) {
        let shared = Self::shared();
        let usage = event.usage();
        if usage >= Usage::MOD_MIN && usage <= Usage::MOD_MAX {
            let bit_position = Modifier::from_bits_truncate(1 << (usage.0 - Usage::MOD_MIN.0));
            shared.key_modifier.set(bit_position, !event.is_break());
        }
        let event = KeyEvent::new(usage, shared.key_modifier.value(), event.flags());
        WindowManager::post_key_event(event);
    }

    #[inline]
    fn key_event_to_char(event: KeyEvent) -> char {
        if event.flags().contains(KeyEventFlags::BREAK) || event.usage() == Usage::NONE {
            '\0'
        } else {
            Self::usage_to_char_109(event.usage(), event.modifier())
        }
    }

    fn usage_to_char_109(usage: Usage, modifier: Modifier) -> char {
        let mut uni: char = INVALID_UNICHAR;

        if usage >= Usage::ALPHABET_MIN && usage <= Usage::ALPHABET_MAX {
            uni = (usage.0 - Usage::KEY_A.0 + 0x61) as char;
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

pub struct GameInputManager;

impl GameInputManager {
    pub fn current_input() -> GameInput {
        let shared = HidManager::shared();

        let game_input = shared.game_inputs.read().unwrap();
        shared
            .current_game_inputs
            .read()
            .unwrap()
            .and_then(|key| game_input.get(&key))
            .map(|v| v.read().unwrap().clone())
            .unwrap_or(shared.simulated_game_input.read().unwrap().clone())
    }

    #[inline]
    fn next_game_input_handle() -> Option<GameInputHandle> {
        static NEXT_HANDLE: AtomicUsize = AtomicUsize::new(1);
        NonZeroUsize::new(NEXT_HANDLE.fetch_add(1, Ordering::AcqRel)).map(|v| GameInputHandle(v))
    }

    pub fn connect_new_input(input: Arc<RwLock<GameInput>>) -> Option<GameInputHandle> {
        Self::next_game_input_handle().map(|handle| {
            let shared = HidManager::shared();
            shared
                .game_inputs
                .write()
                .unwrap()
                .insert(handle, input.clone());
            *shared.current_game_inputs.write().unwrap() = Some(handle);
            handle
        })
    }

    pub fn send_key(event: KeyEvent) {
        let position = match event.usage() {
            Usage::NUMPAD_2 => Some(GameInputButtonType::DpadDown),
            Usage::NUMPAD_4 => Some(GameInputButtonType::DpadLeft),
            Usage::NUMPAD_6 => Some(GameInputButtonType::DpadRight),
            Usage::NUMPAD_8 => Some(GameInputButtonType::DpadUp),
            Usage::KEY_UP_ARROW => Some(GameInputButtonType::DpadUp),
            Usage::KEY_DOWN_ARROW => Some(GameInputButtonType::DpadDown),
            Usage::KEY_RIGHT_ARROW => Some(GameInputButtonType::DpadRight),
            Usage::KEY_LEFT_ARROW => Some(GameInputButtonType::DpadLeft),
            Usage::KEY_W => Some(GameInputButtonType::DpadUp),
            Usage::KEY_A => Some(GameInputButtonType::DpadLeft),
            Usage::KEY_S => Some(GameInputButtonType::DpadDown),
            Usage::KEY_D => Some(GameInputButtonType::DpadRight),

            Usage::KEY_ESCAPE => Some(GameInputButtonType::Menu),
            Usage::KEY_ENTER => Some(GameInputButtonType::Start),
            Usage::KEY_SPACE => Some(GameInputButtonType::Select),

            Usage::KEY_Z => Some(GameInputButtonType::A),
            Usage::KEY_X => Some(GameInputButtonType::B),
            Usage::KEY_LEFT_CONTROL => Some(GameInputButtonType::B),
            Usage::KEY_LEFT_SHIFT => Some(GameInputButtonType::A),
            Usage::KEY_RIGHT_CONTROL => Some(GameInputButtonType::B),
            Usage::KEY_RIGHT_SHIFT => Some(GameInputButtonType::A),

            _ => None,
        };
        if let Some(position) = position {
            let position = 1u16 << (position as usize);
            let mut buttons = HidManager::shared().simulated_game_input.write().unwrap();
            if event.is_break() {
                buttons.bitmap &= !position;
            } else {
                buttons.bitmap |= position;
            }
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GameInputHandle(pub NonZeroUsize);

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GameInput {
    bitmap: u16,
    lt: u8,
    rt: u8,
    x1: u16,
    y1: u16,
    x2: u16,
    y2: u16,
}

impl GameInput {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            bitmap: 0,
            lt: 0,
            rt: 0,
            x1: 0,
            y1: 0,
            x2: 0,
            y2: 0,
        }
    }

    #[inline]
    pub const fn buttons(&self) -> u16 {
        self.bitmap
    }

    #[inline]
    pub fn copy_from(&mut self, other: &Self) {
        unsafe {
            (self as *mut Self).copy_from(other as *const Self, 1);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GameInputButtonType {
    DpadUp = 0,
    DpadDown,
    DpadLeft,
    DpadRight,
    Start,
    Select,
    ThumbL,
    ThumbR,
    LButton,
    RButton,
    Menu,
    _Reserved,
    A,
    B,
    X,
    Y,
}
