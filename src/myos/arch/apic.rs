// Advanced Programmable Interrupt Controller

use alloc::boxed::Box;
use alloc::vec::*;
use core::ffi::c_void;
use core::ptr::*;

use super::cpu::*;
use super::system::*;
use crate::mux::spinlock::Spinlock;
use crate::myos::io::graphics::*;
use crate::myos::mem::alloc::*;
use crate::myos::scheduler::*;
use crate::myos::thread::*;
use crate::stdout;
use crate::*;

#[allow(dead_code)]
const MSI_BASE: usize = 0xFEE00000;
const APIC_REDIR_MASK: u32 = 0x00010000;
const MAX_CPU: usize = 64;

static mut APIC: Apic = Apic::new();
static mut LOCAL_APIC_BASE: Option<NonNull<c_void>> = None;

extern "C" {
    fn setup_smp_init(
        vec_sipi: u8,
        max_cpu: usize,
        stack_chunk_size: usize,
        stack_base: *mut c_void,
    );
}

static mut GLOBALLOCK: Spinlock = Spinlock::new();

#[no_mangle]
pub unsafe extern "C" fn apic_start_ap(_cpuid: u8) {
    GLOBALLOCK.lock();
    let new_cpu = Cpu::new(LocalApic::init_ap());
    System::shared().activate_cpu(new_cpu);
    // println!("Started AP {}", LocalApic::current_processor_id().0);
    GLOBALLOCK.unlock();
}

pub struct Apic {
    master_apic_id: ProcessorId,
    ioapics: Vec<Box<IoApic>>,
    gsi_table: [GsiProps; 256],
    idt: [VirtualAddress; Irq::MAX.0 as usize],
    lapic_timer_value: u32,
}

impl Apic {
    const fn new() -> Self {
        Apic {
            master_apic_id: ProcessorId(0),
            ioapics: Vec::new(),
            gsi_table: [GsiProps::null(); 256],
            idt: [VirtualAddress::NULL; Irq::MAX.0 as usize],
            lapic_timer_value: 0,
        }
    }

    pub(crate) unsafe fn init(acpi_apic: &acpi::interrupt::Apic) {
        if acpi_apic.also_has_legacy_pics {
            // disable legacy PICs
            Cpu::out8(0xA1, 0xFF);
            Cpu::out8(0x21, 0xFF);
        }

        // disable IRQ
        llvm_asm!("cli");

        // init Local Apic
        APIC.master_apic_id = System::shared().cpu(0).as_ref().cpu_id;
        LocalApic::init(acpi_apic.local_apic_address as usize);

        // Define Default GSI table for ISA devices
        for irq in &[1, 12] {
            APIC.gsi_table[*irq as usize] = GsiProps {
                global_irq: Irq(*irq),
                polarity: PackedPolarity(0),
            };
        }

        // import gsi table from ACPI
        for source in &acpi_apic.interrupt_source_overrides {
            let props = GsiProps {
                global_irq: Irq(source.global_system_interrupt as u8),
                polarity: PackedPolarity::new(
                    ApicPolarity::from(&source.polarity),
                    ApicTriggerMode::from(&source.trigger_mode),
                ),
            };
            APIC.gsi_table[source.isa_source as usize] = props;
        }

        // enable IRQ
        llvm_asm!("sti");

        // Init IO Apics
        for acpi_ioapic in &acpi_apic.io_apics {
            APIC.ioapics.push(Box::new(IoApic::new(acpi_ioapic)));
        }

        InterruptDescriptorTable::register(
            Irq(1).as_vec(),
            VirtualAddress(irq_01_handler as usize),
        );
        InterruptDescriptorTable::register(
            Irq(2).as_vec(),
            VirtualAddress(irq_02_handler as usize),
        );
        InterruptDescriptorTable::register(
            Irq(12).as_vec(),
            VirtualAddress(irq_0c_handler as usize),
        );

        // Local APIC Timer
        let vec_latimer = Irq(0).as_vec();
        InterruptDescriptorTable::register(vec_latimer, VirtualAddress(timer_handler as usize));
        LocalApic::clear_timer();
        LocalApic::set_timer_div(LocalApicTimerDivide::By1);
        if let Some(hpet_info) = &System::shared().acpi().hpet {
            // Use HPET
            let hpet = Hpet::new(hpet_info);
            let magic_number = 100;
            let deadline0 = hpet.create(TimeMeasure(1));
            while hpet.until(deadline0) {
                Cpu::relax();
            }
            let deadline1 = hpet.create(TimeMeasure::from_micros(100_0000 / magic_number));
            LocalApic::TimerInitialCount.write(u32::MAX);
            while hpet.until(deadline1) {
                Cpu::relax();
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
        let max_cpu = core::cmp::min(System::shared().number_of_cpus(), MAX_CPU);
        let stack_chunk_size = 0x4000;
        // let stack_base = CustomAlloc::zalloc(max_cpu * stack_chunk_size)
        //     .unwrap()
        //     .as_ptr();
        let stack_base = null_mut();
        setup_smp_init(1, max_cpu, stack_chunk_size, stack_base);
        LocalApic::broadcast_init();
        Thread::usleep(10_000);
        LocalApic::broadcast_sipi(1);
        Thread::usleep(200_000);
        if System::shared().number_of_cpus() != System::shared().number_of_active_cpus() {
            panic!("Some of the processors are not responding");
        }

        llvm_asm!("
        mov $$0xcccccccc, %eax
        mov $$256, %ecx
        xor %edi, %edi
        rep stosl
        ":::"eax","ecx","edi");
    }

    pub unsafe fn register(irq: Irq, f: IrqHandler) -> Result<(), ()> {
        let props = APIC.gsi_table[irq.0 as usize];
        let global_irq = props.global_irq;
        let polarity = props.polarity;
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
                    polarity,
                    APIC.master_apic_id,
                );
                ioapic.write(IoApicIndex::redir_table_high(local_irq), pair.1);
                ioapic.write(IoApicIndex::redir_table_low(local_irq), pair.0);
                return Ok(());
            }
        }
        Err(())
    }

    pub unsafe fn set_irq_enabled(irq: Irq, new_value: bool) -> Result<(), ()> {
        let props = APIC.gsi_table[irq.0 as usize];
        let global_irq = props.global_irq;

        for ioapic in APIC.ioapics.iter_mut() {
            let local_irq = global_irq.0 - ioapic.global_int.0;
            if ioapic.global_int <= global_irq && local_irq < ioapic.entries {
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
        apic_id: ProcessorId,
    ) -> (u32, u32) {
        (vec.0 as u32 | polarity.as_redir(), apic_id.as_u32() << 24)
    }

    fn eoi() {
        unsafe {
            LocalApic::eoi();
        }
    }

    pub fn current_processor_id() -> ProcessorId {
        unsafe { LocalApic::current_processor_id() }
    }
}

pub type IrqHandler = fn(Irq) -> ();

#[no_mangle]
pub unsafe extern "efiapi" fn apic_handle_irq(irq: Irq) {
    let e = APIC.idt[irq.0 as usize];
    if e != VirtualAddress::NULL {
        let old_irql = Irql::raise(Irql::Device).unwrap();
        let f = core::mem::transmute::<usize, IrqHandler>(e.0);
        f(irq);
        Apic::eoi();
        Irql::lower(old_irql).unwrap();
    } else {
        panic!("Why IRQ {} is Enabled, But not installed", irq.0);
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
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
    pub const LPC_PS2M: Irq = Irq(12);
    pub const LPC_RTC: Irq = Irq(8);
    pub const LPC_IDE1: Irq = Irq(14);
    pub const LPC_IDE2: Irq = Irq(15);

    pub const fn as_vec(&self) -> InterruptVector {
        InterruptVector(Self::BASE.0 + self.0)
    }

    pub unsafe fn register(&self, f: IrqHandler) -> Result<(), ()> {
        Apic::register(*self, f)
    }
}

impl From<Irq> for InterruptVector {
    fn from(irq: Irq) -> InterruptVector {
        irq.as_vec()
    }
}

#[derive(Debug, Copy, Clone)]
struct GsiProps {
    global_irq: Irq,
    polarity: PackedPolarity,
}

impl GsiProps {
    const fn null() -> Self {
        GsiProps {
            global_irq: Irq(0),
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
        let ptr = base as *const c_void as *mut c_void;
        LOCAL_APIC_BASE = NonNull::new(ptr);

        let msr = Msr::ApicBase;
        let val = msr.read();
        msr.write(
            (val & Self::IA32_APIC_BASE_MSR_BSP)
                | ((base as u64 & !0xFFF) | Self::IA32_APIC_BASE_MSR_ENABLE),
        );
    }

    unsafe fn init_ap() -> ProcessorId {
        Msr::ApicBase
            .write(LOCAL_APIC_BASE.unwrap().as_ptr() as u64 | Self::IA32_APIC_BASE_MSR_ENABLE);

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

    /// Broadcast INIT IPI to all another APs
    unsafe fn broadcast_init() {
        LocalApic::InterruptCommand.write(0x000C4500);
    }

    /// Broadcast Startup IPI to all another APs
    unsafe fn broadcast_sipi(init_vec: u8) {
        LocalApic::InterruptCommand.write(0x000C4600 | init_vec as u32);
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
    base: *mut u8,
    global_int: Irq,
    entries: u8,
    id: u8,
    lock: Spinlock,
}

impl IoApic {
    unsafe fn new(acpi_ioapic: &acpi::interrupt::IoApic) -> Self {
        let mut ioapic = IoApic {
            base: acpi_ioapic.address as usize as *const u8 as *mut u8,
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
        let ptr_index = self.base;
        let ptr_data = self.base.add(0x0010) as *const u32;
        self.lock.lock();
        ptr_index.write_volatile(index.0);
        let value = ptr_data.read_volatile();
        self.lock.unlock();
        value
    }

    unsafe fn write(&mut self, index: IoApicIndex, data: u32) {
        let ptr_index = self.base;
        let ptr_data = self.base.add(0x0010) as *const u32 as *mut u32;
        self.lock.lock();
        ptr_index.write_volatile(index.0);
        ptr_data.write_volatile(data);
        self.lock.unlock();
    }
}

/// High Precision Event Timer
struct Hpet {
    base: *mut u64,
    main_cnt_period: u64,
    measure_div: u64,
}

impl Hpet {
    unsafe fn new(info: &acpi::HpetInfo) -> Box<Self> {
        let mut hpet = Hpet {
            base: info.base_address as *const u64 as *mut u64,
            main_cnt_period: 0,
            measure_div: 0,
        };

        Irq::LPC_TIMER.register(Self::irq_handler).unwrap();
        hpet.main_cnt_period = hpet.read(0) >> 32;
        hpet.write(0x10, 0);
        hpet.write(0x20, 0); // Clear all interrupts
        hpet.write(0xF0, 0); // Reset MAIN_COUNTER_VALUE
        hpet.write(0x10, 0x03); // LEG_RT_CNF | ENABLE_CNF

        hpet.measure_div = 1000_000_000 / hpet.main_cnt_period;
        hpet.write(0x100, 0x0000_004C); // Tn_INT_ENB_CNF | Tn_TYPE_CNF | Tn_VAL_SET_CNF
        hpet.write(0x108, 1000_000_000_000 / hpet.main_cnt_period);

        Box::new(hpet)
    }

    unsafe fn read(&self, index: usize) -> u64 {
        let ptr = self.base.add(index >> 3);
        ptr.read_volatile()
    }

    unsafe fn write(&self, index: usize, value: u64) {
        let ptr = self.base.add(index >> 3);
        ptr.write_volatile(value);
    }

    fn measure(&self) -> TimeMeasure {
        unsafe { TimeMeasure((self.read(0xF0) / self.measure_div) as i64) }
    }

    fn irq_handler(_irq: Irq) {
        // TODO:
        unsafe {
            TIMER_COUNTER += 0x010203;
            stdout()
                .fb()
                .fill_rect(Rect::new(760, 5, 10, 10), Color::from(TIMER_COUNTER as u32));
        }
    }
}

impl TimerSource for Hpet {
    fn create(&self, duration: TimeMeasure) -> TimeMeasure {
        self.measure() + duration.0 as isize
    }

    fn until(&self, deadline: TimeMeasure) -> bool {
        (deadline.0 - self.measure().0) > 0
    }

    fn diff(&self, from: TimeMeasure) -> isize {
        self.measure().0 as isize - from.0 as isize
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

static mut TIMER_COUNTER: usize = 0;

extern "x86-interrupt" fn timer_handler() {
    unsafe {
        TIMER_COUNTER += 0x040506;
        stdout()
            .fb()
            .fill_rect(Rect::new(780, 5, 10, 10), Color::from(TIMER_COUNTER as u32));
        LocalApic::eoi();
        GlobalScheduler::reschedule();
    }
}
