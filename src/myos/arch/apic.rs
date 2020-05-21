// Advanced Programmable Interrupt Controller

use super::cpu::*;
use super::msr::Msr;
use super::system::*;
use alloc::boxed::Box;
use alloc::vec::*;
use core::ffi::c_void;
use core::ptr::*;

use crate::myos::io::graphics::*;
use crate::stdout;
use crate::*;

// const MSI_BASE: usize = 0xFEE00000;
const APIC_REDIR_MASK: u32 = 0x00010000;
const MAX_CPU: usize = 64;

static mut APIC: Apic = Apic::new();
static mut LOCAL_APIC_BASE: Option<NonNull<c_void>> = None;

extern "efiapi" {
    fn setup_smp_init(vec_sipi: u8, max_cpu: usize, stack_chunk_size: usize, stack_base: Vec<u8>);
}

#[no_mangle]
pub extern "efiapi" fn apic_start_ap(cpuid: usize) {
    println!("Start AP {}", cpuid);
    let mut color: u32 = 0;
    loop {
        color += 0x1234 * cpuid as u32;
        stdout().fb().fill_rect(
            Rect::new(cpuid as isize * 100, 50, 20, 20),
            Color::from(color),
        );
        unsafe {
            for _ in 0..64 {
                llvm_asm!("pause");
            }
        }
    }
}

pub struct Apic {
    master_apic_id: ApicId,
    ioapics: Vec<Box<IoApic>>,
    gsi_table: [GsiProps; 256],
}

impl Apic {
    const fn new() -> Self {
        Apic {
            master_apic_id: ApicId(0),
            ioapics: Vec::new(),
            gsi_table: [GsiProps::null(); 256],
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

        // define default gsi table for PS/2 Keyboard
        APIC.gsi_table[1] = GsiProps {
            gsi: Irq(1),
            polarity: PackedPolarity(0),
        };

        // import gsi table from ACPI
        for source in &acpi_apic.interrupt_source_overrides {
            let props = GsiProps {
                gsi: Irq(source.global_system_interrupt as u8),
                polarity: PackedPolarity::new(
                    ApicPolarity::from(&source.polarity),
                    ApicTriggerMode::from(&source.trigger_mode),
                ),
            };
            APIC.gsi_table[source.isa_source as usize] = props;
        }

        // enable IRQ
        Cpu::enable();

        for acpi_ioapic in &acpi_apic.io_apics {
            APIC.ioapics.push(Box::new(IoApic::new(acpi_ioapic)));
        }

        Self::register(Irq(1), LinearAddress(ps2_handler as usize)).unwrap();

        // Setup SMP
        let max_cpu = core::cmp::min(System::shared().number_of_cpus(), MAX_CPU);
        let stack_chunk_size = 0x4000;
        let stack_base: Vec<u8> = Vec::with_capacity(max_cpu * stack_chunk_size);
        setup_smp_init(1, max_cpu, stack_chunk_size, stack_base);

        // SIPI
        LocalApic::InterruptCommand.write(0x000C4500);
        Cpu::halt();
        LocalApic::InterruptCommand.write(0x000C4601);
        Cpu::halt();
    }

    unsafe fn register(irq: Irq, f: LinearAddress) -> Result<(), ()> {
        let props = APIC.gsi_table[irq.0 as usize];
        let gsi = props.gsi;
        let polarity = props.polarity;

        for ioapic in APIC.ioapics.iter() {
            let local_irq = gsi.0 - ioapic.global_int.0;
            if ioapic.global_int <= gsi && local_irq < ioapic.entries {
                let vec_irq = gsi.as_vec();
                InterruptDescriptorTable::register(vec_irq, f);
                let pair =
                    Self::make_redirect_table_entry_pair(vec_irq, polarity, APIC.master_apic_id);
                ioapic.write(IoApicIndex::redir_table_high(local_irq), pair.1);
                ioapic.write(IoApicIndex::redir_table_low(local_irq), pair.0);
                return Ok(());
            }
        }
        Err(())
    }

    pub unsafe fn set_irq_enabled(irq: Irq, new_value: bool) -> Result<(), ()> {
        let props = APIC.gsi_table[irq.0 as usize];
        let gsi = props.gsi;

        for ioapic in APIC.ioapics.iter() {
            let local_irq = gsi.0 - ioapic.global_int.0;
            if ioapic.global_int <= gsi && local_irq < ioapic.entries {
                let mut value = ioapic.read(IoApicIndex::redir_table_low(local_irq * 2));
                if new_value {
                    value &= !APIC_REDIR_MASK;
                } else {
                    value |= APIC_REDIR_MASK;
                }
                ioapic.write(IoApicIndex::redir_table_low(local_irq * 2), value);
                return Ok(());
            }
        }
        Err(())
    }

    const fn make_redirect_table_entry_pair(
        vec: InterruptVector,
        polarity: PackedPolarity,
        apic_id: ApicId,
    ) -> (u32, u32) {
        (vec.0 as u32 | polarity.as_redir(), apic_id.0 << 24)
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ApicId(pub u32);

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct Irq(pub u8);

impl Irq {
    pub const BASE: InterruptVector = InterruptVector(0x20);
    pub const MAX: Irq = Irq(127);

    pub const fn as_vec(&self) -> InterruptVector {
        InterruptVector(Self::BASE.0 + self.0)
    }
}

impl From<Irq> for InterruptVector {
    fn from(irq: Irq) -> InterruptVector {
        irq.as_vec()
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct GsiProps {
    gsi: Irq,
    polarity: PackedPolarity,
}

impl GsiProps {
    const fn null() -> Self {
        GsiProps {
            gsi: Irq(0),
            polarity: PackedPolarity(0),
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
struct PackedPolarity(pub u8);

impl PackedPolarity {
    const fn new(polarity: ApicPolarity, trigger: ApicTriggerMode) -> Self {
        Self(polarity.as_packed() | trigger.as_packed())
    }

    const fn as_redir(&self) -> u32 {
        (self.0 as u32) << 12
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum ApicPolarity {
    ActiveHigh = 0,
    ActiveLow = 1,
}

impl ApicPolarity {
    const fn as_packed(&self) -> u8 {
        (*self as u8) << 1
    }
}

impl From<&acpi::interrupt::Polarity> for ApicPolarity {
    fn from(src: &acpi::interrupt::Polarity) -> Self {
        match *src {
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
    const fn as_packed(&self) -> u8 {
        (*self as u8) << 3
    }
}

impl From<&acpi::interrupt::TriggerMode> for ApicTriggerMode {
    fn from(src: &acpi::interrupt::TriggerMode) -> Self {
        match *src {
            acpi::interrupt::TriggerMode::SameAsBus => ApicTriggerMode::Edge,
            acpi::interrupt::TriggerMode::Edge => ApicTriggerMode::Edge,
            acpi::interrupt::TriggerMode::Level => ApicTriggerMode::Level,
        }
    }
}

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

        // LAPIC timer
        let vec_irtimer = Irq(0).as_vec();
        InterruptDescriptorTable::register(vec_irtimer, LinearAddress(timer_handler as usize));
        Self::clear_timer();
        Self::set_timer_div(LocalApicTimerDivide::By1);
        Self::set_timer(LocalApicTimerMode::Periodic, vec_irtimer, 1000_000);
    }

    #[allow(dead_code)]
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

    unsafe fn set_timer_div(div: LocalApicTimerDivide) {
        Self::TimerDivideConfiguration.write(div as u32);
    }

    unsafe fn set_timer(mode: LocalApicTimerMode, vec: InterruptVector, count: u32) {
        Self::TimerInitialCount.write(count);
        Self::LvtTimer.write((vec.0 as u32) | mode as u32);
    }

    unsafe fn clear_timer() {
        Self::LvtTimer.write(APIC_REDIR_MASK);
    }
}

#[allow(dead_code)]
#[derive(Debug, Copy, Clone, PartialEq)]
enum LocalApicTimerMode {
    OneShot = 0 << 17,
    Periodic = 1 << 17,
    TscDeadline = 2 << 17,
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum LocalApicTimerDivide {
    By1 = 0b1011,
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
struct IoApicIndex(u8);

impl IoApicIndex {
    #[allow(dead_code)]
    const ID: IoApicIndex = IoApicIndex(0x00);
    const VER: IoApicIndex = IoApicIndex(0x01);
    const REDIR_BASE: IoApicIndex = IoApicIndex(0x10);

    const fn redir_table_low(index: u8) -> Self {
        Self(Self::REDIR_BASE.0 + index * 2)
    }

    const fn redir_table_high(index: u8) -> Self {
        Self(Self::REDIR_BASE.0 + index * 2 + 1)
    }
}

#[allow(dead_code)]
struct IoApic {
    base: *mut u8,
    global_int: Irq,
    entries: u8,
    id: u8,
}

impl IoApic {
    unsafe fn new(acpi_ioapic: &acpi::interrupt::IoApic) -> Self {
        let mut ioapic = IoApic {
            base: acpi_ioapic.address as usize as *const u8 as *mut u8,
            global_int: Irq(acpi_ioapic.global_system_interrupt_base as u8),
            entries: 0,
            id: acpi_ioapic.id,
        };
        let ver = ioapic.read(IoApicIndex::VER);
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

//-//-//-// TEST //-//-//-//

static mut TIMER_COUNTER: usize = 0;

extern "x86-interrupt" fn timer_handler(_stack_frame: &ExceptionStackFrame) {
    unsafe {
        TIMER_COUNTER += 0x123456;
        stdout().fb().fill_rect(
            Rect::new(400, 50, 20, 20),
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
