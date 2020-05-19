// Advanced Programmable Interrupt Controller

use super::cpu::*;
use super::msr::Msr;
use super::x86_64::*;
use alloc::boxed::Box;
use alloc::vec::*;
use core::ffi::c_void;
use core::ptr::*;

use crate::myos::io::graphics::*;
use crate::stdout;
use crate::*;

const IRQ_BASE: InterruptVector = InterruptVector(0x40);
const IRQ_LAPIC_TIMER: InterruptVector = InterruptVector(IRQ_BASE.0);

static mut APIC: Apic = Apic::new();

pub struct Apic {
    ioapics: Vec<Box<IoApic>>,
}

impl Apic {
    const fn new() -> Self {
        Apic {
            ioapics: Vec::new(),
        }
    }

    pub unsafe fn init(acpi_apic: &acpi::interrupt::Apic) {
        if acpi_apic.also_has_legacy_pics {
            // disable legacy PICs
            Cpu::out8(0xA1, 0xFF);
            Cpu::out8(0x21, 0xFF);
        }

        // disable IRQ
        Cpu::disable();

        LocalApic::init(acpi_apic.local_apic_address as usize);

        // enable IRQ
        Cpu::enable();

        for acpi_ioapic in &acpi_apic.io_apics {
            APIC.ioapics.push(Box::new(IoApic::new(acpi_ioapic)));
        }

        let irq_ps2 = IRQ_BASE + 1;
        InterruptDescriptorTable::register(irq_ps2, LinearAddress(ps2_handler as usize));

        let ioapic = APIC.ioapics[0].as_ref();
        ioapic.write(IoApicIndex(0x13), 0x00000000);
        ioapic.write(IoApicIndex(0x12), 0x00000000 | irq_ps2.0 as u32);
    }
}

static mut LOCAL_APIC_BASE: Option<NonNull<c_void>> = None;

#[allow(dead_code)]
#[repr(usize)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
enum LocalApic {
    Id = 0x20,
    Version = 0x30,
    TaskPriority = 0x80,
    Eoi = 0xB0,
    SpuriousInterrupt = 0xF0,
    InterruptCommand = 0x300,
    InterruptCommandHigh = 0x310,
    LvtTimer = 0x320,
    LvtLint0 = 0x350,
    LvtLint1 = 0x360,
    LvtError = 0x370,
    TimerInitialCount = 0x380,
    TimerCurrentCount = 0x390,
    TimerDivideConfiguration = 0x3E0,
}

impl LocalApic {
    const IA32_APIC_BASE_MSR_BSP: u64 = 0x00000100;
    const IA32_APIC_BASE_MSR_ENABLE: u64 = 0x00000800;

    unsafe fn init(base: usize) {
        let ptr = base as *const c_void as *mut c_void;
        LOCAL_APIC_BASE = NonNull::new(ptr);

        let msr = Msr::Ia32ApicBase;
        let val = msr.read();
        msr.write(
            (val & Self::IA32_APIC_BASE_MSR_BSP)
                | ((base as u64 & !0xFFF) | Self::IA32_APIC_BASE_MSR_ENABLE),
        );

        InterruptDescriptorTable::register(IRQ_LAPIC_TIMER, LinearAddress(timer_handler as usize));

        // TODO: LAPIC Timer
        LocalApic::TimerDivideConfiguration.write(0x0000000B);
        // LocalApic::LvtTimer.write(0x00010020);
        LocalApic::LvtTimer.write(0x00020000 | IRQ_LAPIC_TIMER.0 as u32);
        LocalApic::TimerInitialCount.write(0x100000);
    }

    unsafe fn read(&self) -> u32 {
        let ptr = LOCAL_APIC_BASE.unwrap().as_ptr().add(*self as usize) as *const u32;
        ptr.read_volatile()
    }

    unsafe fn write(&self, value: u32) {
        let ptr = LOCAL_APIC_BASE.unwrap().as_ptr().add(*self as usize) as *const u32 as *mut u32;
        ptr.write_volatile(value);
    }

    unsafe fn eoi() {
        Self::Eoi.write(0);
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
struct IoApicIndex(u8);

struct IoApic {
    base: *mut u8,
    global_int: u32,
    id: u8,
}

impl IoApic {
    unsafe fn new(acpi_ioapic: &acpi::interrupt::IoApic) -> Self {
        IoApic {
            base: acpi_ioapic.address as usize as *const u8 as *mut u8,
            global_int: acpi_ioapic.global_system_interrupt_base,
            id: acpi_ioapic.id,
        }
    }

    unsafe fn read(&self, index: IoApicIndex) -> u32 {
        let ptr_index = self.base;
        let ptr_data = self.base.add(0x0010) as *const u32;
        // TODO: lock
        ptr_index.write_volatile(index.0);
        ptr_data.read_volatile()
    }

    unsafe fn write(&self, index: IoApicIndex, data: u32) {
        let ptr_index = self.base;
        let ptr_data = self.base.add(0x0010) as *const u32 as *mut u32;
        // TODO: lock
        ptr_index.write_volatile(index.0);
        ptr_data.write_volatile(data);
    }
}

static mut TIMER_COUNTER: usize = 0;

extern "x86-interrupt" fn timer_handler(_stack_frame: &ExceptionStackFrame) {
    unsafe {
        TIMER_COUNTER += 0x123456;
        stdout()
            .fb()
            .fill_rect(Rect::new(30, 30, 20, 20), Color::from(TIMER_COUNTER as u32));
        LocalApic::eoi();
    }
}

extern "x86-interrupt" fn ps2_handler(_stack_frame: &ExceptionStackFrame) {
    unsafe {
        let mut al: u8;
        llvm_asm!("inb $$0x60, %al": "={al}"(al));
        print!(" {:02x}", al);
        LocalApic::eoi();
    }
}
