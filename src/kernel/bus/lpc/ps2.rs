// PS/2 Device Driver

use crate::kernel::arch::apic::*;
use crate::kernel::arch::cpu::Cpu;
use crate::kernel::io::hid::*;
use crate::kernel::scheduler::*;
use crate::kernel::sync::queue::*;
use crate::kernel::sync::semaphore::*;
use crate::*;
use alloc::boxed::Box;
use bitflags::*;
use core::num::*;
use core::ptr::*;

static mut PS2: Option<Box<Ps2>> = None;

pub(crate) struct Ps2 {
    key_state: Ps2KeyState,
    mouse_phase: Ps2MousePhase,
    modifier: Modifier,
    mouse_buf: [Ps2Data; 3],
    sem: Semaphore,
    buf: Box<AtomicLinkedQueue<CompositePs2Data>>,
}

enum Ps2KeyState {
    Default,
    Extend,
}

bitflags! {
    struct MouseLeadByte: u8 {
        const LEFT_BUTTON = 0b0000_0001;
        const RIGHT_BUTTON = 0b0000_0010;
        const MIDDLE_BUTTON = 0b0000_0100;
        const ALWAYS_ONE = 0b0000_1000;
        const X_SIGN = 0b0001_0000;
        const Y_SIGN = 0b0010_0000;
        const X_OVERFLOW = 0b0100_0000;
        const Y_OVERFLOW = 0b1000_0000;

        const BUTTONS = Self::LEFT_BUTTON.bits | Self::RIGHT_BUTTON.bits | Self::MIDDLE_BUTTON.bits;
    }
}

impl From<Ps2Data> for MouseLeadByte {
    fn from(data: Ps2Data) -> Self {
        unsafe { MouseLeadByte::from_bits_unchecked(data.0) }
    }
}

impl Into<MouseButton> for MouseLeadByte {
    fn into(self) -> MouseButton {
        unsafe { MouseButton::from_bits_unchecked(self.bits() & MouseLeadByte::BUTTONS.bits()) }
    }
}

#[derive(Debug, Copy, Clone)]
enum Ps2MousePhase {
    Ack,
    Leading,
    X,
    Y,
}

impl Ps2MousePhase {
    fn next(&mut self) {
        *self = match *self {
            Ps2MousePhase::Ack => Ps2MousePhase::Leading,
            Ps2MousePhase::Leading => Ps2MousePhase::X,
            Ps2MousePhase::X => Ps2MousePhase::Y,
            Ps2MousePhase::Y => Ps2MousePhase::Leading,
        }
    }

    fn as_index(self) -> usize {
        match self {
            Ps2MousePhase::Leading => 0,
            Ps2MousePhase::X => 1,
            Ps2MousePhase::Y => 2,
            _ => 0,
        }
    }
}

// PS2 scan code to HID usage table
static PS2_TO_HID: [u8; 256] = [
    0x00, 0x29, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x2D, 0x2E, 0x2A, 0x2B,
    0x14, 0x1A, 0x08, 0x15, 0x17, 0x1C, 0x18, 0x0C, 0x12, 0x13, 0x2F, 0x30, 0x28, 0xE0, 0x04, 0x16,
    0x07, 0x09, 0x0A, 0x0B, 0x0D, 0x0E, 0x0F, 0x33, 0x34, 0x35, 0xE1, 0x31, 0x1D, 0x1B, 0x06, 0x19,
    0x05, 0x11, 0x10, 0x36, 0x37, 0x38, 0xE5, 0x55, 0xE2, 0x2C, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E,
    0x3F, 0x40, 0x41, 0x42, 0x43, 0x53, 0x47, 0x5F, 0x60, 0x61, 0x56, 0x5C, 0x5D, 0x5E, 0x57, 0x59,
    0x5A, 0x5B, 0x62, 0x63, 0, 0, 0, 0x44, 0x45, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0x88, 0, 0, 0x87, 0, 0, 0, 0, 0, 0x8A, 0, 0x8B, 0, 0x89, 0, 0,
    // ----
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x58, 0xE4,
    0, 0, 0x7F, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x81, 0, 0x80, 0, 0, 0, 0, 0x54, 0, 0, 0xE6,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x4A, 0x52, 0x4B, 0, 0x50, 0, 0x4F, 0, 0x4D, 0x51,
    0x4E, 0x49, 0x4C, 0, 0, 0, 0, 0, 0, 0, 0xE3, 0xE7, 0x65, 0x66, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq)]
struct Ps2Data(pub u8);

#[allow(dead_code)]
impl Ps2Data {
    const ACK: Ps2Data = Ps2Data(0xFA);
    const NAK: Ps2Data = Ps2Data(0xFE);
    const ECHO: Ps2Data = Ps2Data(0xEE);

    const RESET_COMMAND: Ps2Data = Ps2Data(0xFF);
    const ENABLE_SEND: Ps2Data = Ps2Data(0xF4);
    const DISABLE_SEND: Ps2Data = Ps2Data(0xF5);
    const SET_DEFAULT: Ps2Data = Ps2Data(0xF6);

    const SCAN_EXTEND: Ps2Data = Ps2Data(0xE0);

    const fn is_break(self) -> bool {
        (self.0 & 0x80) != 0
    }

    const fn scancode(self) -> u8 {
        self.0 & 0x7F
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialOrd, PartialEq)]
struct CompositePs2Data(pub NonZeroUsize);

enum Ps2DataType {
    None,
    Key(Ps2Data),
    Mouse(Ps2Data),
}

impl CompositePs2Data {
    const DUMMY: CompositePs2Data =
        CompositePs2Data(unsafe { NonZeroUsize::new_unchecked(0xFFFFFFFF) });
    const KEY_MIN: usize = 0x001;
    const KEY_MAX: usize = 0x0FF;
    const MOUSE_MIN: usize = 0x100;
    const MOUSE_MAX: usize = 0x1FF;

    const fn key(value: Ps2Data) -> Self {
        Self(unsafe { NonZeroUsize::new_unchecked(value.0 as usize) })
    }

    const fn mouse(value: Ps2Data) -> Self {
        Self(unsafe { NonZeroUsize::new_unchecked(CompositePs2Data::MOUSE_MIN + value.0 as usize) })
    }

    fn split(self) -> Ps2DataType {
        match self.0.get() {
            CompositePs2Data::KEY_MIN..=CompositePs2Data::KEY_MAX => {
                Ps2DataType::Key(Ps2Data(self.0.get() as u8))
            }
            CompositePs2Data::MOUSE_MIN..=CompositePs2Data::MOUSE_MAX => {
                Ps2DataType::Mouse(Ps2Data((self.0.get() - CompositePs2Data::MOUSE_MIN) as u8))
            }
            _ => Ps2DataType::None,
        }
    }
}

impl From<NonZeroUsize> for CompositePs2Data {
    fn from(value: NonZeroUsize) -> Self {
        Self(value)
    }
}

impl Into<NonZeroUsize> for CompositePs2Data {
    fn into(self) -> NonZeroUsize {
        self.0
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq)]
struct Ps2Command(pub u8);

#[allow(dead_code)]
impl Ps2Command {
    const WRITE_CONFIG: Ps2Command = Ps2Command(0x60);
    const DISABLE_SECOND_PORT: Ps2Command = Ps2Command(0xA7);
    const ENABLE_SECOND_PORT: Ps2Command = Ps2Command(0xA8);
    const DISABLE_FIRST_PORT: Ps2Command = Ps2Command(0xAD);
    const ENABLE_FIRST_PORT: Ps2Command = Ps2Command(0xAE);
    const WRITE_SECOND_PORT: Ps2Command = Ps2Command(0xD4);
}

bitflags! {
    struct Ps2Status: u8 {
        const OUTPUT_FULL = 0b0000_0001;
        const INPUT_FULL = 0b0000_0010;
        const SYSTEM_FLAG = 0b0000_0100;
        const COMMAND = 0b0000_1000;
        const TIMEOUT_ERROR = 0b0100_0000;
        const PARITY_ERROR = 0b1000_0000;
    }
}

#[allow(dead_code)]
impl Ps2 {
    const WRITE_TIMEOUT: u64 = 10_000;
    const READ_TIMEOUT: u64 = 100_000;

    pub unsafe fn init() -> Result<(), ()> {
        // NO PS/2 Controller
        if Self::wait_for_write(10).is_err() {
            return Err(());
        }

        Self::write_command(Ps2Command::DISABLE_FIRST_PORT);
        Self::send_command(Ps2Command::DISABLE_SECOND_PORT, 1).unwrap();

        for _ in 0..16 {
            let _ = Self::read_data();
        }

        PS2 = Some(Box::new(Ps2 {
            sem: Semaphore::new(0),
            buf: AtomicLinkedQueue::with_capacity(256),
            key_state: Ps2KeyState::Default,
            mouse_phase: Ps2MousePhase::Ack,
            modifier: Modifier::empty(),
            mouse_buf: [Ps2Data(0); 3],
        }));
        Irq::LPC_PS2K.register(Self::irq_01).unwrap();
        Irq::LPC_PS2M.register(Self::irq_12).unwrap();
        GlobalScheduler::spawn_f(Self::data_thread, null_mut(), Priority::Realtime);

        Self::send_command(Ps2Command::WRITE_CONFIG, 1).unwrap();
        Self::send_data(Ps2Data(0x47), 1).unwrap();

        Self::send_command(Ps2Command::ENABLE_FIRST_PORT, 1).unwrap();
        Self::send_command(Ps2Command::ENABLE_SECOND_PORT, 1).unwrap();

        Self::send_data(Ps2Data::RESET_COMMAND, 1).unwrap();
        Timer::usleep(100_000);
        Self::send_data(Ps2Data::ENABLE_SEND, 1).unwrap();

        Self::send_second_data(Ps2Data::RESET_COMMAND, 1).unwrap();
        Timer::usleep(100_000);
        // Self::send_second_data(Ps2Data::SET_DEFAULT, 1).unwrap();
        Self::send_second_data(Ps2Data::ENABLE_SEND, 1).unwrap();

        Ok(())
    }

    unsafe fn read_data() -> Ps2Data {
        let mut al: u8;
        asm!("in al, 0x60", lateout("al") al);
        Ps2Data(al)
    }

    unsafe fn write_data(data: Ps2Data) {
        asm!("out 0x60, al", in("al") data.0);
    }

    unsafe fn read_status() -> Ps2Status {
        let mut al: u8;
        asm!("in al, 0x64", lateout("al") al);
        Ps2Status::from_bits_unchecked(al)
    }

    unsafe fn write_command(command: Ps2Command) {
        asm!("out 0x64, al", in("al") command.0);
    }

    unsafe fn wait_for_write(timeout: u64) -> Result<(), ()> {
        let deadline = Timer::new(TimeMeasure::from_micros(Self::WRITE_TIMEOUT * timeout));
        while deadline.until() {
            if Self::read_status().contains(Ps2Status::INPUT_FULL) {
                Cpu::relax();
            } else {
                return Ok(());
            }
        }
        Err(())
    }

    unsafe fn wait_for_read(timeout: u64) -> Result<(), ()> {
        let deadline = Timer::new(TimeMeasure::from_micros(timeout * Self::READ_TIMEOUT));
        while deadline.until() {
            if Self::read_status().contains(Ps2Status::OUTPUT_FULL) {
                return Ok(());
            } else {
                Cpu::relax();
            }
        }
        Err(())
    }

    // Wait for write, then command
    unsafe fn send_command(command: Ps2Command, timeout: u64) -> Result<(), ()> {
        Self::wait_for_write(timeout).and_then(|_| {
            Self::write_command(command);
            Ok(())
        })
    }

    // Wait for write, then data
    unsafe fn send_data(data: Ps2Data, timeout: u64) -> Result<(), ()> {
        Self::wait_for_write(timeout).and_then(|_| {
            Self::write_data(data);
            Ok(())
        })
    }

    // Send to second port (mouse)
    unsafe fn send_second_data(data: Ps2Data, timeout: u64) -> Result<(), ()> {
        Self::send_command(Ps2Command::WRITE_SECOND_PORT, timeout)
            .and_then(|_| Self::send_data(data, timeout))
    }

    // IRQ 01 PS/2 Keyboard
    fn irq_01(_irq: Irq) {
        let ps2 = unsafe { PS2.as_mut().unwrap() };
        let al = unsafe { Self::read_data() };
        if ps2.buf.enqueue(CompositePs2Data::key(al)).is_ok() {
            ps2.sem.signal();
        }
    }

    // IRQ 12 PS/2 Mouse
    fn irq_12(_irq: Irq) {
        let ps2 = unsafe { PS2.as_mut().unwrap() };
        let al = unsafe { Self::read_data() };
        if ps2.buf.enqueue(CompositePs2Data::mouse(al)).is_ok() {
            ps2.sem.signal();
        }
    }

    // PS/2 thread
    fn data_thread(_args: *mut c_void) {
        let ps2 = unsafe { PS2.as_mut().unwrap() };
        loop {
            let _ = ps2.sem.wait(TimeMeasure::FOREVER);
            loop {
                match ps2.buf.dequeue().unwrap_or(CompositePs2Data::DUMMY).split() {
                    Ps2DataType::Key(data) => ps2.process_key_data(data),
                    Ps2DataType::Mouse(data) => ps2.process_mouse_data(data),
                    Ps2DataType::None => break,
                }
            }
        }
    }

    fn process_key_data(&mut self, data: Ps2Data) {
        if data == Ps2Data::SCAN_EXTEND {
            self.key_state = Ps2KeyState::Extend;
        } else {
            let mut scan = data.scancode();
            match self.key_state {
                Ps2KeyState::Extend => {
                    scan |= 0x80;
                    self.key_state = Ps2KeyState::Default;
                }
                _ => (),
            }
            let usage = Usage(PS2_TO_HID[scan as usize]);
            if usage >= Usage::MOD_MIN && usage < Usage::MOD_MAX {
                let bit_position =
                    unsafe { Modifier::from_bits_unchecked(1 << (usage.0 - Usage::MOD_MIN.0)) };
                if data.is_break() {
                    self.modifier.remove(bit_position);
                } else {
                    self.modifier.insert(bit_position);
                }
            } else {
                if !data.is_break() {
                    HidManager::send_key_event(KeyEvent::new(usage, self.modifier));
                }
            }
        }
    }

    fn process_mouse_data(&mut self, data: Ps2Data) {
        match self.mouse_phase {
            Ps2MousePhase::Ack => {
                if data == Ps2Data::ACK {
                    self.mouse_phase.next();
                }
            }
            Ps2MousePhase::Leading => {
                if (data.0 & 0xC8) == 0x08 {
                    self.mouse_buf[self.mouse_phase.as_index()] = data;
                    self.mouse_phase.next();
                }
            }
            Ps2MousePhase::X => {
                self.mouse_buf[self.mouse_phase.as_index()] = data;
                self.mouse_phase.next();
            }
            Ps2MousePhase::Y => {
                self.mouse_buf[self.mouse_phase.as_index()] = data;
                self.mouse_phase.next();

                fn conv_movement(data: Ps2Data, sign: bool) -> i16 {
                    if sign {
                        ((data.0 as u16) | 0xFF00) as i16
                    } else {
                        data.0 as i16
                    }
                }
                let lead = MouseLeadByte::from(self.mouse_buf[0]);
                let x = conv_movement(self.mouse_buf[1], lead.contains(MouseLeadByte::X_SIGN));
                let y = conv_movement(self.mouse_buf[2], lead.contains(MouseLeadByte::Y_SIGN));
                let report = MouseReport {
                    buttons: lead.into(),
                    x: x,
                    y: 0 - y,
                };
                HidManager::process_mouse_report(report);
            }
        }
    }
}
