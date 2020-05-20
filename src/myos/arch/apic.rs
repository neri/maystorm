// Advanced Programmable Interrupt Controller

use super::cpu::*;
use super::msr::Msr;
use super::system::*;
use super::x86_64::*;
use alloc::boxed::Box;
use alloc::vec::*;
use core::ffi::c_void;
use core::ptr::*;

use crate::myos::io::graphics::*;
use crate::stdout;
use crate::*;

const MSI_BASE: usize = 0xFEE00000;
const APIC_REDIR_MASK: u32 = 0x00010000;

const IRQ_BASE: InterruptVector = InterruptVector(0x20);
const IRQ_LAPIC_TIMER: InterruptVector = InterruptVector(IRQ_BASE.0);
const MAX_IRQ: InterruptVector = InterruptVector(0x7F);
const MAX_MSI: usize = 24;

static mut APIC: Apic = Apic::new();

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ApicId(pub u32);

pub struct Apic {
    master_apic_id: ApicId,
    ioapics: Vec<Box<IoApic>>,
}

impl Apic {
    const fn new() -> Self {
        Apic {
            master_apic_id: ApicId(0),
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

        APIC.master_apic_id = System::shared().cpu(0).as_ref().apic_id;

        LocalApic::init(acpi_apic.local_apic_address as usize);

        // enable IRQ
        Cpu::enable();

        for acpi_ioapic in &acpi_apic.io_apics {
            APIC.ioapics.push(Box::new(IoApic::new(acpi_ioapic)));
        }

        Self::register(1, LinearAddress(ps2_handler as usize)).unwrap();
    }

    unsafe fn register(irq: u8, f: LinearAddress) -> Result<(), ()> {
        let gsi = irq; // TODO
        let polarity = ApicPolarity::ActiveHigh; // TODO
        let trigger = ApicTriggerMode::Edge; // TODO

        for ioapic in APIC.ioapics.iter() {
            if ioapic.global_int <= gsi && (gsi - ioapic.global_int) < ioapic.entries {
                let local_irq = gsi - ioapic.global_int;
                let vec_irq = IRQ_BASE + gsi as usize;
                InterruptDescriptorTable::register(vec_irq, f);
                let pair = Self::make_redirect_table_entry_pair(
                    vec_irq,
                    polarity,
                    trigger,
                    APIC.master_apic_id,
                );
                ioapic.write(IoApicIndex(0x10 + local_irq * 2 + 1), pair.1);
                ioapic.write(IoApicIndex(0x10 + local_irq * 2), pair.0);
                return Ok(());
            }
        }
        Err(())
    }

    const fn make_redirect_table_entry_pair(
        vec: InterruptVector,
        polarity: ApicPolarity,
        trigger: ApicTriggerMode,
        apic_id: ApicId,
    ) -> (u32, u32) {
        (
            vec.0 as u32 | polarity.as_redir() | trigger.as_redir(),
            apic_id.0 << 24,
        )
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum ApicPolarity {
    ActiveHigh = 0,
    ActiveLow = 1,
}

impl ApicPolarity {
    const fn as_redir(&self) -> u32 {
        (*self as u32) << 13
    }
}

impl From<acpi::interrupt::Polarity> for ApicPolarity {
    fn from(src: acpi::interrupt::Polarity) -> Self {
        match src {
            acpi::interrupt::Polarity::SameAsBus => ApicPolarity::ActiveHigh,
            acpi::interrupt::Polarity::ActiveHigh => ApicPolarity::ActiveHigh,
            acpi::interrupt::Polarity::ActiveLow => ApicPolarity::ActiveLow,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum ApicTriggerMode {
    Edge = 0,
    Level = 1,
}

impl ApicTriggerMode {
    const fn as_redir(&self) -> u32 {
        (*self as u32) << 15
    }
}

impl From<acpi::interrupt::TriggerMode> for ApicTriggerMode {
    fn from(src: acpi::interrupt::TriggerMode) -> Self {
        match src {
            acpi::interrupt::TriggerMode::SameAsBus => ApicTriggerMode::Edge,
            acpi::interrupt::TriggerMode::Edge => ApicTriggerMode::Edge,
            acpi::interrupt::TriggerMode::Level => ApicTriggerMode::Level,
        }
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

        let msr = Msr::ApicBase;
        let val = msr.read();
        msr.write(
            (val & Self::IA32_APIC_BASE_MSR_BSP)
                | ((base as u64 & !0xFFF) | Self::IA32_APIC_BASE_MSR_ENABLE),
        );

        InterruptDescriptorTable::register(IRQ_LAPIC_TIMER, LinearAddress(timer_handler as usize));

        // LAPIC timer
        // TODO: magic words
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

const IOAPIC_ID: IoApicIndex = IoApicIndex(0);
const IOAPIC_VER: IoApicIndex = IoApicIndex(1);

struct IoApic {
    base: *mut u8,
    global_int: u8,
    entries: u8,
    id: u8,
}

impl IoApic {
    unsafe fn new(acpi_ioapic: &acpi::interrupt::IoApic) -> Self {
        let mut ioapic = IoApic {
            base: acpi_ioapic.address as usize as *const u8 as *mut u8,
            global_int: acpi_ioapic.global_system_interrupt_base as u8,
            entries: 0,
            id: acpi_ioapic.id,
        };
        let ver = ioapic.read(IOAPIC_VER);
        ioapic.entries = 1 + (ver >> 16) as u8;
        ioapic
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
        stdout().fb().fill_rect(
            Rect::new(100, 100, 20, 20),
            Color::from(TIMER_COUNTER as u32),
        );
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
