// Advanced Programmable Interrupt Controller

use super::cpu::*;
use super::hpet::*;
use crate::mem::memory::*;
use crate::mem::mmio::*;
use crate::sync::spinlock::Spinlock;
use crate::system::*;
use crate::task::scheduler::*;
use crate::*;
use alloc::boxed::Box;
use alloc::vec::*;
use core::ffi::c_void;
use core::time::Duration;

const MAX_CPU: usize = 64;

static mut APIC: Apic = Apic::new();
const INVALID_PROCESSOR_INDEX: u8 = 0xFF;
static mut CURRENT_PROCESSOR_INDEXES: [u8; 256] = [INVALID_PROCESSOR_INDEX; 256];

extern "C" {
    fn asm_apic_setup_sipi(
        vec_sipi: InterruptVector,
        max_cpu: usize,
        stack_chunk_size: usize,
        stack_base: *mut c_void,
    );
}

static mut GLOBALLOCK: Spinlock = Spinlock::new();

#[no_mangle]
pub unsafe extern "C" fn apic_start_ap(_cpuid: u8) {
    GLOBALLOCK.synchronized(|| {
        let new_cpu = Cpu::new(LocalApic::init_ap());
        let new_cpuid = new_cpu.cpu_id;
        let index = System::activate_cpu(new_cpu);
        CURRENT_PROCESSOR_INDEXES[new_cpuid.0 as usize] = index.0 as u8;
    });
}

pub(super) struct Apic {
    master_apic_id: ProcessorId,
    ioapics: Vec<Box<IoApic>>,
    gsi_table: [GsiProps; 256],
    idt: [VirtualAddress; Irq::MAX.0 as usize],
    lapic_timer_value: u32,
}

impl Apic {
    const REDIR_MASK: u32 = 0x00010000;
    #[allow(dead_code)]
    const MSI_BASE: usize = 0xFEE00000;

    const fn new() -> Self {
        Apic {
            master_apic_id: ProcessorId(0),
            ioapics: Vec::new(),
            gsi_table: [GsiProps::zero(); 256],
            idt: [VirtualAddress::NULL; Irq::MAX.0 as usize],
            lapic_timer_value: 0,
        }
    }

    pub unsafe fn init(acpi_apic: &acpi::interrupt::Apic) {
        if acpi_apic.also_has_legacy_pics {
            // disable legacy PICs
            Cpu::out8(0xA1, 0xFF);
            Cpu::out8(0x21, 0xFF);
        }

        Cpu::disable_interrupt();

        // init Local Apic
        APIC.master_apic_id = System::cpu(0).as_ref().cpu_id;
        CURRENT_PROCESSOR_INDEXES[APIC.master_apic_id.0 as usize] = 0;
        LocalApic::init(acpi_apic.local_apic_address as usize);

        // Define Default GSI table for ISA devices
        for irq in &[1, 12] {
            APIC.gsi_table[*irq as usize] = GsiProps {
                global_irq: Irq(*irq),
                trigger: PackedTriggerMode(0),
            };
        }

        // import gsi table from ACPI
        for source in &acpi_apic.interrupt_source_overrides {
            let props = GsiProps {
                global_irq: Irq(source.global_system_interrupt as u8),
                trigger: PackedTriggerMode::new(
                    ApicTriggerMode::from(&source.trigger_mode),
                    ApicPolarity::from(&source.polarity),
                ),
            };
            APIC.gsi_table[source.isa_source as usize] = props;
        }

        Cpu::enable_interrupt();

        // Init IO Apics
        for acpi_ioapic in &acpi_apic.io_apics {
            APIC.ioapics.push(Box::new(IoApic::new(acpi_ioapic)));
        }

        InterruptDescriptorTable::register(Irq(1).into(), VirtualAddress(irq_01_handler as usize));
        InterruptDescriptorTable::register(Irq(2).into(), VirtualAddress(irq_02_handler as usize));
        InterruptDescriptorTable::register(Irq(12).into(), VirtualAddress(irq_0c_handler as usize));

        // Local APIC Timer
        let vec_latimer = Irq(0).as_vec();
        InterruptDescriptorTable::register(vec_latimer, VirtualAddress(timer_handler as usize));
        LocalApic::clear_timer();
        LocalApic::set_timer_div(LocalApicTimerDivide::By1);
        if let Some(hpet_info) = &System::acpi().hpet {
            // Use HPET
            let hpet = Hpet::new(hpet_info);
            let magic_number = 100;
            let deadline0 = hpet.create(Duration::from_micros(1));
            while hpet.until(deadline0) {
                Cpu::spin_loop_hint();
            }
            let deadline1 = hpet.create(Duration::from_micros(100_0000 / magic_number));
            LocalApic::TimerInitialCount.write(u32::MAX);
            while hpet.until(deadline1) {
                Cpu::spin_loop_hint();
            }
            let count = LocalApic::TimerCurrentCount.read() as u64;
            APIC.lapic_timer_value = ((u32::MAX as u64 - count) * magic_number / 1000) as u32;
            Timer::set_timer(hpet);
        } else {
            panic!("No Reference Timer found");
        }
        LocalApic::set_timer(
            LocalApicTimerMode::Periodic,
            vec_latimer,
            APIC.lapic_timer_value,
        );

        // Setup SMP
        let sipi_vec = InterruptVector(MemoryManager::static_alloc_real().unwrap().get());
        let max_cpu = core::cmp::min(System::num_of_cpus(), MAX_CPU);
        let stack_chunk_size = 0x4000;
        let stack_base = MemoryManager::zalloc(max_cpu * stack_chunk_size)
            .unwrap()
            .get() as *mut c_void;
        asm_apic_setup_sipi(sipi_vec, max_cpu, stack_chunk_size, stack_base);
        LocalApic::broadcast_init();
        Timer::usleep(10_000);
        LocalApic::broadcast_startup(sipi_vec);
        Timer::usleep(10_000);
        LocalApic::broadcast_startup(sipi_vec);
        Timer::usleep(10_000);
        if System::num_of_active_cpus() != max_cpu {
            panic!("Some of the processors are not responding");
        }

        asm!("
        mov eax, 0xCCCCCCCC
        mov ecx, 256
        xor edi, edi
        rep stosd
        ",
            lateout("eax") _,
            lateout("ecx") _,
            lateout("edi") _,
        );
    }

    pub unsafe fn register(irq: Irq, f: IrqHandler) -> Result<(), ()> {
        let props = APIC.gsi_table[irq.0 as usize];
        let global_irq = props.global_irq;
        let trigger = props.trigger;
        if global_irq.0 == 0 {
            return Err(());
        }

        for ioapic in APIC.ioapics.iter_mut() {
            let local_irq = global_irq.0 - ioapic.global_int.0;
            if ioapic.global_int <= global_irq && local_irq < ioapic.entries {
                if APIC.idt[global_irq.0 as usize] != VirtualAddress::NULL {
                    return Err(());
                }
                APIC.idt[global_irq.0 as usize] = VirtualAddress(f as usize);
                let pair = Self::make_redirect_table_entry_pair(
                    global_irq.as_vec(),
                    trigger,
                    APIC.master_apic_id,
                );
                ioapic.write(IoApicIndex::redir_table_high(local_irq), pair.1);
                ioapic.write(IoApicIndex::redir_table_low(local_irq), pair.0);
                return Ok(());
            }
        }
        Err(())
    }

    pub unsafe fn set_irq_enabled(irq: Irq, enabled: bool) -> Result<(), ()> {
        let props = APIC.gsi_table[irq.0 as usize];
        let global_irq = props.global_irq;

        for ioapic in APIC.ioapics.iter_mut() {
            let local_irq = global_irq.0 - ioapic.global_int.0;
            if ioapic.global_int <= global_irq && local_irq < ioapic.entries {
                let mut value = ioapic.read(IoApicIndex::redir_table_low(local_irq * 2));
                if enabled {
                    value &= !Apic::REDIR_MASK;
                } else {
                    value |= Apic::REDIR_MASK;
                }
                ioapic.write(IoApicIndex::redir_table_low(local_irq * 2), value);
                return Ok(());
            }
        }
        Err(())
    }

    const fn make_redirect_table_entry_pair(
        vec: InterruptVector,
        trigger: PackedTriggerMode,
        apic_id: ProcessorId,
    ) -> (u32, u32) {
        (vec.0 as u32 | trigger.as_redir(), apic_id.as_u32() << 24)
    }

    pub fn current_processor_id() -> ProcessorId {
        unsafe { LocalApic::current_processor_id() }
    }

    pub fn current_processor_index() -> Option<ProcessorIndex> {
        let index = unsafe { CURRENT_PROCESSOR_INDEXES[Self::current_processor_id().0 as usize] };
        if index != INVALID_PROCESSOR_INDEX {
            Some(ProcessorIndex(index as usize))
        } else {
            None
        }
    }
}

pub type IrqHandler = fn(Irq) -> ();

#[no_mangle]
pub unsafe extern "C" fn apic_handle_irq(irq: Irq) {
    match APIC.idt[irq.0 as usize].into_nonzero() {
        Some(entry) => {
            let f: IrqHandler = core::mem::transmute(entry);
            f(irq);
            LocalApic::eoi();
        }
        None => {
            let _ = irq.disable();
            panic!("IRQ {} is Enabled, But not Installed", irq.0);
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Default)]
pub struct Irq(pub u8);

impl Irq {
    const BASE: InterruptVector = InterruptVector(0x20);
    const MAX: Irq = Irq(127);

    pub const LPC_TIMER: Irq = Irq(0);
    pub const LPC_PS2K: Irq = Irq(1);
    pub const LPC_COM2: Irq = Irq(3);
    pub const LPC_COM1: Irq = Irq(4);
    pub const LPC_FDC: Irq = Irq(6);
    pub const LPC_LPT: Irq = Irq(7);
    pub const LPC_RTC: Irq = Irq(8);
    pub const LPC_PS2M: Irq = Irq(12);
    pub const LPC_IDE1: Irq = Irq(14);
    pub const LPC_IDE2: Irq = Irq(15);

    pub const fn as_vec(self) -> InterruptVector {
        InterruptVector(Self::BASE.0 + self.0)
    }

    pub unsafe fn register(self, f: IrqHandler) -> Result<(), ()> {
        Apic::register(self, f)
    }

    pub unsafe fn enable(self) -> Result<(), ()> {
        Apic::set_irq_enabled(self, true)
    }

    pub unsafe fn disable(self) -> Result<(), ()> {
        Apic::set_irq_enabled(self, false)
    }
}

impl From<Irq> for InterruptVector {
    fn from(irq: Irq) -> InterruptVector {
        irq.as_vec()
    }
}

#[derive(Debug, Copy, Clone, Default)]
struct GsiProps {
    global_irq: Irq,
    trigger: PackedTriggerMode,
}

impl GsiProps {
    const fn zero() -> Self {
        GsiProps {
            global_irq: Irq(0),
            trigger: PackedTriggerMode(0),
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Default)]
struct PackedTriggerMode(pub u8);

impl PackedTriggerMode {
    const fn new(trigger: ApicTriggerMode, polarity: ApicPolarity) -> Self {
        Self(trigger.as_packed() | polarity.as_packed())
    }

    const fn as_redir(self) -> u32 {
        (self.0 as u32) << 12
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum ApicPolarity {
    ActiveHigh = 0,
    ActiveLow = 1,
}

impl ApicPolarity {
    const fn as_packed(self) -> u8 {
        (self as u8) << 1
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
    const fn as_packed(self) -> u8 {
        (self as u8) << 3
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

static mut LOCAL_APIC: Option<Mmio> = None;

#[allow(dead_code)]
#[non_exhaustive]
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
        LOCAL_APIC = Mmio::from_phys(base, 0x1000).ok();

        let msr = Msr::ApicBase;
        let val = msr.read();
        msr.write(
            (val & Self::IA32_APIC_BASE_MSR_BSP)
                | ((base as u64 & !0xFFF) | Self::IA32_APIC_BASE_MSR_ENABLE),
        );
    }

    unsafe fn init_ap() -> ProcessorId {
        Msr::ApicBase
            .write(LOCAL_APIC.as_ref().unwrap().base() as u64 | Self::IA32_APIC_BASE_MSR_ENABLE);

        let myid = LocalApic::current_processor_id();

        LocalApic::SpuriousInterrupt.write(0x010F);

        let vec_latimer = Irq(0).as_vec();
        LocalApic::clear_timer();
        LocalApic::set_timer_div(LocalApicTimerDivide::By1);
        LocalApic::set_timer(
            LocalApicTimerMode::Periodic,
            vec_latimer,
            APIC.lapic_timer_value,
        );

        myid
    }

    unsafe fn read(&self) -> u32 {
        LOCAL_APIC.as_ref().unwrap().read_u32(*self as usize)
    }

    unsafe fn write(&self, val: u32) {
        LOCAL_APIC.as_ref().unwrap().write_u32(*self as usize, val);
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
        Self::LvtTimer.write(Apic::REDIR_MASK);
    }

    /// Broadcast INIT IPI to all another APs
    unsafe fn broadcast_init() {
        Self::InterruptCommandHigh.write(0);
        Self::InterruptCommand.write(0x000C4500);
    }

    /// Broadcast Startup IPI to all another APs
    unsafe fn broadcast_startup(init_vec: InterruptVector) {
        Self::InterruptCommandHigh.write(0);
        Self::InterruptCommand.write(0x000C4600 | init_vec.0 as u32);
    }

    unsafe fn current_processor_id() -> ProcessorId {
        ProcessorId::from(LocalApic::Id.read() >> 24)
    }
}

#[allow(dead_code)]
#[derive(Debug, Copy, Clone, PartialEq)]
enum LocalApicTimerMode {
    OneShot = 0 << 17,
    Periodic = 1 << 17,
    TscDeadline = 2 << 17,
}

#[non_exhaustive]
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
    mmio: Mmio,
    global_int: Irq,
    entries: u8,
    id: u8,
    lock: Spinlock,
}

impl IoApic {
    unsafe fn new(acpi_ioapic: &acpi::interrupt::IoApic) -> Self {
        let mut ioapic = IoApic {
            mmio: Mmio::from_phys(acpi_ioapic.address as usize, 0x14).unwrap(),
            global_int: Irq(acpi_ioapic.global_system_interrupt_base as u8),
            entries: 0,
            id: acpi_ioapic.id,
            lock: Spinlock::new(),
        };
        let ver = ioapic.read(IoApicIndex::VER);
        ioapic.entries = 1 + (ver >> 16) as u8;
        ioapic
    }

    unsafe fn read(&mut self, index: IoApicIndex) -> u32 {
        Cpu::without_interrupts(|| {
            self.lock.synchronized(|| {
                self.mmio.write_u8(0x00, index.0);
                self.mmio.read_u32(0x10)
            })
        })
    }

    unsafe fn write(&mut self, index: IoApicIndex, data: u32) {
        Cpu::without_interrupts(|| {
            self.lock.synchronized(|| {
                self.mmio.write_u8(0x00, index.0);
                self.mmio.write_u32(0x10, data);
            });
        });
    }
}

//-//-//-// TEST //-//-//-//

extern "x86-interrupt" fn irq_01_handler() {
    unsafe {
        apic_handle_irq(Irq(1));
    }
}

extern "x86-interrupt" fn irq_02_handler() {
    unsafe {
        apic_handle_irq(Irq(2));
    }
}

extern "x86-interrupt" fn irq_0c_handler() {
    unsafe {
        apic_handle_irq(Irq(12));
    }
}

extern "x86-interrupt" fn timer_handler() {
    unsafe {
        LocalApic::eoi();
        MyScheduler::reschedule();
    }
}
