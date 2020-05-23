// Legacy PC's Low Pin Count Device

use crate::myos::arch::apic::*;
use crate::myos::arch::cpu::Cpu;
use crate::myos::mux::queue::*;
use crate::myos::thread::*;
use crate::stdout;
use crate::*;
use alloc::boxed::Box;
use bitflags::*;

pub struct LowPinCount {}

impl LowPinCount {
    pub unsafe fn init() {
        let _ = Ps2::init();
    }
}

#[allow(dead_code)]
static PS2_TO_HID: [u8; 256] = [
    0x00, 0x29, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x2D, 0x2E, 0x2A,
    0x2B, // 0
    0x14, 0x1A, 0x08, 0x15, 0x17, 0x1C, 0x18, 0x0C, 0x12, 0x13, 0x2F, 0x30, 0x28, 0xE0, 0x04,
    0x16, // 1
    0x07, 0x09, 0x0A, 0x0B, 0x0D, 0x0E, 0x0F, 0x33, 0x34, 0x35, 0xE1, 0x31, 0x1D, 0x1B, 0x06,
    0x19, // 2
    0x05, 0x11, 0x10, 0x36, 0x37, 0x38, 0xE5, 0x55, 0xE2, 0x2C, 0x39, 0x3A, 0x3B, 0x3C, 0x3D,
    0x3E, // 3
    0x3F, 0x40, 0x41, 0x42, 0x43, 0x53, 0x47, 0x5F, 0x60, 0x61, 0x56, 0x5C, 0x5D, 0x5E, 0x57,
    0x59, // 4
    0x5A, 0x5B, 0x62, 0x63, 0, 0, 0, 0x44, 0x45, 0, 0, 0, 0, 0, 0, 0, // 5
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 6
    0x88, 0, 0, 0x87, 0, 0, 0, 0, 0, 0x8A, 0, 0x8B, 0, 0x89, 0, 0, // 7
    //  ---0  ---1  ---2  ---3  ---4  ---5  ---6  ---7  ---8  ---9  ---A  ---B  ---C  ---D  ---E  ---F
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // E0 0
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x58, 0xE4, 0, 0, // E0 1
    0x7F, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x81, 0, // E0 2
    0x80, 0, 0, 0, 0, 0x54, 0, 0, 0xE6, 0, 0, 0, 0, 0, 0, 0, // E0 3
    0, 0, 0, 0, 0, 0, 0, 0x4A, 0x52, 0x4B, 0, 0x50, 0, 0x4F, 0, 0x4D, // E0 4
    0x51, 0x4E, 0x49, 0x4C, 0, 0, 0, 0, 0, 0, 0, 0xE3, 0xE7, 0x65, 0x66, 0, // E0 5
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // E0 6
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // E0 7
];

enum Ps2KeyState {
    Default,
    Extend,
}

static mut PS2: Option<Box<Ps2>> = None;

struct Ps2 {
    key_state: Ps2KeyState,
    q: Box<Queue<Ps2Data>>,
}

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

    const SCAN_EXTEND: Ps2Data = Ps2Data(0xE0);

    const fn is_break(&self) -> bool {
        (self.0 & 0x80) != 0
    }

    const fn scancode(&self) -> u8 {
        self.0 & 0x7F
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
        if Self::wait_for_write(10).is_err() {
            return Err(());
        }
        Self::write_command(Ps2Command::DISABLE_FIRST_PORT);
        Self::send_command(Ps2Command::DISABLE_SECOND_PORT, 1).unwrap();

        for _ in 0..16 {
            let _ = Self::read_data();
        }

        PS2 = Some(Box::new(Ps2 {
            q: Queue::<Ps2Data>::with_capacity(64),
            key_state: Ps2KeyState::Default,
        }));
        Apic::register(Irq(1), Self::irq_01).unwrap();
        Apic::register(Irq(12), Self::irq_12).unwrap();

        Self::send_command(Ps2Command::WRITE_CONFIG, 1).unwrap();
        Self::send_data(Ps2Data(0x47), 1).unwrap();

        Self::send_data(Ps2Data::RESET_COMMAND, 1).unwrap();
        Thread::usleep(100_000);
        Self::send_data(Ps2Data::ENABLE_SEND, 1).unwrap();

        Self::send_second_data(Ps2Data::ENABLE_SEND, 1).unwrap();

        Ok(())
    }

    // IRQ 01 PS/2 Keyboard
    fn irq_01(_irq: Irq) {
        unsafe {
            let ps2 = PS2.as_mut().unwrap();
            ps2.q.write(Self::read_data()).unwrap();
        }
    }

    // IRQ 12 PS/2 Mouse
    fn irq_12(_irq: Irq) {
        unsafe {
            let _al = Self::read_data();
            // print!(" {:02x}", al.0);
        }
    }

    unsafe fn read_data() -> Ps2Data {
        let mut al: Ps2Data;
        llvm_asm!("inb $$0x60, %al": "={al}"(al));
        al
    }

    unsafe fn write_data(data: Ps2Data) {
        llvm_asm!("outb %al, $$0x60":: "{al}"(data));
    }

    unsafe fn read_status() -> Ps2Status {
        let mut al: Ps2Status;
        llvm_asm!("inb $$0x64, %al": "={al}"(al));
        al
    }

    unsafe fn write_command(command: Ps2Command) {
        llvm_asm!("outb %al, $$0x64":: "{al}"(command));
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

    fn scan_to_usage(data: Ps2Data) -> u8 {
        unsafe {
            let ps2 = PS2.as_mut().unwrap();
            if data == Ps2Data::SCAN_EXTEND {
                ps2.key_state = Ps2KeyState::Extend;
                0
            } else {
                if data.is_break() {
                        ps2.key_state = Ps2KeyState::Default;
                    return 0;
                }
                let mut scan = data.scancode();
                match ps2.key_state {
                    Ps2KeyState::Extend => {
                        scan |= 0x80;
                        ps2.key_state = Ps2KeyState::Default;
                    }
                    _ => (),
                }
                PS2_TO_HID[scan as usize]
            }
        }
    }
}

pub fn get_key() -> Option<u8> {
    unsafe {
        let ps2 = PS2.as_mut().unwrap();
        ps2.q.read().map(|x| Ps2::scan_to_usage(x))
    }
}
