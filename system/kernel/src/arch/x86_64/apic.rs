// Advanced Programmable Interrupt Controller

use super::{cpu::*, hpet::*, page::PageManager, page::PhysicalAddress};
use crate::{
    mem::mmio::*,
    mem::*,
    sync::spinlock::{SpinLoopWait, Spinlock},
    system::*,
    task::scheduler::*,
};
use ::alloc::{boxed::Box, vec::*};
use acpi::platform::ProcessorState;
use bootprot::BootFlags;
use core::{alloc::Layout, ffi::c_void, mem::transmute, sync::atomic::*, time::Duration};
use seq_macro::seq;

/// Maximum number of supported cpu cores
const MAX_CPU: usize = 64;

const STACK_CHUNK_SIZE: usize = 0x4000;

/// Maximum number of supported IOAPIC's IRQ
#[allow(dead_code)]
const MAX_IOAPIC_IRQS: usize = 48;

/// Maximum number of supported MSI IRQ
const MAX_MSI: isize = 16;

#[allow(dead_code)]
const MAX_IRQ: usize = MAX_IOAPIC_IRQS + MAX_MSI as usize;

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

static AP_STALLED: AtomicBool = AtomicBool::new(true);
static GLOBALLOCK: Spinlock = Spinlock::new();

#[no_mangle]
pub unsafe extern "C" fn apic_start_ap() {
    let apic_id = GLOBALLOCK.synchronized(|| {
        let apic_id = LocalApic::init_ap();
        System::activate_cpu(Cpu::new(apic_id));
        apic_id
    });

    // Waiting for TSC synchonization
    while AP_STALLED.load(Ordering::Relaxed) {
        Cpu::spin_loop_hint();
    }
    let tsc = Cpu::rdtsc();

    for index in 0..System::current_device().num_of_active_cpus() {
        let cpu = System::cpu(index);
        if cpu.cpu_id() == apic_id {
            System::cpu_mut(index).set_tsc_base(tsc);
            Msr::TscAux.write(index as u64);
            break;
        }
    }
}

pub(super) struct Apic {
    master_apic_id: ProcessorId,
    ioapics: Vec<Box<IoApic>>,
    gsi_table: [GsiProps; 256],
    idt: [usize; Irq::MAX.0 as usize],
    lapic_timer_value: u32,
    tlb_flush_bitmap: AtomicUsize,
}

impl Apic {
    const REDIR_MASK: u32 = 0x00010000;
    const MSI_DATA: u16 = 0xC000;
    const MSI_BASE: u64 = 0xFEE00000;

    const fn new() -> Self {
        Apic {
            master_apic_id: ProcessorId(0),
            ioapics: Vec::new(),
            gsi_table: [GsiProps::default(); 256],
            idt: [0; Irq::MAX.0 as usize],
            lapic_timer_value: 0,
            tlb_flush_bitmap: AtomicUsize::new(0),
        }
    }

    pub unsafe fn init(acpi_apic: &acpi::platform::Apic) {
        if acpi_apic.also_has_legacy_pics {
            // disable legacy PICs
            Cpu::out8(0xA1, 0xFF);
            Cpu::out8(0x21, 0xFF);
        }

        Cpu::disable_interrupt();

        // init Local Apic
        APIC.master_apic_id = System::acpi_platform()
            .processor_info
            .unwrap()
            .boot_processor
            .local_apic_id
            .into();
        CURRENT_PROCESSOR_INDEXES[APIC.master_apic_id.0 as usize] = 0;
        LocalApic::init(acpi_apic.local_apic_address);

        Msr::TscAux.write(0);

        // Define Default GSI table for ISA devices
        for irq in &[1, 12] {
            APIC.gsi_table[*irq as usize] = GsiProps {
                global_irq: Irq(*irq),
                trigger: PackedTriggerMode(0),
            };
        }

        // import GSI table from ACPI
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

        // Init IO Apics
        for acpi_ioapic in &acpi_apic.io_apics {
            APIC.ioapics.push(Box::new(IoApic::new(acpi_ioapic)));
        }

        seq!(N in 1..64 {
            InterruptDescriptorTable::register(
                Irq(N).into(),
                handle_irq_#N as usize,
                super::cpu::PrivilegeLevel::Kernel,
            );
        });

        // then enable irq
        Cpu::enable_interrupt();

        // Local APIC Timer
        let vec_latimer = Irq(0).as_vec();
        LocalApic::clear_timer();
        LocalApic::set_timer_div(LocalApicTimerDivide::By1);
        if let Ok(hpet_info) = acpi::HpetInfo::new(System::acpi()) {
            // Use HPET
            Timer::set_timer(Hpet::new(&hpet_info));

            let magic_number = 100;
            Timer::epsilon().repeat_until(|| Cpu::spin_loop_hint());
            let timer = Timer::new(Duration::from_micros(100_0000 / magic_number));
            LocalApic::TimerInitialCount.write(u32::MAX);
            timer.repeat_until(|| Cpu::spin_loop_hint());
            let count = LocalApic::TimerCurrentCount.read() as u64;
            APIC.lapic_timer_value = ((u32::MAX as u64 - count) * magic_number / 1000) as u32;
        } else {
            panic!("No Reference Timer found");
        }
        InterruptDescriptorTable::register(
            vec_latimer,
            timer_handler as usize,
            PrivilegeLevel::Kernel,
        );
        LocalApic::set_timer(
            LocalApicTimerMode::Periodic,
            vec_latimer,
            APIC.lapic_timer_value,
        );

        InterruptDescriptorTable::register(
            InterruptVector::IPI_INVALIDATE_TLB,
            ipi_tlb_flush_handler as usize,
            PrivilegeLevel::Kernel,
        );

        InterruptDescriptorTable::register(
            InterruptVector::IPI_SCHEDULE,
            ipi_schedule_handler as usize,
            PrivilegeLevel::Kernel,
        );

        // preparing SMP
        if !System::boot_flags().contains(BootFlags::FORCE_SINGLE) {
            let sipi_vec = InterruptVector(MemoryManager::static_alloc_real().unwrap().get());
            let pi = System::acpi_platform().processor_info.unwrap();
            let max_cpu = core::cmp::min(
                1 + pi
                    .application_processors
                    .iter()
                    .filter(|v| v.state != ProcessorState::Disabled)
                    .count(),
                MAX_CPU,
            );
            let stack_chunk_size = STACK_CHUNK_SIZE;
            let stack_base = MemoryManager::zalloc(Layout::from_size_align_unchecked(
                max_cpu * stack_chunk_size,
                1,
            ))
            .unwrap()
            .get() as *mut c_void;
            asm_apic_setup_sipi(sipi_vec, max_cpu, stack_chunk_size, stack_base);

            // start SMP
            LocalApic::broadcast_init();
            Timer::new(Duration::from_millis(10)).repeat_until(|| Cpu::halt());
            LocalApic::broadcast_startup(sipi_vec);
            let deadline = Timer::new(Duration::from_millis(200));
            while deadline.until() {
                Timer::new(Duration::from_millis(5)).repeat_until(|| Cpu::halt());
                if System::current_device().num_of_active_cpus() == max_cpu {
                    break;
                }
            }
            if System::current_device().num_of_active_cpus() != max_cpu {
                panic!("SMP: Some of application processors are not responding");
            }

            // Since each processor that receives an IPI starts initializing asynchronously,
            // the physical processor ID and the logical ID assigned by the OS will not match.
            // Therefore, sorting is required here.
            System::sort_cpus(|a, b| a.cpu_id().0.cmp(&b.cpu_id().0));

            for index in 0..System::current_device().num_of_active_cpus() {
                let cpu = System::cpu(index);
                CURRENT_PROCESSOR_INDEXES[cpu.cpu_id().0 as usize] = cpu.cpu_index.0 as u8;
            }

            AP_STALLED.store(false, Ordering::SeqCst);
            System::cpu_mut(0).set_tsc_base(Cpu::rdtsc());
        }

        // asm!("
        //     mov eax, 0xCCCCCCCC
        //     mov ecx, 256
        //     xor edi, edi
        //     rep stosd
        //     ",
        //     lateout("eax") _, lateout("ecx") _, lateout("edi") _,);
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
                if APIC.idt[global_irq.0 as usize] != 0 {
                    return Err(());
                }
                APIC.idt[global_irq.0 as usize] = f as usize;
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

    #[inline]
    pub unsafe fn register_msi(f: fn() -> ()) -> Result<(u64, u16), ()> {
        static NEXT_MSI: AtomicIsize = AtomicIsize::new(0);
        NEXT_MSI
            .fetch_update(Ordering::SeqCst, Ordering::Relaxed, |v| {
                if v < MAX_MSI {
                    Some(v + 1)
                } else {
                    None
                }
            })
            .map(|v| {
                let msi = Msi(v);
                let global_irq = msi.as_irq();
                APIC.idt[global_irq.0 as usize] = f as usize;
                let vec = msi.as_vec();
                let addr = Self::MSI_BASE;
                let data = Self::MSI_DATA | vec.0 as u16;
                (addr, data)
            })
            .map_err(|_| ())
    }

    #[inline]
    #[must_use]
    pub unsafe fn broadcast_invalidate_tlb() -> bool {
        Irql::IPI.raise(|| {
            let max_cpu = System::current_device().num_of_active_cpus();
            if max_cpu < 2 {
                return true;
            }
            APIC.tlb_flush_bitmap.store(
                ((1usize << max_cpu) - 1) & !(1usize << Cpu::current_processor_index().0),
                Ordering::SeqCst,
            );

            LocalApic::broadcast_ipi(InterruptVector::IPI_INVALIDATE_TLB);

            let mut hint = SpinLoopWait::new();
            let deadline = Timer::new(Duration::from_millis(200));
            while deadline.until() {
                if APIC.tlb_flush_bitmap.load(Ordering::Relaxed) == 0 {
                    break;
                }
                hint.wait();
            }

            APIC.tlb_flush_bitmap.load(Ordering::Relaxed) == 0
        })
    }

    #[inline]
    pub unsafe fn broadcast_schedule() -> bool {
        without_interrupts!({
            LocalApic::broadcast_ipi(InterruptVector::IPI_SCHEDULE);

            true
        })
    }

    #[inline]
    unsafe fn handle_irq(irq: Irq) {
        match APIC.idt[irq.0 as usize] {
            0 => {
                let _ = irq.disable();
                panic!("IRQ {} is Enabled, But not Installed", irq.0);
            }
            entry => {
                let f: IrqHandler = transmute(entry);
                Irql::DIrql.raise(|| f(irq));
                LocalApic::eoi();
            }
        }
    }
}

pub type IrqHandler = fn(Irq) -> ();

seq!(N in 1..64 {
    unsafe extern "x86-interrupt" fn handle_irq_#N () {
        Apic::handle_irq(Irq(N));
    }
});

unsafe extern "x86-interrupt" fn timer_handler() {
    LocalApic::eoi();
    Scheduler::reschedule();
}

unsafe extern "x86-interrupt" fn ipi_schedule_handler() {
    LocalApic::eoi();
    Scheduler::reschedule();
}

unsafe extern "x86-interrupt" fn ipi_tlb_flush_handler() {
    PageManager::invalidate_all_pages();
    Cpu::interlocked_test_and_clear(&APIC.tlb_flush_bitmap, Cpu::current_processor_index().0);
    LocalApic::eoi();
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub(super) struct ProcessorId(pub u8);

impl ProcessorId {
    pub const fn as_u32(self) -> u32 {
        self.0 as u32
    }
}

impl From<u8> for ProcessorId {
    fn from(val: u8) -> Self {
        Self(val)
    }
}

impl From<u32> for ProcessorId {
    fn from(val: u32) -> Self {
        Self(val as u8)
    }
}

impl From<usize> for ProcessorId {
    fn from(val: usize) -> Self {
        Self(val as u8)
    }
}

/// Interrupt Request
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Default)]
pub struct Irq(pub u8);

impl Irq {
    const BASE: InterruptVector = InterruptVector(0x80);
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

/// Message Signaled Interrupts
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Default)]
pub struct Msi(pub isize);

impl Msi {
    #[inline]
    const fn as_irq(self) -> Irq {
        Irq((MAX_MSI as isize + self.0) as u8)
    }

    #[inline]
    pub const fn as_vec(self) -> InterruptVector {
        self.as_irq().as_vec()
    }
}

impl From<Msi> for InterruptVector {
    fn from(msi: Msi) -> Self {
        msi.as_vec()
    }
}

#[derive(Debug, Copy, Clone, Default)]
struct GsiProps {
    global_irq: Irq,
    trigger: PackedTriggerMode,
}

impl GsiProps {
    const fn default() -> Self {
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

impl From<&acpi::platform::Polarity> for ApicPolarity {
    fn from(src: &acpi::platform::Polarity) -> Self {
        match *src {
            acpi::platform::Polarity::SameAsBus => ApicPolarity::ActiveHigh,
            acpi::platform::Polarity::ActiveHigh => ApicPolarity::ActiveHigh,
            acpi::platform::Polarity::ActiveLow => ApicPolarity::ActiveLow,
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

impl From<&acpi::platform::TriggerMode> for ApicTriggerMode {
    fn from(src: &acpi::platform::TriggerMode) -> Self {
        match *src {
            acpi::platform::TriggerMode::SameAsBus => ApicTriggerMode::Edge,
            acpi::platform::TriggerMode::Edge => ApicTriggerMode::Edge,
            acpi::platform::TriggerMode::Level => ApicTriggerMode::Level,
        }
    }
}

static mut LOCAL_APIC_PA: PhysicalAddress = 0;
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

    #[inline]
    unsafe fn init(base: PhysicalAddress) {
        LOCAL_APIC_PA = base;
        LOCAL_APIC = Mmio::from_phys(base, 0x1000);

        Msr::ApicBase
            .write(LOCAL_APIC_PA | Self::IA32_APIC_BASE_MSR_ENABLE | Self::IA32_APIC_BASE_MSR_BSP);
    }

    unsafe fn init_ap() -> ProcessorId {
        Msr::ApicBase.write(LOCAL_APIC_PA | Self::IA32_APIC_BASE_MSR_ENABLE);

        let apicid = LocalApic::current_processor_id();

        LocalApic::SpuriousInterrupt.write(0x010F);

        let vec_latimer = Irq(0).as_vec();
        LocalApic::clear_timer();
        LocalApic::set_timer_div(LocalApicTimerDivide::By1);
        LocalApic::set_timer(
            LocalApicTimerMode::Periodic,
            vec_latimer,
            APIC.lapic_timer_value,
        );

        apicid
    }

    #[inline]
    #[track_caller]
    unsafe fn read(&self) -> u32 {
        LOCAL_APIC.as_ref().unwrap().read_u32(*self as usize)
    }

    #[inline]
    #[track_caller]
    unsafe fn write(&self, val: u32) {
        LOCAL_APIC.as_ref().unwrap().write_u32(*self as usize, val);
    }

    #[inline]
    unsafe fn eoi() {
        Self::Eoi.write(0);
    }

    #[inline]
    unsafe fn set_timer_div(div: LocalApicTimerDivide) {
        Self::TimerDivideConfiguration.write(div as u32);
    }

    #[inline]
    unsafe fn set_timer(mode: LocalApicTimerMode, vec: InterruptVector, count: u32) {
        Self::TimerInitialCount.write(count);
        Self::LvtTimer.write((vec.0 as u32) | mode as u32);
    }

    #[inline]
    unsafe fn clear_timer() {
        Self::LvtTimer.write(Apic::REDIR_MASK);
    }

    /// Broadcasts INIT IPI to all another APs
    #[inline]
    unsafe fn broadcast_init() {
        Self::InterruptCommandHigh.write(0);
        Self::InterruptCommand.write(0x000C4500);
    }

    /// Broadcasts Startup IPI to all another APs
    #[inline]
    unsafe fn broadcast_startup(init_vec: InterruptVector) {
        Self::InterruptCommandHigh.write(0);
        Self::InterruptCommand.write(0x000C4600 | init_vec.0 as u32);
    }

    /// Broadcasts an inter-processor interrupt to all excluding self.
    #[inline]
    unsafe fn broadcast_ipi(vec: InterruptVector) {
        Self::InterruptCommandHigh.write(0);
        Self::InterruptCommand.write(0x000C4000 | vec.0 as u32);
    }

    #[inline]
    unsafe fn current_processor_id() -> ProcessorId {
        ProcessorId((LocalApic::Id.read() >> 24) as u8)
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
    unsafe fn new(acpi_ioapic: &acpi::platform::IoApic) -> Self {
        let mut ioapic = IoApic {
            mmio: Mmio::from_phys(acpi_ioapic.address as PhysicalAddress, 0x14).unwrap(),
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
        without_interrupts!({
            self.lock.synchronized(|| {
                self.mmio.write_u8(0x00, index.0);
                self.mmio.read_u32(0x10)
            })
        })
    }

    unsafe fn write(&mut self, index: IoApicIndex, data: u32) {
        without_interrupts!({
            self.lock.synchronized(|| {
                self.mmio.write_u8(0x00, index.0);
                self.mmio.write_u32(0x10, data);
            });
        });
    }
}
